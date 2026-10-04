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
            reference.referent = BorrowedSlot::check(BorrowedTy::Exact(zero.aggregate())).unwrap();
        }
        for loan in &mut function.loans {
            loan.referent = BorrowedSlot::check(BorrowedTy::Exact(zero.aggregate())).unwrap();
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

#[test]
fn unit2c_exact_reference_resources_and_all_allocation_hooks() {
    let (sources, s) = raw::context();
    let mut boundary_cases = 0;
    let mut allocation_cases = 0;
    for (ty, n) in [
        (hir::Ty::I32, 0),
        (hir::Ty::Bool, 0),
        (hir::Ty::Unit, 4),
        (hir::Ty::I32, 2),
        (hir::Ty::I32, 1024),
    ] {
        // Length fixture: S=N+1, A=R=L=C=0, O=1, P=max(1,N).
        let slots = n + 1;
        let cells = slots + n.max(1) + 4;
        let payload_bytes = n.max(1) * if ty == hir::Ty::I32 { 4 } else { 1 };
        let bytes = 272 + 8 + 8 * slots + payload_bytes + 32;
        let exact = Limits {
            frames: 1,
            slots,
            cells,
            bytes,
            ..Limits::default()
        };
        let observation = configured(
            core_raw(ty, n, 0, AccessCase::Length, Seed::Distinct, &s),
            &sources,
            exact,
            ObservationControl::default(),
        );
        assert_eq!(observation.result, Ok(Scalar::I32(n as i32)));
        assert!(!observation.truncated);
        for (limits, error) in [
            (
                Limits {
                    fuel: cells,
                    ..exact
                },
                OwnedRunFailure::Scalar(RunFailure::Fuel(s(0))),
            ),
            (
                Limits { frames: 0, ..exact },
                OwnedRunFailure::Scalar(RunFailure::Frames(s(0))),
            ),
            (
                Limits {
                    slots: slots - 1,
                    ..exact
                },
                OwnedRunFailure::Scalar(RunFailure::Slots(s(0))),
            ),
            (
                Limits {
                    cells: cells - 1,
                    ..exact
                },
                OwnedRunFailure::Resource(plan::AdmissionFailure::new(
                    "live expanded cells",
                    Some(s(0)),
                )),
            ),
            (
                Limits {
                    bytes: bytes - 1,
                    ..exact
                },
                OwnedRunFailure::Resource(plan::AdmissionFailure::new(
                    "live requested bytes",
                    Some(s(0)),
                )),
            ),
        ] {
            let actual = configured(
                core_raw(ty, n, 0, AccessCase::Length, Seed::Distinct, &s),
                &sources,
                limits,
                ObservationControl::default(),
            );
            assert_eq!(actual.result, Err(error));
            assert!(!actual
                .events
                .iter()
                .any(|event| matches!(event, Event::Enter(..))));
            assert!(actual.storage.is_empty());
            boundary_cases += 1;
        }
        boundary_cases += 1;
        // One plan Vec + four per-function Vecs, one frame stack, seven Frame Vecs.
        // Observer allocations do not consume these production reservation hooks.
        for fail in 0..=13 {
            let actual = plan::fail_allocation_after(fail, || {
                configured(
                    core_raw(ty, n, 0, AccessCase::Length, Seed::Distinct, &s),
                    &sources,
                    exact,
                    ObservationControl::default(),
                )
            });
            if fail < 13 {
                let expected_span = if fail < 5 { None } else { Some(s(0)) };
                assert_eq!(
                    actual.result,
                    Err(OwnedRunFailure::Resource(plan::AdmissionFailure::new(
                        "injected owned allocation failure",
                        expected_span
                    ))),
                    "N={n},fail={fail}"
                );
            } else {
                assert_eq!(actual.result, Ok(Scalar::I32(n as i32)));
            }
            allocation_cases += 1;
        }
    }
    assert_eq!((boundary_cases, allocation_cases), (30, 70));
}

fn array_loop(iterations: i32) -> (SourceMap, RawOwnedProgram) {
    let (sources, mut program, _) = raw::owner_loop();
    program.records.clear();
    let f = &mut program.functions[0];
    f.owners[0].aggregate = AggregateSlot::try_from_aggregate(array(hir::Ty::I32, 1)).unwrap();
    let OwnedInstruction::Scalar(Statement::Assign(limit)) = &mut f.blocks[0].statements[2].kind
    else {
        unreachable!()
    };
    limit.value = Rvalue::I32(iterations);
    for block in &mut f.blocks {
        for statement in &mut block.statements {
            statement.kind = match &statement.kind {
                OwnedInstruction::Construct {
                    destination,
                    fields,
                } => OwnedInstruction::ConstructArray {
                    destination: *destination,
                    elements: fields.iter().map(|(_, operand)| *operand).collect(),
                },
                OwnedInstruction::ReadField {
                    destination, base, ..
                } => OwnedInstruction::ReadIndex {
                    destination: *destination,
                    base: *base,
                    index: raw::operand(0, statement.span),
                },
                other => other.clone(),
            };
        }
    }
    (sources, program)
}

#[test]
fn unit2c_natural_generation_restarts_and_every_loop_fuel() {
    let mut executions = 0;
    for iterations in [0, 1, 2, 3] {
        let (_sources, program) = array_loop(iterations);
        let f = &program.functions[0];
        // S=8,P=1,O=1, X=13: root14; prefix5, final condition3, return2.
        // Each loop contributes 3 condition + 11 body costs.
        let mut charges = vec![(f.span, 14)];
        charges.extend(
            f.blocks[0]
                .statements
                .iter()
                .map(|statement| (statement.span, 1)),
        );
        charges.push((f.blocks[0].terminator.as_ref().unwrap().span, 1));
        for _ in 0..iterations {
            charges.extend(
                f.blocks[1]
                    .statements
                    .iter()
                    .map(|statement| (statement.span, 1)),
            );
            charges.push((f.blocks[1].terminator.as_ref().unwrap().span, 1));
            charges.extend(
                f.blocks[2]
                    .statements
                    .iter()
                    .zip([1, 2, 1, 2, 2, 1, 1])
                    .map(|(statement, cost)| (statement.span, cost)),
            );
            charges.push((f.blocks[2].terminator.as_ref().unwrap().span, 1));
        }
        charges.extend(
            f.blocks[1]
                .statements
                .iter()
                .map(|statement| (statement.span, 1)),
        );
        charges.push((f.blocks[1].terminator.as_ref().unwrap().span, 1));
        charges.push((f.blocks[3].terminator.as_ref().unwrap().span, 2));
        let sufficient = 24 + 14 * iterations as usize;
        assert_eq!(
            charges.iter().map(|(_, cost)| cost).sum::<usize>(),
            sufficient
        );
        for fuel in 0..=sufficient {
            let (sources, program) = array_loop(iterations);
            let observation = configured(
                program,
                &sources,
                Limits {
                    fuel,
                    ..Default::default()
                },
                ObservationControl::default(),
            );
            assert!(!observation.truncated);
            let mut remaining = fuel;
            let mut paid = vec![];
            let mut error = None;
            for &(span, cost) in &charges {
                if remaining < cost {
                    error = Some(OwnedRunFailure::Scalar(RunFailure::Fuel(span)));
                    break;
                }
                remaining -= cost;
                paid.push((span, cost));
            }
            assert_eq!(
                observation.result,
                error.map_or(Ok(Scalar::I32(iterations)), Err)
            );
            assert_eq!(observation.remaining_fuel, remaining);
            assert_eq!(
                observation
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        Event::Charge(span, cost) => Some((*span, *cost)),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                paid
            );
            let reads: Vec<_> = observation
                .events
                .iter()
                .filter_map(|event| match event {
                    Event::ReadIndex(root, 0, value) => Some((*root, *value)),
                    _ => None,
                })
                .collect();
            for (ordinal, (root, value)) in reads.iter().enumerate() {
                assert_eq!(root.generation, 2 + 4 * ordinal as u64);
                assert_eq!(*value, Scalar::I32(ordinal as i32));
            }
            executions += 1;
        }
    }
    assert_eq!(executions, 184);
}

#[test]
fn unit2c_observation_truncation_keeps_language_result_and_stops_poison() {
    let (sources, program) = array_loop(100);
    let actual = configured(
        program,
        &sources,
        Limits::default(),
        ObservationControl {
            poison_destinations: true,
            ..Default::default()
        },
    );
    assert_eq!(actual.result, Ok(Scalar::I32(100)));
    assert!(actual.truncated);
    assert_eq!(actual.storage.len(), 64);
    assert_eq!(
        actual
            .storage
            .iter()
            .filter(|snapshot| snapshot.poisoned)
            .count(),
        64
    );
    let (sources, mut program) = array_loop(5000);
    let f = &mut program.functions[0];
    f.owners.clear();
    f.blocks[2].statements.retain(|statement| {
        !matches!(
            statement.kind,
            OwnedInstruction::StorageLive(_)
                | OwnedInstruction::StorageEnd(_)
                | OwnedInstruction::ConstructArray { .. }
                | OwnedInstruction::Discard(_)
        )
    });
    for statement in &mut f.blocks[2].statements {
        if let OwnedInstruction::ReadIndex { destination, .. } = statement.kind {
            statement.kind = raw::assign(
                destination.0,
                Rvalue::Copy(raw::operand(3, statement.span)),
                statement.span,
            )
            .kind;
        }
    }
    let actual = configured(
        program,
        &sources,
        Limits::default(),
        ObservationControl::default(),
    );
    assert_eq!(actual.result, Ok(Scalar::I32(5000)));
    assert!(actual.truncated);
    assert_eq!(actual.events.len(), 16_384);
    assert!(actual.storage.is_empty());
    assert_eq!(actual.remaining_fuel, 1_000_000 - (18 + 7 * 5000));
}

fn nested_arrays(exclusive: bool) -> (SourceMap, RawOwnedProgram, Vec<(Span, usize)>) {
    let (sources, mut program, previous) = raw::shared_children(exclusive);
    let file = program.functions[0].span.file;
    let s = |i| Span {
        file,
        start: i * 2,
        end: i * 2 + 1,
    };
    let aggregate = AggregateSlot::try_from_aggregate(array(hir::Ty::I32, 1)).unwrap();
    program.records.clear();
    for function in &mut program.functions {
        for owner in &mut function.owners {
            owner.aggregate = aggregate;
        }
        for reference in &mut function.references {
            reference.referent =
                BorrowedSlot::check(BorrowedTy::Exact(aggregate.aggregate())).unwrap();
        }
        for loan in &mut function.loans {
            loan.referent = BorrowedSlot::check(BorrowedTy::Exact(aggregate.aggregate())).unwrap();
        }
        let index = function.locals.len();
        function
            .locals
            .push(raw::scalar(hir::Ty::I32, s(1000 + function.id.0)));
        for block in &mut function.blocks {
            for statement in &mut block.statements {
                statement.kind = match &statement.kind {
                    OwnedInstruction::Construct {
                        destination,
                        fields,
                    } => {
                        assert_eq!(fields.len(), 1);
                        OwnedInstruction::ConstructArray {
                            destination: *destination,
                            elements: vec![fields[0].1],
                        }
                    }
                    OwnedInstruction::ReadField {
                        destination,
                        base,
                        field,
                    } => {
                        assert_eq!(field.index, 0);
                        OwnedInstruction::ReadIndex {
                            destination: *destination,
                            base: *base,
                            index: raw::operand(index, statement.span),
                        }
                    }
                    OwnedInstruction::WriteField { base, field, value } => {
                        assert_eq!(field.index, 0);
                        OwnedInstruction::WriteIndex {
                            base: *base,
                            index: raw::operand(index, statement.span),
                            value: *value,
                        }
                    }
                    other => other.clone(),
                };
            }
        }
        function.blocks[function.entry.0].statements.insert(
            0,
            raw::assign(index, Rvalue::I32(0), s(1000 + function.id.0)),
        );
    }
    let mut charges = vec![];
    for (span, cost) in previous.events {
        if [s(0), s(6), s(25)].contains(&span) {
            charges.push((span, cost + 1));
            let function = if span == s(0) {
                0
            } else if span == s(6) {
                1
            } else {
                2
            };
            charges.push((s(1000 + function), 1));
        } else {
            charges.push((span, cost));
        }
    }
    (sources, program, charges)
}

#[test]
fn unit2c_natural_nested_reborrows_restore_exclusive_parent_every_fuel() {
    let mut executions = 0;
    for exclusive in [false, true] {
        let (_, _, charges) = nested_arrays(exclusive);
        let sufficient = if exclusive { 122 } else { 119 };
        assert_eq!(
            charges.iter().map(|(_, cost)| cost).sum::<usize>(),
            sufficient
        );
        for fuel in 0..=sufficient {
            let (sources, program, _) = nested_arrays(exclusive);
            let actual = configured(
                program,
                &sources,
                Limits {
                    fuel,
                    ..Default::default()
                },
                ObservationControl::default(),
            );
            assert!(!actual.truncated);
            let mut remaining = fuel;
            let mut paid = vec![];
            let mut error = None;
            for &(span, cost) in &charges {
                if remaining < cost {
                    error = Some(OwnedRunFailure::Scalar(RunFailure::Fuel(span)));
                    break;
                }
                remaining -= cost;
                paid.push((span, cost));
            }
            assert_eq!(actual.result, error.map_or(Ok(Scalar::I32(-94)), Err));
            assert_eq!(actual.remaining_fuel, remaining);
            assert_eq!(
                actual
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        Event::Charge(span, cost) => Some((*span, *cost)),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                paid
            );
            if fuel == sufficient {
                assert_eq!(
                    actual
                        .events
                        .iter()
                        .filter(|event| matches!(event, Event::Acquire(..)))
                        .count(),
                    3
                );
                assert_eq!(
                    actual
                        .events
                        .iter()
                        .filter(|event| matches!(event, Event::Release(..)))
                        .count(),
                    3
                );
                let writes: Vec<_> = actual
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        Event::WriteIndex(_, 0, value) => Some(*value),
                        _ => None,
                    })
                    .collect();
                assert_eq!(
                    writes,
                    if exclusive {
                        vec![Scalar::I32(9)]
                    } else {
                        vec![]
                    }
                );
            }
            executions += 1;
        }
    }
    assert_eq!(executions, 243);
}

#[test]
fn unit2c_observer_reservation_failures_preserve_language_outcome() {
    use super::super::execute::ObservationAllocationSite;
    let (sources, s) = raw::context();
    for site in [
        ObservationAllocationSite::Event,
        ObservationAllocationSite::StorageHeader,
        ObservationAllocationSite::Payload,
    ] {
        let actual = configured(
            core_raw(hir::Ty::I32, 2, 0, AccessCase::Write, Seed::Distinct, &s),
            &sources,
            Limits::default(),
            ObservationControl {
                allocation_failure: Some(site),
                ..Default::default()
            },
        );
        assert_eq!(actual.result, Ok(replacement(hir::Ty::I32, Seed::Distinct)));
        assert!(actual.truncated && actual.allocation_fault_applied);
        assert!(!actual.fault_applied);
        assert!(actual.storage.is_empty());
        if site == ObservationAllocationSite::Event {
            assert!(actual.events.is_empty());
        }
    }
}

#[test]
fn unit2c_fault_selector_consumes_first_matching_charged_origin() {
    let (sources, s) = raw::context();
    let mut program = core_raw(hir::Ty::I32, 2, 0, AccessCase::Length, Seed::Distinct, &s);
    // The first charged statement with the constructor's origin is deliberately
    // a scalar assignment, where this closed corruption is inapplicable.
    program.functions[0].blocks[0].statements[0].span = s(1103);
    let OwnedInstruction::Scalar(Statement::Assign(assign)) =
        &mut program.functions[0].blocks[0].statements[0].kind
    else {
        unreachable!()
    };
    assign.span = s(1103);
    let actual = configured(
        program,
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
    assert_eq!(actual.result, Ok(Scalar::I32(2)));
    assert!(!actual.fault_applied && !actual.truncated);
}

#[test]
fn unit2c_bounds_diagnostics_use_primary_and_filter_invalid_presentation_maps() {
    let (sources, s) = raw::context();
    for access in [AccessCase::Read, AccessCase::Write] {
        for fuel in [14, 15] {
            let mut program = core_raw(hir::Ty::I32, 0, -1, access, Seed::Distinct, &s);
            let target = program.functions[0].blocks[0]
                .statements
                .iter_mut()
                .find(|statement| {
                    matches!(
                        statement.kind,
                        OwnedInstruction::ReadIndex { .. } | OwnedInstruction::WriteIndex { .. }
                    )
                })
                .unwrap();
            target.diagnostic_origins = Some(DiagnosticOrigins {
                primary: s(2000),
                cause: s(2001),
            });
            let observed = configured(
                program,
                &sources,
                Limits {
                    fuel,
                    ..Default::default()
                },
                ObservationControl::default(),
            );
            if fuel == 14 {
                assert_eq!(
                    observed.result,
                    Err(OwnedRunFailure::Scalar(RunFailure::Fuel(s(2000))))
                );
            } else {
                assert_eq!(observed.result, Err(OwnedRunFailure::Bounds(s(2000))));
                let diagnostic = observed.result.as_ref().unwrap_err().diagnostic(&sources);
                assert_eq!(
                    (
                        diagnostic.code,
                        diagnostic.stage,
                        diagnostic.message.as_str(),
                        diagnostic.primary
                    ),
                    (
                        "E0606",
                        "oir-owned-run",
                        "array index out of bounds",
                        Some(s(2000))
                    )
                );
                let missing = observed
                    .result
                    .as_ref()
                    .unwrap_err()
                    .diagnostic(&SourceMap::new());
                assert_eq!(missing.primary, None);
                assert_eq!(
                    missing.render_human(&SourceMap::new()),
                    "error[E0606] (oir-owned-run): array index out of bounds\n"
                );
            }
            assert!(!observed.truncated);
        }
    }
}

#[test]
fn unit2c_scalar_entry_gate_precedes_plan_reservations() {
    let (sources, s) = raw::context();
    for entry in [None, Some(hir::DefId(999)), Some(hir::DefId(0))] {
        let mut program = core_raw(hir::Ty::I32, 0, 0, AccessCase::Length, Seed::Distinct, &s);
        let f = &mut program.functions[0];
        f.result = ValueTy::Owned(array(hir::Ty::I32, 0));
        f.locals.clear();
        f.blocks[0].statements.pop(); // length
        f.blocks[0].terminator.as_mut().unwrap().kind =
            OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0));
        let observed = plan::fail_allocation_after(0, || {
            verified::probe_array_reference(
                program,
                &sources,
                budget::Limits::DEFAULT,
                entry,
                Limits::default(),
                ObservationControl::default(),
            )
            .unwrap()
        });
        let expected = match entry {
            None => OwnedRunFailure::Scalar(RunFailure::Entry(None)),
            Some(hir::DefId(0)) => OwnedRunFailure::EntryResult(s(0)),
            _ => OwnedRunFailure::Invariant("entry identity", None),
        };
        assert_eq!(observed.result, Err(expected));
        assert!(observed.events.is_empty() && observed.storage.is_empty() && !observed.truncated);
    }
}
