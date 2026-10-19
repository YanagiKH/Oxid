//! Ordinary process-source regressions for the primitive's direct return.
//! No fuel sweeps, capacity qualification or native execution is performed.
use super::*;
use binary_io::testing::{DeliveryFault, Script};

const IO: &str = "use std::io::read_some;use std::io::write_some;
use std::io::ReadSomeStatus;use std::io::WriteSomeStatus;
fn read_score(r:ReadSomeStatus)->i32{match r{ReadSomeStatus::Data(n)=>{return n;},
ReadSomeStatus::Eof=>{return 10;},ReadSomeStatus::Error(e)=>{return 20+e;},}}
fn write_score(r:WriteSomeStatus)->i32{match r{WriteSomeStatus::Written(n)=>{return n;},
WriteSomeStatus::Error(e)=>{return 20+e;},}}
fn read_wrap(p:&mut [u8])->ReadSomeStatus{return read_some(&mut *p);}
fn write_wrap(p:&[u8])->WriteSomeStatus{return write_some(&*p);}";

#[test]
fn binary_delivery_source_status_bytes_and_ordinary_wrapper_returns() {
    let project = super::project(&format!(
        "{IMPORTS}{IO}
fn main()->i32{{let mut b=new();try_resize(&mut b,4,byte(99));
let r=read_wrap(&mut b);write_some(&b);return read_score(r);}}"
    ));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    for (read, expected, bytes) in [
        (Ok(vec![0, 128, 255]), 3, vec![0, 128, 255, 99]),
        (Ok(vec![]), 10, vec![99; 4]),
        (Err(4), 24, vec![99; 4]),
        (Err(9), 29, vec![99; 4]),
    ] {
        let script = Script {
            reads: [read].into(),
            ..Default::default()
        };
        let ((result, script), ledger) = byte_buffer::testing::capture(Default::default(), || {
            binary_io::testing::capture(script, || {
                program.run_process(entry.unwrap(), project.sources())
            })
        });
        assert_eq!(result.unwrap(), Scalar::I32(expected));
        assert_eq!(script.output, bytes);
        assert_eq!(
            (script.read_calls, script.write_calls, script.deliveries),
            (1, 1, 2)
        );
        assert_eq!(ledger.before_cleanup.live_allocations, 0);
        assert_eq!(ledger.after_cleanup.live_bytes, 0);
    }
    let project = super::project(&format!(
        "{IMPORTS}{IO}
fn main()->i32{{let a=[byte(0),byte(128),byte(255)];return write_score(write_wrap(&a));}}"
    ));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    for (write, expected, bytes) in [
        (Ok(2), 2, vec![0, 128]),
        (Ok(0), 0, vec![]),
        (Err(4), 24, vec![]),
        (Err(32), 52, vec![]),
    ] {
        let (result, script) = binary_io::testing::capture(
            Script {
                writes: [write].into(),
                ..Default::default()
            },
            || program.run_process(entry.unwrap(), project.sources()),
        );
        assert_eq!(result.unwrap(), Scalar::I32(expected));
        assert_eq!(script.output, bytes);
        assert_eq!((script.write_calls, script.deliveries), (1, 1));
    }
}

#[test]
fn binary_delivery_source_ignored_nested_and_empty_results() {
    for (body, expected, calls, deliveries) in [
        ("read_some(&mut b);write_some(&b);return 0;", 0, 1, 2),
        ("return combine(read_some(&mut b),write_some(&b));", 6, 1, 2),
        ("read_some(&mut b[3..3]);write_some(&b[3..3]);return combine(read_some(&mut b),write_some(&b));", 6, 1, 4),
    ] {
        let project = super::project(&format!("{IMPORTS}{IO}
fn combine(r:ReadSomeStatus,w:WriteSomeStatus)->i32{{return read_score(r)+write_score(w);}}
fn main()->i32{{let mut b=new();try_resize(&mut b,3,byte(99));{body}}}"));
        let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
        let (result, script) = binary_io::testing::capture(
            Script { reads: [Ok(vec![0,128,255])].into(), ..Default::default() },
            || program.run_process(entry.unwrap(), project.sources()),
        );
        assert_eq!(result.unwrap(), Scalar::I32(expected));
        assert_eq!(script.output, [0,128,255]);
        assert_eq!((script.read_calls, script.write_calls, script.deliveries), (calls,calls,deliveries));
        program.native_process_module(entry.unwrap(), project.sources()).unwrap();
    }
}

#[test]
fn binary_delivery_source_preflight_faults_have_zero_external_effects() {
    for body in [
        "read_some(&mut b);return 0;",
        "write_some(&b);return 0;",
        "return read_score(read_wrap(&mut b));",
        "return write_score(write_wrap(&b));",
    ] {
        let project = super::project(&format!(
            "{IMPORTS}{IO}
fn main()->i32{{let mut b=new();try_resize(&mut b,3,byte(99));{body}}}"
        ));
        let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
        for fault in [
            DeliveryFault::LocalGeneration,
            DeliveryFault::DestinationState,
            DeliveryFault::DestinationGeneration,
            DeliveryFault::LoanRelease,
            DeliveryFault::Admission,
        ] {
            let ((result, script), ledger) =
                byte_buffer::testing::capture(Default::default(), || {
                    binary_io::testing::capture(
                        Script {
                            delivery_fault: Some(fault),
                            ..Default::default()
                        },
                        || program.run_process(entry.unwrap(), project.sources()),
                    )
                });
            let crate::frontend::oir::ProcessFailure::Diagnostic(error) = result.unwrap_err()
            else {
                panic!("unexpected setup failure");
            };
            assert_eq!(
                error.code,
                if matches!(fault, DeliveryFault::Admission) {
                    "E0601"
                } else {
                    "E0500"
                },
                "{fault:?}: {error:?}"
            );
            assert!(script.fault_applied);
            assert_eq!(
                (script.read_calls, script.write_calls, script.deliveries),
                (0, 0, 0)
            );
            assert!(script.output.is_empty());
            assert_eq!(ledger.after_cleanup.live_bytes, 0);
            assert_eq!(
                ledger.after_cleanup.allocations,
                ledger.after_cleanup.releases
            );
        }
    }
}

#[test]
fn binary_delivery_source_later_wrapper_failure_keeps_prior_effects() {
    let project = super::project(&format!(
        "{IMPORTS}{IO}
fn failing(p:&[u8])->WriteSomeStatus{{let w=write_some(&*p);let fail=1/0;return w;}}
fn main()->i32{{let a=[byte(65)];return write_score(failing(&a));}}"
    ));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    let (result, script) = binary_io::testing::capture(Default::default(), || {
        program.run_process(entry.unwrap(), project.sources())
    });
    let crate::frontend::oir::ProcessFailure::Diagnostic(error) = result.unwrap_err() else {
        panic!("unexpected setup failure");
    };
    assert_eq!(error.code, "E0607");
    assert_eq!(script.output, [65]);
    assert_eq!((script.write_calls, script.deliveries), (1, 1));
}

#[test]
fn binary_delivery_native_ir_is_canonical_and_has_no_late_return_guard() {
    use crate::frontend::builtin_catalog::experimental::Function;
    let project = super::project(&format!(
        "{IMPORTS}{IO}
fn main()->i32{{let mut b=new();try_resize(&mut b,3,byte(99));
read_some(&mut b[3..3]);return read_score(read_wrap(&mut b))+write_score(write_wrap(&b));}}"
    ));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    let plan = super::super::super::super::plan::ExecutionPlan::build(&program.witness).unwrap();
    let module = program
        .native_process_module(entry.unwrap(), project.sources())
        .unwrap();
    println!(
        "binary delivery control storage: reference={} bytes; native C carrier=16 bytes",
        execute::binary_delivery_control_bytes()
    );
    for (operation, opcode) in [(Function::ReadSome, 8), (Function::WriteSome, 9)] {
        let id = program.witness.experimental_function(operation).unwrap();
        let cost = plan.binary_delivery_cost(id).unwrap();
        let (_, body) = module
            .split_once(&format!("define internal void @__oxid_owned_fn_{}(", id.0))
            .unwrap();
        let body = body.split_once("\n}\n").unwrap().0;
        let call = body
            .lines()
            .find(|line| line.contains("call i32 @__oxid_binary_delivery"))
            .unwrap();
        assert!(call.contains(&format!("i32 {opcode},")));
        assert!(call.ends_with(&format!("ptr %fuel, ptr %result, i64 {cost})")));
        assert!(!body.contains("_g3_remaining"));
        assert!(!body.contains("_term_tag"));
        assert!(body.split_once("_operation_ok:\n").unwrap().1.trim() == "ret void");
        let term = program.witness.functions()[id.0].blocks[0]
            .terminator
            .as_ref()
            .unwrap();
        assert_eq!(cost, plan.terminator_cost(id, &term.kind));
    }
    // Ordinary source wrappers retain their independent return guards and
    // status transfer trees. Source spelling grants no delivery shortcut.
    for function in program.witness.functions().iter().take(5) {
        assert!(plan.binary_delivery_cost(function.id).is_none());
    }
    for id in [hir::DefId(3), hir::DefId(4)] {
        let (_, body) = module
            .split_once(&format!("define internal void @__oxid_owned_fn_{}(", id.0))
            .unwrap();
        let body = body.split_once("\n}\n").unwrap().0;
        assert!(body.contains("_term_tag"));
        assert!(body.contains("_exhausted = icmp ult i64"));
    }
    if let Some(directory) = std::env::var_os("OXID_BYTEBUF_IO_IR_DIR") {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            std::path::PathBuf::from(directory).join("direct-delivery.ll"),
            module,
        )
        .unwrap();
    }
}
