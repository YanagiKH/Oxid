//! Public file ownership and binary transport contracts.
#![cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
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
            "oxid-files-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"), "[project]\nname=\"files\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n").unwrap();
        Self(root)
    }
    fn command(&self, arguments: &[&str]) -> Output {
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
fn success(output: Output, expected: &[u8]) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, expected, "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}
const ROUND_TRIP: &str = r#"
use std::fs::File;
use std::fs::OpenResult;
use std::fs::FileStatus;
use std::fs::create;
use std::fs::open_read;
use std::fs::read_some;
use std::fs::write_some;
use std::fs::sync_all;
use std::fs::close;
use std::io::ReadSomeStatus;
use std::io::WriteSomeStatus;
fn write(file:&mut File, bytes:&[u8])->bool{
    let mut offset=0;
    while offset<bytes.len(){
        let status=write_some(&mut *file,&*bytes[offset..bytes.len()]);
        match status{
            WriteSomeStatus::Written(count)=>{if count==0{return false;}offset=offset+count;},
            WriteSomeStatus::Error(code)=>{return false;},
        }
    }
    return true;
}
fn store()->i32{
    let path=b"data.bin";
    let opened=create(&path);
    match opened{
        OpenResult::Opened(file)=>{
            let mut output=file;
            let bytes=b"Oxid\0\xFF\n👋";
            if !write(&mut output,&bytes){return -1;}
            let synced=sync_all(&mut output);
            match synced{FileStatus::Ok=>{},FileStatus::Error(code)=>{return -2;},}
            let closed=close(&mut output);
            match closed{FileStatus::Ok=>{},FileStatus::Error(code)=>{return -3;},}
            let again=close(&mut output);
            match again{FileStatus::Ok=>{},FileStatus::Error(code)=>{return -4;},}
            let failed=write_some(&mut output,&bytes);
            match failed{WriteSomeStatus::Written(count)=>{return -5;},WriteSomeStatus::Error(code)=>{if code!=9{return -6;}},}
            return bytes.len();
        },
        OpenResult::Error(code)=>{return -7;},
        OpenResult::MemoryLimit=>{return -8;},
        OpenResult::AllocationFailed=>{return -9;},
    }
}
fn main()->i32{
    let stored=store();
    if stored!=11{return stored;}
    let path=b"data.bin";
    let opened=open_read(&path);
    match opened{
        OpenResult::Opened(file)=>{
            let mut input=file;
            let expected=b"Oxid\0\xFF\n👋";
            let mut buffer=b"xxx";
            let mut offset=0;
            while true{
                let status=read_some(&mut input,&mut buffer);
                match status{
                    ReadSomeStatus::Data(count)=>{
                        if count<=0{return -10;}
                        let mut index=0;
                        while index<count{
                            if offset>=expected.len(){return -11;}
                            if buffer[index]!=expected[offset]{return -12;}
                            index=index+1;offset=offset+1;
                        }
                    },
                    ReadSomeStatus::Eof=>{return offset;},
                    ReadSomeStatus::Error(code)=>{return -13;},
                }
            }
            return -17;
        },
        OpenResult::Error(code)=>{return -14;},
        OpenResult::MemoryLimit=>{return -15;},
        OpenResult::AllocationFailed=>{return -16;},
    }
}
"#;

#[test]
fn file_handles_round_trip_binary_data_partial_reads_eof_sync_and_close() {
    let project = Project::new(ROUND_TRIP);
    success(project.command(&["run"]), b"11\n");
    assert_eq!(
        fs::read(project.0.join("data.bin")).unwrap(),
        [b"Oxid\0\xFF\n".as_slice(), "👋".as_bytes()].concat()
    );
}

#[test]
fn invalid_paths_return_errors_without_truncating_the_valid_prefix() {
    let source = r#"
use std::fs::OpenResult;
use std::fs::create;
fn main()->i32{
    let path=b"keep.bin\0suffix";
    let result=create(&path);
    match result{
        OpenResult::Opened(file)=>{return -1;},
        OpenResult::Error(code)=>{return code;},
        OpenResult::MemoryLimit=>{return -2;},
        OpenResult::AllocationFailed=>{return -3;},
    }
}

"#;
    let project = Project::new(source);
    fs::write(project.0.join("keep.bin"), b"retained").unwrap();
    success(project.command(&["run"]), b"22\n");
    assert_eq!(fs::read(project.0.join("keep.bin")).unwrap(), b"retained");
}

const RAW_PATH: &str = r#"
use std::fs::OpenResult;
use std::fs::create;
fn main()->i32{
    let path=b"raw_\xFF.bin";
    let result=create(&path);
    match result{
        OpenResult::Opened(file)=>{return 0;},
        OpenResult::Error(code)=>{return -1;},
        OpenResult::MemoryLimit=>{return -2;},
        OpenResult::AllocationFailed=>{return -3;},
    }
}
"#;

#[test]
fn paths_preserve_non_utf8_linux_bytes() {
    use std::os::unix::ffi::OsStringExt;
    let project = Project::new(RAW_PATH);
    success(project.command(&["run"]), b"0\n");
    assert!(project
        .0
        .join(std::ffi::OsString::from_vec(b"raw_\xFF.bin".to_vec()))
        .is_file());
}

const UNUSED_FILES: &str = r#"
use std::fs::File;
fn relay(file:File)->File{return file;}
fn discard(file:File)->i32{return 0;}
fn inspect(file:&File)->i32{return 0;}
fn main()->i32{return 0;}
"#;

#[test]
fn unused_file_signatures_are_checked_without_opening_a_file() {
    let project = Project::new(UNUSED_FILES);
    success(project.command(&["run"]), b"0\n");
}

#[test]
fn file_ownership_errors_precede_creation_and_preserve_existing_outputs() {
    let source = r#"
use std::fs::OpenResult;
use std::fs::create;
use std::fs::close;
fn main()->i32{
    let path=b"keep.bin";let opened=create(&path);
    match opened{
        OpenResult::Opened(file)=>{
            let mut first=file;let second=first;
            close(&mut first);return 0;
        },
        OpenResult::Error(code)=>{return -1;},
        OpenResult::MemoryLimit=>{return -2;},
        OpenResult::AllocationFailed=>{return -3;},
    }
}
"#;
    let project = Project::new(source);
    fs::write(project.0.join("keep.bin"), b"retained").unwrap();
    let result = project.command(&["run"]);
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("E0310"),
        "{result:?}"
    );
    assert_eq!(fs::read(project.0.join("keep.bin")).unwrap(), b"retained");
}

const BUFFER_PATH_AND_FIELD: &str = r#"
use std::fs::File;
use std::fs::OpenResult;
use std::fs::create;
use std::fs::write_some;
use std::io::WriteSomeStatus;
use alloc::bytes::new;
use alloc::bytes::try_extend;
struct Holder{file:File}
enum Wrapped{Item(Holder),Empty}
fn relay(file:File)->File{return file;}
fn main()->i32{
    let mut path=new();let spelling=b"field.bin";
    try_extend(&mut path,&spelling);
    let opened=create(&path);
    match opened{
        OpenResult::Opened(file)=>{
            let holder=Holder{file:relay(file)};
            let wrapped=Wrapped::Item(holder);
            match wrapped{
                Wrapped::Empty=>{return -1;},
                Wrapped::Item(payload)=>{
                    let mut target=payload;let bytes=b"field";
                    let result=write_some(&mut target.file,&bytes);
                    match result{
                        WriteSomeStatus::Written(n)=>{return n;},
                        WriteSomeStatus::Error(code)=>{return -2;},
                    }
                },
            }
        },
        OpenResult::Error(code)=>{return -3;},
        OpenResult::MemoryLimit=>{return -4;},
        OpenResult::AllocationFailed=>{return -5;},
    }
}
"#;
#[test]
fn buffer_paths_and_file_moves_through_nested_records_and_enums() {
    let project = Project::new(BUFFER_PATH_AND_FIELD);
    success(project.command(&["run"]), b"5\n");
    assert_eq!(fs::read(project.0.join("field.bin")).unwrap(), b"field");
}

#[test]
#[ignore = "requires Linux x86_64 or ARM64 LLVM 19.1.7 in OXID_LLVM_BIN"]
fn native_files_preserve_binary_data_and_survive_source_removal() {
    for source in [UNUSED_FILES, RAW_PATH] {
        for profile in ["dev", "release"] {
            let project = Project::new(source);
            let built = project.command(&["build", "--profile", profile]);
            assert!(built.status.success(), "{built:?}");
            success(
                Command::new(project.0.join(".oxid/bin/files"))
                    .current_dir(&project.0)
                    .env_clear()
                    .output()
                    .unwrap(),
                b"0\n",
            );
        }
    }
    for profile in ["dev", "release"] {
        let project = Project::new(BUFFER_PATH_AND_FIELD);
        let built = project.command(&["build", "--profile", profile]);
        assert!(built.status.success(), "{built:?}");
        success(
            Command::new(project.0.join(".oxid/bin/files"))
                .current_dir(&project.0)
                .env_clear()
                .output()
                .unwrap(),
            b"5\n",
        );
        assert_eq!(fs::read(project.0.join("field.bin")).unwrap(), b"field");
    }
    for profile in ["dev", "release"] {
        let project = Project::new(ROUND_TRIP);
        let built = project.command(&["build", "--profile", profile]);
        assert!(built.status.success(), "{built:?}");
        let executable = project.0.join("files");
        fs::copy(project.0.join(".oxid/bin/files"), &executable).unwrap();
        fs::remove_dir_all(project.0.join("src")).unwrap();
        fs::remove_dir_all(project.0.join(".oxid")).unwrap();
        fs::remove_file(project.0.join("oxid.toml")).unwrap();
        success(
            Command::new(&executable)
                .current_dir(&project.0)
                .env_clear()
                .output()
                .unwrap(),
            b"11\n",
        );
        assert_eq!(
            fs::read(project.0.join("data.bin")).unwrap(),
            [b"Oxid\0\xFF\n".as_slice(), "👋".as_bytes()].concat()
        );
    }
}

const CLOSED_PIPE: &str = r#"
use std::fs::OpenResult;
use std::fs::create;
use std::fs::write_some;
use std::io::WriteSomeStatus;
fn marker()->i32{
    let path=b"marker.bin";let opened=create(&path);
    match opened{
        OpenResult::Opened(file)=>{
            let mut output=file;let bytes=b"caught";
            let result=write_some(&mut output,&bytes);
            match result{WriteSomeStatus::Written(n)=>{if n==6{return 0;}return 3;},WriteSomeStatus::Error(code)=>{return 4;},}
        },
        OpenResult::Error(code)=>{return 5;},OpenResult::MemoryLimit=>{return 6;},OpenResult::AllocationFailed=>{return 7;},
    }
}
fn main()->i32{
    let path=b"/dev/stdout";let opened=create(&path);
    match opened{
        OpenResult::Opened(file)=>{
            let mut output=file;let bytes=b"x";
            let result=write_some(&mut output,&bytes);
            match result{WriteSomeStatus::Written(n)=>{return 1;},WriteSomeStatus::Error(code)=>{if code==32{return marker();}return 2;},}
        },
        OpenResult::Error(code)=>{return 8;},OpenResult::MemoryLimit=>{return 9;},OpenResult::AllocationFailed=>{return 10;},
    }
}
"#;

fn closed_pipe(command: &mut Command, project: &Project) {
    use std::os::fd::FromRawFd;
    let mut descriptors = [-1; 2];
    // SAFETY: pipe2 initializes exactly two descriptors in this writable array.
    assert_eq!(
        unsafe { libc::pipe2(descriptors.as_mut_ptr(), libc::O_CLOEXEC) },
        0
    );
    // SAFETY: Each newly returned descriptor is owned exactly once here.
    let reader = unsafe { fs::File::from_raw_fd(descriptors[0]) };
    let writer = unsafe { fs::File::from_raw_fd(descriptors[1]) };
    drop(reader);
    let output = command
        .current_dir(&project.0)
        .stdout(writer)
        .output()
        .unwrap();
    // The program receives Error(EPIPE) and writes its marker. Result printing
    // subsequently refuses the closed stdout with the documented exit code74.
    assert_eq!(output.status.code(), Some(74), "{output:?}");
    assert_eq!(fs::read(project.0.join("marker.bin")).unwrap(), b"caught");
}

#[test]
fn file_write_delivers_epipe_before_result_output_failure() {
    let project = Project::new(CLOSED_PIPE);
    closed_pipe(
        Command::new(env!("CARGO_BIN_EXE_oxid")).arg("run"),
        &project,
    );
}

#[test]
#[ignore = "requires LLVM 19.1.7 in OXID_LLVM_BIN"]
fn native_file_write_delivers_epipe_after_source_removal_in_both_optimizations() {
    for profile in ["dev", "release"] {
        let project = Project::new(CLOSED_PIPE);
        let built = project.command(&["build", "--profile", profile]);
        assert!(built.status.success(), "{built:?}");
        let executable = project.0.join("files");
        fs::copy(project.0.join(".oxid/bin/files"), &executable).unwrap();
        fs::remove_dir_all(project.0.join("src")).unwrap();
        fs::remove_dir_all(project.0.join(".oxid")).unwrap();
        fs::remove_file(project.0.join("oxid.toml")).unwrap();
        closed_pipe(Command::new(executable).env_clear(), &project);
    }
}
