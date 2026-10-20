//! Focused acceptance for the pure ordinary-source `i32_algorithms` package
//! through its independent `i32_algorithms_contracts` client. Each fixture is
//! repo-shaped and copies exactly the two client files, two algorithms package
//! files, and the existing i32_vec manifest/source; no in-tree project is built.
//! The contract client fixes its own expectations. Acceptance checks the client,
//! interprets its exact `42\n` result, and runs its nine named cases. Child
//! processes are direct, isolated, bounded, and retain raw evidence on failure.
//! This is not native, stress, fault-injection, or whole-runtime qualification.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::{
    fs,
    io::{self, Read},
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const RESULT: &[u8] = b"42\n";
const OUTPUT_LIMIT: usize = 1024 * 1024;
const CHILD_TIMEOUT: Duration = Duration::from_secs(60);
const PROJECT: &str = "examples/projects/i32_algorithms_contracts";
const SOURCES: [&str; 6] = [
    "examples/projects/i32_algorithms_contracts/oxid.toml",
    "examples/projects/i32_algorithms_contracts/src/main.ox",
    "packages/i32_algorithms/oxid.toml",
    "packages/i32_algorithms/src/lib.ox",
    "packages/i32_vec/oxid.toml",
    "packages/i32_vec/src/lib.ox",
];
const CASES: &[&str] = &[
    "test_sort_empty_and_singleton",
    "test_sort_pairs_and_odd_even_heaps",
    "test_sort_signed_extremes_and_duplicates",
    "test_sort_sorted_reverse_equal_and_idempotent",
    "test_sort_preserves_capacity_and_owner",
    "test_lower_bound_empty_singleton_and_edges",
    "test_lower_bound_duplicates_and_gaps",
    "test_lower_bound_read_only",
    "test_sort_then_search_client_composition",
];

struct Evidence {
    dir: PathBuf,
    keep: bool,
}

impl Evidence {
    fn new() -> Self {
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        let base = std::env::var_os("OXID_I32_ALGORITHMS_EVIDENCE")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&base).unwrap_or_else(|error| {
            panic!("cannot create evidence base {}: {error}", base.display())
        });
        let keep = std::env::var_os("OXID_I32_ALGORITHMS_EVIDENCE").is_some();
        let mut created = None;
        for collision in 0..32 {
            let dir = base.join(format!(
                "oxid-i32-algorithms-{}-{serial}-{collision}",
                std::process::id()
            ));
            match fs::create_dir(&dir) {
                Ok(()) => {
                    created = Some(dir);
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!(
                    "cannot create evidence directory {}: {error}",
                    dir.display()
                ),
            }
        }
        let dir = created.expect("exhausted 32 exclusive evidence-directory collision retries");
        Self { dir, keep }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn fixture(&self) -> PathBuf {
        let fixture = self.path("fixture");
        fs::create_dir(&fixture).unwrap_or_else(|error| {
            panic!("cannot create fresh fixture {}: {error}", fixture.display())
        });
        fixture
    }

    fn materialize(&self, fixture: &Path) -> PathBuf {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
        for relative in SOURCES {
            let source = repository.join(relative);
            let destination = fixture.join(relative);
            fs::create_dir_all(destination.parent().unwrap()).unwrap_or_else(|error| {
                panic!(
                    "cannot create fixture parent for {}: {error}",
                    destination.display()
                )
            });
            fs::copy(&source, &destination).unwrap_or_else(|error| {
                panic!(
                    "cannot copy {} to {}: {error}",
                    source.display(),
                    destination.display()
                )
            });
        }
        fixture.join(PROJECT)
    }

    fn command(&self, fixture: &Path, cache: &Path, args: &[&str]) -> Command {
        let absent_path = self.path("absent-path");
        let absent_llvm = self.path("absent-llvm");
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .args(args)
            .current_dir(fixture)
            .env_clear()
            .env("PATH", absent_path)
            .env("OXID_CACHE_DIR", cache)
            .env("OXID_LLVM_BIN", absent_llvm)
            .env("OXID_TEST_LLVM_BIN", self.path("absent-test-llvm"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    /// Run without a shell; drain both pipes concurrently, retaining no more
    /// than 1 MiB from either. Timeout, overflow, signal and all harness errors
    /// are recorded as failures after killing/reaping the child when necessary.
    fn run(&self, label: &str, mut command: Command) -> Result<Captured, String> {
        let argv = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|part| part.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let env = command
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect::<Vec<_>>();
        let cwd = command
            .get_current_dir()
            .map(|path| path.to_string_lossy().into_owned());
        let started = timestamp_ms();
        let clock = Instant::now();
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.write_record(
                    label,
                    started,
                    timestamp_ms(),
                    cwd,
                    argv,
                    env,
                    None,
                    b"",
                    b"",
                    Some(&format!("spawn error: {error}")),
                );
                return Err(format!("{label}: spawn failed: {error}"));
            }
        };

        let stdout_pipe = match child.stdout.take() {
            Some(pipe) => pipe,
            None => {
                return self.abort_child(
                    label,
                    started,
                    clock,
                    cwd,
                    argv,
                    env,
                    child,
                    "stdout pipe missing",
                )
            }
        };
        let stderr_pipe = match child.stderr.take() {
            Some(pipe) => pipe,
            None => {
                return self.abort_child(
                    label,
                    started,
                    clock,
                    cwd,
                    argv,
                    env,
                    child,
                    "stderr pipe missing",
                )
            }
        };
        let stdout_error = Arc::new(AtomicBool::new(false));
        let stderr_error = Arc::new(AtomicBool::new(false));
        let stdout_overflow = Arc::new(AtomicBool::new(false));
        let stderr_overflow = Arc::new(AtomicBool::new(false));
        let stdout_thread = drain_pipe(stdout_pipe, stdout_overflow.clone(), stdout_error.clone());
        let stderr_thread = drain_pipe(stderr_pipe, stderr_overflow.clone(), stderr_error.clone());

        let mut status: Option<ExitStatus> = None;
        let mut failure: Option<String> = None;
        loop {
            match child.try_wait() {
                Ok(Some(done)) => {
                    status = Some(done);
                    break;
                }
                Ok(None) => {}
                Err(error) => {
                    failure = Some(format!("wait error: {error}"));
                    break;
                }
            }
            if stdout_error.load(Ordering::Relaxed) || stderr_error.load(Ordering::Relaxed) {
                failure = Some("stream read error".to_string());
                break;
            }
            if stdout_overflow.load(Ordering::Relaxed) || stderr_overflow.load(Ordering::Relaxed) {
                failure = Some("captured stream exceeded 1 MiB".to_string());
                break;
            }
            if clock.elapsed() >= CHILD_TIMEOUT {
                failure = Some("child timed out after 60 seconds".to_string());
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        if failure.is_some() {
            if let Err(error) = child.kill() {
                // A concurrently exited child can make kill return InvalidInput;
                // confirm its status below before classifying that as a harness error.
                if child.try_wait().ok().flatten().is_none() {
                    failure = Some(format!(
                        "{}; kill error: {error}",
                        failure.as_deref().unwrap_or("failure")
                    ));
                }
            }
        }
        let waited = child.wait();
        if let Err(error) = &waited {
            failure = Some(format!(
                "{}; reap/wait error: {error}",
                failure.as_deref().unwrap_or("failure")
            ));
        }
        if status.is_none() {
            status = waited.ok();
        }

        let stdout_result = join_drain(stdout_thread, "stdout");
        let stderr_result = join_drain(stderr_thread, "stderr");
        let stdout = match stdout_result {
            Ok(bytes) => bytes,
            Err(error) => {
                failure = Some(format!(
                    "{}; {error}",
                    failure.as_deref().unwrap_or("failure")
                ));
                Vec::new()
            }
        };
        let stderr = match stderr_result {
            Ok(bytes) => bytes,
            Err(error) => {
                failure = Some(format!(
                    "{}; {error}",
                    failure.as_deref().unwrap_or("failure")
                ));
                Vec::new()
            }
        };
        if stdout_error.load(Ordering::Relaxed) || stderr_error.load(Ordering::Relaxed) {
            failure = Some(format!(
                "{}; stream read error",
                failure.as_deref().unwrap_or("failure")
            ));
        }
        if stdout_overflow.load(Ordering::Relaxed) || stderr_overflow.load(Ordering::Relaxed) {
            failure = Some(format!(
                "{}; captured stream exceeded 1 MiB",
                failure.as_deref().unwrap_or("failure")
            ));
        }
        if status.as_ref().is_some_and(|s| s.code().is_none()) {
            failure = Some(format!(
                "{}; child terminated by signal {:?}",
                failure.as_deref().unwrap_or("failure"),
                status.as_ref().and_then(ExitStatusExt::signal)
            ));
        }
        let end = timestamp_ms();
        self.write_record(
            label,
            started,
            end,
            cwd,
            argv,
            env,
            status.as_ref(),
            &stdout,
            &stderr,
            failure.as_deref(),
        );
        if let Some(error) = failure {
            return Err(format!("{label}: {error}; evidence={}", self.dir.display()));
        }
        Ok(Captured {
            status: status.expect("successful wait must provide child status"),
            stdout,
            stderr,
        })
    }

    // Keep the same explicit receipt fields as run/write_record on this rare
    // harness-abort path; no process result or safety check is suppressed.
    #[allow(clippy::too_many_arguments)]
    fn abort_child(
        &self,
        label: &str,
        started: u128,
        clock: Instant,
        cwd: Option<String>,
        argv: Vec<String>,
        env: Vec<(String, Option<String>)>,
        mut child: std::process::Child,
        reason: &str,
    ) -> Result<Captured, String> {
        let _ = child.kill();
        let status = child.wait().ok();
        let elapsed = clock.elapsed();
        let message = format!("{reason}; child aborted after {elapsed:?}");
        self.write_record(
            label,
            started,
            timestamp_ms(),
            cwd,
            argv,
            env,
            status.as_ref(),
            b"",
            b"",
            Some(&message),
        );
        Err(format!(
            "{label}: {message}; evidence={}",
            self.dir.display()
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn write_record(
        &self,
        label: &str,
        started: u128,
        ended: u128,
        cwd: Option<String>,
        argv: Vec<String>,
        env: Vec<(String, Option<String>)>,
        status: Option<&ExitStatus>,
        stdout: &[u8],
        stderr: &[u8],
        failure: Option<&str>,
    ) {
        fs::write(self.path(&format!("{label}.stdout")), stdout)
            .unwrap_or_else(|error| panic!("cannot save raw stdout for {label}: {error}"));
        fs::write(self.path(&format!("{label}.stderr")), stderr)
            .unwrap_or_else(|error| panic!("cannot save raw stderr for {label}: {error}"));
        let record = format!(
            "label={label:?}\nstart_unix_ms={started}\nend_unix_ms={ended}\nexit_code={:?}\nsignal={:?}\nstatus={status:?}\ncwd={cwd:?}\nenv_clear=true\nenv={env:?}\nargv={argv:?}\nstdout_bytes={}\nstderr_bytes={}\nstdout_file={:?}\nstderr_file={:?}\nfailure={failure:?}\n",
            status.and_then(ExitStatus::code),
            status.and_then(ExitStatusExt::signal),
            stdout.len(),
            stderr.len(),
            self.path(&format!("{label}.stdout")),
            self.path(&format!("{label}.stderr")),
        );
        fs::write(self.path(&format!("{label}.receipt.txt")), record)
            .unwrap_or_else(|error| panic!("cannot save evidence receipt for {label}: {error}"));
    }

    fn finish(self) {
        if !self.keep {
            fs::remove_dir_all(&self.dir).unwrap_or_else(|error| {
                panic!(
                    "cannot remove successful evidence {}: {error}",
                    self.dir.display()
                )
            });
        }
    }
}

#[derive(Debug)]
struct Captured {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock precedes Unix epoch")
        .as_millis()
}

fn drain_pipe<R: Read + Send + 'static>(
    mut pipe: R,
    overflow: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
) -> JoinHandle<io::Result<Vec<u8>>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::with_capacity(8192);
        let mut buffer = [0u8; 8192];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => return Ok(bytes),
                Ok(count) => {
                    let available = OUTPUT_LIMIT.saturating_sub(bytes.len());
                    let retained = available.min(count);
                    bytes.extend_from_slice(&buffer[..retained]);
                    if retained < count {
                        overflow.store(true, Ordering::Relaxed);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    failed.store(true, Ordering::Relaxed);
                    return Err(error);
                }
            }
        }
    })
}

fn join_drain(handle: JoinHandle<io::Result<Vec<u8>>>, name: &str) -> Result<Vec<u8>, String> {
    match handle.join() {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(error)) => Err(format!("{name} read error: {error}")),
        Err(_) => Err(format!("{name} drain thread panicked")),
    }
}

fn fixture(evidence: &Evidence) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let root = evidence.fixture();
    let project = evidence.materialize(&root);
    let cache = evidence.path("cache");
    fs::create_dir(&cache).unwrap_or_else(|error| {
        panic!(
            "cannot create independent cache {}: {error}",
            cache.display()
        )
    });
    let home = evidence.path("home");
    fs::create_dir(&home)
        .unwrap_or_else(|error| panic!("cannot create isolated HOME {}: {error}", home.display()));
    (root, project, cache, home)
}

fn oxid(evidence: &Evidence, root: &Path, cache: &Path, home: &Path, args: &[&str]) -> Command {
    let mut command = evidence.command(root, cache, args);
    command.env("HOME", home);
    command
}

fn successful(evidence: &Evidence, label: &str, command: Command) -> Captured {
    let output = evidence
        .run(label, command)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        output.status.code(),
        Some(0),
        "{label}: {output:?}; evidence={}",
        evidence.dir.display()
    );
    assert!(
        output.stderr.is_empty(),
        "{label} stderr contamination: {:?}; evidence={}",
        output.stderr,
        evidence.dir.display()
    );
    output
}

fn assert_source_inventory(source: &str) {
    let inventory = source
        .lines()
        .filter_map(|line| {
            let declaration = line.trim_start().strip_prefix("fn test_")?;
            let name = declaration.split('(').next()?;
            Some(format!("test_{name}"))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        inventory, CASES,
        "root test_ function inventory must be exact and ordered"
    );
}

#[test]
fn algorithms_contracts_check_and_interpret_fixed_result() {
    let evidence = Evidence::new();
    let (root, project, cache, home) = fixture(&evidence);
    let manifest = project.join("oxid.toml");
    let manifest = manifest.to_str().expect("fixture path is UTF-8");

    let checked = successful(
        &evidence,
        "check-contracts",
        oxid(
            &evidence,
            &root,
            &cache,
            &home,
            &["check", "--manifest-path", manifest],
        ),
    );
    let summary = std::str::from_utf8(&checked.stdout).expect("check summary must be UTF-8");
    let count = summary
        .strip_prefix("typed-preview check ok (")
        .and_then(|rest| rest.strip_suffix(" functions; check only)\n"))
        .expect("one exact successful typed-preview check summary");
    assert!(
        !count.is_empty()
            && count.bytes().all(|byte| byte.is_ascii_digit())
            && count.parse::<usize>().is_ok_and(|value| value > 0),
        "non-positive or invalid checked function count: {summary:?}"
    );

    let package_source = root.join("packages/i32_algorithms/src/lib.ox");
    let client_source = project.join("src/main.ox");
    for (label, source) in [
        ("fmt-package", package_source),
        ("fmt-contracts", client_source),
    ] {
        let source = source.to_str().expect("fixture source path is UTF-8");
        let formatted = successful(
            &evidence,
            label,
            oxid(
                &evidence,
                &root,
                &cache,
                &home,
                &["--edition=typed-preview", "fmt", "--check", source],
            ),
        );
        assert!(
            formatted.stdout.is_empty(),
            "{label} unexpectedly emitted stdout: {:?}",
            formatted.stdout
        );
    }

    let result = successful(
        &evidence,
        "run-contracts",
        oxid(
            &evidence,
            &root,
            &cache,
            &home,
            &[
                "run",
                "--manifest-path",
                manifest,
                "--engine=interpreter",
                "--entry-mode=result",
            ],
        ),
    );
    assert_eq!(
        result.stdout,
        RESULT,
        "result-mode bytes; evidence={}",
        evidence.dir.display()
    );
    evidence.finish();
}

#[test]
fn algorithms_contracts_pass_every_named_case() {
    let evidence = Evidence::new();
    let (root, project, cache, home) = fixture(&evidence);
    let source_path = project.join("src/main.ox");
    let source = fs::read_to_string(&source_path).unwrap_or_else(|error| {
        panic!(
            "cannot inventory {}: {error}; evidence={}",
            source_path.display(),
            evidence.dir.display()
        )
    });
    assert_source_inventory(&source);
    let manifest = project.join("oxid.toml");
    let manifest = manifest.to_str().expect("fixture path is UTF-8");
    let tested = successful(
        &evidence,
        "test-contracts",
        oxid(
            &evidence,
            &root,
            &cache,
            &home,
            &["test", "--manifest-path", manifest, "--engine=interpreter"],
        ),
    );
    let mut expected = String::new();
    for case in CASES {
        expected.push_str(&format!("PASS {case}\n"));
    }
    expected.push_str("test result: 9 passed; 0 failed; 9 total\n");
    assert_eq!(
        tested.stdout,
        expected.as_bytes(),
        "fixed ordered test inventory; evidence={}",
        evidence.dir.display()
    );
    evidence.finish();
}
