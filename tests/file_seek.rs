#![cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Project(PathBuf);
impl Project {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "oxid-file-seek-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), "[project]\nname=\"seek\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n").unwrap();
        Self(root)
    }
    fn command(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(arguments)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
    fn sparse(&self) {
        let mut file = fs::File::create(self.0.join("data.bin")).unwrap();
        file.write_all(b"ABCDE").unwrap();
        file.seek(SeekFrom::Start(4_294_967_419)).unwrap();
        file.write_all(b"Z").unwrap();
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(output: Output) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"42\n", "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}
const IMPORTS: &str = "use std::fs::File;use std::fs::FileStatus;use std::fs::OpenResult;use std::fs::open_read;use std::fs::seek_absolute;use std::fs::read_some;use std::fs::close;use std::io::ReadSomeStatus;";
const MAIN: &str = "fn main()->i32{let path=b\"data.bin\";let opened=open_read(&path);match opened{OpenResult::Opened(value)=>{let mut file=value;return exercise(&mut file);},OpenResult::Error(code)=>{return code;},OpenResult::MemoryLimit=>{return -100;},OpenResult::AllocationFailed=>{return -101;},}}";
const BODY: &str = r#"
fn byte(file:&mut File,expected:u8)->bool{
    let mut bytes=[0u8];let read=read_some(&mut *file,&mut bytes);
    match read{ReadSomeStatus::Data(count)=>{return count==1&&bytes[0]==expected;},ReadSomeStatus::Eof=>{return false;},ReadSomeStatus::Error(code)=>{return false;},}
}
fn seek(file:&mut File,offset:i64)->bool{let status=seek_absolute(&mut *file,offset);match status{FileStatus::Ok=>{return true;},FileStatus::Error(code)=>{return false;},}}
fn exercise(file:&mut File)->i32{
    if !seek(&mut *file,3i64)||!byte(&mut *file,68u8){return -1;}
    let invalid=seek_absolute(&mut *file,-1i64);
    match invalid{FileStatus::Ok=>{return -2;},FileStatus::Error(code)=>{if code!=22{return -3;}},}
    if !byte(&mut *file,69u8){return -4;}
    if !seek(&mut *file,4294967419i64)||!byte(&mut *file,90u8){return -5;}
    if !seek(&mut *file,4294967420i64){return -6;}
    let mut bytes=[0u8];let eof=read_some(&mut *file,&mut bytes);
    match eof{ReadSomeStatus::Data(count)=>{return -7;},ReadSomeStatus::Eof=>{},ReadSomeStatus::Error(code)=>{return -8;},}
    if !seek(&mut *file,0i64)||!byte(&mut *file,65u8){return -9;}
    let closed=close(&mut *file);match closed{FileStatus::Ok=>{},FileStatus::Error(code)=>{return -10;},}
    let again=seek_absolute(&mut *file,0i64);match again{FileStatus::Ok=>{return -11;},FileStatus::Error(code)=>{if code!=9{return -12;}},}
    return 42;
}
"#;

#[test]
fn seek_uses_signed_64_bit_offsets_and_preserves_position_on_invalid_offset() {
    let project = Project::new(&format!("{IMPORTS}{BODY}{MAIN}"));
    project.sparse();
    success(project.command(&["run"]));
    assert_eq!(
        fs::metadata(project.0.join("data.bin")).unwrap().len(),
        4_294_967_420
    );
}

#[test]
fn nonseekable_file_error_leaves_the_queued_bytes_unread() {
    let body = "fn exercise(file:&mut File)->i32{let status=seek_absolute(&mut *file,0i64);match status{FileStatus::Ok=>{return -1;},FileStatus::Error(code)=>{if code!=29{return -2;}},}let mut bytes=[0u8];let read=read_some(&mut *file,&mut bytes);match read{ReadSomeStatus::Data(n)=>{if n==1&&bytes[0]==81u8{return 42;}return -3;},ReadSomeStatus::Eof=>{return -4;},ReadSomeStatus::Error(code)=>{return -5;},}}";
    let project = Project::new(&format!("{IMPORTS}{body}{MAIN}"));
    let path = project.0.join("data.bin");
    let c_path = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: this test owns the fresh path and the CString is terminated.
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
    use std::os::unix::fs::OpenOptionsExt;
    let mut writer = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .unwrap();
    writer.write_all(b"Q").unwrap();
    let output = project.command(&["run"]);
    drop(writer);
    success(output);
}

#[test]
fn seek_public_signature_refuses_wrong_resource_width_and_conflicting_borrow() {
    for body in [
        "fn f(file:&File)->FileStatus{return seek_absolute(&*file,0i64);}fn main()->i32{return 0;}",
        "fn f(file:&mut File)->FileStatus{return seek_absolute(&mut *file,0);}fn main()->i32{return 0;}",
        "use alloc::bytes::new;fn main()->i32{let mut bytes=new();seek_absolute(&mut bytes,0i64);return 0;}",
        "fn offset(file:&File)->i64{return 0i64;}fn f(file:&mut File)->FileStatus{return seek_absolute(&mut *file,offset(&*file));}fn main()->i32{return 0;}",
    ] {
        let project = Project::new(&format!("{IMPORTS}{body}"));
        let output = project.command(&["check"]);
        assert_eq!(output.status.code(), Some(1), "{body}: {output:?}");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("E0500"), "{output:?}");
        assert!(!project.0.join(".oxid/bin/seek").exists());
    }
}

#[test]
#[ignore = "requires the admitted LLVM native SDK"]
fn native_seek_at_o0_and_o2_runs_without_sources_and_failed_rebuild_preserves_artifact() {
    let project = Project::new(&format!("{IMPORTS}{BODY}{MAIN}"));
    project.sparse();
    for profile in ["dev", "release"] {
        let built = project.command(&["build", "--profile", profile, "--reproducible"]);
        assert!(built.status.success(), "{built:?}");
        let binary = project.0.join(".oxid/bin/seek");
        let before = fs::read(&binary).unwrap();
        fs::rename(project.0.join("src"), project.0.join("retained-source")).unwrap();
        success(
            Command::new(&binary)
                .current_dir(&project.0)
                .env_clear()
                .output()
                .unwrap(),
        );
        let refused = project.command(&["build", "--profile", profile]);
        assert!(!refused.status.success(), "{refused:?}");
        assert_eq!(fs::read(&binary).unwrap(), before);
        fs::rename(project.0.join("retained-source"), project.0.join("src")).unwrap();
    }
}
