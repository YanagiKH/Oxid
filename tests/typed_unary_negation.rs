//! Public checked unary-negation regressions, separate from frozen qualifications.
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-unary-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("main.ox"), source).unwrap();
        Self(path)
    }
    fn command(&self, args: &[&str]) -> Output {
        bounded_output(
            Command::new(env!("CARGO_BIN_EXE_oxid"))
                .args(args)
                .current_dir(&self.0)
                .env("OXID_CACHE_DIR", self.0.join("cache"))
                .env_remove("OXID_PATH"),
        )
    }
    fn typed(&self, operation: &str, json: bool) -> Output {
        let mut args = vec![operation, "main.ox", "--edition=typed-preview"];
        if json {
            args.push("--message-format=json");
        }
        self.command(&args)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn bounded_output(command: &mut Command) -> Output {
    let mut child = command
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
                "unary-negation command timed out: {command:?} ({} stdout bytes, {} stderr bytes)",
                stdout.join().unwrap().len(),
                stderr.join().unwrap().len()
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn run_summary(ty: &str, value: &str) -> String {
    format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":true,\"errors\":0,\"result\":{{\"type\":\"{ty}\",\"value\":{value}}}}}\n")
}
fn success(source: &str, ty: &str, value: &str) {
    let fixture = Fixture::new(source);
    let check = fixture.typed("check", true);
    assert_eq!(check.status.code(), Some(0), "{source}: {check:?}");
    assert!(check.stderr.is_empty());
    for json in [false, true] {
        let output = fixture.typed("run", json);
        assert_eq!(output.status.code(), Some(0), "{source}: {output:?}");
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout,
            if json {
                run_summary(ty, value)
            } else {
                format!("{value}\n")
            }
            .as_bytes()
        );
    }
    assert!(!fixture.0.join("cache").exists());
    assert!(!fixture.0.join("main.oxb").exists());
}
fn diagnostic(output: Output, source: &str, code: &str, stage: &str, start: usize, width: usize) {
    assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [
        format!("\"code\":\"{code}\""),
        format!("\"stage\":\"{stage}\""),
        format!("\"start\":{start},\"end\":{}", start + width),
        format!(
            "\"line\":{}",
            source[..start].bytes().filter(|b| *b == b'\n').count() + 1
        ),
        format!(
            "\"column\":{}",
            source[..start].rsplit('\n').next().unwrap().chars().count() + 1
        ),
    ] {
        assert!(
            text.contains(&expected),
            "{source}: missing {expected}: {text}"
        );
    }
    assert!(!text.contains("E0500"), "{text}");
}

#[test]
fn public_prefix_precedence_associativity_trivia_and_legacy_literals() {
    for prefix in ["", "struct Marker {} "] {
        for (expression, expected) in [
            ("-2147483648", i32::MIN),
            ("- /* 雪 */ 2147483648", i32::MIN),
            ("--1", 1),
            ("---1", -1),
            ("-(1)", -1),
            ("-(-1)", 1),
            ("-x * 3 + 10", 4),
            ("20 / -x", -10),
            ("7 % -x", 1),
            ("1--x", 3),
            ("--x", 2),
            ("---x", -2),
            ("- /* 🦀 */ (x + 1)", -3),
            ("- // 雪\r\n id(x)", -2),
        ] {
            success(&format!("{prefix}fn id(x: i32) -> i32 {{ return x; }} fn main() -> i32 {{ let x = 2; return {expression}; }}"), "i32", &expected.to_string());
        }
        for (expression, expected) in [
            ("-x < 0 && !false", true),
            ("-x * 2 == -4", true),
            ("!(-x < 0)", false),
            ("--x == x", true),
        ] {
            success(
                &format!("{prefix}fn main() -> bool {{ let x = 2; return {expression}; }}"),
                "bool",
                &expected.to_string(),
            );
        }
    }
}

#[test]
fn public_overflow_is_runtime_only_and_reports_the_executed_minus_byte() {
    for prefix in ["", "struct Marker {} "] {
        for (expression, offset) in [
            ("--2147483648", 0),
            ("-(-2147483648)", 0),
            ("--(-2147483648)", 1),
            ("- /* 雪 */ (-2147483648)", 0),
        ] {
            let start_prefix = format!("// 🦀\r\n{prefix}fn main() -> i32 {{ return ");
            let source = format!("{start_prefix}{expression}; }}");
            let fixture = Fixture::new(&source);
            let check = fixture.typed("check", true);
            assert_eq!(check.status.code(), Some(0), "{check:?}");
            let start = start_prefix.len() + offset;
            diagnostic(
                fixture.typed("run", true),
                &source,
                "E0604",
                "oir-run",
                start,
                1,
            );
            let human = fixture.typed("run", false);
            assert_eq!(human.status.code(), Some(1));
            assert!(human.stdout.is_empty());
            let line = source[..start].bytes().filter(|b| *b == b'\n').count() + 1;
            let column = source[..start].rsplit('\n').next().unwrap().chars().count() + 1;
            assert_eq!(human.stderr, format!("error[E0604] (oir-run): checked i32 arithmetic overflow\n  --> main.ox:{line}:{column}\n").as_bytes());
        }
    }
}

#[test]
fn positive_out_of_range_literal_is_not_reinterpreted_through_parentheses() {
    for prefix in ["", "struct Marker {} "] {
        for (expression, literal) in [
            ("-(2147483648)", "2147483648"),
            ("-2147483649", "-2147483649"),
            ("--2147483649", "-2147483649"),
        ] {
            let source = format!("{prefix}fn main() -> i32 {{ return {expression}; }}");
            let start = source.find(literal).unwrap();
            let fixture = Fixture::new(&source);
            for operation in ["check", "run"] {
                diagnostic(
                    fixture.typed(operation, true),
                    &source,
                    "E0203",
                    "resolve",
                    start,
                    literal.len(),
                );
            }
        }
    }
}

#[test]
fn every_non_i32_operand_rejects_at_its_full_span_even_when_unreachable() {
    for (declarations, operand) in [
        ("", "true"),
        ("", "()"),
        ("", "(true)"),
        ("", "!false"),
        ("struct C {} ", "C {}"),
        ("struct C {} fn make() -> C { return C {}; } ", "make()"),
        ("struct Marker {} ", "false"),
        ("struct Marker {} ", "()"),
    ] {
        for (before, after) in [
            ("fn main() -> i32 { return -", "; }"),
            (
                "fn main() -> i32 { return 0; } fn unused() -> i32 { return -",
                "; }",
            ),
            ("fn main() -> bool { return true || -", " == 0; }"),
        ] {
            let prefix = format!("// 雪\r\n{declarations}{before}");
            let source = format!("{prefix}{operand}{after}");
            let fixture = Fixture::new(&source);
            for operation in ["check", "run"] {
                diagnostic(
                    fixture.typed(operation, true),
                    &source,
                    "E0300",
                    "type",
                    prefix.len(),
                    operand.len(),
                );
            }
        }
    }
    let source = "fn main() -> bool { return !-1; }";
    diagnostic(
        Fixture::new(source).typed("check", true),
        source,
        "E0300",
        "type",
        source.find("-1").unwrap(),
        2,
    );
}

#[test]
fn operands_execute_once_and_first_errors_precede_outer_negation() {
    // A duplicate evaluation changes both the scalar result and observable record state.
    success("struct Counter { n: i32 } fn next(c: &mut Counter) -> i32 { c.n = c.n + 1; return c.n; } fn main() -> i32 { let mut c = Counter { n: 0 }; let value = -next(&mut c); return value * 10 + c.n; }", "i32", "-9");
    for prefix in ["", "struct Marker {} "] {
        for (expression, marker, code) in [
            ("-(1 / 0)", "/", "E0607"),
            ("-(2147483647 + 1)", "+", "E0604"),
            ("-(-2147483648) + 1 / 0", "-(-", "E0604"),
            ("1 / 0 + -(-2147483648)", "/", "E0607"),
        ] {
            let begin = format!("{prefix}fn main() -> i32 {{ return ");
            let source = format!("{begin}{expression}; }}");
            diagnostic(
                Fixture::new(&source).typed("run", true),
                &source,
                code,
                "oir-run",
                begin.len() + expression.find(marker).unwrap(),
                1,
            );
        }
        for (expression, expected) in [
            ("false && -(-2147483648) == 0", "false"),
            ("true || -(-2147483648) == 0", "true"),
        ] {
            success(
                &format!("{prefix}fn main() -> bool {{ return {expression}; }}"),
                "bool",
                expected,
            );
        }
        success(&format!("{prefix}fn main() -> i32 {{ if true {{ return 7; }} else {{ return -(-2147483648); }} }}"), "i32", "7");
    }
}

#[test]
fn mixed_prefix_group_call_and_product_nesting_obey_64_65_boundary() {
    for count in [62, 63, 64] {
        for (expression, height) in [
            (format!("{}x", "-".repeat(count)), count + 1),
            // The final minus still belongs to a legacy signed decimal node.
            (format!("{}1", "-".repeat(count + 1)), count + 1),
            (format!("({}x)", "-".repeat(count)), count + 2),
            (format!("id({}x)", "-".repeat(count)), count + 2),
            (format!("{}x * 1", "-".repeat(count)), count + 2),
            (format!("!({}x < 0)", "-".repeat(count)), count + 4),
        ] {
            let result = if expression.starts_with('!') {
                "bool"
            } else {
                "i32"
            };
            let source = format!("fn id(x: i32) -> i32 {{ return x; }} fn main() -> {result} {{ let x = 1; return {expression}; }}");
            let output = Fixture::new(&source).typed("run", true);
            assert_eq!(
                output.status.code(),
                Some(if height <= 64 { 0 } else { 1 }),
                "height={height}: {source}: {output:?}"
            );
            if height > 64 {
                assert!(String::from_utf8(output.stdout)
                    .unwrap()
                    .contains("\"code\":\"E0400\""));
            }
        }
    }
    for expression in [
        format!("{}x", "-".repeat(5000)),
        format!("{}true", "!-".repeat(5000)),
    ] {
        let output = Fixture::new(&format!(
            "fn main() -> i32 {{ let x = 0; return {expression}; }}"
        ))
        .typed("run", true);
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains("\"code\":\"E0400\""));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linked_modules_and_formatter_keep_unary_results_and_child_error_origins() {
    for owned in [false, true] {
        let child = if owned {
            "struct Cell{value:i32} pub fn value()->i32{let c=Cell{value:21};return - c.value;}"
        } else {
            "pub fn value()->i32{return - (21);}"
        };
        let fixture =
            Fixture::new("mod child;use crate::child::value;fn main()->i32{return - value() * 2;}");
        fs::write(fixture.0.join("child.ox"), child).unwrap();
        assert_eq!(
            fixture.typed("run", true).stdout,
            run_summary("i32", "42").as_bytes()
        );
        for file in ["main.ox", "child.ox"] {
            let before = fs::read(fixture.0.join(file)).unwrap();
            let formatted = fixture.command(&["fmt", file, "--edition=typed-preview"]);
            assert_eq!(formatted.status.code(), Some(0), "{formatted:?}");
            assert!(formatted.stderr.is_empty());
            assert_eq!(fs::read(fixture.0.join(file)).unwrap(), before);
            fs::write(fixture.0.join(file), &formatted.stdout).unwrap();
            let check = fixture.command(&["fmt", file, "--edition=typed-preview", "--check"]);
            assert_eq!(check.status.code(), Some(0), "{check:?}");
        }
        let run = fixture.typed("run", true);
        assert_eq!(run.status.code(), Some(0), "{run:?}");
        assert_eq!(run.stdout, run_summary("i32", "42").as_bytes());
        let child = format!(
            "// 雪\r\n{}pub fn value()->i32{{return -(-2147483648);}}",
            if owned { "struct Marker{} " } else { "" }
        );
        fs::write(fixture.0.join("child.ox"), &child).unwrap();
        assert_eq!(fixture.typed("check", true).status.code(), Some(0));
        let output = fixture.typed("run", true);
        assert!(String::from_utf8_lossy(&output.stdout).contains("\"path\":\"child.ox\""));
        diagnostic(
            output,
            &child,
            "E0604",
            "oir-run",
            child.find("-(-").unwrap(),
            1,
        );
    }
}

#[test]
fn unresolved_negated_names_and_calls_keep_eager_resolution_order() {
    for prefix in ["", "struct Marker {} "] {
        for (expression, missing) in [
            ("-x", "x"),
            ("-id()", "id"),
            ("-left() + -right()", "left"),
            ("true || -missing() == 0", "missing"),
        ] {
            let result = if expression.starts_with("true") {
                "bool"
            } else {
                "i32"
            };
            let before = format!("{prefix}fn main() -> {result} {{ return ");
            let source = format!("{before}{expression}; }}");
            let fixture = Fixture::new(&source);
            for operation in ["check", "run"] {
                diagnostic(
                    fixture.typed(operation, true),
                    &source,
                    "E0200",
                    "resolve",
                    before.len() + expression.find(missing).unwrap(),
                    missing.len(),
                );
            }
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn retain_output(path: &std::path::Path, name: &str, output: &Output) {
    fs::write(path.join(format!("{name}.stdout")), &output.stdout).unwrap();
    fs::write(path.join(format!("{name}.stderr")), &output.stderr).unwrap();
    fs::write(
        path.join(format!("{name}.status")),
        output.status.to_string(),
    )
    .unwrap();
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn native_parity_without_sources(source: &str) {
    let fixture = Fixture::new(source);
    let reference = fixture.typed("run", false);
    let evidence = std::env::var_os("OXID_UNARY_NATIVE_EVIDENCE").map(|root| {
        let path = PathBuf::from(root).join(fixture.0.file_name().unwrap());
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("source.ox"), source).unwrap();
        retain_output(&path, "reference", &reference);
        path
    });
    let compile = fixture.command(&[
        "compile",
        "main.ox",
        "--edition=typed-preview",
        "--backend=llvm",
        "--output=program",
    ]);
    if let Some(path) = &evidence {
        retain_output(path, "compile", &compile);
    }
    assert_eq!(compile.status.code(), Some(0), "{source}: {compile:?}");
    if let Some(path) = &evidence {
        fs::copy(fixture.0.join("program"), path.join("program.elf")).unwrap();
    }
    assert_eq!(
        &fs::read(fixture.0.join("program")).unwrap()[..4],
        b"\x7fELF"
    );
    fs::remove_file(fixture.0.join("main.ox")).unwrap();
    let native = bounded_output(
        Command::new(fixture.0.join("program"))
            .current_dir(&fixture.0)
            .env_clear(),
    );
    if let Some(path) = &evidence {
        retain_output(path, "native", &native);
        fs::write(
            path.join("execution-context.txt"),
            "Source file removed before native execution; native environment cleared.\n",
        )
        .unwrap();
    }
    assert_eq!(
        native.status.code(),
        reference.status.code(),
        "{source}: {native:?}"
    );
    assert_eq!(native.stdout, reference.stdout, "{source}");
    assert_eq!(native.stderr, reference.stderr, "{source}");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires LLVM/Clang/LLD 19.1.7; run the public unary source native gate explicitly"]
fn public_unary_source_free_native_scalar_owned_loops_calls_and_joins() {
    for prefix in ["", "struct Marker {} "] {
        for body in [
            "fn main() -> i32 { return --1; }",
            "fn main() -> i32 { return - /* 雪 */ 2147483648; }",
            "fn main() -> i32 { let mut x = 2; x = -x; return -x * 3; }",
            "fn value() -> i32 { return 7; } fn main() -> i32 { return -value(); }",
            "fn main() -> i32 { let mut x = 1; let mut i = 0; while i < 5 { x = -x; i = i + 1; } return x; }",
            "fn main() -> bool { let x = 2; return false || -x < 0; }",
            "fn main() -> bool { let x = 2; return (-x < 0 && -x == -2) || false; }",
            "fn main() -> bool { return true || -(-2147483648) == 0; }",
            "fn main() -> i32 { return -(-2147483648); }",
            "fn main() -> i32 { return --(-2147483648); }",
            "fn main() -> i32 { return -(1 / 0); }",
        ] {
            native_parity_without_sources(&format!("// 🦀\r\n{prefix}{body}"));
        }
    }
    native_parity_without_sources("struct Counter { n: i32 } fn next(c: &mut Counter) -> i32 { c.n = c.n + 1; return c.n; } fn main() -> i32 { let mut c = Counter { n: 0 }; let value = -next(&mut c); return value * 10 + c.n; }");
}
