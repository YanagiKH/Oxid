//! Checked division and remainder through the public typed-preview boundary.
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn bounded_output(command: &mut Command) -> Output {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Drain concurrently so an unexpectedly large diagnostic cannot fill a
    // pipe while the parent is waiting for the child. Bound native loops too.
    let drain = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).unwrap();
            bytes
        })
    };
    let stdout = drain(Box::new(child.stdout.take().unwrap()));
    let stderr = drain(Box::new(child.stderr.take().unwrap()));
    let deadline = Instant::now() + Duration::from_secs(30);
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
                "division test command timed out: {command:?} ({} stdout bytes, {} stderr bytes)",
                stdout.join().unwrap().len(),
                stderr.join().unwrap().len()
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

struct Project(PathBuf);
impl Project {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-public-division-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        Self(root)
    }
    fn typed(&self, operation: &str) -> Output {
        bounded_output(
            Command::new(env!("CARGO_BIN_EXE_oxid"))
                .current_dir(&self.0)
                .env_remove("OXID_PATH")
                .args([operation, "main.ox", "--edition=typed-preview"]),
        )
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn public_division_and_remainder_check_and_run() {
    let project = Project::new("fn main()->i32{return 17 / 10 * 100 + 17 % 10;}");
    let check = project.typed("check");
    assert!(check.status.success(), "{check:?}");
    let run = project.typed("run");
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.stdout, b"107\n");
    assert!(run.stderr.is_empty());
}

impl Project {
    fn json(&self, operation: &str) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command.current_dir(&self.0).env_remove("OXID_PATH").args([
            operation,
            "main.ox",
            "--edition=typed-preview",
            "--message-format=json",
        ]);
        if operation == "compile" {
            command
                .env("OXID_LLVM_BIN", self.0.join("absent-tools"))
                .args(["--backend=llvm", "--output=program"]);
        }
        bounded_output(&mut command)
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn compile(&self) -> Output {
        bounded_output(
            Command::new(env!("CARGO_BIN_EXE_oxid"))
                .current_dir(&self.0)
                .env_remove("OXID_PATH")
                .args([
                    "compile",
                    "main.ox",
                    "--edition=typed-preview",
                    "--backend=llvm",
                    "--output=program",
                ]),
        )
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn native_without_sources(&self) -> Output {
        let compile = self.compile();
        assert!(compile.status.success(), "{compile:?}");
        assert_eq!(&fs::read(self.0.join("program")).unwrap()[..4], b"\x7fELF");
        for entry in fs::read_dir(&self.0).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|extension| extension == "ox") {
                fs::remove_file(path).unwrap();
            }
        }
        bounded_output(
            Command::new(self.0.join("program"))
                .current_dir(&self.0)
                .env_clear(),
        )
    }
}

// An unused fixed array selects the owned consumer without changing the
// arithmetic expression, so the same independent oracle covers both routes.
fn expression_source(expression: &str, owned: bool) -> String {
    format!(
        "// 雪\r\nfn main()->i32{{{}return {expression};}}",
        if owned { "let route=[0];" } else { "" }
    )
}

fn values() -> Vec<(&'static str, i32)> {
    vec![
        ("17 / 10", 1),
        ("-17 / 10", -1),
        ("17 / -10", -1),
        ("-17 / -10", 1),
        ("17 % 10", 7),
        ("-17 % 10", -7),
        ("17 % -10", 7),
        ("-17 % -10", -7),
        ("0 / -1 + 0 % -1", 0),
        ("2147483647 / 1", 2147483647),
        ("-2147483648 / 1", -2147483648),
        ("-2147483648 / 2", -1073741824),
        ("-2147483648 % 2", 0),
        ("-2147483648 % 2147483647", -1),
        ("80 / 4 / 2", 10),
        ("20 / 3 * 2", 12),
        ("20 % 6 * 3", 6),
        ("20 / 3 % 4", 2),
        ("20 % 6 / 2", 1),
        ("2 + 20 / 3 * 2 - 1", 13),
        ("20 / (3 * 2)", 3),
        ("20 % (6 * 3)", 2),
    ]
}

#[test]
fn public_division_sign_boundaries_and_multiplicative_precedence_in_both_routes() {
    for owned in [false, true] {
        for (expression, expected) in values() {
            let source = expression_source(expression, owned);
            let project = Project::new(&source);
            let check = project.typed("check");
            assert!(check.status.success(), "{source}: {check:?}");
            let run = project.typed("run");
            assert!(run.status.success(), "{source}: {run:?}");
            assert_eq!(run.stdout, format!("{expected}\n").as_bytes(), "{source}");
            assert!(run.stderr.is_empty(), "{source}: {run:?}");
            let json = project.json("run");
            assert!(json.status.success(), "{source}: {json:?}");
            assert_eq!(json.stdout, format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":true,\"errors\":0,\"result\":{{\"type\":\"i32\",\"value\":{expected}}}}}\n").as_bytes(), "{source}");
            assert!(json.stderr.is_empty());
        }
    }
}

// The origin string starts at the exact operator expected to fail. Longer
// suffixes distinguish repeated operators independently of the implementation.
fn failures() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("7 / 0", "E0607", "/"),
        ("7 % 0", "E0607", "%"),
        ("-2147483648 / 0", "E0607", "/"),
        ("-2147483648 % 0", "E0607", "%"),
        ("-2147483648 / -1", "E0604", "/"),
        ("-2147483648 % -1", "E0604", "%"),
        ("7 / (4 - 4)", "E0607", "/"),
        ("7 % (4 - 4)", "E0607", "%"),
        ("(2147483647 + 1) / 0", "E0604", "+"),
        ("7 / (2147483647 + 1)", "E0604", "+"),
        ("(1 / 0) % (-2147483648 / -1)", "E0607", "/ 0"),
        ("(-2147483648 % -1) / (1 / 0)", "E0604", "%"),
        ("0 / (1 % 0)", "E0607", "%"),
        ("0 % (1 / 0)", "E0607", "/"),
    ]
}

fn runtime_message(source: &str, path: &str, code: &str, origin: &str) -> String {
    let start = source.rfind(origin).unwrap();
    let before = &source[..start];
    let line = before.bytes().filter(|&byte| byte == b'\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap().chars().count() + 1;
    let message = match code {
        "E0607" => "checked i32 division by zero",
        "E0604" => "checked i32 arithmetic overflow",
        _ => panic!("unexpected test code: {code}"),
    };
    format!("error[{code}] (oir-run): {message}\n  --> {path}:{line}:{column}\n")
}

fn assert_runtime_failure(project: &Project, source: &str, code: &str, origin: &str) {
    let check = project.typed("check");
    assert!(check.status.success(), "{source}: {check:?}");
    let run = project.typed("run");
    assert_eq!(run.status.code(), Some(1), "{source}: {run:?}");
    assert!(run.stdout.is_empty(), "{source}: {run:?}");
    assert_eq!(
        run.stderr,
        runtime_message(source, "main.ox", code, origin).as_bytes(),
        "{source}"
    );
    let json = project.json("run");
    assert_eq!(json.status.code(), Some(1), "{source}: {json:?}");
    assert!(json.stderr.is_empty(), "{source}: {json:?}");
    let text = String::from_utf8(json.stdout).unwrap();
    assert_eq!(text.matches("\"kind\":\"diagnostic\"").count(), 1, "{text}");
    assert!(
        text.contains(&format!("\"code\":\"{code}\",\"stage\":\"oir-run\"")),
        "{text}"
    );
    let start = source.rfind(origin).unwrap();
    assert!(
        text.contains(&format!("\"start\":{start},\"end\":{}", start + 1)),
        "{text}"
    );
    assert!(text.ends_with("{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":false,\"errors\":1,\"result\":null}\n"), "{text}");
}

#[test]
fn public_division_errors_are_runtime_only_and_preserve_first_operator_in_both_routes() {
    for owned in [false, true] {
        for (expression, code, origin) in failures() {
            let source = expression_source(expression, owned);
            assert_runtime_failure(&Project::new(&source), &source, code, origin);
        }
    }
}

fn effect_cases() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("fn next(s:&mut [i32;1])->i32{s[0]=s[0]+1;return s[0];} fn main()->i32{let mut s=[9];let q=next(&mut s)/next(&mut s);let r=next(&mut s)%next(&mut s);return q*10000+r*100+s[0];}", b"1213\n"),
        ("fn index(s:&mut [i32;1])->i32{s[0]=99;return 0;} fn main()->i32{let mut s=[17];s[index(&mut s)]=s[0]/10;return s[0];}", b"1\n"),
        ("fn index(s:&mut [i32;1])->i32{s[0]=99;return 0;} fn main()->i32{let mut s=[17];s[index(&mut s)]=s[0]%10;return s[0];}", b"7\n"),
        ("fn main()->bool{return false && 1/0==0 || true || -2147483648%-1==0;}", b"true\n"),
        ("fn main()->bool{return 10/2==5 && 7%4==3 || 1/0==0;}", b"true\n"),
        ("fn main()->bool{let route=[0];return false || 9%4==1 && 8/2==4;}", b"true\n"),
        ("fn main()->i32{if false{return 1%0;}return 42;}", b"42\n"),
        ("fn main()->i32{let route=[0];if false{return -2147483648/-1;}return 42;}", b"42\n"),
    ]
}

#[test]
fn public_division_call_effects_assignment_rhs_order_and_unexecuted_branches() {
    for (source, expected) in effect_cases() {
        let project = Project::new(source);
        let check = project.typed("check");
        assert!(check.status.success(), "{source}: {check:?}");
        let run = project.typed("run");
        assert!(run.status.success(), "{source}: {run:?}");
        assert_eq!(run.stdout, expected, "{source}");
        assert!(run.stderr.is_empty());
    }
    for (source, code, origin) in ordering_failures() {
        assert_runtime_failure(&Project::new(source), source, code, origin);
    }
}

fn ordering_failures() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("fn left()->i32{return 1/0;} fn right()->i32{return -2147483648%-1;} fn main()->i32{return left()/right();}", "E0607", "/0"),
        ("fn left()->i32{return -2147483648%-1;} fn right()->i32{return 1/0;} fn main()->i32{let route=[0];return left()%right();}", "E0604", "%-1"),
        ("fn main()->i32{let mut a=[7];a[1/0]=7%0;return a[0];}", "E0607", "%"),
        ("fn main()->i32{let mut a=[7];a[2]=7/0;return a[0];}", "E0607", "/"),
        ("fn main()->i32{1%0;return 42;}", "E0607", "%"),
    ]
}

#[test]
fn public_division_operands_are_strict_i32_even_in_dead_code_and_before_native_tools() {
    for expression in ["true / 1", "1 / false", "() % 1", "1 % ()", "true % false"] {
        for owned in [false, true] {
            for body in [expression.to_owned(), format!("false || {expression} == 0")] {
                // Both live and short-circuited expression bodies are checked.
                let source = if body.starts_with("false") {
                    format!(
                        "fn main()->bool{{{}return true || {body};}}",
                        if owned { "let route=[0];" } else { "" }
                    )
                } else {
                    expression_source(&body, owned)
                };
                let project = Project::new(&source);
                for operation in ["check", "run", "compile"] {
                    let output = project.json(operation);
                    assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
                    assert!(output.stderr.is_empty());
                    let text = String::from_utf8(output.stdout).unwrap();
                    assert!(
                        text.contains("\"code\":\"E0300\",\"stage\":\"type\""),
                        "{source}: {text}"
                    );
                    assert!(!text.contains("E0701"), "{source}: {text}");
                    assert!(!project.0.join("program").exists());
                }
            }
        }
    }
}

#[test]
fn public_division_does_not_extend_unary_negation_or_literal_range() {
    for (expression, code) in [
        ("7 / -x", "E0101"),
        ("7 % -(2)", "E0101"),
        ("7 / --1", "E0101"),
        ("7 % +1", "E0101"),
        ("7 / 2147483648", "E0203"),
        ("7 % -2147483649", "E0203"),
    ] {
        let output = Project::new(&expression_source(expression, false)).json("check");
        assert_eq!(output.status.code(), Some(1), "{expression}: {output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            text.contains(&format!("\"code\":\"{code}\"")),
            "{expression}: {text}"
        );
    }
}

#[test]
fn public_division_formatter_preserves_operators_comments_and_idempotence() {
    let source = "fn main()->i32{return -17/* quotient */ /10*100+17%10;}";
    let project = Project::new(source);
    let first = project.typed("fmt");
    assert!(first.status.success(), "{first:?}");
    assert_eq!(
        first.stdout,
        b"fn main() -> i32 { return -17 /* quotient */ / 10 * 100 + 17 % 10; }\n"
    );
    assert!(first.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(project.0.join("main.ox")).unwrap(),
        source
    );
    fs::write(project.0.join("main.ox"), &first.stdout).unwrap();
    let second = project.typed("fmt");
    assert!(second.status.success(), "{second:?}");
    assert_eq!(first.stdout, second.stdout);
    let run = project.typed("run");
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.stdout, b"-93\n");
}

#[cfg(target_os = "linux")]
fn checksum_project() -> Project {
    let project = Project::new(
        "mod digits; fn main()->i32{let values=[17,23,35];return crate::digits::checksum(values);}",
    );
    fs::write(project.0.join("digits.ox"), "pub fn checksum(values:[i32;3])->i32{let mut i=0;let mut total=0;while i<values.len(){let v=values[i];total=total+(v/10)*100+v%10;i=i+1;}return total;}").unwrap();
    project
}

#[cfg(target_os = "linux")]
#[test]
fn public_division_owned_array_module_checksum_is_615() {
    let project = checksum_project();
    let check = project.typed("check");
    assert!(check.status.success(), "{check:?}");
    let run = project.typed("run");
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.stdout, b"615\n");
    assert!(run.stderr.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn public_division_module_runtime_failure_keeps_child_file_and_operator() {
    let child = "// 雪\n pub fn fail()->i32{return 9 % 0;}";
    for owned in [false, true] {
        let project = Project::new(&format!(
            "mod child; fn main()->i32{{{}return crate::child::fail();}}",
            if owned { "let route=[0];" } else { "" }
        ));
        fs::write(project.0.join("child.ox"), child).unwrap();
        assert!(project.typed("check").status.success());
        let run = project.typed("run");
        assert_eq!(run.status.code(), Some(1), "{run:?}");
        assert!(run.stdout.is_empty());
        assert_eq!(
            run.stderr,
            runtime_message(child, "child.ox", "E0607", "%").as_bytes()
        );
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires LLVM/Clang/LLD 19.1.7; run the public checked-division native gate explicitly"]
fn public_division_source_free_native_parity() {
    for owned in [false, true] {
        for (expression, expected) in values() {
            let source = expression_source(expression, owned);
            let native = Project::new(&source).native_without_sources();
            assert!(native.status.success(), "{source}: {native:?}");
            assert_eq!(
                native.stdout,
                format!("{expected}\n").as_bytes(),
                "{source}"
            );
            assert!(native.stderr.is_empty());
        }
        for (expression, code, origin) in failures() {
            let source = expression_source(expression, owned);
            let native = Project::new(&source).native_without_sources();
            assert_eq!(native.status.code(), Some(1), "{source}: {native:?}");
            assert!(native.stdout.is_empty());
            assert_eq!(
                native.stderr,
                runtime_message(&source, "main.ox", code, origin).as_bytes(),
                "{source}"
            );
        }
    }
    for (source, expected) in effect_cases() {
        let native = Project::new(source).native_without_sources();
        assert!(native.status.success(), "{source}: {native:?}");
        assert_eq!(native.stdout, expected, "{source}");
        assert!(native.stderr.is_empty());
    }
    for (source, code, origin) in ordering_failures() {
        let native = Project::new(source).native_without_sources();
        assert_eq!(native.status.code(), Some(1), "{source}: {native:?}");
        assert!(native.stdout.is_empty());
        assert_eq!(
            native.stderr,
            runtime_message(source, "main.ox", code, origin).as_bytes()
        );
    }
    let native = checksum_project().native_without_sources();
    assert!(native.status.success(), "{native:?}");
    assert_eq!(native.stdout, b"615\n");
    assert!(native.stderr.is_empty());
    for owned in [false, true] {
        let child = "// 雪\n pub fn fail()->i32{return 9 % 0;}";
        let project = Project::new(&format!(
            "mod child; fn main()->i32{{{}return crate::child::fail();}}",
            if owned { "let route=[0];" } else { "" }
        ));
        fs::write(project.0.join("child.ox"), child).unwrap();
        let native = project.native_without_sources();
        assert_eq!(native.status.code(), Some(1), "{native:?}");
        assert!(native.stdout.is_empty());
        assert_eq!(
            native.stderr,
            runtime_message(child, "child.ox", "E0607", "%").as_bytes()
        );
    }
}

fn fuel_source(owned: bool) -> String {
    format!(
        "fn main()->i32{{{}while true{{let q=3/2;q%2;}}return 0;}}",
        if owned { "let route=[0];" } else { "" }
    )
}

#[test]
fn public_division_loops_remain_fuel_bounded_in_both_routes() {
    for owned in [false, true] {
        let project = Project::new(&fuel_source(owned));
        let check = project.typed("check");
        assert!(check.status.success(), "{check:?}");
        let output = project.json("run");
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            text.contains("\"code\":\"E0601\",\"stage\":\"oir-run\""),
            "{text}"
        );
        assert!(!text.contains("E0607"), "{text}");
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires LLVM/Clang/LLD 19.1.7; run the public checked-division native gate explicitly"]
fn public_division_source_free_native_loop_fuel_parity() {
    for owned in [false, true] {
        let project = Project::new(&fuel_source(owned));
        let reference = project.typed("run");
        assert_eq!(reference.status.code(), Some(1), "{reference:?}");
        assert!(String::from_utf8_lossy(&reference.stderr).contains("E0601"));
        let native = project.native_without_sources();
        assert_eq!(native.status.code(), reference.status.code());
        assert_eq!(native.stdout, reference.stdout);
        assert_eq!(native.stderr, reference.stderr);
    }
}
