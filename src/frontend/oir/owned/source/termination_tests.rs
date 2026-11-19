//! Source identities and independent raw refusals; OS signals run in child tests.
use super::*;
use crate::frontend::{
    build_config::ExecutionPolicy,
    builtin_catalog::{experimental, termination, BuiltinItem, BuiltinSet},
    declaration_index::{collect_originals, IndexLimits, WorkMeter},
    project::{budget::Allocator, ProjectLimits, ProjectSources},
};
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
#[cfg(unix)]
extern "C" fn prior_handler(_: libc::c_int) {}
#[cfg(unix)]
fn disposition(number: libc::c_int) -> libc::sigaction {
    let mut action = std::mem::MaybeUninit::uninit();
    assert_eq!(
        unsafe { libc::sigaction(number, std::ptr::null(), action.as_mut_ptr()) },
        0
    );
    unsafe { action.assume_init() }
}
#[cfg(unix)]
fn same_disposition(number: libc::c_int, expected: &libc::sigaction) {
    let actual = disposition(number);
    assert_eq!(actual.sa_sigaction, expected.sa_sigaction);
    assert_eq!(actual.sa_flags, expected.sa_flags);
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGUSR1] {
        assert_eq!(
            unsafe { libc::sigismember(&actual.sa_mask, signal) },
            unsafe { libc::sigismember(&expected.sa_mask, signal) }
        );
    }
}
const IMPORTS: &str =
    "use std::process::enable_termination_notifications;use std::process::termination_requested;";
fn project(text: &str) -> ProjectSources {
    let path = std::env::temp_dir().join(format!(
        "oxid-termination-source-{}-{}.ox",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).unwrap();
    let result = ProjectSources::load_typed(path.to_str().unwrap(), ProjectLimits::default());
    std::fs::remove_file(path).unwrap();
    result.unwrap()
}

fn duplicate_without_records(original: &RawOwnedProgram) -> RawOwnedProgram {
    assert!(original.records.is_empty());
    RawOwnedProgram {
        builtins: original.builtins,
        vector_functions: original.vector_functions.clone(),
        enums: original.enums.clone(),
        records: Vec::new(),
        functions: original.functions.clone(),
    }
}

#[test]
fn process_source_mints_two_separate_scalar_descriptors_after_old_suffixes() {
    let project = project(&format!("{IMPORTS}fn main()->i32{{let enabled=enable_termination_notifications();if termination_requested(){{return 1;}}return 42;}}"));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    let raw = program.witness.raw_program();
    assert_eq!(raw.builtins.process().count(), 2);
    assert_eq!(raw.builtins.extra_functions(), 0);
    assert_eq!(raw.functions.len(), 3);
    for (rank, item) in termination::Function::ALL.into_iter().enumerate() {
        let id = program
            .witness
            .experimental_function(item.operation())
            .unwrap();
        assert_eq!(id.0, 1 + rank);
        assert_eq!(raw.functions[id.0].result, ValueTy::Scalar(hir::Ty::Bool));
        assert!(raw.functions[id.0].parameters.is_empty());
        assert_eq!(
            raw.builtins.experimental_function_rank(item.operation()),
            None
        );
    }
    assert_eq!(
        program.run(entry, project.sources()).unwrap_err().code,
        "E0609"
    );
    assert!(program.native_module(entry, project.sources()).is_err());
    if super::super::super::process::supported_host() {
        let module = program
            .native_module_with_execution(
                entry,
                project.sources(),
                true,
                ExecutionPolicy::Hosted,
                None,
            )
            .unwrap();
        assert!(module.contains("@__oxid_process_termination = constant i1 true\n"));
        assert!(module.contains("call i32 @__oxid_enable_termination_notifications()"));
        assert!(module.contains("call void @__oxid_termination_scope_end()"));
    }
}

#[test]
fn process_opcodes_cannot_forge_old_family_authority_or_a_third_descriptor() {
    let project = project(&format!("{IMPORTS}fn main()->i32{{return 42;}}"));
    let (program, _) = check_enum_source(SourceOwner::project(&project)).unwrap();
    let original = program.witness.raw_program();
    let enable = program
        .witness
        .experimental_function(experimental::Function::EnableTermination)
        .unwrap();
    for mutation in 0..5 {
        let mut raw = duplicate_without_records(original);
        match mutation {
            0 => raw.builtins = BuiltinSet::None,
            1 => {
                raw.builtins = BuiltinSet::None.admit(BuiltinItem::Experimental(
                    experimental::Item::EnableTermination,
                ))
            }
            2 => {
                raw.functions[enable.0].blocks[0].statements[0].kind =
                    OwnedInstruction::Experimental {
                        operation: experimental::Function::TerminationRequested,
                        destination: CallResult::Scalar(LocalId(0)),
                    }
            }
            3 => {
                let mut duplicate = raw.functions.last().unwrap().clone();
                duplicate.id = hir::DefId(raw.functions.len());
                raw.functions.push(duplicate);
            }
            4 => raw.functions[enable.0].locals[0].ty = hir::Ty::I32,
            _ => unreachable!(),
        }
        assert!(
            verified::verify_owned(raw, project.sources()).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn process_identity_survives_generic_calls_and_concrete_copy_vector_suffixes() {
    for body in [
        "fn poll<T:Copy>(value:T)->bool{return termination_requested();}fn main()->i32{if poll(7){return 1;}return 42;}",
        "use alloc::vec::new;use alloc::vec::len;fn main()->i32{let values=new::<i32>();if termination_requested(){return 1;}return len(&values);}",
    ] {
        let project = project(&format!("{IMPORTS}{body}"));
        let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
        let raw = program.witness.raw_program();
        let first = raw.functions.len() - raw.builtins.process().count();
        assert_eq!(program.witness.experimental_function(experimental::Function::EnableTermination).unwrap().0, first);
        assert_eq!(program.witness.experimental_function(experimental::Function::TerminationRequested).unwrap().0, first + 1);
        if super::super::super::process::supported_host() {
            program.native_module_with_execution(entry, project.sources(), true, ExecutionPolicy::Hosted, None).unwrap();
        }
    }
}

#[test]
fn process_descriptors_follow_the_separately_authenticated_owned_vector_suffix() {
    let source = format!("{IMPORTS}use alloc::bytes::ByteBuf;use alloc::bytes::new as bytes_new;use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::try_reserve;use alloc::vec::push_reserved;use alloc::vec::pop_last;struct Message{{number:i32,bytes:ByteBuf}}fn take_number(message:Message)->i32{{return message.number;}}fn main()->i32{{let mut values=new::<Message>();try_reserve(&mut values,1);push_reserved(&mut values,Message{{number:42,bytes:bytes_new()}});if termination_requested(){{return 1;}}return take_number(pop_last(&mut values));}}");
    let project = project(&source);
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
    let entry = typed.entry();
    let program = check_owned_vector_typed(&typed).unwrap();
    let raw = program.witness.raw_program();
    assert!(program.witness.uses_owned_vectors());
    let base = raw.functions.len() - 2;
    assert_eq!(
        program
            .witness
            .experimental_function(experimental::Function::EnableTermination),
        Some(hir::DefId(base))
    );
    assert_eq!(
        program
            .witness
            .experimental_function(experimental::Function::TerminationRequested),
        Some(hir::DefId(base + 1))
    );
    if super::super::super::process::supported_host() {
        program
            .native_module_with_execution(
                entry,
                project.sources(),
                true,
                ExecutionPolicy::Hosted,
                None,
            )
            .unwrap();
    }
}

#[test]
fn canonical_process_lookup_is_bounded_paid_and_uses_the_first_family_import() {
    let project = project("use std::io::ReadStatus;use std::process::termination_requested as pending;use std::process::enable_termination_notifications as enable;fn main()->i32{return 42;}");
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_originals(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap();
    for item in termination::Function::ALL {
        let measured = WorkMeter::default();
        measured.enable_observation();
        let anchor = index.process_anchor(item, &measured).unwrap();
        assert_eq!(
            index.sources().text(anchor).unwrap(),
            "termination_requested"
        );
        let rows = measured
            .events
            .borrow()
            .iter()
            .filter(|event| event.operation == "process canonical import row")
            .count();
        assert_eq!(rows, 2, "every preceding import row must be debited");
        let exact = WorkMeter::new(measured.used());
        assert_eq!(index.process_anchor(item, &exact).unwrap(), anchor);
        assert_eq!(exact.used(), measured.used());
        let one_less = WorkMeter::new(measured.used() - 1);
        assert_eq!(
            index.process_anchor(item, &one_less).unwrap_err().code,
            "E0400"
        );
    }
}

#[test]
fn vm_child() {
    let Ok(mode) = std::env::var("OXID_TERMINATION_VM_CHILD") else {
        return;
    };
    if !super::super::super::process::supported_host() {
        return;
    }
    let body = if mode == "failure" {
        "fn main()->i32{let enabled=enable_termination_notifications();while enabled{}return 42;}"
    } else {
        "fn main()->i32{let enabled=enable_termination_notifications();if enabled{return 42;}return 7;}"
    };
    let project = project(&format!("{IMPORTS}{body}"));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    #[cfg(unix)]
    let previous = {
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = prior_handler as *const () as usize;
        action.sa_flags = libc::SA_RESTART;
        assert_eq!(unsafe { libc::sigemptyset(&mut action.sa_mask) }, 0);
        assert_eq!(
            unsafe { libc::sigaddset(&mut action.sa_mask, libc::SIGUSR1) },
            0
        );
        for signal in [libc::SIGINT, libc::SIGTERM] {
            assert_eq!(
                unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) },
                0
            );
        }
        [disposition(libc::SIGINT), disposition(libc::SIGTERM)]
    };
    let run = |fuel, bytes| {
        crate::program_termination::reset_observation();
        let result = execute::run_process_limits(
            &program.witness,
            entry,
            execute::Limits {
                fuel,
                bytes,
                ..Default::default()
            },
        );
        #[cfg(unix)]
        {
            same_disposition(libc::SIGINT, &previous[0]);
            same_disposition(libc::SIGTERM, &previous[1]);
        }
        (result, crate::program_termination::observation())
    };
    match mode.as_str() {
        "fuel" => {
            let mut first_install = None;
            for fuel in 0..256 {
                let (result, (installs, deliveries)) = run(fuel, execute::Limits::default().bytes);
                if installs != 0 {
                    assert_eq!(installs, 2);
                    assert_eq!(
                        deliveries, 1,
                        "installation cannot precede the prepared caller return"
                    );
                    let _ = first_install.get_or_insert(fuel);
                } else {
                    assert_eq!(deliveries, 0);
                    assert!(first_install.is_none());
                }
                if let Ok(value) = result {
                    assert_eq!(value, Scalar::I32(42));
                }
                assert!(!crate::program_termination::enable());
                assert!(!crate::program_termination::requested());
            }
            let first = first_install.expect("actual installer must execute");
            assert!(first > 1);
            assert_eq!(run(first - 1, execute::Limits::default().bytes).1, (0, 0));
        }
        "memory" => {
            let maximum = execute::Limits::default();
            let (mut low, mut high) = (0, maximum.bytes);
            assert_eq!(run(maximum.fuel, high).0.unwrap(), Scalar::I32(42));
            while low < high {
                let middle = low + (high - low) / 2;
                if run(maximum.fuel, middle).0.is_ok() {
                    high = middle;
                } else {
                    low = middle + 1;
                }
            }
            assert!(low > crate::program_termination::control_bytes());
            assert_eq!(run(maximum.fuel, low).0.unwrap(), Scalar::I32(42));
            let (refused, observation) = run(maximum.fuel, low - 1);
            assert!(refused.is_err());
            assert_eq!(observation, (0, 0));
            assert!(!crate::program_termination::enable());
        }
        "failure" => {
            let (result, observed) = run(256, execute::Limits::default().bytes);
            assert!(
                result.is_err(),
                "the source loop must exhaust its real VM fuel"
            );
            assert_eq!(observed, (2, 1));
            assert!(!crate::program_termination::enable());
            assert!(!crate::program_termination::requested());
        }
        _ => panic!("unknown VM child mode"),
    }
}

#[test]
fn process_vm_exact_and_one_less_admission_are_observed_in_children() {
    if !super::super::super::process::supported_host() {
        return;
    }
    for mode in ["fuel", "memory", "failure"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "frontend::oir::owned::source::program::termination_tests::vm_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("OXID_TERMINATION_VM_CHILD", mode)
            .output()
            .unwrap();
        assert!(output.status.success(), "{mode}: {output:?}");
    }
}

#[test]
fn absent_process_family_never_traverses_source_vm_or_native_catalogs() {
    use super::super::super::termination_queries;
    for (text, expected) in [
        ("use std::io::ReadStatus;fn main()->i32{return 42;}", 42),
        ("use alloc::bytes::new;use alloc::bytes::len;fn main()->i32{let bytes=new();return len(&bytes);}", 0),
        ("use std::io::ReadStatus;fn keep<T:Copy>(value:T)->T{return value;}fn main()->i32{return keep(42);}", 42),
        ("use alloc::vec::new;use alloc::vec::len;fn main()->i32{let values=new::<i32>();return len(&values);}", 0),
    ] {
        let project = project(text);
        let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
        assert!(program.witness.raw_program().builtins.process().is_empty());
        termination_queries::reset_catalogs();
        assert_eq!(program.run(entry, project.sources()).unwrap(), Scalar::I32(expected));
        assert_eq!(termination_queries::catalogs(), [0, 0, 0]);
        let module = program.native_module(entry, project.sources()).unwrap();
        assert!(!module.contains("@__oxid_process_termination"));
        assert_eq!(termination_queries::catalogs(), [0, 0, 0]);
    }
}

#[test]
fn separately_present_process_neighbors_still_traverse_the_guarded_catalogs() {
    use super::super::super::{native, termination_queries};
    for (imports, expected) in [
        (
            "use std::process::enable_termination_notifications;",
            1usize,
        ),
        ("use std::process::termination_requested;", 2usize),
        (IMPORTS, 1usize),
    ] {
        let project = project(&format!("{imports}fn main()->i32{{return 42;}}"));
        let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
        termination_queries::reset_catalogs();
        assert_eq!(
            program.run(entry, project.sources()).unwrap_err().code,
            "E0609"
        );
        assert_eq!(termination_queries::catalogs(), [expected, 0, 0]);
        termination_queries::reset_catalogs();
        let refusal = execute::run(&program.witness, entry).unwrap_err();
        assert_eq!(refusal.diagnostic(project.sources()).code, "E0609");
        assert_eq!(termination_queries::catalogs(), [0, expected, 0]);
        termination_queries::reset_catalogs();
        assert_eq!(
            native::native_module(&program.witness, entry, project.sources())
                .unwrap_err()
                .code,
            "E0700"
        );
        assert_eq!(termination_queries::catalogs(), [0, 0, expected]);
    }
}

#[test]
fn shared_selector_metadata_is_paid_even_for_an_empty_builtin_inventory() {
    use super::super::super::{budget as raw_budget, termination_queries};
    let shared = crate::frontend::builtin_catalog::termination::shared_selector_bytes()
        + termination_queries::raw_bytes();
    assert!(shared > 0);
    let mut counts = raw_budget::ProgramCounts::default();
    raw_budget::account_builtin_descriptors(
        BuiltinSet::None,
        raw_budget::Limits::DEFAULT,
        &mut counts,
    )
    .unwrap();
    assert_eq!(counts.usage().metadata_bytes, shared);
    assert_eq!(
        counts.usage().work,
        0,
        "an absent catalog has no descriptor-row work"
    );
    let exact = raw_budget::Limits {
        metadata: shared,
        ..raw_budget::Limits::DEFAULT
    };
    let mut exact_counts = raw_budget::ProgramCounts::default();
    raw_budget::account_builtin_descriptors(BuiltinSet::None, exact, &mut exact_counts).unwrap();
    let mut short_counts = raw_budget::ProgramCounts::default();
    assert!(raw_budget::account_builtin_descriptors(
        BuiltinSet::None,
        raw_budget::Limits {
            metadata: shared - 1,
            ..exact
        },
        &mut short_counts
    )
    .is_err());
}

#[test]
fn ordinary_plan_fixed_metadata_includes_its_real_shared_query_roles() {
    use super::super::super::{plan, termination_queries, verified};
    let project = project("use std::io::ReadStatus;fn main()->i32{return 42;}");
    let (program, _) = check_enum_source(SourceOwner::project(&project)).unwrap();
    let raw = duplicate_without_records(program.witness.raw_program());
    assert_eq!(raw.functions.len(), 1);
    assert!(
        raw.functions[0].owners.is_empty()
            && raw.functions[0].references.is_empty()
            && raw.functions[0].calls.is_empty()
            && raw.functions[0].loans.is_empty()
    );
    let witness = verified::verify_owned(raw, project.sources()).unwrap();
    let plan = plan::ExecutionPlan::build(&witness).unwrap();
    assert!(!plan.has_termination());
    assert_eq!(plan.process_delivery_cost(hir::DefId(0)), None);
    assert_eq!(
        plan.metadata_bytes(),
        std::mem::size_of::<plan::FunctionPlan>()
            + termination_queries::plan_bytes::<verified::VerifiedOwnedProgram>()
    );
    assert!(plan::ExecutionPlan::build_with_test_limit(&witness, plan.metadata_bytes()).is_ok());
    assert_eq!(
        plan::ExecutionPlan::build_with_test_limit(&witness, plan.metadata_bytes() - 1)
            .unwrap_err()
            .name,
        "owned plan bytes"
    );
    println!("termination shared selectors frontend={} raw={} plan={} family={} emission measurement required",
        crate::frontend::builtin_catalog::termination::shared_selector_bytes(),
        termination_queries::raw_bytes(), termination_queries::plan_bytes::<verified::VerifiedOwnedProgram>(),
        crate::frontend::builtin_catalog::termination::compiler_carrier_bytes());
}
