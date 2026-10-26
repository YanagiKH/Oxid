//! Integer range iteration through ordinary typed project commands.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-range-for-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), "[project]\nname=\"ranges\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n").unwrap();
        Self(root)
    }
    fn invoke(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn returns(source: &str, expected: &[u8]) {
    let result = Project::new(source).invoke(&["run"]);
    assert!(result.status.success(), "{source}: {result:?}");
    assert_eq!(result.stdout, expected);
    assert!(result.stderr.is_empty(), "{result:?}");
}

const SUM: &str = "fn main()->i32{let mut sum=0;for index in 0..7{sum=sum+index;}return sum;}";
const ORDER: &str = "struct State{value:i32}fn first(state:&mut State)->i32{state.value=state.value*10+1;return 1;}fn last(state:&mut State)->i32{state.value=state.value*10+2;return 4;}fn main()->i32{let mut state=State{value:0};let mut sum=0;for index in first(&mut state)..last(&mut state){sum=sum+index;}return state.value+sum;}";
const TRANSFERS: &str = "fn main()->i32{let mut sum=0;for outer in 0..3{if outer==1{continue;}for inner in 0..5{if inner==1{continue;}if inner==4{break;}sum=sum+outer*10+inner;}}return sum;}";
const LIMITS: &str = "fn main()->i32{let mut total=0;for index in 2147483645..2147483647{total=total+1;}for index in -2147483648..-2147483646{total=total+1;}return total;}";
const OWNED: &str = "use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::try_push;use alloc::bytes::len;fn choose()->ByteBuf{for index in 0..3{let mut bytes=new();try_push(&mut bytes,index.to_u8_checked());if index==1{return bytes;}}return new();}fn main()->i32{let bytes=choose();return len(&bytes);}";
const SLICE: &str = "fn sum(items:&[i32])->i32{let mut value=0;for index in 0..items.len(){value=value+items[index];}return value;}fn main()->i32{let items=[10,20,12];return sum(&items);}";

fn all_integer_ranges() -> String {
    let mut program = String::new();
    let mut calls = Vec::new();
    for (ty, first, last, near, maximum, minimum) in [
        (
            "i8",
            "0i8",
            "3i8",
            "125i8",
            "127i8",
            Some(("-128i8", "-126i8")),
        ),
        ("u8", "0u8", "3u8", "253u8", "255u8", None),
        (
            "i16",
            "0i16",
            "3i16",
            "32765i16",
            "32767i16",
            Some(("-32768i16", "-32766i16")),
        ),
        ("u16", "0u16", "3u16", "65533u16", "65535u16", None),
        (
            "i32",
            "0",
            "3",
            "2147483645",
            "2147483647",
            Some(("-2147483648", "-2147483646")),
        ),
        (
            "u32",
            "0u32",
            "3u32",
            "4294967293u32",
            "4294967295u32",
            None,
        ),
        (
            "i64",
            "0i64",
            "3i64",
            "9223372036854775805i64",
            "9223372036854775807i64",
            Some(("-9223372036854775808i64", "-9223372036854775806i64")),
        ),
        (
            "u64",
            "0u64",
            "3u64",
            "18446744073709551613u64",
            "18446744073709551615u64",
            None,
        ),
        (
            "isize",
            "0isize",
            "3isize",
            "9223372036854775805isize",
            "9223372036854775807isize",
            Some(("-9223372036854775808isize", "-9223372036854775806isize")),
        ),
        (
            "usize",
            "0usize",
            "3usize",
            "18446744073709551613usize",
            "18446744073709551615usize",
            None,
        ),
        (
            "i128",
            "0i128",
            "3i128",
            "170141183460469231731687303715884105725i128",
            "170141183460469231731687303715884105727i128",
            Some((
                "-170141183460469231731687303715884105728i128",
                "-170141183460469231731687303715884105726i128",
            )),
        ),
        (
            "u128",
            "0u128",
            "3u128",
            "340282366920938463463374607431768211453u128",
            "340282366920938463463374607431768211455u128",
            None,
        ),
    ] {
        program.push_str(&format!("fn width_{ty}()->i32{{let mut count=0;"));
        for (first, last) in [(first, last), (near, maximum)].into_iter().chain(minimum) {
            program.push_str(&format!(
                "for index in {first}..{last}{{let value:{ty}=index;count=count+1;}}"
            ));
        }
        program.push_str("return count;}");
        calls.push(format!("width_{ty}()"));
    }
    program.push_str(&format!("fn main()->i32{{return {};}}", calls.join("+")));
    program
}

#[test]
fn all_integer_types_preserve_index_types_and_extreme_boundaries() {
    returns(&all_integer_ranges(), b"72\n");
}

#[test]
fn ranges_sum_exclusive_integer_indexes() {
    returns(SUM, b"21\n");
}

#[test]
fn empty_reversed_and_negative_ranges_have_their_expected_iterations() {
    returns("fn main()->i32{let mut sum=42;for index in 3..3{sum=0;}for index in 4..1{sum=0;}for index in -3..4{sum=sum+index;}return sum;}", b"42\n");
}

#[test]
fn bounds_are_evaluated_once_in_source_order() {
    returns(ORDER, b"18\n");
}

#[test]
fn later_changes_to_the_end_binding_do_not_change_the_snapshot() {
    returns("fn main()->i32{let mut end=4;let mut count=0;for index in 0..end{end=0;count=count+1;}return count;}", b"4\n");
}

#[test]
fn nested_break_and_continue_target_the_nearest_loop() {
    returns(TRANSFERS, b"70\n");
}

#[test]
fn extreme_exclusive_bounds_do_not_overflow_the_implicit_step() {
    returns(LIMITS, b"4\n");
}

#[test]
fn loop_scopes_release_resources_and_preserve_owned_returns() {
    returns(OWNED, b"1\n");
}

#[test]
fn ranges_iterate_borrowed_slice_indexes() {
    returns(SLICE, b"42\n");
}

#[test]
fn generic_function_bodies_can_use_range_iteration() {
    returns("fn repeat<T:Copy>(value:T,count:i32)->T{for index in 0..count{let copy=value;}return value;}fn main()->i32{return repeat(42,3);}", b"42\n");
}

#[test]
fn invalid_bounds_and_index_uses_fail_without_building_an_artifact() {
    for (source, message) in [
        ("fn main()->i32{for index in true..3{}return 0;}", "type"),
        ("fn main()->i32{for index in 0..3i64{}return 0;}", "type"),
        (
            "fn main()->i32{for index in 0..3{index=7;}return 0;}",
            "immutable",
        ),
        ("fn main()->i32{for index in 0..3{}return index;}", "index"),
        ("fn main()->i32{for index in 0..=3{}return 0;}", "parse"),
    ] {
        let project = Project::new(source);
        fs::write(project.0.join("sentinel"), b"keep").unwrap();
        let result = project.invoke(&["build"]);
        assert_eq!(result.status.code(), Some(1), "{result:?}");
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(
            error.contains(message) && !error.contains("E0500"),
            "{result:?}"
        );
        assert_eq!(fs::read(project.0.join("sentinel")).unwrap(), b"keep");
        assert!(!project.0.join(".oxid/bin/ranges").exists());
    }
}

#[test]
fn formatting_preserves_range_program_meaning() {
    let project = Project::new(TRANSFERS);
    let formatted = project.invoke(&["fmt", "main.ox"]);
    assert!(formatted.status.success(), "{formatted:?}");
    let result = project.invoke(&["run"]);
    assert!(result.status.success(), "{result:?}");
    assert_eq!(result.stdout, b"70\n");
    assert!(result.stderr.is_empty());
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]
fn native_ranges_keep_bounds_control_flow_and_cleanup_after_source_removal() {
    let integers = all_integer_ranges();
    for (source, expected) in [
        (SUM, b"21\n".as_slice()),
        (ORDER, b"18\n"),
        (TRANSFERS, b"70\n"),
        (LIMITS, b"4\n"),
        (OWNED, b"1\n"),
        (SLICE, b"42\n"),
        (integers.as_str(), b"72\n"),
    ] {
        for policy in ["bounded", "hosted"] {
            for profile in ["dev", "release"] {
                let project = Project::new(source);
                let mut manifest = fs::read_to_string(project.0.join("oxid.toml")).unwrap();
                manifest.push_str(&format!("[execution]\npolicy=\"{policy}\"\n"));
                fs::write(project.0.join("oxid.toml"), manifest).unwrap();
                let result = project.invoke(&["build", "--profile", profile]);
                assert!(result.status.success(), "{source}: {result:?}");
                fs::remove_file(project.0.join("main.ox")).unwrap();
                fs::remove_file(project.0.join("oxid.toml")).unwrap();
                let result = Command::new(project.0.join(".oxid/bin/ranges"))
                    .env_clear()
                    .output()
                    .unwrap();
                assert!(result.status.success(), "{source}: {result:?}");
                assert_eq!(result.stdout, expected);
                assert!(result.stderr.is_empty(), "{result:?}");
            }
        }
    }
}
