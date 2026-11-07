//! Range-loop source authority and resource cleanup at failure boundaries.
use super::super::*;
use super::{association, budget, lower, record_pattern_tests::with_typed, resolve};
use crate::frontend::{
    build_config::ExecutionPolicy, declaration_index::WorkMeter, project::budget::Allocator,
};

const SUM: &str = "fn main()->i32{let mut sum=0;for index in 0..7{sum=sum+index;}return sum;}";
const RESOURCES: &str = "use alloc::bytes::new;use alloc::bytes::try_push;use alloc::bytes::len;fn main()->i32{let mut count=0;for index in 0..4{let mut bytes=new();try_push(&mut bytes,index.to_u8_checked());if index==1{continue;}if index==3{break;}count=count+len(&bytes);}return count+40;}";

#[test]
fn trusted_range_lowering_keeps_its_step_independent_of_other_valid_raw_code() {
    with_typed(SUM, |typed, sources| {
        assert!(typed.index().has_range_for());
        assert!(typed.source_storage_bytes().is_some());
        fn substitute(raw: &mut RawOwnedProgram) {
            let mut changed = 0;
            for statement in raw
                .functions
                .iter_mut()
                .flat_map(|f| &mut f.blocks)
                .flat_map(|b| &mut b.statements)
            {
                if let OwnedInstruction::Scalar(Statement::Assign(Assign {
                    value: Rvalue::I32(value),
                    ..
                })) = &mut statement.kind
                {
                    if *value == 1 {
                        *value = 2;
                        changed += 1;
                    }
                }
            }
            assert_eq!(changed, 1);
        }
        let mut raw = lower::lower(typed).unwrap();
        substitute(&mut raw);
        let witness = verified::verify_owned(raw, sources).unwrap();
        assert_eq!(
            execute::run(&witness, typed.entry()).unwrap(),
            Scalar::I32(12)
        );
        // Production accepts only its fresh lowering, with no raw-code input.
        let associated = association::lower_and_associate(typed).unwrap();
        let witness = verified::verify_associated(associated).unwrap();
        assert_eq!(
            execute::run(&witness, typed.entry()).unwrap(),
            Scalar::I32(21)
        );
    });
}

#[test]
fn pure_range_loops_reclaim_every_failed_paid_typing_reservation() {
    with_typed(SUM, |typed, _| {
        let index = typed.index();
        let mut baseline = Allocator::default();
        drop(resolve::type_enum_source(index, &WorkMeter::default(), &mut baseline).unwrap());
        assert!(baseline.attempts > 10);
        for failure in 1..=baseline.attempts {
            let mut allocator = Allocator {
                fail_at: Some(failure),
                ..Default::default()
            };
            allocator.observer_trace_bound(256).unwrap();
            let (refused, (_, live, _)) = super::reviewer_source::integration_measured(|| {
                match resolve::type_enum_source(index, &WorkMeter::default(), &mut allocator) {
                    Ok(typed) => {
                        drop(typed);
                        false
                    }
                    Err(errors) => errors.iter().any(|e| e.code == "E0400"),
                }
            });
            assert!(refused, "reservation {failure}");
            assert_eq!(live, 0, "reservation {failure}");
            assert!(!allocator.observer_trace_overflow);
        }
    });
}

#[test]
fn range_lowering_reclaims_every_failed_raw_reservation() {
    with_typed(SUM, |typed, sources| {
        let mut completed = false;
        for failure in 0..400 {
            let (result, (_, live, _)) = super::reviewer_source::integration_measured(|| {
                budget::fail_allocation_after(failure, || lower::lower(typed))
            });
            match result {
                Err(error) => {
                    assert!(matches!(error.kind, OwnedFailureKind::Resource(_)));
                    assert_eq!(live, 0, "reservation {failure}");
                }
                Ok(raw) => {
                    let witness = verified::verify_owned(raw, sources).unwrap();
                    assert_eq!(
                        execute::run(&witness, typed.entry()).unwrap(),
                        Scalar::I32(21)
                    );
                    completed = true;
                    break;
                }
            }
        }
        assert!(completed);
    });
}

#[test]
fn range_transfers_release_resources_at_every_fuel_boundary() {
    let checked = super::resource_fixtures::checked_enum_project(RESOURCES);
    for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        let mut completed = false;
        for fuel in 0..1800 {
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
                "{policy:?} fuel={fuel}"
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
                    assert_eq!(ledger.after_cleanup.allocations, 4);
                    completed = true;
                    break;
                }
                Err(error) => assert_eq!(error.diagnostic(checked.project.sources()).code, "E0601"),
            }
        }
        assert!(completed);
    }
}
