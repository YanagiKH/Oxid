//! Character storage and admission through the same sealed source witness.
use super::super::{execute, plan};
use super::resource_fixtures::checked_enum_project;
use crate::frontend::build_config::ExecutionPolicy;

#[test]
fn character_cells_and_mixed_quad_cells_keep_the_existing_carrier_sizes() {
    assert_eq!(std::mem::size_of::<plan::ExecutionPlan<'_>>(), 40);
    for (body, width) in [
        ("return '中';", 16),
        ("let n=129408u128;return n.to_char_checked();", 32),
    ] {
        let checked = checked_enum_project(&format!("enum Tag{{One}}fn main()->char{{{body}}}"));
        let plan = plan::ExecutionPlan::build(&checked.witness).unwrap();
        assert!(plan.uses_characters());
        assert!(plan.uses_wide_scalars());
        assert_eq!(plan.uses_quad_scalars(), width == 32);
        let usage = plan.function(checked.entry).usage();
        assert_eq!(
            usage.reference_bytes,
            (usage.scalar_slots + usage.arguments) * width
        );
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn character_exact_memory_admission_precedes_any_argument_delivery() {
    use super::super::binary_io;
    let _scope = crate::program_arguments::install(vec!["oxid".into(), "hello".into()]).unwrap();
    for body in [
        "let n='中';",
        "let code=20013u128;let n=code.to_char_checked();",
    ] {
        let checked = checked_enum_project(&format!("use std::env::arg_bytes;use std::env::ArgStatus;fn main()->char{{{body}let mut bytes=b\"xxxxxx\";arg_bytes(1,&mut bytes);return n;}}"));
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
            assert_eq!(value.unwrap().to_string(), "中");
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
