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
        Capacity,
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
