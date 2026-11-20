//! Typed termination is observed through real child handshakes and file handles.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
#[cfg(unix)]
use std::{
    io::{BufRead, BufReader, Read},
    path::Path,
    process::Child,
    sync::mpsc,
    time::{Duration, Instant},
};
const SUPPORTED: bool = cfg!(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
));
static NEXT: AtomicUsize = AtomicUsize::new(0);
const SOURCE: &str = r#"
use std::process::enable_termination_notifications;
use std::process::termination_requested;
use std::fs::create;use std::fs::open_read;use std::fs::OpenResult;
use std::fs::sync_all;use std::fs::FileStatus;
use alloc::bytes::new;use alloc::bytes::try_resize;use alloc::bytes::GrowStatus;
use alloc::bytes::len;
use std::io::write_some;use std::io::WriteSomeStatus;
fn announce(bytes:&[u8])->bool{
    let mut offset=0;
    while offset<bytes.len(){
        let written=write_some(&*bytes[offset..bytes.len()]);
        match written{WriteSomeStatus::Written(count)=>{if count<=0{return false;}offset=offset+count;},WriteSomeStatus::Error(code)=>{return false;},}
    }return true;
}
fn work()->i32{
    let mut bytes=new();let grown=try_resize(&mut bytes,4,7u8);
    match grown{GrowStatus::Ok=>{},GrowStatus::MemoryLimit=>{return 71;},GrowStatus::CapacityOverflow=>{return 72;},GrowStatus::AllocationFailed=>{return 73;},}
    let path=b"target.dat";let opened=create(&path);
    match opened{
        OpenResult::Opened(value)=>{
            let mut file=value;
            let ready=b"1\n";if !announce(&ready){return 83;}
            while !termination_requested(){}
            let synced=sync_all(&mut file);
            match synced{FileStatus::Ok=>{},FileStatus::Error(code)=>{return 74;},}
            if len(&bytes)!=4{return 75;}
            return 0;
        },
        OpenResult::Error(code)=>{return 76;},OpenResult::MemoryLimit=>{return 77;},OpenResult::AllocationFailed=>{return 78;},
    }
}
fn main()->i32{
    if !enable_termination_notifications(){return 79;}
    let result=work();if result!=0{return result;}
    let cleaned=b"2\n";if !announce(&cleaned){return 84;}
    let path=b"continue.dat";
    while true{
        let opened=open_read(&path);
        match opened{
            OpenResult::Opened(file)=>{return 42;},OpenResult::Error(code)=>{},
            OpenResult::MemoryLimit=>{return 80;},OpenResult::AllocationFailed=>{return 81;},
        }
    }
    return 82;
}
"#;
struct Fixture(PathBuf);
impl Fixture {
    fn fresh(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-termination-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    #[cfg(unix)]
    fn waiting_project() -> Self {
        let fixture = Self::project();
        let source = SOURCE
            .replace(
                "use std::process::termination_requested;",
                "use std::process::wait_for_termination;",
            )
            .replace("while !termination_requested(){}", "if !wait_for_termination(20000){return 85;}");
        fs::write(fixture.0.join("src/main.ox"), source).unwrap();
        fixture
    }
    fn project() -> Self {
        let fixture = Self::fresh("source");
        fs::create_dir(fixture.0.join("src")).unwrap();
        fs::write(fixture.0.join("src/main.ox"), SOURCE).unwrap();
        fs::write(fixture.0.join("oxid.toml"), "[project]\nname=\"worker\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"process\"\n[execution]\npolicy=\"hosted\"\n").unwrap();
        fixture
    }
    fn command(&self, arguments: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .args(arguments)
            .current_dir(&self.0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_remove("OXID_PATH")
            .env("OXID_CACHE_DIR", self.0.join("compiler-cache"));
        command
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[cfg(unix)]
struct ChildGuard(Option<Child>);
#[cfg(unix)]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
#[cfg(target_os = "linux")]
fn holds_file(pid: u32, path: &Path) -> bool {
    fs::read_dir(format!("/proc/{pid}/fd"))
        .unwrap()
        .any(|entry| fs::read_link(entry.unwrap().path()).is_ok_and(|target| target == path))
}
#[cfg(unix)]
fn exercise(command: &mut Command, data: &Path, signal: libc::c_int, waiting: bool) {
    let mut child = ChildGuard(Some(command.spawn().unwrap()));
    let stdout = child.0.as_mut().unwrap().stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            send.send(line.unwrap()).unwrap();
        }
    });
    assert_eq!(receive.recv_timeout(Duration::from_secs(30)).unwrap(), "1");
    let target = fs::canonicalize(data.join("target.dat")).unwrap();
    #[cfg(target_os = "linux")]
    assert!(
        holds_file(child.0.as_ref().unwrap().id(), &target),
        "worker File must be live before notification"
    );
    if waiting {
        // Let the worker enter the positive wait after announcing readiness.
        std::thread::sleep(Duration::from_millis(40));
        assert!(child.0.as_mut().unwrap().try_wait().unwrap().is_none());
    }
    assert_eq!(
        unsafe { libc::kill(child.0.as_ref().unwrap().id() as libc::pid_t, signal) },
        0
    );
    assert_eq!(receive.recv_timeout(Duration::from_secs(30)).unwrap(), "2");
    assert!(child.0.as_mut().unwrap().try_wait().unwrap().is_none());
    #[cfg(target_os = "linux")]
    assert!(
        !holds_file(child.0.as_ref().unwrap().id(), &target),
        "File must close on source return, while the process is still alive"
    );
    fs::write(data.join("continue.dat"), b"continue").unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.0.as_mut().unwrap().try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "child did not return normally");
        std::thread::sleep(Duration::from_millis(5));
    };
    let mut stderr = String::new();
    child
        .0
        .as_mut()
        .unwrap()
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    drop(child.0.take());
    reader.join().unwrap();
    assert_eq!(status.code(), Some(42), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(receive.try_recv().is_err());
}

#[test]
fn typed_process_notifications_return_and_drop_resources_after_sigint_and_sigterm() {
    if !SUPPORTED {
        return;
    }
    #[cfg(unix)]
    for signal in [libc::SIGINT, libc::SIGTERM] {
        let fixture = Fixture::project();
        exercise(&mut fixture.command(&["run"]), &fixture.0, signal, false);
        let fixture = Fixture::waiting_project();
        exercise(&mut fixture.command(&["run"]), &fixture.0, signal, true);
    }
}

#[test]
#[cfg(unix)]
fn merely_importing_notifications_keeps_the_default_sigterm_disposition() {
    use std::os::unix::process::ExitStatusExt;
    if !SUPPORTED {
        return;
    }
    let fixture = Fixture::project();
    fs::write(fixture.0.join("src/main.ox"), "use std::process::termination_requested;use std::io::write_some;use std::io::WriteSomeStatus;fn main()->i32{let ready=b\"1\\n\";let written=write_some(&ready);match written{WriteSomeStatus::Written(count)=>{if count!=2{return 83;}},WriteSomeStatus::Error(code)=>{return 83;},}while !termination_requested(){}return 42;}").unwrap();
    let mut child = ChildGuard(Some(fixture.command(&["run"]).spawn().unwrap()));
    let stdout = child.0.as_mut().unwrap().stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut line = String::new();
        BufReader::new(stdout).read_line(&mut line).unwrap();
        send.send(line).unwrap();
    });
    assert_eq!(
        receive.recv_timeout(Duration::from_secs(30)).unwrap(),
        "1\n"
    );
    assert_eq!(
        unsafe { libc::kill(child.0.as_ref().unwrap().id() as libc::pid_t, libc::SIGTERM) },
        0
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.0.as_mut().unwrap().try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "unopted child did not receive default termination"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    drop(child.0.take());
    reader.join().unwrap();
    assert_eq!(status.signal(), Some(libc::SIGTERM));
}

#[test]
fn result_library_and_unsupported_hosts_cannot_install_handlers() {
    let fixture = Fixture::project();
    let mut command = fixture.command(&["run", "src/main.ox", "--edition=typed-preview"]);
    let result = command.output().unwrap();
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!fixture.0.join("target.dat").exists());
    fs::write(fixture.0.join("src/poll.ox"), "use std::process::termination_requested;pub fn poll()->i32{if termination_requested(){return 1;}return 0;}").unwrap();
    let result = fixture
        .command(&[
            "--edition=typed-preview",
            "library",
            "--export",
            "crate::poll",
            "--output",
            "library",
            "src/poll.ox",
        ])
        .env("OXID_LLVM_BIN", fixture.0.join("missing-tools"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!fixture.0.join("library").exists());
    if !SUPPORTED {
        let result = fixture.command(&["run"]).output().unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!fixture.0.join("target.dat").exists());
    }
}

#[test]
#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[ignore = "requires the actual LLVM host toolchain; run in native acceptance"]
fn native_termination_notifications_keep_cleanup_after_source_free_relocation() {
    let llvm = std::env::var_os("OXID_TEST_LLVM_BIN").expect("OXID_TEST_LLVM_BIN is required");
    for (profile, waiting) in [
        ("dev", false),
        ("dev", true),
        ("release", false),
        ("release", true),
    ] {
        for signal in [libc::SIGINT, libc::SIGTERM] {
            let fixture = if waiting {
                Fixture::waiting_project()
            } else {
                Fixture::project()
            };
            let built = fixture
                .command(&["build", "--profile", profile])
                .env("OXID_LLVM_BIN", &llvm)
                .output()
                .unwrap();
            assert!(built.status.success(), "{profile}: {built:?}");
            let deployment = Fixture::fresh("deployment");
            let executable = deployment.0.join("worker");
            fs::copy(fixture.0.join(".oxid/bin/worker"), &executable).unwrap();
            fs::remove_dir_all(&fixture.0).unwrap();
            let mut command = Command::new(&executable);
            command
                .current_dir(&deployment.0)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .env("PATH", deployment.0.join("missing-tools"))
                .env("OXID_LLVM_BIN", deployment.0.join("missing-tools"))
                .env("OXID_PATH", deployment.0.join("missing-source"))
                .env_remove("OXID_TEST_LLVM_BIN")
                .env_remove("OXID_TEST_CC")
                .env_remove("CARGO_MANIFEST_DIR")
                .env_remove("CARGO_HOME")
                .env_remove("RUSTC")
                .env_remove("CC")
                .env_remove("CXX");
            exercise(&mut command, &deployment.0, signal, waiting);
        }
    }
}
