//! Arguments use ordinary source signatures and the prepared byte delivery.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use super::*;

const IMPORTS: &str = "use std::env::arg_count;use std::env::arg_bytes;use std::env::ArgStatus;";

#[test]
fn arguments_without_an_installed_context_are_empty_in_embedded_execution() {
    let project = super::project(&format!("{IMPORTS}fn main()->i32{{let mut bytes=b\"keep\";let status=arg_bytes(0,&mut bytes);match status{{ArgStatus::Missing=>{{return arg_count();}},ArgStatus::Copied(n)=>{{return -1;}},ArgStatus::TooSmall(n)=>{{return -2;}},ArgStatus::TooLong=>{{return -3;}},}}}}"));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    assert_eq!(
        program.run(entry, project.sources()).unwrap(),
        Scalar::I32(0)
    );
    let module = program.native_module(entry, project.sources()).unwrap();
    assert!(module.contains("define i32 @main(i32 %argc, ptr %argv)"));
    assert!(module.contains("@__oxid_arg_bytes"));
}

#[test]
fn argument_status_moves_through_wrappers_and_borrows_a_growable_buffer() {
    let _scope = crate::program_arguments::install(vec!["oxid".into(), "hello".into()]).unwrap();
    let project = super::project(&format!("{IMPORTS}use alloc::bytes::new;use alloc::bytes::try_resize;use alloc::bytes::GrowStatus;
fn wrap(index:i32,bytes:&mut [u8])->ArgStatus{{return arg_bytes(index,&mut *bytes);}}
fn score(status:ArgStatus)->i32{{match status{{ArgStatus::Copied(n)=>{{return n;}},ArgStatus::TooSmall(n)=>{{return -1;}},ArgStatus::Missing=>{{return -2;}},ArgStatus::TooLong=>{{return -3;}},}}}}
fn inspect(bytes:&[u8])->i32{{let first=bytes[0];let tail=bytes[5];return first.to_i32()+tail.to_i32();}}
fn main()->i32{{let mut bytes=new();let zero=0;let resized=try_resize(&mut bytes,8,zero.to_u8_checked());match resized{{GrowStatus::Ok=>{{}},GrowStatus::CapacityOverflow=>{{return -4;}},GrowStatus::AllocationFailed=>{{return -5;}},GrowStatus::MemoryLimit=>{{return -6;}},}}let status=wrap(1,&mut bytes);let length=score(status);return length+inspect(&bytes);}}"));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    assert_eq!(
        program.run(entry, project.sources()).unwrap(),
        Scalar::I32(109)
    );
    program.native_module(entry, project.sources()).unwrap();
}

#[test]
fn argument_prepared_delivery_refusals_do_not_commit_a_status_or_issue_io() {
    use binary_io::testing::{DeliveryFault, Script};
    let _scope = crate::program_arguments::install(vec!["oxid".into(), "hello".into()]).unwrap();
    let project = super::project(&format!("{IMPORTS}fn main()->i32{{let mut bytes=b\"xxxxxx\";let status=arg_bytes(1,&mut bytes);return 0;}}"));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    for fault in [
        DeliveryFault::LocalGeneration,
        DeliveryFault::DestinationState,
        DeliveryFault::DestinationGeneration,
        DeliveryFault::LoanRelease,
    ] {
        let (result, script) = binary_io::testing::capture(
            Script {
                delivery_fault: Some(fault),
                ..Default::default()
            },
            || program.run(entry, project.sources()),
        );
        assert!(result.is_err(), "{fault:?}");
        assert!(script.fault_applied);
        assert_eq!(
            (script.deliveries, script.read_calls, script.write_calls),
            (0, 0, 0)
        );
    }
}

#[test]
fn argument_copy_and_immediate_return_share_one_fuel_admission() {
    let _scope = crate::program_arguments::install(vec!["oxid".into(), "hello".into()]).unwrap();
    let project = super::project(&format!(
        "{IMPORTS}fn main()->i32{{let mut bytes=b\"xxxxxx\";arg_bytes(1,&mut bytes);return 1/0;}}"
    ));
    let (program, entry) = check_enum_source(SourceOwner::project(&project)).unwrap();
    let mut first_delivery = None;
    let mut reached_caller_error = false;
    for fuel in 0..256 {
        let (result, script) = binary_io::testing::capture(Default::default(), || {
            execute::run_limits(
                &program.witness,
                entry,
                execute::Limits {
                    fuel,
                    ..Default::default()
                },
            )
        });
        let code = result.unwrap_err().diagnostic(project.sources()).code;
        assert!(matches!(code, "E0601" | "E0607"), "{code}");
        assert_eq!((script.read_calls, script.write_calls), (0, 0));
        if script.deliveries == 1 {
            first_delivery.get_or_insert(fuel);
        } else {
            assert!(first_delivery.is_none());
        }
        if code == "E0607" {
            assert_eq!(script.deliveries, 1);
            reached_caller_error = true;
        }
    }
    assert!(first_delivery.is_some_and(|fuel| fuel > 11));
    assert!(reached_caller_error);
}
