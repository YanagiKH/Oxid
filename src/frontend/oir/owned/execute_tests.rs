use super::super::consumer_fixtures::*;
use super::*;

#[path = "source/runtime_resource_tests.rs"]
mod source_resources;

#[test]
fn independently_counted_complete_traces_and_every_lower_fuel() {
    type Builder = fn() -> (SourceMap, RawOwnedProgram, Schedule);
    for build in [empty_record as Builder, owned_relay, shared_read] {
        let (sources, raw, schedule) = build();
        let p = verified::verify_owned(raw, &sources).unwrap();
        for fuel in 0..=schedule.fuel() {
            let mut events = vec![];
            let result = run_observed(
                &p,
                schedule.entry,
                Limits {
                    fuel,
                    ..Limits::default()
                },
                &mut events,
            );
            match schedule.failure(fuel) {
                Some(span) => assert_eq!(
                    result,
                    Err(OwnedRunFailure::Scalar(RunFailure::Fuel(span))),
                    "fuel={fuel}"
                ),
                None => assert_eq!(result, Ok(schedule.result)),
            }
            let actual: Vec<_> = events
                .iter()
                .filter_map(|e| {
                    if let Event::Charge(span, cost) = e {
                        Some((*span, *cost))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(actual, schedule.events[..actual.len()]);
        }
    }
}

#[test]
fn batch_pilot_matches_independent_state_and_all_1086_lower_budgets() {
    let (sources, raw, schedule) = super::super::consumer_pilot::batch();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(schedule.fuel(), 1086);
    let p = ExecutionPlan::build(&witness).unwrap();
    let census = [
        (111, 808, 304),
        (12, 96, 40),
        (17, 136, 80),
        (26, 208, 48),
        (11, 88, 32),
        (8, 48, 16),
        (15, 104, 72),
    ];
    for (i, (cells, bytes, native)) in census.into_iter().enumerate() {
        let u = p.function(hir::DefId(i)).usage();
        assert_eq!(
            (u.expanded_cells, u.reference_bytes, u.native_bytes),
            (cells, bytes, native)
        );
    }
    for fuel in 0..=1086 {
        let mut events = vec![];
        let result = run_observed(
            &witness,
            schedule.entry,
            Limits {
                fuel,
                ..Limits::default()
            },
            &mut events,
        );
        match schedule.failure(fuel) {
            Some(s) => assert_eq!(
                result,
                Err(OwnedRunFailure::Scalar(RunFailure::Fuel(s))),
                "fuel={fuel}"
            ),
            None => assert_eq!(result, Ok(Scalar::I32(816))),
        }
        let actual: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let Event::Charge(s, c) = e {
                    Some((*s, *c))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(actual, schedule.events[..actual.len()], "fuel={fuel}");
        if fuel == 1086 {
            let writes: Vec<_> = events
                .iter()
                .filter_map(|e| {
                    if let Event::WriteField(_, field, value) = e {
                        Some((field.index, *value))
                    } else {
                        None
                    }
                })
                .collect();
            let mut expected = vec![];
            let mut checksum = 0;
            for job in 1..=6 {
                checksum += job * 10;
                expected.extend([
                    (1, Scalar::I32(job)),
                    (0, Scalar::I32(job)),
                    (2, Scalar::I32(checksum)),
                ]);
            }
            expected.push((3, Scalar::Bool(false)));
            assert_eq!(writes, expected);
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, Event::Acquire(..)))
                    .count(),
                24
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, Event::Release(..)))
                    .count(),
                24
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, Event::Transfer(..)))
                    .count(),
                6
            );
        }
    }
}

#[test]
fn nested_shared_aliases_allow_parent_reads_and_restore_exclusive_writes() {
    for exclusive in [false, true] {
        let (sources, raw, schedule) = shared_children(exclusive);
        let p = verified::verify_owned(raw, &sources).unwrap();
        assert_eq!(schedule.fuel(), if exclusive { 116 } else { 113 });
        for fuel in 0..=schedule.fuel() {
            let mut events = vec![];
            let result = run_observed(
                &p,
                schedule.entry,
                Limits {
                    fuel,
                    ..Limits::default()
                },
                &mut events,
            );
            match schedule.failure(fuel) {
                Some(s) => assert_eq!(
                    result,
                    Err(OwnedRunFailure::Scalar(RunFailure::Fuel(s))),
                    "exclusive={exclusive},fuel={fuel}"
                ),
                None => assert_eq!(result, Ok(schedule.result)),
            }
            let charged: Vec<_> = events
                .iter()
                .filter_map(|e| {
                    if let Event::Charge(s, c) = e {
                        Some((*s, *c))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(charged, schedule.events[..charged.len()]);
            if fuel == schedule.fuel() {
                let writes: Vec<_> = events
                    .iter()
                    .filter_map(|e| {
                        if let Event::WriteField(_, _, v) = e {
                            Some(*v)
                        } else {
                            None
                        }
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
                assert_eq!(
                    events
                        .iter()
                        .filter(|e| matches!(e, Event::Acquire(..)))
                        .count(),
                    3
                );
                assert_eq!(
                    events
                        .iter()
                        .filter(|e| matches!(e, Event::Release(..)))
                        .count(),
                    3
                );
            }
        }
    }
}

fn stopped_machine<'p, 'w>(plan: &'p ExecutionPlan<'w>, fuel: usize) -> Machine<'p, 'w> {
    let limits = Limits {
        fuel,
        ..Limits::default()
    };
    let mut m = Machine {
        plan,
        frames: vec![],
        limits,
        fuel,
        next_activation: 1,
        live_slots: 0,
        live_cells: 0,
        live_bytes: 0,
        header_bytes: limits.frames * size_of::<Frame>(),
        events: vec![],
    };
    let f = &plan.witness().functions()[0];
    m.activation_preflight(f.id, 1 + plan.function(f.id).usage().expanded_cells, f.span)
        .unwrap();
    m.frames = plan::reserve(limits.frames).unwrap();
    m.install(Frame::allocate(plan, f.id, 1, None).unwrap());
    assert!(matches!(
        m.execute(),
        Err(OwnedRunFailure::Scalar(RunFailure::Fuel(_)))
    ));
    m
}
fn prefix_before(schedule: &Schedule, span: Span) -> usize {
    let mut cost = 0;
    for &(s, c) in &schedule.events {
        if s == span {
            return cost;
        }
        cost += c;
    }
    panic!("missing schedule origin")
}
#[test]
fn stale_identity_and_nested_permission_checks_are_transition_inductive() {
    let (sources, raw, schedule) = super::super::consumer_pilot::batch();
    let p = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&p).unwrap();
    let first_commit = p.functions()[2].blocks[0].statements[0].span;
    let mut m = stopped_machine(&plan, prefix_before(&schedule, first_commit));
    assert_eq!(m.frames.len(), 3);
    let child = m.frames[2].references[0];
    let parent = m.frames[1].references[0];
    assert!(m
        .validate_handle(child, Access::Write, first_commit)
        .is_ok());
    assert!(m
        .validate_handle(parent, Access::Read, first_commit)
        .is_err());
    let before = m.frames[0].loans[1];
    assert!(m.release(parent.permission, first_commit).is_err());
    assert_eq!(m.frames[0].loans[1], before); // child-active ancestor cannot end
    for corrupted in [
        ReferenceHandle {
            root: OwnerKey {
                activation: child.root.activation + 1,
                ..child.root
            },
            ..child
        },
        ReferenceHandle {
            root: OwnerKey {
                generation: child.root.generation + 1,
                ..child.root
            },
            ..child
        },
        ReferenceHandle {
            root: OwnerKey {
                frame: u64::MAX,
                ..child.root
            },
            ..child
        },
        ReferenceHandle {
            root: OwnerKey {
                owner: u64::MAX,
                ..child.root
            },
            ..child
        },
        ReferenceHandle {
            permission: LoanKey {
                instance: child.permission.instance + 1,
                ..child.permission
            },
            ..child
        },
        ReferenceHandle {
            permission: LoanKey {
                activation: child.permission.activation + 1,
                ..child.permission
            },
            ..child
        },
        ReferenceHandle {
            permission: LoanKey {
                loan: u64::MAX,
                ..child.permission
            },
            ..child
        },
    ] {
        assert!(m
            .validate_handle(corrupted, Access::Read, first_commit)
            .is_err());
    }
    let parent_return = p.functions()[3].blocks[1].terminator.as_ref().unwrap().span;
    let after = stopped_machine(&plan, prefix_before(&schedule, parent_return));
    assert_eq!(after.frames.len(), 2);
    assert!(after
        .validate_handle(after.frames[1].references[0], Access::Write, parent_return)
        .is_ok());
    assert!(after
        .validate_handle(child, Access::Read, parent_return)
        .is_err()); // child loan ended
                    // The next iteration reuses frame indices and static loans but has fresh activation/instance epochs.
    let mut seen = 0;
    let mut second_prefix = 0;
    for &(s, c) in &schedule.events {
        if s == first_commit {
            seen += 1;
            if seen == 2 {
                break;
            }
        }
        second_prefix += c;
    }
    let again = stopped_machine(&plan, second_prefix);
    assert_eq!(again.frames.len(), 3);
    assert!(again
        .validate_handle(child, Access::Read, first_commit)
        .is_err());
    assert_ne!(again.frames[2].activation, m.frames[2].activation);
    assert!(again.frames[0].loans[1].instance > m.frames[0].loans[1].instance);
    // A corrupted dynamic function identity is diagnosed before any indexed plan access.
    m.frames[0].function = hir::DefId(usize::MAX);
    assert!(m.owner(child.root, first_commit).is_err());
}

#[test]
fn preparation_snapshots_are_stored_independently_of_test_owned_scalar_cells() {
    let (sources, raw, schedule) = super::super::consumer_pilot::batch();
    let p = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&p).unwrap();
    let invoke = p.functions()[0].blocks[7].terminator.as_ref().unwrap().span;
    let mut m = stopped_machine(&plan, prefix_before(&schedule, invoke));
    let snapshot = plan
        .function(hir::DefId(0))
        .call(CallSiteId(1))
        .argument_start()
        + 1;
    assert_eq!(m.frames[0].snapshots[snapshot], Some(Scalar::I32(1)));
    m.frames[0].slots[13] = Some(Scalar::I32(999)); // test-owned machine fault, not executable invalid OIR
    assert_eq!(m.frames[0].snapshots[snapshot], Some(Scalar::I32(1)));
}

#[test]
fn exact_activation_caps_and_failure_precedence_charge_before_allocation() {
    let (sources, raw, schedule) = super::super::consumer_pilot::batch();
    let p = verified::verify_owned(raw, &sources).unwrap();
    let bytes = 3 * size_of::<Frame>() + 1152 + size_of::<Scalar>();
    let limits = Limits {
        fuel: 1086,
        frames: 3,
        slots: 30,
        cells: 154,
        bytes,
    };
    assert_eq!(
        run_limits(&p, Some(schedule.entry), limits),
        Ok(Scalar::I32(816))
    );
    let call = p.functions()[3].blocks[0].terminator.as_ref().unwrap().span;
    assert_eq!(
        run_limits(
            &p,
            Some(schedule.entry),
            Limits {
                frames: 2,
                ..limits
            }
        ),
        Err(OwnedRunFailure::Scalar(RunFailure::Frames(call)))
    );
    assert_eq!(
        run_limits(
            &p,
            Some(schedule.entry),
            Limits {
                slots: 29,
                ..limits
            }
        ),
        Err(OwnedRunFailure::Scalar(RunFailure::Slots(call)))
    );
    assert_eq!(
        run_limits(
            &p,
            Some(schedule.entry),
            Limits {
                cells: 153,
                ..limits
            }
        ),
        Err(OwnedRunFailure::Resource(plan::AdmissionFailure::new(
            "live expanded cells",
            Some(call)
        )))
    );
    assert_eq!(
        run_limits(
            &p,
            Some(schedule.entry),
            Limits {
                bytes: bytes - 1,
                ..limits
            }
        ),
        Err(OwnedRunFailure::Resource(plan::AdmissionFailure::new(
            "live requested bytes",
            Some(call)
        )))
    );
    assert_eq!(
        run_limits(
            &p,
            Some(schedule.entry),
            Limits {
                fuel: 111,
                frames: 0,
                slots: 0,
                cells: 0,
                bytes: 0
            }
        ),
        Err(OwnedRunFailure::Scalar(RunFailure::Fuel(
            p.functions()[0].span
        )))
    );
    assert_eq!(
        run_limits(
            &p,
            Some(schedule.entry),
            Limits {
                fuel: 112,
                frames: 0,
                slots: 0,
                cells: 0,
                bytes: 0
            }
        ),
        Err(OwnedRunFailure::Scalar(RunFailure::Frames(
            p.functions()[0].span
        )))
    );
    eprintln!("owned reference sizes: Frame={} Scalar={} Owner={} Reference={} Loan={} Call={}; pilot exact requested bytes={bytes}",size_of::<Frame>(),size_of::<Scalar>(),size_of::<OwnerRuntime>(),size_of::<ReferenceHandle>(),size_of::<LoanRuntime>(),size_of::<CallRuntime>());
}

#[test]
fn every_runtime_reservation_failure_stops_before_an_unadmitted_activation() {
    let (sources, raw, schedule) = super::super::consumer_pilot::batch();
    let p = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&p).unwrap();
    let mut failures = 0;
    loop {
        let outcome = plan::fail_allocation_after(failures, || {
            execute_plan(&plan, schedule.entry, Limits::default(), None)
        });
        match outcome {
            Ok(v) => {
                assert_eq!(v, Scalar::I32(816));
                break;
            }
            Err(OwnedRunFailure::Resource(e)) => {
                assert_eq!(e.name, "injected owned allocation failure")
            }
            other => panic!("unexpected allocation sweep result {other:?}"),
        }
        failures += 1;
    }
    assert_eq!(failures, 190); // one frame-header reserve, 27 activations * seven fixed arrays
}

#[test]
fn mixed_layouts_replacement_and_loop_lifetimes_match_frozen_schedules() {
    let cases = vec![
        mixed_relay(0),
        mixed_relay(1),
        mixed_relay(2),
        owner_loop(),
        replacement(false),
        replacement(true),
    ];
    for ((sources, raw, schedule), expected_fuel) in cases.into_iter().zip([90, 90, 90, 66, 33, 35])
    {
        assert_eq!(schedule.fuel(), expected_fuel);
        let p = verified::verify_owned(raw, &sources).unwrap();
        for fuel in 0..=schedule.fuel() {
            let actual = run_limits(
                &p,
                Some(schedule.entry),
                Limits {
                    fuel,
                    ..Limits::default()
                },
            );
            match schedule.failure(fuel) {
                Some(s) => assert_eq!(
                    actual,
                    Err(OwnedRunFailure::Scalar(RunFailure::Fuel(s))),
                    "fuel={fuel},total={expected_fuel}"
                ),
                None => assert_eq!(actual, Ok(schedule.result)),
            }
        }
    }
    let (sources, raw, schedule) = owner_loop();
    let p = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&p).unwrap();
    assert_eq!(
        run_limits(
            &p,
            Some(schedule.entry),
            Limits {
                frames: 1,
                slots: 8,
                cells: 13,
                ..Limits::default()
            }
        ),
        Ok(Scalar::I32(3))
    );
    let discard = p.functions()[0].blocks[2].statements[3].span;
    let first = stopped_machine(&plan, prefix_before(&schedule, discard));
    let key = first.owner_key(0, OwnerPlaceId(0), discard).unwrap();
    let mut count = 0;
    let mut prefix = 0;
    for &(s, c) in &schedule.events {
        if s == discard {
            count += 1;
            if count == 2 {
                break;
            }
        }
        prefix += c;
    }
    let second = stopped_machine(&plan, prefix);
    assert_eq!(key.generation, 2);
    assert_eq!(second.frames[0].owners[0].generation, 6);
    assert!(second.owner(key, discard).is_err());
}

#[test]
fn canonical_scalar_redefinition_across_preparation_is_verifier_negative() {
    let (sources, raw) = forbidden_snapshot_redefinition();
    let error = verified::verify_owned(raw, &sources).unwrap_err();
    assert_eq!(
        error.kind,
        OwnedFailureKind::Ownership(Violation::CallRegion)
    );
}

#[test]
fn allocation_errors_keep_root_or_invoke_origin_without_relabeling_plan_admission() {
    let (sources, raw, schedule) = super::super::consumer_pilot::batch();
    let p = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&p).unwrap();
    for point in 0..190 {
        let mut events = vec![];
        let result = plan::fail_allocation_after(point, || {
            execute_plan(&plan, schedule.entry, Limits::default(), Some(&mut events))
        });
        let Err(OwnedRunFailure::Resource(error)) = result else {
            panic!("expected allocation denial at point{point}")
        };
        let origin = events.iter().rev().find_map(|event| {
            if let Event::Charge(span, _) = event {
                Some(*span)
            } else {
                None
            }
        });
        assert_eq!(error.span, origin, "allocation point{point}");
        assert!(error.span.is_some());
    }
    let prior = plan::fail_allocation_after(0, || run(&p, Some(schedule.entry))).unwrap_err();
    assert_eq!(
        prior,
        OwnedRunFailure::Resource(plan::AdmissionFailure::new(
            "injected owned allocation failure",
            None
        ))
    );
}

#[test]
fn distinct_call_results_and_nested_later_argument_loops_keep_value_snapshots() {
    for ((sources, raw, schedule), fuel) in [distinct_owned_results(), later_argument_loop()]
        .into_iter()
        .zip([104, 292])
    {
        assert_eq!(schedule.fuel(), fuel);
        let p = verified::verify_owned(raw, &sources).unwrap();
        for budget in 0..=fuel {
            let mut events = vec![];
            let actual = run_observed(
                &p,
                schedule.entry,
                Limits {
                    fuel: budget,
                    ..Limits::default()
                },
                &mut events,
            );
            match schedule.failure(budget) {
                Some(s) => assert_eq!(
                    actual,
                    Err(OwnedRunFailure::Scalar(RunFailure::Fuel(s))),
                    "fuel={budget}/{fuel}"
                ),
                None => assert_eq!(actual, Ok(schedule.result)),
            }
            let charged: Vec<_> = events
                .iter()
                .filter_map(|e| {
                    if let Event::Charge(s, c) = e {
                        Some((*s, *c))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(charged, schedule.events[..charged.len()]);
            if budget == fuel && fuel == 292 {
                let writes: Vec<_> = events
                    .iter()
                    .filter_map(|e| {
                        if let Event::WriteField(_, _, v) = e {
                            Some(*v)
                        } else {
                            None
                        }
                    })
                    .collect();
                assert_eq!(writes, (11..=16).map(Scalar::I32).collect::<Vec<_>>());
            }
        }
    }
    let (sources, raw, schedule) = later_argument_loop();
    let p = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&p).unwrap();
    let dispatch = p.functions()[0].blocks[6].terminator.as_ref().unwrap().span;
    let m = stopped_machine(&plan, prefix_before(&schedule, dispatch));
    let start = plan
        .function(hir::DefId(0))
        .call(CallSiteId(0))
        .argument_start();
    assert_eq!(
        &m.frames[0].snapshots[start..start + 2],
        &[Some(Scalar::I32(10)), Some(Scalar::I32(13))]
    );
    let mut seen = 0;
    let mut fuel = 0;
    for &(s, c) in &schedule.events {
        if s == dispatch {
            seen += 1;
            if seen == 2 {
                break;
            }
        }
        fuel += c;
    }
    let next = stopped_machine(&plan, fuel);
    assert_eq!(
        &next.frames[0].snapshots[start..start + 2],
        &[Some(Scalar::I32(13)), Some(Scalar::I32(16))]
    );
}

#[test]
fn entry_denials_precede_plan_allocations_for_every_parameter_kind_and_owned_result() {
    let (sources, raw, _) = empty_record();
    let p = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(
        plan::fail_allocation_after(0, || run(&p, None)),
        Err(OwnedRunFailure::Scalar(RunFailure::Entry(None)))
    );
    assert!(matches!(
        plan::fail_allocation_after(0, || run(&p, Some(hir::DefId(usize::MAX)))),
        Err(OwnedRunFailure::Invariant("entry identity", None))
    ));
    for (sources, raw, index) in [
        (owned_relay().0, owned_relay().1, 1),
        (shared_read().0, shared_read().1, 1),
        (later_argument_loop().0, later_argument_loop().1, 2),
    ] {
        let p = verified::verify_owned(raw, &sources).unwrap();
        let span = p.functions()[index].span;
        assert_eq!(
            plan::fail_allocation_after(0, || run(&p, Some(hir::DefId(index)))),
            Err(OwnedRunFailure::Scalar(RunFailure::Entry(Some(span))))
        );
    }
    let (sources, mut raw, _) = owned_relay();
    let f = &mut raw.functions[0];
    f.result = ValueTy::Owned(RecordId(0));
    f.blocks[1].statements.clear();
    f.locals.truncate(1);
    f.blocks[1].terminator.as_mut().unwrap().kind =
        OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(2));
    let p = verified::verify_owned(raw, &sources).unwrap();
    let span = p.functions()[0].span;
    assert_eq!(
        plan::fail_allocation_after(0, || run(&p, Some(hir::DefId(0)))),
        Err(OwnedRunFailure::EntryResult(span))
    );
}

#[test]
fn failure_after_prior_write_preserves_order_and_skips_return_cleanup() {
    let (sources, raw, overflow, charges) = super::super::consumer_pilot::overflow_after_write();
    let p = verified::verify_owned(raw, &sources).unwrap();
    for fuel in 0..=235 {
        let mut events = vec![];
        let result = run_observed(
            &p,
            hir::DefId(0),
            Limits {
                fuel,
                ..Limits::default()
            },
            &mut events,
        );
        let mut left = fuel;
        let mut unpaid = None;
        for &(span, cost) in &charges {
            if left < cost {
                unpaid = Some(span);
                break;
            }
            left -= cost;
        }
        if let Some(span) = unpaid {
            assert_eq!(
                result,
                Err(OwnedRunFailure::Scalar(RunFailure::Fuel(span))),
                "fuel={fuel}"
            );
        } else {
            assert_eq!(
                result,
                Err(OwnedRunFailure::Scalar(RunFailure::Overflow(overflow)))
            );
        }
        if fuel >= 230 {
            let writes: Vec<_> = events
                .iter()
                .filter_map(|e| {
                    if let Event::WriteField(_, f, v) = e {
                        Some((f.index, *v))
                    } else {
                        None
                    }
                })
                .collect();
            let expected = if fuel == 230 {
                vec![(1, Scalar::I32(1))]
            } else {
                vec![(1, Scalar::I32(1)), (0, Scalar::I32(1))]
            };
            assert_eq!(writes, expected, "fuel={fuel}");
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, Event::Release(..)))
                    .count(),
                1
            ); // only the earlier retry returned
        }
    }
}

#[test]
fn transfer_observations_identify_the_newly_initialized_destination_generation() {
    let (sources, raw, schedule) = super::super::consumer_pilot::batch();
    let p = verified::verify_owned(raw, &sources).unwrap();
    let mut events = vec![];
    assert_eq!(
        run_observed(&p, schedule.entry, Limits::default(), &mut events),
        Ok(Scalar::I32(816))
    );
    let endpoints: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let Event::Transfer(from, to) = e {
                Some((from.generation, to.generation))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(endpoints, [(2, 2), (2, 1), (1, 1), (1, 2), (2, 2), (2, 1)]);
}

// Independent reviewer artifact is retained byte-for-byte.
include!("reviewer_reference_tests.rs");

#[test]
fn default_200000_expanded_cell_boundary_is_real_and_precedes_activation_allocation() {
    let (sources, s) = context();
    let span = s(0);
    let mut records = vec![
        record(&vec![hir::Ty::I32; 1024], span),
        record(&vec![hir::Ty::I32; 563], span),
    ];
    records[1].id = RecordId(1);
    for field in &mut records[1].fields {
        field.id.record = RecordId(1);
    }
    let mut f = function(0, ValueTy::Scalar(hir::Ty::Unit), span);
    f.locals = vec![scalar(hir::Ty::Unit, span)];
    f.owners = (0..195)
        .map(|i| OwnerDecl {
            record: RecordId(usize::from(i == 194)),
            kind: OwnerKind::Local { mutable: false },
            span,
        })
        .collect();
    let mut statements = vec![assign(0, Rvalue::Unit, span)];
    for i in 0..195 {
        statements.extend([
            instruction(OwnedInstruction::StorageLive(OwnerPlaceId(i)), span),
            instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(i)), span),
        ]);
    }
    f.blocks = vec![OwnedBlock {
        merge: None,
        span,
        statements,
        terminator: end(OwnedTerminatorKind::ReturnScalar(operand(0, span)), span),
    }];
    let mut larger = f.clone();
    larger.locals.push(scalar(hir::Ty::Unit, span));
    larger.blocks[0]
        .statements
        .push(assign(1, Rvalue::Unit, span));
    let p = verified::verify_owned(
        RawOwnedProgram {
            records,
            functions: vec![f],
        },
        &sources,
    )
    .unwrap();
    let plan = ExecutionPlan::build(&p).unwrap();
    let u = plan.function(hir::DefId(0)).usage();
    assert_eq!(u.expanded_cells, 200000);
    assert_eq!(u.reference_bytes, 803124);
    assert_eq!(run(&p, Some(hir::DefId(0))), Ok(Scalar::Unit));
    let mut records = vec![
        record(&vec![hir::Ty::I32; 1024], span),
        record(&vec![hir::Ty::I32; 563], span),
    ];
    records[1].id = RecordId(1);
    for field in &mut records[1].fields {
        field.id.record = RecordId(1);
    }
    let p = verified::verify_owned(
        RawOwnedProgram {
            records,
            functions: vec![larger],
        },
        &sources,
    )
    .unwrap();
    let plan = ExecutionPlan::build(&p).unwrap();
    assert_eq!(plan.function(hir::DefId(0)).usage().expanded_cells, 200001);
    let result = plan::fail_allocation_after(0, || {
        execute_plan(&plan, hir::DefId(0), Limits::default(), None)
    });
    assert_eq!(
        result,
        Err(OwnedRunFailure::Resource(plan::AdmissionFailure::new(
            "live expanded cells",
            Some(span)
        )))
    );
}

#[test]
fn reference_default_1024_frame_bound_supports_deep_reborrow_provenance() {
    let (sources, s) = context();
    let mut main = function(0, ValueTy::Scalar(hir::Ty::Unit), s(0));
    main.locals = vec![scalar(hir::Ty::Unit, s(0))];
    main.owners = vec![owner(OwnerKind::Local { mutable: false }, s(0))];
    main.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
        result: CallResult::Scalar(LocalId(0)),
        parent: None,
        span: s(1),
    }];
    main.loans = vec![LoanDecl {
        call: CallSiteId(0),
        argument: 0,
        authority: AccessBase::Owner(OwnerPlaceId(0)),
        kind: BorrowKind::Shared,
        record: RecordId(0),
        span: s(2),
    }];
    main.blocks = vec![
        OwnedBlock {
            span: s(0),
            merge: None,
            statements: vec![
                instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(0)),
                instruction(
                    OwnedInstruction::Construct {
                        destination: OwnerPlaceId(0),
                        fields: vec![],
                    },
                    s(0),
                ),
                instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(1)),
                instruction(
                    OwnedInstruction::PrepareBorrow {
                        call: CallSiteId(0),
                        argument: 0,
                        loan: LoanId(0),
                    },
                    s(2),
                ),
            ],
            terminator: end(
                OwnedTerminatorKind::Invoke {
                    call: CallSiteId(0),
                    continuation: BlockId(1),
                },
                s(3),
            ),
        },
        OwnedBlock {
            span: s(4),
            merge: None,
            statements: vec![instruction(
                OwnedInstruction::StorageEnd(OwnerPlaceId(0)),
                s(4),
            )],
            terminator: end(OwnedTerminatorKind::ReturnScalar(operand(0, s(4))), s(4)),
        },
    ];
    let mut recursive = function(1, ValueTy::Scalar(hir::Ty::Unit), s(5));
    recursive.locals = vec![scalar(hir::Ty::Unit, s(5))];
    recursive.parameters = vec![ParameterBinding::Reference(ReferenceParamId(0))];
    recursive.references = vec![ReferenceDecl {
        record: RecordId(0),
        kind: BorrowKind::Shared,
        position: 0,
        span: s(5),
    }];
    recursive.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
        result: CallResult::Scalar(LocalId(0)),
        parent: None,
        span: s(6),
    }];
    recursive.loans = vec![LoanDecl {
        call: CallSiteId(0),
        argument: 0,
        authority: AccessBase::Parameter(ReferenceParamId(0)),
        kind: BorrowKind::Shared,
        record: RecordId(0),
        span: s(7),
    }];
    recursive.blocks = vec![
        OwnedBlock {
            span: s(5),
            merge: None,
            statements: vec![
                instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(6)),
                instruction(
                    OwnedInstruction::PrepareBorrow {
                        call: CallSiteId(0),
                        argument: 0,
                        loan: LoanId(0),
                    },
                    s(7),
                ),
            ],
            terminator: end(
                OwnedTerminatorKind::Invoke {
                    call: CallSiteId(0),
                    continuation: BlockId(1),
                },
                s(8),
            ),
        },
        OwnedBlock {
            span: s(9),
            merge: None,
            statements: vec![],
            terminator: end(OwnedTerminatorKind::ReturnScalar(operand(0, s(9))), s(9)),
        },
    ];
    let p = verified::verify_owned(
        RawOwnedProgram {
            records: vec![record(&[], s(0))],
            functions: vec![main, recursive],
        },
        &sources,
    )
    .unwrap();
    let mut events = vec![];
    assert_eq!(
        run_observed(&p, hir::DefId(0), Limits::default(), &mut events),
        Err(OwnedRunFailure::Scalar(RunFailure::Frames(s(8))))
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Enter(..)))
            .count(),
        1024
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Acquire(..)))
            .count(),
        1024
    );
    assert!(
        super::super::native::native_module(&p, Some(hir::DefId(0)), &sources)
            .unwrap_err()
            .message
            .contains("recursive")
    );
}

#[path = "source/reviewer_resource_runtime.rs"]
mod reviewer_resources;
