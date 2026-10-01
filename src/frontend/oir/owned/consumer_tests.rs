use super::{consumer_fixtures::*, plan::ExecutionPlan, *};

#[test]
fn reviewed_raw_fuel_fixtures_obtain_authoritative_witnesses() {
    type Builder = fn() -> (SourceMap, RawOwnedProgram, Schedule);
    for (build, fuel, cells, bytes, native) in [
        (empty_record as Builder, 17, 6, 41, 12),
        (owned_relay as Builder, 52, 20, 148, 36),
        (shared_read as Builder, 49, 22, 172, 36),
    ] {
        let (sources, raw, schedule) = build();
        assert_eq!(schedule.fuel(), fuel);
        for budget in 0..fuel {
            assert!(schedule.failure(budget).is_some());
        }
        assert_eq!(schedule.failure(fuel), None);
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let plan = ExecutionPlan::build(&witness).unwrap();
        assert!(std::ptr::eq(plan.witness(), &witness));
        let u = plan.function(hir::DefId(0)).usage();
        assert_eq!(u.expanded_cells, cells);
        assert_eq!(u.reference_bytes, bytes);
        assert_eq!(u.native_bytes, native);
        // The accepted raw program is only inspected here; execution follows later.
        assert_eq!(witness.functions()[0].id, schedule.entry);
    }
}

#[test]
fn mixed_multifunction_plan_reservation_sweep_includes_all_nonempty_array_classes() {
    let (sources, raw, _) = super::consumer_pilot::batch();
    let p = verified::verify_owned(raw, &sources).unwrap();
    let main = &p.functions()[0];
    assert!(!main.owners.is_empty() && !main.calls.is_empty() && !main.loans.is_empty());
    let mut failures = 0;
    loop {
        match plan::fail_allocation_after(failures, || ExecutionPlan::build(&p)) {
            Ok(plan) => {
                assert_eq!(plan.functions().len(), 7);
                assert_eq!(
                    plan.function(hir::DefId(0)).owned_stages(CallSiteId(3)),
                    &[OwnerPlaceId(1)]
                );
                assert_eq!(
                    plan.function(hir::DefId(0)).borrowed_loans(CallSiteId(1)),
                    &[LoanId(1)]
                );
                break;
            }
            Err(e) => assert_eq!(e.name, "injected owned allocation failure"),
        }
        failures += 1;
    }
    assert_eq!(failures, 29); // function vector plus four arrays for each of seven functions
}
