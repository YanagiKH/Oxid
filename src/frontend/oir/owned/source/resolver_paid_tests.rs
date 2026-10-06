//! Fixed resolver observations only. No resolved owner or typed witness escapes.
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
fn attempt_error(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Vec<Diagnostic> {
    probe_enum_resolver_storage(index, work, allocator).unwrap_err()
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
fn c3a_paid_resolver_enum_value_types_preserve_later_body_diagnostics() {
    for text in [
        "enum E{V} fn take(value:E)->i32{return missing;}",
        "enum E{V} fn give()->E{return missing;}",
        "enum E{V} fn main()->i32{return missing;} fn late()->i32{let x:E=missing;return 0;}",
    ] {
        with_index(text, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(64).unwrap();
            let work = WorkMeter::default();
            work.enable_observation();
            let errors = attempt_error(index, &work, &mut allocator);
            assert!(!errors.is_empty());
            for error in &errors {
                assert_eq!((error.code, error.stage), ("E0200", "resolve"));
                assert_eq!(
                    index.sources().text(error.primary.unwrap()).unwrap(),
                    "missing"
                );
            }
            assert!(work
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
            let errors = attempt_error(index, &WorkMeter::default(), &mut allocator);
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
            let errors = attempt_error(index, &WorkMeter::default(), &mut allocator);
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
fn c3a_paid_resolver_prefix_order_is_preserved_in_fixed_observation() {
    with_index(PREFIX, |index| {
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(32).unwrap();
        let work = WorkMeter::default();
        let observation = probe_enum_resolver_storage(index, &work, &mut allocator)
            .unwrap()
            .unwrap();
        assert_eq!(observation.reservation_attempts, 24);
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
                let result =
                    probe_enum_resolver_storage(index, &WorkMeter::default(), &mut allocator);
                if fail_at.is_some() {
                    assert!(result
                        .as_ref()
                        .unwrap_err()
                        .iter()
                        .any(|error| error.code == "E0400"));
                } else {
                    assert!(result.as_ref().unwrap().is_some());
                }
                drop(result);
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
fn c3a_paid_resolver_record_containment_guard_is_a_no_allocation_scalar_check() {
    let mut sources = SourceMap::new();
    sources.add("guard.ox".into(), "x".into());
    let at = sources.get(SourceFileId(0)).span(0, 1);
    let (result, stats) = super::super::reviewer_source::integration_measured(|| {
        record_field_type(ValueTy::Scalar(Ty::I32), at)
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
            let errors = attempt_error(index, &WorkMeter::default(), &mut allocator);
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
        let errors = attempt_error(index, &work, &mut allocator);
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
                let errors = attempt_error(index, &work, &mut allocator);
                assert_eq!(errors.len(), 1);
                assert_eq!(work.used(), limit);
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
                    // Resolution used 13 units; final inventory now runs
                    // before an inert observation can escape.
                    assert_eq!(errors[0].code, "E0400");
                    assert_eq!(errors[0].primary, Some(sources.eof()));
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
fn c3a_paid_resolver_fixed_observations_match_independent_small_oracles() {
    // Fixed before integration in independent-small-oracles.md. These are
    // logical exact reserve requests, including all zero-capacity requests.
    for (text, expected_attempts, expected_slots) in [
        ("enum Only { V }", 3, [0; 18]),
        ("enum Unused { V } fn main() -> i32 { return 0; }", 13,
            [0, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 12]),
        ("enum Unused { V } fn id(x: i32) -> i32 { return x; } fn main() -> i32 { let n = id(7); return n; }", 24,
            [0, 0, 2, 1, 2, 2, 4, 2, 3, 1, 0, 0, 0, 2, 2, 2, 2, 24]),
    ] {
        with_index(text, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(64).unwrap();
            let (observation, (_, live, _)) = super::super::reviewer_source::integration_measured(|| {
                probe_enum_resolver_storage(index, &WorkMeter::default(), &mut allocator).unwrap().unwrap()
            });
            assert_eq!(live, 0);
            assert_eq!(allocator.attempts, expected_attempts);
            assert_eq!(observation.reservation_attempts, expected_attempts);
            assert_eq!(observation.retained_counts.as_slice(), &expected_slots[..13]);
            assert_eq!(observation.capacities, expected_slots);
            let payloads = match expected_attempts { 3 => (0, 0), 13 => (480, 304 + 12 * 8), 24 => (1552, 704 + 24 * 8), _ => unreachable!() };
            assert_eq!((observation.retained_bytes, observation.scratch_capacity_bytes), payloads);
            let labels = ["paid HIR records", "paid HIR record fields", "paid HIR signatures",
                "paid HIR parameters", "paid HIR functions", "paid HIR bindings", "paid HIR expressions",
                "paid HIR blocks", "paid HIR statements", "paid HIR arguments", "paid HIR field initializers",
                "paid HIR array entries", "paid HIR match arms", "paid HIR scope names", "paid HIR scope exits", "paid HIR scope marks",
                "paid HIR loops", "paid HIR resolve frames"];
            let mut slots = [0; 18];
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
fn c3a_paid_resolver_mixed_observation_pays_every_kind_and_every_failure_drops() {
    with_index(MIXED_BODY, |index| {
        let mut successful = Allocator::default();
        successful.observer_trace_bound(256).unwrap();
        let (observation, (_, baseline_live, baseline_peak)) =
            super::super::reviewer_source::integration_measured(|| {
                probe_enum_resolver_storage(index, &WorkMeter::default(), &mut successful)
                    .unwrap()
                    .unwrap()
            });
        // The syntax-only indexed-store wrapper is conservatively reserved but
        // never retained as a resolved expression; lengths and capacity differ.
        assert_eq!(
            observation.retained_counts[6] + 1,
            observation.capacities[6]
        );
        assert_eq!(baseline_live, 0);
        assert!(baseline_peak > 0);
        let attempts = successful.attempts;
        // Independent syntax sites: 3 + 2 records + 9*4 functions + 7 blocks
        // + 5 calls + 2 record literals + 2 arrays. Only main's params is empty.
        assert_eq!(attempts, 57);
        let zero_requests = successful
            .trace
            .iter()
            .filter(|event| event.length == 0)
            .count();
        assert_eq!(zero_requests, 1);
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
                let errors = attempt_error(index, &WorkMeter::default(), &mut allocator);
                assert!(errors.iter().any(|error| error.code == "E0400"));
                drop(errors);
            });
            assert_eq!(live, 0, "resolver probe leaked at request {fail_at}");
            assert!(peak > 0);
            assert!(!allocator.trace[fail_at - 1].success);
            assert!(!allocator.observer_trace_overflow);
        }
        println!("C3A_PAID_RESOLVER_MIXED logical_reserve_positions={attempts} zero_slot_requests={zero_requests} swept_positions={attempts} baseline_live={baseline_live} baseline_peak={baseline_peak}");
    });
}

#[test]
fn c3a_paid_resolver_active_shadow_and_function_conflict_keep_origins() {
    for text in [
        "enum E{V} fn take(same:i32)->(){if true{let same=1;}}",
        "enum E{V} fn same()->i32{return 0;} fn main()->i32{let same=1;return 0;}",
    ] {
        with_index(text, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(128).unwrap();
            let errors = attempt_error(index, &WorkMeter::default(), &mut allocator);
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, "E0201");
            let primary = errors[0].primary.unwrap();
            assert_eq!(primary.start, text.rfind("same").unwrap());
            assert_eq!(errors[0].secondary.len(), 1);
            assert_eq!(errors[0].secondary[0].0.start, text.find("same").unwrap());
            assert_eq!(index.sources().text(primary).unwrap(), "same");
        });
    }
}

#[test]
fn c3a_paid_resolver_sorted_inventory_does_not_preactivate_or_leak_names() {
    for (text, unknown) in [
        (
            "enum E{V} fn main()->i32{let self_name=self_name;return 0;}",
            "self_name",
        ),
        (
            "enum E{V} fn main()->i32{let first=later;let later=1;return 0;}",
            "later",
        ),
        (
            "enum E{V} fn take(same:i32)->i32{let same=bad_init;return 0;}",
            "bad_init",
        ),
        (
            "enum E{V} fn main()->i32{if true{let sibling=1;}else{let sibling=2;}return sibling;}",
            "sibling",
        ),
    ] {
        with_index(text, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(128).unwrap();
            let errors = attempt_error(index, &WorkMeter::default(), &mut allocator);
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, "E0200");
            assert_eq!(
                index.sources().text(errors[0].primary.unwrap()).unwrap(),
                unknown
            );
            assert!(errors[0].secondary.is_empty());
        });
    }
    with_index(
        "enum E{V} fn main()->i32{if true{let sibling=1;}else{let sibling=2;}return 0;}",
        |index| {
            assert!(probe_enum_resolver_storage(
                index,
                &WorkMeter::default(),
                &mut Allocator::default()
            )
            .unwrap()
            .is_some());
        },
    );
}

#[test]
fn c3a_paid_resolver_target_child_and_loop_diagnostic_order_is_preserved() {
    for (text, code, origin) in [
        (
            "enum E{V} fn main()->i32{missing_target=bad_rhs;return 0;}",
            "E0200",
            "missing_target",
        ),
        (
            "enum E{V} fn main()->i32{let mut n=0;n=bad_rhs;return 0;}",
            "E0200",
            "bad_rhs",
        ),
        (
            "enum E{V} fn main()->i32{missing_root.x=bad_rhs;return 0;}",
            "E0200",
            "missing_root",
        ),
        (
            "enum E{V} fn main()->i32{let mut a=[1];a[bad_index]=bad_rhs;return 0;}",
            "E0200",
            "bad_rhs",
        ),
        (
            "enum E{V} fn main()->i32{if true{return then_error;}else{return else_error;}}",
            "E0200",
            "then_error",
        ),
        (
            "enum E{V} fn main()->i32{while true{while true{break;}continue;}break;return 0;}",
            "E0204",
            "break;",
        ),
    ] {
        with_index(text, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(128).unwrap();
            let errors = attempt_error(index, &WorkMeter::default(), &mut allocator);
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, code);
            assert_eq!(
                index.sources().text(errors[0].primary.unwrap()).unwrap(),
                origin
            );
            if code == "E0204" {
                assert_eq!(
                    errors[0].primary.unwrap().start,
                    text.rfind("break;").unwrap()
                );
            }
        });
    }
}

#[test]
fn c3a_paid_resolver_callee_then_argument_error_order_and_reserve_boundary() {
    for (text, expected, arguments_reserved) in [
        ("enum E{V} fn main()->i32{return missing_callee(first_bad,second_bad);}", "missing_callee", false),
        ("enum E{V} fn pair(a:i32,b:i32)->i32{return 0;} fn main()->i32{return pair(first_bad,second_bad);}", "first_bad", true),
    ] {
        with_index(text, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(128).unwrap();
            let errors = attempt_error(index, &WorkMeter::default(), &mut allocator);
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, "E0200");
            assert_eq!(index.sources().text(errors[0].primary.unwrap()).unwrap(), expected);
            assert_eq!(allocator.trace.iter().any(|event| event.kind == "paid HIR arguments"), arguments_reserved);
        });
    }
    // Arity and ownership checks belong to later stages, so even this mismatch
    // yields only fixed name-resolution statistics, never a typed witness.
    with_index(
        "enum E{V} fn zero()->i32{return 0;} fn main()->i32{return zero(1);}",
        |index| {
            assert!(probe_enum_resolver_storage(
                index,
                &WorkMeter::default(),
                &mut Allocator::default()
            )
            .unwrap()
            .is_some());
        },
    );
}

#[test]
fn c3a_paid_resolver_literal_selection_duplicate_and_value_order() {
    for (text, code, origin, reserves) in [
        (
            "enum E{V} fn main()->i32{let r=Missing{x:bad_value};return 0;}",
            "E0202",
            "Missing",
            false,
        ),
        (
            "enum E{V} struct R{x:i32} fn main()->i32{let r=R{unknown:bad_value};return 0;}",
            "E0200",
            "unknown",
            true,
        ),
        (
            "enum E{V} struct R{x:i32} fn main()->i32{let r=R{x:1,x:bad_value};return 0;}",
            "E0201",
            "x",
            true,
        ),
    ] {
        with_index(text, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(128).unwrap();
            let errors = attempt_error(index, &WorkMeter::default(), &mut allocator);
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, code);
            assert_eq!(
                index.sources().text(errors[0].primary.unwrap()).unwrap(),
                origin
            );
            assert_eq!(
                allocator
                    .trace
                    .iter()
                    .any(|event| event.kind == "paid HIR field initializers"),
                reserves
            );
            if code == "E0201" {
                assert_eq!(
                    errors[0].primary.unwrap().start,
                    text.find("x:bad_value").unwrap()
                );
                assert_eq!(errors[0].secondary.len(), 1);
                assert_eq!(errors[0].secondary[0].0.start, text.find("x:1").unwrap());
            }
        });
    }
}

#[test]
fn c3a_paid_resolver_enum_branches_preserve_payload_and_scrutinee_diagnostics() {
    for (text, primary) in [
        (
            "enum E{V,P(i32)} fn main()->i32{let value=E::P(bad_payload);return 0;}",
            "bad_payload",
        ),
        (
            "enum E{V} fn main()->i32{match bad_scrutinee{E::V=>{return bad_arm;}}}",
            "bad_scrutinee",
        ),
    ] {
        with_index(text, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(128).unwrap();
            let errors = attempt_error(index, &WorkMeter::default(), &mut allocator);
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, "E0200");
            assert_eq!(
                index.sources().text(errors[0].primary.unwrap()).unwrap(),
                primary
            );
            assert!(!allocator
                .trace
                .iter()
                .any(|event| event.kind == "paid HIR arguments"));
        });
    }
    with_index(
        "enum E{V} fn helper()->i32{return 0;} fn main()->i32{return crate::helper();}",
        |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(128).unwrap();
            let observation =
                probe_enum_resolver_storage(index, &WorkMeter::default(), &mut allocator)
                    .unwrap()
                    .unwrap();
            assert_eq!(observation.plan.counts.calls, 1);
            assert_eq!(
                allocator
                    .trace
                    .iter()
                    .filter(|event| event.kind == "paid HIR arguments")
                    .count(),
                1
            );
            assert_eq!(
                resolve_index(index, &WorkMeter::default(), &mut Allocator::default()).unwrap_err()
                    [0]
                .code,
                "E0101"
            );
        },
    );
}

#[test]
fn c3a_paid_resolver_imported_enum_and_constructor_privacy_precede_payload_storage() {
    use crate::frontend::project::{ProjectLimits, ProjectSources};
    struct Files(std::path::PathBuf);
    impl Drop for Files {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let files =
        Files(std::env::temp_dir().join(format!("oxid-c3a-paid-private-{}", std::process::id())));
    std::fs::create_dir(&files.0).unwrap();
    std::fs::write(files.0.join("main.ox"),
        "mod m;use crate::m::Unused as Marker;use crate::m::R as R;fn main()->i32{let r=R{y:0,x:bad_value};return 0;}").unwrap();
    std::fs::write(
        files.0.join("m.ox"),
        "pub enum Unused{V} pub struct R{x:i32,pub y:i32}",
    )
    .unwrap();
    let project = ProjectSources::load_enum_index_candidate(
        files.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    let mut allocator = Allocator::default();
    let work = WorkMeter::default();
    let index = index::collect_enum_candidate(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap();
    assert_eq!(index.enum_count(), 1);
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(128).unwrap();
    let errors = attempt_error(&index, &WorkMeter::default(), &mut allocator);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "E0206");
    assert_eq!(
        index.sources().text(errors[0].primary.unwrap()).unwrap(),
        "x"
    );
    assert_eq!(errors[0].secondary.len(), 1);
    assert_eq!(index.sources().text(errors[0].secondary[0].0).unwrap(), "x");
    assert!(!allocator
        .trace
        .iter()
        .any(|event| event.kind == "paid HIR field initializers"));
}

#[test]
fn c3a_paid_resolver_zero_enum_is_exact_noop_with_preused_counters_and_logs() {
    for text in [
        "fn main()->i32{return 0;}",
        "fn main()->(){match missing{E::V=>{return;}}}",
    ] {
        with_index(text, |index| {
            assert_eq!(index.enum_count(), 0);
            for starting_work in [0, 7] {
                let work = WorkMeter::new(if starting_work == 0 { 0 } else { 100 });
                work.enable_observation();
                work.debit(starting_work, index.sources().eof(), "before probe")
                    .unwrap();
                work.phase("before probe");
                let events = format!("{:?}", work.events.borrow());
                let observations = format!("{:?}", work.observations.borrow());
                let mut allocator = Allocator::default();
                allocator.observer_trace_bound(8).unwrap();
                let mut prior = Vec::<u8>::new();
                allocator
                    .vector_exact(&mut prior, 1, "before probe")
                    .unwrap();
                let trace = format!("{:?}", allocator.trace);
                let (result, stats) = super::super::reviewer_source::integration_measured(|| {
                    probe_enum_resolver_storage(index, &work, &mut allocator)
                });
                assert!(result.unwrap().is_none());
                assert_eq!(stats, (0, 0, 0));
                assert_eq!(work.used(), starting_work);
                assert_eq!(format!("{:?}", work.events.borrow()), events);
                assert_eq!(format!("{:?}", work.observations.borrow()), observations);
                assert_eq!(allocator.attempts, 1);
                assert_eq!(format!("{:?}", allocator.trace), trace);
            }
        });
    }
}

#[test]
fn c3a_paid_resolver_reports_checked_attempt_delta_without_resetting_allocator() {
    with_index("enum Unused{V} fn main()->i32{return 0;}", |index| {
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(32).unwrap();
        let mut prior = Vec::<u8>::new();
        allocator
            .vector_exact(&mut prior, 1, "before probe")
            .unwrap();
        let observation = probe_enum_resolver_storage(index, &WorkMeter::default(), &mut allocator)
            .unwrap()
            .unwrap();
        assert_eq!(observation.reservation_attempts, 13);
        assert_eq!(allocator.attempts, 14);
        assert_eq!(allocator.trace.len(), 14);
        assert_eq!(allocator.trace[0].kind, "before probe");
        assert!(!allocator.observer_trace_overflow);
    });
}

#[test]
fn c3a_paid_resolver_final_work_failures_drop_parts_constructed_inside_heap_window() {
    with_index("enum Unused{V} fn main()->i32{return 0;}", |index| {
        // Independently: preflight4 + i32 result query7 + local recount3 +
        // name inventory1 =15; completed-HIR inventory13 + reconcile17 =45.
        // Every chosen failure occurs after all 13 exact reserves succeeded.
        for limit in [15, 20, 28, 44, 45, 46] {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(13).unwrap();
            let (_, (_, live, peak)) = super::super::reviewer_source::integration_measured(|| {
                let work = WorkMeter::new(limit);
                let result = probe_enum_resolver_storage(index, &work, &mut allocator);
                assert_eq!(result.is_ok(), limit >= 45);
                assert_eq!(work.used(), limit.min(45));
                if limit < 45 {
                    assert_eq!(result.as_ref().unwrap_err()[0].code, "E0400");
                } else {
                    assert_eq!(
                        result.as_ref().unwrap().as_ref().unwrap().retained_bytes,
                        480
                    );
                }
                drop(result);
            });
            assert_eq!(
                live, 0,
                "probe retained parts after final work limit {limit}"
            );
            assert!(peak > 0);
            assert_eq!(allocator.attempts, 13);
            assert_eq!(allocator.trace.len(), 13);
            assert!(allocator.trace.iter().all(|event| event.success));
            assert!(!allocator.observer_trace_overflow);
            println!("C3A_RESOLVER_FINAL_WORK limit={limit} resolver_reserves=13 live={live} peak={peak}");
        }
    });
}

#[test]
fn c3a_paid_resolver_imported_unused_enum_selects_inert_project_observation() {
    use crate::frontend::project::{ProjectLimits, ProjectSources};
    struct Files(std::path::PathBuf);
    impl Drop for Files {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let files =
        Files(std::env::temp_dir().join(format!("oxid-c3a-paid-imported-{}", std::process::id())));
    std::fs::create_dir(&files.0).unwrap();
    std::fs::write(
        files.0.join("main.ox"),
        "mod child;use crate::child::Unused as Alias;fn main()->i32{return 0;}",
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
    // Index and resolver requests share this separately prepaid observer log.
    allocator.observer_trace_bound(256).unwrap();
    let work = WorkMeter::default();
    let index = index::collect_enum_candidate(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap();
    let before = allocator.attempts;
    let (observation, (_, live, _)) = super::super::reviewer_source::integration_measured(|| {
        probe_enum_resolver_storage(&index, &WorkMeter::default(), &mut allocator)
            .unwrap()
            .unwrap()
    });
    assert_eq!(live, 0);
    assert_eq!(index.enum_count(), 1);
    assert_eq!(observation.reservation_attempts, 13);
    assert_eq!(allocator.attempts - before, 13);
    assert!(!allocator.observer_trace_overflow);
    assert_eq!(
        (
            observation.retained_bytes,
            observation.scratch_capacity_bytes
        ),
        (480, 304 + 12 * 8)
    );
    assert_eq!(
        observation.capacities,
        [0, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 12]
    );
    assert_eq!(
        resolve_project(&index, &WorkMeter::default()).unwrap_err()[0].code,
        "E0101"
    );
}

#[test]
fn c3a_paid_resolver_allocator_counter_overflow_is_separate_from_injected_failures() {
    for text in [
        "fn main()->i32{return 0;}",
        "enum Unused{V} fn main()->i32{return 0;}",
    ] {
        with_index(text, |index| {
            let enum_bearing = index.enum_count() != 0;
            let work = if enum_bearing {
                WorkMeter::default()
            } else {
                WorkMeter::new(0)
            };
            let mut allocator = Allocator {
                attempts: usize::MAX,
                ..Allocator::default()
            };
            allocator.observer_trace_bound(1).unwrap();
            let (_, stats) = super::super::reviewer_source::integration_measured(|| {
                let result = probe_enum_resolver_storage(index, &work, &mut allocator);
                if enum_bearing {
                    let errors = result.as_ref().unwrap_err();
                    assert_eq!(errors.len(), 1);
                    assert_eq!(errors[0].code, "E0400");
                    assert!(errors[0].message.contains("overflow"));
                } else {
                    assert!(result.as_ref().unwrap().is_none());
                }
                drop(result);
            });
            assert_eq!(stats.1, 0);
            if !enum_bearing {
                assert_eq!(stats, (0, 0, 0));
                assert_eq!(work.used(), 0);
            }
            // request() rejects checked counter addition before try_reserve or
            // a ReserveEvent. This is not an injected allocation-failure row.
            assert_eq!(allocator.attempts, usize::MAX);
            assert!(allocator.trace.is_empty());
            assert!(!allocator.observer_trace_overflow);
        });
    }
}

#[test]
fn c3_t1_private_probe_selects_enum_free_before_work_or_storage() {
    for text in [
        "fn main()->i32{return 0;}",
        "fn main()->(){match missing{E::V=>{return;}}}",
        "enum Unused{V} fn main()->i32{return 0;}",
    ] {
        with_index(text, |index| {
            let work = WorkMeter::new(0);
            work.enable_observation();
            let mut allocator = Allocator {
                attempts: 7,
                ..Allocator::default()
            };
            let (result, measured) = super::super::reviewer_source::integration_measured(
                || -> Result<Option<EnumTypeStorageObservation>, Vec<Diagnostic>> {
                    probe_enum_type_storage(index, &work, &mut allocator)
                },
            );
            if index.enum_count() == 0 {
                assert!(result.unwrap().is_none());
                assert_eq!(measured, (0, 0, 0));
            } else {
                let errors = result.unwrap_err();
                assert_eq!(errors.len(), 1);
                // The private selector now reaches metered preflight, which
                // fails before the first affected reserve under this zero cap.
                assert_eq!(
                    (errors[0].code, errors[0].stage),
                    ("E0400", "resolve-project")
                );
                assert_eq!(errors[0].message, "declaration index work limit exceeded");
            }
            assert_eq!(allocator.attempts, 7);
            assert_eq!(work.used(), 0);
            assert!(work.events.borrow().is_empty());
            assert!(work.observations.borrow().is_empty());
        });
    }
}

#[test]
fn c3_t1_enum_admission_rejects_ordinary_typing_before_phase_or_buffers() {
    let mut sources = SourceMap::new();
    let file = sources.add(
        "denied-enum-type-owner.ox".into(),
        "fn main()->i32{return 0;}".into(),
    );
    let source = sources.get(file);
    let ast = parser::parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    let work = WorkMeter::new(0);
    work.enable_observation();
    let mut resolved = resolve(source, &ast).unwrap();
    // Isolated synthetic owner for negative admission tests only. It has no
    // paid seed/provenance and is never passed to a positive checker path.
    resolved.admission = SourceAdmission::ObserveEnumTypes;
    resolved.work = MeterOwner::Borrowed(&work);
    assert!(!resolved.admission.executable());
    assert!(!resolved.admission.allows_lowering());
    let errors = super::super::typeck::check(resolved).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!((errors[0].code, errors[0].stage), ("E0500", "type"));
    assert_eq!(
        errors[0].message,
        "paid enum type observation requires its private checker"
    );
    assert!(work.events.borrow().is_empty());
    assert!(work.observations.borrow().is_empty());
    assert_eq!(work.used(), 0);
}

#[test]
fn c3_t1_denied_probe_and_admission_actual_layouts() {
    macro_rules! layout {
        ($($ty:ty),* $(,)?) => { $(
            println!("C3_T1_EARLY_LAYOUT {} {} {}", stringify!($ty), std::mem::size_of::<$ty>(), std::mem::align_of::<$ty>());
        )* };
    }
    layout!(
        SourceAdmission,
        Option<SourceAdmission>,
        IndexOwner<'static>,
        MeterOwner<'static>,
        ResolvedOwnedProgram<'static>,
        DeniedTypeProbeCarriers
    );
    assert_eq!(
        denied_type_probe_carrier_bytes(),
        std::mem::size_of::<DeniedTypeProbeCarriers>()
    );
}

#[test]
fn c3_t1_enum_admission_downstream_fences_precede_inventory_and_allocation() {
    for text in ["", "fn main()->(){return;}"] {
        let mut sources = SourceMap::new();
        let file = sources.add("enum-downstream-fence.ox".into(), text.into());
        let source = sources.get(file);
        let ast = parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::OwnedCandidate,
        )
        .unwrap();
        let work = WorkMeter::new(0);
        work.enable_observation();
        let mut resolved = resolve_in_map(source, &ast, &sources).unwrap();
        // Negative admission fixture only: never a paid owner or checker seed.
        resolved.admission = SourceAdmission::ObserveEnumTypes;
        resolved.work = MeterOwner::Borrowed(&work);
        super::super::typeck::assert_enum_observation_downstream_fences(resolved);
        assert_eq!(work.used(), 0);
        assert!(work.events.borrow().is_empty());
        assert!(work.observations.borrow().is_empty());
    }
}

#[test]
fn c3_t1_inhabited_denied_selector_prices_its_complete_return_representation() {
    use std::mem::{align_of, size_of};
    type Returned = Result<Option<EnumTypeStorageObservation>, Vec<Diagnostic>>;
    macro_rules! role {
        ($field:ident : $ty:ty) => {{
            let _: for<'a> fn(&'a DeniedTypeProbeCarriers) -> &'a $ty = |model| &model.$field;
            (
                std::mem::offset_of!(DeniedTypeProbeCarriers, $field),
                size_of::<$ty>(),
                align_of::<$ty>(),
            )
        }};
    }
    let roles = [
        role!(index: &'static DeclarationIndex<'static>),
        role!(work: &'static WorkMeter),
        role!(allocator: &'static mut Allocator),
        role!(enum_count: usize),
        role!(source: SourceOwner<'static>),
        role!(origin: Span),
        role!(returned: Returned),
    ];
    assert_eq!(roles.len(), 7);
    let mut occupied = 0;
    for (i, (offset, bytes, alignment)) in roles.iter().copied().enumerate() {
        assert_eq!(offset % alignment, 0);
        assert!(offset + bytes <= size_of::<DeniedTypeProbeCarriers>());
        occupied += bytes;
        for (j, (other, width, _)) in roles.iter().copied().enumerate() {
            if i != j {
                assert!(offset + bytes <= other || other + width <= offset);
            }
        }
    }
    assert_eq!(occupied, size_of::<DeniedTypeProbeCarriers>());
    let unchanged = size_of::<&DeclarationIndex<'_>>()
        + size_of::<&WorkMeter>()
        + size_of::<&mut Allocator>()
        + size_of::<usize>()
        + size_of::<SourceOwner<'_>>()
        + size_of::<Span>();
    // Historical standard Result type only; no fake previous owner/model value.
    let old_return = size_of::<Result<Option<std::convert::Infallible>, Vec<Diagnostic>>>();
    assert_eq!(
        denied_type_probe_carrier_bytes(),
        unchanged + size_of::<Returned>()
    );
    assert_eq!(enum_type_observation_return_bytes(), size_of::<Returned>());
    let old_carrier = unchanged + old_return;
    let delta = denied_type_probe_carrier_bytes() - old_carrier;
    #[cfg(target_pointer_width = "64")]
    assert_eq!(
        (
            old_carrier,
            old_return,
            size_of::<Returned>(),
            denied_type_probe_carrier_bytes(),
            delta
        ),
        (112, 24, 896, 984, 872)
    );
    println!("C3_T1_DENIED_SELECTOR_LAYOUT fields={} typed_bytes={} carrier={} return={} old_carrier={} fixed_delta={}",
        roles.len(), occupied, denied_type_probe_carrier_bytes(), size_of::<Returned>(), old_carrier, delta);
}

#[test]
fn c3_t1_first_tiny_source_observations_match_independent_ledgers_and_drop_all_buffers() {
    use super::super::typeck::{FlowSummary, TypeFrame, TypedBody};
    use std::mem::size_of;
    // Frozen from the independent pre-execution source ledger, not a trace or
    // returned observation. Marks and loops reserve1 even in loop-free main.
    for (text, has_body) in [
        ("enum Unused{V}", false),
        ("enum Unused{V} fn main()->i32{return 0;}", true),
    ] {
        for preused in [false, true] {
            with_index(text, |index| {
                let mut allocator = Allocator::default();
                allocator.observer_trace_bound(32).unwrap(); // Before any prefix row.
                let mut sentinel = Vec::<u8>::new();
                if preused {
                    allocator
                        .vector_exact(&mut sentinel, 3, "typed observation sentinel")
                        .unwrap();
                    sentinel.extend_from_slice(&[23, 41, 59]);
                }
                let before = allocator.attempts;
                let prefix_rows = allocator.trace.len();
                let trace_capacity = allocator.trace.capacity();
                let sentinel_capacity = sentinel.capacity();
                let work = WorkMeter::default(); // No growing work/event log in this window.
                let (result, heap) = super::super::reviewer_source::integration_measured(|| {
                    probe_enum_type_storage(index, &work, &mut allocator)
                });
                let facts = result.unwrap().unwrap();
                assert_eq!(
                    heap.1, 0,
                    "selected buffers must be gone while fixed facts live"
                );
                if has_body {
                    assert!(heap.0 > 0 && heap.2 > 0);
                } else {
                    assert_eq!(heap, (0, 0, 0));
                }
                assert!(work.used() > 0);
                assert!(work.events.borrow().is_empty() && work.observations.borrow().is_empty());
                assert_eq!(allocator.trace.capacity(), trace_capacity);
                assert_eq!(sentinel.capacity(), sentinel_capacity);
                assert_eq!(
                    sentinel.as_slice(),
                    if preused { &[23, 41, 59][..] } else { &[][..] }
                );
                assert_eq!(before, usize::from(preused));
                assert_eq!(prefix_rows, usize::from(preused));
                if preused {
                    let prefix = &allocator.trace[0];
                    assert_eq!(
                        (
                            prefix.kind,
                            prefix.length,
                            prefix.element_bytes,
                            prefix.success
                        ),
                        ("typed observation sentinel", 3, 1, true)
                    );
                }
                let resolver_attempts = if has_body { 13 } else { 3 };
                let typed_attempts = if has_body { 12 } else { 1 };
                assert_eq!(facts.resolver.reservation_attempts, resolver_attempts);
                assert_eq!(facts.typed.typed_attempts, typed_attempts);
                assert_eq!(
                    allocator.attempts.checked_sub(before).unwrap(),
                    facts
                        .resolver
                        .reservation_attempts
                        .checked_add(facts.typed.typed_attempts)
                        .unwrap()
                );
                assert_eq!(
                    allocator.trace.len() - prefix_rows,
                    resolver_attempts + typed_attempts
                );
                assert!(!allocator.observer_trace_overflow);
                assert!(allocator.trace[prefix_rows..].iter().all(|row| row.success));
                assert_eq!(
                    facts.typed.materialized_vectors,
                    if has_body {
                        [1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 1, 1]
                    } else {
                        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
                    }
                );
                assert_eq!(
                    facts.typed.capacities,
                    if has_body {
                        [1, 0, 1, 1, 1, 1, 1, 0, 11, 0, 0, 0, 1, 1]
                    } else {
                        [0; 14]
                    }
                );
                assert_eq!(
                    facts.typed.retained_lengths,
                    if has_body {
                        [1, 1, 1, 1, 0, 0, 1, 1]
                    } else {
                        [0; 8]
                    }
                );
                assert_eq!(
                    facts.resolver.retained_counts,
                    if has_body {
                        [0, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 0, 0]
                    } else {
                        [0; 13]
                    }
                );
                assert_eq!(
                    facts.resolver.capacities,
                    if has_body {
                        [0, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 12]
                    } else {
                        [0; 18]
                    }
                );
                let resolver_backing = if has_body {
                    size_of::<Signature>()
                        + size_of::<Function>()
                        + size_of::<Expr>()
                        + size_of::<BodyBlock>()
                        + size_of::<Stmt>()
                } else {
                    0
                };
                let resolver_scratch = if has_body {
                    size_of::<usize>() + size_of::<LoopId>() + 12 * size_of::<ResolveFrame>()
                } else {
                    0
                };
                let typed_backing = if has_body {
                    size_of::<TypedBody>()
                        + 2 * size_of::<Option<Projection>>()
                        + size_of::<Vec<Option<Projection>>>()
                        + size_of::<FlowSummary>()
                        + size_of::<ValueTy>()
                } else {
                    0
                };
                let staging = if has_body {
                    size_of::<Option<ValueTy>>() + size_of::<Option<FlowSummary>>()
                } else {
                    0
                };
                let scratch = if has_body {
                    11 * size_of::<TypeFrame>()
                } else {
                    0
                };
                assert_eq!(
                    (
                        facts.resolver.retained_bytes,
                        facts.resolver.scratch_capacity_bytes
                    ),
                    (resolver_backing, resolver_scratch)
                );
                assert_eq!(
                    (
                        facts.typed.retained_backing_bytes,
                        facts.typed.staging_backing_bytes,
                        facts.typed.scratch_backing_bytes
                    ),
                    (typed_backing, staging, scratch)
                );
                assert_eq!(
                    (
                        facts.typed.path_vectors,
                        facts.typed.path_length_fields,
                        facts.typed.path_capacity_fields
                    ),
                    (0, 0, 0)
                );
                assert_eq!(
                    (
                        facts.typed.materialized_path_bytes,
                        facts.typed.retained_path_bytes,
                        facts.typed.precharged_path_bytes
                    ),
                    (0, 0, 0)
                );
                assert_eq!(facts.typed.final_cell, facts.resolver.plan.total);
                assert_eq!(
                    facts.typed.typed_attempts,
                    facts.typed.materialized_vectors.iter().sum::<usize>()
                        + facts.typed.path_vectors
                );
                assert_eq!(
                    (
                        facts.resolver.plan.counts.functions,
                        facts.resolver.plan.counts.blocks,
                        facts.resolver.plan.counts.statements,
                        facts.resolver.plan.counts.expressions
                    ),
                    if has_body { (1, 1, 1, 1) } else { (0, 0, 0, 0) }
                );
                println!("C3_T1_FIRST_SOURCE body={has_body} preused={preused} resolver_attempts={resolver_attempts} typed_attempts={typed_attempts} whole_attempts={} final_cell={} live={} peak={}",
                    allocator.attempts - before, facts.typed.final_cell, heap.1, heap.2);
                // Source/index, prefix payload and pre-admitted trace are still
                // live here; none can cancel a leak by dropping inside the window.
            });
        }
    }
}

#[test]
fn c3_t1_enum_free_tiny_twins_preserve_preused_storage_and_observation_state() {
    for text in ["", "fn main()->i32{return 0;}"] {
        for preused in [false, true] {
            with_index(text, |index| {
                let mut allocator = Allocator::default();
                allocator.observer_trace_bound(2).unwrap();
                let mut sentinel = Vec::<u8>::new();
                if preused {
                    allocator
                        .vector_exact(&mut sentinel, 3, "typed observation sentinel")
                        .unwrap();
                    sentinel.extend_from_slice(&[23, 41, 59]);
                }
                let before = (
                    allocator.attempts,
                    allocator.trace.len(),
                    allocator.trace.capacity(),
                    sentinel.capacity(),
                );
                let work = WorkMeter::new(0);
                work.enable_observation();
                work.phase("preserved enum-free phase"); // Outside the selected heap window.
                let event_count = work.events.borrow().len();
                let observation_count = work.observations.borrow().len();
                let (result, heap) = super::super::reviewer_source::integration_measured(|| {
                    probe_enum_type_storage(index, &work, &mut allocator)
                });
                assert!(result.unwrap().is_none());
                assert_eq!(heap, (0, 0, 0));
                assert_eq!(work.used(), 0);
                assert_eq!(
                    (work.events.borrow().len(), work.observations.borrow().len()),
                    (event_count, observation_count)
                );
                assert!(matches!(
                    work.observations.borrow().as_slice(),
                    [index::Observation::Phase("preserved enum-free phase")]
                ));
                assert_eq!(
                    (
                        allocator.attempts,
                        allocator.trace.len(),
                        allocator.trace.capacity(),
                        sentinel.capacity()
                    ),
                    before
                );
                assert!(!allocator.observer_trace_overflow);
                assert_eq!(
                    sentinel.as_slice(),
                    if preused { &[23, 41, 59][..] } else { &[][..] }
                );
                if preused {
                    let prefix = &allocator.trace[0];
                    assert_eq!(
                        (
                            prefix.kind,
                            prefix.length,
                            prefix.element_bytes,
                            prefix.success
                        ),
                        ("typed observation sentinel", 3, 1, true)
                    );
                }
            });
        }
    }
}

// Frozen independent rich-success ledger, reviewed before selector execution.
// This is test data only; no per-function observation table enters the checker.
// The enum resolver adds an unused MatchArms kind after ArrayEntries. Semantic
// counts and attempts are unchanged; its MatchArm cursor grows ResolveFrame
// from 24 to 32 bytes on 64-bit targets, adding 8 bytes per reserved frame.
struct TypedRichSuccessCase {
    id: &'static str,
    text: &'static str,
    source_bytes: usize,
    source_sha256: &'static str,
    counts: super::super::hir_budget::HirCounts,
    resolver_requests: [usize; 18],
    resolver_capacities: [usize; 18],
    resolver_lengths: [usize; 13],
    resolver_zero_requests: [usize; 18],
    typed_requests: [usize; 14],
    typed_capacities: [usize; 14],
    typed_lengths: [usize; 8],
    typed_zero_requests: [usize; 14],
    attempts: (usize, usize),
    backing_64bit: [usize; 5],
}
const TYPED_RICH_SUCCESS_A: [TypedRichSuccessCase; 4] = [
    TypedRichSuccessCase {
        id: "A1_nested_scalar_calls",
        text: "enum Unused{V} fn zero()->i32{return 0;} fn id(x:i32)->i32{return x;} fn pair(a:i32,b:i32)->i32{return a+b;} fn main()->i32{return pair(id(zero()),id(2));}",
        source_sha256: "8a5dbc4845c38416c39f5a2e76a14fea42bb66b8121a1f7f9b9ad542acfd053b",
        source_bytes: 155,
        counts: super::super::hir_budget::HirCounts {
            records: 0,
            record_fields: 0,
            max_record_fields: 0,
            functions: 4,
            parameters: 3,
            bindings: 3,
            expressions: 10,
            blocks: 4,
            statements: 4,
            calls: 4,
            call_arguments: 4,
            borrow_arguments: 0,
            record_literals: 0,
            field_initializers: 0,
            array_literals: 0,
            array_entries: 0,
            matches: 0,
            match_arms: 0,
            scope_marks: 4,
            loop_slots: 4,
            resolve_frames: 48,
            type_frames: 44,
            max_expression_depth: 3,
            max_body_depth: 1,
        },
        resolver_requests: [1, 0, 1, 4, 1, 4, 4, 4, 4, 4, 0, 0, 0, 4, 4, 4, 4, 4],
        resolver_capacities: [0, 0, 4, 3, 4, 3, 10, 4, 4, 4, 0, 0, 0, 3, 3, 4, 4, 48],
        resolver_lengths: [0, 0, 4, 3, 4, 3, 10, 4, 4, 4, 0, 0, 0],
        resolver_zero_requests: [1, 0, 0, 2, 0, 2, 0, 0, 0, 1, 0, 0, 0, 2, 2, 0, 0, 0],
        typed_requests: [1, 4, 4, 4, 4, 4, 4, 4, 4, 4, 0, 4, 4, 4],
        typed_capacities: [4, 3, 10, 4, 10, 4, 4, 0, 44, 4, 0, 3, 4, 10],
        typed_lengths: [4, 10, 4, 4, 0, 3, 4, 10],
        typed_zero_requests: [0, 2, 0, 0, 0, 0, 0, 4, 0, 1, 0, 2, 0, 0],
        attempts: (47, 49),
        backing_64bit: [3152, 1360 + 48 * 8, 1704, 248, 3008],
    },
    TypedRichSuccessCase {
        id: "A2_nested_records_root_borrow",
        text: "enum Unused{V} struct Empty{} struct Leaf{x:i32} struct Pair{left:Leaf,right:bool} struct Wide{a:i32,b:i32,c:i32,d:i32} fn take(p:&Pair)->(){return;} fn main()->i32{let e=Empty{};let p=Pair{right:true,left:Leaf{x:7}};take(&p);return 0;}",
        source_sha256: "f55171861463d4ad26788ed9e5996d3e7ec9b468a704a2c9492b9743232075df",
        source_bytes: 236,
        counts: super::super::hir_budget::HirCounts {
            records: 4,
            record_fields: 7,
            max_record_fields: 4,
            functions: 2,
            parameters: 1,
            bindings: 3,
            expressions: 7,
            blocks: 2,
            statements: 5,
            calls: 1,
            call_arguments: 1,
            borrow_arguments: 1,
            record_literals: 3,
            field_initializers: 3,
            array_literals: 0,
            array_entries: 0,
            matches: 0,
            match_arms: 0,
            scope_marks: 2,
            loop_slots: 2,
            resolve_frames: 24,
            type_frames: 22,
            max_expression_depth: 3,
            max_body_depth: 1,
        },
        resolver_requests: [1, 4, 1, 2, 1, 2, 2, 2, 2, 1, 3, 0, 0, 2, 2, 2, 2, 2],
        resolver_capacities: [4, 7, 2, 1, 2, 3, 7, 2, 5, 1, 3, 0, 0, 3, 3, 2, 2, 24],
        resolver_lengths: [4, 7, 2, 1, 2, 3, 7, 2, 5, 1, 3, 0, 0],
        resolver_zero_requests: [0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
        typed_requests: [1, 2, 2, 2, 2, 2, 2, 2, 2, 1, 3, 2, 2, 2],
        typed_capacities: [2, 3, 7, 2, 7, 2, 5, 1, 22, 1, 3, 3, 2, 7],
        typed_lengths: [2, 7, 2, 5, 0, 3, 2, 7],
        typed_zero_requests: [0, 0, 1, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1],
        attempts: (31, 27),
        backing_64bit: [3296, 752 + 24 * 8, 1272, 192, 1459],
    },
    TypedRichSuccessCase {
        id: "A3_empty_branches_loop_transfers",
        text: "enum Unused{V} fn main()->i32{let mut n=0;if true{}else{}while n<2{n=n+1;if n==1{continue;}else{break;}}return n;}",
        source_sha256: "1ca17707b5ca27e4c0db66094e99fff3ef257f9656b45a9c1cb62683558ed69c",
        source_bytes: 114,
        counts: super::super::hir_budget::HirCounts {
            records: 0,
            record_fields: 0,
            max_record_fields: 0,
            functions: 1,
            parameters: 0,
            bindings: 1,
            expressions: 12,
            blocks: 6,
            statements: 8,
            calls: 0,
            call_arguments: 0,
            borrow_arguments: 0,
            record_literals: 0,
            field_initializers: 0,
            array_literals: 0,
            array_entries: 0,
            matches: 0,
            match_arms: 0,
            scope_marks: 3,
            loop_slots: 3,
            resolve_frames: 20,
            type_frames: 17,
            max_expression_depth: 2,
            max_body_depth: 3,
        },
        resolver_requests: [1, 0, 1, 1, 1, 1, 1, 1, 6, 0, 0, 0, 0, 1, 1, 1, 1, 1],
        resolver_capacities: [0, 0, 1, 0, 1, 1, 12, 6, 8, 0, 0, 0, 0, 1, 1, 3, 3, 20],
        resolver_lengths: [0, 0, 1, 0, 1, 1, 12, 6, 8, 0, 0, 0, 0],
        resolver_zero_requests: [1, 0, 0, 1, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        typed_requests: [1, 1, 1, 1, 1, 1, 6, 1, 1, 0, 0, 1, 1, 1],
        typed_capacities: [1, 1, 12, 6, 12, 6, 8, 0, 17, 0, 0, 1, 6, 12],
        typed_lengths: [1, 12, 6, 8, 0, 1, 6, 12],
        typed_zero_requests: [0, 0, 0, 0, 0, 0, 2, 1, 0, 0, 0, 0, 0, 0],
        attempts: (18, 17),
        backing_64bit: [2888, 576 + 20 * 8, 1648, 240, 1088],
    },
    TypedRichSuccessCase {
        id: "A4_grouped_empty_array_cache",
        text: "enum Unused{V} fn main()->i32{let a:[i32;0]=(([]));return a.len();}",
        source_sha256: "7e4e77f1779d55275c51be91dbe8f169e42340acd5e44c9882dfa1f43c3d8b71",
        source_bytes: 67,
        counts: super::super::hir_budget::HirCounts {
            records: 0,
            record_fields: 0,
            max_record_fields: 0,
            functions: 1,
            parameters: 0,
            bindings: 1,
            expressions: 4,
            blocks: 1,
            statements: 2,
            calls: 0,
            call_arguments: 0,
            borrow_arguments: 0,
            record_literals: 0,
            field_initializers: 0,
            array_literals: 1,
            array_entries: 0,
            matches: 0,
            match_arms: 0,
            scope_marks: 1,
            loop_slots: 1,
            resolve_frames: 12,
            type_frames: 11,
            max_expression_depth: 3,
            max_body_depth: 1,
        },
        resolver_requests: [1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 0, 1, 1, 1, 1, 1],
        resolver_capacities: [0, 0, 1, 0, 1, 1, 4, 1, 2, 0, 0, 0, 0, 1, 1, 1, 1, 12],
        resolver_lengths: [0, 0, 1, 0, 1, 1, 4, 1, 2, 0, 0, 0, 0],
        resolver_zero_requests: [1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0],
        typed_requests: [1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 1, 1],
        typed_capacities: [1, 1, 4, 1, 4, 1, 2, 0, 11, 0, 0, 1, 1, 4],
        typed_lengths: [1, 4, 1, 2, 0, 1, 1, 4],
        typed_zero_requests: [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0],
        attempts: (14, 12),
        backing_64bit: [960, 352 + 12 * 8, 596, 92, 704],
    },
];

#[test]
fn c3_t1_path_free_rich_sources_reconcile_all_kinds_and_release_new_backing() {
    for case in &TYPED_RICH_SUCCESS_A {
        assert_typed_rich_success(case, &[], &[]);
    }
}

fn assert_typed_rich_success(
    case: &TypedRichSuccessCase,
    path_lengths: &[usize],
    typed_sequence: &[(usize, usize)],
) {
    use super::super::{
        hir_budget::ScopeName,
        typeck::{BorrowProjection, FlowSummary, TypeFrame, TypedBody},
    };
    use std::mem::size_of;
    // Literal label/type maps independently cross-check trace request identity;
    // they never provide observed Vec capacities to the implementation.
    let resolver_kinds = [
        ("paid HIR records", size_of::<Record>()),
        ("paid HIR record fields", size_of::<Field>()),
        ("paid HIR signatures", size_of::<Signature>()),
        ("paid HIR parameters", size_of::<ParameterTy>()),
        ("paid HIR functions", size_of::<Function>()),
        ("paid HIR bindings", size_of::<Binding>()),
        ("paid HIR expressions", size_of::<Expr>()),
        ("paid HIR blocks", size_of::<BodyBlock>()),
        ("paid HIR statements", size_of::<Stmt>()),
        ("paid HIR arguments", size_of::<Argument>()),
        ("paid HIR field initializers", size_of::<FieldInit>()),
        ("paid HIR array entries", size_of::<ExprId>()),
        ("paid HIR match arms", size_of::<MatchArm>()),
        ("paid HIR scope names", size_of::<ScopeName>()),
        ("paid HIR scope exits", size_of::<usize>()),
        ("paid HIR scope marks", size_of::<usize>()),
        ("paid HIR loops", size_of::<LoopId>()),
        ("paid HIR resolve frames", size_of::<ResolveFrame>()),
    ];
    let typed_kinds = [
        ("paid typed bodies", size_of::<TypedBody>()),
        ("paid typed binding stage", size_of::<Option<ParameterTy>>()),
        ("paid typed expression stage", size_of::<Option<ValueTy>>()),
        ("paid typed flow stage", size_of::<Option<FlowSummary>>()),
        (
            "paid typed expression projections",
            size_of::<Option<Projection>>(),
        ),
        (
            "paid typed statement rows",
            size_of::<Vec<Option<Projection>>>(),
        ),
        (
            "paid typed statement projections",
            size_of::<Option<Projection>>(),
        ),
        (
            "paid typed borrow projections",
            size_of::<BorrowProjection>(),
        ),
        ("paid typed frames", size_of::<TypeFrame>()),
        ("paid typed call actuals", size_of::<(ParameterTy, Span)>()),
        ("paid typed record presence", size_of::<bool>()),
        ("paid typed final bindings", size_of::<ParameterTy>()),
        ("paid typed final flows", size_of::<FlowSummary>()),
        ("paid typed final expressions", size_of::<ValueTy>()),
    ];
    assert_eq!(case.text.len(), case.source_bytes);
    for preused in [false, true] {
        with_index(case.text, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(128).unwrap();
            let mut sentinel = Vec::<u8>::new();
            if preused {
                allocator
                    .vector_exact(&mut sentinel, 3, "rich typed sentinel")
                    .unwrap();
                sentinel.extend_from_slice(&[31, 47, 83]);
            }
            let before = allocator.attempts;
            let prefix = allocator.trace.len();
            let capacities = (allocator.trace.capacity(), sentinel.capacity());
            let work = WorkMeter::default();
            let (result, heap) = super::super::reviewer_source::integration_measured(|| {
                probe_enum_type_storage(index, &work, &mut allocator)
            });
            let facts = result.unwrap().unwrap();
            assert_eq!(heap.1, 0, "{} preused={preused}", case.id);
            assert!(heap.0 > 0 && heap.2 > 0);
            assert!(!work.observing());
            assert!(work.events.borrow().is_empty() && work.observations.borrow().is_empty());
            assert_eq!(
                (allocator.trace.capacity(), sentinel.capacity()),
                capacities
            );
            assert_eq!(
                sentinel.as_slice(),
                if preused { &[31, 47, 83][..] } else { &[][..] }
            );
            assert_eq!(
                (before, prefix),
                (usize::from(preused), usize::from(preused))
            );
            if preused {
                let row = &allocator.trace[0];
                assert_eq!(
                    (row.kind, row.length, row.element_bytes, row.success),
                    ("rich typed sentinel", 3, 1, true)
                );
            }
            assert!(!allocator.observer_trace_overflow);
            let suffix = &allocator.trace[prefix..];
            assert!(suffix.iter().all(|row| row.success));
            assert_eq!(facts.resolver.plan.counts, case.counts);
            assert_eq!(
                (
                    facts.resolver.reservation_attempts,
                    facts.typed.typed_attempts
                ),
                case.attempts
            );
            assert_eq!(
                allocator.attempts.checked_sub(before).unwrap(),
                facts
                    .resolver
                    .reservation_attempts
                    .checked_add(facts.typed.typed_attempts)
                    .unwrap()
            );
            assert_eq!(suffix.len(), case.attempts.0 + case.attempts.1);
            assert_eq!(facts.resolver.retained_counts, case.resolver_lengths);
            assert_eq!(facts.resolver.capacities, case.resolver_capacities);
            assert_eq!(facts.typed.materialized_vectors, case.typed_requests);
            assert_eq!(facts.typed.capacities, case.typed_capacities);
            assert_eq!(facts.typed.retained_lengths, case.typed_lengths);
            for (k, (label, width)) in resolver_kinds.iter().copied().enumerate() {
                let rows = suffix.iter().filter(|row| row.kind == label);
                assert_eq!(
                    rows.clone().count(),
                    case.resolver_requests[k],
                    "{} {label}",
                    case.id
                );
                assert_eq!(
                    rows.clone().filter(|row| row.length == 0).count(),
                    case.resolver_zero_requests[k]
                );
                assert!(rows.clone().all(|row| row.element_bytes == width));
                assert_eq!(
                    rows.map(|row| row.length).sum::<usize>(),
                    case.resolver_capacities[k]
                );
            }
            for (k, (label, width)) in typed_kinds.iter().copied().enumerate() {
                let rows = suffix.iter().filter(|row| row.kind == label);
                assert_eq!(
                    rows.clone().count(),
                    case.typed_requests[k],
                    "{} {label}",
                    case.id
                );
                assert_eq!(
                    rows.clone().filter(|row| row.length == 0).count(),
                    case.typed_zero_requests[k]
                );
                assert!(rows.clone().all(|row| row.element_bytes == width));
                assert_eq!(
                    rows.map(|row| row.length).sum::<usize>(),
                    case.typed_capacities[k]
                );
            }
            assert_eq!(
                case.resolver_requests.iter().sum::<usize>(),
                case.attempts.0
            );
            assert_eq!(
                case.typed_requests.iter().sum::<usize>() + path_lengths.len(),
                case.attempts.1
            );
            let typed_rows = &suffix[case.attempts.0..];
            if !typed_sequence.is_empty() {
                assert_eq!(typed_rows.len(), typed_sequence.len());
                for (row, &(kind, slots)) in typed_rows.iter().zip(typed_sequence) {
                    let (label, width) = if kind == typed_kinds.len() {
                        ("paid typed projection fields", size_of::<FieldId>())
                    } else {
                        typed_kinds[kind]
                    };
                    assert_eq!(
                        (row.kind, row.length, row.element_bytes),
                        (label, slots, width)
                    );
                }
            }
            let path_rows: Vec<_> = typed_rows
                .iter()
                .enumerate()
                .filter(|(_, row)| row.kind == "paid typed projection fields")
                .collect();
            assert_eq!(path_rows.len(), path_lengths.len());
            for ((ordinal, row), &length) in path_rows.iter().zip(path_lengths) {
                assert_eq!(
                    (row.length, row.element_bytes),
                    (length, size_of::<FieldId>())
                );
                // The supplied allocator's earlier sentinel remains outside the interval.
                let absolute = before
                    .checked_add(case.attempts.0)
                    .unwrap()
                    .checked_add(*ordinal + 1)
                    .unwrap();
                let original = &allocator.trace[absolute - 1];
                assert_eq!((original.kind, original.length), (row.kind, length));
            }
            let resolver_backing = (0..13)
                .map(|k| case.resolver_capacities[k] * resolver_kinds[k].1)
                .sum::<usize>();
            let resolver_scratch = (13..18)
                .map(|k| case.resolver_capacities[k] * resolver_kinds[k].1)
                .sum::<usize>();
            let typed_backing = [0, 4, 5, 6, 7, 11, 12, 13]
                .into_iter()
                .map(|k| case.typed_capacities[k] * typed_kinds[k].1)
                .sum::<usize>();
            let staging = [1, 2, 3]
                .into_iter()
                .map(|k| case.typed_capacities[k] * typed_kinds[k].1)
                .sum::<usize>();
            let scratch = [8, 9, 10]
                .into_iter()
                .map(|k| case.typed_capacities[k] * typed_kinds[k].1)
                .sum::<usize>();
            let backing = [
                resolver_backing,
                resolver_scratch,
                typed_backing,
                staging,
                scratch,
            ];
            assert_eq!(
                [
                    facts.resolver.retained_bytes,
                    facts.resolver.scratch_capacity_bytes,
                    facts.typed.retained_backing_bytes,
                    facts.typed.staging_backing_bytes,
                    facts.typed.scratch_backing_bytes
                ],
                backing
            );
            #[cfg(target_pointer_width = "64")]
            assert_eq!(backing, case.backing_64bit);
            assert_eq!(resolver_backing, facts.resolver.plan.resolved);
            let expression_slack = case.counts.expressions - case.resolver_lengths[6];
            assert_eq!(
                case.resolver_capacities[6] - case.resolver_lengths[6],
                expression_slack
            );
            assert_eq!(case.typed_capacities[2], case.resolver_lengths[6]);
            assert_eq!(case.typed_capacities[4], case.resolver_lengths[6]);
            assert_eq!(case.typed_capacities[13], case.resolver_lengths[6]);
            assert_eq!(
                facts.resolver.plan.typed - typed_backing,
                expression_slack * (size_of::<ValueTy>() + size_of::<Option<Projection>>())
            );
            assert_eq!(
                facts.resolver.plan.staging - staging,
                case.counts.functions
                    * (size_of::<Vec<Option<ValueTy>>>()
                        + size_of::<Vec<Option<ParameterTy>>>()
                        + size_of::<Vec<Option<FlowSummary>>>())
                    + expression_slack * size_of::<Option<ValueTy>>()
            );
            assert!(resolver_scratch <= facts.resolver.plan.resolver_scratch);
            assert!(scratch <= facts.resolver.plan.typeck_scratch);
            assert_eq!(
                (
                    facts.typed.path_vectors,
                    facts.typed.path_length_fields,
                    facts.typed.path_capacity_fields
                ),
                (
                    path_lengths.len(),
                    path_lengths.iter().sum(),
                    path_lengths.iter().sum()
                )
            );
            let path_bytes = path_lengths.iter().sum::<usize>() * size_of::<FieldId>();
            assert_eq!(
                (
                    facts.typed.materialized_path_bytes,
                    facts.typed.retained_path_bytes,
                    facts.typed.precharged_path_bytes
                ),
                (path_bytes, path_bytes, path_bytes)
            );
            assert_eq!(
                facts
                    .typed
                    .final_cell
                    .checked_sub(facts.resolver.plan.total),
                Some(path_bytes)
            );
            assert_eq!(
                facts.typed.typed_attempts,
                facts.typed.materialized_vectors.iter().sum::<usize>() + facts.typed.path_vectors
            );
            if case.id == "A2_nested_records_root_borrow" {
                assert!(facts.typed.capacities.iter().all(|capacity| *capacity > 0));
                assert_eq!(
                    (facts.typed.capacities[7], facts.typed.retained_lengths[4]),
                    (1, 0)
                );
                assert_eq!(
                    (
                        facts.typed.capacities[10],
                        case.counts.record_literals * case.counts.max_record_fields
                    ),
                    (3, 12)
                );
            }
            let marker = if case.id.starts_with('A') {
                "C3_T1_RICH_A"
            } else {
                "C3_T1_PROJECTION_B"
            };
            println!("{marker} case={} expected_source_sha256={} preused={preused} resolver_attempts={} typed_attempts={} whole={} backing={backing:?} live={} peak={}",
                case.id, case.source_sha256, case.attempts.0, case.attempts.1, suffix.len(), heap.1, heap.2);
        });
    }
}

#[test]
fn c3_t1_rich_root_borrow_and_grouped_empty_array_events_are_independent_runs() {
    // Event logs intentionally grow in these separate runs. They are not
    // included in, or subtracted from, the clean-drop heap measurements above.
    for case_index in [1, 3] {
        let case = &TYPED_RICH_SUCCESS_A[case_index];
        with_index(case.text, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(128).unwrap();
            let work = WorkMeter::default();
            work.enable_observation();
            let facts = probe_enum_type_storage(index, &work, &mut allocator)
                .unwrap()
                .unwrap();
            assert_eq!(allocator.attempts, case.attempts.0 + case.attempts.1);
            assert!(!allocator.observer_trace_overflow);
            assert_eq!(
                (facts.typed.path_vectors, facts.typed.precharged_path_bytes),
                (0, 0)
            );
            assert!(allocator
                .trace
                .iter()
                .all(|row| row.kind != "paid typed projection fields"));
            let observations = work.observations.borrow();
            assert!(!observations
                .iter()
                .any(|event| matches!(event, index::Observation::Projection { .. })));
            if case_index == 1 {
                let mut borrows = 0;
                for event in observations.iter() {
                    if let index::Observation::BorrowArgument {
                        function,
                        origin,
                        binding,
                        ty,
                    } = event
                    {
                        borrows += 1;
                        assert_eq!((*function, *binding), (DefId(1), 1));
                        assert_eq!(index.sources().text(*origin).unwrap(), "&p");
                        assert_eq!(
                            *ty,
                            ParameterTy::Reference {
                                referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(2))),
                                kind: BorrowKind::Shared,
                            }
                        );
                    }
                }
                assert_eq!(borrows, 1);
                assert_eq!(
                    (facts.typed.capacities[7], facts.typed.retained_lengths[4]),
                    (1, 0)
                );
                let presence_requests: Vec<_> = allocator
                    .trace
                    .iter()
                    .filter(|row| row.kind == "paid typed record presence")
                    .map(|row| row.length)
                    .collect();
                assert_eq!(presence_requests, [0, 2, 1]);
            } else {
                let array = ValueTy::Owned(AggregateTy::FixedArray(
                    FixedArrayTy::check(Ty::I32, 0).unwrap(),
                ));
                let expected_spans = ["[]", "([])", "(([]))", "a.len()"];
                let expected_types = [array, array, array, ValueTy::Scalar(Ty::I32)];
                let mut next = 0;
                for event in observations.iter() {
                    if let index::Observation::Expression {
                        function,
                        origin,
                        ty,
                        field,
                    } = event
                    {
                        assert!(next < expected_spans.len());
                        assert_eq!(*function, DefId(0));
                        assert_eq!(index.sources().text(*origin).unwrap(), expected_spans[next]);
                        assert_eq!(*ty, expected_types[next]);
                        assert!(field.is_none());
                        next += 1;
                    }
                }
                assert_eq!(next, 4); // The pretyped leaf's cache revisit emits no fifth event.
                assert!(!observations
                    .iter()
                    .any(|event| matches!(event, index::Observation::BorrowArgument { .. })));
            }
        });
    }
}

// Frozen projection-success batch B: reviewed source-derived counts and reserve
// order. One indexed-store target wrapper per source is not retained in HIR.
const TYPED_PROJECTION_SUCCESS_B: [TypedRichSuccessCase; 3] = [
    TypedRichSuccessCase {
        id: "B1_owned_one_hop",
        text: "enum Unused{V} struct R{n:i32,a:[i32;2]} fn read(xs:&[i32],ys:&mut[i32])->i32{return xs[0]+ys.len();} fn main()->i32{let mut r=R{n:1,a:[2,3]};let whole=[4,5];r.n=6;r.a[0]=7;return r.n+r.a[1]+r.a.len()+read(&whole,&mut r.a);}",
        source_sha256: "bab1974afaee4bc84d33756cd810a286ab112926f9077bef94255630d15b4470",
        source_bytes: 224,
        counts: super::super::hir_budget::HirCounts {
            records: 1,
            record_fields: 2,
            max_record_fields: 2,
            functions: 2,
            parameters: 2,
            bindings: 4,
            expressions: 24,
            blocks: 2,
            statements: 6,
            calls: 1,
            call_arguments: 2,
            borrow_arguments: 2,
            record_literals: 1,
            field_initializers: 2,
            array_literals: 2,
            array_entries: 4,
            matches: 0,
            match_arms: 0,
            scope_marks: 2,
            loop_slots: 2,
            resolve_frames: 24,
            type_frames: 22,
            max_expression_depth: 5,
            max_body_depth: 1,
        },
        resolver_requests: [1, 1, 1, 2, 1, 2, 2, 2, 2, 1, 1, 2, 0, 2, 2, 2, 2, 2],
        resolver_capacities: [1, 2, 2, 2, 2, 4, 24, 2, 6, 2, 2, 4, 0, 4, 4, 2, 2, 24],
        resolver_lengths: [1, 2, 2, 2, 2, 4, 23, 2, 6, 2, 2, 4, 0],
        resolver_zero_requests: [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        typed_requests: [1, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 2, 2, 2],
        typed_capacities: [2, 4, 23, 2, 23, 2, 6, 2, 22, 2, 2, 4, 2, 23],
        typed_lengths: [2, 23, 2, 6, 1, 4, 2, 23],
        typed_zero_requests: [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0],
        attempts: (28, 31),
        backing_64bit: [4408, 800 + 24 * 8, 2576, 472, 1506],
    },
    TypedRichSuccessCase {
        id: "B2_reference_two_hop",
        text: "enum Unused{V} struct Leaf{n:i32,a:[i32;2]} struct Outer{leaf:Leaf} fn view(xs:&[i32])->i32{return xs[0];} fn inspect(p:&Outer)->i32{return p.leaf.n+p.leaf.a[1]+p.leaf.a.len()+view(&*p.leaf.a);} fn edit(p:&mut Outer)->(){p.leaf.n=2;p.leaf.a[0]=3;return;} fn main()->i32{let mut x=Outer{leaf:Leaf{n:1,a:[4,5]}};edit(&mut x);return inspect(&x);}",
        source_sha256: "6e38e890e50a1ecfb3a783593c611bd8acb0ab82800ee1c57d4dde9344d9e502",
        source_bytes: 343,
        counts: super::super::hir_budget::HirCounts {
            records: 2,
            record_fields: 3,
            max_record_fields: 2,
            functions: 4,
            parameters: 3,
            bindings: 4,
            expressions: 22,
            blocks: 4,
            statements: 8,
            calls: 3,
            call_arguments: 3,
            borrow_arguments: 3,
            record_literals: 2,
            field_initializers: 3,
            array_literals: 1,
            array_entries: 2,
            matches: 0,
            match_arms: 0,
            scope_marks: 4,
            loop_slots: 4,
            resolve_frames: 48,
            type_frames: 44,
            max_expression_depth: 5,
            max_body_depth: 1,
        },
        resolver_requests: [1, 2, 1, 4, 1, 4, 4, 4, 4, 3, 2, 1, 0, 4, 4, 4, 4, 4],
        resolver_capacities: [2, 3, 4, 3, 4, 4, 22, 4, 8, 3, 3, 2, 0, 4, 4, 4, 4, 48],
        resolver_lengths: [2, 3, 4, 3, 4, 4, 21, 4, 8, 3, 3, 2, 0],
        resolver_zero_requests: [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        typed_requests: [1, 4, 4, 4, 4, 4, 4, 4, 4, 3, 2, 4, 4, 4],
        typed_capacities: [4, 4, 21, 4, 21, 4, 8, 3, 44, 3, 3, 4, 4, 21],
        typed_lengths: [4, 21, 4, 8, 1, 4, 4, 21],
        typed_zero_requests: [0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0],
        attempts: (51, 56),
        backing_64bit: [5360, 1408 + 48 * 8, 2960, 448, 2963],
    },
    TypedRichSuccessCase {
        id: "B3_root_array_twin",
        text: "enum Unused{V} fn read(xs:&[i32],ys:&mut[i32])->i32{return xs[0]+ys.len();} fn main()->i32{let mut n=1;let mut a=[2,3];let whole=[4,5];n=6;a[0]=7;return n+a[1]+a.len()+read(&whole,&mut a);}",
        source_sha256: "fc13aac7335ef5556f985857724b3862f7b78cdd75961113802489ba351e401d",
        source_bytes: 189,
        counts: super::super::hir_budget::HirCounts {
            records: 0,
            record_fields: 0,
            max_record_fields: 0,
            functions: 2,
            parameters: 2,
            bindings: 5,
            expressions: 23,
            blocks: 2,
            statements: 7,
            calls: 1,
            call_arguments: 2,
            borrow_arguments: 2,
            record_literals: 0,
            field_initializers: 0,
            array_literals: 2,
            array_entries: 4,
            matches: 0,
            match_arms: 0,
            scope_marks: 2,
            loop_slots: 2,
            resolve_frames: 24,
            type_frames: 22,
            max_expression_depth: 5,
            max_body_depth: 1,
        },
        resolver_requests: [1, 0, 1, 2, 1, 2, 2, 2, 2, 1, 0, 2, 0, 2, 2, 2, 2, 2],
        resolver_capacities: [0, 0, 2, 2, 2, 5, 23, 2, 7, 2, 0, 4, 0, 5, 5, 2, 2, 24],
        resolver_lengths: [0, 0, 2, 2, 2, 5, 22, 2, 7, 2, 0, 4, 0],
        resolver_zero_requests: [1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        typed_requests: [1, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 2, 2, 2],
        typed_capacities: [2, 5, 22, 2, 22, 2, 7, 2, 22, 2, 0, 5, 2, 22],
        typed_lengths: [2, 22, 2, 7, 0, 5, 2, 22],
        typed_zero_requests: [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0],
        attempts: (26, 24),
        backing_64bit: [4176, 848 + 24 * 8, 2584, 480, 1504],
    },
];

// (typed kind ordinal, requested slots), in the frozen reserve sequence.
// Kinds 0..14 follow the label/type map in assert_typed_rich_success;
// kind 14 is the separately owned FieldId path payload.
const TYPED_PROJECTION_RESERVES_B: [&[(usize, usize)]; 3] = [
    &[
        (0, 2),   // 1: Bodies
        (1, 2),   // 2: BindingStage
        (2, 4),   // 3: ExpressionStage
        (4, 4),   // 4: ExpressionProjections
        (7, 0),   // 5: BorrowProjections
        (5, 1),   // 6: StatementRows
        (6, 1),   // 7: StatementProjections
        (3, 1),   // 8: FlowStage
        (8, 11),  // 9: TypeFrames
        (11, 2),  // 10: BindingFinal
        (12, 1),  // 11: FlowFinal
        (13, 4),  // 12: ExpressionFinal
        (1, 2),   // 13: BindingStage
        (2, 19),  // 14: ExpressionStage
        (4, 19),  // 15: ExpressionProjections
        (7, 2),   // 16: BorrowProjections
        (5, 1),   // 17: StatementRows
        (6, 5),   // 18: StatementProjections
        (3, 1),   // 19: FlowStage
        (8, 11),  // 20: TypeFrames
        (10, 2),  // 21: RecordPresence
        (14, 1),  // 22: Path
        (14, 1),  // 23: Path
        (14, 1),  // 24: Path
        (14, 1),  // 25: Path
        (14, 1),  // 26: Path
        (9, 2),   // 27: CallActuals
        (14, 1),  // 28: Path
        (11, 2),  // 29: BindingFinal
        (12, 1),  // 30: FlowFinal
        (13, 19), // 31: ExpressionFinal
    ],
    &[
        (0, 4),  // 1: Bodies
        (1, 1),  // 2: BindingStage
        (2, 2),  // 3: ExpressionStage
        (4, 2),  // 4: ExpressionProjections
        (7, 0),  // 5: BorrowProjections
        (5, 1),  // 6: StatementRows
        (6, 1),  // 7: StatementProjections
        (3, 1),  // 8: FlowStage
        (8, 11), // 9: TypeFrames
        (11, 1), // 10: BindingFinal
        (12, 1), // 11: FlowFinal
        (13, 2), // 12: ExpressionFinal
        (1, 1),  // 13: BindingStage
        (2, 8),  // 14: ExpressionStage
        (4, 8),  // 15: ExpressionProjections
        (7, 1),  // 16: BorrowProjections
        (5, 1),  // 17: StatementRows
        (6, 1),  // 18: StatementProjections
        (3, 1),  // 19: FlowStage
        (8, 11), // 20: TypeFrames
        (14, 2), // 21: Path
        (14, 2), // 22: Path
        (14, 2), // 23: Path
        (9, 1),  // 24: CallActuals
        (14, 2), // 25: Path
        (11, 1), // 26: BindingFinal
        (12, 1), // 27: FlowFinal
        (13, 8), // 28: ExpressionFinal
        (1, 1),  // 29: BindingStage
        (2, 3),  // 30: ExpressionStage
        (4, 3),  // 31: ExpressionProjections
        (7, 0),  // 32: BorrowProjections
        (5, 1),  // 33: StatementRows
        (6, 3),  // 34: StatementProjections
        (3, 1),  // 35: FlowStage
        (8, 11), // 36: TypeFrames
        (14, 2), // 37: Path
        (14, 2), // 38: Path
        (11, 1), // 39: BindingFinal
        (12, 1), // 40: FlowFinal
        (13, 3), // 41: ExpressionFinal
        (1, 1),  // 42: BindingStage
        (2, 8),  // 43: ExpressionStage
        (4, 8),  // 44: ExpressionProjections
        (7, 2),  // 45: BorrowProjections
        (5, 1),  // 46: StatementRows
        (6, 3),  // 47: StatementProjections
        (3, 1),  // 48: FlowStage
        (8, 11), // 49: TypeFrames
        (10, 1), // 50: RecordPresence
        (10, 2), // 51: RecordPresence
        (9, 1),  // 52: CallActuals
        (9, 1),  // 53: CallActuals
        (11, 1), // 54: BindingFinal
        (12, 1), // 55: FlowFinal
        (13, 8), // 56: ExpressionFinal
    ],
    &[
        (0, 2),   // 1: Bodies
        (1, 2),   // 2: BindingStage
        (2, 4),   // 3: ExpressionStage
        (4, 4),   // 4: ExpressionProjections
        (7, 0),   // 5: BorrowProjections
        (5, 1),   // 6: StatementRows
        (6, 1),   // 7: StatementProjections
        (3, 1),   // 8: FlowStage
        (8, 11),  // 9: TypeFrames
        (11, 2),  // 10: BindingFinal
        (12, 1),  // 11: FlowFinal
        (13, 4),  // 12: ExpressionFinal
        (1, 3),   // 13: BindingStage
        (2, 18),  // 14: ExpressionStage
        (4, 18),  // 15: ExpressionProjections
        (7, 2),   // 16: BorrowProjections
        (5, 1),   // 17: StatementRows
        (6, 6),   // 18: StatementProjections
        (3, 1),   // 19: FlowStage
        (8, 11),  // 20: TypeFrames
        (9, 2),   // 21: CallActuals
        (11, 3),  // 22: BindingFinal
        (12, 1),  // 23: FlowFinal
        (13, 18), // 24: ExpressionFinal
    ],
];

#[test]
fn c3_t1_projection_sources_reconcile_paths_and_release_new_backing() {
    let path_lengths: [&[usize]; 3] = [&[1; 6], &[2; 6], &[]];
    for (i, case) in TYPED_PROJECTION_SUCCESS_B.iter().enumerate() {
        assert_eq!(case.counts.expressions - case.resolver_lengths[6], 1);
        assert_typed_rich_success(case, path_lengths[i], TYPED_PROJECTION_RESERVES_B[i]);
    }
}

#[test]
fn c3_t1_projection_work_and_terminal_events_are_independent_runs() {
    // These logs are intentionally outside the clean-drop measurements. Existing
    // events expose terminal fields, not a per-site path table; they cannot form
    // an interleaved reserve/precharge timeline. Full chains and cache slots are
    // the independently reviewed source audit, plus the selector's reconciliation.
    for (case_index, case) in TYPED_PROJECTION_SUCCESS_B.iter().enumerate() {
        with_index(case.text, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(128).unwrap();
            let work = WorkMeter::default();
            work.enable_observation();
            let facts = probe_enum_type_storage(index, &work, &mut allocator)
                .unwrap()
                .unwrap();
            assert_eq!(allocator.attempts, case.attempts.0 + case.attempts.1);
            assert!(!allocator.observer_trace_overflow);
            assert!(allocator.trace.iter().all(|row| row.success));
            assert_eq!(
                (facts.typed.capacities[7], facts.typed.retained_lengths[4]),
                [(2, 1), (3, 1), (2, 0)][case_index]
            );
            assert_eq!(
                (
                    facts.typed.capacities[10],
                    case.counts.record_literals * case.counts.max_record_fields
                ),
                [(2, 2), (3, 4), (0, 0)][case_index]
            );

            let expected_path_work: &[(usize, usize)] = match case_index {
                1 => &[
                    (142, 148),
                    (151, 157),
                    (163, 169),
                    (185, 191),
                    (223, 229),
                    (234, 240),
                ],
                _ => &[],
            };
            let permission_spans: &[(usize, usize)] = match case_index {
                0 => &[
                    (160, 161),
                    (166, 167),
                    (182, 183),
                    (186, 187),
                    (193, 194),
                    (220, 221),
                ],
                1 => &[
                    (142, 146),
                    (147, 148),
                    (151, 155),
                    (156, 157),
                    (163, 167),
                    (168, 169),
                    (185, 189),
                    (190, 191),
                    (223, 227),
                    (228, 229),
                    (234, 238),
                    (239, 240),
                ],
                _ => &[],
            };
            // Inventory events carry function.end, deliberately not field spans.
            // (end start, end end, retained path vectors, retained path fields).
            let inventory_functions: &[(usize, usize, usize, usize)] = match case_index {
                0 => &[(223, 224, 6, 6)],
                1 => &[(193, 194, 4, 8), (253, 254, 2, 4)],
                _ => &[],
            };
            let ordering_spans: &[(usize, usize)] = match case_index {
                0 => &[(223, 224)],
                1 => &[(193, 194)],
                _ => &[],
            };
            let array_operations: &[(&str, usize, usize)] = match case_index {
                0 => &[
                    ("array type read", 85, 90),
                    ("array type length", 91, 99),
                    ("array type store", 164, 170),
                    ("array type read", 184, 190),
                    ("array type length", 191, 200),
                ],
                1 => &[
                    ("array type read", 99, 104),
                    ("array type read", 149, 160),
                    ("array type length", 161, 175),
                    ("array type store", 232, 243),
                ],
                _ => &[
                    ("array type read", 59, 64),
                    ("array type length", 65, 73),
                    ("array type store", 139, 143),
                    ("array type read", 155, 159),
                    ("array type length", 160, 167),
                ],
            };
            let events = work.events.borrow();
            let path_work: Vec<_> = events
                .iter()
                .filter(|event| event.operation == "record projection path")
                .map(|event| (event.origin.start, event.origin.end, event.units))
                .collect();
            let expected: Vec<_> = expected_path_work
                .iter()
                .map(|&(start, end)| (start, end, 2))
                .collect();
            assert_eq!(path_work, expected, "{}", case.id);
            let permissions: Vec<_> = events
                .iter()
                .filter(|event| {
                    event.operation == "field permission"
                        && permission_spans.contains(&(event.origin.start, event.origin.end))
                })
                .map(|event| (event.origin.start, event.origin.end, event.units))
                .collect();
            let expected: Vec<_> = permission_spans
                .iter()
                .map(|&(start, end)| (start, end, 1))
                .collect();
            assert_eq!(permissions, expected, "{}", case.id);
            for (label, field_counts) in [
                ("typed retained path", false),
                ("typed retained path field", true),
            ] {
                let actual: Vec<_> = events
                    .iter()
                    .filter(|event| event.operation == label)
                    .map(|event| (event.origin.start, event.origin.end, event.units))
                    .collect();
                let expected: Vec<_> = inventory_functions
                    .iter()
                    .flat_map(|&(start, end, vectors, fields)| {
                        std::iter::repeat_n(
                            (start, end, 1),
                            if field_counts { fields } else { vectors },
                        )
                    })
                    .collect();
                assert_eq!(actual, expected, "{} {label}", case.id);
                for &(start, end, _, _) in inventory_functions {
                    assert_eq!(&case.text[start..end], "}");
                }
            }
            let ordering: Vec<_> = events
                .iter()
                .filter(|event| event.operation == "projected borrow lookup ordering")
                .map(|event| (event.origin.start, event.origin.end, event.units))
                .collect();
            let expected: Vec<_> = ordering_spans
                .iter()
                .map(|&(start, end)| (start, end, 1))
                .collect();
            assert_eq!(ordering, expected);
            assert!(!events
                .iter()
                .any(|event| event.operation == "projected borrow inventory growth"));
            let arrays: Vec<_> = events
                .iter()
                .filter(|event| {
                    matches!(
                        event.operation,
                        "array type read"
                            | "array type length"
                            | "array type store"
                            | "array type access"
                    )
                })
                .map(|event| {
                    (
                        event.operation,
                        event.origin.start,
                        event.origin.end,
                        event.units,
                    )
                })
                .collect();
            let expected: Vec<_> = array_operations
                .iter()
                .flat_map(|&(operation, start, end)| {
                    [
                        (operation, start, end, 1),
                        ("array type access", start, end, 1),
                    ]
                })
                .collect();
            assert_eq!(arrays, expected, "{}", case.id);

            let observations = work.observations.borrow();
            let projected_expressions: Vec<_> = observations
                .iter()
                .filter_map(|event| {
                    if let index::Observation::Expression {
                        function,
                        origin,
                        ty,
                        field: Some(field),
                    } = event
                    {
                        assert_eq!(*ty, ValueTy::Scalar(Ty::I32));
                        assert_eq!(field.record, RecordId(0));
                        Some((function.0, origin.start, origin.end, field.index))
                    } else {
                        None
                    }
                })
                .collect();
            let expected: &[(usize, usize, usize, usize)] = match case_index {
                0 => &[(1, 180, 183, 0), (1, 184, 190, 1), (1, 191, 200, 1)],
                1 => &[(1, 140, 148, 0), (1, 149, 160, 1), (1, 161, 175, 1)],
                _ => &[],
            };
            assert_eq!(projected_expressions, expected);
            let projections: Vec<_> = observations
                .iter()
                .filter_map(|event| {
                    if let index::Observation::Projection {
                        operation,
                        function,
                        origin,
                        field,
                        ty,
                    } = event
                    {
                        assert_eq!(
                            (*field, *ty),
                            (
                                FieldId {
                                    record: RecordId(0),
                                    index: 0
                                },
                                ValueTy::Scalar(Ty::I32)
                            )
                        );
                        Some((*operation, function.0, origin.start, origin.end))
                    } else {
                        None
                    }
                })
                .collect();
            let expected: &[(&str, usize, usize, usize)] = match case_index {
                0 => &[("write", 1, 160, 161), ("read", 1, 182, 183)],
                1 => &[("read", 1, 142, 148), ("write", 2, 223, 229)],
                _ => &[],
            };
            assert_eq!(projections, expected);
            let borrows: Vec<_> = observations
                .iter()
                .filter_map(|event| {
                    if let index::Observation::BorrowArgument {
                        function,
                        origin,
                        binding,
                        ty,
                    } = event
                    {
                        Some((function.0, origin.start, origin.end, *binding, *ty))
                    } else {
                        None
                    }
                })
                .collect();
            let slice = |kind| ParameterTy::Reference {
                referent: BorrowedTy::ScalarSlice(Ty::I32),
                kind,
            };
            let outer = |kind| ParameterTy::Reference {
                referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(1))),
                kind,
            };
            let expected = match case_index {
                0 => vec![
                    (1, 206, 212, 1, slice(BorrowKind::Shared)),
                    (1, 213, 221, 0, slice(BorrowKind::Exclusive)),
                ],
                1 => vec![
                    (1, 181, 191, 0, slice(BorrowKind::Shared)),
                    (3, 315, 321, 0, outer(BorrowKind::Exclusive)),
                    (3, 338, 340, 0, outer(BorrowKind::Shared)),
                ],
                _ => vec![
                    (1, 173, 179, 2, slice(BorrowKind::Shared)),
                    (1, 180, 186, 1, slice(BorrowKind::Exclusive)),
                ],
            };
            assert_eq!(borrows, expected);
            println!(
                "C3_T1_PROJECTION_B_EVENTS case={} path_vectors={} path_fields={} cell_delta={}",
                case.id,
                facts.typed.path_vectors,
                facts.typed.path_length_fields,
                facts.typed.final_cell - facts.resolver.plan.total
            );
        });
    }
}
