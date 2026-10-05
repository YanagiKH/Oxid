//! Closed paid resolver controls. No successful observation or owner can escape.
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
fn c3a_paid_resolver_prefix_order_is_preserved_behind_final_observation_fence() {
    with_index(PREFIX, |index| {
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(32).unwrap();
        let work = WorkMeter::default();
        let errors = closed_attempt(index, &work, &mut allocator);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "E0101");
        assert_eq!(
            errors[0].message,
            "paid resolver observations are not connected"
        );
        assert_eq!(
            allocator
                .trace
                .iter()
                .take(5)
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
        assert_eq!(allocator.attempts, 24);
        assert!(!allocator.observer_trace_overflow);
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
                        "paid resolver observations are not connected"
                    );
                }
                drop(errors);
            });
            assert_eq!(live, 0, "prefix leak after reserve failure {fail_at:?}");
            assert!(peak > 0);
            if fail_at.is_some() {
                prefix_only(&allocator);
            }
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

#[test]
fn c3a_paid_resolver_declaration_duplicate_keeps_first_origin_and_beats_its_bad_type() {
    with_index(
        "enum E{V} struct R{same:i32,other:bool,same:Missing}",
        |index| {
            let sources = index.sources();
            let (key, module) = index.record(RecordId(0)).unwrap();
            let record = &sources.ast(module).unwrap().records[key.index];
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(16).unwrap();
            let errors = closed_attempt(index, &WorkMeter::default(), &mut allocator);
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, "E0201");
            assert_eq!(errors[0].primary, Some(record.fields[2].name));
            assert_eq!(errors[0].secondary.len(), 1);
            assert_eq!(errors[0].secondary[0].0, record.fields[0].name);
            assert_eq!(errors[0].secondary[0].1, "first declared here");
            assert!(!errors[0].message.contains("Missing"));
            prefix_only(&allocator);
        },
    );
}

#[test]
fn c3a_paid_resolver_earlier_field_query_failure_precedes_later_duplicate() {
    with_index("enum E{V} struct R{same:Missing,same:i32}", |index| {
        let sources = index.sources();
        let (key, module) = index.record(RecordId(0)).unwrap();
        let record = &sources.ast(module).unwrap().records[key.index];
        let expected = value_type(
            &mut index.query(&WorkMeter::default()),
            module,
            record.fields[0].ty,
        )
        .unwrap_err();
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(16).unwrap();
        let work = WorkMeter::default();
        work.enable_observation();
        let errors = closed_attempt(index, &work, &mut allocator);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            (
                errors[0].code,
                errors[0].primary,
                errors[0].message.as_str()
            ),
            (expected.code, expected.primary, expected.message.as_str())
        );
        assert!(errors[0].secondary.is_empty());
        assert!(!work
            .events
            .borrow()
            .iter()
            .any(|event| event.operation == "paid resolver name byte"));
        prefix_only(&allocator);
    });
}

#[test]
fn c3a_paid_resolver_declaration_long_prefix_scan_work_boundaries_drop_storage() {
    with_index("enum E{V} struct R{aaaaaaaa:(),aaaaaaab:()}", |index| {
        let sources = index.sources();
        let (key, module) = index.record(RecordId(0)).unwrap();
        let record = &sources.ast(module).unwrap().records[key.index];
        // Independently: preflight visits 1 record + 2 fields; the first Unit
        // query costs 1; the duplicate scan compares 8 bytes; the second Unit
        // query costs 1. Limit 8 stops four bytes into the only comparison.
        for limit in [4, 8, 11, 12, 13, 14] {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(16).unwrap();
            let (_, (_, live, peak)) = super::super::reviewer_source::integration_measured(|| {
                let work = WorkMeter::new(limit);
                work.enable_observation();
                let errors = closed_attempt(index, &work, &mut allocator);
                assert_eq!(errors.len(), 1);
                assert_eq!(work.used(), limit.min(13));
                let compared = work
                    .events
                    .borrow()
                    .iter()
                    .filter(|event| event.operation == "paid resolver name byte")
                    .count();
                assert_eq!(compared as u64, limit.saturating_sub(4).min(8));
                if limit < 13 {
                    assert_eq!(errors[0].code, "E0400");
                    assert_eq!(
                        errors[0].primary,
                        Some(if limit < 12 {
                            record.fields[1].name
                        } else {
                            record.fields[1].ty.span
                        })
                    );
                } else {
                    assert_eq!(
                        errors[0].message,
                        "paid resolver observations are not connected"
                    );
                }
                drop(errors);
            });
            assert_eq!(live, 0, "declaration scan leaked at work limit {limit}");
            assert!(peak > 0);
            assert_eq!(allocator.attempts, if limit < 13 { 3 } else { 4 });
            if limit < 13 {
                prefix_only(&allocator);
            } else {
                assert_eq!(allocator.trace.last().unwrap().kind, "paid HIR functions");
            }
        }
    });
}

#[test]
fn c3a_paid_resolver_closed_body_matches_independent_small_request_oracles() {
    // Fixed before integration in independent-small-oracles.md. These are
    // logical exact reserve requests, including all zero-capacity requests.
    for (text, expected_attempts, expected_slots) in [
        ("enum Only { V }", 3, [0; 17]),
        ("enum Unused { V } fn main() -> i32 { return 0; }", 13,
            [0,0,1,0,1,0,1,1,1,0,0,0,0,0,1,1,12]),
        ("enum Unused { V } fn id(x: i32) -> i32 { return x; } fn main() -> i32 { let n = id(7); return n; }", 24,
            [0,0,2,1,2,2,4,2,3,1,0,0,2,2,2,2,24]),
    ] {
        with_index(text, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(64).unwrap();
            let (_, (_, live, _)) = super::super::reviewer_source::integration_measured(|| {
                let errors = closed_attempt(index, &WorkMeter::default(), &mut allocator);
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].message, "paid resolver observations are not connected");
                drop(errors);
            });
            assert_eq!(live, 0);
            assert_eq!(allocator.attempts, expected_attempts);
            let labels = ["paid HIR records", "paid HIR record fields", "paid HIR signatures",
                "paid HIR parameters", "paid HIR functions", "paid HIR bindings", "paid HIR expressions",
                "paid HIR blocks", "paid HIR statements", "paid HIR arguments", "paid HIR field initializers",
                "paid HIR array entries", "paid HIR scope names", "paid HIR scope exits", "paid HIR scope marks",
                "paid HIR loops", "paid HIR resolve frames"];
            let mut slots = [0; 17];
            for event in &allocator.trace {
                assert!(event.success);
                let kind = labels.iter().position(|label| *label == event.kind).expect("only paid requests");
                slots[kind] += event.length;
            }
            assert_eq!(slots, expected_slots);
            assert!(!allocator.observer_trace_overflow);
        });
    }
}

const MIXED_BODY: &str = "enum Unused{V} struct R{x:i32} struct O{r:R,a:[i32;2]} fn read(p:&R)->i32{return p.x;} fn relay(p:&R)->i32{return read(&*p);} fn id(x:i32)->i32{return x;} fn main()->i32{let r=R{x:id(id(1))};let o=O{r:r,a:[1,2]};let mut a=[3,4];let mut n=0;while n<2{if n==0{let same=relay(&o.r);n=n+1;continue;}else{let same=id(n);a[n]=same;}break;}return a[0]+o.r.x;}";
#[test]
fn c3a_paid_resolver_closed_mixed_body_pays_every_kind_and_every_failure_drops() {
    with_index(MIXED_BODY, |index| {
        let mut successful = Allocator::default();
        successful.observer_trace_bound(256).unwrap();
        let errors = closed_attempt(index, &WorkMeter::default(), &mut successful);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "paid resolver observations are not connected"
        );
        let attempts = successful.attempts;
        let labels = [
            "paid HIR records",
            "paid HIR record fields",
            "paid HIR signatures",
            "paid HIR parameters",
            "paid HIR functions",
            "paid HIR bindings",
            "paid HIR expressions",
            "paid HIR blocks",
            "paid HIR statements",
            "paid HIR arguments",
            "paid HIR field initializers",
            "paid HIR array entries",
            "paid HIR scope names",
            "paid HIR scope exits",
            "paid HIR scope marks",
            "paid HIR loops",
            "paid HIR resolve frames",
        ];
        for label in labels {
            assert!(
                successful
                    .trace
                    .iter()
                    .any(|event| event.kind == label && event.length > 0),
                "missing {label}"
            );
        }
        assert!(!successful
            .trace
            .iter()
            .any(|event| event.kind == "array HIR elements"));
        assert!(!successful.observer_trace_overflow);
        for fail_at in 1..=attempts {
            let mut allocator = Allocator {
                fail_at: Some(fail_at),
                ..Allocator::default()
            };
            allocator.observer_trace_bound(256).unwrap();
            let (_, (_, live, peak)) = super::super::reviewer_source::integration_measured(|| {
                let errors = closed_attempt(index, &WorkMeter::default(), &mut allocator);
                assert!(errors.iter().any(|error| error.code == "E0400"));
                drop(errors);
            });
            assert_eq!(live, 0, "closed body leaked at request {fail_at}");
            assert!(peak > 0);
            assert!(!allocator.trace[fail_at - 1].success);
            assert!(!allocator.observer_trace_overflow);
        }
    });
}
