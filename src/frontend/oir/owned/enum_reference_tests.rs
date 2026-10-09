//! Existing sealed reference consumer, independent raw schedules, and bounded
//! failure-before-effect controls. This module never manufactures a witness.
use super::consumer_fixtures as f;
use super::enum_consumer_fixtures as e;
use super::execute::{
    Event, FaultInjection, FaultKind, Limits, ObservationControl, OwnedRunFailure,
    ReferenceObservation, StorageObservationKind as StorageKind,
};
use super::*;
use std::mem::size_of;

fn observe(
    witness: &verified::VerifiedOwnedProgram,
    fuel: usize,
    control: ObservationControl,
) -> ReferenceObservation {
    execute::run_array_observed(
        witness,
        Some(hir::DefId(0)),
        Limits {
            fuel,
            ..Limits::default()
        },
        control,
    )
}
fn fault(
    witness: &verified::VerifiedOwnedProgram,
    at: Span,
    kind: FaultKind,
) -> ReferenceObservation {
    let observed = observe(
        witness,
        plan::MAX_FUEL,
        ObservationControl {
            fault: Some(FaultInjection { at, kind }),
            ..ObservationControl::default()
        },
    );
    assert!(observed.fault_applied, "fault {kind:?} at {at:?}");
    assert!(!observed.truncated);
    observed
}
fn assert_oracle(observed: &ReferenceObservation, schedule: &f::Schedule, fuel: usize) {
    assert!(!observed.truncated, "fuel={fuel}");
    assert!(!observed.fault_applied);
    let mut remaining = fuel;
    let mut paid = Vec::new();
    for &(span, cost) in &schedule.events {
        if remaining < cost {
            break;
        }
        remaining -= cost;
        paid.push((span, cost));
    }
    let charges: Vec<_> = observed
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Charge(span, cost) => Some((*span, *cost)),
            _ => None,
        })
        .collect();
    assert_eq!(charges, paid, "fuel={fuel}");
    assert_eq!(observed.remaining_fuel, remaining, "fuel={fuel}");
    match schedule.failure(fuel) {
        Some(span) => assert_eq!(
            observed.result,
            Err(OwnedRunFailure::Scalar(RunFailure::Fuel(span))),
            "fuel={fuel}"
        ),
        None => assert_eq!(observed.result, Ok(schedule.result), "fuel={fuel}"),
    }
    for snapshot in &observed.storage {
        assert_eq!(snapshot.guards_before, snapshot.guards_after);
    }
}
fn transfer_count(observed: &ReferenceObservation) -> usize {
    observed
        .events
        .iter()
        .filter(|event| matches!(event, Event::Transfer(..)))
        .count()
}
fn payload_reads(observed: &ReferenceObservation) -> usize {
    observed
        .events
        .iter()
        .filter(|event| matches!(event, Event::EnumPayloadRead(..)))
        .count()
}
fn bindings(observed: &ReferenceObservation) -> usize {
    observed
        .events
        .iter()
        .filter(|event| matches!(event, Event::EnumBind(..)))
        .count()
}
fn failure_owner(
    observed: &ReferenceObservation,
    frame: u64,
    owner: u64,
) -> &execute::StorageSnapshot {
    observed
        .storage
        .iter()
        .find(|snapshot| {
            snapshot.kind == StorageKind::Failure
                && snapshot.key.frame == frame
                && snapshot.key.owner == owner
        })
        .unwrap()
}

#[test]
fn enum_reference_all_payloads_results_and_written_orders_every_fuel() {
    let (sources, s) = f::context();
    let mut cases = 0;
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let order = [a, b, c, d];
                    if order
                        .iter()
                        .enumerate()
                        .any(|(i, v)| order[..i].contains(v))
                    {
                        continue;
                    }
                    for constructed in 0..4 {
                        for result in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
                            let (raw, schedule) =
                                e::mixed_case(&e::MIXED, &order, constructed, result, s(0));
                            let witness = verify_owned(raw, &sources).unwrap();
                            for fuel in 0..=schedule.fuel() {
                                let observed =
                                    observe(&witness, fuel, ObservationControl::default());
                                assert_oracle(&observed, &schedule, fuel);
                                let paid_consume = observed.events.iter().any(|event| matches!(event,Event::Charge(span,3) if *span == e::at(s(0),5)));
                                assert_eq!(bindings(&observed), usize::from(paid_consume));
                                assert_eq!(
                                    payload_reads(&observed),
                                    usize::from(paid_consume && constructed != 0)
                                );
                                let constructors = observed
                                    .storage
                                    .iter()
                                    .filter(|snapshot| snapshot.kind == StorageKind::Construction)
                                    .count();
                                assert!(constructors <= 1);
                                if !paid_consume && constructors == 1 {
                                    assert_eq!(failure_owner(&observed, 0, 0).state, 2);
                                }
                            }
                            cases += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 288);
}

#[test]
fn enum_reference_single_variant_final_dispatch_is_paid_and_checked() {
    let (sources, s) = f::context();
    for payload in e::MIXED {
        let result = payload.unwrap_or(hir::Ty::Unit);
        let (raw, schedule) = e::mixed_case(&[payload], &[0], 0, result, s(0));
        let witness = verify_owned(raw, &sources).unwrap();
        for fuel in 0..=schedule.fuel() {
            assert_oracle(
                &observe(&witness, fuel, ObservationControl::default()),
                &schedule,
                fuel,
            );
        }
        for tag in [1, u32::MAX] {
            let observed = fault(
                &witness,
                e::at(s(0), 5),
                FaultKind::EnumDispatchTag { arm: 0, tag },
            );
            assert_eq!(
                observed.result,
                Err(OwnedRunFailure::Invariant("enum tag", Some(e::at(s(0), 5))))
            );
            assert_eq!((payload_reads(&observed), bindings(&observed)), (0, 0));
            let source = failure_owner(&observed, 0, 0);
            assert_eq!((source.state, source.key.generation), (2, 2));
        }
    }
}

#[test]
fn enum_reference_invalid_tags_first_intermediate_final_and_consume_fail_closed() {
    let (sources, s) = f::context();
    let (raw, _) = e::mixed_case(&e::MIXED, &[0, 1, 2, 3], 3, hir::Ty::Unit, s(0));
    let witness = verify_owned(raw, &sources).unwrap();
    let span = e::at(s(0), 5);
    for arm in 0..4 {
        let observed = fault(
            &witness,
            span,
            FaultKind::EnumDispatchTag { arm, tag: u32::MAX },
        );
        assert_eq!(
            observed.result,
            Err(OwnedRunFailure::Invariant("enum tag", Some(span)))
        );
        assert_eq!((payload_reads(&observed), bindings(&observed)), (0, 0));
        assert_eq!(
            observed
                .events
                .iter()
                .filter(|event| matches!(event, Event::EnumTagRead(..)))
                .count(),
            arm + 1
        );
        assert_eq!(
            (
                failure_owner(&observed, 0, 0).state,
                failure_owner(&observed, 0, 0).key.generation
            ),
            (2, 2)
        );
    }
    // An in-range tag for an earlier arm cannot pass the final check.
    for kind in [
        FaultKind::EnumDispatchTag { arm: 3, tag: 0 },
        FaultKind::EnumConsumeTag { arm: 3, tag: 0 },
        FaultKind::EnumConsumeTag {
            arm: 3,
            tag: u32::MAX,
        },
    ] {
        let observed = fault(&witness, span, kind);
        assert_eq!(
            observed.result,
            Err(OwnedRunFailure::Invariant("enum tag", Some(span)))
        );
        assert_eq!((payload_reads(&observed), bindings(&observed)), (0, 0));
        assert_eq!(failure_owner(&observed, 0, 0).state, 2);
    }
    let observed = fault(&witness, span, FaultKind::EnumConsumeEpoch { arm: 3 });
    assert_eq!(
        observed.result,
        Err(OwnedRunFailure::Invariant(
            "identity epoch overflow",
            Some(span)
        ))
    );
    assert_eq!((payload_reads(&observed), bindings(&observed)), (0, 0));
    assert_eq!(failure_owner(&observed, 0, 0).state, 2);
}

#[test]
fn enum_reference_active_bool_and_unit_bytes_reject_before_binding() {
    let (sources, s) = f::context();
    for (ty, byte) in [
        (hir::Ty::Bool, 2),
        (hir::Ty::Bool, 255),
        (hir::Ty::Unit, 1),
        (hir::Ty::Unit, 255),
    ] {
        let (raw, _) = e::mixed_case(&[Some(ty)], &[0], 0, ty, s(0));
        let witness = verify_owned(raw, &sources).unwrap();
        let span = e::at(s(0), 5);
        let observed = fault(
            &witness,
            span,
            FaultKind::EnumOwnerByte {
                owner: OwnerPlaceId(0),
                offset: 4,
                byte,
            },
        );
        assert_eq!(
            observed.result,
            Err(OwnedRunFailure::Invariant("enum payload", Some(span)))
        );
        assert_eq!((payload_reads(&observed), bindings(&observed)), (1, 0));
        assert_eq!(
            (
                failure_owner(&observed, 0, 0).state,
                failure_owner(&observed, 0, 0).key.generation
            ),
            (2, 2)
        );
    }
}

#[test]
fn enum_reference_all_whole_transfers_and_loop_every_fuel() {
    let (sources, s) = f::context();
    let mut cases = vec![e::loop_case(s(0))];
    for (index, payload) in e::MIXED.iter().enumerate() {
        cases.push(e::relay_case(index, payload.unwrap_or(hir::Ty::I32), s(0)));
        cases.push(e::discard_case(*payload, s(0)));
    }
    for (raw, schedule) in cases {
        let witness = verify_owned(raw, &sources).unwrap();
        for fuel in 0..=schedule.fuel() {
            assert_oracle(
                &observe(&witness, fuel, ObservationControl::default()),
                &schedule,
                fuel,
            );
        }
    }
}

#[test]
fn enum_reference_each_transfer_validates_before_copy_and_source_transition() {
    let (sources, s) = f::context();
    // span, source, frame, already completed transfers, destination/frame/state/generation
    let transfers = [
        (9, 0, 0, 0, 1, 0, 1, 1),
        (12, 2, 0, 1, 1, 0, 2, 2),
        (14, 1, 0, 2, 3, 0, 1, 1),
        (15, 3, 0, 3, 4, 0, 0, 0),
        (16, 0, 1, 4, 4, 0, 0, 0),
        (18, 4, 0, 5, 5, 0, 1, 1),
    ];
    for constructed in [0, 1, 2, 3] {
        let (raw, _) = e::relay_case(constructed, hir::Ty::Unit, s(0));
        let witness = verify_owned(raw, &sources).unwrap();
        for &(at, source, frame, completed, destination, destination_frame, state, generation) in
            &transfers
        {
            let span = e::at(s(0), at);
            let origin = if at == 15 { e::at(s(0), 50) } else { span };
            let mut controls = vec![(
                FaultKind::EnumOwnerTag {
                    owner: OwnerPlaceId(source),
                    tag: u32::MAX,
                },
                "enum tag",
            )];
            if constructed == 1 || constructed == 3 {
                controls.push((
                    FaultKind::EnumOwnerByte {
                        owner: OwnerPlaceId(source),
                        offset: 4,
                        byte: 255,
                    },
                    "enum payload",
                ));
            }
            for (kind, message) in controls {
                let observed = fault(&witness, span, kind);
                assert_eq!(
                    observed.result,
                    Err(OwnedRunFailure::Invariant(message, Some(origin))),
                    "site={at} variant={constructed}"
                );
                assert_eq!(transfer_count(&observed), completed);
                assert_eq!(failure_owner(&observed, frame, source as u64).state, 2);
                let target = failure_owner(&observed, destination_frame, destination);
                assert_eq!((target.state, target.key.generation), (state, generation));
                if at != 12 {
                    assert_eq!(target.bytes, [0; 8]);
                } else {
                    assert_eq!(&target.bytes[..4], &(constructed as u32).to_le_bytes());
                }
                if at == 15 {
                    assert!(!observed
                        .events
                        .iter()
                        .any(|event| matches!(event, Event::Enter(hir::DefId(1), _))));
                }
            }
        }
    }
}

#[test]
fn enum_reference_transfer_epoch_preflight_precedes_any_copy() {
    let (sources, s) = f::context();
    let (raw, _) = e::relay_case(1, hir::Ty::Bool, s(0));
    let witness = verify_owned(raw, &sources).unwrap();
    for (at, owner, completed) in [
        (9, 0, 0),
        (12, 2, 1),
        (14, 1, 2),
        (15, 3, 3),
        (16, 0, 4),
        (18, 4, 5),
    ] {
        let span = e::at(s(0), at);
        let observed = fault(
            &witness,
            span,
            FaultKind::EnumOwnerEpoch {
                owner: OwnerPlaceId(owner),
            },
        );
        assert_eq!(
            observed.result,
            Err(OwnedRunFailure::Invariant(
                "identity epoch overflow",
                Some(span)
            ))
        );
        assert_eq!(transfer_count(&observed), completed);
    }
    // Destination epochs also have to be checked before source read/copy.
    for (at, owner, completed) in [(9, 1, 0), (12, 1, 1), (14, 3, 2), (18, 5, 5)] {
        let span = e::at(s(0), at);
        let observed = fault(
            &witness,
            span,
            FaultKind::EnumOwnerEpoch {
                owner: OwnerPlaceId(owner),
            },
        );
        assert_eq!(
            observed.result,
            Err(OwnedRunFailure::Invariant(
                "identity epoch overflow",
                Some(span)
            ))
        );
        assert_eq!(transfer_count(&observed), completed);
    }
}

#[test]
fn enum_reference_constructor_preflights_type_epoch_before_payload_store() {
    let (sources, s) = f::context();
    let (raw, _) = e::mixed_case(&[Some(hir::Ty::Bool)], &[0], 0, hir::Ty::Bool, s(0));
    let witness = verify_owned(raw, &sources).unwrap();
    for (kind, error) in [
        (
            FaultKind::ConstructorLastScalarType,
            "enum construction type",
        ),
        (
            FaultKind::EnumOwnerEpoch {
                owner: OwnerPlaceId(0),
            },
            "identity epoch overflow",
        ),
    ] {
        let span = e::at(s(0), 4);
        let observed = fault(&witness, span, kind);
        assert_eq!(
            observed.result,
            Err(OwnedRunFailure::Invariant(error, Some(span)))
        );
        let target = failure_owner(&observed, 0, 0);
        assert_eq!(target.state, 1);
        assert_eq!(target.bytes, [0; 8]);
        assert!(!observed
            .storage
            .iter()
            .any(|snapshot| snapshot.kind == StorageKind::Construction));
    }
}

#[test]
fn enum_reference_poisoned_destinations_copy_only_tag_and_active_bytes() {
    let (sources, s) = f::context();
    for (constructed, payload) in e::MIXED.iter().enumerate() {
        let (raw, schedule) = e::relay_case(constructed, payload.unwrap_or(hir::Ty::Unit), s(0));
        let witness = verify_owned(raw, &sources).unwrap();
        let observed = observe(
            &witness,
            schedule.fuel(),
            ObservationControl {
                poison_destinations: true,
                ..ObservationControl::default()
            },
        );
        assert_oracle(&observed, &schedule, schedule.fuel());
        assert_eq!(transfer_count(&observed), 6);
        let mut copies = 0;
        for snapshot in &observed.storage {
            if !snapshot.poisoned {
                continue;
            }
            copies += 1;
            assert_eq!(&snapshot.bytes[..4], &(constructed as u32).to_le_bytes());
            match payload {
                Some(hir::Ty::U8) => {
                    unreachable!("u8 is outside this predecessor fixture or observation domain")
                }
                None => assert_eq!(&snapshot.bytes[4..], &[0xa5; 4]),
                Some(hir::Ty::Bool) => {
                    assert_eq!(snapshot.bytes[4], 1);
                    assert_eq!(&snapshot.bytes[5..], &[0xa5; 3]);
                }
                Some(hir::Ty::Unit) => {
                    assert_eq!(snapshot.bytes[4], 0);
                    assert_eq!(&snapshot.bytes[5..], &[0xa5; 3]);
                }
                Some(hir::Ty::I32) => assert_eq!(&snapshot.bytes[4..], &(-71i32).to_le_bytes()),
            }
        }
        assert_eq!(copies, 8); // two constructions and six whole transfers
        if payload.is_none() {
            assert_eq!(payload_reads(&observed), 0);
        }
    }
}

#[test]
fn enum_reference_inactive_source_bytes_do_not_authorize_payload_reads() {
    let (sources, s) = f::context();
    for (constructed, offset) in [(0, 4), (0, 7), (1, 5), (1, 7), (3, 5), (3, 7)] {
        let (raw, schedule) = e::relay_case(constructed, hir::Ty::Unit, s(0));
        let witness = verify_owned(raw, &sources).unwrap();
        for (at, source) in [(9, 0), (12, 2), (14, 1), (15, 3), (16, 0), (18, 4), (30, 5)] {
            let observed = fault(
                &witness,
                e::at(s(0), at),
                FaultKind::EnumOwnerByte {
                    owner: OwnerPlaceId(source),
                    offset,
                    byte: 255,
                },
            );
            assert_eq!(observed.result, Ok(schedule.result));
            if constructed == 0 {
                assert_eq!(payload_reads(&observed), 0);
            }
        }
    }
}

#[test]
fn enum_reference_discard_validates_but_lifetime_end_never_decodes_poison() {
    let (sources, s) = f::context();
    for payload in e::MIXED {
        let (raw, schedule) = e::discard_case(payload, s(0));
        let witness = verify_owned(raw, &sources).unwrap();
        let span = e::at(s(0), 5);
        let observed = fault(
            &witness,
            span,
            FaultKind::EnumOwnerTag {
                owner: OwnerPlaceId(0),
                tag: u32::MAX,
            },
        );
        assert_eq!(
            observed.result,
            Err(OwnedRunFailure::Invariant("enum tag", Some(span)))
        );
        assert_eq!(failure_owner(&observed, 0, 0).state, 2);
        assert_eq!(payload_reads(&observed), 0);
        if matches!(payload, Some(hir::Ty::Bool | hir::Ty::Unit)) {
            let observed = fault(
                &witness,
                span,
                FaultKind::EnumOwnerByte {
                    owner: OwnerPlaceId(0),
                    offset: 4,
                    byte: 255,
                },
            );
            assert_eq!(
                observed.result,
                Err(OwnedRunFailure::Invariant("enum payload", Some(span)))
            );
            assert_eq!(failure_owner(&observed, 0, 0).state, 2);
        }
        // Moved slot, uninitialized StorageEnd, dead slot at return, and
        // still-uninitialized slot at frame teardown are all payload-free.
        for (at, owner) in [(6, 0), (9, 1), (7, 0), (7, 2)] {
            let observed = fault(
                &witness,
                e::at(s(0), at),
                FaultKind::EnumOwnerPoison {
                    owner: OwnerPlaceId(owner),
                },
            );
            assert_eq!(observed.result, Ok(schedule.result));
            assert_eq!(payload_reads(&observed), usize::from(payload.is_some()));
        }
    }
}

#[test]
fn enum_reference_exact_plan_and_live_frame_limits() {
    let (sources, s) = f::context();
    let (raw, schedule) = e::relay_case(2, hir::Ty::I32, s(0));
    let witness = verify_owned(raw, &sources).unwrap();
    let execution = plan::ExecutionPlan::build(&witness).unwrap();
    let metadata = 2 * size_of::<plan::FunctionPlan>()
        + 7 * size_of::<usize>()
        + size_of::<plan::CallPlan>()
        + size_of::<OwnerPlaceId>();
    assert_eq!(execution.metadata_bytes(), metadata);
    assert!(plan::ExecutionPlan::build_with_test_limit(&witness, metadata).is_ok());
    assert!(plan::ExecutionPlan::build_with_test_limit(&witness, usize::MAX).is_ok());
    let error = plan::fail_allocation_after(0, || {
        plan::ExecutionPlan::build_with_test_limit(&witness, metadata - 1)
    })
    .unwrap_err();
    assert_eq!(error, plan::AdmissionFailure::new("owned plan bytes", None));
    // Frozen carrier sizes are measured separately; 2*272 frame headers,
    // root304 + child40 requested bytes, and the8-byte scalar result.
    let limits = Limits {
        fuel: schedule.fuel(),
        frames: 2,
        slots: 5,
        cells: 50,
        bytes: 896,
    };
    assert_eq!(
        execute::run_limits(&witness, Some(schedule.entry), limits),
        Ok(schedule.result)
    );
    for (below, expected) in [
        (
            Limits {
                frames: 1,
                ..limits
            },
            OwnedRunFailure::Scalar(RunFailure::Frames(e::at(s(0), 15))),
        ),
        (
            Limits { slots: 4, ..limits },
            OwnedRunFailure::Scalar(RunFailure::Slots(s(0))),
        ),
        (
            Limits {
                cells: 49,
                ..limits
            },
            OwnedRunFailure::Resource(plan::AdmissionFailure::new(
                "live expanded cells",
                Some(e::at(s(0), 15)),
            )),
        ),
        (
            Limits {
                bytes: 895,
                ..limits
            },
            OwnedRunFailure::Resource(plan::AdmissionFailure::new(
                "live requested bytes",
                Some(e::at(s(0), 15)),
            )),
        ),
    ] {
        assert_eq!(
            execute::run_limits(&witness, Some(schedule.entry), below),
            Err(expected)
        );
    }
    assert_eq!(execution.function(hir::DefId(0)).usage().payload_bytes, 48);
    assert_eq!(execution.function(hir::DefId(1)).usage().payload_bytes, 8);
    assert_eq!(
        execution.function(hir::DefId(0)).usage().reference_bytes,
        304
    );
    assert_eq!(
        execution.function(hir::DefId(1)).usage().reference_bytes,
        40
    );
}

#[test]
fn enum_reference_every_real_plan_and_frame_allocation_failure() {
    let (sources, s) = f::context();
    let (raw, schedule) = e::relay_case(1, hir::Ty::Bool, s(0));
    let witness = verify_owned(raw, &sources).unwrap();
    let mut failures = 0;
    loop {
        let observed = plan::fail_allocation_after(failures, || {
            observe(&witness, schedule.fuel(), ObservationControl::default())
        });
        match observed.result {
            Ok(value) => {
                assert_eq!(value, schedule.result);
                break;
            }
            Err(OwnedRunFailure::Resource(error)) => {
                assert_eq!(error.name, "injected owned allocation failure");
                assert!(!observed
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::Enter(hir::DefId(1), _))));
                assert!(transfer_count(&observed) <= 3);
                if failures < 9 {
                    assert!(observed.events.is_empty());
                    assert_eq!(error.span, None);
                } else if failures < 17 {
                    assert_eq!(error.span, Some(s(0)));
                } else {
                    assert_eq!(error.span, Some(e::at(s(0), 15)));
                }
            }
            other => panic!("allocation #{failures}: {other:?}"),
        }
        failures += 1;
        assert!(failures <= 24);
    }
    assert_eq!(failures, 24); //9 plan reservations +1 headers +7 root +7 child
}

#[test]
fn enum_reference_canonical_false_and_every_i32_bit_pattern_extremes() {
    let (sources, s) = f::context();
    for value in [
        Scalar::Bool(false),
        Scalar::I32(i32::MIN),
        Scalar::I32(i32::MAX),
        Scalar::I32(0),
    ] {
        let (mut raw, mut schedule) = e::mixed_case(&[Some(value.ty())], &[0], 0, value.ty(), s(0));
        raw.functions[0].blocks[0].statements[1] = f::assign(1, e::literal(value), e::at(s(0), 2));
        schedule.result = value;
        let witness = verify_owned(raw, &sources).unwrap();
        for fuel in 0..=schedule.fuel() {
            assert_oracle(
                &observe(&witness, fuel, ObservationControl::default()),
                &schedule,
                fuel,
            );
        }
    }
}

#[test]
fn enum_reference_maximum_written_arm_chain_uses_all_256_checked_tests() {
    let (sources, s) = f::context();
    let payloads = [None; 256];
    for order in [(0..256).collect::<Vec<_>>(), (0..256).rev().collect()] {
        let (raw, schedule) = e::mixed_case(&payloads, &order, 255, hir::Ty::Unit, s(0));
        let witness = verify_owned(raw, &sources).unwrap();
        let observed = observe(&witness, schedule.fuel(), ObservationControl::default());
        assert_oracle(&observed, &schedule, schedule.fuel());
        let expected_tests = order.iter().position(|&variant| variant == 255).unwrap() + 1;
        assert_eq!(
            observed
                .events
                .iter()
                .filter(|event| matches!(event, Event::EnumTagRead(..)))
                .count(),
            expected_tests + 1
        );
        assert_eq!(payload_reads(&observed), 0);
    }
}

#[test]
fn enum_reference_binding_storage_preflight_precedes_owner_transition() {
    let (sources, s) = f::context();
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        let (raw, _) = e::mixed_case(&[Some(ty)], &[0], 0, ty, s(0));
        let witness = verify_owned(raw, &sources).unwrap();
        let span = e::at(s(0), 5);
        let observed = fault(&witness, span, FaultKind::EnumConsumeMissingSlot { arm: 0 });
        assert_eq!(
            observed.result,
            Err(OwnedRunFailure::Invariant("scalar storage", Some(span)))
        );
        assert_eq!(bindings(&observed), 0);
        assert_eq!(payload_reads(&observed), 1);
        let source = failure_owner(&observed, 0, 0);
        assert_eq!((source.state, source.key.generation), (2, 2));
        assert_eq!(&source.bytes[..4], &[0; 4]);
    }
}

#[test]
fn enum_reference_replacement_restores_moved_destination_every_fuel() {
    let (sources, s) = f::context();
    for (constructed, payload) in e::MIXED.iter().enumerate() {
        let (mut raw, mut schedule) =
            e::relay_case(constructed, payload.unwrap_or(hir::Ty::Unit), s(0));
        let statements = &mut raw.functions[0].blocks[0].statements;
        let before = statements
            .iter()
            .position(|statement| matches!(statement.kind, OwnedInstruction::Replace { .. }))
            .unwrap();
        statements.insert(
            before,
            f::instruction(OwnedInstruction::Discard(OwnerPlaceId(1)), e::at(s(0), 19)),
        );
        let before = schedule
            .events
            .iter()
            .position(|event| *event == (e::at(s(0), 12), 5))
            .unwrap();
        schedule.events.insert(before, (e::at(s(0), 19), 3));
        let witness = verify_owned(raw, &sources).unwrap();
        for fuel in 0..=schedule.fuel() {
            assert_oracle(
                &observe(&witness, fuel, ObservationControl::default()),
                &schedule,
                fuel,
            );
        }
        let observed = observe(&witness, schedule.fuel(), ObservationControl::default());
        let replacements: Vec<_> = observed
            .storage
            .iter()
            .filter(|row| row.kind == StorageKind::Transfer && row.key.owner == 1)
            .collect();
        assert_eq!(replacements.len(), 2);
        assert_eq!(
            (replacements[1].state, replacements[1].key.generation),
            (3, 3)
        );
        assert_eq!(transfer_count(&observed), 6);
    }
}

#[test]
fn enum_reference_available_lifetime_end_and_frame_teardown_ignore_poison() {
    let (sources, s) = f::context();
    for payload in e::MIXED {
        for frame_teardown in [false, true] {
            let (mut raw, mut schedule) = e::discard_case(payload, s(0));
            raw.functions[0].blocks[0].statements.retain(|statement| {
                !matches!(statement.kind, OwnedInstruction::Discard(OwnerPlaceId(0)))
                    && !(frame_teardown
                        && matches!(
                            statement.kind,
                            OwnedInstruction::StorageEnd(OwnerPlaceId(0))
                        ))
            });
            schedule.events.retain(|(span, _)| {
                *span != e::at(s(0), 5) && !(frame_teardown && *span == e::at(s(0), 6))
            });
            let witness = verify_owned(raw, &sources).unwrap();
            let span = e::at(s(0), if frame_teardown { 7 } else { 6 });
            let observed = fault(
                &witness,
                span,
                FaultKind::EnumOwnerPoison {
                    owner: OwnerPlaceId(0),
                },
            );
            assert_eq!(observed.result, Ok(schedule.result));
            assert_eq!(payload_reads(&observed), 0);
            assert!(!observed
                .events
                .iter()
                .any(|event| matches!(event, Event::EnumTagRead(..))));
        }
    }
}

#[test]
fn enum_reference_changed_variant_replacement_every_fuel_and_active_bytes() {
    let (sources, s) = f::context();
    for (initial, replacement) in e::REPLACEMENT_PAIRS {
        for moved in [false, true] {
            let (raw, schedule) = e::replacement_case(initial, replacement, moved, s(0));
            let witness = verify_owned(raw, &sources).unwrap();
            for fuel in 0..=schedule.fuel() {
                let observed = observe(&witness, fuel, ObservationControl::default());
                assert_oracle(&observed, &schedule, fuel);
                let paid_replace = observed.events.iter().any(
                    |event| matches!(event, Event::Charge(span, 5) if *span == e::at(s(0), 12)),
                );
                assert_eq!(transfer_count(&observed), usize::from(paid_replace));
            }
            let replace = schedule
                .events
                .iter()
                .position(|event| *event == (e::at(s(0), 12), 5))
                .unwrap();
            let prefix: usize = schedule.events[..replace].iter().map(|event| event.1).sum();
            let old_identity = if moved { (3, 3) } else { (2, 2) };
            for paid in [false, true] {
                let observed = observe(
                    &witness,
                    prefix + if paid { 5 } else { 4 },
                    ObservationControl::default(),
                );
                let destination = failure_owner(&observed, 0, 0);
                let source = failure_owner(&observed, 0, 1);
                assert_eq!(
                    (destination.state, destination.key.generation),
                    if paid {
                        (2, old_identity.1 + 1)
                    } else {
                        old_identity
                    },
                );
                assert_eq!(
                    (source.state, source.key.generation),
                    if paid { (3, 3) } else { (2, 2) }
                );
                assert_eq!(
                    &destination.bytes[..4],
                    &(if paid { replacement } else { initial } as u32).to_le_bytes()
                );
                assert_eq!(bindings(&observed), 0);
            }
            for poison_destinations in [false, true] {
                let observed = observe(
                    &witness,
                    schedule.fuel(),
                    ObservationControl {
                        poison_destinations,
                        ..ObservationControl::default()
                    },
                );
                assert_oracle(&observed, &schedule, schedule.fuel());
                let old = observed
                    .storage
                    .iter()
                    .find(|row| row.kind == StorageKind::Construction && row.key.owner == 0)
                    .unwrap();
                let source = observed
                    .storage
                    .iter()
                    .find(|row| row.kind == StorageKind::Construction && row.key.owner == 1)
                    .unwrap();
                let replaced = observed
                    .storage
                    .iter()
                    .find(|row| row.kind == StorageKind::Transfer)
                    .unwrap();
                assert_eq!((replaced.state, replaced.key.generation), old_identity);
                assert_eq!(&replaced.bytes[..4], &(replacement as u32).to_le_bytes());
                let active_end = match e::MIXED[replacement] {
                    Some(hir::Ty::U8) => {
                        unreachable!("u8 is outside this predecessor fixture or observation domain")
                    }
                    None => 4,
                    Some(hir::Ty::Bool | hir::Ty::Unit) => 5,
                    Some(hir::Ty::I32) => 8,
                };
                assert_eq!(&replaced.bytes[4..active_end], &source.bytes[4..active_end]);
                if poison_destinations {
                    // Inactive bytes retain the observer's destination poison,
                    // even when they held an active payload in the old value.
                    assert!(replaced.bytes[active_end..]
                        .iter()
                        .all(|byte| *byte == 0xa5));
                } else {
                    assert_eq!(&replaced.bytes[active_end..], &old.bytes[active_end..]);
                }
            }
        }
    }
}

#[test]
fn enum_reference_changed_variant_replacement_fault_preserves_old_destination() {
    let (sources, s) = f::context();
    for (initial, replacement) in e::REPLACEMENT_PAIRS {
        for moved in [false, true] {
            let (raw, schedule) = e::replacement_case(initial, replacement, moved, s(0));
            let witness = verify_owned(raw, &sources).unwrap();
            let mut faults = vec![(
                FaultKind::EnumOwnerTag {
                    owner: OwnerPlaceId(1),
                    tag: u32::MAX,
                },
                "enum tag",
            )];
            if matches!(e::MIXED[replacement], Some(hir::Ty::Bool | hir::Ty::Unit)) {
                faults.push((
                    FaultKind::EnumOwnerByte {
                        owner: OwnerPlaceId(1),
                        offset: 4,
                        byte: 255,
                    },
                    "enum payload",
                ));
            }
            for (kind, error) in faults {
                let observed = fault(&witness, e::at(s(0), 12), kind);
                assert_eq!(
                    observed.result,
                    Err(OwnedRunFailure::Invariant(error, Some(e::at(s(0), 12))))
                );
                assert_eq!((transfer_count(&observed), bindings(&observed)), (0, 0));
                let old = observed
                    .storage
                    .iter()
                    .find(|row| row.kind == StorageKind::Construction && row.key.owner == 0)
                    .unwrap();
                let destination = failure_owner(&observed, 0, 0);
                let source = failure_owner(&observed, 0, 1);
                assert_eq!(
                    (destination.state, destination.key.generation),
                    if moved { (3, 3) } else { (2, 2) }
                );
                assert_eq!(destination.bytes, old.bytes);
                assert_eq!((source.state, source.key.generation), (2, 2));
            }
            if moved {
                // A moved destination's previous tag and payload are dead.
                let observed = fault(
                    &witness,
                    e::at(s(0), 12),
                    FaultKind::EnumOwnerPoison {
                        owner: OwnerPlaceId(0),
                    },
                );
                assert_eq!(observed.result, Ok(schedule.result));
                assert_eq!(transfer_count(&observed), 1);
            }
        }
    }
}

#[test]
fn enum_reference_later_owned_input_fault_preserves_caller_and_blocks_body() {
    let (sources, s) = f::context();
    for second in [1, 3] {
        let (raw, schedule) = e::two_owned_case(second, s(0));
        let witness = verify_owned(raw, &sources).unwrap();
        for fuel in 0..=schedule.fuel() {
            assert_oracle(
                &observe(&witness, fuel, ObservationControl::default()),
                &schedule,
                fuel,
            );
        }
        let invoke = schedule
            .events
            .iter()
            .position(|event| *event == (e::at(s(0), 10), 20))
            .unwrap();
        let prefix: usize = schedule.events[..invoke].iter().map(|event| event.1).sum();
        for (kind, error) in [
            (
                FaultKind::EnumOwnerTag {
                    owner: OwnerPlaceId(3),
                    tag: u32::MAX,
                },
                "enum tag",
            ),
            (
                FaultKind::EnumOwnerByte {
                    owner: OwnerPlaceId(3),
                    offset: 4,
                    byte: 255,
                },
                "enum payload",
            ),
        ] {
            let control = ObservationControl {
                fault: Some(FaultInjection {
                    at: e::at(s(0), 10),
                    kind,
                }),
                ..ObservationControl::default()
            };
            let unpaid = observe(&witness, prefix + 19, control);
            assert_oracle(&unpaid, &schedule, prefix + 19);
            assert_eq!(
                unpaid.result,
                Err(OwnedRunFailure::Scalar(RunFailure::Fuel(e::at(s(0), 10))))
            );
            assert_eq!(transfer_count(&unpaid), 2);
            assert!(!unpaid
                .storage
                .iter()
                .any(|row| row.kind == StorageKind::Incoming));
            for observed in [
                observe(&witness, prefix + 20, control),
                fault(&witness, e::at(s(0), 10), kind),
            ] {
                assert!(observed.fault_applied && !observed.truncated);
                assert_eq!(
                    observed.result,
                    Err(OwnedRunFailure::Invariant(error, Some(e::at(s(0), 30))))
                );
                assert!(!observed.events.iter().any(|event| matches!(
                    event,
                    Event::Enter(hir::DefId(1), _) | Event::Return(hir::DefId(1))
                )));
                assert!(!observed.events.iter().any(
                    |event| matches!(event, Event::Charge(span, _) if *span == e::at(s(0), 40))
                ));
                for stage in [2, 3] {
                    for snapshot in [
                        failure_owner(&unpaid, 0, stage),
                        failure_owner(&observed, 0, stage),
                    ] {
                        assert_eq!((snapshot.state, snapshot.key.generation), (2, 2));
                    }
                }
                assert_eq!(
                    failure_owner(&observed, 0, 2).bytes,
                    failure_owner(&unpaid, 0, 2).bytes
                );
                let mut expected_second = failure_owner(&unpaid, 0, 3).bytes.clone();
                match kind {
                    FaultKind::EnumOwnerTag { tag, .. } => {
                        expected_second[..4].copy_from_slice(&tag.to_le_bytes())
                    }
                    FaultKind::EnumOwnerByte { offset, byte, .. } => expected_second[offset] = byte,
                    _ => unreachable!(),
                }
                assert_eq!(failure_owner(&observed, 0, 3).bytes, expected_second);
                // The first input may already have been copied into a private,
                // uninstalled child. These test observations are not caller
                // ownership transitions or language-visible callee effects.
                assert_eq!(transfer_count(&observed), 3);
                let incoming: Vec<_> = observed
                    .storage
                    .iter()
                    .filter(|row| row.kind == StorageKind::Incoming)
                    .collect();
                assert_eq!(incoming.len(), 1);
                assert_eq!(
                    (
                        incoming[0].key.frame,
                        incoming[0].key.owner,
                        incoming[0].key.generation
                    ),
                    (1, 0, 1)
                );
                assert_eq!(incoming[0].bytes, failure_owner(&unpaid, 0, 2).bytes);
                assert!(observed
                    .storage
                    .iter()
                    .filter(|row| row.kind == StorageKind::Failure)
                    .all(|row| row.key.frame == 0));
                assert!(observed
                    .storage
                    .iter()
                    .all(|row| row.guards_before == row.guards_after));
            }
        }
    }
}
