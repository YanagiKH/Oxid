//! Isolated helpers only. No paid resolver consumer or program witness is called.
use super::*;
use crate::frontend::{
    declaration_index::{collect_enum_candidate, IndexLimits, SourceOwner},
    lexer, parser,
    source::{SourceFileId, SourceMap, SourceView},
};
use std::mem::align_of;

fn with_index<T>(text: &str, action: impl FnOnce(&DeclarationIndex<'_>) -> T) -> T {
    let mut sources = SourceMap::new();
    let file = sources.add("paid-storage-helper.ox".into(), text.into());
    let source = sources.get(file);
    let ast = parser::parse_enum_candidate_counted(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut Allocator::default(),
        &mut Default::default(),
    )
    .unwrap()
    .0;
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_enum_candidate(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
    action(&index)
}
fn origin() -> Span {
    let mut map = SourceMap::new();
    map.add("quota.ox".into(), "x".into());
    map.get(SourceFileId(0)).span(0, 1)
}

#[test]
fn c3a_paid_helper_each_kind_quota_is_consumable_and_exact() {
    let at = origin();
    let counts = HirCounts {
        records: 1,
        record_fields: 1,
        functions: 1,
        parameters: 1,
        bindings: 1,
        expressions: 1,
        blocks: 1,
        statements: 1,
        call_arguments: 1,
        field_initializers: 1,
        array_entries: 1,
        scope_marks: 1,
        loop_slots: 1,
        resolve_frames: 1,
        ..HirCounts::default()
    };
    let mut storage = PaidStorage::new(counts);
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(2 * KINDS).unwrap();
    macro_rules! check {
        ($kind:ident, $ty:ty) => {{
            let empty = storage
                .reserve::<$ty>(&mut allocator, Kind::$kind, 0, at)
                .unwrap();
            assert_eq!(empty.capacity(), 0);
            let values = storage
                .reserve::<$ty>(&mut allocator, Kind::$kind, 1, at)
                .unwrap();
            assert_eq!(values.capacity(), 1);
            let attempts = allocator.attempts;
            let error = storage
                .reserve::<$ty>(&mut allocator, Kind::$kind, 1, at)
                .unwrap_err();
            assert_eq!(error.code, "E0400");
            assert_eq!(allocator.attempts, attempts);
        }};
    }
    check!(Records, Record);
    check!(Fields, Field);
    check!(Signatures, Signature);
    check!(Parameters, ParameterTy);
    check!(Functions, Function);
    check!(Bindings, Binding);
    check!(Expressions, Expr);
    check!(Blocks, BodyBlock);
    check!(Statements, Stmt);
    check!(Arguments, Argument);
    check!(FieldInitializers, FieldInit);
    check!(ArrayEntries, ExprId);
    check!(Names, ScopeName);
    check!(Exits, usize);
    check!(Marks, usize);
    check!(Loops, LoopId);
    // ResolveFrame is not Debug; avoid Result::unwrap_err's success bound.
    let empty = storage
        .reserve::<ResolveFrame>(&mut allocator, Kind::Frames, 0, at)
        .unwrap();
    let values = storage
        .reserve::<ResolveFrame>(&mut allocator, Kind::Frames, 1, at)
        .unwrap();
    assert_eq!((empty.capacity(), values.capacity()), (0, 1));
    let attempts = allocator.attempts;
    assert!(storage
        .reserve::<ResolveFrame>(&mut allocator, Kind::Frames, 1, at)
        .is_err());
    assert_eq!(allocator.attempts, attempts);
    assert_eq!(storage.remaining, [0; KINDS]);
    assert_eq!(storage.reserved, [1; KINDS]);
    assert_eq!(allocator.attempts, 2 * KINDS);
    assert!(allocator.trace.iter().all(|event| event.success));
    assert!(!allocator.observer_trace_overflow);
}

#[test]
fn c3a_paid_helper_overflow_wrong_width_and_failed_reserve_do_not_reuse_quota() {
    let at = origin();
    let mut storage = PaidStorage::new(HirCounts {
        array_entries: usize::MAX,
        ..HirCounts::default()
    });
    let mut allocator = Allocator::default();
    assert!(storage
        .reserve::<ExprId>(&mut allocator, Kind::ArrayEntries, usize::MAX, at)
        .is_err());
    assert_eq!(allocator.attempts, 0);
    assert_eq!(storage.remaining[Kind::ArrayEntries as usize], usize::MAX);
    assert!(storage
        .reserve::<u8>(&mut allocator, Kind::ArrayEntries, 1, at)
        .is_err());
    assert_eq!(allocator.attempts, 0);
    let mut storage = PaidStorage::new(HirCounts {
        array_entries: 1,
        ..HirCounts::default()
    });
    allocator.fail_at = Some(1);
    assert!(storage
        .reserve::<ExprId>(&mut allocator, Kind::ArrayEntries, 1, at)
        .is_err());
    assert_eq!(storage.remaining[Kind::ArrayEntries as usize], 0);
    assert_eq!(storage.reserved[Kind::ArrayEntries as usize], 0);
    allocator.fail_at = None;
    assert!(storage
        .reserve::<ExprId>(&mut allocator, Kind::ArrayEntries, 1, at)
        .is_err());
    assert_eq!(allocator.attempts, 1);
}

#[test]
fn c3a_paid_helper_append_and_actual_capacity_reject_unpaid_growth() {
    let at = origin();
    let ticket = Capacity::new::<ExprId>(2, 2 * size_of::<ExprId>(), at).unwrap();
    assert!(ticket.check_observed(1, at).is_ok());
    assert!(ticket.check_observed(2, at).is_ok());
    assert_eq!(ticket.check_observed(3, at).unwrap_err().code, "E0400");
    let mut values = Vec::with_capacity(2);
    for index in 0..2 {
        room(&values, values.capacity(), true, at).unwrap();
        values.push(ExprId(index));
    }
    assert_eq!(
        room(&values, values.capacity(), true, at).unwrap_err().code,
        "E0400"
    );
    assert_eq!(values.capacity(), 2);
    assert!(room(&values, values.capacity(), false, at).is_ok());
}

const SCOPES: &str = "enum Unused{V} fn names(root:i32)->i32{let z=1;if true{let same=2;}else{let same=3;}return root;}";
#[test]
fn c3a_paid_helper_sorted_names_preserve_activation_and_lexical_exits() {
    with_index(SCOPES, |index| {
        let sources = index.sources();
        let (key, module) = index.function(DefId(0)).unwrap();
        let ast = sources.ast(module).unwrap();
        let function = &ast.functions[key.index];
        let work = WorkMeter::default();
        let mut counts = HirCounts::default();
        super::super::hir_budget::count_function(ast, function, &work, &mut counts).unwrap();
        let mut storage = PaidStorage::new(counts);
        let mut allocator = Allocator::default();
        let mut scope = PaidScope::new(
            index,
            &work,
            function,
            &counts,
            &mut storage,
            &mut allocator,
        )
        .unwrap();
        assert_eq!(scope.names.capacity(), 4);
        assert_eq!(
            scope
                .names
                .iter()
                .map(|row| sources.text(row.name).unwrap())
                .collect::<Vec<_>>(),
            ["root", "same", "z"]
        );
        let root = scope.names[0].name;
        let same = scope.names[1].name;
        let z = scope.names[2].name;
        assert_eq!(scope.active(index, &work, root).unwrap(), None);
        scope.activate(0, BindingId(0), root).unwrap();
        scope.enter(root).unwrap();
        scope.activate(2, BindingId(1), z).unwrap();
        scope.enter(same).unwrap();
        scope.activate(1, BindingId(2), same).unwrap();
        assert_eq!(
            scope.active(index, &work, same).unwrap(),
            Some(BindingId(2))
        );
        scope.leave(&work, same).unwrap();
        assert_eq!(scope.active(index, &work, same).unwrap(), None);
        assert_eq!(
            scope.active(index, &work, root).unwrap(),
            Some(BindingId(0))
        );
        scope.enter(same).unwrap();
        scope.activate(1, BindingId(3), same).unwrap();
        scope.leave(&work, same).unwrap();
        scope.leave(&work, z).unwrap();
        assert_eq!(scope.active(index, &work, z).unwrap(), None);
        assert_eq!(
            scope.active(index, &work, root).unwrap(),
            Some(BindingId(0))
        );
        assert_eq!((scope.exits.len(), scope.marks.len()), (1, 0));
        assert!(scope.activate(0, BindingId(4), root).is_err());
        assert!(work.used() > 0);
        assert_eq!(
            index.require_current_source_pipeline().unwrap_err().code,
            "E0101"
        );
    });
}

#[test]
fn c3a_paid_helper_scope_success_and_each_partial_reserve_failure_release_storage() {
    with_index(SCOPES, |index| {
        let sources = index.sources();
        let (key, module) = index.function(DefId(0)).unwrap();
        let ast = sources.ast(module).unwrap();
        let function = &ast.functions[key.index];
        let mut counts = HirCounts::default();
        super::super::hir_budget::count_function(ast, function, &WorkMeter::default(), &mut counts)
            .unwrap();
        for fail_at in [None, Some(1), Some(2), Some(3)] {
            let mut allocator = Allocator {
                fail_at,
                ..Allocator::default()
            };
            allocator.observer_trace_bound(3).unwrap();
            let (succeeded, (_, live, peak)) =
                super::super::reviewer_source::integration_measured(|| {
                    let mut storage = PaidStorage::new(counts);
                    let result = PaidScope::new(
                        index,
                        &WorkMeter::default(),
                        function,
                        &counts,
                        &mut storage,
                        &mut allocator,
                    );
                    let succeeded = result.is_ok();
                    drop(result);
                    succeeded
                });
            assert_eq!(succeeded, fail_at.is_none());
            assert_eq!(live, 0, "scope leaked after reserve failure {fail_at:?}");
            assert!(peak > 0);
            assert_eq!(allocator.attempts, fail_at.unwrap_or(3));
        }
        // Exhaust work after all three reserves; even a sort/inventory failure
        // must release every already allocated buffer and bounded diagnostic.
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(3).unwrap();
        let (_, (_, live, _)) = super::super::reviewer_source::integration_measured(|| {
            let mut storage = PaidStorage::new(counts);
            let result = PaidScope::new(
                index,
                &WorkMeter::new(0),
                function,
                &counts,
                &mut storage,
                &mut allocator,
            );
            assert_eq!(result.as_ref().err().unwrap().code, "E0400");
            drop(result);
        });
        assert_eq!(live, 0);
    });
}

#[test]
fn c3a_paid_helper_long_prefix_comparison_is_fallible_and_byte_metered() {
    with_index(
        "enum Unused{V} fn names(aaaaaaaa:i32,aaaaaaab:i32)->i32{return 0;}",
        |index| {
            let sources = index.sources();
            let (key, module) = index.function(DefId(0)).unwrap();
            let function = &sources.ast(module).unwrap().functions[key.index];
            let left = function.params[0].name;
            let right = function.params[1].name;
            for limit in [7, 8, 9] {
                let work = WorkMeter::new(limit);
                let result = compare(index, &work, left, right);
                if limit == 7 {
                    assert_eq!(result.unwrap_err().code, "E0400");
                    assert_eq!(work.used(), 7);
                } else {
                    assert_eq!(result.unwrap(), Ordering::Less);
                    assert_eq!(work.used(), 8);
                }
            }
            let work = WorkMeter::new(8);
            assert!(compare(index, &work, left, left).is_err());
            let work = WorkMeter::new(9);
            assert_eq!(compare(index, &work, left, left).unwrap(), Ordering::Equal);
            assert_eq!(work.used(), 9);
        },
    );
}

#[test]
fn c3a_paid_helper_complete_carriers_are_measured_before_consumer_activation() {
    macro_rules! layout { ($($ty:ty),* $(,)?) => { $(
        println!("C3A_PAID_LAYOUT {} bytes={} align={}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());
    )* }; }
    layout!(Kind, PaidStorage, PaidScope, ResolverStorage<'_>, ResolverStorageObservation,
        Option<ResolverStorageObservation>, Result<Option<ResolverStorageObservation>, Vec<Diagnostic>>,
        HirCounts, Option<HirCounts>, Result<HirCounts, Box<Diagnostic>>,
        Result<PaidScope, Box<Diagnostic>>, Option<PaidScope>, ScopeName,
        Result<Ordering, Box<Diagnostic>>, Result<Option<usize>, Box<Diagnostic>>,
        Result<Option<BindingId>, Box<Diagnostic>>, FixedCarriers, FunctionCarriers);
    assert!(
        fixed_carrier_bytes()
            >= 2 * size_of::<PaidStorage>()
                + 2 * size_of::<HirCounts>()
                + 4 * size_of::<[usize; KINDS]>()
                + 2 * size_of::<ResolverStorageObservation>()
                + size_of::<Option<ResolverStorageObservation>>()
                + size_of::<Result<Option<ResolverStorageObservation>, Vec<Diagnostic>>>()
                + size_of::<(Vec<Record>, Vec<Signature>, Vec<Function>)>()
                + size_of::<Result<(Vec<Record>, Vec<Signature>, Vec<Function>), Vec<Diagnostic>>>(
                )
    );
    assert_eq!(
        size_of::<ResolverStorage<'_>>(),
        size_of::<Option<&mut PaidStorage>>() + size_of::<Option<PaidScope>>()
    );
    println!(
        "C3A_PAID_LAYOUT resolver-header={} fixed={} per-function={}",
        super::super::resolve::resolver_carrier_bytes(),
        fixed_carrier_bytes(),
        function_carrier_bytes()
    );
    // An unused enum still cannot enter the old shared resolver consumer.
    with_index("enum Unused{V} fn main()->i32{return 0;}", |index| {
        assert_eq!(
            super::super::resolve::resolve_project(index, &WorkMeter::default()).unwrap_err()[0]
                .code,
            "E0101"
        );
    });
}
