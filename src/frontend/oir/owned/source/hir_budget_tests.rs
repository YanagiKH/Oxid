//! C3a count-only controls. These never construct enum HIR or a source witness.
use super::*;
use crate::frontend::{
    declaration_index::{collect_enum_candidate, IndexLimits, SourceOwner},
    lexer, parser,
    source::{SourceFileId, SourceMap, SourceView},
};
use std::mem::{align_of, size_of};

fn parse(sources: &SourceMap) -> ast::Program {
    let file = sources.get(SourceFileId(0));
    parser::parse_enum_candidate_counted(
        file,
        lexer::lex(file).unwrap(),
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut Allocator::default(),
        &mut Default::default(),
    )
    .unwrap()
    .0
}
fn sources(text: &str) -> SourceMap {
    let mut sources = SourceMap::new();
    sources.add("hir-budget.ox".into(), text.into());
    sources
}
fn with_index<T>(text: &str, action: impl FnOnce(&DeclarationIndex<'_>) -> T) -> T {
    let sources = sources(text);
    let ast = parse(&sources);
    let owner = SourceOwner::original(
        sources.get(SourceFileId(0)),
        &ast,
        SourceView::Map(&sources),
    )
    .unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_enum_candidate(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
    action(&index)
}
// Independent test-side composition, with a separate frozen measured tuple
// checked below. This tuple is test machinery, never a production pricing table.
fn checker_only_components() -> (usize, usize, usize, usize, usize) {
    use typeck::semantic_carriers::*;
    let iterators = typeck::semantic_iterator_layout();
    let map_or = typeck::semantic_map_or_layout();
    let fixed = size_of::<typeck::PaidProgramControls>()
        + size_of::<ProjectionSemanticCarriers>()
        + size_of::<PredicateInvocationCarriers>()
        + iterators[0]
        + iterators[2]
        + map_or[2]
        + size_of::<type_storage::MeteredProjectionWorkControls>()
        + MAX_NESTING
            * (size_of::<ExpressionSemanticCarriers>()
                + size_of::<typeck::PaidExpressionReborrows>());
    let function = size_of::<typeck::PaidBodyControls>()
        + size_of::<typeck::PaidBodyReceivers>()
        + size_of::<StatementPatternCarriers>()
        + size_of::<BodyFrameSemanticCarriers>()
        + size_of::<InitializerSemanticCarriers>()
        + map_or[0];
    (
        fixed,
        function,
        size_of::<typeck::PaidRowReceiver>(),
        size_of::<CallSemanticCarriers>(),
        size_of::<LiteralSemanticCarriers>(),
    )
}

// Independent test-side A–D composition. Its tuple and grouped additions are
// test machinery, not a production price array or source observation.
fn observation_components() -> (usize, usize, usize, usize) {
    let construction = resolve::fresh_type_observation_carrier_bytes()
        + resolve::type_storage_cell_carrier_bytes()
        + typeck::borrowed_type_observation_carrier_bytes();
    let inventory = type_storage::inventory_construction_carrier_bytes()
        + type_storage::retained_sample_carrier_bytes()
        + type_storage::typed_reconciliation_carrier_bytes()
        + typeck::body_inventory_carrier_bytes()
        + typeck::path_inventory_carrier_bytes()
        + typeck::inventory_sum_carrier_bytes();
    let samples = type_storage::observed_construction_carrier_bytes()
        + type_storage::sample_carrier_bytes()
        + type_storage::endpoint_sample_carrier_bytes()
        + type_storage::path_sample_carrier_bytes()
        + type_storage::completion_carrier_bytes()
        + type_storage::quota_completion_carrier_bytes()
        + type_storage::plan_completion_carrier_bytes()
        + type_storage::counts_access_carrier_bytes()
        + type_storage::sample_sizing_carrier_bytes();
    (
        construction
            + inventory
            + samples
            + typeck::program_observation_control_bytes()
            + typeck::projection_sampling_control_bytes(),
        typeck::body_completion_control_bytes() + typeck::body_sampling_control_bytes(),
        typeck::call_sampling_control_bytes(),
        typeck::literal_sampling_control_bytes(),
    )
}

// New denied-pipeline controls remain separate from the old T0/checker and
// fixed-statistics components. Each concrete bank has its own layout receipt.
fn pipeline_components() -> usize {
    resolve::enum_pipeline_source_extra_bytes()
        + typeck::enum_pipeline_type_carrier_bytes()
        + super::super::program::enum_pipeline_program_carrier_bytes()
}

const MIXED: &str = "enum Token { Number(i32), End } struct R { x:i32, y:bool } fn plain(a:i32)->i32{return a;} fn main()->i32{let token=Token::Number(plain(7));match token{Token::Number(value)=>{return value;},Token::End=>{return 0;},}}";

#[test]
fn c3a_counts_complete_mixed_hir_without_heap_allocation_or_activation() {
    with_index(MIXED, |index| {
        let work = WorkMeter::default();
        let (result, stats) = super::super::reviewer_source::integration_measured(|| {
            preflight_enum_hir(index, &work)
        });
        let plan = result.unwrap().unwrap();
        assert_eq!(stats, (0, 0, 0));
        assert_eq!(
            plan.counts,
            HirCounts {
                records: 1,
                record_fields: 2,
                max_record_fields: 2,
                functions: 2,
                parameters: 1,
                bindings: 3,
                expressions: 6,
                blocks: 4,
                statements: 5,
                calls: 2,
                call_arguments: 2,
                matches: 1,
                match_arms: 2,
                scope_marks: 3,
                loop_slots: 3,
                resolve_frames: 28,
                type_frames: 25,
                max_expression_depth: 3,
                max_body_depth: 2,
                ..HirCounts::default()
            }
        );
        assert!(plan.total < MAX_HIR_BYTES);
        assert!(work.used() > 0);
        // Counting is no bypass for old scalar or owned index-taking paths.
        assert_eq!(
            index.require_current_source_pipeline().unwrap_err().code,
            "E0101"
        );
        assert_eq!(
            resolve::resolve_project(index, &work).unwrap_err()[0].code,
            "E0101"
        );
        assert_eq!(
            crate::frontend::hir::resolve_project(index, &work).unwrap_err()[0].code,
            "E0101"
        );
    });
}

#[test]
fn c3a_zero_enum_selector_leaves_legacy_work_and_admission_untouched() {
    // Candidate syntax alone does not choose a new HIR admission lane.
    for text in [
        "fn main()->i32{return 0;}",
        "fn main()->(){match x{Missing::V=>{return;}}}",
    ] {
        with_index(text, |index| {
            let work = WorkMeter::new(0);
            let (result, stats) = super::super::reviewer_source::integration_measured(|| {
                preflight_enum_hir(index, &work)
            });
            assert!(result.unwrap().is_none());
            assert_eq!(work.used(), 0);
            assert_eq!(stats, (0, 0, 0));
        });
    }
    with_index(
        "enum Unused{V} struct R{x:i32} fn plain(a:i32)->i32{return a;} fn main()->i32{return 0;}",
        |index| {
            let plan = preflight_enum_hir(index, &WorkMeter::default())
                .unwrap()
                .unwrap();
            assert_eq!(
                (
                    plan.counts.records,
                    plan.counts.record_fields,
                    plan.counts.functions,
                    plan.counts.parameters
                ),
                (1, 1, 2, 1)
            );
            assert_eq!((plan.counts.matches, plan.counts.match_arms), (0, 0));
        },
    );
}

#[test]
fn c3a_nested_arguments_presence_and_assignment_target_are_conservative() {
    with_index("enum Unused{V} struct R{x:i32,y:i32,z:i32} fn one(x:i32)->i32{return x;} fn pair(a:i32,b:i32)->i32{return a;} fn main()->i32{let bad=R{}; let a=[1]; a[0]=pair(one(1),one(2));return 0;}", |index| {
        let plan = preflight_enum_hir(index, &WorkMeter::default()).unwrap().unwrap();
        assert_eq!((plan.counts.calls, plan.counts.call_arguments), (3, 4));
        assert_eq!((plan.counts.record_literals, plan.counts.field_initializers, plan.counts.max_record_fields), (1, 0, 3));
        assert_eq!((plan.counts.array_literals, plan.counts.array_entries), (1, 1));
        // 2 helper returns + R{} + [1] + target IndexRead/index + nested
        // pair/one/one/two integers + main return = 13 visits. The target
        // wrapper is a conservative extra row, never an actual HIR load.
        assert_eq!(plan.counts.expressions, 13);
        assert!(plan.typeck_scratch >= 4 * size_of::<(ParameterTy, Span)>() + 3);
    });
}

#[test]
fn c3a_expression_dag_visits_are_counted_with_multiplicity() {
    // Adversarial unsealed syntax probe, not a forged immutable index/witness.
    let sources = sources("enum Unused{V} fn main()->i32{return 1+2+3+4;}");
    let mut ast = parse(&sources);
    for expression in &mut ast.expressions {
        if let ast::ExprKind::Arithmetic { left, right, .. } = &mut expression.kind {
            *right = *left;
        }
    }
    assert_eq!(ast.expressions.len(), 7);
    let mut counts = HirCounts::default();
    count_function(&ast, &ast.functions[0], &WorkMeter::default(), &mut counts).unwrap();
    // 1 + 2 * (1 + 2 * (1 + 2 * 1)), rather than seven arena rows.
    assert_eq!(counts.expressions, 15);
    assert_eq!(counts.max_expression_depth, 4);
}

#[test]
fn c3a_block_tree_certificate_rejects_shared_and_unreachable_arenas() {
    for shared in [false, true] {
        let sources = sources("enum Unused{V} fn main()->i32{if true{return 1;}else{return 2;}}");
        let mut ast = parse(&sources);
        let ast::StmtKind::If {
            then_block,
            else_block,
            ..
        } = &mut ast.functions[0].blocks[0].body[0].kind
        else {
            panic!()
        };
        *else_block = if shared { Some(*then_block) } else { None };
        let mut counts = HirCounts::default();
        let error = count_function(&ast, &ast.functions[0], &WorkMeter::default(), &mut counts)
            .unwrap_err();
        assert_eq!(error.code, "E0500");
    }
}

#[test]
fn c3a_preflight_work_is_bounded_before_affected_reservations() {
    with_index(MIXED, |index| {
        let measured = WorkMeter::default();
        preflight_enum_hir(index, &measured).unwrap();
        let required = measured.used();
        for (limit, accepted) in [
            (required - 1, false),
            (required, true),
            (required + 1, true),
        ] {
            let work = WorkMeter::new(limit);
            let result = preflight_enum_hir(index, &work);
            assert_eq!(result.is_ok(), accepted);
            assert!(work.used() <= limit);
            if let Err(error) = result {
                assert_eq!(error.code, "E0400");
            }
        }
    });
}

#[test]
fn c3a_structural_and_dynamic_bytes_share_one_unchanged_ceiling() {
    with_index(MIXED, |index| {
        let plan = preflight_enum_hir(index, &WorkMeter::default())
            .unwrap()
            .unwrap();
        let at = index.sources().eof();
        let available = MAX_HIR_BYTES - plan.total;
        assert_eq!(
            plan.with_dynamic(available - 1, at).unwrap(),
            MAX_HIR_BYTES - 1
        );
        assert_eq!(plan.with_dynamic(available, at).unwrap(), MAX_HIR_BYTES);
        assert!(plan.with_dynamic(available + 1, at).is_err());
        assert!(plan.with_dynamic(usize::MAX, at).is_err());
        assert_eq!(
            admit(MAX_HIR_BYTES, 0, usize::MAX, at).unwrap(),
            MAX_HIR_BYTES
        );
        assert!(admit(MAX_HIR_BYTES, 1, usize::MAX, at).is_err());
        assert!(add(usize::MAX, 1, at).is_err());
        assert!(mul(usize::MAX, 2, at).is_err());
        assert!(Capacity::coexist::<u64>(usize::MAX, 1, at).is_err());
        assert!(HirPlan::calculate(
            HirCounts {
                expressions: usize::MAX,
                ..HirCounts::default()
            },
            at
        )
        .is_err());
        assert_eq!(
            plan.total,
            plan.resolved
                + plan.typed
                + plan.staging
                + plan.resolver_scratch
                + plan.typeck_scratch
                + plan.fixed
                + plan.lower_fixed
        );
    });
}

#[test]
fn c3a_arm_rows_and_option_conversion_are_complete_actual_carriers() {
    let sources = sources("x");
    let at = sources.get(SourceFileId(0)).span(0, 1);
    let empty = HirPlan::calculate(HirCounts::default(), at).unwrap();
    let one_arm = HirPlan::calculate(
        HirCounts {
            match_arms: 1,
            ..HirCounts::default()
        },
        at,
    )
    .unwrap();
    assert_eq!(one_arm.total - empty.total, size_of::<MatchArm>());
    let one = HirPlan::calculate(
        HirCounts {
            expressions: 1,
            bindings: 1,
            blocks: 1,
            statements: 1,
            ..HirCounts::default()
        },
        at,
    )
    .unwrap();
    assert_eq!(
        one.staging,
        size_of::<Option<ValueTy>>()
            + size_of::<Option<ParameterTy>>()
            + size_of::<Option<typeck::FlowSummary>>()
    );
    assert_eq!(
        one.typed,
        size_of::<ValueTy>()
            + size_of::<ParameterTy>()
            + size_of::<typeck::FlowSummary>()
            + 2 * size_of::<Option<Projection>>()
            + size_of::<Vec<Option<Projection>>>()
    );
    assert_eq!(size_of::<typeck::TypedBody>(), 6 * size_of::<Vec<()>>());
}

#[test]
fn c3a_prepaid_capacity_uses_length_and_rejects_excess_before_retention() {
    let sources = sources("x");
    let at = sources.get(SourceFileId(0)).span(0, 1);
    let mut values = Vec::<u64>::with_capacity(10);
    values.extend([1, 2]);
    let mut allocator = Allocator::default();
    let ticket = Capacity::new::<u64>(12, 12 * size_of::<u64>(), at).unwrap();
    let values = ticket
        .reserve(&mut allocator, values, at, "C3a capacity control")
        .unwrap();
    assert_eq!((values.len(), values.capacity()), (2, 12));
    assert_eq!(allocator.trace[0].length, 12);
    assert_eq!(allocator.trace[0].element_bytes, size_of::<u64>());
    assert_eq!(
        Capacity::coexist::<u64>(10, 12, at).unwrap(),
        22 * size_of::<u64>()
    );
    assert!(ticket.check_observed(13, at).is_err());
    assert!(Capacity::new::<u64>(12, 12 * size_of::<u64>() - 1, at).is_err());
    assert!(Capacity::new::<u64>(usize::MAX, usize::MAX, at).is_err());
    let attempts = allocator.attempts;
    assert!(Capacity::new::<u64>(1, size_of::<u64>(), at)
        .unwrap()
        .reserve(&mut allocator, values, at, "oversize input")
        .is_err());
    assert_eq!(allocator.attempts, attempts);
}

#[test]
fn c3a_exact_capacity_zero_and_real_failures_leave_no_retained_payload() {
    let sources = sources("x");
    let at = sources.get(SourceFileId(0)).span(0, 1);
    for slots in [0, 1, 32] {
        for fail in [false, true] {
            let mut allocator = Allocator {
                fail_at: fail.then_some(1),
                ..Allocator::default()
            };
            allocator.observer_trace_bound(1).unwrap();
            let ticket = Capacity::new::<u64>(slots, MAX_HIR_BYTES, at).unwrap();
            let ((), (_, live, _)) = super::super::reviewer_source::integration_measured(|| {
                let result =
                    ticket.reserve(&mut allocator, Vec::<u64>::new(), at, "C3a exact capacity");
                assert_eq!(result.is_err(), fail);
                if let Ok(ref values) = result {
                    assert_eq!(values.capacity(), slots);
                }
                drop(result);
            });
            assert_eq!(live, 0);
            assert_eq!(allocator.attempts, 1);
            assert_eq!(allocator.trace.len(), 1);
            assert_eq!(allocator.trace[0].success, !fail);
        }
    }
}

#[test]
fn c3a_actual_source_hir_cache_staging_and_scope_layouts() {
    macro_rules! layout { ($($ty:ty),* $(,)?) => { $(println!("C3A_LAYOUT {} bytes={} align={}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());)* }; }
    layout!(
        ast::Program,
        ast::ExprKind,
        ast::Expr,
        ast::StmtKind,
        ast::Stmt,
        ast::Function,
        ast::BodyBlock,
        ast::MatchArmSyntax,
        Record,
        Field,
        FieldId,
        Signature,
        Function,
        Binding,
        ExprKind,
        Expr,
        StmtKind,
        Stmt,
        BodyBlock,
        MatchArm,
        Argument,
        FieldInit,
        Projection,
        ValueTy,
        Option<ValueTy>,
        ParameterTy,
        Option<ParameterTy>,
        Option<Projection>,
        typeck::FlowSummary,
        Option<typeck::FlowSummary>,
        typeck::TypedBody,
        typeck::BorrowProjection,
        resolve::ResolvedOwnedProgram<'_>,
        typeck::TypedOwnedProgram<'_>,
        typeck::TypedOwnedFunction<'_>,
        resolve::ResolveFrame,
        typeck::TypeFrame,
        ScopeName,
        ScopeStorage,
        HirCounts,
        HirPlan,
        Option<HirPlan>,
        Result<HirPlan, Box<Diagnostic>>,
        Result<Option<HirPlan>, Box<Diagnostic>>,
        Capacity,
        Result<Capacity, Box<Diagnostic>>,
        Result<Vec<u64>, Box<Diagnostic>>,
        Result<Vec<MatchArm>, Box<Diagnostic>>,
        Result<(), Box<Diagnostic>>,
        Result<usize, Box<Diagnostic>>,
        Result<u64, Box<Diagnostic>>,
        PlanReturnEnvelope,
        CapacityReturnEnvelope,
        CursorTemporaries,
        ScalarReturnEnvelope,
        VectorReturnEnvelope<u64>,
        VectorReturnEnvelope<MatchArm>,
        ExprCursor,
        BlockCursor,
        [Option<ExprCursor>; MAX_NESTING],
        [Option<BlockCursor>; MAX_BLOCK_NESTING],
        Vec<Option<Projection>>,
        (ParameterTy, Span),
        lower::BindingLocation,
        Option<lower::BindingLocation>,
        lower::EvaluatedValue,
        Option<lower::EvaluatedValue>,
    );
    println!(
        "C3A_LAYOUT resolver-header={} lower-fixed-parts={:?}",
        resolve::resolver_carrier_bytes(),
        lower::fixed_carrier_bytes()
    );
}

#[test]
fn c3a_imported_unused_enum_selects_complete_index() {
    use crate::frontend::project::{ProjectLimits, ProjectSources};
    struct Files(std::path::PathBuf);
    impl Drop for Files {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let files = Files(std::env::temp_dir().join(format!("oxid-c3a-index-{}", std::process::id())));
    std::fs::create_dir(&files.0).unwrap();
    std::fs::write(
        files.0.join("main.ox"),
        "mod child; use crate::child::Unused as Alias; fn main()->i32{return 0;}",
    )
    .unwrap();
    std::fs::write(files.0.join("child.ox"), "pub enum Unused{V}").unwrap();
    let project = ProjectSources::load_enum_index_candidate(
        files.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    let mut allocator = Allocator::default();
    let work = WorkMeter::default();
    let index = collect_enum_candidate(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap();
    assert_eq!(index.enum_count(), 1);
    let probe = WorkMeter::default();
    let (plan, stats) =
        super::super::reviewer_source::integration_measured(|| preflight_enum_hir(&index, &probe));
    assert_eq!(stats, (0, 0, 0));
    let plan = plan.unwrap().unwrap();
    assert_eq!(
        (
            plan.counts.functions,
            plan.counts.expressions,
            plan.counts.matches
        ),
        (1, 1, 0)
    );
    assert!(probe.used() > 0);
    assert_eq!(
        index.require_current_source_pipeline().unwrap_err().code,
        "E0101"
    );
}

#[test]
fn c3a_complete_fallible_return_envelopes_and_copies_are_prepaid() {
    let sources = sources("x");
    let at = sources.get(SourceFileId(0)).span(0, 1);
    let plan = HirPlan::calculate(HirCounts::default(), at).unwrap();
    // Independent member sums check that whole modeled carriers cover all
    // complete wrapper payloads/padding, not only their success values.
    let plans = 2 * size_of::<HirCounts>()
        + 4 * size_of::<HirPlan>()
        + size_of::<Result<HirPlan, Box<Diagnostic>>>()
        + size_of::<Option<HirPlan>>()
        + size_of::<Result<Option<HirPlan>, Box<Diagnostic>>>();
    let capacities = 4 * size_of::<Capacity>() + size_of::<Result<Capacity, Box<Diagnostic>>>();
    let cursors = size_of::<ExprCursor>()
        + size_of::<Option<ExprCursor>>()
        + size_of::<Option<ast::ExprId>>()
        + size_of::<BlockCursor>()
        + size_of::<Option<BlockCursor>>()
        + size_of::<Option<ast::BodyBlockId>>();
    let scalars = 5 * size_of::<Result<(), Box<Diagnostic>>>()
        + 3 * size_of::<Result<usize, Box<Diagnostic>>>()
        + size_of::<Result<u64, Box<Diagnostic>>>();
    assert!(size_of::<PlanReturnEnvelope>() >= plans);
    assert!(size_of::<CapacityReturnEnvelope>() >= capacities);
    assert!(size_of::<CursorTemporaries>() >= cursors);
    assert!(size_of::<ScalarReturnEnvelope>() >= scalars);
    assert_eq!(
        plan.fixed,
        size_of::<typeck::TypedOwnedProgram<'_>>()
            + size_of::<PlanReturnEnvelope>()
            + size_of::<CapacityReturnEnvelope>()
            + size_of::<CursorTemporaries>()
            + size_of::<ScalarReturnEnvelope>()
            + resolver_storage::fixed_carrier_bytes()
            + VECTOR_RETURN_ENVELOPE_BYTES
            + type_storage::fixed_control_carrier_bytes()
            + typeck::borrowed_check_carrier_bytes()
            + resolve::denied_type_probe_carrier_bytes()
            + checker_only_components().0
            + observation_components().0
            + pipeline_components()
            + size_of::<[Option<ExprCursor>; MAX_NESTING]>()
            + size_of::<[Option<BlockCursor>; MAX_BLOCK_NESTING]>()
    );
    // The generic return contains a second complete Vec header while local
    // input storage may still exist. Check every currently budgeted row type.
    macro_rules! vector {
        ($($ty:ty),* $(,)?) => { $(
            let returned = size_of::<Result<Vec<$ty>, Box<Diagnostic>>>();
            let envelope = size_of::<VectorReturnEnvelope<$ty>>();
            assert!(envelope >= size_of::<Vec<$ty>>() + returned);
            assert!(envelope <= VECTOR_RETURN_ENVELOPE_BYTES);
            println!("C3A_VECTOR_RETURN {} result={} envelope={} align={}",
                stringify!($ty), returned, envelope,
                align_of::<VectorReturnEnvelope<$ty>>());
        )* };
    }
    vector!(
        Record,
        Field,
        FieldId,
        Signature,
        Function,
        ParameterTy,
        Binding,
        Expr,
        BodyBlock,
        Stmt,
        Argument,
        FieldInit,
        ExprId,
        MatchArm,
        typeck::TypedBody,
        ValueTy,
        Option<Projection>,
        typeck::FlowSummary,
        Vec<Option<Projection>>,
        typeck::BorrowProjection,
        Option<ValueTy>,
        Option<ParameterTy>,
        Option<typeck::FlowSummary>,
        ScopeName,
        usize,
        LoopId,
        resolve::ResolveFrame,
        typeck::TypeFrame,
        (ParameterTy, Span),
        bool,
        u64,
    );
    // These models must not increase the numeric ceiling or receive a second
    // allowance: their additional complete bytes reduce dynamic headroom.
    let remaining = MAX_HIR_BYTES - plan.total;
    assert_eq!(plan.with_dynamic(remaining, at).unwrap(), MAX_HIR_BYTES);
    assert!(plan.with_dynamic(remaining + 1, at).is_err());
    println!("C3A_FIXED_RETURN_ENVELOPE plan={} capacity={} cursor={} scalar={} vector={} fixed={} total={}",
        size_of::<PlanReturnEnvelope>(), size_of::<CapacityReturnEnvelope>(),
        size_of::<CursorTemporaries>(), size_of::<ScalarReturnEnvelope>(),
        VECTOR_RETURN_ENVELOPE_BYTES, plan.fixed, plan.total);
}

#[test]
fn c3a_paid_branch_header_formula_keeps_old_and_new_buffers_separate() {
    let sources = sources("x");
    let at = sources.get(SourceFileId(0)).span(0, 1);
    // Synthetic arithmetic-only counters isolate 3F + B + Calls extra headers.
    // No source traversal, allocations or inferred consumer output is involved.
    let counts = HirCounts {
        functions: 2,
        blocks: 3,
        calls: 4,
        ..HirCounts::default()
    };
    let plan = HirPlan::calculate(counts, at).unwrap();
    let existing = 2
        * (resolver_storage::function_carrier_bytes()
            + resolve::resolver_carrier_bytes()
            + size_of::<Vec<LoopId>>()
            + size_of::<Vec<resolve::ResolveFrame>>()
            + size_of::<Vec<ParameterTy>>()
            + size_of::<Vec<BodyBlock>>())
        + 4 * size_of::<Vec<Argument>>();
    let added = 2 * resolver_storage::function_branch_header_bytes()
        + 3 * size_of::<Vec<Stmt>>()
        + 4 * size_of::<Vec<Argument>>();
    assert_eq!(
        added,
        2 * (size_of::<Vec<ParameterTy>>()
            + size_of::<Vec<BodyBlock>>()
            + size_of::<Vec<resolve::ResolveFrame>>())
            + 3 * size_of::<Vec<Stmt>>()
            + 4 * size_of::<Vec<Argument>>()
    );
    assert_eq!(plan.resolver_scratch, existing + added);
    println!("C3A_PAID_BRANCH_HEADER_FORMULA functions=2 blocks=3 calls=4 existing={existing} added={added} resolver_scratch={}", plan.resolver_scratch);
}

#[test]
fn c3_t0_passive_controls_add_one_fixed_bank_and_exact_function_outputs() {
    let at = sources("x").get(SourceFileId(0)).span(0, 1);
    let base = HirPlan::calculate(HirCounts::default(), at).unwrap();
    let legacy_fixed = size_of::<typeck::TypedOwnedProgram<'_>>()
        + size_of::<PlanReturnEnvelope>()
        + size_of::<CapacityReturnEnvelope>()
        + size_of::<CursorTemporaries>()
        + size_of::<ScalarReturnEnvelope>()
        + resolver_storage::fixed_carrier_bytes()
        + VECTOR_RETURN_ENVELOPE_BYTES
        + size_of::<[Option<ExprCursor>; MAX_NESTING]>()
        + size_of::<[Option<BlockCursor>; MAX_BLOCK_NESTING]>();
    assert_eq!(
        base.fixed
            - legacy_fixed
            - typeck::borrowed_check_carrier_bytes()
            - resolve::denied_type_probe_carrier_bytes()
            - checker_only_components().0
            - observation_components().0
            - pipeline_components(),
        type_storage::fixed_control_carrier_bytes()
    );
    for functions in [0, 1, 2] {
        let plan = HirPlan::calculate(
            HirCounts {
                functions,
                ..HirCounts::default()
            },
            at,
        )
        .unwrap();
        assert_eq!(plan.fixed, base.fixed);
        assert_eq!(
            plan.typeck_scratch,
            functions
                * (size_of::<Vec<typeck::TypeFrame>>()
                    + size_of::<type_storage::FunctionQuota>()
                    + typeck::borrowed_body_return_carrier_bytes()
                    + checker_only_components().1
                    + observation_components().1)
        );
    }
    for count in [0, 1, 2] {
        let calls = HirPlan::calculate(
            HirCounts {
                calls: count,
                ..HirCounts::default()
            },
            at,
        )
        .unwrap();
        assert_eq!(calls.fixed, base.fixed);
        assert_eq!(
            calls.typeck_scratch,
            count
                * (size_of::<Vec<(ParameterTy, Span)>>()
                    + checker_only_components().3
                    + observation_components().2)
        );
        let literals = HirPlan::calculate(
            HirCounts {
                record_literals: count,
                max_record_fields: 3,
                ..HirCounts::default()
            },
            at,
        )
        .unwrap();
        assert_eq!(literals.fixed, base.fixed);
        assert_eq!(
            literals.typeck_scratch,
            count
                * (3 * size_of::<bool>()
                    + size_of::<Vec<bool>>()
                    + checker_only_components().4
                    + observation_components().3)
        );
        let other = HirPlan::calculate(
            HirCounts {
                expressions: count,
                statements: count,
                borrow_arguments: count,
                ..HirCounts::default()
            },
            at,
        )
        .unwrap();
        assert_eq!(other.fixed, base.fixed);
        assert_eq!(other.typeck_scratch, 0);
    }
}

#[test]
fn c3_t0_new_surcharge_uses_the_existing_checked_capacity_boundary() {
    let at = sources("x").get(SourceFileId(0)).span(0, 1);
    let plan = HirPlan::calculate(
        HirCounts {
            functions: 2,
            ..HirCounts::default()
        },
        at,
    )
    .unwrap();
    let remaining = MAX_HIR_BYTES - plan.total;
    assert_eq!(
        plan.with_dynamic(remaining - 1, at).unwrap(),
        MAX_HIR_BYTES - 1
    );
    assert_eq!(plan.with_dynamic(remaining, at).unwrap(), MAX_HIR_BYTES);
    assert!(plan.with_dynamic(remaining + 1, at).is_err());
    let mut unchanged = 7;
    assert!(charge::<type_storage::FunctionQuota>(&mut unchanged, usize::MAX, at).is_err());
    assert_eq!(unchanged, 7);
    let mut near_overflow = usize::MAX;
    assert!(increment(
        &mut near_overflow,
        type_storage::fixed_control_carrier_bytes(),
        at
    )
    .is_err());
    assert_eq!(near_overflow, usize::MAX);
    assert!(HirPlan::calculate(
        HirCounts {
            functions: usize::MAX,
            ..HirCounts::default()
        },
        at
    )
    .is_err());
}

#[test]
fn c3_t0_real_enum_free_index_does_not_enter_the_new_control_admission() {
    with_index("struct Empty{} struct Wide{a:i32,b:bool} fn helper()->(){Empty{};return;} fn main()->i32{return 0;}", |index| {
        let work = WorkMeter::new(0);
        let (result, stats) = super::super::reviewer_source::integration_measured(|| preflight_enum_hir(index, &work));
        assert!(result.unwrap().is_none());
        assert_eq!(work.used(), 0);
        assert_eq!(stats, (0, 0, 0));
    });
}

#[test]
fn c3_t1_passive_checker_components_have_independent_measured_slopes() {
    let at = sources("x").get(SourceFileId(0)).span(0, 1);
    let components = checker_only_components();
    #[cfg(target_pointer_width = "64")]
    // Stage C's observation borrow remains. The measured enum constructor bank
    // adds 152 per supported expression depth; Match adds 472 per function.
    assert_eq!(
        components,
        (
            33448 + 64 * 152,
            1920 + size_of::<&mut type_storage::TypedObserved>() + 472,
            24,
            784,
            88
        )
    );
    assert_eq!(MAX_NESTING, 64);
    let base = HirPlan::calculate(HirCounts::default(), at).unwrap();
    for count in [0, 1, 2] {
        for which in 0..4 {
            let mut c = HirCounts::default();
            match which {
                0 => c.functions = count,
                1 => c.blocks = count,
                2 => c.calls = count,
                _ => c.record_literals = count,
            }
            let plan = HirPlan::calculate(c, at).unwrap();
            assert_eq!(plan.fixed, base.fixed);
            let old = c.functions
                * (size_of::<Vec<typeck::TypeFrame>>()
                    + size_of::<type_storage::FunctionQuota>()
                    + typeck::borrowed_body_return_carrier_bytes())
                + c.calls * size_of::<Vec<(ParameterTy, Span)>>()
                + c.record_literals * size_of::<Vec<bool>>();
            let observation = observation_components();
            let observed = c.functions * observation.1
                + c.calls * observation.2
                + c.record_literals * observation.3;
            assert_eq!(
                plan.typeck_scratch - old - observed,
                c.functions * components.1
                    + c.blocks * components.2
                    + c.calls * components.3
                    + c.record_literals * components.4
            );
        }
        // The depth bank is one fixed64 bound, not F times depth, and does not
        // silently trust a supplied synthetic max_expression_depth as proof.
        let plan = HirPlan::calculate(
            HirCounts {
                max_expression_depth: count,
                expressions: count,
                statements: count,
                borrow_arguments: count,
                ..HirCounts::default()
            },
            at,
        )
        .unwrap();
        assert_eq!(plan.fixed, base.fixed);
        assert_eq!(plan.typeck_scratch, 0);
    }
    println!(
        "C3_T1_CHECKER_ONLY_FORMULA fixed={} function={} block={} call={} literal={}",
        components.0, components.1, components.2, components.3, components.4
    );
}

#[test]
fn c3_t1_passive_checker_products_and_aggregate_cap_remain_checked() {
    let at = sources("x").get(SourceFileId(0)).span(0, 1);
    let components = checker_only_components();
    for width in [
        components.1,
        components.2,
        components.3,
        components.4,
        size_of::<typeck::semantic_carriers::ExpressionSemanticCarriers>()
            + size_of::<typeck::PaidExpressionReborrows>(),
    ] {
        let overflow = usize::MAX / width + 1;
        assert_eq!(mul(overflow, width, at).unwrap_err().code, "E0400");
    }
    macro_rules! unchanged {
        ($($ty:ty),* $(,)?) => { $(
            let mut bytes = 7;
            assert_eq!(charge::<$ty>(&mut bytes, usize::MAX, at).unwrap_err().code, "E0400");
            assert_eq!(bytes, 7);
        )* };
    }
    unchanged!(
        typeck::PaidBodyControls,
        typeck::PaidBodyReceivers,
        typeck::PaidRowReceiver,
        typeck::semantic_carriers::CallSemanticCarriers,
        typeck::semantic_carriers::LiteralSemanticCarriers,
        typeck::semantic_carriers::ExpressionSemanticCarriers
    );
    let mut bytes = usize::MAX;
    assert!(increment(&mut bytes, components.0, at).is_err());
    assert_eq!(bytes, usize::MAX);
    let plan = HirPlan::calculate(
        HirCounts {
            functions: 2,
            blocks: 3,
            calls: 4,
            record_literals: 5,
            max_record_fields: 7,
            ..HirCounts::default()
        },
        at,
    )
    .unwrap();
    let remainder = MAX_HIR_BYTES - plan.total;
    assert_eq!(
        plan.with_dynamic(remainder - 1, at).unwrap(),
        MAX_HIR_BYTES - 1
    );
    assert_eq!(plan.with_dynamic(remainder, at).unwrap(), MAX_HIR_BYTES);
    assert_eq!(
        plan.with_dynamic(remainder + 1, at).unwrap_err().code,
        "E0400"
    );
    for c in [
        HirCounts {
            functions: usize::MAX,
            ..HirCounts::default()
        },
        HirCounts {
            blocks: usize::MAX,
            ..HirCounts::default()
        },
        HirCounts {
            calls: usize::MAX,
            ..HirCounts::default()
        },
        HirCounts {
            record_literals: usize::MAX,
            ..HirCounts::default()
        },
    ] {
        assert_eq!(HirPlan::calculate(c, at).unwrap_err().code, "E0400");
    }
}

#[test]
fn c3_t1_passive_checker_price_does_not_change_enum_free_selection_or_denial() {
    for text in [
        "fn main()->(){return;}",
        "struct R{x:i32} fn helper(a:i32)->i32{return a;} fn main()->i32{R{x:helper(1)};return 0;}",
        "fn main()->(){match missing{E::V=>{return;}}}",
    ] {
        with_index(text, |index| {
            let work = WorkMeter::new(0);
            work.enable_observation();
            let mut allocator = Allocator {
                attempts: 7,
                ..Allocator::default()
            };
            let (result, measured) = super::super::reviewer_source::integration_measured(|| {
                let plan = preflight_enum_hir(index, &work).unwrap();
                let denied_probe =
                    resolve::probe_enum_type_storage(index, &work, &mut allocator).unwrap();
                (plan, denied_probe)
            });
            assert!(result.0.is_none() && result.1.is_none());
            assert_eq!(measured, (0, 0, 0));
            assert_eq!((work.used(), allocator.attempts), (0, 7));
            assert!(work.events.borrow().is_empty());
            assert!(work.observations.borrow().is_empty());
        });
    }
    with_index("enum Unused{V} fn main()->(){return;}", |index| {
        let work = WorkMeter::new(0);
        let mut allocator = Allocator {
            attempts: 7,
            ..Allocator::default()
        };
        let errors = resolve::probe_enum_type_storage(index, &work, &mut allocator).unwrap_err();
        // Enum-bearing selection now enters preflight, still before reserves.
        assert_eq!((errors.len(), errors[0].code), (1, "E0400"));
        assert_eq!(errors[0].stage, "resolve-project");
        assert_eq!(errors[0].message, "declaration index work limit exceeded");
        assert_eq!((work.used(), allocator.attempts), (0, 7));
        assert_eq!(
            resolve::resolve_project(index, &work).unwrap_err()[0].code,
            "E0101"
        );
    });
}

#[test]
fn c3_t1_observation_price_has_independent_fixed_and_mixed_source_slopes() {
    let at = sources("x").get(SourceFileId(0)).span(0, 1);
    let observation = observation_components();
    #[cfg(target_pointer_width = "64")]
    // MatchArms extends each complete constructed/Option/Result observation
    // in FreshTypeObservationCarriers by two usize fields: 3 * 16 bytes.
    assert_eq!(observation, (11792 + 3 * 16, 320, 24, 24));
    let checker = checker_only_components();
    let base = HirPlan::calculate(HirCounts::default(), at).unwrap();
    let before_observation = size_of::<typeck::TypedOwnedProgram<'_>>()
        + size_of::<PlanReturnEnvelope>()
        + size_of::<CapacityReturnEnvelope>()
        + size_of::<CursorTemporaries>()
        + size_of::<ScalarReturnEnvelope>()
        + resolver_storage::fixed_carrier_bytes()
        + VECTOR_RETURN_ENVELOPE_BYTES
        + type_storage::fixed_control_carrier_bytes()
        + typeck::borrowed_check_carrier_bytes()
        + resolve::denied_type_probe_carrier_bytes()
        + checker.0
        + pipeline_components()
        + size_of::<[Option<ExprCursor>; MAX_NESTING]>()
        + size_of::<[Option<BlockCursor>; MAX_BLOCK_NESTING]>();
    assert_eq!(base.fixed - before_observation, observation.0);
    for scale in [0, 1, 2, 3] {
        // Scalar resource arithmetic only, not a forged source/HIR witness.
        let c = HirCounts {
            functions: scale,
            blocks: 2 * scale,
            calls: 3 * scale,
            record_literals: 4 * scale,
            type_frames: 5 * scale,
            call_arguments: 6 * scale,
            max_record_fields: 7,
            matches: 8 * scale,
            ..HirCounts::default()
        };
        let plan = HirPlan::calculate(c, at).unwrap();
        let inherited = c.type_frames * size_of::<typeck::TypeFrame>()
            + c.call_arguments * size_of::<(ParameterTy, Span)>()
            + c.record_literals * c.max_record_fields * size_of::<bool>()
            + c.matches * size_of::<[u64; 4]>()
            + c.functions
                * (size_of::<Vec<typeck::TypeFrame>>()
                    + size_of::<type_storage::FunctionQuota>()
                    + typeck::borrowed_body_return_carrier_bytes()
                    + checker.1)
            + c.blocks * checker.2
            + c.calls * (size_of::<Vec<(ParameterTy, Span)>>() + checker.3)
            + c.record_literals * (size_of::<Vec<bool>>() + checker.4);
        assert_eq!(plan.fixed, base.fixed);
        assert_eq!(
            plan.typeck_scratch - inherited,
            c.functions * observation.1
                + c.calls * observation.2
                + c.record_literals * observation.3
        );
    }
    println!(
        "C3_T1_OBSERVATION_FORMULA fixed={} function={} call={} literal={}",
        observation.0, observation.1, observation.2, observation.3
    );
}

#[test]
fn c3_t1_observation_price_products_aggregation_and_shared_cap_are_checked() {
    let at = sources("x").get(SourceFileId(0)).span(0, 1);
    let observation = observation_components();
    for width in [observation.1, observation.2, observation.3] {
        assert_eq!(
            mul(usize::MAX / width + 1, width, at).unwrap_err().code,
            "E0400"
        );
        assert!(mul(usize::MAX / width, width, at).is_ok());
    }
    let mut bytes = usize::MAX - observation.0 + 1;
    let before = bytes;
    assert_eq!(
        increment(&mut bytes, observation.0, at).unwrap_err().code,
        "E0400"
    );
    assert_eq!(bytes, before);
    bytes = usize::MAX - observation.0;
    increment(&mut bytes, observation.0, at).unwrap();
    assert_eq!(bytes, usize::MAX);
    let plan = HirPlan::calculate(
        HirCounts {
            functions: 2,
            calls: 3,
            record_literals: 4,
            ..HirCounts::default()
        },
        at,
    )
    .unwrap();
    let remaining = MAX_HIR_BYTES - plan.total;
    assert_eq!(
        plan.with_dynamic(remaining - 1, at).unwrap(),
        MAX_HIR_BYTES - 1
    );
    assert_eq!(plan.with_dynamic(remaining, at).unwrap(), MAX_HIR_BYTES);
    assert_eq!(
        plan.with_dynamic(remaining + 1, at).unwrap_err().code,
        "E0400"
    );
    assert_eq!(plan.with_dynamic(usize::MAX, at).unwrap_err().code, "E0400");
    // Independent primitive projection reservation sees this complete seed;
    // there is no paid checker context or fresh owner in this control.
    let total = std::cell::Cell::new(plan.total);
    let mut allocator = Allocator::default();
    let work = WorkMeter::new(0);
    let path =
        type_storage::projection_fields_metered(&total, &mut allocator, 1, &work, at).unwrap();
    assert_eq!(path.capacity(), 1);
    assert_eq!(
        total.get(),
        plan.with_dynamic(size_of::<FieldId>(), at).unwrap()
    );
    assert_eq!((allocator.attempts, work.used()), (1, 0));
}

#[test]
fn c3_t1_inhabited_denied_selector_grows_only_the_existing_fixed_return_charge() {
    let at = sources("x").get(SourceFileId(0)).span(0, 1);
    let plan = HirPlan::calculate(HirCounts::default(), at).unwrap();
    let old_return = size_of::<Result<Option<std::convert::Infallible>, Vec<Diagnostic>>>();
    let inputs = size_of::<&DeclarationIndex<'_>>()
        + size_of::<&WorkMeter>()
        + size_of::<&mut Allocator>()
        + size_of::<usize>()
        + size_of::<SourceOwner<'_>>()
        + size_of::<Span>();
    let old_fixed = size_of::<typeck::TypedOwnedProgram<'_>>()
        + size_of::<PlanReturnEnvelope>()
        + size_of::<CapacityReturnEnvelope>()
        + size_of::<CursorTemporaries>()
        + size_of::<ScalarReturnEnvelope>()
        + resolver_storage::fixed_carrier_bytes()
        + VECTOR_RETURN_ENVELOPE_BYTES
        + type_storage::fixed_control_carrier_bytes()
        + typeck::borrowed_check_carrier_bytes()
        + inputs
        + old_return
        + checker_only_components().0
        + observation_components().0
        + pipeline_components()
        + size_of::<[Option<ExprCursor>; MAX_NESTING]>()
        + size_of::<[Option<BlockCursor>; MAX_BLOCK_NESTING]>();
    let delta = resolve::enum_type_observation_return_bytes() - old_return;
    assert_eq!(plan.fixed - old_fixed, delta);
    #[cfg(target_pointer_width = "64")]
    {
        // MatchArms adds two usize slots to the returned resolver facts.
        assert_eq!(delta, 888);
        assert_eq!(observation_components(), (11792 + 3 * 16, 320, 24, 24));
        assert_eq!(
            checker_only_components(),
            (33448 + 64 * 152, 1928 + 472, 24, 784, 88)
        );
        // Independently measured embedded banks, rather than substituting a
        // changed aggregate outcome for the preceding member composition.
        assert_eq!(
            size_of::<typeck::semantic_carriers::EnumConstructorCarriers>(),
            152
        );
        assert_eq!(
            size_of::<typeck::semantic_carriers::EnumMatchCarriers>(),
            472
        );
    }
    let remaining = MAX_HIR_BYTES - plan.total;
    assert_eq!(
        plan.with_dynamic(remaining - 1, at).unwrap(),
        MAX_HIR_BYTES - 1
    );
    assert_eq!(plan.with_dynamic(remaining, at).unwrap(), MAX_HIR_BYTES);
    assert_eq!(
        plan.with_dynamic(remaining + 1, at).unwrap_err().code,
        "E0400"
    );
    let mut bytes = usize::MAX - resolve::denied_type_probe_carrier_bytes() + 1;
    let before = bytes;
    assert_eq!(
        increment(&mut bytes, resolve::denied_type_probe_carrier_bytes(), at)
            .unwrap_err()
            .code,
        "E0400"
    );
    assert_eq!(bytes, before);
}

#[test]
fn bounded_enum_resolver_branch_charges_follow_actual_visit_dimensions() {
    let at = sources("x").get(SourceFileId(0)).span(0, 1);
    let empty = HirPlan::calculate(HirCounts::default(), at).unwrap();
    for visits in [0, 1, 64] {
        // Arithmetic-only dimension controls, not manufactured source owners.
        let expressions = HirPlan::calculate(
            HirCounts {
                max_expression_depth: visits,
                ..HirCounts::default()
            },
            at,
        )
        .unwrap();
        assert_eq!(
            expressions.resolver_scratch - empty.resolver_scratch,
            visits * resolve::enum_expression_carrier_bytes()
        );
        let matches = HirPlan::calculate(
            HirCounts {
                matches: visits,
                ..HirCounts::default()
            },
            at,
        )
        .unwrap();
        assert_eq!(
            matches.resolver_scratch - empty.resolver_scratch,
            visits * resolve::enum_match_carrier_bytes()
        );
    }
    let arms = HirPlan::calculate(
        HirCounts {
            match_arms: 256,
            ..HirCounts::default()
        },
        at,
    )
    .unwrap();
    assert_eq!(arms.resolved - empty.resolved, 256 * size_of::<MatchArm>());
    assert_eq!(arms.resolver_scratch, empty.resolver_scratch);
    assert_eq!(MAX_HIR_BYTES, 64 * 1024 * 1024);
    let remainder = MAX_HIR_BYTES - arms.total;
    assert_eq!(
        arms.with_dynamic(remainder - 1, at).unwrap(),
        MAX_HIR_BYTES - 1
    );
    assert_eq!(arms.with_dynamic(remainder, at).unwrap(), MAX_HIR_BYTES);
    assert_eq!(
        arms.with_dynamic(remainder + 1, at).unwrap_err().code,
        "E0400"
    );
}
