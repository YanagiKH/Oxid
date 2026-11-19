//! Real parsed sources through paid typing, exact association and full proof.
use super::*;
use crate::frontend::{
    build_config::{ExecutionPolicy, NativeBuildSettings, NativeOptimization},
    project::{ProjectLimits, ProjectSources},
};
use std::path::PathBuf;

const IMPORTS: &str = "use alloc::bytes::ByteBuf;use alloc::bytes::new as bytes_new;use alloc::bytes::try_push as bytes_push;use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::len;use alloc::vec::capacity;use alloc::vec::try_reserve;use alloc::vec::push_reserved;use alloc::vec::pop_last;use alloc::vec::truncate;use alloc::vec::take;struct Message{value:i32,bytes:ByteBuf}";
const BODY: &str = "fn identity<T>(value:T)->T{return value;}fn make(value:i32)->Message{let mut bytes=bytes_new();bytes_push(&mut bytes,value.to_u8_checked());return Message{value:value,bytes:bytes};}fn read_and_drop(message:Message)->i32{return message.value;}fn main()->i32{let mut values=new::<Message>();try_reserve(&mut values,8);let reserved=capacity(&values);push_reserved(&mut values,identity(make(41)));let length=len(&values);let number=read_and_drop(pop_last(&mut values));push_reserved(&mut values,make(42));let mut taken=take(&mut values);truncate(&mut taken,0);return number+reserved+length+capacity(&taken)+capacity(&values);}";

struct Fixture(PathBuf);
impl Fixture {
    fn new(text: &str) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "oxid-owned-vector-source-execution-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("main.ox"), text).unwrap();
        Self(path)
    }
    fn project(&self) -> ProjectSources {
        ProjectSources::load_typed(
            self.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() && std::env::var_os("OXID_KEEP_TEST_PROJECTS").is_none() {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

fn with_typed(
    text: &str,
    inspect: impl FnOnce(&typeck::TypedOwnedProgram<'_>, &SourceMap, &Fixture),
) {
    let fixture = Fixture::new(text);
    let project = fixture.project();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = index::collect_owned_vector_candidate(
        SourceOwner::project(&project),
        IndexLimits::default(),
        8 * 1024 * 1024,
        &work,
        &mut allocator,
    )
    .unwrap();
    let typed = resolve::type_owned_vector_source(&index, &work, &mut allocator).unwrap();
    inspect(&typed, project.sources(), &fixture);
}

#[test]
fn owned_vector_source_execution_runs_live_resources_through_the_shared_vm() {
    with_typed(&format!("{IMPORTS}{BODY}"), |typed, _, _| {
        let associated =
            super::super::association::lower_and_associate_owned_vector(typed).unwrap();
        let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            super::super::super::budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        let limits = execute::Limits::default();
        let ((result, remaining), observation) =
            super::super::super::byte_buffer::testing::capture(Default::default(), || {
                execute::observe_witness(&witness, typed.entry(), limits, ExecutionPolicy::Bounded)
            });
        assert_eq!(result, Ok(Scalar::I32(58)));
        assert_eq!(observation.before_cleanup.live_allocations, 0);
        assert_eq!(observation.before_cleanup.allocations, 3);
        assert_eq!(observation.before_cleanup.releases, 3);
        assert_eq!(observation.before_cleanup.peak_bytes, 360);
        assert_eq!(observation.before_cleanup, observation.after_cleanup);
        let exact = limits.fuel - remaining;
        for (fuel, succeeds) in [(exact, true), (exact - 1, false)] {
            let (result, _) = execute::observe_witness(
                &witness,
                typed.entry(),
                execute::Limits { fuel, ..limits },
                ExecutionPolicy::Bounded,
            );
            assert_eq!(result.is_ok(), succeeds);
        }
        let (result, remaining) = execute::observe_witness(
            &witness,
            typed.entry(),
            execute::Limits { fuel: 0, ..limits },
            ExecutionPolicy::Hosted,
        );
        assert_eq!(result, Ok(Scalar::I32(58)));
        assert_eq!(remaining, crate::frontend::build_config::HOSTED_FUEL);
    });
}

#[test]
fn owned_vector_source_execution_native_results_survive_selection_and_optimization() {
    if std::env::var_os("OXID_TEST_LLVM_BIN").is_none() {
        return;
    }
    with_typed(&format!("{IMPORTS}{BODY}"), |typed, sources, fixture| {
        assert_native_outputs(typed, sources, fixture, 58);
    });
}

#[test]
fn owned_vector_source_execution_conversion_authority_requires_the_consumed_source_owner() {
    with_typed(&format!("{IMPORTS}{BODY}"), |typed, sources, _| {
        use super::super::super::owned_vector_descriptors::{
            OwnedVectorImportClaims, RawOwnedVectorProgram,
        };
        use crate::frontend::builtin_catalog::owned_vector::Item;
        assert!(check_typed(typed).is_err());
        let raw = lower::lower(typed).unwrap();
        let wrapper = RawOwnedVectorProgram::prepare(
            raw,
            OwnedVectorImportClaims::empty()
                .admit(Item::PushReserved)
                .admit(Item::PopLast),
            typed.index().owned_vector_functions(),
            sources,
            super::super::super::budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        let refusal = verified::VerifiedOwnedVectorProgram::verify(
            wrapper,
            sources,
            super::super::super::budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap_err();
        assert!(
            matches!(
                refusal.kind,
                OwnedFailureKind::Malformed(Malformed::Scalar(
                    FailureKind::UnauthenticatedConversion
                ))
            ),
            "{refusal:?}"
        );
        let associated =
            super::super::association::lower_and_associate_owned_vector(typed).unwrap();
        verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            super::super::super::budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
    });
}

#[test]
fn owned_vector_source_execution_native_fuel_matches_the_reference_boundary() {
    if std::env::var_os("OXID_TEST_LLVM_BIN").is_none() {
        return;
    }
    with_typed(&format!("{IMPORTS}{BODY}"), |typed, sources, fixture| {
        let associated =
            super::super::association::lower_and_associate_owned_vector(typed).unwrap();
        let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
            associated,
            super::super::super::budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        let limits = execute::Limits::default();
        let (result, remaining) =
            execute::observe_witness(&witness, typed.entry(), limits, ExecutionPolicy::Bounded);
        assert_eq!(result, Ok(Scalar::I32(58)));
        let exact = limits.fuel - remaining;
        for (fuel, succeeds) in [(exact, true), (exact - 1, false)] {
            let module =
                native::native_module_with_fuel(&witness, typed.entry().unwrap(), sources, fuel)
                    .unwrap();
            for optimization in [NativeOptimization::O0, NativeOptimization::O2] {
                let output = fixture.0.join(format!("fuel-{fuel}-{optimization:?}"));
                crate::frontend::native::compile_with_settings(
                    &module,
                    output.to_str().unwrap(),
                    &NativeBuildSettings::with_optimization(false, optimization),
                )
                .unwrap();
                let observed = std::process::Command::new(&output)
                    .env_clear()
                    .output()
                    .unwrap();
                assert_eq!(
                    observed.status.success(),
                    succeeds,
                    "fuel{fuel}: {}",
                    String::from_utf8_lossy(&observed.stderr)
                );
                if succeeds {
                    assert_eq!(observed.stdout, b"58\n");
                    assert!(observed.stderr.is_empty());
                } else {
                    assert!(observed.stdout.is_empty());
                    assert!(String::from_utf8_lossy(&observed.stderr).contains("fuel"));
                }
            }
        }
    });
}

#[test]
fn owned_vector_source_execution_full_ownership_proof_rejects_duplicate_moves_and_borrows() {
    for body in [
        "fn main()->i32{let mut values=new::<Message>();let first=values;let second=values;return 0;}",
        "fn main()->i32{let mut values=new::<Message>();push_reserved(&mut values,pop_last(&mut values));return 0;}",
    ] {
        with_typed(&format!("{IMPORTS}{body}"), |typed, _, _| {
            let associated = super::super::association::lower_and_associate_owned_vector(typed).unwrap();
            let refusal = verified::VerifiedOwnedVectorProgram::verify_associated(
                associated, super::super::super::budget::Limits::DEFAULT, &mut Allocator::default(),
            ).unwrap_err();
            assert!(matches!(refusal.kind, OwnedFailureKind::Ownership(_)), "{refusal:?}");
        });
    }
}

#[test]
fn owned_vector_source_execution_overlapping_lower_and_tracker_scopes_precede_allocations() {
    with_typed(&format!("{IMPORTS}{BODY}"), |typed, _, _| {
        use super::super::{association, budget};
        use crate::frontend::oir::source::association::{
            conversion_carrier_bytes, conversion_seen_bytes,
        };
        let original = typed.source_storage_bytes().unwrap();
        let caller = association::owned_vector_scope_bytes();
        let usage = budget::preflight(typed, budget::Limits::DEFAULT).unwrap();
        let exact = original + caller + usage.scratch_bytes + lower::invocation_control_bytes();
        budget::reset_guard_counts();
        let refusal = budget::with_lower_limit(exact - 1, || lower::lower(typed)).unwrap_err();
        assert_eq!(
            refusal.kind,
            OwnedFailureKind::Resource("source HIR scratch")
        );
        assert_eq!(
            budget::guard_counts()[budget::GuardEvent::RawReservation as usize],
            0
        );
        budget::with_lower_limit(exact, || lower::lower(typed)).unwrap();
        assert!(budget::guard_counts()[budget::GuardEvent::RawReservation as usize] > 0);
        assert_eq!(typed.source_storage_bytes(), Some(original));

        let owners = typed.index().sources().count();
        let expressions = (0..owners)
            .map(|slot| {
                typed
                    .index()
                    .sources()
                    .try_ast_borrowed(crate::frontend::project::ModuleId(slot))
                    .unwrap()
                    .expressions
                    .len()
            })
            .sum();
        let tracker =
            conversion_seen_bytes(owners, expressions).unwrap() - conversion_carrier_bytes();
        assert!(tracker > 0);
        let exact = typed.conversion_association_storage_bytes().unwrap() + caller + tracker;
        let before = association::CONVERSION_RESERVE_ENTRIES.with(|entries| entries.get());
        let refusal = budget::with_conversion_limit(exact - 1, || {
            association::lower_and_associate_owned_vector(typed)
        })
        .err()
        .expect("tracker overlap must be admitted before reserving it");
        assert_eq!(refusal.code, "E0400");
        assert_eq!(
            association::CONVERSION_RESERVE_ENTRIES.with(|entries| entries.get()),
            before
        );
        budget::with_conversion_limit(exact, || {
            association::lower_and_associate_owned_vector(typed)
        })
        .unwrap();
        assert_eq!(
            association::CONVERSION_RESERVE_ENTRIES.with(|entries| entries.get()),
            before + 1
        );
        assert_eq!(typed.source_storage_bytes(), Some(original));
    });
}

#[test]
fn owned_vector_source_execution_zero_extension_consumes_into_the_original_limited_proof() {
    with_typed(
        "use alloc::vec::push_reserved;use alloc::vec::pop_last;fn main()->i32{return 42;}",
        |typed, _, _| {
            use super::super::association::{
                lower_and_associate_owned_vector, AssociatedSourceDomain,
            };
            let associated = lower_and_associate_owned_vector(typed).unwrap();
            let AssociatedSourceDomain::Legacy(legacy) = associated.into_source_domain() else {
                panic!("unused templates must retain the original proof domain");
            };
            let witness = verified::verify_associated_with_limits(
                legacy,
                super::super::super::budget::Limits::DEFAULT,
            )
            .unwrap();
            assert_eq!(
                execute::run(&witness, typed.entry()).unwrap(),
                Scalar::I32(42)
            );
            let exact = witness.usage().metadata_bytes;
            for (metadata, succeeds) in [(exact, true), (exact - 1, false)] {
                let AssociatedSourceDomain::Legacy(legacy) =
                    lower_and_associate_owned_vector(typed)
                        .unwrap()
                        .into_source_domain()
                else {
                    panic!("legacy domain changed");
                };
                let proof = verified::verify_associated_with_limits(
                    legacy,
                    super::super::super::budget::Limits {
                        metadata,
                        ..super::super::super::budget::Limits::DEFAULT
                    },
                );
                assert_eq!(proof.is_ok(), succeeds);
            }
        },
    );
    with_typed(&format!("{IMPORTS}{BODY}"), |typed, _, _| {
        let associated =
            super::super::association::lower_and_associate_owned_vector(typed).unwrap();
        assert!(matches!(
            associated.into_source_domain(),
            super::super::association::AssociatedSourceDomain::OwnedVector(_)
        ));
    });
}

fn assert_native_outputs(
    typed: &typeck::TypedOwnedProgram<'_>,
    sources: &SourceMap,
    fixture: &Fixture,
    expected: i32,
) {
    let associated = super::super::association::lower_and_associate_owned_vector(typed).unwrap();
    let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
        associated,
        super::super::super::budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        for reachable in [false, true] {
            let module = if reachable {
                native::native_optimized_module_with_execution(
                    &witness,
                    typed.entry(),
                    sources,
                    false,
                    policy,
                    None,
                    false,
                )
            } else {
                native::native_module_with_execution(
                    &witness,
                    typed.entry(),
                    sources,
                    false,
                    policy,
                    None,
                )
            }
            .unwrap();
            for optimization in [NativeOptimization::O0, NativeOptimization::O2] {
                let output = fixture
                    .0
                    .join(format!("{policy:?}-{reachable}-{optimization:?}"));
                let settings = NativeBuildSettings::with_optimization(false, optimization)
                    .with_execution(policy);
                crate::frontend::native::compile_with_settings(
                    &module,
                    output.to_str().unwrap(),
                    &settings,
                )
                .unwrap();
                let observed = std::process::Command::new(&output)
                    .current_dir(&fixture.0)
                    .env_clear()
                    .output()
                    .unwrap();
                assert!(
                    observed.status.success(),
                    "{}",
                    String::from_utf8_lossy(&observed.stderr)
                );
                assert_eq!(observed.stdout, format!("{expected}\n").as_bytes());
                assert!(observed.stderr.is_empty());
            }
        }
    }
}

#[path = "owned_vector_source_resource_tests.rs"]
mod resource_tests;
