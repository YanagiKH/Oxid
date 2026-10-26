//! Physical full-width storage and admission before observable argument reads.
use super::super::{execute, plan, RuntimeValue};
use super::resource_fixtures::checked_enum_project;
use crate::frontend::build_config::ExecutionPolicy;
use std::mem::size_of;

#[test]
fn integer128_storage_prices_match_the_selected_carrier_and_native_scratch() {
    assert_eq!(size_of::<RuntimeValue>(), 32);
    assert_eq!(size_of::<Option<RuntimeValue>>(), 32);
    for ty in ["i128", "u128"] {
        let checked =
            checked_enum_project(&format!("enum Tag{{One}}fn main()->{ty}{{return 7{ty};}}"));
        let plan = plan::ExecutionPlan::build(&checked.witness).unwrap();
        assert!(plan.uses_quad_scalars());
        let usage = plan.function(checked.entry).usage();
        assert_eq!(
            usage.reference_bytes,
            (usage.scalar_slots + usage.arguments) * 32
        );
        assert_eq!(
            usage.native_bytes,
            (usage.scalar_slots + usage.arguments) * 16 + 32
        );
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let result = execute::run_result(
                &checked.witness,
                Some(checked.entry),
                Default::default(),
                policy,
            )
            .unwrap();
            assert_eq!(result.to_string(), "7");
        }
    }
    // An empty aggregate still needs a 128-bit codec when it moves through the
    // owner machinery; no scalar local has to advertise that requirement.
    for source in [
        "fn main()->i32{let empty:[u128;0]=[];return 0;}",
        "struct Wide{value:u128}fn main()->i32{return 0;}",
        "enum Wide{Empty,Value(i128)}fn main()->i32{return 0;}",
    ] {
        let checked = checked_enum_project(&format!("enum Tag{{One}}{source}"));
        assert!(plan::ExecutionPlan::build(&checked.witness)
            .unwrap()
            .uses_quad_scalars());
        assert_eq!(
            execute::run_result(
                &checked.witness,
                Some(checked.entry),
                Default::default(),
                ExecutionPolicy::Bounded
            )
            .unwrap()
            .to_string(),
            "0"
        );
    }
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn integer128_exact_memory_admission_precedes_argument_delivery() {
    use super::super::binary_io;
    let _scope = crate::program_arguments::install(vec!["oxid".into(), "hello".into()]).unwrap();
    let checked = checked_enum_project("use std::env::arg_bytes;use std::env::ArgStatus;fn main()->u128{let n=340282366920938463463374607431768211454u128;let mut bytes=b\"xxxxxx\";arg_bytes(1,&mut bytes);return n+1u128;}");
    for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        let run = |bytes| {
            binary_io::testing::capture(Default::default(), || {
                execute::run_result(
                    &checked.witness,
                    Some(checked.entry),
                    execute::Limits {
                        frames: 2,
                        bytes,
                        ..Default::default()
                    },
                    policy,
                )
            })
        };
        let (mut low, mut high) = (0, execute::Limits::default().bytes);
        assert!(run(high).0.is_ok());
        while low < high {
            let middle = low + (high - low) / 2;
            if run(middle).0.is_ok() {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        assert!(low > 0);
        let (value, delivery) = run(low);
        assert_eq!(value.unwrap(), RuntimeValue::U128(u128::MAX));
        assert_eq!(delivery.deliveries, 1);
        let (denied, untouched) = run(low - 1);
        assert_eq!(
            denied
                .unwrap_err()
                .diagnostic(checked.project.sources())
                .code,
            "E0605"
        );
        assert_eq!(
            (
                untouched.deliveries,
                untouched.read_calls,
                untouched.write_calls
            ),
            (0, 0, 0)
        );
    }
}
