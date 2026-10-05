//! Closed paid declaration/signature slice. All successful body probes are fenced.
use super::*;
use crate::frontend::{lexer, parser, source::SourceFileId};

fn with_index<T>(text: &str, action: impl FnOnce(&DeclarationIndex<'_>) -> T) -> T {
    let mut sources = SourceMap::new();
    let file = sources.add("paid-resolver.ox".into(), text.into());
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
    let index = index::collect_enum_candidate(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
    action(&index)
}
fn closed_attempt(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Vec<Diagnostic> {
    let plan = super::super::hir_budget::preflight_enum_hir(index, work)
        .unwrap()
        .unwrap();
    let mut paid = PaidStorage::new(plan.counts);
    resolve_index_impl(index, work, allocator, Some(&mut paid)).unwrap_err()
}
fn prefix_only(allocator: &Allocator) {
    assert!(allocator.trace.iter().all(|event| matches!(
        event.kind,
        "paid HIR records"
            | "paid HIR record fields"
            | "paid HIR signatures"
            | "paid HIR parameters"
    )));
    assert!(!allocator.observer_trace_overflow);
}

#[test]
fn c3a_paid_resolver_enum_value_type_guards_run_before_any_body_storage() {
    for text in [
        "enum E{V} fn take(value:E)->i32{return missing;}",
        "enum E{V} fn give()->E{return missing;}",
        "enum E{V} fn main()->i32{return missing;} fn late()->i32{let x:E=missing;return 0;}",
    ] {
        with_index(text, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(32).unwrap();
            let work = WorkMeter::default();
            work.enable_observation();
            let errors = closed_attempt(index, &work, &mut allocator);
            assert_eq!(errors.len(), 1);
            let error = &errors[0];
            assert_eq!((error.code, error.stage), ("E0101", "resolve"));
            assert_eq!(
                error.message,
                "enum value types are unavailable in this resolver-storage checkpoint"
            );
            assert_eq!(index.sources().text(error.primary.unwrap()).unwrap(), "E");
            prefix_only(&allocator);
            assert!(!work
                .observations
                .borrow()
                .iter()
                .any(|event| matches!(event, index::Observation::Phase("body-resolution"))));
        });
    }
}

#[test]
fn c3a_paid_resolver_existing_query_and_record_field_errors_remain_authoritative() {
    for text in [
        "enum E{V} fn take(value:&E)->E{return missing;}",
        "enum E{V} fn take(value:Missing)->E{return missing;}",
    ] {
        with_index(text, |index| {
            let sources = index.sources();
            let (key, module) = index.function(DefId(0)).unwrap();
            let function = &sources.ast(module).unwrap().functions[key.index];
            let expected = parameter_type(
                &mut index.query(&WorkMeter::default()),
                module,
                function.params[0].ty,
            )
            .unwrap_err();
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(32).unwrap();
            let errors = closed_attempt(index, &WorkMeter::default(), &mut allocator);
            assert_eq!(errors.len(), 1);
            assert_eq!(
                (
                    errors[0].code,
                    errors[0].primary,
                    errors[0].message.as_str()
                ),
                (expected.code, expected.primary, expected.message.as_str())
            );
            prefix_only(&allocator);
        });
    }
    with_index(
        "enum E{V} struct R{x:E} fn main()->i32{return missing;}",
        |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(32).unwrap();
            let errors = closed_attempt(index, &WorkMeter::default(), &mut allocator);
            assert_eq!(errors[0].code, "E0300");
            assert_eq!(errors[0].message, "enum values cannot be record fields");
            assert_eq!(
                index.sources().text(errors[0].primary.unwrap()).unwrap(),
                "x:E"
            );
            prefix_only(&allocator);
        },
    );
}

const PREFIX: &str =
    "enum E{V} struct R{x:i32,y:bool} fn take(value:R)->R{return value;} fn main()->i32{return 0;}";
#[test]
fn c3a_paid_resolver_partial_prefix_never_returns_success_or_reaches_bodies() {
    with_index(PREFIX, |index| {
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(32).unwrap();
        let work = WorkMeter::default();
        let errors = closed_attempt(index, &work, &mut allocator);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "E0101");
        assert_eq!(
            errors[0].message,
            "paid resolver body storage is not connected"
        );
        assert_eq!(
            allocator
                .trace
                .iter()
                .map(|event| (event.kind, event.length))
                .collect::<Vec<_>>(),
            [
                ("paid HIR records", 1),
                ("paid HIR record fields", 2),
                ("paid HIR signatures", 2),
                ("paid HIR parameters", 1),
                ("paid HIR parameters", 0),
            ]
        );
        assert_eq!(allocator.attempts, 5);
        prefix_only(&allocator);
        let mut denied = Allocator::default();
        assert_eq!(
            resolve_index(index, &WorkMeter::default(), &mut denied).unwrap_err()[0].code,
            "E0101"
        );
        assert_eq!(denied.attempts, 0);
        assert_eq!(
            resolve_project(index, &WorkMeter::default()).unwrap_err()[0].code,
            "E0101"
        );
    });
}

#[test]
fn c3a_paid_resolver_prefix_each_reserve_failure_drops_private_parts() {
    with_index(PREFIX, |index| {
        for fail_at in [None, Some(1), Some(2), Some(3), Some(4), Some(5)] {
            let mut allocator = Allocator {
                fail_at,
                ..Allocator::default()
            };
            allocator.observer_trace_bound(32).unwrap();
            let (_, (_, live, peak)) = super::super::reviewer_source::integration_measured(|| {
                let errors = closed_attempt(index, &WorkMeter::default(), &mut allocator);
                if fail_at.is_some() {
                    assert!(errors.iter().any(|error| error.code == "E0400"));
                } else {
                    assert_eq!(
                        errors[0].message,
                        "paid resolver body storage is not connected"
                    );
                }
                drop(errors);
            });
            assert_eq!(live, 0, "prefix leak after reserve failure {fail_at:?}");
            assert!(peak > 0);
            prefix_only(&allocator);
            if let Some(ordinal) = fail_at {
                assert!(!allocator.trace[ordinal - 1].success);
            }
        }
    });
}

#[test]
fn c3a_paid_resolver_type_guard_is_a_no_allocation_scalar_check() {
    let mut sources = SourceMap::new();
    sources.add("guard.ox".into(), "x".into());
    let at = sources.get(SourceFileId(0)).span(0, 1);
    let (result, stats) = super::super::reviewer_source::integration_measured(|| {
        deny_checkpoint_enum(&ValueTy::Scalar(Ty::I32), at)
    });
    result.unwrap();
    assert_eq!(stats, (0, 0, 0));
}
