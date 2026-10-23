//! Physical storage and admission before observable argument delivery.
use super::super::{execute, plan};
use super::resource_fixtures::checked_enum_project;
use crate::frontend::build_config::ExecutionPolicy;

#[test]
fn integer_width_source_slots_use_the_actual_runtime_representation() {
    for (ty, literal, width, extended) in [
        ("i32", "7", 8, false),
        ("i64", "7i64", 16, false),
        ("i8", "7i8", 16, true),
        ("i16", "7i16", 16, true),
        ("u16", "7u16", 16, true),
        ("u32", "7u32", 16, true),
        ("u64", "7u64", 16, true),
        ("isize", "7isize", 16, true),
        ("usize", "7usize", 16, true),
    ] {
        let text = format!("enum Tag{{One}}fn main()->{ty}{{return {literal};}}");
        let checked = checked_enum_project(&text);
        let plan = plan::ExecutionPlan::build(&checked.witness).unwrap();
        let usage = plan.function(checked.entry).usage();
        assert_eq!(usage.owners, 0);
        assert_eq!(
            usage.reference_bytes,
            (usage.scalar_slots + usage.arguments) * width,
            "{ty}"
        );
        assert_eq!(plan.uses_integer_extension(), extended, "{ty}");
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn integer_width_source_exact_memory_admission_precedes_argument_delivery() {
    use super::super::binary_io;
    let _scope = crate::program_arguments::install(vec!["oxid".into(), "hello".into()]).unwrap();
    for (ty, literal) in [
        ("i8", "-128i8"),
        ("i16", "-32768i16"),
        ("u16", "65535u16"),
        ("u32", "4294967295u32"),
        ("u64", "18446744073709551615u64"),
        ("isize", "-9223372036854775808isize"),
        ("usize", "18446744073709551615usize"),
    ] {
        let text = format!("use std::env::arg_bytes;use std::env::ArgStatus;fn main()->{ty}{{let n={literal};let mut bytes=b\"xxxxxx\";arg_bytes(1,&mut bytes);return n;}}");
        let checked = checked_enum_project(&text);
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
                    ExecutionPolicy::Bounded,
                )
            })
        };
        let (mut low, mut high) = (0, execute::Limits::default().bytes);
        assert!(run(high).0.is_ok(), "{ty}");
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
        assert!(value.is_ok(), "{ty}: {value:?}");
        assert_eq!(delivered.deliveries, 1, "{ty}");
        let (denied, untouched) = run(low - 1);
        assert_eq!(
            denied
                .unwrap_err()
                .diagnostic(checked.project.sources())
                .code,
            "E0605",
            "{ty}"
        );
        assert_eq!(
            (
                untouched.deliveries,
                untouched.read_calls,
                untouched.write_calls
            ),
            (0, 0, 0),
            "{ty}"
        );
    }
}
