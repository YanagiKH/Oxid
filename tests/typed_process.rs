//! Public process routes are observed in isolated CLI subprocesses. Linux
//! execution and portable terminal diagnostics are distinct host qualifications.
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const QUALIFIED: bool = cfg!(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
));
const TERMINAL: bool = QUALIFIED
    || cfg!(all(
        target_os = "macos",
        target_pointer_width = "64",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ));

struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-process-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        Self(root)
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .args(args)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env("OXID_LLVM_BIN", self.0.join("missing-tools"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn invoke(&self, args: &[&str]) -> Output {
        wait(self.command(args).spawn().unwrap())
    }

    fn process(&self) -> Output {
        self.invoke(&[
            "run",
            "main.ox",
            "--edition=typed-preview",
            "--entry-mode=process",
        ])
    }

    fn untouched(&self) {
        assert!(!self.0.join("program").exists());
        assert!(!self.0.join("cache").exists());
        assert!(!self.0.join("sentinel.txt").exists());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn wait(mut child: Child) -> Output {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            panic!(
                "process route timed out: {:?}",
                child.wait_with_output().unwrap()
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn process_error(output: Output, code: &str, stage: &str) -> String {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(
        text.starts_with(&format!("error[{code}] ({stage}): ")),
        "{text}"
    );
    assert!(!text.contains("-summary"), "{text}");
    text
}

#[test]
fn process_argv_errors_are_text_only_before_source_access_or_legacy_fallback() {
    let fixture = Fixture::new("write_text(\"sentinel.txt\", \"executed\");");
    for args in [
        vec![
            "run",
            "missing.ox",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--message-format=json",
        ],
        vec![
            "run",
            "missing.ox",
            "--edition=typed-preview",
            "--message-format=json",
            "--entry-mode",
        ],
        vec![
            "run",
            "missing.ox",
            "--edition=typed-preview",
            "--message-format=json",
            "--entry-mode=",
        ],
        vec![
            "run",
            "missing.ox",
            "--edition=typed-preview",
            "--message-format=json",
            "--entry-mode=unknown",
        ],
        vec![
            "run",
            "missing.ox",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--entry-mode=result",
            "--message-format=json",
        ],
        vec![
            "run",
            "missing.ox",
            "--edition=unknown",
            "--message-format=json",
            "--entry-mode=process",
        ],
        vec![
            "run",
            "missing.ox",
            "--edition=typed-preview",
            "--unknown",
            "--entry-mode=process",
        ],
        vec![
            "run",
            "missing.ox",
            "extra.ox",
            "--edition=typed-preview",
            "--entry-mode=process",
        ],
        vec!["run", "--edition=typed-preview", "--entry-mode=process"],
        vec!["--entry-mode=process"],
        vec!["unknown", "main.ox", "--entry-mode=process"],
        vec!["main.ox", "--entry-mode=process"],
        vec![
            "run",
            "main.ox",
            "--edition=legacy-0.9",
            "--entry-mode=process",
        ],
    ] {
        let output = fixture.invoke(&args);
        if TERMINAL {
            let text = process_error(output, "E0001", "cli");
            assert!(!text.contains("E0002"), "{args:?}: {text}");
        } else {
            assert_eq!(output.status.code(), Some(74), "{output:?}");
            assert!(output.stdout.is_empty() && output.stderr.is_empty());
        }
        fixture.untouched();
    }
}

#[test]
fn ordinary_nonexecuting_and_result_errors_preserve_json_reporting() {
    let fixture = Fixture::new("fn helper()->i32{return 7;}");
    for command in ["check", "fmt", "compile"] {
        for entry in [
            "--entry-mode=process",
            "--entry-mode=unknown",
            "--entry-mode=",
        ] {
            let out = fixture.invoke(&[
                command,
                "missing.ox",
                "--edition=typed-preview",
                "--message-format=json",
                entry,
            ]);
            assert_eq!(out.status.code(), Some(1), "{out:?}");
            assert!(out.stderr.is_empty(), "{out:?}");
            let text = String::from_utf8(out.stdout).unwrap();
            assert!(text.contains("\"code\":\"E0001\""), "{text}");
            assert!(text.contains("-summary"), "{text}");
        }
    }
    let out = fixture.invoke(&[
        "run",
        "missing.ox",
        "--edition=typed-preview",
        "--entry-mode=result",
        "--message-format=json",
        "--unknown",
    ]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stderr.is_empty());
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("run-summary"));
    let check = fixture.invoke(&[
        "check",
        "main.ox",
        "--edition=typed-preview",
        "--message-format=json",
    ]);
    assert_eq!(check.status.code(), Some(0), "{check:?}");
    assert!(check.stderr.is_empty());
    assert!(String::from_utf8(check.stdout)
        .unwrap()
        .contains("\"success\":true"));
    fixture.untouched();
}

#[test]
fn process_compile_json_is_a_build_report_without_source_effects() {
    let fixture = Fixture::new("fn main()->i32{return true;}");
    let out = fixture.invoke(&[
        "compile",
        "main.ox",
        "--edition=typed-preview",
        "--entry-mode=process",
        "--backend=llvm",
        "--output=program",
        "--message-format=json",
    ]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    let text = String::from_utf8(out.stdout).unwrap();
    let code = if QUALIFIED { "E0300" } else { "E0608" };
    assert!(text.contains(&format!("\"code\":\"{code}\"")), "{text}");
    assert!(
        text.contains("\"kind\":\"compile-summary\",\"success\":false"),
        "{text}"
    );
    assert!(!text.contains("native-toolchain"), "{text}");
    fixture.untouched();
}

#[test]
fn unsupported_process_host_is_rejected_before_source_and_output_access() {
    if QUALIFIED {
        return;
    }
    let fixture = Fixture::new("invalid source must not load");
    let out = fixture.invoke(&[
        "run",
        "missing.ox",
        "--edition=typed-preview",
        "--entry-mode=process",
    ]);
    if TERMINAL {
        assert_eq!(
            process_error(out, "E0608", "oir-run"),
            "error[E0608] (oir-run): process execution requires Linux x86_64\n"
        );
    } else {
        assert_eq!(out.status.code(), Some(74));
        assert!(out.stdout.is_empty() && out.stderr.is_empty());
    }
    let out = fixture.invoke(&[
        "compile",
        "missing.ox",
        "--edition=typed-preview",
        "--entry-mode=process",
        "--backend=llvm",
        "--output=program",
        "--message-format=json",
    ]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stderr.is_empty());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains("\"code\":\"E0608\",\"stage\":\"oir-run\""),
        "{text}"
    );
    assert!(!text.contains("E0002"), "{text}");
    fixture.untouched();
}

#[test]
fn process_scalar_and_owned_statuses_have_no_stdout_trailer() {
    if !QUALIFIED {
        return;
    }
    for status in [0, 1, 39, 63, 255] {
        for source in [
            format!("fn main()->i32{{return {status};}}"),
            format!("fn main()->i32{{let a=[{status}];return a[0];}}"),
        ] {
            let fixture = Fixture::new(&source);
            let out = fixture.process();
            assert_eq!(out.status.code(), Some(status), "{source}: {out:?}");
            assert!(out.stdout.is_empty() && out.stderr.is_empty(), "{out:?}");
            let ordinary = fixture.invoke(&["run", "main.ox", "--edition=typed-preview"]);
            assert_eq!(ordinary.status.code(), Some(0), "{ordinary:?}");
            assert_eq!(ordinary.stdout, format!("{status}\n").as_bytes());
            assert!(ordinary.stderr.is_empty());
            fixture.untouched();
        }
    }
    for status in [-256, 256] {
        for source in [
            format!("fn main()->i32{{return {status};}}"),
            format!("fn main()->i32{{let a=[{status}];return a[0];}}"),
        ] {
            let text = process_error(Fixture::new(&source).process(), "E0600", "oir-run");
            assert!(
                text.contains("process main must return a status in 0..255"),
                "{text}"
            );
        }
    }
}

#[test]
fn process_source_and_entry_failures_stay_on_stderr() {
    if !QUALIFIED {
        return;
    }
    for source in [
        "",
        "fn helper()->i32{return 1;}",
        "fn main()->bool{return false;}",
        "fn main()->(){return;}",
        "fn main(n:i32)->i32{return n;}",
    ] {
        let fixture = Fixture::new(source);
        let text = process_error(fixture.process(), "E0600", "oir-run");
        assert!(
            text.contains(
                "process entry requires original-root fn main() -> i32 with no parameters"
            ),
            "{text}"
        );
        let compile = fixture.invoke(&[
            "compile",
            "main.ox",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--backend=llvm",
            "--output=program",
            "--message-format=json",
        ]);
        assert_eq!(compile.status.code(), Some(1), "{compile:?}");
        assert!(compile.stderr.is_empty());
        let text = String::from_utf8(compile.stdout).unwrap();
        assert!(text.contains("\"code\":\"E0600\""), "{text}");
        assert!(!text.contains("native-toolchain"));
        fixture.untouched();
    }
    let fixture = Fixture::new("fn main()->i32{return true;}");
    process_error(fixture.process(), "E0300", "type");
    process_error(
        fixture.invoke(&[
            "run",
            "missing.ox",
            "--edition=typed-preview",
            "--entry-mode=process",
        ]),
        "E0002",
        "source",
    );
    let child = Fixture::new("mod child; use crate::child::main;");
    fs::write(child.0.join("child.ox"), "pub fn main()->i32{return 0;}").unwrap();
    process_error(child.process(), "E0600", "oir-run");
}

#[test]
fn process_output_is_exact_and_result_inventory_denial_covers_compile() {
    if !QUALIFIED {
        return;
    }
    let fixture = Fixture::new("use std::io::write_stdout as emit; use std::io::WriteStatus as Written; fn main()->i32{let bytes=[0,65,255];let written=emit(&bytes);match written{Written::Complete=>{return 39;},Written::InvalidInput=>{return 2;},Written::IoError(n)=>{return n+3;},}}");
    let out = fixture.process();
    assert_eq!(out.status.code(), Some(39), "{out:?}");
    assert_eq!(out.stdout, [0, 65, 255]);
    assert!(out.stderr.is_empty());
    for source in [
        "use std::io::write_stdout; fn main()->i32{return 0;}",
        "use std::io::write_stdout as unused; fn main()->i32{return 0;}",
    ] {
        let fixture = Fixture::new(source);
        for operation in ["run", "compile"] {
            let mut args = vec![
                operation,
                "main.ox",
                "--edition=typed-preview",
                "--message-format=json",
            ];
            if operation == "compile" {
                args.extend(["--backend=llvm", "--output=program"]);
            }
            let out = fixture.invoke(&args);
            assert_eq!(out.status.code(), Some(1), "{out:?}");
            assert!(out.stderr.is_empty());
            let text = String::from_utf8(out.stdout).unwrap();
            assert!(
                text.contains("\"code\":\"E0609\",\"stage\":\"oir-owned-run\""),
                "{text}"
            );
            assert!(
                text.contains("bounded stdout execution requires process entry mode"),
                "{text}"
            );
            assert!(!text.contains("native-toolchain"));
            fixture.untouched();
        }
    }
    let status_only = Fixture::new("use std::io::WriteStatus; fn main()->i32{return 7;}");
    let out = status_only.invoke(&["run", "main.ox", "--edition=typed-preview"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(out.stdout, b"7\n");
    assert!(out.stderr.is_empty());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn process_closed_reader_stderr_returns_74_without_stdout_fallback() {
    use std::os::fd::FromRawFd;
    let fixture = Fixture::new("fn main()->i32{return 0;}");
    for args in [
        vec![
            "run",
            "main.ox",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--message-format=json",
        ],
        vec![
            "run",
            "missing.ox",
            "--edition=typed-preview",
            "--entry-mode=process",
        ],
    ] {
        let mut command = fixture.command(&args);
        // Keep fd2 valid across Rust startup, which repairs a missing standard
        // descriptor. A real pipe with no reader instead fails its first write.
        let mut descriptors = [-1; 2];
        unsafe {
            unsafe extern "C" {
                fn pipe(descriptors: *mut std::ffi::c_int) -> std::ffi::c_int;
                fn close(fd: std::ffi::c_int) -> std::ffi::c_int;
            }
            assert_eq!(pipe(descriptors.as_mut_ptr()), 0);
            assert_eq!(close(descriptors[0]), 0);
            // Ownership of the sole write end transfers once to Command.
            command.stderr(Stdio::from(fs::File::from_raw_fd(descriptors[1])));
        }
        let out = wait(command.spawn().unwrap());
        assert_eq!(out.status.code(), Some(74), "{out:?}");
        assert!(out.stdout.is_empty() && out.stderr.is_empty(), "{out:?}");
        fixture.untouched();
    }
}
