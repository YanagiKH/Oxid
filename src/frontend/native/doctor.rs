//! Native environment diagnosis. Probes use fixed inputs and private temporary
//! storage; they never execute a project's program or publish a build artifact.
use super::{LLVM_VERSION, TARGET};
use crate::frontend::diagnostic::json_string;
use std::io::Write;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Passed,
    Failed,
    Skipped,
    NotApplicable,
}
impl Status {
    fn name(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::NotApplicable => "not-applicable",
        }
    }
}
struct Check {
    id: &'static str,
    status: Status,
    message: String,
}

pub(crate) struct Report {
    checks: Vec<Check>,
}
impl Report {
    pub(crate) fn invalid(message: &str) -> Self {
        Self {
            checks: vec![Check {
                id: "options",
                status: Status::Failed,
                message: clipped(message),
            }],
        }
    }
    fn pending() -> Self {
        Self {
            checks: [
                "host",
                "temporary-directory",
                "clang",
                "opt",
                "linker",
                "sdk",
                "compile-link",
                "execute",
            ]
            .into_iter()
            .map(|id| Check {
                id,
                status: Status::Skipped,
                message: "prerequisite check did not pass".into(),
            })
            .collect(),
        }
    }
    fn set(&mut self, id: &'static str, result: Result<String, String>) {
        let check = self
            .checks
            .iter_mut()
            .find(|check| check.id == id)
            .expect("known doctor check");
        let (status, message) = match result {
            Ok(message) => (Status::Passed, message),
            Err(message) => (Status::Failed, message),
        };
        check.status = status;
        check.message = clipped(&message);
    }
    pub(crate) fn project(&mut self, result: Result<Option<String>, String>) {
        let (status, message) = match result {
            Ok(Some(message)) => (Status::Passed, message),
            Ok(None) => (Status::NotApplicable, "no typed-preview project selected; legacy project health remains available through `oxid doctor`".into()),
            Err(message) => (Status::Failed, message),
        };
        self.checks.push(Check {
            id: "project",
            status,
            message: clipped(&message),
        });
    }
    fn ready(&self) -> bool {
        self.checks
            .iter()
            .filter(|check| check.id != "project")
            .all(|check| check.status == Status::Passed)
    }
    fn success(&self) -> bool {
        self.checks
            .iter()
            .all(|check| check.status != Status::Failed)
    }
    pub(crate) fn print(&self, json: bool) -> i32 {
        let mut text = if json {
            format!("{{\"schema_version\":1,\"kind\":\"native-doctor\",\"edition\":\"typed-preview\",\"target\":\"{TARGET}\",\"llvm_version\":\"{LLVM_VERSION}\",\"toolchain_ready\":{},\"success\":{},\"checks\":[", self.ready(), self.success())
        } else {
            format!("Oxid native doctor (LLVM {LLVM_VERSION}, {TARGET})\n")
        };
        for (index, check) in self.checks.iter().enumerate() {
            if json {
                if index != 0 {
                    text.push(',');
                }
                text.push_str(&format!(
                    "{{\"id\":\"{}\",\"status\":\"{}\",\"message\":{}}}",
                    check.id,
                    check.status.name(),
                    json_string(&check.message)
                ));
            } else {
                text.push_str(&format!(
                    "{}: {}: {}\n",
                    check.id,
                    check.status.name(),
                    escaped(&check.message)
                ));
            }
        }
        if json {
            text.push_str("]}\n");
        } else {
            text.push_str(&format!(
                "toolchain-ready: {}\nsuccess: {}\n",
                self.ready(),
                self.success()
            ));
        }
        if std::io::stdout().lock().write_all(text.as_bytes()).is_err() {
            74
        } else if self.success() {
            0
        } else {
            1
        }
    }
}

fn clipped(value: &str) -> String {
    let mut end = value.len().min(2048);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}
fn escaped(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        if character.is_control() {
            output.extend(character.escape_default());
        } else {
            output.push(character);
        }
    }
    output
}

pub(crate) fn run() -> Report {
    let mut report = Report::pending();
    if !super::NativeBuildSettings::host_supported() {
        report.set(
            "host",
            Err(format!(
                "native builds require Linux or macOS on x86_64 or ARM64; current host is {} {}",
                std::env::consts::OS,
                std::env::consts::ARCH
            )),
        );
        return report;
    }
    report.set(
        "host",
        Ok(format!(
            "{} {}; bounded native target {TARGET}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )),
    );
    #[cfg(all(
        target_os = "windows",
        target_env = "msvc",
        target_arch = "x86_64",
        target_pointer_width = "64",
        target_endian = "little"
    ))]
    report.set(
        "compile-link",
        Err("this build cannot yet verify a Windows native toolchain".into()),
    );
    #[cfg(all(
        any(target_os = "linux", target_os = "macos"),
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    supported::probe(&mut report);
    report
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod supported {
    use super::super::{
        resolve_tool, runtime_compile_args, tool, validate_version_output, Workspace, LLVM_VERSION,
        RUNTIME,
    };
    use super::{Report, Status};
    use std::{
        fs,
        io::{ErrorKind, Read},
        os::{fd::AsRawFd, unix::process::CommandExt},
        path::Path,
        process::{Child, Command, ExitStatus, Output, Stdio},
        time::{Duration, Instant},
    };
    const OUTPUT_LIMIT: usize = 65_536;
    const TOOL_TIMEOUT: Duration = Duration::from_secs(5);
    const BUILD_TIMEOUT: Duration = Duration::from_secs(20);
    const RUN_TIMEOUT: Duration = Duration::from_secs(2);
    #[cfg(target_os = "linux")]
    const ADDRESS_SPACE_LIMIT: libc::rlim_t = 1024 * 1024 * 1024;

    // Keep the waitable leader until group cleanup, including after it exits.
    // Reaping before kill(-pgid) would allow the numeric process identity to be
    // reused for an unrelated group. No signal is sent after cleanup/reaping.
    struct ChildGroup {
        child: Child,
        cleaned: bool,
    }
    impl ChildGroup {
        fn exited(&mut self) -> Result<bool, String> {
            // SAFETY: siginfo_t is initialized storage for waitid's output.
            // WNOWAIT observes this owned child without releasing its PID.
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.child.id(),
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == ErrorKind::Interrupted {
                    return Ok(false);
                }
                // An unexpected external reaper/ECHILD means the saved numeric
                // identity is no longer safe for either group or PID signals.
                self.cleaned = true;
                return Err(format!("cannot retain probe process identity: {error}"));
            }
            Ok(unsafe { info.si_pid() } != 0)
        }
        fn cleanup(&mut self) -> Result<ExitStatus, String> {
            self.exited()?;
            self.cleaned = true;
            // The leader is still waitable: its PID/PGID cannot have been
            // reused. Kill the group and the leader before the first reap.
            unsafe {
                libc::kill(-(self.child.id() as i32), libc::SIGKILL);
            }
            let _ = self.child.kill();
            let deadline = Instant::now() + Duration::from_millis(100);
            loop {
                match self.child.try_wait() {
                    Ok(Some(status)) => return Ok(status),
                    Err(error) => return Err(format!("cannot reap probe: {error}")),
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Ok(None) => return Err("probe leader cleanup exceeded 100 milliseconds".into()),
                }
            }
        }
    }
    impl Drop for ChildGroup {
        fn drop(&mut self) {
            if !self.cleaned {
                let _ = self.cleanup();
            }
        }
    }
    fn require_waitable_children() -> Result<(), String> {
        // Do not change the caller's signal policy. Native doctor has no other
        // child reaper; reject inherited auto-reaping before acquiring a child.
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), &mut action) } != 0 {
            return Err(format!(
                "cannot inspect child signal policy: {}",
                std::io::Error::last_os_error()
            ));
        }
        if action.sa_sigaction != libc::SIG_DFL || action.sa_flags & libc::SA_NOCLDWAIT != 0 {
            return Err(
                "native doctor requires default SIGCHLD handling and waitable children".into(),
            );
        }
        Ok(())
    }
    fn nonblocking(stream: &impl AsRawFd) -> Result<(), String> {
        let fd = stream.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(format!(
                "cannot configure probe capture: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
    fn drain(stream: &mut impl Read, bytes: &mut Vec<u8>, name: &str) -> Result<bool, String> {
        let mut buffer = [0u8; 4096];
        let available = (OUTPUT_LIMIT + 1 - bytes.len()).min(buffer.len());
        match stream.read(&mut buffer[..available]) {
            Ok(0) => Ok(true),
            Ok(count) => {
                bytes.extend_from_slice(&buffer[..count]);
                if bytes.len() > OUTPUT_LIMIT {
                    Err(format!("probe {name} exceeds {OUTPUT_LIMIT} bytes"))
                } else {
                    Ok(false)
                }
            }
            Err(error)
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) =>
            {
                Ok(false)
            }
            Err(error) => Err(format!("cannot capture probe {name}: {error}")),
        }
    }
    fn capture(command: &mut Command, timeout: Duration) -> Result<Output, String> {
        require_waitable_children()?;
        let program = command.get_program().to_owned();
        // The fixed compiler probe calls the ordinary native pipeline, whose
        // trusted tools may buffer output internally. Bound that whole process
        // tree too, and prevent an abnormal probe from writing a core file in
        // the caller's directory. Never increase an inherited resource limit.
        // SAFETY: the post-fork callback uses only async-signal-safe getrlimit /
        // setrlimit and constructs an OS error; it allocates nothing, takes no
        // locks and does not access shared Rust state before exec.
        unsafe {
            command.pre_exec(|| {
                // Darwin executables reserve a large unmapped page-zero range;
                // a Linux-sized address-space ceiling prevents dyld startup.
                #[cfg(target_os = "linux")]
                {
                    let mut limit = libc::rlimit {
                        rlim_cur: 0,
                        rlim_max: 0,
                    };
                    if libc::getrlimit(libc::RLIMIT_AS, &mut limit) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    limit.rlim_cur = limit.rlim_cur.min(ADDRESS_SPACE_LIMIT);
                    limit.rlim_max = limit.rlim_max.min(ADDRESS_SPACE_LIMIT);
                    if libc::setrlimit(libc::RLIMIT_AS, &limit) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                let no_core = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                if libc::setrlimit(libc::RLIMIT_CORE, &no_core) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = ChildGroup {
            child: command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .env_remove("CCC_OVERRIDE_OPTIONS")
                .process_group(0)
                .spawn()
                .map_err(|error| format!("cannot launch {}: {error}", program.to_string_lossy()))?,
            cleaned: false,
        };
        let mut stdout = child
            .child
            .stdout
            .take()
            .ok_or("missing probe stdout pipe")?;
        let mut stderr = child
            .child
            .stderr
            .take()
            .ok_or("missing probe stderr pipe")?;
        nonblocking(&stdout)?;
        nonblocking(&stderr)?;
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let mut eof = (false, false);
        let deadline = Instant::now() + timeout;
        loop {
            if !eof.0 {
                eof.0 = drain(&mut stdout, &mut output, "stdout")?;
            }
            if !eof.1 {
                eof.1 = drain(&mut stderr, &mut errors, "stderr")?;
            }
            if child.exited()? && eof.0 && eof.1 {
                let status = child.cleanup()?;
                if !status.success() {
                    let detail = if errors.is_empty() { &output } else { &errors };
                    return Err(format!(
                        "{} failed ({status}): {}",
                        program.to_string_lossy(),
                        super::clipped(&String::from_utf8_lossy(detail))
                    ));
                }
                return Ok(Output {
                    status,
                    stdout: output,
                    stderr: errors,
                });
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "probe timed out after {} seconds",
                    timeout.as_secs()
                ));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn artifact(path: &Path, executable: bool) -> Result<(), String> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| format!("probe artifact is missing: {error}"))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("probe artifact must be a regular file".into());
        }
        let mut header = [0u8; 64];
        fs::File::open(path)
            .and_then(|mut file| file.read_exact(&mut header))
            .map_err(|error| format!("cannot read probe executable header: {error}"))?;
        let valid = if cfg!(target_os = "macos") {
            let cpu = if cfg!(target_arch = "aarch64") {
                0x0100_000c
            } else {
                0x0100_0007
            };
            header[..4] == [0xcf, 0xfa, 0xed, 0xfe]
                && u32::from_le_bytes(header[4..8].try_into().unwrap()) == cpu
                && u32::from_le_bytes(header[12..16].try_into().unwrap())
                    == if executable { 2 } else { 1 }
                && (!executable
                    || u32::from_le_bytes(header[24..28].try_into().unwrap()) & 0x20_0000 != 0)
        } else {
            &header[..7] == b"\x7fELF\x02\x01\x01"
                && u16::from_le_bytes([header[16], header[17]]) == if executable { 3 } else { 1 }
                && u16::from_le_bytes([header[18], header[19]])
                    == super::super::NativeBuildSettings::elf_machine()
        };
        if !valid {
            return Err(format!(
                "probe artifact is not the required {} object or position-independent executable for this host",
                std::env::consts::ARCH
            ));
        }
        Ok(())
    }

    pub(super) fn probe(report: &mut Report) {
        let workspace = match fs::canonicalize(std::env::temp_dir())
            .map_err(|error| error.to_string())
            .and_then(|parent| Workspace::create(&parent).map_err(|error| error.message))
        {
            Ok(workspace) => workspace,
            Err(error) => {
                report.set("temporary-directory", Err(error));
                return;
            }
        };
        report.set(
            "temporary-directory",
            Ok("private temporary probe storage available".into()),
        );
        let mut clang = None;
        for (id, name, marker) in [
            ("clang", "clang", "clang version"),
            ("opt", "opt", "LLVM version"),
            ("linker", super::super::NativeBuildSettings::linker(), "LLD"),
        ] {
            let result = (|| {
                let path = resolve_tool(tool(name)).map_err(|error| error.message)?;
                let output = capture(
                    Command::new(&path)
                        .arg("--version")
                        .current_dir(&workspace.0),
                    TOOL_TIMEOUT,
                )?;
                validate_version_output(&path, marker, &output.stdout)
                    .map_err(|error| error.message)?;
                if name == "clang" {
                    clang = Some(path.clone());
                }
                Ok(format!("{}; LLVM {LLVM_VERSION}", path.display()))
            })();
            report.set(id, result);
        }
        if report
            .checks
            .iter()
            .any(|check| check.status == Status::Failed)
        {
            return;
        }
        let sdk = (|| {
            fs::write(workspace.0.join("runtime.c"), RUNTIME).map_err(|error| error.to_string())?;
            capture(
                Command::new(clang.as_ref().expect("validated clang"))
                    .args(runtime_compile_args(
                        &super::super::NativeBuildSettings::new(false),
                    ))
                    .current_dir(&workspace.0),
                TOOL_TIMEOUT,
            )?;
            artifact(&workspace.0.join("runtime.o"), false)?;
            Ok(format!(
                "production C runtime compiled against target SDK headers to an {} object",
                std::env::consts::ARCH
            ))
        })();
        let sdk_ok = sdk.is_ok();
        report.set("sdk", sdk);
        if !sdk_ok {
            return;
        }
        let executable = workspace.0.join("probe");
        let linked = (|| {
            let compiler = std::env::current_exe().map_err(|error| error.to_string())?;
            let source = workspace.0.join("probe.ox");
            fs::write(&source, "fn main()->i32{return 42;}\n")
                .map_err(|error| error.to_string())?;
            // Keep the caller's cwd so relative OXID_LLVM_BIN/PATH components
            // select exactly the tools just checked. All probe paths are absolute.
            capture(
                Command::new(&compiler)
                    .arg("compile")
                    .arg(&source)
                    .args([
                        "--edition=typed-preview",
                        "--backend=llvm",
                        "--message-format=json",
                    ])
                    .arg("--output")
                    .arg(&executable),
                BUILD_TIMEOUT,
            )?;
            artifact(&executable, true)?;
            let format = if cfg!(target_os = "macos") {
                "Mach-O64"
            } else {
                "ELF64"
            };
            Ok(format!("{} compiled, verified and linked a fixed Oxid program with the production runtime and target libraries ({format} {} PIE)", compiler.display(),std::env::consts::ARCH))
        })();
        let link_ok = linked.is_ok();
        report.set("compile-link", linked);
        if !link_ok {
            return;
        }
        report.set("execute", capture(Command::new(&executable).current_dir(&workspace.0), RUN_TIMEOUT)
            .and_then(|output| {
                if output.stdout != b"42\n" || !output.stderr.is_empty() {
                    Err("native probe returned unexpected stdout or stderr; expected exactly 42 followed by a newline".into())
                } else { Ok("fixed native probe exited 0 with exact stdout 42\\n; target loader and runtime libraries work".into()) }
            }));
    }
}
