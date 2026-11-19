//! Real parsed projects reach the distinct source owner through exact typing,
//! fresh association and the ordinary complete ownership proof.
use super::*;
use crate::frontend::{
    build_config::ExecutionPolicy,
    project::{ProjectLimits, ProjectSources},
    source::SourceFileId,
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
            "oxid-owned-vector-source-facade-{}-{}",
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
    inspect: impl FnOnce(&typeck::TypedOwnedProgram<'_>, &ProjectSources, &Fixture),
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
    inspect(&typed, &project, &fixture);
}

#[test]
fn owned_vector_source_facade_runs_real_resource_workflow_through_existing_methods() {
    with_typed(&format!("{IMPORTS}{BODY}"), |typed, project, _| {
        let sources = project.sources();
        assert!(check_typed(typed).is_err());
        assert!(!typed.index().is_current_source_pipeline());
        let before = typed.source_storage_bytes();
        let program = super::super::check_owned_vector_typed(typed).unwrap();
        assert_eq!(program.function_count(), typed.index().function_count());
        for execution in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            for value_api in [false, true] {
                let (result, observation) =
                    super::super::super::byte_buffer::testing::capture(Default::default(), || {
                        if value_api {
                            program
                                .run_value(typed.entry(), sources, execution)
                                .map(RuntimeValue::from)
                        } else {
                            program.run_result(typed.entry(), sources, execution)
                        }
                    });
                assert_eq!(result.unwrap(), RuntimeValue::from(Scalar::I32(58)));
                assert_eq!(observation.before_cleanup.live_allocations, 0);
                assert_eq!(observation.before_cleanup.allocations, 3);
                assert_eq!(observation.before_cleanup.releases, 3);
                assert_eq!(observation.before_cleanup.peak_bytes, 360);
                assert_eq!(observation.before_cleanup, observation.after_cleanup);
            }
            assert_eq!(
                program
                    .run_with_execution(typed.entry(), sources, execution)
                    .unwrap(),
                Scalar::I32(58)
            );
        }
        assert_eq!(
            program.run(typed.entry(), sources).unwrap(),
            Scalar::I32(58)
        );
        assert_eq!(typed.source_storage_bytes(), before);
    });
}

#[test]
fn owned_vector_source_facade_native_debug_signature_and_process_interfaces_share_the_owner() {
    with_typed(&format!("{IMPORTS}{BODY}"), |typed, project, fixture| {
        let sources = project.sources();
        let program = super::super::check_owned_vector_typed(typed).unwrap();
        let entry = typed.entry().unwrap();
        let (parameters, integer, _) = program.entry_signature(entry).unwrap();
        assert_eq!(parameters, 0);
        assert!(integer);
        assert!(program.entry_signature(hir::DefId(usize::MAX)).is_none());
        let records = program.debug_records();
        assert_eq!(records.len(), 1);
        let record = records.record(0).unwrap();
        assert_eq!(record.size(), 24);
        assert_eq!(record.align(), 8);
        assert_eq!(record.field(0).unwrap().offset, 0);
        assert!(record.field(1).is_none());
        let path = fixture.0.join("main.ox");
        let names = NativeSourceNames::project_relative(
            sources,
            &fixture.0,
            [(SourceFileId(0), path.as_path())],
        )
        .unwrap();
        let module = program.native_module(Some(entry), sources).unwrap();
        assert!(module.contains("@oxid_owned_vector_push_reserved"));
        let named = program
            .native_module_with_source_names(Some(entry), sources, &names)
            .unwrap();
        assert!(named.contains("@oxid_owned_vector_take"));
        for execution in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let ordinary = program
                .native_module_with_execution(Some(entry), sources, false, execution, Some(&names))
                .unwrap();
            assert!(ordinary.contains("@oxid_owned_vector_take"));
            let debug = program
                .native_debug_module_with_execution(
                    Some(entry),
                    sources,
                    false,
                    execution,
                    Some(&names),
                )
                .unwrap();
            assert!(debug.contains("; oxid.debug.function "));
            let debug = crate::frontend::native_debug::add(
                &debug,
                project,
                Some(&names),
                false,
                Some(program.debug_records()),
            )
            .unwrap();
            assert!(debug.contains("!DICompileUnit"));
            assert!(debug.contains("!DISubprogram(name: \"main\""));
            assert!(debug.contains("isOptimized: false"));
            let optimized = program
                .native_optimized_module_with_execution(
                    Some(entry),
                    sources,
                    false,
                    execution,
                    Some(&names),
                    true,
                )
                .unwrap();
            assert!(optimized.contains("@oxid_owned_vector_take"));
            assert!(optimized.contains("; oxid.debug.function "));
            let optimized = crate::frontend::native_debug::add(
                &optimized,
                project,
                Some(&names),
                true,
                Some(program.debug_records()),
            )
            .unwrap();
            assert!(optimized.contains("!DICompileUnit"));
            assert!(optimized.contains("!DISubprogram(name: \"main\""));
            assert!(optimized.contains("isOptimized: true"));
        }
        let process = program.native_process_module(entry, sources).unwrap();
        assert!(process.contains("@__oxid_process_setup"));
        let named_process = program
            .native_process_module_with_source_names(entry, sources, &names)
            .unwrap();
        assert!(named_process.contains("@__oxid_process_setup"));
        type ProcessResult = Result<Scalar, crate::frontend::oir::ProcessFailure>;
        let _process: fn(&OwnedVectorSourceProgram, hir::DefId, &SourceMap) -> ProcessResult =
            OwnedVectorSourceProgram::run_process;
        let _process_with_execution: fn(
            &OwnedVectorSourceProgram,
            hir::DefId,
            &SourceMap,
            ExecutionPolicy,
        ) -> ProcessResult = OwnedVectorSourceProgram::run_process_with_execution;
    });
}

#[test]
fn owned_vector_source_facade_type_only_results_keep_scalar_and_quad_entry_policies() {
    let prefix = "use alloc::bytes::ByteBuf;use alloc::vec::Vec;struct Message{value:i32,bytes:ByteBuf}fn inspect(values:&Vec<Message>)->(){return;}";
    for (ty, value, expected, scalar) in [
        (
            "u8",
            "7u8",
            RuntimeValue::from(Scalar::U8(7)),
            Some(RuntimeScalar::from(Scalar::U8(7))),
        ),
        (
            "i64",
            "9223372036854775807i64",
            RuntimeValue::from(RuntimeScalar::I64(i64::MAX)),
            Some(RuntimeScalar::I64(i64::MAX)),
        ),
        (
            "i128",
            "170141183460469231731687303715884105727i128",
            RuntimeValue::I128(i128::MAX),
            None,
        ),
        (
            "u128",
            "340282366920938463463374607431768211455u128",
            RuntimeValue::U128(u128::MAX),
            None,
        ),
    ] {
        with_typed(
            &format!("{prefix}fn main()->{ty}{{return {value};}}"),
            |typed, project, _| {
                let sources = project.sources();
                assert!(typed.index().owned_vector_functions().is_empty());
                let CandidateSourceProgram::OwnedVector(program) =
                    super::super::check_owned_vector_candidate_typed(typed).unwrap()
                else {
                    panic!("a type-only owned vector must retain its distinct source owner");
                };
                for execution in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
                    assert_eq!(
                        program
                            .run_result(typed.entry(), sources, execution)
                            .unwrap(),
                        expected
                    );
                    let result = program.run_value(typed.entry(), sources, execution);
                    if let Some(expected) = scalar {
                        assert_eq!(result.unwrap(), expected);
                    } else {
                        assert!(result.is_err());
                    }
                    assert!(program
                        .run_with_execution(typed.entry(), sources, execution)
                        .is_err());
                }
            },
        );
    }
}

#[test]
fn owned_vector_source_facade_prepays_roles_and_preserves_legacy_and_ownership_boundaries() {
    use super::super::budget;
    #[cfg(target_pointer_width = "64")]
    assert_eq!(std::mem::size_of::<SourceProgram>(), 328);
    assert_eq!(
        std::mem::size_of::<OwnedVectorSourceProgram>(),
        std::mem::size_of::<verified::VerifiedOwnedVectorProgram>()
    );
    with_typed(&format!("{IMPORTS}{BODY}"), |typed, _, _| {
        let seed = typed.conversion_association_storage_bytes().unwrap();
        let exact = seed + owned_vector_source_carrier_bytes();
        budget::with_conversion_limit(exact, || {
            budget::admit_conversion_scratch(seed, owned_vector_source_carrier_bytes())
        })
        .unwrap();
        budget::reset_guard_counts();
        let errors = budget::with_conversion_limit(exact - 1, || {
            super::super::check_owned_vector_candidate_typed(typed)
        })
        .unwrap_err();
        assert_eq!(errors[0].code, "E0400");
        assert_eq!(
            budget::guard_counts()[budget::GuardEvent::RawReservation as usize],
            0
        );
    });
    for source in [
        "use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::push_reserved;use alloc::vec::pop_last;fn main()->i32{return 7;}",
        "use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::len;fn main()->i32{let values=new::<i32>();return len(&values)+7;}",
    ] {
        with_typed(source, |typed, project, _| {
            let sources = project.sources();
            assert!(!typed.index().is_current_source_pipeline());
            let CandidateSourceProgram::Legacy(program) = super::super::check_owned_vector_candidate_typed(typed).unwrap() else {
                panic!("a true zero-extension candidate must use the historical owner");
            };
            assert_eq!(program.run(typed.entry(), sources).unwrap(), Scalar::I32(7));
            assert!(super::super::check_owned_vector_typed(typed).is_err());
            assert!(!typed.index().is_current_source_pipeline());
        });
    }
    with_typed(&format!("{IMPORTS}fn main()->i32{{let values=new::<Message>();let first=values;let second=values;return 0;}}"), |typed, _, _| {
        let errors = super::super::check_owned_vector_candidate_typed(typed).unwrap_err();
        assert_eq!(errors[0].code, "E0310");
    });
    let fixture = Fixture::new("fn main()->i32{return 0;}");
    let project = fixture.project();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = index::collect_originals(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap();
    let typed = typeck::check(resolve::resolve_project(&index, &work).unwrap()).unwrap();
    budget::reset_guard_counts();
    let errors = super::super::check_owned_vector_candidate_typed(&typed).unwrap_err();
    assert_eq!(errors[0].code, "E0500");
    assert_eq!(
        budget::guard_counts()[budget::GuardEvent::RawReservation as usize],
        0
    );
    assert_eq!(
        check_typed(&typed)
            .unwrap()
            .run(typed.entry(), project.sources())
            .unwrap(),
        Scalar::I32(0)
    );
}
