//! Constructor inference through the public project commands.
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
            "oxid-constructor-inference-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), "[project]\nname=\"inference\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n").unwrap();
        Self(root)
    }
    fn invoke(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(arguments)
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
fn returns(source: &str) {
    let output = Project::new(source).invoke(&["run"]);
    assert!(output.status.success(), "{source}: {output:?}");
    assert_eq!(output.stdout, b"42\n", "{source}");
    assert!(output.stderr.is_empty(), "{output:?}");
}
fn rejected(source: &str, message: &str) {
    let output = Project::new(source).invoke(&["check"]);
    assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("E0300") && error.contains(message) && !error.contains("E0500"),
        "{source}: {output:?}"
    );
}

#[test]
fn record_arguments_follow_field_values_and_names_in_any_order() {
    returns("struct Cell<T>{value:T}fn main()->i32{let cell=Cell{value:42};return cell.value;}");
    returns("struct Pair<A,B>{first:A,second:B}fn main()->i32{let pair=Pair{second:42,first:'雪'};if pair.first=='雪'{return pair.second;}return 1;}");
    returns("struct Pair<T>{first:T,second:T}fn main()->i32{let pair=Pair{second:22,first:20};return pair.first+pair.second;}");
}

#[test]
fn enum_arguments_follow_the_selected_payload() {
    returns("enum Option<T>{None,Some(T)}fn main()->i32{let value=Option::Some(42);match value{Option::None=>{return 1;},Option::Some(n)=>{return n;},}}");
    returns("enum Result<T>{Value(T),Error(i32)}fn main()->i32{let value=Result::Value(42);match value{Result::Value(n)=>{return n;},Result::Error(code)=>{return code;},}}");
}

#[test]
fn nested_constructors_infer_complete_nominal_arguments() {
    returns("enum Option<T>{None,Some(T)}struct Cell<T>{value:T}fn main()->i32{let cell=Cell{value:Option::Some(42)};let Cell{value}=cell;match value{Option::None=>{return 1;},Option::Some(n)=>{return n;},}}");
    returns("enum Option<T>{None,Some(T)}struct Cell<T>{value:T}fn main()->i32{let outer=Option::Some(Cell{value:Option::Some(42)});match outer{Option::None=>{return 1;},Option::Some(cell)=>{let Cell{value}=cell;match value{Option::None=>{return 2;},Option::Some(n)=>{return n;},}},}}");
}

#[test]
fn inferred_constructors_check_opaque_generic_function_bodies() {
    returns("enum Option<T>{None,Some(T)}struct Cell<T>{value:T}fn wrap<T>(value:T)->Cell<Option<T>>{return Cell{value:Option::Some(value)};}fn main()->i32{let cell=wrap(42);let Cell{value}=cell;match value{Option::None=>{return 1;},Option::Some(n)=>{return n;},}}");
}

#[test]
fn inference_moves_owned_payloads_without_copy_requirements() {
    returns("use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;enum Option<T>{None,Some(T)}struct Cell<T>{value:T}fn wrap<T>(value:T)->Cell<Option<T>>{return Cell{value:Option::Some(value)};}fn main()->i32{let cell=wrap(new());let Cell{value}=cell;match value{Option::None=>{return 1;},Option::Some(bytes)=>{return len(&bytes)+42;},}}");
}

#[test]
fn inference_keeps_copy_bounds_and_explicit_arguments_authoritative() {
    returns(
        "struct Cell<T:Copy>{value:T}fn main()->i32{let cell=Cell{value:42};return cell.value;}",
    );
    rejected("use alloc::bytes::ByteBuf;use alloc::bytes::new;struct Cell<T:Copy>{value:T}fn main()->i32{let cell=Cell{value:new()};return 0;}", "Copy");
    rejected(
        "struct Cell<T>{value:T}fn main()->i32{let cell=Cell::<bool>{value:42};return 0;}",
        "mismatch",
    );
    rejected(
        "enum Option<T>{None,Some(T)}fn main()->i32{let value=Option::<bool>::Some(42);return 0;}",
        "mismatch",
    );
}

#[test]
fn ambiguous_and_conflicting_inference_reports_a_type_error() {
    for source in [
        "struct Empty<T>{}fn main()->i32{let empty=Empty{};return 0;}",
        "enum Option<T>{None,Some(T)}fn main()->i32{let value=Option::None;return 0;}",
        "struct Tagged<T,U>{value:T}fn main()->i32{let value=Tagged{value:42};return 0;}",
    ] {
        rejected(source, "infer");
    }
    rejected("struct Pair<T>{first:T,second:T}fn main()->i32{let pair=Pair{first:42,second:true};return 0;}", "incompatible");
    rejected(
        "enum Result<T,E>{Ok(T),Err(E)}fn main()->i32{let value=Result::Ok(42);return 0;}",
        "infer",
    );
    rejected(
        "enum Option<T>{None,Some(T)}fn main()->i32{let value:Option<i32>=Option::None;return 0;}",
        "infer",
    );
    rejected("struct Pair<T>{first:T,second:T}fn main()->i32{let value=Pair{first:42,first:42};return 0;}", "duplicate");
    rejected(
        "struct Cell<T>{value:T}fn main()->i32{let value=Cell{unknown:42};return 0;}",
        "unknown",
    );
}

#[test]
fn inferred_arguments_preserve_arrays_unicode_and_wide_values() {
    returns("struct Cell<T>{value:T}fn main()->i32{let cell=Cell{value:['人','雪']};let Cell{value:letters}=cell;if letters[1]=='雪'{return 42;}return 1;}");
    returns("struct Pair<A,B>{first:A,second:B}fn main()->i32{let pair=Pair{first:340282366920938463463374607431768211455u128,second:0.25f64};if pair.first==340282366920938463463374607431768211455u128&&pair.second==0.25f64{return 42;}return 1;}");
    rejected("struct Left{value:i32}struct Right{value:i32}struct Cell<T>{value:T}fn need(cell:Cell<Left>)->i32{return cell.value.value;}fn main()->i32{return need(Cell{value:Right{value:42}});}","mismatch");
}

const ORDERED: &str = "struct Counter{value:i32}struct Pair<T>{first:T,second:T}fn next(counter:&mut Counter)->i32{counter.value=counter.value+1;return counter.value;}fn main()->i32{let mut counter=Counter{value:19};let pair=Pair{second:next(&mut counter),first:next(&mut counter)};if pair.second==20&&pair.first==21&&counter.value==21{return 42;}return 1;}";
const NESTED: &str = "enum Option<T>{None,Some(T)}struct Cell<T>{value:T}fn wrap<T>(value:T)->Cell<Option<T>>{return Cell{value:Option::Some(value)};}fn main()->i32{let cell=wrap(['人','雪']);let Cell{value}=cell;match value{Option::None=>{return 1;},Option::Some(letters)=>{if letters[1]=='雪'{return 42;}return 2;},}}";
const OWNED: &str = "use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::try_push;use alloc::bytes::len;enum Option<T>{None,Some(T)}struct Cell<T>{value:T}fn wrap<T>(value:T)->Cell<Option<T>>{return Cell{value:Option::Some(value)};}fn main()->i32{let mut bytes=new();try_push(&mut bytes,255u8);let cell=wrap(bytes);let Cell{value}=cell;match value{Option::None=>{return 1;},Option::Some(found)=>{return len(&found)+41;},}}";

#[test]
fn constructor_fields_execute_once_in_written_order() {
    returns(ORDERED);
    returns(NESTED);
    returns(OWNED);
}

#[test]
fn imported_aliases_keep_definition_identity_and_private_fields() {
    let project=Project::new("mod model;use crate::model::Cell as Boxed;use crate::model::Option as Maybe;fn main()->i32{let cell=Boxed{value:Maybe::Some(42)};let Boxed{value}=cell;match value{Maybe::None=>{return 1;},Maybe::Some(number)=>{return number;},}}");
    let module = project.0.join("model.ox");
    fs::write(
        &module,
        "pub enum Option<T>{None,Some(T)}pub struct Cell<T>{pub value:T}",
    )
    .unwrap();
    let output = project.invoke(&["run"]);
    if !cfg!(any(
        target_os = "linux",
        target_os = "macos",
        all(
            target_os = "windows",
            target_arch = "x86_64",
            target_pointer_width = "64",
            target_endian = "little"
        )
    )) {
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("error[E0005] (source):"),
            "{output:?}"
        );
        return;
    }
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    assert!(output.stderr.is_empty());
    fs::write(
        &module,
        "pub enum Option<T>{None,Some(T)}pub struct Cell<T>{value:T}",
    )
    .unwrap();
    let output = project.invoke(&["check"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("private"),
        "{output:?}"
    );
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn deployed(source: &str, trap: bool) {
    for policy in ["bounded", "hosted"] {
        let project = Project::new(source);
        fs::write(project.0.join("oxid.toml"),format!("[project]\nname=\"inference\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
        let deployment = project.0.join("deployed");
        fs::create_dir(&deployment).unwrap();
        for profile in ["dev", "release"] {
            let output = project.invoke(&["build", "--profile", profile]);
            assert!(
                output.status.success(),
                "{source} {policy} {profile}: {output:?}"
            );
            fs::copy(
                project.0.join(".oxid/bin/inference"),
                deployment.join(profile),
            )
            .unwrap();
        }
        for name in ["main.ox", "oxid.toml"] {
            fs::remove_file(project.0.join(name)).unwrap();
        }
        fs::remove_dir_all(project.0.join(".oxid")).unwrap();
        for profile in ["dev", "release"] {
            let output = Command::new(deployment.join(profile))
                .env_clear()
                .current_dir(&deployment)
                .output()
                .unwrap();
            if trap {
                assert_eq!(
                    output.status.code(),
                    Some(1),
                    "{policy} {profile}: {output:?}"
                );
                assert!(output.stdout.is_empty());
                assert!(
                    String::from_utf8_lossy(&output.stderr).contains("E0607"),
                    "{output:?}"
                );
            } else {
                assert_eq!(
                    (output.status.code(), output.stdout, output.stderr),
                    (Some(0), b"42\n".to_vec(), Vec::new()),
                    "{policy} {profile}"
                );
            }
        }
    }
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]
fn native_inferred_constructors_run_without_source_files() {
    for source in [ORDERED, NESTED, OWNED] {
        deployed(source, false);
    }
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]
fn native_inferred_owned_constructors_report_later_traps() {
    deployed("use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::try_push;enum Option<T>{None,Some(T)}struct Cell<T>{value:T}fn main()->i32{let mut bytes=new();try_push(&mut bytes,7u8);let cell=Cell{value:Option::Some(bytes)};let zero=0;return 1/zero;}",true);
}
