//! Enum-vector admission and cleanup boundaries through real source programs.
use super::*;
use crate::frontend::{
    build_config::{NativeBuildSettings, NativeOptimization},
    oir::owned::{
        budget as proof_budget, byte_buffer,
        source::{association, budget as source_budget},
    },
};

fn with_case(body: &str, test: impl FnOnce(&typeck::TypedOwnedProgram<'_>, &SourceMap, &Fixture)) {
    let fixture = Fixture::new(&format!("{}{}", cases::API, body));
    let project = ProjectSources::load_typed(
        fixture.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default(),
    )
    .unwrap();
    let index = index::collect_owned_vector_candidate(
        SourceOwner::project(&project),
        IndexLimits::default(),
        8 * 1024 * 1024,
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    let typing_work = WorkMeter::default();
    let typed =
        resolve::type_owned_vector_source(&index, &typing_work, &mut Allocator::default()).unwrap();
    test(&typed, project.sources(), &fixture);
}

#[test]
fn owned_enum_vector_record_field_replacement_and_destructuring_release_before_early_return() {
    with_case(cases::RECORD_FIELD_LIFETIME, |typed, _, _| {
        let associated = association::lower_and_associate_owned_vector(typed).unwrap();
        let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            proof_budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let ((result, _), observed) = byte_buffer::testing::capture(Default::default(), || {
                execute::observe_witness(
                    &witness,
                    typed.entry(),
                    execute::Limits::default(),
                    policy,
                )
            });
            assert_eq!(result, Ok(Scalar::I32(42)));
            // Each enum cell is32 bytes; eight cells and eight identity slots
            // plus the64-byte header are384. Each Holder also owns two40-byte
            // byte buffers. Replacing the first Holder retains both until the
            // incoming complete value is admitted:2 * (384 + 40 + 40) =928.
            assert_eq!(observed.before_cleanup.peak_bytes, 928);
            assert_eq!(observed.before_cleanup.allocations, 6);
            assert_eq!(observed.before_cleanup.releases, 6);
            assert_eq!(observed.before_cleanup.live_allocations, 0);
            assert_eq!(observed.before_cleanup, observed.after_cleanup);
        }
    });
}

#[test]
fn owned_enum_vector_joint_record_enum_fields_keep_layout_and_cleanup_admission() {
    with_case(cases::JOINT_RECORD_ENUM_FIELDS, |typed, _, _| {
        let associated = association::lower_and_associate_owned_vector(typed).unwrap();
        let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            proof_budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let ((result, _), observed) = byte_buffer::testing::capture(Default::default(), || {
                execute::observe_witness(
                    &witness,
                    typed.entry(),
                    execute::Limits::default(),
                    policy,
                )
            });
            assert_eq!(result, Ok(Scalar::I32(42)));
            assert_eq!(observed.before_cleanup.allocations, 1);
            assert_eq!(observed.before_cleanup.releases, 1);
            assert_eq!(observed.before_cleanup.live_allocations, 0);
            assert_eq!(observed.before_cleanup, observed.after_cleanup);
        }
    });
}

#[test]
fn owned_enum_vector_source_memory_limit_preserves_the_vector_and_independent_incoming_enum() {
    with_case(cases::MEMORY_LIMIT, |typed, _, _| {
        let associated = association::lower_and_associate_owned_vector(typed).unwrap();
        let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            proof_budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let ((result, _), observed) = byte_buffer::testing::capture(
                byte_buffer::testing::Control {
                    limit: Some(1024),
                    ..Default::default()
                },
                || {
                    execute::observe_witness(
                        &witness,
                        typed.entry(),
                        execute::Limits::default(),
                        policy,
                    )
                },
            );
            assert_eq!(result, Ok(Scalar::I32(42)));
            //24-byte enum cells at capacity8, eight identity slots and a64-byte
            // header:320. Two live40-byte child buffers give400; the rejected
            // 128-cell replacement requests4160 and must not be allocated.
            assert_eq!(observed.before_cleanup.peak_bytes, 400);
            assert_eq!(observed.before_cleanup.allocations, 3);
            assert_eq!(observed.before_cleanup.releases, 3);
            assert_eq!(observed.before_cleanup.live_allocations, 0);
            assert_eq!(observed.before_cleanup, observed.after_cleanup);
        }
    });
}

#[test]
fn owned_enum_vector_sparse_builtin_inventory_uses_active_rank_for_the_last_catalog_enum() {
    with_case(cases::SPARSE_ARGUMENT_STATUS, |typed, _, _| {
        let index = typed.index();
        assert_eq!(index.source_enum_count(), 2);
        assert_eq!(index.builtin_set().extra_enums(), 2);
        let argument = index
            .experimental_enum_id(
                crate::frontend::builtin_catalog::experimental::Enumeration::Arguments,
            )
            .unwrap();
        let grow = index
            .experimental_enum_id(crate::frontend::builtin_catalog::experimental::Enumeration::Grow)
            .unwrap();
        assert_eq!((grow.0, argument.0), (2, 3));
        let associated = association::lower_and_associate_owned_vector(typed).unwrap();
        let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            proof_budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        let layout = witness
            .declarations()
            .aggregate_layout(AggregateTy::Enum(argument))
            .unwrap();
        assert_eq!((layout.size(), layout.align()), (8, 4));
        assert_eq!(
            witness
                .declarations()
                .enums()
                .variants(argument)
                .unwrap()
                .len(),
            4
        );
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let ((result, _), observed) = byte_buffer::testing::capture(Default::default(), || {
                execute::observe_witness(
                    &witness,
                    typed.entry(),
                    execute::Limits::default(),
                    policy,
                )
            });
            assert_eq!(result, Ok(Scalar::I32(42)));
            assert_eq!(observed.before_cleanup.live_allocations, 0);
            assert_eq!(
                observed.before_cleanup.allocations,
                observed.before_cleanup.releases
            );
            assert_eq!(observed.before_cleanup, observed.after_cleanup);
        }
    });
}

#[test]
fn owned_enum_vector_source_raw_and_full_proof_budgets_keep_exact_and_one_less_boundaries() {
    with_case(cases::INTERLEAVED_BUILTINS, |typed, _, _| {
        let source_usage = source_budget::preflight(typed, source_budget::Limits::DEFAULT).unwrap();
        assert!(source_usage.raw_bytes > 0);
        source_budget::reset_guard_counts();
        let short = lower::lower_with_limits(
            typed,
            source_budget::Limits {
                raw_bytes: source_usage.raw_bytes - 1,
            },
        )
        .unwrap_err();
        assert!(
            matches!(short.kind, OwnedFailureKind::Resource(_)),
            "{short:?}"
        );
        assert_eq!(
            source_budget::guard_counts()[source_budget::GuardEvent::RawReservation as usize],
            0
        );
        lower::lower_with_limits(
            typed,
            source_budget::Limits {
                raw_bytes: source_usage.raw_bytes,
            },
        )
        .unwrap();
        assert!(
            source_budget::guard_counts()[source_budget::GuardEvent::RawReservation as usize] > 0
        );

        let associated = association::lower_and_associate_owned_vector(typed).unwrap();
        let baseline = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            proof_budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        let usage = baseline.usage();
        for boundary in 0..3 {
            let mut limits = proof_budget::Limits {
                metadata: usage.metadata_bytes,
                work: usage.work,
                ..proof_budget::Limits::DEFAULT
            };
            if boundary == 1 {
                limits.metadata -= 1;
            }
            if boundary == 2 {
                limits.work -= 1;
            }
            let associated = association::lower_and_associate_owned_vector(typed).unwrap();
            let result = verified::VerifiedOwnedVectorProgram::verify_associated(
                associated,
                limits,
                &mut Allocator::default(),
            );
            if boundary == 0 {
                let witness = result.expect("exact enum-vector proof allowance");
                assert_eq!(witness.usage().metadata_bytes, usage.metadata_bytes);
                assert_eq!(witness.usage().work, usage.work);
            } else {
                assert!(matches!(
                    result,
                    Err(OwnedFailure {
                        kind: OwnedFailureKind::Resource(_),
                        ..
                    })
                ));
            }
        }
    });
}

#[test]
fn owned_enum_vector_native_exact_fuel_and_early_return_cleanup_match_the_reference() {
    if std::env::var_os("OXID_TEST_LLVM_BIN").is_none() {
        return;
    }
    with_case(cases::RECORD_FIELD_LIFETIME, |typed, sources, fixture| {
        let associated = association::lower_and_associate_owned_vector(typed).unwrap();
        let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            proof_budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        let limits = execute::Limits::default();
        let (result, remaining) =
            execute::observe_witness(&witness, typed.entry(), limits, ExecutionPolicy::Bounded);
        assert_eq!(result, Ok(Scalar::I32(42)));
        let exact = limits.fuel - remaining;
        assert!(exact > 0);
        let mut outputs = Vec::new();
        for (fuel, succeeds) in [(exact, true), (exact - 1, false)] {
            let ((result, _), observed) = byte_buffer::testing::capture(Default::default(), || {
                execute::observe_witness(
                    &witness,
                    typed.entry(),
                    execute::Limits { fuel, ..limits },
                    ExecutionPolicy::Bounded,
                )
            });
            assert_eq!(result.is_ok(), succeeds);
            assert_eq!(observed.after_cleanup.live_allocations, 0);
            assert_eq!(
                observed.after_cleanup.allocations,
                observed.after_cleanup.releases
            );
            let module =
                native::native_module_with_fuel(&witness, typed.entry().unwrap(), sources, fuel)
                    .unwrap();
            for optimization in [NativeOptimization::O0, NativeOptimization::O2] {
                let output = fixture.0.join(format!("enum-fuel-{fuel}-{optimization:?}"));
                crate::frontend::native::compile_with_settings(
                    &module,
                    output.to_str().unwrap(),
                    &NativeBuildSettings::with_optimization(false, optimization),
                )
                .unwrap();
                outputs.push((output, succeeds));
            }
        }
        std::fs::remove_file(fixture.0.join("main.ox")).unwrap();
        for (output, succeeds) in outputs {
            let observed = std::process::Command::new(&output)
                .current_dir(&fixture.0)
                .env_clear()
                .output()
                .unwrap();
            assert_eq!(observed.status.success(), succeeds, "{observed:?}");
            if succeeds {
                assert_eq!(observed.stdout, b"42\n");
                assert!(observed.stderr.is_empty(), "{observed:?}");
            } else {
                assert!(observed.stdout.is_empty(), "{observed:?}");
                assert!(
                    String::from_utf8_lossy(&observed.stderr).contains("fuel"),
                    "{observed:?}"
                );
            }
        }
    });
}
