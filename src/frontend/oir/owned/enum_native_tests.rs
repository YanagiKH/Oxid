//! B2b native enum controls. Fixtures and fuel schedules are raw declarations
//! and independent hand counts; only production sealed verification is used.
use super::*;
use crate::frontend::oir::owned::enum_consumer_fixtures as e;

fn checked(raw: RawOwnedProgram, sources: &SourceMap, guarded: bool) -> VerifiedOwnedProgram {
    let mut raw = raw;
    if guarded {
        append_cycle(&mut raw);
    }
    verified::verify_owned(raw, sources).unwrap()
}

fn invariant(name: &'static str, span: Span, sources: &SourceMap) -> String {
    execute::OwnedRunFailure::Invariant(name, Some(span))
        .diagnostic(sources)
        .render_human(sources)
}

fn before(module: &str, needle: &str, insertion: &str) -> String {
    assert_eq!(
        module.matches(needle).count(),
        1,
        "unique mutation site: {needle}"
    );
    module.replacen(needle, &format!("{insertion}{needle}"), 1)
}

#[test]
fn native_enums_declared_payloads_written_orders_and_expansion_inventory() {
    let (sources, s) = fixtures::context();
    for order in [[0, 1, 2, 3], [3, 2, 1, 0], [1, 3, 0, 2]] {
        for (constructed, payload) in e::MIXED.into_iter().enumerate() {
            let result = payload.unwrap_or(hir::Ty::I32);
            for guarded in [false, true] {
                let (raw, schedule) = e::mixed_case(&e::MIXED, &order, constructed, result, s(0));
                let witness = checked(raw, &sources, guarded);
                assert_eq!(
                    execute::run(&witness, Some(schedule.entry)),
                    Ok(schedule.result)
                );
                let observation = run_array_observed(
                    &witness,
                    Some(schedule.entry),
                    &sources,
                    NativeControl::default(),
                );
                let module = observation.result.unwrap();
                let metrics = observation.metrics;
                assert_eq!(metrics.count_bytes, module.len());
                assert_eq!(metrics.count_bytes, metrics.render_bytes);
                assert_eq!(metrics.count_expansions, metrics.render_expansions);
                assert_eq!(
                    metrics.count_expansion_kinds,
                    metrics.render_expansion_kinds
                );
                assert_eq!(metrics.transfer_cells, 1 + usize::from(constructed != 0));
                assert_eq!(metrics.count_expansion_kinds[0], metrics.transfer_cells);
                assert_eq!(metrics.count_expansion_kinds[1], 0);
                assert_eq!(
                    metrics.count_expansions,
                    metrics.transfer_cells + metrics.message_bytes
                );
                assert_eq!(module.contains("%fuel = alloca"), guarded);
                assert_eq!(module.matches("_valid = icmp ult i32").count(), 4);
                assert_eq!(module.matches("_selected = icmp eq i32").count(), 8);
                assert!(!module.contains("load i1, ptr"));
                assert!(!module.contains("memcpy"));
                assert!(!module.contains("load i64, ptr %o"));
                assert!(!module.contains("store i64 0, ptr %o"));
                for (arm, tag) in order.into_iter().enumerate() {
                    let dispatch = format!("f0_b{arm}_term");
                    assert!(module.contains(&format!(
                        "%{dispatch}_selected = icmp eq i32 %{dispatch}_tag, {tag}"
                    )));
                    if arm == 3 {
                        assert!(module.contains(&format!(
                            "br i1 %{dispatch}_selected, label %b7, label %{dispatch}_tag_error"
                        )));
                    }
                }
                for (name, failure) in [
                    ("enum tag", FailureKind::EnumTag),
                    ("enum payload", FailureKind::EnumPayload),
                ] {
                    assert_eq!(
                        failure
                            .diagnostic(e::at(s(0), 5), &sources)
                            .render_human(&sources),
                        invariant(name, e::at(s(0), 5), &sources)
                    );
                    assert!(failure.diagnostic(s(0), &sources).message.capacity() <= 64);
                }
            }
        }
    }
}

#[test]
fn native_enums_all_whole_copy_sites_validate_before_destination_stores() {
    let (sources, s) = fixtures::context();
    let (raw, schedule) = e::relay_case(1, hir::Ty::Bool, s(0));
    let witness = checked(raw, &sources, false);
    let observation = run_array_observed(
        &witness,
        Some(schedule.entry),
        &sources,
        NativeControl::default(),
    );
    let module = observation.result.unwrap();
    // Six copies, each emitting four switch rows and four case bodies. Two
    // constructors each write one tag and one active boolean byte.
    assert_eq!(observation.metrics.count_expansion_kinds[0], 4);
    assert_eq!(observation.metrics.count_expansion_kinds[1], 48);
    assert_eq!(observation.metrics.transfer_cells, 52);
    assert_eq!(module.matches("switch i32").count(), 6);
    for line in module.lines().filter(|line| line.contains("switch i32")) {
        let name = line
            .trim()
            .strip_prefix("switch i32 %")
            .unwrap()
            .split_once("_tag,")
            .unwrap()
            .0;
        let case = format!("{name}_variant1");
        let start = module.find(&format!("{case}:\n")).unwrap();
        let end = module[start..]
            .find(&format!("br label %{name}_enum_ok"))
            .unwrap()
            + start;
        let text = &module[start..end];
        let load = text.find(" = load i8, ptr").unwrap();
        let check = text.find(" = icmp ule i8").unwrap();
        let success = text.find(&format!("{case}_payload_ok:\n")).unwrap();
        let convert = text.find(" = trunc i8").unwrap();
        let store = text.find("  store i32 1, ptr").unwrap();
        assert!(load < check && check < success && success < convert && convert < store);
        assert!(text.contains("zext i1"));
        assert!(text.contains("store i8"));
        let nullary_start = module.find(&format!("{name}_variant0:\n")).unwrap();
        let nullary_end = module[nullary_start..]
            .find(&format!("br label %{name}_enum_ok"))
            .unwrap()
            + nullary_start;
        assert!(!module[nullary_start..nullary_end].contains("load "));
        assert!(!module[nullary_start..nullary_end].contains("getelementptr"));
    }
    assert!(module.contains("%f1_param0_tag = load i32, ptr %arg0"));
    assert!(module.contains("%f1_b0_term_tag = load i32, ptr %o0"));
    assert!(module.contains("store i32 1, ptr %result"));
}

#[derive(Clone, Copy, Debug)]
enum PhiOperation {
    Move,
    Replace,
    Prepare,
    Discard,
    Consume,
}

/// The operation under test is the final split in a phi predecessor. Earlier
/// boolean definitions dominate both edges, with canonical owner/call sites.
fn phi_case(
    operation: PhiOperation,
    left: bool,
    span: Span,
) -> (RawOwnedProgram, fixtures::Schedule) {
    use fixtures::{assign, end, function, instruction as i, operand as o, scalar};
    let s = |n| e::at(span, n);
    let mut f = function(0, ValueTy::Scalar(hir::Ty::Bool), span);
    f.locals = vec![
        scalar(hir::Ty::Bool, s(1)),
        scalar(hir::Ty::Bool, s(2)),
        scalar(hir::Ty::Bool, s(3)),
        scalar(hir::Ty::Bool, s(11)),
    ];
    f.owners
        .push(e::owner(OwnerKind::Local { mutable: false }, span));
    let mut functions = vec![];
    let mut events;
    let initial = vec![
        assign(0, Rvalue::Bool(true), s(1)),
        assign(1, Rvalue::Bool(left), s(2)),
        assign(2, Rvalue::Bool(false), s(3)),
    ];
    if matches!(operation, PhiOperation::Consume) {
        // A two-arm match supplies exactly the two phi predecessors. Bindings
        // are deliberately unused: the last split remains the consume itself.
        f.locals.extend([
            LocalDecl {
                ty: hir::Ty::Bool,
                kind: LocalKind::Binding,
                span: s(6),
            },
            LocalDecl {
                ty: hir::Ty::Bool,
                kind: LocalKind::Binding,
                span: s(6),
            },
        ]);
        let mut initial = initial;
        initial.extend([
            i(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(4)),
            i(
                OwnedInstruction::ConstructEnum {
                    destination: OwnerPlaceId(0),
                    variant: e::variant(usize::from(!left)),
                    payload: Some(o(0, s(5))),
                },
                s(5),
            ),
        ]);
        f.matches.push(MatchDecl {
            source: OwnerPlaceId(0),
            span: s(6),
            arms: vec![
                MatchArm {
                    variant: e::variant(0),
                    dispatch: BlockId(0),
                    entry: BlockId(2),
                },
                MatchArm {
                    variant: e::variant(1),
                    dispatch: BlockId(1),
                    entry: BlockId(3),
                },
            ],
        });
        f.blocks = vec![
            e::block(
                initial,
                OwnedTerminatorKind::MatchDispatch {
                    match_id: MatchId(0),
                    arm: 0,
                },
                s(6),
            ),
            e::block(
                vec![],
                OwnedTerminatorKind::MatchDispatch {
                    match_id: MatchId(0),
                    arm: 1,
                },
                s(6),
            ),
        ];
        for arm in 0..2 {
            f.blocks.push(e::block(
                vec![i(
                    OwnedInstruction::ConsumeVariant {
                        match_id: MatchId(0),
                        arm,
                        destination: Some(LocalId(4 + arm)),
                    },
                    s(6),
                )],
                OwnedTerminatorKind::Goto(BlockId(4)),
                s(10),
            ));
        }
        events = vec![
            (span, 13),
            (s(1), 1),
            (s(2), 1),
            (s(3), 1),
            (s(4), 1),
            (s(5), 3),
            (s(6), 1),
        ];
        if !left {
            events.push((s(6), 1));
        }
        events.extend([(s(6), 3), (s(10), 1), (s(11), 1), (s(12), 3), (s(15), 3)]);
    } else {
        let has_destination = !matches!(operation, PhiOperation::Discard);
        if has_destination {
            let kind = if matches!(operation, PhiOperation::Prepare) {
                OwnerKind::StagedArgument {
                    call: CallSiteId(0),
                    argument: 0,
                }
            } else {
                OwnerKind::Local { mutable: true }
            };
            f.owners.push(e::owner(kind, span));
        }
        if matches!(operation, PhiOperation::Prepare) {
            f.locals.push(scalar(hir::Ty::Unit, s(13)));
            f.calls.push(CallDecl {
                target: hir::DefId(1),
                arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(1))],
                result: CallResult::Scalar(LocalId(4)),
                parent: None,
                span: s(7),
            });
            let mut callee = function(1, ValueTy::Scalar(hir::Ty::Unit), s(20));
            callee.locals.push(scalar(hir::Ty::Unit, s(21)));
            callee
                .parameters
                .push(ParameterBinding::Owned(OwnerPlaceId(0)));
            callee
                .owners
                .push(e::owner(OwnerKind::Parameter { position: 0 }, s(20)));
            callee.blocks.push(e::block(
                vec![
                    i(OwnedInstruction::Discard(OwnerPlaceId(0)), s(21)),
                    assign(0, Rvalue::Unit, s(22)),
                ],
                OwnedTerminatorKind::ReturnScalar(o(0, s(23))),
                s(23),
            ));
            functions.push(callee);
        }
        // One canonical live/initialize/prepare site dominates the split.
        // Its direct edge reaches the merge; the other edge passes through an
        // empty block, so one phi predecessor must be the enum continuation.
        let mut statements = initial;
        statements.extend([
            i(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(5)),
            i(
                OwnedInstruction::ConstructEnum {
                    destination: OwnerPlaceId(0),
                    variant: e::variant(0),
                    payload: Some(o(0, s(6))),
                },
                s(6),
            ),
        ]);
        if matches!(operation, PhiOperation::Prepare) {
            statements.push(i(OwnedInstruction::OpenCall(CallSiteId(0)), s(7)));
        } else if has_destination {
            statements.push(i(OwnedInstruction::StorageLive(OwnerPlaceId(1)), s(7)));
        }
        if matches!(operation, PhiOperation::Replace) {
            statements.push(i(
                OwnedInstruction::ConstructEnum {
                    destination: OwnerPlaceId(1),
                    variant: e::variant(1),
                    payload: Some(o(2, s(8))),
                },
                s(8),
            ));
        }
        statements.push(i(
            match operation {
                PhiOperation::Move => OwnedInstruction::MoveInitialize {
                    destination: OwnerPlaceId(1),
                    source: OwnerPlaceId(0),
                },
                PhiOperation::Replace => OwnedInstruction::Replace {
                    destination: OwnerPlaceId(1),
                    source: OwnerPlaceId(0),
                },
                PhiOperation::Prepare => OwnedInstruction::PrepareOwned {
                    call: CallSiteId(0),
                    argument: 0,
                    source: OwnerPlaceId(0),
                },
                PhiOperation::Discard => OwnedInstruction::Discard(OwnerPlaceId(0)),
                PhiOperation::Consume => unreachable!(),
            },
            s(9),
        ));
        f.blocks.push(e::block(
            statements,
            OwnedTerminatorKind::Branch {
                condition: o(1, s(10)),
                then_block: BlockId(2),
                else_block: BlockId(1),
            },
            s(10),
        ));
        f.blocks.push(e::block(
            vec![],
            OwnedTerminatorKind::Goto(BlockId(2)),
            s(16),
        ));
        // Activation: 1 root + 4 (or5) scalar cells + 6 per owner;
        // Prepare adds one staged scalar slot and two call-state cells.
        let activation = match operation {
            PhiOperation::Discard => 11,
            PhiOperation::Prepare => 21,
            _ => 17,
        };
        events = vec![
            (span, activation),
            (s(1), 1),
            (s(2), 1),
            (s(3), 1),
            (s(5), 1),
            (s(6), 3),
        ];
        if has_destination {
            events.push((
                s(7),
                if matches!(operation, PhiOperation::Prepare) {
                    2
                } else {
                    1
                },
            ));
        }
        if matches!(operation, PhiOperation::Replace) {
            events.push((s(8), 3));
        }
        events.extend([
            (
                s(9),
                if matches!(operation, PhiOperation::Replace) {
                    5
                } else {
                    3
                },
            ),
            (s(10), 1),
        ]);
        if !left {
            events.push((s(16), 1));
        }
        events.push((s(11), 1));
        if matches!(operation, PhiOperation::Prepare) {
            events.extend([
                (s(13), 11),
                (s(21), 3),
                (s(22), 1),
                (s(23), 3),
                (s(12), 3),
                (s(15), 6),
            ]);
        } else {
            events.push((s(12), 3));
            if has_destination {
                events.extend([(s(13), 3), (s(14), 3)]);
            }
            events.push((s(15), if has_destination { 5 } else { 3 }));
        }
    }
    let join = f.blocks.len();
    let predecessors = if matches!(operation, PhiOperation::Consume) {
        [2, 3]
    } else {
        [0, 1]
    };
    let mut cleanup = vec![i(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s(12))];
    if matches!(operation, PhiOperation::Move | PhiOperation::Replace) {
        cleanup.extend([
            i(OwnedInstruction::Discard(OwnerPlaceId(1)), s(13)),
            i(OwnedInstruction::StorageEnd(OwnerPlaceId(1)), s(14)),
        ]);
    }
    let mut join_block = if matches!(operation, PhiOperation::Prepare) {
        e::block(
            vec![],
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(join + 1),
            },
            s(13),
        )
    } else {
        e::block(
            cleanup.clone(),
            OwnedTerminatorKind::ReturnScalar(o(3, s(15))),
            s(15),
        )
    };
    join_block.merge = Some(BoolMerge {
        destination: LocalId(3),
        incoming: [
            MergeInput {
                predecessor: BlockId(predecessors[0]),
                value: o(0, s(11)),
            },
            MergeInput {
                predecessor: BlockId(predecessors[1]),
                value: o(2, s(11)),
            },
        ],
        operator_span: s(11),
        span: s(11),
    });
    f.blocks.push(join_block);
    if matches!(operation, PhiOperation::Prepare) {
        f.blocks.push(OwnedBlock {
            merge: None,
            span: s(12),
            statements: cleanup,
            terminator: end(OwnedTerminatorKind::ReturnScalar(o(3, s(15))), s(15)),
        });
    }
    functions.insert(0, f);
    (
        RawOwnedProgram {
            enums: vec![e::enumeration(
                &[Some(hir::Ty::Bool), Some(hir::Ty::Bool)],
                span,
            )],
            records: vec![],
            functions,
        },
        fixtures::Schedule {
            entry: hir::DefId(0),
            result: Scalar::Bool(left),
            events,
        },
    )
}

#[test]
fn native_enums_every_split_operation_names_the_real_phi_predecessor() {
    let (sources, s) = fixtures::context();
    for operation in [
        PhiOperation::Move,
        PhiOperation::Replace,
        PhiOperation::Prepare,
        PhiOperation::Discard,
        PhiOperation::Consume,
    ] {
        for guarded in [false, true] {
            for left in [false, true] {
                let (raw, schedule) = phi_case(operation, left, s(0));
                let witness = checked(raw, &sources, guarded);
                assert_eq!(
                    execute::run(&witness, Some(schedule.entry)),
                    Ok(schedule.result),
                    "{operation:?}"
                );
                let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
                let f = &witness.functions()[0];
                for merge in f.blocks.iter().filter_map(|b| b.merge.as_ref()) {
                    for input in merge.incoming {
                        let b = input.predecessor.0;
                        let statements = f.blocks[b].statements.len();
                        let label = if guarded {
                            format!("f0_b{b}_g{}_ok", statements + 1)
                        } else if statements == 0 {
                            format!("b{b}")
                        } else {
                            format!("f0_b{b}_i{}_enum_ok", statements - 1)
                        };
                        assert!(
                            module.contains(&format!("[ %s{}, %{label} ]", input.value.local.0)),
                            "{operation:?}: {label}"
                        );
                        assert!(module.contains(&format!("{label}:\n")));
                    }
                }
            }
        }
    }
}

#[test]
fn native_enums_native_limits_and_real_reservation_failures_remain_closed() {
    let (sources, s) = fixtures::context();
    let (raw, schedule) = e::mixed_case(&[None], &[0], 0, hir::Ty::I32, s(0));
    let witness = checked(raw, &sources, false);
    let observation = run_array_observed(
        &witness,
        Some(schedule.entry),
        &sources,
        NativeControl::default(),
    );
    let module = observation.result.unwrap();
    let metrics = observation.metrics;
    // One i64 scalar, one8-byte enum; scalar plus2 payload and4 owner-state cells.
    let exact = Limits {
        functions: 1,
        parameters: 0,
        function_slots: 2,
        scalar_slots: 1,
        blocks: 2,
        depth: 1,
        cost: schedule.fuel(),
        cells: 7,
        live_cells: 7,
        bytes: 16,
        live_bytes: 16,
        diagnostic_bytes: metrics.message_bytes,
        ir_bytes: module.len(),
        ..Limits::DEFAULT
    };
    assert_eq!(
        native_module_limits(
            &witness,
            Some(schedule.entry),
            &sources,
            plan::MAX_FUEL,
            exact
        )
        .unwrap(),
        module
    );
    for (limits, marker) in [
        (Limits { cells: 6, ..exact }, "expanded cells"),
        (Limits { bytes: 15, ..exact }, "storage bytes"),
        (
            Limits {
                live_bytes: 15,
                ..exact
            },
            "live storage bytes",
        ),
        (
            Limits {
                diagnostic_bytes: metrics.message_bytes - 1,
                ..exact
            },
            "diagnostic bytes",
        ),
        (
            Limits {
                ir_bytes: module.len() - 1,
                ..exact
            },
            "LLVM bytes",
        ),
    ] {
        let denied = native_module_limits(
            &witness,
            Some(schedule.entry),
            &sources,
            plan::MAX_FUEL,
            limits,
        )
        .unwrap_err();
        assert_eq!(denied.code, "E0700");
        assert!(denied.message.contains(marker), "{}", denied.message);
    }
    assert!(metrics.metadata_peak <= Limits::DEFAULT.metadata_bytes);
    assert!(metrics.metadata_admitted_bytes <= Limits::DEFAULT.metadata_bytes);
    assert_eq!(
        metrics.occurrence_bytes,
        metrics.occurrences * size_of::<DiagnosticOccurrence>()
    );
    assert_eq!(
        metrics.lookup_bytes,
        metrics.unique * size_of::<DiagnosticLookup>()
    );
    for failure in 0..metrics.allocation_attempts {
        let observed = run_array_observed(
            &witness,
            Some(schedule.entry),
            &sources,
            NativeControl {
                fail_after: Some(failure),
                ..NativeControl::default()
            },
        );
        let error = observed.result.unwrap_err();
        assert_eq!(error.code, "E0700");
        assert!(error.message.contains("injected native owned"));
        assert!(observed.metrics.failed_allocation.is_some());
        assert_eq!(observed.metrics.render_bytes, 0);
    }
    let below = run_array_observed(
        &witness,
        Some(schedule.entry),
        &sources,
        NativeControl {
            limits: Limits {
                metadata_bytes: metrics.metadata_admitted_bytes - 1,
                ..Limits::DEFAULT
            },
            ..NativeControl::default()
        },
    );
    assert!(below.result.unwrap_err().message.contains("metadata bytes"));
    assert_eq!(below.metrics.allocation_attempts, 0);
}

#[test]
fn native_enums_variant_count_not_width_bounds_copy_case_emission() {
    let (sources, s) = fixtures::context();
    let (raw, schedule) = e::mixed_case(
        &[None; 256],
        &(0..256).collect::<Vec<_>>(),
        0,
        hir::Ty::I32,
        s(0),
    );
    let witness = checked(raw, &sources, false);
    let plan = ExecutionPlan::build(&witness).unwrap();
    let diagnostics =
        Diagnostics::new(&plan, schedule.entry, &sources, false, MAX_DIAGNOSTIC_BYTES).unwrap();
    assert_eq!(plan.owner_width(schedule.entry, OwnerPlaceId(0)), 2);
    assert_eq!(
        transfer_expansions(&plan, schedule.entry, OwnerPlaceId(0)).unwrap(),
        512
    );
    let mut full = Emission {
        text: Some(String::new()),
        ..Emission::count(MAX_IR_BYTES)
    };
    enum_transfer(
        &mut full,
        &plan,
        "enum_copy",
        EnumId(0),
        "%source",
        Some("%destination"),
        (&diagnostics, e::at(s(0), 5)),
    );
    assert_eq!(full.expansions, 512);
    assert_eq!(full.expansion_kinds[1], 512);
    assert_eq!(
        full.text.as_ref().unwrap().matches("store i32").count(),
        256
    );
    assert!(!full.text.as_ref().unwrap().contains("getelementptr"));
    for maximum in [512, 1024, 4096] {
        let mut count = Emission::count(maximum);
        enum_transfer(
            &mut count,
            &plan,
            "enum_copy",
            EnumId(0),
            "%source",
            Some("%destination"),
            (&diagnostics, e::at(s(0), 5)),
        );
        assert!(count.exceeded);
        assert!(count.expansions < 512);
        assert!(count.len <= maximum);
    }
}

fn assert_schedule(
    witness: &VerifiedOwnedProgram,
    sources: &SourceMap,
    schedule: &fixtures::Schedule,
) {
    for fuel in 0..=schedule.fuel() + 1 {
        let actual = execute::run_limits(
            witness,
            Some(schedule.entry),
            execute::Limits {
                fuel,
                ..execute::Limits::default()
            },
        );
        if let Some(span) = schedule.failure(fuel) {
            assert_eq!(
                actual,
                Err(execute::OwnedRunFailure::Scalar(RunFailure::Fuel(span))),
                "fuel={fuel}"
            );
        } else {
            assert_eq!(actual, Ok(schedule.result), "fuel={fuel}");
        }
        // The oracle supplies the budget and error location independently;
        // emission at each exact budget also exercises count/render admission.
        let module = native_module_with_fuel(witness, schedule.entry, sources, fuel).unwrap();
        assert!(module.contains(&format!("store i64 {fuel}, ptr %fuel, align 8")));
    }
}

#[test]
fn native_enums_handcounted_schedules_and_dispatch_cycle_edges() {
    let (sources, s) = fixtures::context();
    for operation in [
        PhiOperation::Move,
        PhiOperation::Replace,
        PhiOperation::Prepare,
        PhiOperation::Discard,
        PhiOperation::Consume,
    ] {
        for left in [false, true] {
            let (raw, schedule) = phi_case(operation, left, s(0));
            let witness = checked(raw, &sources, true);
            assert_schedule(&witness, &sources, &schedule);
        }
    }
    let (raw, schedule) = e::loop_case(s(0));
    let witness = checked(raw, &sources, false);
    let f = &witness.functions()[0];
    assert!(has_cycle(f).unwrap().0);
    for descriptor in &f.matches {
        for (arm, site) in descriptor.arms.iter().enumerate() {
            let actual: Vec<_> = successors(
                f,
                &f.blocks[site.dispatch.0].terminator.as_ref().unwrap().kind,
            )
            .collect();
            let mut expected = vec![site.entry];
            if let Some(next) = descriptor.arms.get(arm + 1) {
                expected.push(next.dispatch);
            }
            assert_eq!(actual, expected);
        }
    }
    let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
    assert!(module.contains("%fuel = alloca"));
    assert_schedule(&witness, &sources, &schedule);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn exercise(
    scratch: &Scratch,
    sources: &SourceMap,
    name: &str,
    build: impl Fn() -> (RawOwnedProgram, fixtures::Schedule),
) -> usize {
    let (raw, schedule) = build();
    let witness = checked(raw, sources, false);
    let module = native_module(&witness, Some(schedule.entry), sources).unwrap();
    let binary = scratch.compile(&module, &format!("enum-{name}-production"));
    assert_result(
        scratch.run(&binary, &[]),
        &scalar_output(schedule.result),
        b"",
        0,
    );
    let (raw, schedule) = build();
    let witness = checked(raw, sources, true);
    let production =
        native_module_with_fuel(&witness, schedule.entry, sources, schedule.fuel()).unwrap();
    if let Some(evidence) = std::env::var_os("OXID_OWNED_NATIVE_EVIDENCE") {
        let path = std::path::PathBuf::from(evidence);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            path.join(format!("enum-{name}-guarded-production.ll")),
            &production,
        )
        .unwrap();
    }
    let harness = argv_fuel_harness(&production, schedule.fuel());
    let binary = scratch.compile(&harness, &format!("enum-{name}-all-fuel"));
    for fuel in 0..=schedule.fuel() + 1 {
        let actual = execute::run_limits(
            &witness,
            Some(schedule.entry),
            execute::Limits {
                fuel,
                ..execute::Limits::default()
            },
        );
        let output = scratch.run(&binary, &[fuel.to_string()]);
        if let Some(span) = schedule.failure(fuel) {
            let expected = RunFailure::Fuel(span)
                .diagnostic(sources)
                .render_human(sources);
            assert_eq!(
                actual
                    .unwrap_err()
                    .diagnostic(sources)
                    .render_human(sources),
                expected,
                "{name}: {fuel}"
            );
            assert_result(output, b"", expected.as_bytes(), 1);
        } else {
            assert_eq!(actual, Ok(schedule.result));
            assert_result(output, &scalar_output(schedule.result), b"", 0);
        }
    }
    schedule.fuel() + 3
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn native_enums_source_free_all_payloads_orders_sites_loops_and_every_fuel() {
    let scratch = Scratch::new();
    let (sources, s) = fixtures::context();
    let mut processes = 0;
    for (permutation, order) in [[0, 1, 2, 3], [3, 2, 1, 0], [1, 3, 0, 2]]
        .into_iter()
        .enumerate()
    {
        for (constructed, payload) in e::MIXED.into_iter().enumerate() {
            let result = payload.unwrap_or(hir::Ty::I32);
            processes += exercise(
                &scratch,
                &sources,
                &format!("mixed-{permutation}-{constructed}"),
                || e::mixed_case(&e::MIXED, &order, constructed, result, s(0)),
            );
        }
    }
    for (index, payload) in e::MIXED.into_iter().enumerate() {
        let result = payload.unwrap_or(hir::Ty::I32);
        processes += exercise(&scratch, &sources, &format!("singleton-{index}"), || {
            e::mixed_case(&[payload], &[0], 0, result, s(0))
        });
        processes += exercise(&scratch, &sources, &format!("relay-{index}"), || {
            e::relay_case(index, result, s(0))
        });
        processes += exercise(&scratch, &sources, &format!("discard-{index}"), || {
            e::discard_case(payload, s(0))
        });
    }
    for (index, operation) in [
        PhiOperation::Move,
        PhiOperation::Replace,
        PhiOperation::Prepare,
        PhiOperation::Discard,
        PhiOperation::Consume,
    ]
    .into_iter()
    .enumerate()
    {
        for left in [false, true] {
            processes += exercise(&scratch, &sources, &format!("phi-{index}-{left}"), || {
                phi_case(operation, left, s(0))
            });
        }
    }
    for (index, (scalar, value)) in [
        (hir::Ty::Bool, Scalar::Bool(false)),
        (hir::Ty::I32, Scalar::I32(i32::MIN)),
        (hir::Ty::I32, Scalar::I32(i32::MAX)),
        (hir::Ty::I32, Scalar::I32(0)),
    ]
    .into_iter()
    .enumerate()
    {
        processes += exercise(&scratch, &sources, &format!("scalar-edge-{index}"), || {
            let (mut raw, mut schedule) = e::mixed_case(&[Some(scalar)], &[0], 0, scalar, s(0));
            let OwnedInstruction::Scalar(Statement::Assign(assign)) =
                &mut raw.functions[0].blocks[0].statements[1].kind
            else {
                unreachable!("fixture payload assignment")
            };
            assign.value = e::literal(value);
            schedule.result = value;
            (raw, schedule)
        });
    }
    processes += exercise(&scratch, &sources, "loop", || e::loop_case(s(0)));
    eprintln!("native enum core:39 input cases,78 compiled source-free artifacts,39 preserved guarded production modules,{processes} ELF executions including every independent fuel budget");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn native_enums_source_free_invalid_dispatch_consume_and_active_bytes() {
    let scratch = Scratch::new();
    let (sources, s) = fixtures::context();
    let mut artifacts = 0;
    for guarded in [false, true] {
        let (raw, schedule) = e::mixed_case(&e::MIXED, &[0, 2, 3, 1], 1, hir::Ty::Bool, s(0));
        let witness = checked(raw, &sources, guarded);
        let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
        // All four dispatches are reached. A valid but unexpected tag at the
        // final arm must fail as well as the out-of-range tag at every arm.
        for arm in 0..4 {
            for tag in if arm == 3 { vec![-1, 0] } else { vec![-1] } {
                let needle = format!("  %f0_b{arm}_term_tag = load i32, ptr %o0, align 1\n");
                let mutant = before(
                    &module,
                    &needle,
                    &format!("  store i32 {tag}, ptr %o0, align 1\n"),
                );
                let binary = scratch.compile(
                    &mutant,
                    &format!("enum-invalid-dispatch-{guarded}-{arm}-{tag}"),
                );
                assert_result(
                    scratch.run(&binary, &[]),
                    b"",
                    invariant("enum tag", e::at(s(0), 5), &sources).as_bytes(),
                    1,
                );
                artifacts += 1;
            }
        }
        for tag in [-1, 0] {
            let mutant = before(
                &module,
                "  %f0_b7_i0_tag = load i32, ptr %o0, align 1\n",
                &format!("  store i32 {tag}, ptr %o0, align 1\n"),
            );
            let binary = scratch.compile(&mutant, &format!("enum-invalid-consume-{guarded}-{tag}"));
            assert_result(
                scratch.run(&binary, &[]),
                b"",
                invariant("enum tag", e::at(s(0), 5), &sources).as_bytes(),
                1,
            );
            artifacts += 1;
        }
        for payload in [None, Some(hir::Ty::Bool), Some(hir::Ty::Unit)] {
            let result = payload.unwrap_or(hir::Ty::I32);
            let (raw, schedule) = e::mixed_case(&[payload], &[0], 0, result, s(0));
            let witness = checked(raw, &sources, guarded);
            let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
            let corrupt_tag = before(
                &module,
                "  %f0_b0_term_tag = load i32, ptr %o0, align 1\n",
                "  store i32 1, ptr %o0, align 1\n",
            );
            let binary = scratch.compile(
                &corrupt_tag,
                &format!("enum-singleton-tag-{guarded}-{payload:?}"),
            );
            assert_result(
                scratch.run(&binary, &[]),
                b"",
                invariant("enum tag", e::at(s(0), 5), &sources).as_bytes(),
                1,
            );
            artifacts += 1;
            if payload.is_some() {
                let corrupt_payload=before(&module,"  %f0_b1_i0_tag = load i32, ptr %o0, align 1\n","  %test_active = getelementptr i8, ptr %o0, i64 4\n  store i8 2, ptr %test_active, align 1\n");
                let binary = scratch.compile(
                    &corrupt_payload,
                    &format!("enum-singleton-payload-{guarded}-{payload:?}"),
                );
                assert_result(
                    scratch.run(&binary, &[]),
                    b"",
                    invariant("enum payload", e::at(s(0), 5), &sources).as_bytes(),
                    1,
                );
                artifacts += 1;
            }
        }
    }
    assert_eq!(artifacts, 24);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn native_enums_source_free_all_transfer_faults_and_no_partial_write() {
    let scratch = Scratch::new();
    let (sources, s) = fixtures::context();
    let mut artifacts = 0;
    for constructed in [1, 3] {
        for guarded in [false, true] {
            let (raw, schedule) = e::relay_case(constructed, e::MIXED[constructed].unwrap(), s(0));
            let witness = checked(raw, &sources, guarded);
            let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
            let sites: Vec<_> = module
                .lines()
                .filter(|line| line.contains("switch i32"))
                .map(|line| {
                    line.trim()
                        .strip_prefix("switch i32 %")
                        .unwrap()
                        .split_once("_tag,")
                        .unwrap()
                        .0
                        .to_owned()
                })
                .collect();
            assert_eq!(sites.len(), 6);
            for (site, name) in sites.iter().enumerate() {
                let needle = module
                    .lines()
                    .find(|line| line.contains(&format!("%{name}_tag = load i32, ptr")))
                    .unwrap();
                let pointer = needle
                    .split_once("ptr ")
                    .unwrap()
                    .1
                    .split_once(',')
                    .unwrap()
                    .0;
                let span = e::at(s(0), [9, 12, 14, 18, 50, 16][site]);
                let marker = format!("{name}_variant{constructed}_payload_ok:\n");
                let marked = before(&module, &marker, "").replacen(
                    &marker,
                    &format!("{marker}  %test_effect = call i32 @__oxid_print_i32(i32 777)\n"),
                    1,
                );
                let valid = scratch.compile(
                    &marked,
                    &format!("enum-copy-effect-{constructed}-{guarded}-{site}"),
                );
                let mut expected = b"777\n".to_vec();
                expected.extend(scalar_output(schedule.result));
                assert_result(scratch.run(&valid, &[]), &expected, b"", 0);
                artifacts += 1;
                for (kind,insertion) in [("enum tag",format!("  store i32 -1, ptr {pointer}, align 1\n")),("enum payload",format!("  %test_active = getelementptr i8, ptr {pointer}, i64 4\n  store i8 2, ptr %test_active, align 1\n"))] {
                    let mutant=before(&marked,needle,&insertion);
                    let binary=scratch.compile(&mutant,&format!("enum-copy-fault-{constructed}-{guarded}-{site}-{kind}"));
                    assert_result(scratch.run(&binary,&[]),b"",invariant(kind,span,&sources).as_bytes(),1);
                    artifacts+=1;
                }
            }
        }
    }
    assert_eq!(artifacts, 72);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn native_enums_source_free_discard_and_inactive_moved_uninitialized_poison() {
    let scratch = Scratch::new();
    let (sources, s) = fixtures::context();
    let mut artifacts = 0;
    for guarded in [false, true] {
        for (index, payload) in e::MIXED.into_iter().enumerate() {
            let (raw, schedule) = e::discard_case(payload, s(0));
            let witness = checked(raw, &sources, guarded);
            let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
            let discard = module
                .lines()
                .find(|line| line.contains("switch i32"))
                .unwrap()
                .trim()
                .strip_prefix("switch i32 %")
                .unwrap()
                .split_once("_tag,")
                .unwrap()
                .0;
            let needle = format!("  %{discard}_tag = load i32, ptr %o0, align 1\n");
            let corrupt = before(&module, &needle, "  store i32 -1, ptr %o0, align 1\n");
            let binary = scratch.compile(&corrupt, &format!("enum-discard-tag-{guarded}-{index}"));
            assert_result(
                scratch.run(&binary, &[]),
                b"",
                invariant("enum tag", e::at(s(0), 5), &sources).as_bytes(),
                1,
            );
            artifacts += 1;
            if matches!(payload, Some(hir::Ty::Bool | hir::Ty::Unit)) {
                let corrupt=before(&module,&needle,"  %test_active = getelementptr i8, ptr %o0, i64 4\n  store i8 255, ptr %test_active, align 1\n");
                let binary =
                    scratch.compile(&corrupt, &format!("enum-discard-payload-{guarded}-{index}"));
                assert_result(
                    scratch.run(&binary, &[]),
                    b"",
                    invariant("enum payload", e::at(s(0), 5), &sources).as_bytes(),
                    1,
                );
                artifacts += 1;
            }
            let owners_ready = "  %o2 = getelementptr i8, ptr %owners, i64 16\n";
            assert_eq!(module.matches(owners_ready).count(), 1);
            let mut poison = module.replacen(owners_ready, &format!("{owners_ready}  store i64 -1, ptr %o1, align 1\n  store i64 -1, ptr %o2, align 1\n"), 1);
            let marker = format!("{discard}_enum_ok:\n");
            poison = poison.replacen(
                &marker,
                &format!("{marker}  store i64 -1, ptr %o0, align 1\n"),
                1,
            );
            let inactive = match payload {
                None => Some(4),
                Some(hir::Ty::Bool | hir::Ty::Unit) => Some(5),
                Some(hir::Ty::I32) => None,
            };
            if let Some(offset) = inactive {
                poison=before(&poison,&needle,&format!("  %test_inactive = getelementptr i8, ptr %o0, i64 {offset}\n  store i8 255, ptr %test_inactive, align 1\n"));
            }
            let binary =
                scratch.compile(&poison, &format!("enum-teardown-poison-{guarded}-{index}"));
            assert_result(scratch.run(&binary, &[]), b"()\n", b"", 0);
            artifacts += 1;
        }
        // Poison inactive bytes before every copy, then poison the moved
        // initial source. Every subsequent transfer and return must succeed.
        for constructed in [0, 1, 3] {
            let (raw, schedule) = e::relay_case(
                constructed,
                e::MIXED[constructed].unwrap_or(hir::Ty::I32),
                s(0),
            );
            let witness = checked(raw, &sources, guarded);
            let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
            let sites: Vec<_> = module
                .lines()
                .filter(|line| line.contains("switch i32"))
                .map(|line| {
                    line.trim()
                        .strip_prefix("switch i32 %")
                        .unwrap()
                        .split_once("_tag,")
                        .unwrap()
                        .0
                        .to_owned()
                })
                .collect();
            let mut poison = module.clone();
            for (site, name) in sites.iter().enumerate() {
                let needle = module
                    .lines()
                    .find(|line| line.contains(&format!("%{name}_tag = load i32, ptr")))
                    .unwrap();
                let pointer = needle
                    .split_once("ptr ")
                    .unwrap()
                    .1
                    .split_once(',')
                    .unwrap()
                    .0;
                let offset = if constructed == 0 { 4 } else { 5 };
                let mut insertion = String::new();
                for byte in offset..8 {
                    insertion.push_str(&format!("  %test_padding_{site}_{byte} = getelementptr i8, ptr {pointer}, i64 {byte}\n  store i8 255, ptr %test_padding_{site}_{byte}, align 1\n"));
                }
                poison = before(&poison, needle, &insertion);
            }
            let marker = format!("{}_enum_ok:\n", sites[0]);
            poison = poison.replacen(
                &marker,
                &format!("{marker}  store i64 -1, ptr %o0, align 1\n"),
                1,
            );
            let binary = scratch.compile(
                &poison,
                &format!("enum-copy-padding-{guarded}-{constructed}"),
            );
            assert_result(
                scratch.run(&binary, &[]),
                &scalar_output(schedule.result),
                b"",
                0,
            );
            artifacts += 1;
        }
    }
    assert_eq!(artifacts, 26);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn charge_boundary(
    scratch: &Scratch,
    witness: &VerifiedOwnedProgram,
    sources: &SourceMap,
    schedule: &fixtures::Schedule,
    target: (Span, usize),
    marker: &str,
    name: &str,
) {
    let index = schedule
        .events
        .iter()
        .position(|event| *event == target)
        .unwrap();
    let prefix: usize = schedule.events[..index].iter().map(|event| event.1).sum();
    for (fuel, paid) in [(prefix + target.1 - 1, false), (prefix + target.1, true)] {
        let module = native_module_with_fuel(witness, schedule.entry, sources, fuel).unwrap();
        let instrumented = before(
            &module,
            marker,
            "  %test_effect = call i32 @__oxid_print_i32(i32 777)\n",
        );
        let binary = scratch.compile(&instrumented, &format!("enum-charge-{name}-{paid}"));
        let expected = schedule.failure(fuel).unwrap();
        let diagnostic = RunFailure::Fuel(expected)
            .diagnostic(sources)
            .render_human(sources);
        assert_result(
            scratch.run(&binary, &[]),
            if paid { b"777\n" } else { b"" },
            diagnostic.as_bytes(),
            1,
        );
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn native_enums_source_free_guards_precede_every_binding_and_transfer_effect() {
    let scratch = Scratch::new();
    let (sources, s) = fixtures::context();
    let (raw, schedule) = e::mixed_case(&[Some(hir::Ty::Bool)], &[0], 0, hir::Ty::Bool, s(0));
    let witness = checked(raw, &sources, true);
    charge_boundary(
        &scratch,
        &witness,
        &sources,
        &schedule,
        (e::at(s(0), 4), 3),
        "  store i32 0, ptr %o0, align 1\n",
        "constructor",
    );
    charge_boundary(
        &scratch,
        &witness,
        &sources,
        &schedule,
        (e::at(s(0), 5), 3),
        "  store i64 %f0_b1_i0_bind_wide, ptr %s2, align 8\n",
        "consume",
    );
    let (raw, schedule) = e::relay_case(1, hir::Ty::Bool, s(0));
    let witness = checked(raw, &sources, true);
    let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
    let sites: Vec<_> = module
        .lines()
        .filter(|line| line.contains("switch i32"))
        .map(|line| {
            line.trim()
                .strip_prefix("switch i32 %")
                .unwrap()
                .split_once("_tag,")
                .unwrap()
                .0
                .to_owned()
        })
        .collect();
    for (site, name) in sites.iter().enumerate() {
        let marker = format!("  %{name}_variant1_value = trunc i8 %{name}_variant1_active to i1\n");
        // Incoming parameter copying is paid by the Invoke guard in its caller.
        let (span, cost) = [(9, 3), (12, 5), (14, 3), (18, 3), (15, 10), (16, 5)][site];
        charge_boundary(
            &scratch,
            &witness,
            &sources,
            &schedule,
            (e::at(s(0), span), cost),
            &marker,
            &format!("copy-{site}"),
        );
    }
    let (raw, schedule) = e::discard_case(Some(hir::Ty::Bool), s(0));
    let witness = checked(raw, &sources, true);
    let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
    let discard = module
        .lines()
        .find(|line| line.contains("switch i32"))
        .unwrap()
        .trim()
        .strip_prefix("switch i32 %")
        .unwrap()
        .split_once("_tag,")
        .unwrap()
        .0;
    let marker =
        format!("  %{discard}_variant0_value = trunc i8 %{discard}_variant0_active to i1\n");
    charge_boundary(
        &scratch,
        &witness,
        &sources,
        &schedule,
        (e::at(s(0), 5), 3),
        &marker,
        "discard",
    );
}

#[test]
fn native_enums_changed_variants_and_later_owned_input_boundary_modules() {
    let (sources, s) = fixtures::context();
    for (initial, replacement) in e::REPLACEMENT_PAIRS {
        for moved in [false, true] {
            for guarded in [false, true] {
                let (raw, schedule) = e::replacement_case(initial, replacement, moved, s(0));
                let witness = checked(raw, &sources, guarded);
                assert_eq!(
                    execute::run(&witness, Some(schedule.entry)),
                    Ok(schedule.result)
                );
                let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
                assert_eq!(module.contains("%fuel = alloca"), guarded);
                assert!(module.contains(&format!("store i32 {initial}, ptr %o0, align 1")));
                assert!(module.contains(&format!("store i32 {replacement}, ptr %o1, align 1")));
                assert_eq!(module.matches("switch i32").count(), 1 + usize::from(moved));
            }
        }
    }
    for second in [1, 3] {
        for guarded in [false, true] {
            let (raw, schedule) = e::two_owned_case(second, s(0));
            let witness = checked(raw, &sources, guarded);
            assert_eq!(
                execute::run(&witness, Some(schedule.entry)),
                Ok(schedule.result)
            );
            let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
            let first = module.find("%f1_param0_tag = load i32, ptr %arg0").unwrap();
            let first_copied = module.find("f1_param0_enum_ok:\n").unwrap();
            let second = module.find("%f1_param1_tag = load i32, ptr %arg1").unwrap();
            let second_copied = module.find("f1_param1_enum_ok:\n").unwrap();
            let body = module
                .find("%f1_b0_i0_store_wide = zext i32 73 to i64")
                .unwrap();
            assert!(
                first < first_copied
                    && first_copied < second
                    && second < second_copied
                    && second_copied < body
            );
            assert_eq!(module.contains("%fuel = alloca"), guarded);
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn native_enums_source_free_changed_variants_and_later_owned_input_boundary() {
    let scratch = Scratch::new();
    let (sources, s) = fixtures::context();
    let mut processes = 0;
    for (initial, replacement) in e::REPLACEMENT_PAIRS {
        for moved in [false, true] {
            processes += exercise(
                &scratch,
                &sources,
                &format!("boundary-replace-{initial}-{replacement}-{moved}"),
                || e::replacement_case(initial, replacement, moved, s(0)),
            );
        }
    }
    for second in [1, 3] {
        processes += exercise(
            &scratch,
            &sources,
            &format!("boundary-two-owned-{second}"),
            || e::two_owned_case(second, s(0)),
        );
        for guarded in [false, true] {
            let (raw, schedule) = e::two_owned_case(second, s(0));
            let witness = checked(raw, &sources, guarded);
            let module = if guarded {
                native_module_with_fuel(&witness, schedule.entry, &sources, schedule.fuel())
                    .unwrap()
            } else {
                native_module(&witness, Some(schedule.entry), &sources).unwrap()
            };
            // 701 observes a private first-parameter copy, before the child
            // body. 777 observes the first body effect. Neither marker is a
            // language operation; the valid control proves both are reachable.
            let copied = "f1_param0_enum_ok:\n";
            let marked = before(&module, copied, "").replacen(
                copied,
                &format!("{copied}  %test_private_copy = call i32 @__oxid_print_i32(i32 701)\n"),
                1,
            );
            let marked = before(
                &marked,
                "  %f1_b0_i0_store_wide = zext i32 73 to i64\n",
                "  %test_body_effect = call i32 @__oxid_print_i32(i32 777)\n",
            );
            let binary = scratch.compile(
                &marked,
                &format!("enum-boundary-two-owned-markers-{second}-{guarded}"),
            );
            assert_result(scratch.run(&binary, &[]), b"701\n777\n73\n", b"", 0);
            processes += 1;
            let invoke_line = module
                .lines()
                .find(|line| line.contains("%f0_b0_term_result = call i32 @__oxid_owned_fn_1("))
                .unwrap();
            let invoke = schedule
                .events
                .iter()
                .position(|event| *event == (e::at(s(0), 10), 20))
                .unwrap();
            let prefix: usize = schedule.events[..invoke].iter().map(|event| event.1).sum();
            for (kind, insertion) in [
                ("enum tag", "  store i32 -1, ptr %o3, align 1\n"),
                ("enum payload", "  %test_second_active = getelementptr i8, ptr %o3, i64 4\n  store i8 255, ptr %test_second_active, align 1\n"),
            ] {
                let mutant = before(&marked, invoke_line, insertion);
                if guarded {
                    let harness = argv_fuel_harness(&mutant, schedule.fuel());
                    let binary = scratch.compile(&harness, &format!("enum-boundary-later-input-{second}-{guarded}-{kind}"));
                    for fuel in [prefix + 19, prefix + 20, schedule.fuel()] {
                        if fuel == prefix + 19 {
                            // Invoke must be paid before mutation/callee work;
                            // its fuel diagnostic belongs to the caller site.
                            let diagnostic = RunFailure::Fuel(e::at(s(0), 10)).diagnostic(&sources).render_human(&sources);
                            assert_result(scratch.run(&binary, &[fuel.to_string()]), b"", diagnostic.as_bytes(), 1);
                        } else {
                            // A later input may fail after a private copy, but
                            // the body marker must never execute. Incoming
                            // invariant diagnostics keep the callee origin.
                            assert_result(scratch.run(&binary, &[fuel.to_string()]), b"701\n", invariant(kind, e::at(s(0), 30), &sources).as_bytes(), 1);
                        }
                        processes += 1;
                    }
                } else {
                    let binary = scratch.compile(&mutant, &format!("enum-boundary-later-input-{second}-{guarded}-{kind}"));
                    assert_result(scratch.run(&binary, &[]), b"701\n", invariant(kind, e::at(s(0), 30), &sources).as_bytes(), 1);
                    processes += 1;
                }
            }
        }
    }
    eprintln!("native enum boundaries:8 changed-variant replacement cases,2 two-owned-input cases,32 compiled source-free artifacts,{processes} ELF executions including independent every-fuel schedules and later-input fault boundaries");
}
