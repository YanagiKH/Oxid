//! Bit operations preserve scalar storage and admit scratch before argument I/O.
use super::super::{execute, plan};
use super::resource_fixtures::checked_enum_project;
use crate::frontend::build_config::ExecutionPolicy;

#[test]
fn integer_bit_cells_keep_their_original_physical_representation() {
    assert_eq!(std::mem::size_of::<plan::ExecutionPlan<'_>>(), 40);
    for (ty, body, width) in [
        ("u8", "let n=1;let a=n.to_u8_checked();return a<<1;", 8),
        ("i32", "return ~0;", 8),
        ("i64", "return 1i64<<1;", 16),
        ("u128", "return 1u128<<127;", 32),
    ] {
        let checked = checked_enum_project(&format!("enum Tag{{One}}fn main()->{ty}{{{body}}}"));
        let plan = plan::ExecutionPlan::build(&checked.witness).unwrap();
        assert!(plan.uses_bitwise());
        let usage = plan.function(checked.entry).usage();
        assert_eq!(usage.owners, 0);
        assert_eq!(
            usage.reference_bytes,
            (usage.scalar_slots + usage.arguments) * width
        );
        assert_eq!(plan.uses_quad_scalars(), width == 32);
    }
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn integer_bit_exact_memory_admission_precedes_argument_delivery() {
    use super::super::binary_io;
    let _scope = crate::program_arguments::install(vec!["oxid".into(), "hello".into()]).unwrap();
    for (ty, body) in [
        ("i32", "let n=1;"),
        ("i64", "let n=1i64;"),
        ("u128", "let n=1u128;"),
    ] {
        let checked=checked_enum_project(&format!("use std::env::arg_bytes;use std::env::ArgStatus;fn main()->{ty}{{{body}let mut bytes=b\"xxxxxx\";arg_bytes(1,&mut bytes);return n<<1;}}"));
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
            assert_eq!(value.unwrap().to_string(), "2");
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
}
