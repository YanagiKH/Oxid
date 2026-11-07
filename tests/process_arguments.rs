//! Program arguments stay separate from compiler options and preserve OS bytes.
#![cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use std::{
    ffi::OsString,
    fs,
    os::unix::ffi::OsStringExt,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Project(PathBuf);
impl Project {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-arguments-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), "[project]\nname=\"arguments\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"process\"\n").unwrap();
        Self(root)
    }

    fn invoke(&self, arguments: &[OsString]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(arguments)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn success(output: Output) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

const BYTES: &str = r#"
use std::env::arg_count;
use std::env::arg_bytes;
use std::env::ArgStatus;
fn copied(status:ArgStatus, expected:i32)->bool{
    match status{
        ArgStatus::Copied(count)=>{return count==expected;},
        ArgStatus::TooSmall(required)=>{return false;},
        ArgStatus::Missing=>{return false;},
        ArgStatus::TooLong=>{return false;},
    }
}
fn main()->i32{
    if arg_count()!=4{return 1;}
    let mut buffer=b"xxxx";
    let status=arg_bytes(1,&mut buffer);
    if !copied(status,4){return 2;}
    let expected=b"Oxid";
    let mut index=0;
    while index<4{
        if buffer[index]!=expected[index]{return 3;}
        index=index+1;
    }
    let mut empty=b"";
    let small=arg_bytes(1,&mut empty);
    match small{
        ArgStatus::TooSmall(required)=>{if required!=4{return 4;}},
        ArgStatus::Copied(count)=>{return 5;},
        ArgStatus::Missing=>{return 6;},
        ArgStatus::TooLong=>{return 7;},
    }
    let empty_status=arg_bytes(2,&mut empty);
    if !copied(empty_status,0){return 8;}
    let raw=arg_bytes(3,&mut buffer);
    if !copied(raw,2){return 9;}
    let tail=b"r\xFFid";
    if buffer[0]!=tail[0] || buffer[1]!=tail[1] || buffer[2]!=tail[2] || buffer[3]!=tail[3]{return 10;}
    let missing=arg_bytes(-1,&mut buffer);
    match missing{
        ArgStatus::Missing=>{},
        ArgStatus::Copied(count)=>{return 11;},
        ArgStatus::TooSmall(required)=>{return 12;},
        ArgStatus::TooLong=>{return 13;},
    }
    let past_end=arg_bytes(4,&mut buffer);
    match past_end{
        ArgStatus::Missing=>{},
        ArgStatus::Copied(count)=>{return 14;},
        ArgStatus::TooSmall(required)=>{return 15;},
        ArgStatus::TooLong=>{return 16;},
    }
    if buffer[0]!=tail[0] || buffer[1]!=tail[1] || buffer[2]!=tail[2] || buffer[3]!=tail[3]{return 17;}
    return 0;
}
"#;

fn arguments(prefix: &[&str]) -> Vec<OsString> {
    let mut result: Vec<_> = prefix.iter().map(OsString::from).collect();
    result.extend([
        OsString::from("Oxid"),
        OsString::new(),
        OsString::from_vec(vec![b'r', 255]),
    ]);
    result
}

#[test]
fn project_arguments_preserve_empty_non_utf8_and_partial_destination_bytes() {
    let project = Project::new(BYTES);
    success(project.invoke(&arguments(&["run", "--"])));
}

#[test]
fn explicit_file_arguments_follow_the_source_and_both_separator_forms() {
    let project = Project::new(BYTES);
    for prefix in [
        vec![
            "--edition=typed-preview",
            "--entry-mode=process",
            "run",
            "src/main.ox",
            "--",
        ],
        vec![
            "--edition=typed-preview",
            "--entry-mode=process",
            "run",
            "--",
            "src/main.ox",
            "--",
        ],
    ] {
        success(project.invoke(&arguments(&prefix)));
    }
}

#[test]
fn compiler_options_after_the_program_separator_are_literal_arguments() {
    let project = Project::new(
        "use std::env::arg_count; fn main()->i32{if arg_count()==2{return 0;}return 1;}",
    );
    success(project.invoke(&["run".into(), "--".into(), "--entry-mode=result".into()]));
}

#[test]
#[ignore = "requires LLVM 19.1.7 in OXID_LLVM_BIN"]
fn native_arguments_preserve_os_bytes_after_source_removal_at_both_optimizations() {
    for profile in ["dev", "release"] {
        let project = Project::new(BYTES);
        let built = project.invoke(&["build".into(), "--profile".into(), profile.into()]);
        assert!(built.status.success(), "{built:?}");
        assert!(built.stderr.is_empty(), "{built:?}");
        let executable = project.0.join("arguments");
        fs::copy(project.0.join(".oxid/bin/arguments"), &executable).unwrap();
        fs::remove_dir_all(project.0.join("src")).unwrap();
        fs::remove_dir_all(project.0.join(".oxid")).unwrap();
        fs::remove_file(project.0.join("oxid.toml")).unwrap();
        success(
            Command::new(executable)
                .args(arguments(&[]))
                .current_dir(&project.0)
                .env_clear()
                .output()
                .unwrap(),
        );
    }
}
