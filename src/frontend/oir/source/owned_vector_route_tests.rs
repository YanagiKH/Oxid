//! Genuine source/project/package router controls. No fabricated AST or seal.
use super::*;
use crate::frontend::{
    build_config::ExecutionPolicy,
    declaration_index::{self as index, IndexLimits, Observation, SourceOwner, WorkMeter},
    project::{
        budget::Allocator,
        packages::{PackageDependency, PackageSources, PackageSpec},
        ProjectLimits, ProjectSources,
    },
    source::Span,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new(text: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "oxid-owned-vector-public-route-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("main.ox"), text).unwrap();
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
        if std::thread::panicking() || std::env::var_os("OXID_KEEP_TEST_PROJECTS").is_some() {
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}
const TYPES: &str =
    "use alloc::bytes::ByteBuf;use alloc::vec::Vec;struct Message{value:i32,bytes:ByteBuf}";
fn workflow() -> String {
    format!("{TYPES}use alloc::bytes::new as bytes_new;use alloc::bytes::try_push as bytes_push;use alloc::vec::new;use alloc::vec::len;use alloc::vec::capacity;use alloc::vec::try_reserve;use alloc::vec::push_reserved;use alloc::vec::pop_last;use alloc::vec::truncate;use alloc::vec::take;fn identity<T>(value:T)->T{{return value;}}fn make(value:i32)->Message{{let mut bytes=bytes_new();bytes_push(&mut bytes,value.to_u8_checked());return Message{{value:value,bytes:bytes}};}}fn read_and_drop(message:Message)->i32{{return message.value;}}fn main()->i32{{let mut values=new::<Message>();try_reserve(&mut values,8);let reserved=capacity(&values);push_reserved(&mut values,make(41));let length=len(&values);let number=read_and_drop(identity(pop_last(&mut values)));push_reserved(&mut values,make(42));let mut taken=take(&mut values);truncate(&mut taken,0);return number+reserved+length+capacity(&taken)+capacity(&values);}}")
}
fn checked<'s>(project: &'s ProjectSources, allocator: &mut Allocator) -> CheckedSourceProgram<'s> {
    check_project_executable_candidate(
        project,
        IndexLimits::default(),
        &WorkMeter::default(),
        allocator,
    )
    .unwrap()
}
fn boxed(allocator: &Allocator) -> bool {
    allocator
        .trace
        .iter()
        .any(|event| event.kind == "owned vector source body")
}
fn candidate_attempted(allocator: &Allocator) -> bool {
    allocator
        .trace
        .iter()
        .any(|event| event.kind == "owned vector candidate planning owner")
}

#[test]
fn owned_vector_public_route_all_eight_calls_reach_checked_consumers() {
    let fixture = Fixture::new(&workflow());
    let project = fixture.project();
    let mut type_allocator = Allocator::default();
    let summary = check_project_candidate(
        &project,
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut type_allocator,
    )
    .unwrap();
    assert_eq!(summary.route(), super::super::project::ProjectRoute::Owned);
    assert!(candidate_attempted(&type_allocator));
    assert!(!boxed(&type_allocator));
    let mut allocator = Allocator::default();
    let program = checked(&project, &mut allocator);
    assert!(boxed(&allocator));
    assert_eq!(program.function_count(), summary.functions());
    assert_eq!(program.run_test_i32().unwrap(), 58);
    assert_eq!(
        program.run_with_execution(ExecutionPolicy::Hosted).unwrap(),
        Scalar::I32(58)
    );
    assert_eq!(
        program.run_result(ExecutionPolicy::Bounded).unwrap(),
        RuntimeValue::Scalar(RuntimeScalar::Narrow(Scalar::I32(58)))
    );
    assert!(program.debug_records().is_some());
    assert!(program.native_module().is_ok());
    assert!(program
        .native_debug_module_with_execution(false, ExecutionPolicy::Bounded, None)
        .is_ok());
    assert!(program
        .native_optimized_module_with_execution(false, ExecutionPolicy::Bounded, None, false)
        .is_ok());
}

#[test]
fn owned_vector_public_route_zero_key_types_and_unused_new_imports_choose_real_domain() {
    for (source, expect_box) in [
        (format!("{TYPES}fn inspect(values:&Vec<Message>)->(){{return;}}fn main()->i32{{return 0;}}"), true),
        (format!("{TYPES}fn inspect<Marker>(values:&Vec<Message>)->(){{return;}}fn main()->i32{{return 0;}}"), false),
        ("use alloc::vec::push_reserved;fn main()->i32{return 0;}".into(), false),
        ("use alloc::vec::pop_last;fn main()->i32{return 0;}".into(), false),
        (format!("{TYPES}use alloc::vec::new;fn main()->i32{{new::<Message>();return 0;}}"), true),
    ] {
        let fixture = Fixture::new(&source);
        let project = fixture.project();
        let mut allocator = Allocator::default();
        let program = checked(&project, &mut allocator);
        assert!(candidate_attempted(&allocator));
        assert_eq!(boxed(&allocator), expect_box, "{source}");
        assert_eq!(program.run_test_i32().unwrap(), 0);
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    all(
        windows,
        target_arch = "x86_64",
        target_pointer_width = "64",
        target_endian = "little"
    )
))]
#[test]
fn owned_vector_public_route_package_owner_keeps_namespaces_and_unused_generic_facts() {
    let fixture = Fixture::new(&workflow());
    let root = fs::canonicalize(&fixture.0).unwrap();
    let specs = [PackageSpec {
        root: root.clone(),
        entry: root.join("main.ox"),
        dependencies: vec![],
    }];
    let package = PackageSources::load(&specs, None, ProjectLimits::default()).unwrap();
    let mut allocator = Allocator::default();
    let program = check_package_executable(
        &package,
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut allocator,
    )
    .unwrap();
    assert!(boxed(&allocator));
    assert_eq!(program.run_test_i32().unwrap(), 58);

    fs::create_dir(root.join("app")).unwrap();
    fs::create_dir(root.join("model")).unwrap();
    fs::write(root.join("app/main.ox"), "fn main()->i32{return 3;}").unwrap();
    fs::write(root.join("model/lib.ox"), "use alloc::bytes::ByteBuf;use alloc::vec::Vec;pub struct Entry{pub bytes:ByteBuf}pub fn inspect<Marker>(values:&Vec<Entry>)->(){return;}").unwrap();
    let specs = [
        PackageSpec {
            root: root.join("app"),
            entry: root.join("app/main.ox"),
            dependencies: vec![PackageDependency {
                alias: "model".into(),
                package: 1,
            }],
        },
        PackageSpec {
            root: root.join("model"),
            entry: root.join("model/lib.ox"),
            dependencies: vec![],
        },
    ];
    let package = PackageSources::load(&specs, None, ProjectLimits::default()).unwrap();
    let mut allocator = Allocator::default();
    let program = check_package_executable(
        &package,
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut allocator,
    )
    .unwrap();
    assert!(candidate_attempted(&allocator));
    assert!(!boxed(&allocator));
    assert_eq!(program.run_test_i32().unwrap(), 3);
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    all(
        windows,
        target_arch = "x86_64",
        target_pointer_width = "64",
        target_endian = "little"
    )
)))]
#[test]
fn owned_vector_public_route_package_owner_refuses_unqualified_hosts_before_source_loading() {
    let fixture = Fixture::new(&workflow());
    let root = fs::canonicalize(&fixture.0).unwrap();
    let single = [PackageSpec {
        root: root.clone(),
        entry: root.join("main.ox"),
        dependencies: vec![],
    }];
    fs::create_dir(root.join("app")).unwrap();
    fs::create_dir(root.join("model")).unwrap();
    fs::write(root.join("app/main.ox"), "fn main()->i32{return 3;}").unwrap();
    fs::write(root.join("model/lib.ox"), "use alloc::bytes::ByteBuf;use alloc::vec::Vec;pub struct Entry{pub bytes:ByteBuf}pub fn inspect<Marker>(values:&Vec<Entry>)->(){return;}").unwrap();
    let multiple = [
        PackageSpec {
            root: root.join("app"),
            entry: root.join("app/main.ox"),
            dependencies: vec![PackageDependency {
                alias: "model".into(),
                package: 1,
            }],
        },
        PackageSpec {
            root: root.join("model"),
            entry: root.join("model/lib.ox"),
            dependencies: vec![],
        },
    ];
    for specs in [single.as_slice(), multiple.as_slice()] {
        let failure = PackageSources::load(specs, None, ProjectLimits::default()).unwrap_err();
        assert_eq!(failure.diagnostics.len(), 1);
        assert_eq!(
            (failure.diagnostics[0].code, failure.diagnostics[0].stage),
            ("E0006", "package"),
        );
        assert_eq!(
            failure.diagnostics[0].message,
            "package source policy is not qualified on this host",
        );
        assert!(failure.sources.files().is_empty());
        assert_eq!(
            failure.usage,
            crate::frontend::project::SourceUsage::default()
        );
        assert_eq!(failure.allocator.attempts, 0);
        assert!(failure.allocator.trace.is_empty());
    }
}

#[test]
fn owned_vector_public_route_borrow_move_and_copy_misuse_cannot_reach_consumers() {
    for body in [
        "let mut values=new::<Message>();let first=values;let second=values;",
        "let mut values=new::<Message>();push_reserved(&mut values,pop_last(&mut values));",
    ] {
        let source = format!("{TYPES}use alloc::vec::new;use alloc::vec::push_reserved;use alloc::vec::pop_last;fn main()->i32{{{body}return 0;}}");
        let fixture = Fixture::new(&source);
        let project = fixture.project();
        let mut allocator = Allocator::default();
        let errors = check_project_executable_candidate(
            &project,
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut allocator,
        )
        .unwrap_err();
        assert!(candidate_attempted(&allocator));
        assert!(!boxed(&allocator));
        assert!(
            errors.iter().any(|error| error.code != "E0205"),
            "full ownership proof was not reached: {errors:?}"
        );
    }
    let fixture = Fixture::new("use alloc::vec::new;use alloc::vec::push_reserved;fn main()->i32{let mut values=new::<i32>();push_reserved(&mut values,1);return 0;}");
    let project = fixture.project();
    let mut allocator = Allocator::default();
    assert!(check_project_executable_candidate(
        &project,
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut allocator
    )
    .is_err());
    assert!(candidate_attempted(&allocator));
    assert!(!boxed(&allocator));
}

#[test]
fn owned_vector_public_route_accepted_scalar_byte_and_copy_vectors_use_old_path() {
    for (source, expected) in [
        ("fn main()->i32{return 7;}", RuntimeValue::Scalar(RuntimeScalar::Narrow(Scalar::I32(7)))),
        ("fn main()->u8{return 9u8;}", RuntimeValue::Scalar(RuntimeScalar::Narrow(Scalar::U8(9)))),
        ("use alloc::vec::new;use alloc::vec::len;fn main()->i32{let values=new::<i32>();return len(&values);}", RuntimeValue::Scalar(RuntimeScalar::Narrow(Scalar::I32(0)))),
    ] {
        let fixture = Fixture::new(source);
        let project = fixture.project();
        let mut allocator = Allocator::default();
        let program = checked(&project, &mut allocator);
        assert!(!candidate_attempted(&allocator));
        assert!(!boxed(&allocator));
        assert_eq!(program.run_result(ExecutionPolicy::Bounded).unwrap(), expected);
    }
    #[cfg(target_pointer_width = "64")]
    assert_eq!(std::mem::size_of::<CheckedSourceProgram<'static>>(), 352);
}

#[test]
fn owned_vector_public_route_caps_and_box_reservation_fail_before_authority_escapes() {
    let fixture = Fixture::new(&workflow());
    let project = fixture.project();
    let mut baseline = Allocator::default();
    checked(&project, &mut baseline);
    let ordinal = baseline
        .trace
        .iter()
        .position(|event| event.kind == "owned vector source body")
        .unwrap()
        + 1;
    let mut allocator = Allocator {
        fail_at: Some(ordinal),
        ..Default::default()
    };
    let errors = check_project_executable_candidate(
        &project,
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut allocator,
    )
    .unwrap_err();
    assert!(errors.iter().any(|error| error.code == "E0400"));
    assert!(allocator
        .trace
        .iter()
        .any(|event| event.kind == "owned vector source body" && !event.success));

    let fixture = Fixture::new("use alloc::vec::push_reserved;fn main()->i32{return 0;}");
    let project = fixture.project();
    let mut allocator = Allocator::default();
    let scopes = IndexLimits {
        retained: owned_vector_route_carrier_bytes() as u64 - 1,
        scratch: owned_vector_route_carrier_bytes() as u64 - 1,
        ..Default::default()
    };
    assert!(check_project_executable_candidate(
        &project,
        scopes,
        &WorkMeter::default(),
        &mut allocator
    )
    .is_err());
    assert_eq!(allocator.attempts, 0);

    let old = Fixture::new("use alloc::vec::new;fn main()->i32{let values=new::<i32>();return 0;}");
    let project = old.project();
    let mut allocator = Allocator::default();
    let errors = check_project_executable_candidate(
        &project,
        IndexLimits {
            retained: 0,
            ..Default::default()
        },
        &WorkMeter::default(),
        &mut allocator,
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "E0400");
    assert!(!candidate_attempted(&allocator));
}

#[test]
fn owned_vector_public_route_work_spends_the_failed_attempt_and_original_adapter_stays_closed() {
    let fixture = Fixture::new(&workflow());
    let project = fixture.project();
    let work = WorkMeter::default();
    check_project_executable_candidate(
        &project,
        IndexLimits::default(),
        &work,
        &mut Allocator::default(),
    )
    .unwrap();
    let exact = WorkMeter::new(work.used());
    check_project_executable_candidate(
        &project,
        IndexLimits::default(),
        &exact,
        &mut Allocator::default(),
    )
    .unwrap();
    assert_eq!(exact.used(), work.used());
    assert!(check_project_executable_candidate(
        &project,
        IndexLimits::default(),
        &WorkMeter::new(work.used() - 1),
        &mut Allocator::default()
    )
    .is_err());

    let original = Fixture::new("use alloc::vec::Vec;fn main()->i32{return 0;}");
    let project = original.project();
    let root = project.modules()[0].file;
    let file = project.sources().get(root);
    let ast = project.try_file_ast(root).unwrap();
    let errors = check_source(file, ast, project.sources()).unwrap_err();
    assert_eq!(errors[0].code, "E0500");
}

#[test]
fn owned_vector_public_route_retained_errors_pay_capacity_not_text_length() {
    let mut errors = Vec::with_capacity(3);
    errors.push(*Diagnostic::new(
        "E0205",
        "resolve",
        "unsupported standard library import",
        None,
    ));
    let at = Span {
        file: crate::frontend::source::SourceFileId(0),
        start: 0,
        end: 0,
    };
    let before =
        sealed::owned_route_storage(&errors, errors.capacity(), &WorkMeter::default(), at).unwrap();
    let old_capacity = errors[0].message.capacity();
    errors[0].message.reserve_exact(256);
    let extra = errors[0].message.capacity() - old_capacity;
    let expanded =
        sealed::owned_route_storage(&errors, errors.capacity(), &WorkMeter::default(), at).unwrap();
    assert_eq!(expanded, before + extra);
    errors[0].message.clear();
    assert_eq!(
        sealed::owned_route_storage(&errors, errors.capacity(), &WorkMeter::default(), at).unwrap(),
        expanded
    );
    assert!(sealed::owned_route_storage(&errors, 0, &WorkMeter::default(), at).is_err());
}

#[test]
fn owned_vector_semantic_observations_failed_candidate_keeps_the_original_transaction() {
    let fixture =
        Fixture::new("struct Message{value:i32}struct Message{value:i32}fn main()->i32{return 0;}");
    let project = fixture.project();
    let original = WorkMeter::default();
    original.enable_observation();
    let expected = index::collect_originals(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &original,
        &mut Allocator::default(),
    )
    .unwrap()
    .finish(&original, &mut Allocator::default())
    .unwrap_err();
    let work = WorkMeter::default();
    work.enable_observation();
    let mut allocator = Allocator::default();
    let errors = check_project_candidate(&project, IndexLimits::default(), &work, &mut allocator)
        .expect_err("the failed candidate must retain the original diagnostics");
    assert!(candidate_attempted(&allocator));
    let diagnostics = |errors: &[Diagnostic]| {
        errors
            .iter()
            .map(|error| error.render_json(project.sources()))
            .collect::<Vec<_>>()
    };
    assert_eq!(diagnostics(&errors), diagnostics(&expected));
    let semantic = |meter: &WorkMeter| {
        meter
            .observations
            .borrow()
            .iter()
            .filter(|event| !matches!(event, Observation::Route { .. }))
            .map(|event| format!("{event:?}"))
            .collect::<Vec<_>>()
    };
    assert_eq!(semantic(&work), semantic(&original));
    let fills = |meter: &WorkMeter| {
        meter
            .events
            .borrow()
            .iter()
            .filter(|event| event.operation == "declaration fill")
            .count()
    };
    assert!(fills(&original) > 0);
    assert_eq!(fills(&work), 2 * fills(&original));
    assert!(work.used() > original.used());
    assert_eq!(work.limit(), original.limit());
    work.record_error(&errors[0]);
    assert!(matches!(
        work.observations.borrow().last(),
        Some(Observation::Diagnostic {
            phase: "original-conflicts",
            ..
        })
    ));
}

#[test]
fn owned_vector_semantic_observations_success_has_one_selected_declaration_inventory() {
    let fixture = Fixture::new(&workflow());
    let project = fixture.project();
    let work = WorkMeter::default();
    work.enable_observation();
    check_project_candidate(
        &project,
        IndexLimits::default(),
        &work,
        &mut Allocator::default(),
    )
    .unwrap();
    let observations = work.observations.borrow();
    let originals = observations
        .iter()
        .filter_map(|event| match event {
            Observation::Original { kind, id, .. } => Some((*kind, *id)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!originals.is_empty());
    assert_eq!(
        originals.len(),
        originals
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    );
    assert_eq!(
        observations
            .iter()
            .filter(|event| matches!(event, Observation::Phase("collection")))
            .count(),
        1
    );
    assert_eq!(
        observations
            .iter()
            .filter(|event| matches!(event, Observation::Frozen { .. }))
            .count(),
        1
    );
    assert!(!observations
        .iter()
        .any(|event| matches!(event, Observation::Diagnostic { .. })));
    assert!(work
        .events
        .borrow()
        .iter()
        .any(|event| { event.operation == "owned vector retained diagnostic" }));
}

#[test]
fn owned_vector_semantic_observations_selected_allocation_failure_is_not_rolled_back() {
    let fixture = Fixture::new(&workflow());
    let project = fixture.project();
    let mut baseline = Allocator::default();
    check_project_candidate(
        &project,
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut baseline,
    )
    .unwrap();
    let ordinals = baseline
        .trace
        .iter()
        .enumerate()
        .filter_map(|(index, event)| (event.kind == "paid typed bodies").then_some(index + 1))
        .collect::<Vec<_>>();
    assert_eq!(ordinals.len(), 1);
    let ordinal = ordinals[0];
    assert!(baseline.trace[ordinal - 1].success);
    let work = WorkMeter::default();
    work.enable_observation();
    let mut allocator = Allocator {
        fail_at: Some(ordinal),
        ..Default::default()
    };
    let errors = check_project_candidate(&project, IndexLimits::default(), &work, &mut allocator)
        .expect_err(
            "the selected candidate must preserve its actual typed-body allocation failure",
        );
    assert!(candidate_attempted(&allocator));
    assert_eq!(allocator.attempts, ordinal);
    assert_eq!(
        allocator
            .trace
            .last()
            .map(|event| (event.kind, event.success)),
        Some(("paid typed bodies", false))
    );
    assert_eq!(errors.len(), 1);
    assert_eq!((errors[0].code, errors[0].stage), ("E0400", "resolve"));
    assert_eq!(errors[0].message, "affected HIR allocation failed");
    let observations = work.observations.borrow();
    let observed = observations
        .iter()
        .filter_map(|event| match event {
            Observation::Diagnostic { phase, diagnostic } => {
                assert_eq!(*phase, "type");
                Some(diagnostic.render_json(project.sources()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        errors
            .iter()
            .map(|error| error.render_json(project.sources()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        observations
            .iter()
            .filter(|event| matches!(event, Observation::Phase("collection")))
            .count(),
        1
    );
    assert_eq!(
        observations
            .iter()
            .filter(|event| matches!(event, Observation::Frozen { .. }))
            .count(),
        1
    );
}

#[test]
fn owned_vector_semantic_observations_do_not_change_work_limits_or_allocator_attempts() {
    for source in [
        workflow(),
        "struct Message{value:i32}struct Message{value:i32}fn main()->i32{return 0;}".into(),
    ] {
        let fixture = Fixture::new(&source);
        let project = fixture.project();
        let observing = WorkMeter::default();
        observing.enable_observation();
        let disabled = WorkMeter::default();
        let mut first = Allocator::default();
        let mut second = Allocator::default();
        let observed =
            check_project_candidate(&project, IndexLimits::default(), &observing, &mut first);
        let plain =
            check_project_candidate(&project, IndexLimits::default(), &disabled, &mut second);
        assert_eq!(observed.is_ok(), plain.is_ok());
        assert!(observing.used() > 0);
        assert_eq!(observing.used(), disabled.used());
        assert_eq!(observing.limit(), disabled.limit());
        assert!(disabled.observations.borrow().is_empty());
        assert!(disabled.events.borrow().is_empty());
        assert_eq!(first.attempts, second.attempts);
        let allocations = |allocator: &Allocator| {
            allocator
                .trace
                .iter()
                .map(|event| (event.kind, event.length, event.element_bytes, event.success))
                .collect::<Vec<_>>()
        };
        assert_eq!(allocations(&first), allocations(&second));
        if observed.is_ok() {
            let exact = WorkMeter::new(observing.used());
            exact.enable_observation();
            check_project_candidate(
                &project,
                IndexLimits::default(),
                &exact,
                &mut Allocator::default(),
            )
            .unwrap();
            assert_eq!(exact.used(), observing.used());
            let short = WorkMeter::new(observing.used() - 1);
            short.enable_observation();
            assert!(check_project_candidate(
                &project,
                IndexLimits::default(),
                &short,
                &mut Allocator::default(),
            )
            .is_err());
        }
    }
}
