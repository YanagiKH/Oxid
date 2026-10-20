//! Ordinary text compiler diagnostics, with exact JSON and RUN controls.
//! Invalid inputs stop before native admission; no LLVM or native execution.
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

const TYPE_ERROR: &str = "fn main() -> i32 {\n    return true;\n}\n";
const PARSE_ERROR: &str = "fn main() -> i32 {\n    return 1 }\n";
const TYPE_HEADER: &str =
    "error[E0300] (type): type mismatch: expected i32, found bool\n  --> main.ox:2:12\n";
const PARSE_HEADER: &str = "error[E0100] (parse): statement requires `;`\n  --> main.ox:2:14\n";
const TYPE_JSON: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0300\",\"stage\":\"type\",\"message\":\"type mismatch: expected i32, found bool\",\"primary\":{\"file_id\":0,\"path\":\"main.ox\",\"start\":30,\"end\":34,\"line\":2,\"column\":12,\"end_line\":2,\"end_column\":16},\"secondary\":[],\"notes\":[]}";
const PARSE_JSON: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":\"E0100\",\"stage\":\"parse\",\"message\":\"statement requires `;`\",\"primary\":{\"file_id\":0,\"path\":\"main.ox\",\"start\":32,\"end\":33,\"line\":2,\"column\":14,\"end_line\":2,\"end_column\":15},\"secondary\":[],\"notes\":[]}";
const CHECK_FAILURE: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":1,\"functions\":null}";
const COMPILE_FAILURE: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"compile-summary\",\"success\":false,\"errors\":1,\"output\":null}";
const BUILD_FAILURE: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"build-summary\",\"success\":false,\"errors\":1,\"output\":null,\"entry_mode\":\"result\"}";
const RUN_FAILURE: &str = "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":false,\"errors\":1,\"result\":null}";

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-diagnostic-context-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        fs::write(
            root.join("oxid.toml"),
            "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n",
        )
        .unwrap();
        // Failed compilation must preserve a pre-existing requested output.
        fs::write(root.join("program"), "existing output\n").unwrap();
        Self(root)
    }

    fn command(&self, args: &[&str]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .env("OXID_LLVM_BIN", self.0.join("missing-llvm"))
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env_remove("OXID_PATH")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let drain = |mut pipe: Box<dyn Read + Send>| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                pipe.read_to_end(&mut bytes).unwrap();
                bytes
            })
        };
        let stdout = drain(Box::new(child.stdout.take().unwrap()));
        let stderr = drain(Box::new(child.stderr.take().unwrap()));
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                return Output {
                    status,
                    stdout: stdout.join().unwrap(),
                    stderr: stderr.join().unwrap(),
                };
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!(
                    "diagnostic command timed out: {args:?}; stdout={:?}; stderr={:?}",
                    stdout.join().unwrap(),
                    stderr.join().unwrap()
                );
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn source(&self, operation: &str, json: bool) -> Output {
        let mut args = vec![operation, "main.ox", "--edition=typed-preview"];
        if operation == "compile" {
            args.extend(["--backend=llvm", "--output=program"]);
        }
        if json {
            args.push("--message-format=json");
        }
        self.command(&args)
    }

    fn manifest(&self, operation: &str, json: bool) -> Output {
        let mut args = vec![operation, "--manifest-path=oxid.toml"];
        if json {
            args.push("--message-format=json");
        }
        self.command(&args)
    }

    fn unchanged_files(&self, source: &str) {
        assert_eq!(fs::read_to_string(self.0.join("main.ox")).unwrap(), source);
        assert_eq!(
            fs::read(self.0.join("program")).unwrap(),
            b"existing output\n"
        );
        let mut names: Vec<_> = fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, vec!["main.ox", "oxid.toml", "program"]);
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn text_context(output: &Output, old_header: &str, source_line: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let text = std::str::from_utf8(&output.stderr).unwrap();
    assert!(text.starts_with(old_header), "{text}");
    let context = &text[old_header.len()..];
    assert!(context.contains(source_line), "{text}");
    assert!(context.lines().any(|line| line.contains('^')), "{text}");
    assert!(
        !text.contains("E0701"),
        "frontend failure reached toolchain: {text}"
    );
    assert!(!text.contains('\u{1b}'), "{text}");
}

#[test]
fn ordinary_check_and_compile_show_source_context_for_syntax_and_type_errors() {
    for (source, header, line) in [
        (TYPE_ERROR, TYPE_HEADER, "return true;"),
        (PARSE_ERROR, PARSE_HEADER, "return 1 }"),
    ] {
        let project = Project::new(source);
        for operation in ["check", "compile"] {
            text_context(&project.source(operation, false), header, line);
        }
        project.unchanged_files(source);
    }
}

#[test]
fn manifest_check_and_build_show_source_context_before_toolchain_or_publication() {
    for (source, header, line) in [
        (TYPE_ERROR, TYPE_HEADER, "return true;"),
        (PARSE_ERROR, PARSE_HEADER, "return 1 }"),
    ] {
        let project = Project::new(source);
        for operation in ["check", "build"] {
            text_context(&project.manifest(operation, false), header, line);
        }
        project.unchanged_files(source);
    }
}

#[test]
fn source_and_manifest_json_keep_exact_original_diagnostic_and_summary_records() {
    for (source, diagnostic) in [(TYPE_ERROR, TYPE_JSON), (PARSE_ERROR, PARSE_JSON)] {
        let project = Project::new(source);
        for (output, summary) in [
            (project.source("check", true), CHECK_FAILURE),
            (project.source("compile", true), COMPILE_FAILURE),
            (project.manifest("check", true), CHECK_FAILURE),
            (project.manifest("build", true), BUILD_FAILURE),
        ] {
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            assert!(output.stderr.is_empty(), "{output:?}");
            assert_eq!(
                output.stdout,
                format!("{diagnostic}\n{summary}\n").as_bytes()
            );
        }
        project.unchanged_files(source);
    }
}

#[test]
fn result_run_source_failures_keep_exact_old_text_and_json_without_context() {
    for (source, header, diagnostic) in [
        (TYPE_ERROR, TYPE_HEADER, TYPE_JSON),
        (PARSE_ERROR, PARSE_HEADER, PARSE_JSON),
    ] {
        let project = Project::new(source);
        for output in [project.source("run", false), project.manifest("run", false)] {
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            assert_eq!(output.stderr, header.as_bytes());
        }
        for output in [project.source("run", true), project.manifest("run", true)] {
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            assert!(output.stderr.is_empty(), "{output:?}");
            assert_eq!(
                output.stdout,
                format!("{diagnostic}\n{RUN_FAILURE}\n").as_bytes()
            );
        }
        project.unchanged_files(source);
    }
}

#[test]
fn result_runtime_failure_keeps_the_original_two_line_diagnostic() {
    let source = "fn main() -> i32 { return 7 / 0; }";
    let project = Project::new(source);
    let column = source.find('/').unwrap() + 1;
    let expected =
        format!("error[E0607] (oir-run): checked i32 division by zero\n  --> main.ox:1:{column}\n");
    for output in [project.source("run", false), project.manifest("run", false)] {
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert_eq!(output.stderr, expected.as_bytes());
    }
    project.unchanged_files(source);
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn process_entry_and_native_run_source_errors_keep_the_original_text() {
    let project = Project::new(TYPE_ERROR);
    for args in [
        vec![
            "run",
            "main.ox",
            "--edition=typed-preview",
            "--entry-mode=process",
        ],
        vec![
            "compile",
            "main.ox",
            "--edition=typed-preview",
            "--backend=llvm",
            "--output=program",
            "--entry-mode=process",
        ],
        vec!["run", "--manifest-path=oxid.toml", "--entry-mode=process"],
        vec!["build", "--manifest-path=oxid.toml", "--entry-mode=process"],
        vec!["run", "--manifest-path=oxid.toml", "--engine=native"],
        vec![
            "run",
            "--manifest-path=oxid.toml",
            "--engine=native",
            "--entry-mode=process",
        ],
    ] {
        let output = project.command(&args);
        assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert_eq!(output.stderr, TYPE_HEADER.as_bytes(), "{args:?}");
    }
    project.unchanged_files(TYPE_ERROR);
}
