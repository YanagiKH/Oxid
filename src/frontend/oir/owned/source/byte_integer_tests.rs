//! Byte result admission retains narrow storage and precedes argument delivery.
use super::super::{execute, plan, RuntimeScalar, Scalar};
use super::resource_fixtures::checked_enum_project;
use crate::frontend::build_config::ExecutionPolicy;

#[test]
fn byte_integer_result_slots_remain_narrow_with_conditional_helper_storage() {
    for (body, helpers) in [
        ("let n=255;return n.to_u8_checked();", false),
        ("return 255u8;", true),
        ("let n=250u8;return n+5u8;", true),
    ] {
        let checked = checked_enum_project(&format!("enum Tag{{One}}fn main()->u8{{{body}}}"));
        let plan = plan::ExecutionPlan::build(&checked.witness).unwrap();
        assert!(!plan.uses_wide_scalars());
        assert_eq!(plan.uses_integer_extension(), helpers);
        let usage = plan.function(checked.entry).usage();
        assert_eq!(usage.owners, 0);
        assert_eq!(
            usage.reference_bytes,
            (usage.scalar_slots + usage.arguments) * 8
        );
        assert_eq!(
            execute::run_value(
                &checked.witness,
                Some(checked.entry),
                Default::default(),
                ExecutionPolicy::Bounded
            )
            .unwrap(),
            RuntimeScalar::Narrow(Scalar::U8(255))
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
fn byte_integer_exact_memory_limit_precedes_argument_delivery() {
    use super::super::binary_io;
    let _scope = crate::program_arguments::install(vec!["oxid".into(), "hello".into()]).unwrap();
    let checked = checked_enum_project("use std::env::arg_bytes;use std::env::ArgStatus;fn main()->u8{let n=254u8;let mut bytes=b\"xxxxxx\";arg_bytes(1,&mut bytes);return n+1u8;}");
    for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        let run = |bytes| {
            binary_io::testing::capture(Default::default(), || {
                execute::run_value(
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
        let (value, delivered) = run(low);
        assert_eq!(value.unwrap(), RuntimeScalar::Narrow(Scalar::U8(255)));
        assert_eq!(delivered.deliveries, 1);
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
