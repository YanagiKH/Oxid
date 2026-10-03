//! Consumer boundary controls. Sequences/schedules come from the parent oracle;
//! invariant corruptions use only the reviewed closed runtime fault selectors.
use super::super::execute::{FaultInjection, FaultKind};
use super::*;

fn configured(
    raw: RawOwnedProgram,
    sources: &SourceMap,
    limits: Limits,
    control: ObservationControl,
) -> ReferenceObservation {
    verified::probe_array_reference(
        raw,
        sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        limits,
        control,
    )
    .unwrap()
}

#[test]
fn unit2c_poisoned_full_sentinels_and_all_transfer_destinations() {
    let (sources, s) = raw::context();
    let mut cases = 0;
    for (ty, n) in [
        (hir::Ty::I32, 0),
        (hir::Ty::Bool, 0),
        (hir::Ty::Unit, 0),
        (hir::Ty::Unit, 2),
        (hir::Ty::I32, 2),
    ] {
        let observation = configured(
            relay_raw(ty, n, &s),
            &sources,
            Limits::default(),
            ObservationControl {
                poison_destinations: true,
                ..Default::default()
            },
        );
        assert_eq!(observation.result, relay_schedule(ty, n, &s).result);
        assert!(!observation.truncated);
        let mut kinds = [0; 3];
        for snapshot in &observation.storage {
            assert_eq!(snapshot.guards_before, snapshot.guards_after);
            if snapshot.poisoned {
                let index = match snapshot.kind {
                    StorageKind::Construction => 0,
                    StorageKind::Transfer => 1,
                    StorageKind::Incoming => 2,
                    _ => panic!("unexpected poisoned path"),
                };
                kinds[index] += 1;
                if n == 0 || ty == hir::Ty::Unit {
                    assert_eq!(
                        snapshot.bytes,
                        vec![0; n.max(1) * if ty == hir::Ty::I32 { 4 } else { 1 }]
                    );
                }
            }
        }
        // Two constructors; move, two replacements, self-replacement move,
        // staging and returned result; separate incoming parameter copy.
        assert_eq!(kinds, [2, 6, 1]);
        cases += 1;
    }
    assert_eq!(cases, 5);
}

fn add_other_root(
    program: &mut RawOwnedProgram,
    ty: hir::Ty,
    n: usize,
    s: &impl Fn(usize) -> Span,
) {
    let f = &mut program.functions[0];
    let local = f.locals.len();
    f.locals.push(raw::scalar(ty, s(50)));
    f.owners
        .push(owner(ty, n, OwnerKind::Local { mutable: true }, s(50)));
    f.blocks[0].statements.splice(
        0..0,
        [
            raw::assign(
                local,
                literal(match ty {
                    hir::Ty::Bool => Scalar::Bool(true),
                    hir::Ty::I32 => Scalar::I32(77),
                    hir::Ty::Unit => Scalar::Unit,
                }),
                s(51),
            ),
            raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), s(52)),
            raw::instruction(
                OwnedInstruction::ConstructArray {
                    destination: OwnerPlaceId(1),
                    elements: vec![raw::operand(local, s(53)); n],
                },
                s(53),
            ),
        ],
    );
}

#[test]
fn unit2c_closed_provenance_faults_precede_bounds_and_zero_length_permission() {
    let (sources, s) = raw::context();
    let faults = [
        (FaultKind::StaleOwnerActivation, "stale owner activation"),
        (FaultKind::StaleOwnerGeneration, "stale owner generation"),
        (FaultKind::StaleLoanActivation, "stale loan activation"),
        (FaultKind::StaleLoanInstance, "stale loan instance"),
        (
            FaultKind::SuspendedReferencePermission,
            "suspended parent permission",
        ),
        (FaultKind::LoanRootMismatch, "loan root mismatch"),
        (FaultKind::ArrayReferenceDifferentType, "array base type"),
    ];
    let mut cases = 0;
    for (kind, expected) in faults {
        for site in [11, 12, 19] {
            let mut program = helper_raw(HelperCase::Success, &s);
            add_other_root(&mut program, hir::Ty::Bool, 4, &s);
            if site != 11 {
                let function = if site == 12 { 1 } else { 2 };
                let statement = &mut program.functions[function].blocks[0].statements[0];
                let OwnedInstruction::Scalar(Statement::Assign(assign)) = &mut statement.kind
                else {
                    unreachable!()
                };
                assign.value = Rvalue::I32(-1);
            }
            let observed = configured(
                program,
                &sources,
                Limits::default(),
                ObservationControl {
                    fault: Some(FaultInjection { at: s(site), kind }),
                    ..Default::default()
                },
            );
            assert!(observed.fault_applied, "{kind:?}/{site}");
            assert!(!observed.truncated);
            assert_eq!(
                observed.result,
                Err(OwnedRunFailure::Invariant(expected, Some(s(site)))),
                "{kind:?}/{site}"
            );
            let terminal = observed
                .storage
                .iter()
                .find(|snapshot| {
                    snapshot.kind == StorageKind::Failure
                        && snapshot.key.frame == 0
                        && snapshot.key.owner == 0
                })
                .unwrap();
            assert_eq!(
                terminal.bytes,
                payload(hir::Ty::I32, &[Scalar::I32(10), Scalar::I32(20)])
            );
            cases += 1;
        }
    }
    let mut program = helper_raw(HelperCase::Success, &s);
    let zero = AggregateSlot::try_from_aggregate(array(hir::Ty::I32, 0)).unwrap();
    for function in &mut program.functions {
        for owner in &mut function.owners {
            owner.aggregate = zero;
        }
        for reference in &mut function.references {
            reference.aggregate = zero;
        }
        for loan in &mut function.loans {
            loan.aggregate = zero;
        }
        for block in &mut function.blocks {
            for statement in &mut block.statements {
                if let OwnedInstruction::ConstructArray { elements, .. } = &mut statement.kind {
                    elements.clear();
                }
            }
        }
    }
    let observed = configured(
        program,
        &sources,
        Limits::default(),
        ObservationControl {
            fault: Some(FaultInjection {
                at: s(11),
                kind: FaultKind::SuspendedReferencePermission,
            }),
            ..Default::default()
        },
    );
    assert!(observed.fault_applied && !observed.truncated);
    assert_eq!(
        observed.result,
        Err(OwnedRunFailure::Invariant(
            "suspended parent permission",
            Some(s(11))
        ))
    );
    assert_eq!(cases + 1, 22);
}

#[test]
fn unit2c_last_constructor_snapshot_is_validated_before_any_write() {
    let (sources, s) = raw::context();
    for ty in [hir::Ty::I32, hir::Ty::Bool, hir::Ty::Unit] {
        let observed = configured(
            core_raw(ty, 2, 0, AccessCase::Length, Seed::Distinct, &s),
            &sources,
            Limits::default(),
            ObservationControl {
                fault: Some(FaultInjection {
                    at: s(1103),
                    kind: FaultKind::ConstructorLastScalarType,
                }),
                ..Default::default()
            },
        );
        assert!(observed.fault_applied && !observed.truncated);
        assert_eq!(
            observed.result,
            Err(OwnedRunFailure::Invariant(
                "array construction type",
                Some(s(1103))
            ))
        );
        assert_eq!(observed.storage.len(), 1);
        assert_eq!(observed.storage[0].kind, StorageKind::Failure);
        assert_eq!(observed.storage[0].state, 1);
        assert!(observed.storage[0].bytes.iter().all(|byte| *byte == 0));
    }
}

#[test]
fn unit2c_incoming_alias_and_structural_type_rechecks() {
    let (sources, s) = raw::context();
    let mut program = helper_raw(HelperCase::Success, &s);
    add_other_root(&mut program, hir::Ty::I32, 2, &s);
    for loan in &mut program.functions[0].loans[..2] {
        loan.kind = BorrowKind::Exclusive;
    }
    program.functions[0].loans[1].authority = AccessBase::Owner(OwnerPlaceId(1));
    for reference in &mut program.functions[1].references {
        reference.kind = BorrowKind::Exclusive;
    }
    let observed = configured(
        program,
        &sources,
        Limits::default(),
        ObservationControl {
            fault: Some(FaultInjection {
                at: s(9),
                kind: FaultKind::IncomingExclusiveAlias,
            }),
            ..Default::default()
        },
    );
    assert!(observed.fault_applied && !observed.truncated);
    assert_eq!(
        observed.result,
        Err(OwnedRunFailure::Invariant(
            "incoming reference alias contract",
            Some(s(9))
        ))
    );
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|event| matches!(event, Event::Enter(..)))
            .count(),
        1
    );
    let mut program = helper_raw(HelperCase::Success, &s);
    add_other_root(&mut program, hir::Ty::I32, 0, &s);
    let observed = configured(
        program,
        &sources,
        Limits::default(),
        ObservationControl {
            fault: Some(FaultInjection {
                at: s(9),
                kind: FaultKind::IncomingReferenceDifferentType,
            }),
            ..Default::default()
        },
    );
    assert!(observed.fault_applied && !observed.truncated);
    assert_eq!(
        observed.result,
        Err(OwnedRunFailure::Invariant(
            "incoming reference type",
            Some(s(9))
        ))
    );
    assert_eq!(
        observed
            .events
            .iter()
            .filter(|event| matches!(event, Event::Enter(..)))
            .count(),
        1
    );
}
