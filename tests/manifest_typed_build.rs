//! Ordinary explicit-manifest BUILD acceptance (R1b) with the real native
//! toolchain. Tests marked `#[ignore]` need LLVM 19.1.7: run them with
//! `OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test
//! manifest_typed_build -- --include-ignored`. The remaining tests prove that
//! managed-output refusals precede the toolchain, so they need no LLVM.
//! No qualification, capacity or fuel probes; build is never asked to run code.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    fs,
    os::{
        fd::AsRawFd,
        unix::fs::{symlink, MetadataExt},
    },
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const NAME: &str = "typed-manifest-build-example";
const RESULT_OUT: &str = "example/.oxid/typed-preview/result/typed-manifest-build-example";
const PROCESS_OUT: &str = "example/.oxid/typed-preview/process/typed-manifest-build-example";
const MANIFEST: &str = "[project]\nname = \"typed-manifest-build-example\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n";
const MAIN: &str = "mod answer;\nfn main() -> i32 {\n    return crate::answer::value();\n}\n";
const ANSWER_7: &str = "pub fn value() -> i32 {\n    return 7;\n}\n";
const ANSWER_9: &str = "pub fn value() -> i32 {\n    return 9;\n}\n";
const ANSWER_BOOL: &str = "pub fn value() -> i32 {\n    return true;\n}\n";
const ANSWER_RECURSIVE: &str = "pub fn value() -> i32 {\n    return value();\n}\n";
/// Process-only root: bounded stdout write, status from the child module.
const PROCESS_MAIN: &str = "mod answer;\nuse std::io::write_stdout;\nuse std::io::WriteStatus;\nfn main() -> i32 {\n    let bytes = [79, 75, 10];\n    let written = write_stdout(&bytes);\n    match written {\n        WriteStatus::Complete => {\n            return crate::answer::value();\n        },\n        WriteStatus::InvalidInput => {\n            return 2;\n        },\n        WriteStatus::IoError(n) => {\n            return n + 3;\n        },\n    }\n}\n";
const TYPE_DIAGNOSTIC: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0300\",\"stage\":\"type\",\"message\":\"type mismatch: expected i32, found bool\",\"primary\":{\"file_id\":1,\"path\":\"example/src/answer.ox\",\"start\":35,\"end\":39,\"line\":2,\"column\":12,\"end_line\":2,\"end_column\":16},\"secondary\":[],\"notes\":[]}";
const STALE_STAGE: &str = "interrupted build staging exists; after confirming no build is active, remove only the output's .build/stage directory and retry";

fn summary(success: bool, output: Option<&str>, mode: &str) -> String {
    format!(
        "{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"build-summary\",\"success\":{success},\"errors\":{},\"output\":{},\"entry_mode\":\"{mode}\"}}",
        u8::from(!success),
        output.map_or("null".into(), |o| format!("\"{o}\""))
    )
}
fn e0710(message: &str) -> String {
    format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0710\",\"stage\":\"project-build\",\"message\":\"{message}\",\"primary\":null,\"secondary\":[],\"notes\":[]}}")
}
fn llvm_bin() -> PathBuf {
    let dir =
        PathBuf::from(std::env::var_os("OXID_TEST_LLVM_BIN").expect(
            "native build acceptance requires OXID_TEST_LLVM_BIN (LLVM 19.1.7 bin directory)",
        ));
    let version = Command::new(dir.join("clang"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&version.stdout).contains("clang version 19.1.7"),
        "{version:?}"
    );
    dir
}

struct Fixture {
    root: PathBuf,
    llvm: PathBuf,
}
impl Fixture {
    /// Copies the checked-in example to `<root>/example` (never built in-tree).
    fn new(llvm: Option<PathBuf>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-manifest-build-accept-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self {
            llvm: llvm.unwrap_or_else(|| root.join("absent-llvm")),
            root,
        };
        let example =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/projects/typed_manifest_build");
        for (file, expected) in [
            ("oxid.toml", MANIFEST),
            ("src/main.ox", MAIN),
            ("src/answer.ox", ANSWER_7),
        ] {
            let text = fs::read_to_string(example.join(file)).unwrap();
            assert_eq!(text, expected, "checked-in example {file} changed");
            fixture.write(&format!("example/{file}"), &text);
        }
        fixture
    }
    fn native() -> Self {
        Self::new(Some(llvm_bin()))
    }
    fn path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.path(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn command_in(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .args(args)
            .current_dir(cwd)
            .env("OXID_LLVM_BIN", &self.llvm)
            .env("OXID_CACHE_DIR", self.path("cache"))
            .env("PATH", self.path("absent-path"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null());
        command
    }
    fn oxid_in(&self, cwd: &Path, args: &[&str]) -> Output {
        self.command_in(cwd, args).output().unwrap()
    }
    fn oxid(&self, args: &[&str]) -> Output {
        self.oxid_in(&self.root, args)
    }
    fn build(&self, extra: &[&str]) -> Output {
        let mut args = vec!["build", "--manifest-path", "example/oxid.toml"];
        args.extend_from_slice(extra);
        self.oxid(&args)
    }
    fn build_json(&self, mode: &str) -> Output {
        self.build(&["--message-format=json", &format!("--entry-mode={mode}")])
    }
    fn exec(&self, path: &str) -> Output {
        Command::new(self.path(path))
            .current_dir("/")
            .env_clear()
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn compile(&self, mode: &str, output: &str) -> Output {
        self.oxid(&[
            "compile",
            "example/src/main.ox",
            "--edition=typed-preview",
            "--backend=llvm",
            &format!("--entry-mode={mode}"),
            &format!("--output={output}"),
            "--message-format=json",
        ])
    }
    fn state(&self, mode: &str) -> PathBuf {
        self.path(&format!("example/.oxid/typed-preview/{mode}/.{NAME}.build"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap()
}
fn ok_json(out: &Output, output: &str, mode: &str) {
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    assert_eq!(
        text(&out.stdout),
        format!("{}\n", summary(true, Some(output), mode))
    );
}
fn ok_text(out: &Output, output: &str) {
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    assert_eq!(
        text(&out.stdout),
        format!("typed-preview native build ok: \"{output}\"\n")
    );
}
/// JSON failure: exact diagnostic lines then exactly one failing build summary.
fn failed_json(out: &Output, mode: &str) -> Vec<String> {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    let mut lines: Vec<String> = text(&out.stdout).lines().map(str::to_owned).collect();
    assert_eq!(lines.pop().unwrap(), summary(false, None, mode));
    assert_eq!(text(&out.stdout).matches("summary").count(), 1);
    assert_eq!(lines.len(), 1, "{lines:?}");
    lines
}
fn refused(out: &Output, mode: &str, message: &str) {
    assert_eq!(failed_json(out, mode), [e0710(message)]);
}
/// (inode, bytes) of a published artifact.
fn identity(path: &Path) -> (u64, Vec<u8>) {
    let meta = fs::symlink_metadata(path).unwrap();
    assert!(meta.file_type().is_file(), "{path:?}");
    (meta.ino(), fs::read(path).unwrap())
}
fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}
fn assert_result_exec(out: &Output, value: &str) {
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(text(&out.stdout), format!("{value}\n"));
    assert!(out.stderr.is_empty(), "{out:?}");
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn result_first_build_executes_and_matches_compile_and_run_routes() {
    let f = Fixture::native();
    let out = f.build(&[]);
    ok_text(&out, RESULT_OUT);
    assert!(!text(&out.stdout).contains("\n7\n"));
    assert_eq!(entries(&f.path("example/.oxid/typed-preview")), ["result"]);
    assert_eq!(
        entries(&f.path("example/.oxid/typed-preview/result")),
        [format!(".{NAME}.build"), NAME.to_owned()]
    );
    // Only receipts and the owner marker remain; no stage or tool workspace.
    assert_eq!(entries(&f.state("result")), ["next", "owner"]);
    let meta = fs::metadata(f.path(RESULT_OUT)).unwrap();
    assert_eq!(meta.mode() & 0o111, 0o111);
    assert_result_exec(&f.exec(RESULT_OUT), "7");
    // Existing routes: explicit compile emits byte-identical executable bytes,
    // and manifest RUN on the reference runner agrees with the native result.
    let compiled = f.compile("result", "compiled-result");
    assert_eq!(compiled.status.code(), Some(0), "{compiled:?}");
    assert_eq!(
        fs::read(f.path("compiled-result")).unwrap(),
        fs::read(f.path(RESULT_OUT)).unwrap()
    );
    let run = f.oxid(&["run", "--manifest-path=example/oxid.toml"]);
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(run.stdout, f.exec(RESULT_OUT).stdout);
    // JSON success is exactly one summary naming the final path.
    ok_json(&f.build_json("result"), RESULT_OUT, "result");
    assert_result_exec(&f.exec(RESULT_OUT), "7");
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn process_first_build_executes_without_running_during_build() {
    let f = Fixture::native();
    // The checked-in example as a Process program: status 7, no stdout.
    ok_json(&f.build_json("process"), PROCESS_OUT, "process");
    let exec = f.exec(PROCESS_OUT);
    assert_eq!(exec.status.code(), Some(7), "{exec:?}");
    assert!(exec.stdout.is_empty() && exec.stderr.is_empty());
    assert!(!f.path("example/.oxid/typed-preview/result").exists());
    // A program with output: build itself prints only its summary.
    f.write("example/src/main.ox", PROCESS_MAIN);
    f.write(
        "example/src/answer.ox",
        "pub fn value() -> i32 {\n    return 39;\n}\n",
    );
    let out = f.build(&["--entry-mode", "process"]);
    ok_text(&out, PROCESS_OUT);
    assert!(!text(&out.stdout).contains("OK"));
    let exec = f.exec(PROCESS_OUT);
    assert_eq!(exec.status.code(), Some(39), "{exec:?}");
    assert_eq!(exec.stdout, b"OK\n");
    assert!(exec.stderr.is_empty());
    // Existing routes: Process compile bytes and Process run behaviour.
    let compiled = f.compile("process", "compiled-process");
    assert_eq!(compiled.status.code(), Some(0), "{compiled:?}");
    assert_eq!(
        fs::read(f.path("compiled-process")).unwrap(),
        fs::read(f.path(PROCESS_OUT)).unwrap()
    );
    let run = f.oxid(&[
        "run",
        "example/src/main.ox",
        "--edition=typed-preview",
        "--entry-mode=process",
    ]);
    assert_eq!(run.status.code(), exec.status.code());
    assert_eq!(run.stdout, exec.stdout);
    // The same program is refused in Result mode exactly as Result compile.
    let result = f.build_json("result");
    let lines = failed_json(&result, "result");
    assert!(lines[0].contains("\"code\":\"E0609\""), "{lines:?}");
    let compile = f.compile("result", "never");
    assert_eq!(text(&compile.stdout).lines().next().unwrap(), lines[0]);
    assert!(!f.path("never").exists());
    assert!(!f.path("example/.oxid/typed-preview/result").exists());
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn child_module_edit_rebuilds_and_replaces_only_managed_output() {
    let f = Fixture::native();
    ok_text(&f.build(&[]), RESULT_OUT);
    ok_text(&f.build(&["--entry-mode=process"]), PROCESS_OUT);
    let first = identity(&f.path(RESULT_OUT));
    let process_before = identity(&f.path(PROCESS_OUT));
    f.write("example/src/answer.ox", ANSWER_9);
    ok_text(&f.build(&[]), RESULT_OUT);
    let second = identity(&f.path(RESULT_OUT));
    assert_ne!(first.0, second.0);
    assert_ne!(first.1, second.1);
    assert_result_exec(&f.exec(RESULT_OUT), "9");
    // The previous executable is kept only as a receipt; the new one is `next`.
    assert_eq!(entries(&f.state("result")), ["current", "next", "owner"]);
    assert_eq!(
        fs::metadata(f.state("result").join("next")).unwrap().ino(),
        second.0
    );
    assert_eq!(
        fs::metadata(f.state("result").join("current"))
            .unwrap()
            .ino(),
        first.0
    );
    // The Process output of another mode is untouched until rebuilt.
    assert_eq!(identity(&f.path(PROCESS_OUT)), process_before);
    assert_eq!(f.exec(PROCESS_OUT).status.code(), Some(7));
    ok_text(&f.build(&["--entry-mode=process"]), PROCESS_OUT);
    assert_eq!(f.exec(PROCESS_OUT).status.code(), Some(9));
    // Rebuilding unchanged sources republishes (no incremental cache).
    ok_text(&f.build(&[]), RESULT_OUT);
    let third = identity(&f.path(RESULT_OUT));
    assert_ne!(third.0, second.0);
    assert_eq!(third.1, second.1);
    let compiled = f.compile("result", "compiled-9");
    assert_eq!(compiled.status.code(), Some(0));
    assert_eq!(fs::read(f.path("compiled-9")).unwrap(), third.1);
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn pre_publish_failures_keep_previous_artifact_and_match_existing_routes() {
    let f = Fixture::native();
    ok_text(&f.build(&[]), RESULT_OUT);
    let kept = identity(&f.path(RESULT_OUT));
    let check_kept = |f: &Fixture| {
        assert_eq!(identity(&f.path(RESULT_OUT)), kept);
        assert_result_exec(&f.exec(RESULT_OUT), "7");
        assert!(!f.state("result").join("stage").exists());
    };
    // Type error in the child module: fixed diagnostic, same as CHECK/compile.
    f.write("example/src/answer.ox", ANSWER_BOOL);
    let lines = failed_json(&f.build_json("result"), "result");
    assert_eq!(lines, [TYPE_DIAGNOSTIC]);
    let check = f.oxid(&[
        "check",
        "--manifest-path=example/oxid.toml",
        "--message-format=json",
    ]);
    assert_eq!(text(&check.stdout).lines().next().unwrap(), TYPE_DIAGNOSTIC);
    let compile = f.compile("result", "never");
    assert_eq!(
        text(&compile.stdout).lines().next().unwrap(),
        TYPE_DIAGNOSTIC
    );
    assert!(text(&compile.stdout).contains("compile-summary"));
    check_kept(&f);
    // Human diagnostics equal the explicit compile route; stdout stays empty.
    let human = f.build(&[]);
    assert_eq!(human.status.code(), Some(1));
    assert!(human.stdout.is_empty());
    let source = f.oxid(&[
        "compile",
        "example/src/main.ox",
        "--edition=typed-preview",
        "--backend=llvm",
        "--output=never",
    ]);
    assert_eq!(human.stderr, source.stderr);
    assert!(text(&human.stderr).contains("error[E0300] (type)"));
    check_kept(&f);
    // Native admission failure (whole-program recursion) also precedes output.
    f.write("example/src/answer.ox", ANSWER_RECURSIVE);
    let lines = failed_json(&f.build_json("result"), "result");
    assert!(lines[0].contains("\"code\":\"E0700\""), "{lines:?}");
    let compile = f.compile("result", "never");
    assert_eq!(text(&compile.stdout).lines().next().unwrap(), lines[0]);
    check_kept(&f);
    // Toolchain failure after staging: previous artifact and receipts survive.
    f.write("example/src/answer.ox", ANSWER_9);
    let mut missing = f.command_in(
        &f.root,
        &[
            "build",
            "--manifest-path=example/oxid.toml",
            "--message-format=json",
        ],
    );
    let out = missing
        .env("OXID_LLVM_BIN", f.path("absent-llvm"))
        .output()
        .unwrap();
    let lines = failed_json(&out, "result");
    assert!(
        lines[0].starts_with("{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0701\",\"stage\":\"native-toolchain\",\"message\":\"cannot launch "),
        "{lines:?}"
    );
    check_kept(&f);
    assert!(!f.path("never").exists());
    // Recovery: the next good build replaces the kept artifact.
    ok_text(&f.build(&[]), RESULT_OUT);
    assert_result_exec(&f.exec(RESULT_OUT), "9");
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn unmanaged_files_symlinks_and_low_level_outputs_are_refused_with_toolchain_present() {
    let f = Fixture::native();
    let result_dir = f.path("example/.oxid/typed-preview/result");
    let out = f.path(RESULT_OUT);
    fs::create_dir_all(&result_dir).unwrap();
    // Low-level compile output at the managed path grants no ownership.
    let compiled = f.compile("result", RESULT_OUT);
    assert_eq!(compiled.status.code(), Some(0), "{compiled:?}");
    let low = identity(&out);
    refused(
        &f.build_json("result"),
        "result",
        "refusing to replace an unowned managed output",
    );
    assert_eq!(identity(&out), low);
    fs::remove_file(&out).unwrap();
    // A user file, a symlink (live or dangling) and a directory are refused.
    f.write("outside/target", "outside bytes");
    for (setup, message) in [
        (0, "refusing to replace an unowned managed output"),
        (
            1,
            "managed output must be a regular file; symlinks are rejected",
        ),
        (
            2,
            "managed output must be a regular file; symlinks are rejected",
        ),
        (
            3,
            "managed output must be a regular file; symlinks are rejected",
        ),
    ] {
        match setup {
            0 => fs::write(&out, "user bytes").unwrap(),
            1 => symlink(f.path("outside/target"), &out).unwrap(),
            2 => symlink(f.path("outside/missing"), &out).unwrap(),
            _ => fs::create_dir(&out).unwrap(),
        }
        refused(&f.build_json("result"), "result", message);
        assert_eq!(
            fs::read_to_string(f.path("outside/target")).unwrap(),
            "outside bytes"
        );
        assert_eq!(entries(&f.path("outside")), ["target"]);
        if setup == 0 {
            assert_eq!(fs::read_to_string(&out).unwrap(), "user bytes");
        }
        if setup == 3 {
            fs::remove_dir(&out).unwrap();
        } else {
            fs::remove_file(&out).unwrap();
        }
    }
    // A managed artifact replaced by a byte-identical copy is no longer owned.
    ok_text(&f.build(&[]), RESULT_OUT);
    let bytes = fs::read(&out).unwrap();
    fs::remove_file(&out).unwrap();
    fs::write(&out, &bytes).unwrap();
    refused(
        &f.build_json("result"),
        "result",
        "refusing to replace an unowned managed output",
    );
    assert_eq!(fs::read(&out).unwrap(), bytes);
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn different_cwd_and_symlinked_parent_build_the_same_managed_output() {
    let f = Fixture::native();
    fs::create_dir_all(f.path("elsewhere/deep")).unwrap();
    let deep = f.oxid_in(
        &f.path("elsewhere/deep"),
        &[
            "build",
            "--manifest-path=../../example/oxid.toml",
            "--message-format=json",
        ],
    );
    ok_json(&deep, &format!("../../{RESULT_OUT}"), "result");
    let first = identity(&f.path(RESULT_OUT));
    assert_eq!(entries(&f.path("elsewhere/deep")), Vec::<String>::new());
    assert_result_exec(&f.exec(RESULT_OUT), "7");
    // Absolute manifest path from `/`, and the project directory itself.
    let absolute = f.path("example/oxid.toml");
    let out = f.oxid_in(
        Path::new("/"),
        &["build", "--manifest-path", absolute.to_str().unwrap()],
    );
    ok_text(&out, f.path(RESULT_OUT).to_str().unwrap());
    let inside = f.oxid_in(&f.path("example"), &["build", "--manifest-path=oxid.toml"]);
    ok_text(&inside, &format!(".oxid/typed-preview/result/{NAME}"));
    assert_ne!(identity(&f.path(RESULT_OUT)).0, first.0);
    // The manifest's lexical parent may be a symlink (R1a semantics); the
    // reported path stays lexical and the artifact lands in the real project.
    symlink(f.path("example"), f.path("alias")).unwrap();
    let alias = f.oxid(&["build", "--manifest-path=alias/oxid.toml"]);
    ok_text(&alias, &format!("alias/.oxid/typed-preview/result/{NAME}"));
    assert!(fs::symlink_metadata(f.path("alias"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert_result_exec(&f.exec(RESULT_OUT), "7");
    // The artifact runs from any cwd with an empty environment.
    let mut exec = Command::new(f.path(RESULT_OUT));
    let exec = exec.current_dir("/").env_clear().output().unwrap();
    assert_result_exec(&exec, "7");
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn reporting_failure_after_publication_is_explicit() {
    let f = Fixture::native();
    ok_text(&f.build(&[]), RESULT_OUT);
    f.write("example/src/answer.ox", ANSWER_9);
    for json in [false, true] {
        let old = identity(&f.path(RESULT_OUT));
        let mut args = vec!["build", "--manifest-path=example/oxid.toml"];
        if json {
            args.push("--message-format=json");
        }
        let full = fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap();
        let out = f
            .command_in(&f.root, &args)
            .stdout(full)
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert_eq!(
            text(&out.stderr),
            format!("error: build reporting failed: No space left on device (os error 28); executable already published at \"{RESULT_OUT}\"; previous artifact was not restored\n")
        );
        // Published despite the report failure; the old bytes are not restored.
        let now = identity(&f.path(RESULT_OUT));
        assert_ne!(now.0, old.0);
        assert_result_exec(&f.exec(RESULT_OUT), "9");
    }
    // A report failure before publication says nothing was published.
    let kept = identity(&f.path(RESULT_OUT));
    f.write("example/src/answer.ox", ANSWER_BOOL);
    let full = fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .unwrap();
    let out = f
        .command_in(
            &f.root,
            &[
                "build",
                "--manifest-path=example/oxid.toml",
                "--message-format=json",
            ],
        )
        .stdout(full)
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        text(&out.stderr),
        "error: build reporting failed: No space left on device (os error 28); no executable was published\n"
    );
    assert_eq!(identity(&f.path(RESULT_OUT)), kept);
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn lock_and_stale_stage_refuse_then_recover_with_toolchain_present() {
    let f = Fixture::native();
    ok_text(&f.build(&[]), RESULT_OUT);
    let kept = identity(&f.path(RESULT_OUT));
    f.write("example/src/answer.ox", ANSWER_9);
    // Another holder of the per-output lock excludes this build.
    let lock = fs::File::open(f.state("result")).unwrap();
    assert_eq!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    refused(
        &f.build_json("result"),
        "result",
        "another build owns this output; retry after it finishes",
    );
    assert_eq!(identity(&f.path(RESULT_OUT)), kept);
    // A different mode's output is a different lock.
    ok_text(&f.build(&["--entry-mode=process"]), PROCESS_OUT);
    drop(lock);
    // A leftover stage is refused, not replayed; removing only it recovers.
    fs::create_dir(f.state("result").join("stage")).unwrap();
    fs::write(f.state("result").join("stage/executable"), "stale").unwrap();
    refused(&f.build_json("result"), "result", STALE_STAGE);
    assert_eq!(identity(&f.path(RESULT_OUT)), kept);
    assert_eq!(
        fs::read_to_string(f.state("result").join("stage/executable")).unwrap(),
        "stale"
    );
    fs::remove_dir_all(f.state("result").join("stage")).unwrap();
    ok_text(&f.build(&[]), RESULT_OUT);
    assert_result_exec(&f.exec(RESULT_OUT), "9");
    assert!(!f.state("result").join("stage").exists());
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn compile_output_still_never_overwrites_managed_or_symlinked_paths() {
    let f = Fixture::native();
    ok_text(&f.build(&[]), RESULT_OUT);
    let kept = identity(&f.path(RESULT_OUT));
    f.write("example/src/answer.ox", ANSWER_9);
    let out = f.compile("result", RESULT_OUT);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(
        text(&out.stdout),
        "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0701\",\"stage\":\"native-toolchain\",\"message\":\"native output already exists; choose a new path (existing files and symlinks are never overwritten)\",\"primary\":null,\"secondary\":[],\"notes\":[]}\n{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"compile-summary\",\"success\":false,\"errors\":1,\"output\":null}\n"
    );
    assert_eq!(identity(&f.path(RESULT_OUT)), kept);
    symlink(f.path(RESULT_OUT), f.path("link")).unwrap();
    let out = f.compile("process", "link");
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(text(&out.stdout).contains("native output already exists"));
    assert_eq!(identity(&f.path(RESULT_OUT)), kept);
    // The managed build still owns and replaces its own output afterwards.
    ok_text(&f.build(&[]), RESULT_OUT);
    assert_result_exec(&f.exec(RESULT_OUT), "9");
}

#[test]
#[ignore = "requires LLVM 19.1.7 via OXID_TEST_LLVM_BIN"]
fn example_readme_commands_from_examples_projects() {
    let f = Fixture::native();
    fs::rename(f.path("example"), f.path("typed_manifest_build")).unwrap();
    let out = f.oxid(&["build", "--manifest-path", "typed_manifest_build/oxid.toml"]);
    let result = format!("typed_manifest_build/.oxid/typed-preview/result/{NAME}");
    ok_text(&out, &result);
    assert_result_exec(&f.exec(&result), "7");
    let process = format!("typed_manifest_build/.oxid/typed-preview/process/{NAME}");
    let out = f.oxid(&[
        "build",
        "--manifest-path",
        "typed_manifest_build/oxid.toml",
        "--entry-mode",
        "process",
    ]);
    ok_text(&out, &process);
    let exec = Command::new(f.path(&process)).output().unwrap();
    assert_eq!(exec.status.code(), Some(7));
    assert!(exec.stdout.is_empty());
    let out = f.oxid(&[
        "build",
        "--manifest-path",
        "typed_manifest_build/oxid.toml",
        "--message-format=json",
    ]);
    ok_json(&out, &result, "result");
    let readme = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/projects/typed_manifest_build/README.md"),
    )
    .unwrap();
    assert!(readme.contains(&format!("typed-preview native build ok: \"{result}\"")));
    assert!(readme.contains(&summary(true, Some(&result), "result")));
}

// ---- No toolchain needed: managed refusals precede LLVM resolution. ----

#[test]
fn managed_refusals_precede_toolchain_and_write_nothing_outside() {
    let f = Fixture::new(None);
    let typed = f.path("example/.oxid/typed-preview");
    f.write("outside/keep", "outside bytes");
    let not_dir = "managed native output I/O failed: Not a directory (os error 20)";
    // Symlinked managed components are not followed.
    symlink(f.path("outside"), f.path("example/.oxid")).unwrap();
    refused(&f.build_json("result"), "result", not_dir);
    fs::remove_file(f.path("example/.oxid")).unwrap();
    fs::create_dir(f.path("example/.oxid")).unwrap();
    symlink(f.path("outside"), &typed).unwrap();
    refused(&f.build_json("process"), "process", not_dir);
    fs::remove_file(&typed).unwrap();
    fs::create_dir(&typed).unwrap();
    for mode in ["result", "process"] {
        symlink(f.path("outside"), typed.join(mode)).unwrap();
        refused(&f.build_json(mode), mode, not_dir);
        fs::remove_file(typed.join(mode)).unwrap();
    }
    assert_eq!(entries(&f.path("outside")), ["keep"]);
    // An unowned file is refused with E0710, not the missing-toolchain E0701.
    f.write(
        &format!("example/.oxid/typed-preview/result/{NAME}"),
        "mine",
    );
    refused(
        &f.build_json("result"),
        "result",
        "refusing to replace an unowned managed output",
    );
    assert_eq!(fs::read_to_string(f.path(RESULT_OUT)).unwrap(), "mine");
    // An invalid persistent state directory is reported for inspection.
    fs::write(f.state("result").join("owner"), "not a marker").unwrap();
    refused(
        &f.build_json("result"),
        "result",
        "unrecognized managed build directory; inspect it before retrying",
    );
    assert_eq!(
        fs::read_to_string(f.state("result").join("owner")).unwrap(),
        "not a marker"
    );
    // Without ownership problems the missing toolchain is reached and reported.
    let lines = failed_json(&f.build_json("process"), "process");
    assert!(lines[0].contains("\"code\":\"E0701\""), "{lines:?}");
    assert!(!f.path(PROCESS_OUT).exists());
    assert_eq!(entries(&f.state("process")), ["owner"]);
    assert_eq!(entries(&f.path("outside")), ["keep"]);
}

#[test]
fn stale_stage_and_lock_refusals_precede_toolchain() {
    let f = Fixture::new(None);
    // First attempt creates only the state directory before E0701.
    let lines = failed_json(&f.build_json("result"), "result");
    assert!(lines[0].contains("\"code\":\"E0701\""), "{lines:?}");
    assert_eq!(entries(&f.state("result")), ["owner"]);
    let lock = fs::File::open(f.state("result")).unwrap();
    assert_eq!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    refused(
        &f.build_json("result"),
        "result",
        "another build owns this output; retry after it finishes",
    );
    drop(lock);
    fs::create_dir(f.state("result").join("stage")).unwrap();
    refused(&f.build_json("result"), "result", STALE_STAGE);
    assert!(f.state("result").join("stage").is_dir());
    // Text mode: diagnostics on stderr, no success line, exit 1.
    let out = f.build(&[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(text(&out.stderr).contains("E0710"), "{out:?}");
    assert!(text(&out.stderr).contains(STALE_STAGE), "{out:?}");
}
