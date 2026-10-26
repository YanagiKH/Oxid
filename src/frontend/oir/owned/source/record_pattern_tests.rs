//! Record consumption keeps raw validation and source correspondence separate.
use super::super::*;
use super::{association, budget, lower, resolve, typeck};
use crate::frontend::{
    build_config::ExecutionPolicy,
    declaration_index::{collect_originals, IndexLimits, SourceOwner, WorkMeter},
    project::{budget::Allocator, ProjectLimits, ProjectSources},
};
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) fn with_typed(
    text: &str,
    test: impl FnOnce(&typeck::TypedOwnedProgram<'_>, &SourceMap),
) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "oxid-pattern-trust-{}-{}.ox",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).unwrap();
    let project = ProjectSources::load_typed(path.to_str().unwrap(), ProjectLimits::default());
    std::fs::remove_file(path).unwrap();
    let project = project.unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_originals(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap();
    let typed = if !resolve::requires_paid_source(&index) {
        typeck::check(resolve::resolve_project(&index, &work).unwrap()).unwrap()
    } else {
        resolve::type_enum_source(&index, &work, &mut allocator).unwrap()
    };
    test(&typed, project.sources());
}

fn pattern(raw: &mut RawOwnedProgram) -> &mut OwnedInstruction {
    &mut raw
        .functions
        .iter_mut()
        .flat_map(|f| &mut f.blocks)
        .flat_map(|b| &mut b.statements)
        .find(|s| matches!(s.kind, OwnedInstruction::DestructureRecord { .. }))
        .unwrap()
        .kind
}

#[test]
fn record_pattern_raw_shape_requires_exact_coverage_fresh_destinations_and_types() {
    with_typed("struct Pair{first:[i32;1],second:[i32;1]}fn main()->i32{let pair=Pair{first:[20],second:[22]};let Pair{second:b,first:a}=pair;return a[0]+b[0];}", |typed, sources| {
        assert!(association::associate(lower::lower(typed).unwrap(), typed).is_ok());
        for mutation in 0..8 {
            let mut raw = lower::lower(typed).unwrap();
            let OwnedInstruction::DestructureRecord { source, fields } = pattern(&mut raw) else { unreachable!() };
            match mutation {
                0 => { fields.pop(); },
                1 => fields[1].0 = fields[0].0,
                2 => fields[1].1 = fields[0].1,
                3 => fields[0].1 = *source,
                4 => fields[0].0.index = usize::MAX,
                5 => fields[0].0.record.0 = usize::MAX,
                6 => fields[0].1.0 = usize::MAX,
                7 => *source = fields[0].1,
                _ => unreachable!(),
            }
            assert!(verified::verify_owned(raw, sources).is_err(), "mutation {mutation}");
        }
    });
}

#[test]
fn record_pattern_same_layout_field_swaps_cannot_change_binding_meanings() {
    for scalar in [true, false] {
        let text = if scalar {
            "struct Pair{first:i32,second:i32}fn main()->i32{let pair=Pair{first:20,second:22};let Pair{second:b,first:a}=pair;return a+b;}"
        } else {
            "struct Pair{first:[i32;1],second:[i32;1]}fn main()->i32{let pair=Pair{first:[20],second:[22]};let Pair{second:b,first:a}=pair;return a[0]+b[0];}"
        };
        with_typed(text, |typed, sources| {
            for independent in [true, false] {
                let mut raw = lower::lower(typed).unwrap();
                if scalar {
                    for statement in raw
                        .functions
                        .iter_mut()
                        .flat_map(|f| &mut f.blocks)
                        .flat_map(|b| &mut b.statements)
                    {
                        if let OwnedInstruction::ReadField { field, .. } = &mut statement.kind {
                            field.index = 1 - field.index;
                        }
                    }
                } else {
                    let OwnedInstruction::DestructureRecord { fields, .. } = pattern(&mut raw)
                    else {
                        unreachable!()
                    };
                    let first = fields[0].0;
                    fields[0].0 = fields[1].0;
                    fields[1].0 = first;
                }
                if independent {
                    assert!(verified::verify_owned(raw, sources).is_ok());
                } else {
                    assert!(association::associate(raw, typed).is_err());
                }
            }
        });
    }
}

#[test]
fn record_pattern_missing_empty_consumption_is_not_a_source_program() {
    with_typed(
        "struct Empty{}fn main()->i32{let value=Empty{};let Empty{}=value;return 42;}",
        |typed, sources| {
            let mut raw = lower::lower(typed).unwrap();
            for statement in raw
                .functions
                .iter_mut()
                .flat_map(|f| &mut f.blocks)
                .flat_map(|b| &mut b.statements)
            {
                if let OwnedInstruction::DestructureRecord { source, .. } = statement.kind {
                    statement.kind = OwnedInstruction::Discard(source);
                }
            }
            assert!(verified::verify_owned(raw, sources).is_ok());
            let mut raw = lower::lower(typed).unwrap();
            for statement in raw
                .functions
                .iter_mut()
                .flat_map(|f| &mut f.blocks)
                .flat_map(|b| &mut b.statements)
            {
                if let OwnedInstruction::DestructureRecord { source, .. } = statement.kind {
                    statement.kind = OwnedInstruction::Discard(source);
                }
            }
            assert!(association::associate(raw, typed).is_err());
        },
    );
}

#[test]
fn record_pattern_output_reservations_and_exact_metadata_limit_are_fallible() {
    with_typed("struct Pair{first:[i32;1],second:[i32;1]}fn main()->i32{let pair=Pair{first:[20],second:[22]};let Pair{second:b,first:a}=pair;return a[0]+b[0];}", |typed, sources| {
        let usage = budget::preflight(typed, budget::Limits::DEFAULT).unwrap();
        let raw = lower::lower_with_limits(typed, budget::Limits { raw_bytes: usage.raw_bytes }).unwrap();
        assert_eq!(super::super::budget::preflight(&raw, super::super::budget::Limits::DEFAULT).unwrap(), usage.analysis);
        verified::verify_owned(raw, sources).unwrap();
        assert_eq!(budget::fail_allocation_after(0, || lower::lower_with_limits(typed, budget::Limits { raw_bytes: usage.raw_bytes - 1 })).unwrap_err().kind, OwnedFailureKind::Resource("source raw payload"));
        let mut completed = false;
        for failure in 0..300 {
            match budget::fail_allocation_after(failure, || lower::lower(typed)) {
                Err(error) => assert_eq!(error.kind, OwnedFailureKind::Resource("injected source allocation failure")),
                Ok(raw) => {
                    let witness = verified::verify_owned(raw, sources).unwrap();
                    assert_eq!(execute::run(&witness, typed.entry()).unwrap(), Scalar::I32(42));
                    completed = true;
                    break;
                }
            }
        }
        assert!(completed);
    });
}

const RESOURCES: &str = "use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;use alloc::bytes::try_push;struct Pair<A,B>{first:A,second:B}fn make(n:u8)->ByteBuf{let mut b=new();try_push(&mut b,n);return b;}fn first<A,B>(pair:Pair<A,B>)->A{let Pair{second:rest,first:value}=pair;return value;}fn main()->i32{let value=first(Pair::<ByteBuf,ByteBuf>{first:make(40u8),second:make(2u8)});return len(&value)+41;}";

#[test]
fn record_pattern_typing_allocation_failures_release_paid_field_and_scope_storage() {
    with_typed(RESOURCES, |typed, _| {
        let index = typed.index();
        let work = WorkMeter::default();
        let mut baseline = Allocator::default();
        drop(resolve::type_enum_source(index, &work, &mut baseline).unwrap());
        assert!(baseline.attempts > 10);
        for failure in 1..=baseline.attempts {
            let work = WorkMeter::default();
            let mut allocator = Allocator {
                fail_at: Some(failure),
                ..Default::default()
            };
            allocator.observer_trace_bound(256).unwrap();
            let (refused, (_, live, _)) = super::reviewer_source::integration_measured(|| {
                match resolve::type_enum_source(index, &work, &mut allocator) {
                    Ok(typed) => {
                        drop(typed);
                        false
                    }
                    Err(errors) => errors.iter().any(|error| error.code == "E0400"),
                }
            });
            assert!(refused, "allocation {failure}");
            assert_eq!(live, 0, "allocation {failure}");
            assert!(!allocator.observer_trace_overflow);
            assert!(!allocator.trace[failure - 1].success);
        }
    });
}

#[test]
fn record_pattern_resources_are_reclaimed_at_every_fuel_boundary_and_return() {
    let checked = super::resource_fixtures::checked_enum_project(RESOURCES);
    for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        let mut completed = false;
        for fuel in 0..1000 {
            let (result, ledger) = byte_buffer::testing::capture(Default::default(), || {
                execute::run_result(
                    &checked.witness,
                    Some(checked.entry),
                    execute::Limits {
                        fuel,
                        ..Default::default()
                    },
                    policy,
                )
            });
            assert_eq!(
                ledger.after_cleanup.live_allocations, 0,
                "fuel {fuel}, {policy:?}"
            );
            assert_eq!(ledger.after_cleanup.live_bytes, 0);
            assert_eq!(
                ledger.after_cleanup.allocations,
                ledger.after_cleanup.releases
            );
            match result {
                Ok(value) => {
                    assert_eq!(value.to_string(), "42");
                    assert_eq!(ledger.before_cleanup.live_allocations, 0);
                    assert_eq!(ledger.after_cleanup.allocations, 2);
                    completed = true;
                    break;
                }
                Err(error) => assert_eq!(error.diagnostic(checked.project.sources()).code, "E0601"),
            }
        }
        assert!(completed);
    }
}

#[test]
fn record_pattern_failed_initialization_and_later_traps_release_all_resources() {
    for body in [
        "let Pair{first:a,second:b}=Pair::<ByteBuf,ByteBuf>{first:make(40u8),second:fail()};return 0;",
        "let Pair{first:a,second:b}=Pair::<ByteBuf,ByteBuf>{first:make(40u8),second:make(2u8)};return 1/0;",
    ] {
        let text = format!("{}fn fail()->ByteBuf{{let zero=0;let n=1/zero;return new();}}fn main()->i32{{{body}}}", RESOURCES.split("fn main").next().unwrap());
        let checked = super::resource_fixtures::checked_enum_project(&text);
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let (result, ledger) = byte_buffer::testing::capture(Default::default(), || execute::run_result(&checked.witness, Some(checked.entry), Default::default(), policy));
            assert!(result.is_err());
            assert!(ledger.after_cleanup.allocations > 0);
            assert_eq!(ledger.after_cleanup.live_allocations, 0);
            assert_eq!(ledger.after_cleanup.allocations, ledger.after_cleanup.releases);
        }
    }
}
