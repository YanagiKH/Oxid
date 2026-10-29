use super::*;
use crate::frontend::oir::owned::owned_vector_descriptors::tests as fixture;
use crate::frontend::oir::owned::verified::VerifiedOwnedVectorProgram;

fn new_and_len() -> (SourceMap, VerifiedOwnedVectorProgram) {
    let (sources, mut raw, keys, imports) = fixture::fixture(None);
    let span = raw.records[0].span;
    let vector = AggregateTy::OwnedRecordVector(RecordId(0));
    let mut main = consumer_fixtures::function(0, ValueTy::Scalar(hir::Ty::I32), span);
    main.locals
        .push(consumer_fixtures::scalar(hir::Ty::I32, span));
    main.owners = vec![
        OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(vector).unwrap(),
            kind: OwnerKind::CallResult {
                call: CallSiteId(0),
            },
            span,
        },
        OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(vector).unwrap(),
            kind: OwnerKind::Local { mutable: false },
            span,
        },
    ];
    main.loans.push(LoanDecl {
        projection: Vec::new(),
        region: LoanRegion::Call {
            call: CallSiteId(1),
            argument: 0,
        },
        authority: AccessBase::Owner(OwnerPlaceId(1)),
        kind: BorrowKind::Shared,
        referent: BorrowedSlot::check(BorrowedTy::Exact(vector)).unwrap(),
        span,
    });
    main.calls = vec![
        CallDecl {
            target: hir::DefId(2),
            arguments: Vec::new(),
            result: CallResult::Owned(OwnerPlaceId(0)),
            parent: None,
            span,
        },
        CallDecl {
            target: hir::DefId(3),
            arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
            result: CallResult::Scalar(LocalId(0)),
            parent: None,
            span,
        },
    ];
    let statement = |kind| consumer_fixtures::instruction(kind, span);
    let block = |statements, terminator| OwnedBlock {
        merge: None,
        span,
        statements,
        terminator: consumer_fixtures::end(terminator, span),
    };
    main.blocks = vec![
        block(
            vec![statement(OwnedInstruction::OpenCall(CallSiteId(0)))],
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
        ),
        block(
            vec![
                statement(OwnedInstruction::StorageLive(OwnerPlaceId(1))),
                statement(OwnedInstruction::MoveInitialize {
                    destination: OwnerPlaceId(1),
                    source: OwnerPlaceId(0),
                }),
                statement(OwnedInstruction::OpenCall(CallSiteId(1))),
                statement(OwnedInstruction::PrepareBorrow {
                    call: CallSiteId(1),
                    argument: 0,
                    loan: LoanId(0),
                }),
            ],
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(1),
                continuation: BlockId(2),
            },
        ),
        block(
            Vec::new(),
            OwnedTerminatorKind::ReturnScalar(consumer_fixtures::operand(0, span)),
        ),
    ];
    for function in &mut raw.functions {
        function.id.0 += 1;
    }
    raw.functions.insert(0, main);
    let wrapper = fixture::prepare(&sources, raw, &keys, imports);
    let witness = VerifiedOwnedVectorProgram::verify(
        wrapper,
        &sources,
        budget::Limits::DEFAULT,
        &mut crate::frontend::project::budget::Allocator::default(),
    )
    .unwrap();
    (sources, witness)
}

#[test]
fn owned_vector_native_real_seal_emits_all_primitives_and_exact_ir_limits() {
    let (sources, witness) = new_and_len();
    let module = native_module(&witness, Some(hir::DefId(0)), &sources).unwrap();
    for operation in [
        "new",
        "len",
        "capacity",
        "reserve",
        "push_reserved",
        "pop_last",
        "truncate",
        "take",
    ] {
        assert!(
            module.contains(&format!("call i32 @oxid_owned_vector_{operation}(")),
            "{operation}"
        );
    }
    assert!(module.contains("@__oxid_owned_records = constant"));
    assert!(module.contains("call i32 @oxid_owned_vector_validate("));
    assert!(module.contains("call i32 @oxid_owned_vector_release("));
    for (limit, succeeds) in [(module.len(), true), (module.len() - 1, false)] {
        let result = native_module_limits(
            &witness,
            Some(hir::DefId(0)),
            &sources,
            plan::MAX_FUEL,
            Limits {
                ir_bytes: limit,
                ..Limits::DEFAULT
            },
        );
        if succeeds {
            assert_eq!(result.unwrap(), module);
        } else {
            assert!(result.unwrap_err().message.contains("LLVM bytes"));
        }
    }
}

#[test]
fn owned_vector_native_real_call_move_query_and_owner_cleanup_run() {
    if std::env::var_os("OXID_TEST_LLVM_BIN").is_none() {
        return;
    }
    let (sources, witness) = new_and_len();
    for policy in [
        crate::frontend::build_config::ExecutionPolicy::Bounded,
        crate::frontend::build_config::ExecutionPolicy::Hosted,
    ] {
        let module = native_module_with_execution(
            &witness,
            Some(hir::DefId(0)),
            &sources,
            false,
            policy,
            None,
        )
        .unwrap();
        let scratch = tests::Scratch::new();
        let binary = scratch.compile(&module, "owned-vector-new-move-len");
        let output = scratch.run(&binary, &[]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"0\n");
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn owned_vector_native_all_eight_calls_execute_under_both_policies_and_optimizations() {
    if std::env::var_os("OXID_TEST_LLVM_BIN").is_none() {
        return;
    }
    let (sources, witness, _, expected) = fixture::execution_fixture::all_operations();
    assert_eq!(expected, 58);
    for policy in [
        crate::frontend::build_config::ExecutionPolicy::Bounded,
        crate::frontend::build_config::ExecutionPolicy::Hosted,
    ] {
        for reachable_only in [false, true] {
            let module = if reachable_only {
                native_optimized_module_with_execution(
                    &witness,
                    Some(hir::DefId(0)),
                    &sources,
                    false,
                    policy,
                    None,
                    false,
                )
            } else {
                native_module_with_execution(
                    &witness,
                    Some(hir::DefId(0)),
                    &sources,
                    false,
                    policy,
                    None,
                )
            }
            .unwrap();
            assert_eq!(
                module.contains("define internal void @__oxid_owned_fn_1("),
                !reachable_only,
                "the unused scalar-vector primitive follows release selection"
            );
            let scratch = tests::Scratch::new();
            let debug = scratch.compile(&module, "owned-vector-all-eight-O0");
            let optimized = debug.with_file_name("owned-vector-all-eight-O2");
            let settings = crate::frontend::build_config::NativeBuildSettings::with_optimization(
                false,
                crate::frontend::build_config::NativeOptimization::O2,
            )
            .with_execution(policy);
            crate::frontend::native::compile_with_settings(
                &module,
                optimized.to_str().unwrap(),
                &settings,
            )
            .unwrap();
            for binary in [&debug, &optimized] {
                let output = scratch.run(binary, &[]);
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert_eq!(output.stdout, b"58\n");
                assert!(output.stderr.is_empty());
            }
        }
    }
}

#[test]
fn owned_vector_native_metadata_and_every_emitter_allocation_refusal_preserve_the_owner() {
    let (sources, witness, _, _) = fixture::execution_fixture::all_operations();
    let mut baseline = Accounting::default();
    let module = native_module_accounted(
        &witness,
        Some(hir::DefId(0)),
        &sources,
        plan::MAX_FUEL,
        Limits::DEFAULT,
        &mut baseline,
    )
    .unwrap();
    let metadata = baseline.metrics.metadata_peak;
    assert!(metadata > baseline.metrics.plan_bytes);
    assert!(
        baseline.metrics.emitter_transient_bound
            >= EMITTER_TRANSIENT_BYTES + owned_vector::control_bytes()
    );
    for (limit, succeeds) in [(metadata, true), (metadata - 1, false)] {
        let result = native_module_limits(
            &witness,
            Some(hir::DefId(0)),
            &sources,
            plan::MAX_FUEL,
            Limits {
                metadata_bytes: limit,
                ..Limits::DEFAULT
            },
        );
        if succeeds {
            assert_eq!(result.unwrap(), module);
        } else {
            assert!(result.unwrap_err().message.contains("metadata"));
        }
    }
    assert!(baseline.metrics.allocation_attempts > 0);
    for ordinal in 0..baseline.metrics.allocation_attempts {
        let mut failing = Accounting {
            metrics: NativeMetrics::default(),
            fail_after: Some(ordinal),
        };
        let error = native_module_accounted(
            &witness,
            Some(hir::DefId(0)),
            &sources,
            plan::MAX_FUEL,
            Limits::DEFAULT,
            &mut failing,
        )
        .unwrap_err();
        assert_eq!(error.code, "E0700");
        assert!(
            error.message.contains("allocation"),
            "{ordinal}: {}",
            error.message
        );
        assert_eq!(failing.metrics.allocation_attempts, ordinal + 1);
        assert!(witness.elements().belongs_to(witness.declarations()));
        assert_eq!(witness.elements().element(RecordId(0)).unwrap().size(), 4);
    }
    assert_eq!(
        native_module(&witness, Some(hir::DefId(0)), &sources).unwrap(),
        module
    );
}

#[test]
fn owned_vector_native_debug_records_borrow_the_exact_new_witness_declarations() {
    let (_, witness, _, _) = fixture::execution_fixture::all_operations();
    let records = debug::DebugRecords::new(&witness);
    assert_eq!(records.len(), 2);
    assert_eq!(records.record(0).unwrap().size(), 4);
    assert_eq!(records.record(1).unwrap().size(), 4);
    assert!(records.record(2).is_none());
}

#[test]
fn owned_vector_native_matches_reference_whole_program_fuel_at_both_boundaries() {
    if std::env::var_os("OXID_TEST_LLVM_BIN").is_none() {
        return;
    }
    let (sources, witness, _, expected) = fixture::execution_fixture::all_operations();
    let limits = execute::Limits::default();
    let (value, remaining) = execute::observe_witness(
        &witness,
        Some(hir::DefId(0)),
        limits,
        crate::frontend::build_config::ExecutionPolicy::Bounded,
    );
    assert_eq!(value, Ok(Scalar::I32(expected)));
    let exact = limits.fuel - remaining;
    assert!(exact > 0);
    for (fuel, succeeds) in [(exact, true), (exact - 1, false)] {
        let (reference, _) = execute::observe_witness(
            &witness,
            Some(hir::DefId(0)),
            execute::Limits { fuel, ..limits },
            crate::frontend::build_config::ExecutionPolicy::Bounded,
        );
        assert_eq!(reference.is_ok(), succeeds);
        let module = native_module_with_fuel(&witness, hir::DefId(0), &sources, fuel).unwrap();
        let scratch = tests::Scratch::new();
        let debug = scratch.compile(&module, &format!("owned-vector-fuel-{fuel}-O0"));
        let optimized = debug.with_file_name(format!("owned-vector-fuel-{fuel}-O2"));
        let settings = crate::frontend::build_config::NativeBuildSettings::with_optimization(
            false,
            crate::frontend::build_config::NativeOptimization::O2,
        );
        crate::frontend::native::compile_with_settings(
            &module,
            optimized.to_str().unwrap(),
            &settings,
        )
        .unwrap();
        for binary in [&debug, &optimized] {
            let output = scratch.run(binary, &[]);
            assert_eq!(
                output.status.success(),
                succeeds,
                "fuel{fuel}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            if succeeds {
                assert_eq!(output.stdout, b"58\n");
                assert!(output.stderr.is_empty());
            } else {
                assert!(output.stdout.is_empty());
                assert!(String::from_utf8_lossy(&output.stderr).contains("fuel"));
            }
        }
    }
}

#[test]
fn owned_vector_native_live_buffers_and_active_aligned_enums_survive_all_eight_calls() {
    if std::env::var_os("OXID_TEST_LLVM_BIN").is_none() {
        return;
    }
    for (enumerated, live_take) in [(false, false), (true, false), (false, true)] {
        let (sources, witness, _, expected) = if live_take {
            fixture::execution_fixture::resources::buffers_with_live_take()
        } else if enumerated {
            fixture::execution_fixture::resources::enum_buffers()
        } else {
            fixture::execution_fixture::resources::buffers()
        };
        assert_eq!(expected, 58);
        for policy in [
            crate::frontend::build_config::ExecutionPolicy::Bounded,
            crate::frontend::build_config::ExecutionPolicy::Hosted,
        ] {
            for reachable in [false, true] {
                let module = if reachable {
                    native_optimized_module_with_execution(
                        &witness,
                        Some(hir::DefId(0)),
                        &sources,
                        false,
                        policy,
                        None,
                        false,
                    )
                } else {
                    native_module_with_execution(
                        &witness,
                        Some(hir::DefId(0)),
                        &sources,
                        false,
                        policy,
                        None,
                    )
                }
                .unwrap();
                let scratch = tests::Scratch::new();
                let debug = scratch.compile(&module, "owned-vector-live-children-O0");
                let optimized = debug.with_file_name("owned-vector-live-children-O2");
                let settings =
                    crate::frontend::build_config::NativeBuildSettings::with_optimization(
                        false,
                        crate::frontend::build_config::NativeOptimization::O2,
                    )
                    .with_execution(policy);
                crate::frontend::native::compile_with_settings(
                    &module,
                    optimized.to_str().unwrap(),
                    &settings,
                )
                .unwrap();
                for binary in [&debug, &optimized] {
                    let output = scratch.run(binary, &[]);
                    assert!(
                        output.status.success(),
                        "enum{enumerated}, live take{live_take}: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    assert_eq!(output.stdout, b"58\n");
                    assert!(output.stderr.is_empty());
                }
            }
        }
    }
}

#[test]
fn owned_vector_native_file_callers_release_success_and_error_paths() {
    if std::env::var_os("OXID_TEST_LLVM_BIN").is_none() {
        return;
    }
    for (missing, live_take) in [(false, false), (true, false), (false, true)] {
        let scratch = tests::Scratch::new();
        let first = scratch.path(if missing { "missing/file" } else { "popped" });
        let second = scratch.path("truncated");
        let paths = [
            first.as_os_str().as_encoded_bytes(),
            second.as_os_str().as_encoded_bytes(),
        ];
        let (sources, witness, _, _) = if live_take {
            fixture::execution_fixture::files::files_with_live_take(paths)
        } else {
            fixture::execution_fixture::files::files(paths)
        };
        for policy in [
            crate::frontend::build_config::ExecutionPolicy::Bounded,
            crate::frontend::build_config::ExecutionPolicy::Hosted,
        ] {
            for reachable in [false, true] {
                let module = if reachable {
                    native_optimized_module_with_execution(
                        &witness,
                        Some(hir::DefId(0)),
                        &sources,
                        false,
                        policy,
                        None,
                        false,
                    )
                } else {
                    native_module_with_execution(
                        &witness,
                        Some(hir::DefId(0)),
                        &sources,
                        false,
                        policy,
                        None,
                    )
                }
                .unwrap();
                // The production entry rejects live resource nodes before its
                // terminal cleanup, so success cannot conceal a leaked File.
                assert!(module.contains("%resource_status = call i32 @__oxid_bytebuf_finish()"));
                let name = format!("owned-vector-files-{policy:?}-reachable{reachable}");
                let debug = scratch.compile(&module, &format!("{name}-O0"));
                let optimized = scratch.path(&format!("{name}-O2"));
                let settings =
                    crate::frontend::build_config::NativeBuildSettings::with_optimization(
                        false,
                        crate::frontend::build_config::NativeOptimization::O2,
                    )
                    .with_execution(policy);
                crate::frontend::native::compile_with_settings(
                    &module,
                    optimized.to_str().unwrap(),
                    &settings,
                )
                .unwrap();
                for binary in [&debug, &optimized] {
                    let output = scratch.run(binary, &[]);
                    assert!(
                        output.status.success(),
                        "missing{missing}, live take{live_take}: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    assert_eq!(
                        output.stdout,
                        if missing { &b"-19\n"[..] } else { &b"58\n"[..] }
                    );
                    assert!(output.stderr.is_empty());
                    if missing {
                        assert!(!first.exists() && !second.exists());
                    } else {
                        assert!(first.is_file() && second.is_file());
                        std::fs::write(&first, b"reopened popped file").unwrap();
                        std::fs::write(&second, b"reopened truncated file").unwrap();
                    }
                }
            }
        }
    }
}
