//! Actual Windows File execution through the public typed compiler workflow.
#![cfg(all(
    target_os = "windows",
    target_env = "msvc",
    target_arch = "x86_64",
    target_pointer_width = "64",
    target_endian = "little"
))]
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "oxid-windows-file-資料-😀-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("src")).unwrap();
        fs::write(path.join("oxid.toml"), "[project]\nname=\"file-copy\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"bounded\"\n").unwrap();
        fs::write(path.join("src/main.ox"), source()).unwrap();
        Self(path)
    }
    fn invoke(&self, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .args(args)
            .current_dir(&self.0)
            .env_remove("OXID_PATH");
        if let Some(llvm) = std::env::var_os("OXID_TEST_LLVM_BIN") {
            command.env("OXID_LLVM_BIN", llvm);
        }
        command.output().unwrap()
    }
    fn bytes(&self) -> PathBuf {
        self.0.join("資料-😀.bin")
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(output: Output, expected: &[u8]) {
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty(), "{output:?}");
}
fn source() -> String {
    let payload: String = (0..=255).map(|byte| format!("\\x{byte:02X}")).collect();
    r#"
use std::fs::File;use std::fs::OpenResult;use std::fs::FileStatus;
use std::fs::create;use std::fs::open_read;use std::fs::read_some;use std::fs::write_some;
use std::fs::sync_all;use std::fs::close;use std::fs::seek_absolute;
use std::io::ReadSomeStatus;use std::io::WriteSomeStatus;
fn store(file:&mut File,bytes:&[u8])->bool{
    let mut offset=0;while offset<bytes.len(){
        let result=write_some(&mut *file,&*bytes[offset..bytes.len()]);
        match result{WriteSomeStatus::Written(n)=>{if n<=0{return false;}offset=offset+n;},WriteSomeStatus::Error(code)=>{return false;},}
    }return true;
}
fn check(file:&mut File,expected:&[u8])->i32{
    let mut bytes=b"xxx";let mut offset=0;
    while true{let result=read_some(&mut *file,&mut bytes);
        match result{ReadSomeStatus::Data(n)=>{let mut i=0;while i<n{if offset>=expected.len()||bytes[i]!=expected[offset]{return -10;}i=i+1;offset=offset+1;}},
            ReadSomeStatus::Eof=>{return offset;},ReadSomeStatus::Error(code)=>{return -11;},}
    }return -12;
}
fn main()->i32{
    let path=b"資料-😀.bin";let expected=b"__PAYLOAD__";
    let opened=create(&path);
    match opened{OpenResult::Opened(value)=>{let mut file=value;
        if !store(&mut file,&expected){return -1;}
        let synced=sync_all(&mut file);match synced{FileStatus::Ok=>{},FileStatus::Error(code)=>{return -2;},}
        let closed=close(&mut file);match closed{FileStatus::Ok=>{},FileStatus::Error(code)=>{return -3;},}
        let again=close(&mut file);match again{FileStatus::Ok=>{},FileStatus::Error(code)=>{return -4;},}
    },OpenResult::Error(code)=>{return -5;},OpenResult::MemoryLimit=>{return -6;},OpenResult::AllocationFailed=>{return -7;},}
    let input=open_read(&path);
    match input{OpenResult::Opened(value)=>{let mut file=value;
        let invalid=seek_absolute(&mut file,-1i64);match invalid{FileStatus::Ok=>{return -8;},FileStatus::Error(code)=>{if code!=22{return -9;}},}
        return check(&mut file,&expected);
    },OpenResult::Error(code)=>{return -13;},OpenResult::MemoryLimit=>{return -14;},OpenResult::AllocationFailed=>{return -15;},}
}
"#.replace("__PAYLOAD__", &payload)
}

#[test]
fn windows_file_vm_roundtrip_uses_the_real_utf8_path_and_binary_contract() {
    let project = Project::new();
    assert!(!project.bytes().exists());
    assert!(project.invoke(&["check"]).status.success());
    assert!(!project.bytes().exists());
    success(project.invoke(&["run"]), b"256\n");
    assert_eq!(
        fs::read(project.bytes()).unwrap(),
        (0..=255).collect::<Vec<u8>>()
    );
}

#[test]
fn windows_file_subset_does_not_admit_unrelated_aggregate_or_vector_abis() {
    for declaration in [
        "struct Other{value:i32}",
        "enum Other{Value}",
        "use alloc::bytes::new;fn other()->(){let bytes=new();return;}",
        "use alloc::vec::new;fn other()->(){let values=new::<i32>();return;}",
        "fn other<T:Copy>(value:T)->T{return value;}",
        "fn other(value:f64)->f64{return value;}",
    ] {
        let project = Project::new();
        let text = format!("use std::fs::File;use std::fs::FileStatus;use std::fs::close;fn shutdown(file:&mut File)->FileStatus{{return close(&mut *file);}}{declaration}fn main()->i32{{return 42;}}");
        fs::write(project.0.join("src/main.ox"), text).unwrap();
        let checked = project.invoke(&["check"]);
        assert!(checked.status.success(), "{checked:?}");
        let refused = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args([
                "compile",
                "src/main.ox",
                "--edition=typed-preview",
                "--backend=llvm",
                "--output=refused.exe",
                "--message-format=json",
            ])
            .env("OXID_LLVM_BIN", project.0.join("missing-tools"))
            .current_dir(&project.0)
            .output()
            .unwrap();
        assert_eq!(refused.status.code(), Some(1), "{declaration}: {refused:?}");
        let diagnostic = String::from_utf8_lossy(&refused.stdout);
        assert!(
            diagnostic.contains("E0700") && diagnostic.contains("Windows native"),
            "{diagnostic}"
        );
        assert!(
            !diagnostic.contains("E0701") && !diagnostic.contains("E0500"),
            "{diagnostic}"
        );
        assert!(!project.0.join("refused.exe").exists());
    }
}

#[test]
fn windows_file_candidate_keeps_unused_child_generics_and_other_owned_types_closed() {
    for declaration in [
        "fn other<T:Copy>(value:T)->T{return value;}",
        "struct Other{value:i32}",
        "use alloc::vec::new;fn other()->(){let values=new::<i32>();return;}",
    ] {
        let project = Project::new();
        fs::write(project.0.join("src/hidden.ox"), declaration).unwrap();
        fs::write(project.0.join("src/main.ox"), "mod hidden;use std::fs::File;use std::fs::FileStatus;use std::fs::close;fn shutdown(file:&mut File)->FileStatus{return close(&mut *file);}fn main()->i32{return 42;}").unwrap();
        let checked = project.invoke(&["check"]);
        assert!(checked.status.success(), "{checked:?}");
        for profile in ["dev", "release"] {
            let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
                .args(["build", "--profile", profile, "--message-format=json"])
                .env("OXID_LLVM_BIN", project.0.join("missing-tools"))
                .current_dir(&project.0)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            let text = String::from_utf8_lossy(&output.stdout);
            assert!(
                text.contains("E0700") && text.contains("Windows native"),
                "{text}"
            );
            assert!(!text.contains("E0701") && !text.contains("E0500"), "{text}");
            assert!(!project.0.join(".oxid/bin/file-copy.exe").exists());
        }
    }
}

fn invalid_path_source(suffix: &str) -> String {
    format!("use std::fs::OpenResult;use std::fs::create;fn main()->i32{{let path=b\"資料-😀.bin{suffix}\";let opened=create(&path);match opened{{OpenResult::Opened(file)=>{{return -1;}},OpenResult::Error(code)=>{{if code==22{{return 42;}}return code;}},OpenResult::MemoryLimit=>{{return -2;}},OpenResult::AllocationFailed=>{{return -3;}},}}}}")
}

#[test]
fn windows_file_vm_rejects_nul_and_invalid_utf8_without_truncating_the_prefix() {
    for suffix in ["\\x00ignored", "\\xC0\\xAF"] {
        let project = Project::new();
        fs::write(project.bytes(), b"retained").unwrap();
        fs::write(project.0.join("src/main.ox"), invalid_path_source(suffix)).unwrap();
        success(project.invoke(&["run"]), b"42\n");
        assert_eq!(fs::read(project.bytes()).unwrap(), b"retained");
    }
}

fn seek_fixture(project: &Project) {
    use std::{
        io::{Seek, SeekFrom, Write},
        os::windows::io::AsRawHandle,
    };
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn DeviceIoControl(
            handle: *mut std::ffi::c_void,
            code: u32,
            input: *mut std::ffi::c_void,
            input_bytes: u32,
            output: *mut std::ffi::c_void,
            output_bytes: u32,
            returned: *mut u32,
            overlapped: *mut std::ffi::c_void,
        ) -> i32;
    }
    let mut file = fs::File::create(project.bytes()).unwrap();
    let mut returned = 0;
    assert_ne!(
        unsafe {
            DeviceIoControl(
                file.as_raw_handle(),
                0x000900c4,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        },
        0,
        "NTFS sparse fixture setup failed"
    );
    file.write_all(b"A").unwrap();
    file.seek(SeekFrom::Start(4_294_967_419)).unwrap();
    file.write_all(b"Z").unwrap();
    fs::write(project.0.join("src/main.ox"), r#"
use std::fs::File;use std::fs::OpenResult;use std::fs::FileStatus;use std::fs::open_read;
use std::fs::read_some;use std::fs::seek_absolute;use std::io::ReadSomeStatus;
fn byte(file:&mut File,expected:u8)->bool{let mut buffer=b"x";let result=read_some(&mut *file,&mut buffer);match result{ReadSomeStatus::Data(n)=>{return n==1&&buffer[0]==expected;},ReadSomeStatus::Eof=>{return false;},ReadSomeStatus::Error(code)=>{return false;},}}
fn main()->i32{let path=b"資料-😀.bin";let opened=open_read(&path);match opened{
OpenResult::Opened(value)=>{let mut file=value;let invalid=seek_absolute(&mut file,-1i64);
match invalid{FileStatus::Ok=>{return -1;},FileStatus::Error(code)=>{if code!=22{return -2;}},}
if !byte(&mut file,65u8){return -3;}let sought=seek_absolute(&mut file,4294967419i64);
match sought{FileStatus::Ok=>{},FileStatus::Error(code)=>{return -4;},}
if !byte(&mut file,90u8){return -5;}return 42;},OpenResult::Error(code)=>{return -6;},OpenResult::MemoryLimit=>{return -7;},OpenResult::AllocationFailed=>{return -8;},}}
"#).unwrap();
}

#[test]
fn windows_file_vm_seek_uses_real_signed64_positions() {
    let project = Project::new();
    seek_fixture(&project);
    success(project.invoke(&["run"]), b"42\n");
    assert_eq!(fs::metadata(project.bytes()).unwrap().len(), 4_294_967_420);
}

#[test]
#[ignore = "requires actual Windows MSVC SDK and LLVM 19.1.7"]
fn windows_file_native_profiles_execute_after_source_and_cache_removal() {
    for profile in ["dev", "release"] {
        let project = Project::new();
        let built = project.invoke(&["build", "--profile", profile]);
        assert!(built.status.success(), "{built:?}");
        assert!(!project.bytes().exists());
        let native = project.0.join("standalone.exe");
        fs::copy(project.0.join(".oxid/bin/file-copy.exe"), &native).unwrap();
        fs::remove_dir_all(project.0.join("src")).unwrap();
        fs::remove_file(project.0.join("oxid.toml")).unwrap();
        fs::remove_dir_all(project.0.join(".oxid")).unwrap();
        let lock = project.0.join("oxid.lock");
        if lock.exists() {
            fs::remove_file(lock).unwrap();
        }
        let ran = Command::new(native)
            .current_dir(&project.0)
            .env_clear()
            .output()
            .unwrap();
        success(ran, b"256\n");
        assert_eq!(
            fs::read(project.bytes()).unwrap(),
            (0..=255).collect::<Vec<u8>>()
        );
    }
}

#[test]
#[ignore = "requires actual Windows MSVC SDK and LLVM 19.1.7"]
fn windows_file_c_runtime_contracts_execute_at_o0_and_o2() {
    let llvm = std::env::var_os("OXID_TEST_LLVM_BIN").expect("set OXID_TEST_LLVM_BIN");
    for optimization in ["-O0", "-O2"] {
        let project = Project::new();
        let output = project.0.join("contracts.exe");
        let object = project.0.join("contracts.obj");
        let built = Command::new(PathBuf::from(&llvm).join("clang.exe"))
            .args([
                "--no-default-config",
                "--target=x86_64-pc-windows-msvc",
                "-fms-runtime-lib=static",
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                optimization,
            ])
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/windows_file_runtime.c"))
            .arg("-c")
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(built.status.success(), "{built:?}");
        let linked = Command::new(PathBuf::from(&llvm).join("lld-link.exe"))
            .args([
                "/SUBSYSTEM:CONSOLE",
                "/MACHINE:X64",
                "/DEFAULTLIB:libcmt",
                "/DEFAULTLIB:oldnames",
                "/DEFAULTLIB:kernel32",
            ])
            .arg(&object)
            .arg(format!("/OUT:{}", output.to_str().unwrap()))
            .output()
            .unwrap();
        assert!(linked.status.success(), "{linked:?}");
        success(
            Command::new(output)
                .current_dir(&project.0)
                .output()
                .unwrap(),
            b"",
        );
    }
}

#[test]
#[ignore = "requires actual Windows MSVC SDK and LLVM 19.1.7"]
fn windows_file_native_seek_remains_64_bit_after_source_removal() {
    for profile in ["dev", "release"] {
        let project = Project::new();
        seek_fixture(&project);
        let built = project.invoke(&["build", "--profile", profile]);
        assert!(built.status.success(), "{built:?}");
        let native = project.0.join("standalone.exe");
        fs::copy(project.0.join(".oxid/bin/file-copy.exe"), &native).unwrap();
        fs::remove_dir_all(project.0.join("src")).unwrap();
        fs::remove_file(project.0.join("oxid.toml")).unwrap();
        fs::remove_dir_all(project.0.join(".oxid")).unwrap();
        let lock = project.0.join("oxid.lock");
        if lock.exists() {
            fs::remove_file(lock).unwrap();
        }
        success(
            Command::new(native)
                .current_dir(&project.0)
                .env_clear()
                .output()
                .unwrap(),
            b"42\n",
        );
    }
}

#[test]
#[ignore = "requires actual Windows MSVC SDK and LLVM 19.1.7"]
fn windows_file_native_invalid_paths_keep_exact_error_and_prefix_bytes() {
    for profile in ["dev", "release"] {
        for suffix in ["\\x00ignored", "\\xC0\\xAF"] {
            let project = Project::new();
            fs::write(project.bytes(), b"retained").unwrap();
            fs::write(project.0.join("src/main.ox"), invalid_path_source(suffix)).unwrap();
            let built = project.invoke(&["build", "--profile", profile]);
            assert!(built.status.success(), "{built:?}");
            assert_eq!(fs::read(project.bytes()).unwrap(), b"retained");
            let native = project.0.join("standalone.exe");
            fs::copy(project.0.join(".oxid/bin/file-copy.exe"), &native).unwrap();
            fs::remove_dir_all(project.0.join("src")).unwrap();
            fs::remove_file(project.0.join("oxid.toml")).unwrap();
            fs::remove_dir_all(project.0.join(".oxid")).unwrap();
            let lock = project.0.join("oxid.lock");
            if lock.exists() {
                fs::remove_file(lock).unwrap();
            }
            success(
                Command::new(native)
                    .current_dir(&project.0)
                    .env_clear()
                    .output()
                    .unwrap(),
                b"42\n",
            );
            assert_eq!(fs::read(project.bytes()).unwrap(), b"retained");
        }
    }
}
