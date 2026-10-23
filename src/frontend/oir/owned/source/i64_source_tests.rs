//! Checked source exercises full-width values through the ordinary consumers.
use super::*;
use crate::frontend::{
    build_config::ExecutionPolicy,
    project::{ProjectLimits, ProjectSources},
};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn project(text: &str) -> ProjectSources {
    let path = std::env::temp_dir().join(format!(
        "oxid-i64-source-{}-{}.ox",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).unwrap();
    let project = ProjectSources::load_typed(path.to_str().unwrap(), ProjectLimits::default());
    std::fs::remove_file(path).unwrap();
    project.unwrap()
}

fn checked(project: &ProjectSources) -> (SourceProgram, Option<hir::DefId>) {
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let facts = index::collect_originals(
        SourceOwner::project(project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap();
    let index = facts.finish(&work, &mut allocator).unwrap();
    let typed = if resolve::requires_paid_source(&index) {
        resolve::type_enum_source(&index, &work, &mut allocator).unwrap()
    } else {
        typeck::check(resolve::resolve_project(&index, &work).unwrap()).unwrap()
    };
    let entry = typed.entry();
    (check_typed(&typed).unwrap(), entry)
}

fn result(text: &str, expected: i64) {
    let project = project(text);
    let (program, entry) = checked(&project);
    for execution in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        assert_eq!(
            program
                .run_value(entry, project.sources(), execution)
                .unwrap(),
            RuntimeScalar::I64(expected)
        );
        program
            .native_module_with_execution(entry, project.sources(), false, execution, None)
            .unwrap();
    }
}

#[test]
fn i64_source_literal_extremes_leading_zeroes_and_signed_arithmetic() {
    for (literal, expected) in [
        ("9223372036854775807i64", i64::MAX),
        ("-9223372036854775808i64", i64::MIN),
        ("00000000000000000000000000000000000000000000001i64", 1),
        ("-00000000000000000000000000000000000000000000000i64", 0),
    ] {
        result(&format!("fn main()->i64{{return {literal};}}"), expected);
    }
    result(
        "fn main()->i64{let a=-7000000000i64;let b=3i64;return (a/b)*b+a%b;}",
        -7_000_000_000,
    );
}

#[test]
fn i64_source_copies_loops_nested_records_and_reborrowed_array_elements() {
    result("struct Count{value:i64}struct Data{count:Count,other:i32}fn bump(a:&mut[i64])->(){let mut i=0;while i<a.len(){a[i]=a[i]+1i64;i=i+1;}return;}fn main()->i64{let x=Data{count:Count{value:7000000000i64},other:1};let mut values=[x.count.value,-3i64];bump(&mut values);let copy=values[0];values[0]=0i64;return copy+values[1];}", 6_999_999_999);
}

#[test]
fn i64_source_first_scalar_argument_is_copied_before_a_later_argument_mutates_its_owner() {
    result("struct Count{value:i64}fn next(value:&mut Count)->i64{value.value=value.value+1i64;return value.value;}fn pair(a:i64,b:i64)->i64{return a*10i64+b;}fn main()->i64{let mut value=Count{value:7000000000i64};return pair(value.value,next(&mut value));}", 77_000_000_001);
}

#[test]
fn i64_source_enum_payload_survives_move_call_return_and_binding() {
    result("enum Value{Large(i64),Empty}fn pass(value:Value)->Value{return value;}fn make()->Value{return Value::Large(-7000000000i64);}fn main()->i64{let value=pass(make());match value{Value::Large(n)=>{return n;},Value::Empty=>{return 0i64;},}}", -7_000_000_000);
}

#[test]
fn i64_source_checked_conversions_preserve_values_and_reject_ranges() {
    result("fn main()->i64{let n=255;let byte=n.to_u8_checked();let wide=byte.to_i64();let narrow=wide.to_i32_checked();return narrow.to_i64();}", 255);
    for (body, code, name) in [
        (
            "let n=2147483648i64;return n.to_i32_checked();",
            "E0610",
            "to_i32_checked",
        ),
        (
            "let n=-1i64;let b=n.to_u8_checked();return 0;",
            "E0610",
            "to_u8_checked",
        ),
        (
            "let n=256i64;let b=n.to_u8_checked();return 0;",
            "E0610",
            "to_u8_checked",
        ),
    ] {
        let text = format!("fn main()->i32{{{body}}}");
        let project = project(&text);
        let (program, entry) = checked(&project);
        for execution in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let error = program
                .run_value(entry, project.sources(), execution)
                .unwrap_err();
            assert_eq!(error.code, code);
            let span = error.primary.unwrap();
            assert_eq!(project.sources().get(span.file).text_at(span), name);
        }
    }
}

#[test]
fn i64_source_arithmetic_errors_keep_operator_origins_in_both_policies() {
    for (body, code, operator) in [
        ("let a=9223372036854775807i64;return a+1i64;", "E0604", "+"),
        ("let a=-9223372036854775808i64;return a-1i64;", "E0604", "-"),
        ("let a=9223372036854775807i64;return a*2i64;", "E0604", "*"),
        ("let a=-9223372036854775808i64;return -a;", "E0604", "-"),
        (
            "let a=-9223372036854775808i64;return a/-1i64;",
            "E0604",
            "/",
        ),
        (
            "let a=-9223372036854775808i64;return a%-1i64;",
            "E0604",
            "%",
        ),
        ("let a=1i64;return a/0i64;", "E0607", "/"),
        ("let a=1i64;return a%0i64;", "E0607", "%"),
    ] {
        let text = format!("fn main()->i64{{{body}}}");
        let project = project(&text);
        let (program, entry) = checked(&project);
        for execution in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let error = program
                .run_value(entry, project.sources(), execution)
                .unwrap_err();
            assert_eq!(error.code, code);
            let span = error.primary.unwrap();
            assert_eq!(project.sources().get(span.file).text_at(span), operator);
        }
    }
}

#[test]
fn i64_source_full_width_slots_have_actual_physical_prices() {
    for (text, width) in [
        ("fn main()->i32{return 7;}", 8),
        ("fn main()->i64{return 7i64;}", 16),
    ] {
        let project = project(text);
        let (program, entry) = checked(&project);
        let plan = super::super::super::plan::ExecutionPlan::build(&program.witness).unwrap();
        let usage = plan.function(entry.unwrap()).usage();
        assert_eq!(usage.owners, 0);
        assert_eq!(
            usage.reference_bytes,
            (usage.scalar_slots + usage.arguments) * width
        );
    }
}

#[test]
fn i64_source_primitive_type_names_cannot_be_rebound_but_value_names_remain_available() {
    for text in [
        "struct i64{value:i32}fn main()->i32{return 0;}",
        "enum i64{Value}fn main()->i32{return 0;}",
        "struct Named{value:i32}use crate::Named as i64;fn main()->i32{return 0;}",
    ] {
        let project = project(text);
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let facts = index::collect_originals(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap();
        let errors = facts.finish(&work, &mut allocator).unwrap_err();
        assert_eq!(errors[0].code, "E0208");
        assert!(errors[0].message.contains("i64"));
    }
    let project = project("fn i64()->i32{return 7;}fn main()->i32{return i64();}");
    let (program, entry) = checked(&project);
    assert_eq!(
        program
            .run_value(entry, project.sources(), ExecutionPolicy::Bounded)
            .unwrap(),
        RuntimeScalar::Narrow(Scalar::I32(7))
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn i64_source_memory_boundary_precedes_argument_delivery() {
    use super::super::super::binary_io;
    let _scope = crate::program_arguments::install(vec!["oxid".into(), "hello".into()]).unwrap();
    let project = project("use std::env::arg_bytes;use std::env::ArgStatus;fn main()->i32{let n=1i64;let mut bytes=b\"xxxxxx\";arg_bytes(1,&mut bytes);return 0;}");
    let (program, entry) = checked(&project);
    let run = |bytes| {
        binary_io::testing::capture(Default::default(), || {
            execute::run_value(
                &program.witness,
                entry,
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
    assert_eq!(value.unwrap(), RuntimeScalar::Narrow(Scalar::I32(0)));
    assert_eq!(delivered.deliveries, 1);
    let (denied, untouched) = run(low - 1);
    assert_eq!(
        denied.unwrap_err().diagnostic(project.sources()).code,
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
