use super::*;
use crate::frontend::builtin_catalog::owned_vector::parameter_cases as cases;
use crate::frontend::{
    build_config::ExecutionPolicy,
    project::{ProjectLimits, ProjectSources},
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "oxid-source-vector-wrappers-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), format!("mod owned_vec;{source}")).unwrap();
        fs::write(root.join("owned_vec.ox"), cases::LIBRARY).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() && std::env::var_os("OXID_KEEP_TEST_PROJECTS").is_none() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn with_typed(source: &str, check: impl FnOnce(&typeck::TypedOwnedProgram<'_>)) {
    let fixture = Fixture::new(source);
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
    // Check the actual immutable module bytes, using its canonical module
    // identity rather than a host-specific display spelling of SourceFile.path.
    let path = fixture.0.join("owned_vec.ox").canonicalize().unwrap();
    let mut modules = project
        .modules()
        .iter()
        .filter(|module| module.canonical_path.as_ref() == Some(&path));
    let module = modules.next().unwrap();
    assert!(modules.next().is_none());
    assert_eq!(module.relative_path, "owned_vec.ox");
    assert_eq!(project.sources().get(module.file).text(), cases::LIBRARY);
    let work = WorkMeter::default();
    let typed =
        resolve::type_owned_vector_source(&index, &work, &mut Allocator::default()).unwrap();
    check(&typed);
}

#[test]
fn source_owned_parameter_results_use_finite_roots_and_exact_owner_cleanup() {
    with_typed(cases::EXAMPLE, |typed| {
        let associated =
            super::super::association::lower_and_associate_owned_vector(typed).unwrap();
        let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            super::super::super::budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let ((result, _), observed) =
                super::super::super::byte_buffer::testing::capture(Default::default(), || {
                    execute::observe_witness(
                        &witness,
                        typed.entry(),
                        execute::Limits::default(),
                        policy,
                    )
                });
            assert_eq!(result, Ok(Scalar::I32(42)));
            // One independent byte buffer and three concrete vector roots.
            assert_eq!(observed.before_cleanup.allocations, 4);
            assert_eq!(observed.before_cleanup.releases, 4);
            assert_eq!(observed.before_cleanup.live_allocations, 0);
            assert_eq!(observed.before_cleanup, observed.after_cleanup);
        }
    });
}

#[test]
fn source_function_vector_owners_transfer_detach_and_drop_without_new_allocations() {
    with_typed(cases::OWNERSHIP, |typed| {
        let associated =
            super::super::association::lower_and_associate_owned_vector(typed).unwrap();
        let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            super::super::super::budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let ((result, _), observed) =
                super::super::super::byte_buffer::testing::capture(Default::default(), || {
                    execute::observe_witness(
                        &witness,
                        typed.entry(),
                        execute::Limits::default(),
                        policy,
                    )
                });
            assert_eq!(result, Ok(Scalar::I32(42)));
            // Two separately allocated buffers and five vector allocations:
            // Message, Token, GrowStatus, i32 and checked Copy Point. Empty
            // vector construction, whole moves and detach allocate nothing.
            assert_eq!(observed.before_cleanup.allocations, 7);
            assert_eq!(observed.before_cleanup.releases, 7);
            assert_eq!(observed.before_cleanup.live_allocations, 0);
            assert_eq!(observed.before_cleanup.live_bytes, 0);
            assert_eq!(observed.before_cleanup, observed.after_cleanup);
        }
    });
}

#[test]
fn source_function_vector_owners_clean_up_every_fallible_allocation_and_fuel_exit() {
    use super::super::super::byte_buffer;
    with_typed(cases::OWNERSHIP, |typed| {
        let associated =
            super::super::association::lower_and_associate_owned_vector(typed).unwrap();
        let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            super::super::super::budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            for skip in 0..7 {
                let control = byte_buffer::testing::Control {
                    fault: Some((byte_buffer::Boundary::Allocate, skip)),
                    ..Default::default()
                };
                let ((result, _), observed) = byte_buffer::testing::capture(control, || {
                    execute::observe_witness(
                        &witness,
                        typed.entry(),
                        execute::Limits::default(),
                        policy,
                    )
                });
                assert!(observed.fault_applied, "{policy:?}: {skip}");
                assert_ne!(result, Ok(Scalar::I32(42)), "{policy:?}: {skip}");
                assert_eq!(observed.after_cleanup.live_bytes, 0);
                assert_eq!(observed.after_cleanup.live_allocations, 0);
                assert_eq!(
                    observed.after_cleanup.allocations,
                    observed.after_cleanup.releases
                );
            }
        }
        let limits = execute::Limits::default();
        let ((result, remaining), _) = byte_buffer::testing::capture(Default::default(), || {
            execute::observe_witness(&witness, typed.entry(), limits, ExecutionPolicy::Bounded)
        });
        assert_eq!(result, Ok(Scalar::I32(42)));
        let consumed = limits.fuel - remaining;
        for (fuel, succeeds) in [
            (consumed, true),
            (consumed - 1, false),
            (consumed / 2, false),
        ] {
            let ((result, _), observed) = byte_buffer::testing::capture(Default::default(), || {
                execute::observe_witness(
                    &witness,
                    typed.entry(),
                    execute::Limits { fuel, ..limits },
                    ExecutionPolicy::Bounded,
                )
            });
            assert_eq!(result.is_ok(), succeeds);
            assert_eq!(observed.after_cleanup.live_bytes, 0);
            assert_eq!(observed.after_cleanup.live_allocations, 0);
            assert_eq!(
                observed.after_cleanup.allocations,
                observed.after_cleanup.releases
            );
        }
    });
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn source_function_vector_discard_closes_the_remaining_file_owner_once() {
    let scope = Fixture::new("");
    let path = std::fs::canonicalize(&scope.0).unwrap().join("owned-file");
    let source = cases::FILE_OWNERSHIP.replace(
        "__OWNED_VECTOR_FILE_PATH__",
        &format!("{:?}", path.to_str().unwrap()),
    );
    with_typed(&source, |typed| {
        let associated =
            super::super::association::lower_and_associate_owned_vector(typed).unwrap();
        let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            super::super::super::budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let ((result, _), observed) =
                super::super::super::byte_buffer::testing::capture(Default::default(), || {
                    execute::observe_witness(
                        &witness,
                        typed.entry(),
                        execute::Limits::default(),
                        policy,
                    )
                });
            assert_eq!(result, Ok(Scalar::I32(42)));
            // One registered file and one vector; no allocation for either
            // whole-owner call or detaching its existing initialized prefix.
            assert_eq!(observed.before_cleanup.allocations, 2);
            assert_eq!(observed.before_cleanup.releases, 2);
            assert_eq!(observed.before_cleanup.live_allocations, 0);
            assert_eq!(observed.before_cleanup, observed.after_cleanup);
            assert!(path.is_file());
            assert!(std::fs::read(&path).unwrap().is_empty());
        }
    });
}

#[test]
fn source_owned_parameter_failed_push_returns_incoming_owner_and_unchanged_vector() {
    use super::super::super::byte_buffer;
    with_typed(cases::FAILURE, |typed| {
        let associated =
            super::super::association::lower_and_associate_owned_vector(typed).unwrap();
        let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            super::super::super::budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            for (control, expected, fault) in [
                (
                    byte_buffer::testing::Control {
                        fault: Some((byte_buffer::Boundary::Allocate, 3)),
                        ..Default::default()
                    },
                    42,
                    true,
                ),
                (
                    byte_buffer::testing::Control {
                        limit: Some(512),
                        ..Default::default()
                    },
                    43,
                    false,
                ),
            ] {
                let ((result, _), observed) = byte_buffer::testing::capture(control, || {
                    execute::observe_witness(
                        &witness,
                        typed.entry(),
                        execute::Limits::default(),
                        policy,
                    )
                });
                assert_eq!(result, Ok(Scalar::I32(expected)), "{policy:?}");
                assert_eq!(observed.fault_applied, fault);
                //24-byte Parcel cells +8-byte identities at capacity8 +64
                // header =320; two independent40-byte buffers remain live.
                let ledger = byte_buffer::Ledger {
                    allocations: 3,
                    releases: 3,
                    peak_bytes: 400,
                    ..Default::default()
                };
                assert_eq!(observed.before_cleanup, ledger);
                assert_eq!(observed.after_cleanup, ledger);
            }
            // With no imposed failure, the same actual source must take the
            // success branch; a fabricated failure result cannot pass this.
            let ((result, _), observed) = byte_buffer::testing::capture(Default::default(), || {
                execute::observe_witness(
                    &witness,
                    typed.entry(),
                    execute::Limits::default(),
                    policy,
                )
            });
            assert_eq!(result, Ok(Scalar::I32(0)));
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
fn source_owned_parameter_early_return_and_fuel_trap_release_failure_and_vector_children() {
    use super::super::super::byte_buffer;
    with_typed(cases::FAILURE, |typed| {
        let associated =
            super::super::association::lower_and_associate_owned_vector(typed).unwrap();
        let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            super::super::super::budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        let control = byte_buffer::testing::Control {
            limit: Some(512),
            ..Default::default()
        };
        let limits = execute::Limits::default();
        let ((result, remaining), _) = byte_buffer::testing::capture(control, || {
            execute::observe_witness(&witness, typed.entry(), limits, ExecutionPolicy::Bounded)
        });
        assert_eq!(result, Ok(Scalar::I32(43)));
        let consumed = limits.fuel - remaining;
        for (fuel, succeeds) in [
            (consumed, true),
            (consumed - 1, false),
            (consumed / 2, false),
        ] {
            let ((result, _), observed) = byte_buffer::testing::capture(control, || {
                execute::observe_witness(
                    &witness,
                    typed.entry(),
                    execute::Limits { fuel, ..limits },
                    ExecutionPolicy::Bounded,
                )
            });
            assert_eq!(result.is_ok(), succeeds);
            assert_eq!(observed.after_cleanup.live_bytes, 0);
            assert_eq!(observed.after_cleanup.live_allocations, 0);
            assert_eq!(
                observed.after_cleanup.allocations,
                observed.after_cleanup.releases
            );
        }
    });
}
