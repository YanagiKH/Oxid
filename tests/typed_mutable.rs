//! The public typed-run boundary uses isolated, timeout-bounded subprocesses.
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
        let root = std::env::temp_dir().join(format!(
            "oxid-mutable-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        Self(root)
    }
    fn command(&self, args: &[&str]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // Drain both pipes during the bounded wait: diagnostic snippets can
        // exceed a pipe buffer for token-limit tests.
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
                    "typed run timed out ({} stdout bytes, {} stderr bytes)",
                    stdout.join().unwrap().len(),
                    stderr.join().unwrap().len()
                );
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn run(&self, json: bool) -> Output {
        let mut args = vec!["run", "main.ox", "--edition=typed-preview"];
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
fn summary(success: bool, errors: usize, result: &str) -> String {
    format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"run-summary\",\"success\":{success},\"errors\":{errors},\"result\":{result}}}\n")
}

fn assert_result(source: &str, tag: &str, value: &str) {
    let fixture = Fixture::new(source);
    let checked = fixture.command(&["check", "main.ox", "--edition=typed-preview"]);
    assert_eq!(checked.status.code(), Some(0), "{source}: {checked:?}");
    assert!(checked.stderr.is_empty());
    for json in [false, true] {
        let output = fixture.run(json);
        assert_eq!(output.status.code(), Some(0), "{source}: {output:?}");
        assert!(output.stderr.is_empty());
        let result = if tag == "unit" {
            "{\"type\":\"unit\"}".to_owned()
        } else {
            format!("{{\"type\":\"{tag}\",\"value\":{value}}}")
        };
        let expected = if json {
            summary(true, 0, &result)
        } else {
            format!("{value}\n")
        };
        assert_eq!(output.stdout, expected.as_bytes(), "{source}");
    }
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
}

#[test]
fn mutable_values_update_while_immutable_copies_remain_snapshots() {
    assert_result("fn main() -> i32 { let mut x: i32 = 7; let snapshot = x; x = x + 5; return snapshot * 100 + x; }", "i32", "712");
    assert_result("fn main() -> bool { let mut x = true; let snapshot = x; x = false; return snapshot && !x; }", "bool", "true");
    assert_result(
        "fn main() -> () { let mut x: () = (); let snapshot = x; x = (); return snapshot; }",
        "unit",
        "()",
    );
}

fn reject(source: &str, code: &str, stage: &str, marker: Option<&str>) {
    let fixture = Fixture::new(source);
    for operation in ["check", "run", "compile"] {
        let mut args = vec![
            operation,
            "main.ox",
            "--edition=typed-preview",
            "--message-format=json",
        ];
        if operation == "compile" {
            args.extend(["--backend=llvm", "--output", "invalid-output"]);
        }
        let output = fixture.command(&args);
        assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            text.contains(&format!("\"code\":\"{code}\"")),
            "{source}: {text}"
        );
        assert!(
            text.contains(&format!("\"stage\":\"{stage}\"")),
            "{source}: {text}"
        );
        if let Some(marker) = marker {
            let start = source.rfind(marker).unwrap();
            let end = start + marker.len();
            assert!(
                text.contains(&format!("\"start\":{start},\"end\":{end}")),
                "{source}: {text}"
            );
        }
        if operation == "run" {
            assert!(text.ends_with(&summary(false, 1, "null")), "{text}");
        }
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
}

#[test]
fn sequential_assignments_read_the_latest_value_and_preserve_prior_copies() {
    for (source, result) in [
        ("fn main() -> i32 { let mut x = 1; x = x + x; x = x * x; x = x - 1; return x; }", "3"),
        ("fn main() -> i32 { let mut x = 1; let mut y = x; x = 10; y = y + x; x = y + x; return x * 100 + y; }", "2111"),
        ("fn main() -> i32 { let mut x = 3; let first = x; x = 4; let second = x; x = 5; return first * 100 + second * 10 + x; }", "345"),
        ("fn main() -> i32 { let mut x = -2147483648; x = x + 1; x = 2147483647; x = x - 1; return x; }", "2147483646"),
        ("fn main() -> i32 { let /* 雪 */ mut /*🦀*/ x /*é*/: i32 = 1; x /*雪*/ = /*é*/ x + 1; return x; }", "2"),
    ] {
        assert_result(source, "i32", result);
    }
}

#[test]
fn branch_stores_survive_joins_and_closed_scopes_allow_distinct_reused_names() {
    for condition in ["true", "false"] {
        let source = format!("fn main() -> i32 {{ let mut x = 1; if {condition} {{ x = 20; let mut y = x; y = y + 2; x = y; }} else {{ x = 30; let mut y = x; y = y + 3; x = y; }} let mut y = 4; y = y + x; return y; }}");
        assert_result(
            &source,
            "i32",
            if condition == "true" { "26" } else { "37" },
        );
        let source =
            format!("fn main() -> i32 {{ let mut x = 5; if {condition} {{ x = 9; }} return x; }}");
        assert_result(&source, "i32", if condition == "true" { "9" } else { "5" });
        let source = format!("fn main() -> i32 {{ let mut x = 2; if {condition} {{ x = 3; return x; }} else {{ x = 4; }} x = x * 10; return x; }}");
        assert_result(&source, "i32", if condition == "true" { "3" } else { "40" });
    }
    assert_result("fn main() -> i32 { let mut x = 1; if true { x = 2; if false { x = 3; } else { x = x + 4; } x = x + 8; } else { x = 16; } return x; }", "i32", "14");
}

#[test]
fn mutable_conditions_and_short_circuit_rhs_preserve_selected_values() {
    for (source, tag, value) in [
        ("fn main() -> bool { let mut x = false; x = true || 2147483647 + 1 == 0; x = x && !false; return x; }", "bool", "true"),
        ("fn main() -> bool { let mut x = true; x = false && -2147483648 - 1 == 0; return x; }", "bool", "false"),
        ("fn main() -> i32 { let mut take = false; let mut x = 1; if take { x = 2; } take = true; if take && (x == 1) { x = 4; } return x; }", "i32", "4"),
        ("fn main() -> i32 { let mut x = 7; if false { x = 2147483647 + 1; } else { x = x + 1; } return x; }", "i32", "8"),
        ("fn main() -> i32 { let mut x = 7; if true { return x; } else { x = 2147483647 + 1; return x; } }", "i32", "7"),
    ] {
        assert_result(source, tag, value);
    }
}

#[test]
fn function_parameters_are_values_and_mutable_activations_are_independent() {
    assert_result("fn bump(p: i32) -> i32 { let mut x = p; x = x + 1; return x; } fn main() -> i32 { let mut x = 10; let old = bump(x); x = bump(x); return old * 100 + bump(x); }", "i32", "1112");
    assert_result("fn sum(n: i32) -> i32 { let mut x = n; if n == 0 { return x; } x = x + sum(n - 1); return x; } fn main() -> i32 { let mut x = 100; let r = sum(4); return x + r; }", "i32", "110");
    assert_result("fn id(p: ()) -> () { let mut x = p; x = (); return x; } fn main() -> () { let mut x = (); x = id(x); return x; }", "unit", "()");
}

#[test]
fn failed_assignment_rhs_preserves_first_error_origin_and_no_partial_result() {
    let helpers = "// 雪\r\nfn first() -> i32 { return 2147483647 + 1; }\r\nfn later() -> i32 { return -2147483648 - 1; }\r\nfn pair(a: i32, b: i32) -> i32 { return b; }\r\n";
    for (body, marker) in [
        ("x = first(); x = later();", "+ 1"),
        ("x = first() + later();", "+ 1"),
        ("x = later() + first();", "- 1"),
        ("x = pair(first(), later());", "+ 1"),
        ("x = pair(later(), first());", "- 1"),
        ("if true { x = first(); } else { x = later(); }", "+ 1"),
        ("if false { x = first(); } else { x = later(); }", "- 1"),
    ] {
        let source = format!("{helpers}fn main() -> i32 {{ let mut x = 8; {body} return x; }}");
        let fixture = Fixture::new(&source);
        let checked = fixture.command(&["check", "main.ox", "--edition=typed-preview"]);
        assert_eq!(checked.status.code(), Some(0), "{checked:?}");
        let output = fixture.run(true);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        let start = source.find(marker).unwrap();
        assert!(text.contains("\"code\":\"E0604\""), "{text}");
        assert!(
            text.contains(&format!("\"start\":{start},\"end\":{}", start + 1)),
            "{text}"
        );
        assert!(text.ends_with(&summary(false, 1, "null")), "{text}");
        let output = fixture.run(false);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let line = source[..start]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1;
        let column = source[..start].rsplit('\n').next().unwrap().chars().count() + 1;
        assert_eq!(output.stderr, format!("error[E0604] (oir-run): checked i32 arithmetic overflow\n  --> main.ox:{line}:{column}\n").as_bytes());
    }
}

#[test]
fn immutable_bindings_and_parameters_reject_at_target_with_declaration_label() {
    for source in [
        "// 雪\r\nfn main() -> () { let x = 1; x = 2; return; }",
        "fn f(x: bool) -> () { x = false; return; }",
        "fn main() -> () { let x = (); if false { x = (); } return; }",
        "fn main() -> () { return; } fn unused(x: i32) -> () { x = 1; return; }",
    ] {
        reject(source, "E0304", "type", None);
        let output = Fixture::new(source).run(true);
        let text = String::from_utf8(output.stdout).unwrap();
        let target = source.rfind("x =").unwrap();
        let declaration = source.find("x").unwrap();
        assert!(
            text.contains(&format!("\"start\":{target},\"end\":{}", target + 1)),
            "{text}"
        );
        assert!(
            text.contains(&format!(
                "\"start\":{declaration},\"end\":{}",
                declaration + 1
            )),
            "{text}"
        );
        assert!(!text.contains("\"secondary\":[]"), "{text}");
    }
}

#[test]
fn fixed_scalar_types_are_checked_on_every_assignment_and_initializer() {
    for (initial, other) in [
        ("1", "true"),
        ("true", "1"),
        ("()", "1"),
        ("1", "()"),
        ("false", "()"),
        ("()", "false"),
    ] {
        for body in [
            format!("x = {other};"),
            format!("if false {{ x = {other}; }}"),
        ] {
            reject(
                &format!("fn main() -> () {{ let mut x = {initial}; {body} return; }}"),
                "E0300",
                "type",
                Some(other),
            );
        }
    }
    reject(
        "fn main() -> () { let mut x: bool = 1; return; }",
        "E0300",
        "type",
        Some("1"),
    );
    reject(
        "fn main() -> () { let mut x: i64 = 1; return; }",
        "E0202",
        "resolve",
        Some("i64"),
    );
}

#[test]
fn lexical_target_resolution_and_no_active_shadowing_stay_strict() {
    for source in [
        "fn main() -> () { missing = 1; return; }",
        "fn main() -> () { let mut x = x; return; }",
        "fn main() -> () { x = 1; let mut x = 0; return; }",
        "fn main() -> () { if true { let mut x = 1; } x = 2; return; }",
        "fn main() -> () { if true { let mut x = 1; } else { x = 2; } return; }",
        "fn f() -> () { return; } fn main() -> () { f = (); return; }",
        "fn main() -> () { let mut x = 1; if false { x = missing; } return; }",
    ] {
        reject(source, "E0200", "resolve", None);
    }
    for source in [
        "fn main() -> () { let mut x = 1; let x = 2; return; }",
        "fn main() -> () { let x = 1; let mut x = 2; return; }",
        "fn main() -> () { let mut x = 1; if true { let mut x = 2; } return; }",
        "fn f(x: i32) -> () { let mut x = 2; return; }",
        "fn f() -> () { return; } fn main() -> () { let mut f = 1; return; }",
    ] {
        reject(source, "E0201", "resolve", None);
    }
}

#[test]
fn assignments_remain_statements_and_unsupported_mutation_forms_fail() {
    for source in [
        "fn main() -> () { let mut x; return; }",
        "fn main() -> () { let mut x: i32; return; }",
        "fn main() -> () { let mut x = ; return; }",
        "fn main() -> () { let mut = 1; return; }",
        "fn main() -> () { let mut mut x = 1; return; }",
        "fn main() -> () { let mut x = 1; x = ; return; }",
        "fn main() -> i32 { let mut x = 1; return x = 2; }",
        "fn main() -> () { let mut x = 1; let y = (x = 2); return; }",
        "fn main() -> () { let mut x = 1; let mut y = 2; x = y = 3; return; }",
        "fn main() -> () { let mut x = true; if x = false { return; } return; }",
        "fn f(x: i32) -> () { return; } fn main() -> () { let mut x = 1; f(x = 2); return; }",
        "fn main() -> () { let mut x = 1; (x) = 2; return; }",
        "fn f() -> i32 { return 1; } fn main() -> () { f() = 2; return; }",
        "fn main() -> () { 1 = 2; return; }",
        "fn f(mut x: i32) -> () { return; }",
    ] {
        reject(source, "E0100", "parse", None);
    }
    for source in [
        "fn main() -> () { let mut x = 1; x += 1; return; }",
        "fn main() -> () { let mut x = 1; x -= 1; return; }",
        "fn main() -> () { let mut x = 1; x *= 1; return; }",
        "fn main() -> () { let mut x = 1; loop { x = 2; } return; }",
    ] {
        let out = Fixture::new(source).run(true);
        assert_eq!(out.status.code(), Some(1), "{source}: {out:?}");
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains("\"stage\":\"parse\""), "{text}");
        assert!(text.ends_with(&summary(false, 1, "null")), "{text}");
    }
}

#[test]
fn mutation_rhs_height_and_large_ordered_store_sequences_remain_bounded() {
    for count in [62, 63, 64] {
        let source = format!(
            "fn main() -> bool {{ let mut x = true; x = {}x; return x; }}",
            "!".repeat(count)
        );
        let output = Fixture::new(&source).run(true);
        assert_eq!(
            output.status.code(),
            Some(if count < 64 { 0 } else { 1 }),
            "{output:?}"
        );
        if count == 64 {
            assert!(String::from_utf8(output.stdout)
                .unwrap()
                .contains("\"code\":\"E0400\""));
        }
    }
    let source = format!(
        "fn main() -> i32 {{ let mut x = 0; {} return x; }}",
        "x = x + 1;".repeat(4000)
    );
    assert_result(&source, "i32", "4000");
}
