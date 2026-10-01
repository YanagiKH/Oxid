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
            "oxid-i32-{}-{}",
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
#[test]
fn exact_i32_text_json_and_copies_are_not_process_exit_codes() {
    for (literal, value) in [
        ("0", "0"),
        ("-0", "0"),
        ("000000", "0"),
        ("-000000", "0"),
        ("1", "1"),
        ("-1", "-1"),
        ("2147483647", "2147483647"),
        ("2147483646", "2147483646"),
        ("-2147483648", "-2147483648"),
        ("-2147483647", "-2147483647"),
        ("0002147483647", "2147483647"),
        ("- /* 雪 */ 0002147483648", "-2147483648"),
        ("- // é\r\n 1", "-1"),
    ] {
        let source = format!("fn id(x: i32) -> i32 {{ return (x); }} fn main() -> i32 {{ let inferred = ({literal}); let explicit: i32 = inferred; return id(explicit); }}");
        let fixture = Fixture::new(&source);
        let check = fixture.command(&["check", "main.ox", "--edition=typed-preview"]);
        assert_eq!(check.status.code(), Some(0), "{literal}: {check:?}");
        for _ in 0..2 {
            let out = fixture.run(false);
            assert_eq!(out.status.code(), Some(0), "{literal}: {out:?}");
            assert_eq!(out.stdout, format!("{value}\n").as_bytes());
            assert!(out.stderr.is_empty());
            let out = fixture.run(true);
            assert_eq!(out.status.code(), Some(0), "{literal}: {out:?}");
            assert_eq!(
                out.stdout,
                summary(true, 0, &format!("{{\"type\":\"i32\",\"value\":{value}}}")).as_bytes()
            );
            assert!(out.stderr.is_empty());
        }
    }
}
fn reject(source: &str, code: &str, stage: &str, origin: &str) {
    let fixture = Fixture::new(source);
    for operation in ["check", "run"] {
        let out = fixture.command(&[
            operation,
            "main.ox",
            "--edition=typed-preview",
            "--message-format=json",
        ]);
        assert_eq!(out.status.code(), Some(1), "{source}: {out:?}");
        assert!(out.stderr.is_empty());
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(
            text.contains(&format!("\"code\":\"{code}\"")),
            "{source}: {text}"
        );
        assert!(
            text.contains(&format!("\"stage\":\"{stage}\"")),
            "{source}: {text}"
        );
        let search_from = if origin == "-" {
            source.find("return ").unwrap() + 7
        } else {
            0
        };
        let start = search_from + source[search_from..].find(origin).unwrap();
        assert!(
            text.contains(&format!(
                "\"start\":{start},\"end\":{}",
                start + origin.len()
            )),
            "{source}: {text}"
        );
        if operation == "run" {
            assert!(text.ends_with(&summary(false, 1, "null")));
        }
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
}
#[test]
fn range_errors_have_exact_signed_origins_and_never_round() {
    for literal in [
        "2147483648",
        "-2147483649",
        "9007199254740992",
        "9007199254740993",
        "- /* 雪 */ 2147483649",
        "- // é\r\n 2147483649",
    ] {
        reject(
            &format!("// 前\r\nfn main() -> i32 {{ return {literal}; }}"),
            "E0203",
            "resolve",
            literal,
        );
    }
    // All declarations and unchosen arms are checked before execution.
    reject(
        "fn main() -> i32 { if true { return 0; } else { return 2147483648; } }",
        "E0203",
        "resolve",
        "2147483648",
    );
    reject(
        "fn main() -> i32 { return 0; } fn unused() -> i32 { return -2147483649; }",
        "E0203",
        "resolve",
        "-2147483649",
    );
}
#[test]
fn spelling_is_validated_before_range_and_other_operators_remain_unavailable() {
    for literal in [
        "1i32",
        "1_000",
        "0xff",
        "0o7",
        "0b1",
        "1.0",
        "1e9",
        "1e999999",
        "999999999999999999999i32",
        "１",
        "١",
        "1é",
        "1١",
        "- /* 雪 */ 1.0",
    ] {
        reject(
            &format!("fn main() -> i32 {{ return {literal}; }}"),
            "E0101",
            "parse",
            literal,
        );
    }
    for (expr, origin) in [
        ("+1", "+"),
        ("-(1)", "-"),
        ("--1", "-"),
        ("-x", "-"),
        ("-id()", "-"),
        ("1 / 2", "/"),
        ("1 % 2", "%"),
        ("!true", "!"),
        ("true && false", "&"),
        ("1 as i32", "as"),
    ] {
        reject(
            &format!("fn main() -> i32 {{ return {expr}; }}"),
            "E0101",
            "parse",
            origin,
        );
    }
}
#[test]
fn i32_contexts_remain_strict_without_numeric_coercion() {
    for (source, code, stage, origin) in [
        ("fn main() -> bool { return 1; }", "E0300", "type", "1"),
        ("fn main() -> () { return 1; }", "E0300", "type", "1"),
        ("fn main() -> i32 { return true; }", "E0300", "type", "true"),
        (
            "fn main(x: bool) -> i32 { return (); }",
            "E0300",
            "type",
            "()",
        ),
        (
            "fn main() -> i32 { let x: bool = 1; return 0; }",
            "E0300",
            "type",
            "1",
        ),
        (
            "fn main() -> i32 { let x: i32 = false; return 0; }",
            "E0300",
            "type",
            "false",
        ),
        (
            "fn main() -> i32 { if 1 { return 0; } return 1; }",
            "E0300",
            "type",
            "1",
        ),
        (
            "fn id(x: i32) -> i32 { return x; } fn main() -> i32 { return id(true); }",
            "E0300",
            "type",
            "true",
        ),
        (
            "fn id(x: bool) -> bool { return x; } fn main() -> bool { return id(1); }",
            "E0300",
            "type",
            "1",
        ),
        (
            "fn id(x: i32) -> i32 { return x; } fn main() -> i32 { return id(); }",
            "E0301",
            "type",
            "id()",
        ),
        ("fn main() -> u32 { return 1; }", "E0202", "resolve", "u32"),
        ("fn main() -> i64 { return 1; }", "E0202", "resolve", "i64"),
    ] {
        reject(source, code, stage, origin);
    }
}
#[test]
fn long_zeroes_are_decimal_and_token_limit_is_unchanged() {
    for literal in [
        "0".repeat(65_536),
        format!("-{}", "0".repeat(65_536)),
        format!("{}2147483647", "0".repeat(65_526)),
    ] {
        let expected = if literal.ends_with("2147483647") {
            "2147483647\n"
        } else {
            "0\n"
        };
        let out = Fixture::new(&format!("fn main() -> i32 {{ return {literal}; }}")).run(false);
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        assert_eq!(out.stdout, expected.as_bytes());
    }
    for literal in [
        "9".repeat(65_536),
        format!("{}2147483648", "0".repeat(65_526)),
    ] {
        reject(
            &format!("fn main() -> i32 {{ return {literal}; }}"),
            "E0203",
            "resolve",
            &literal,
        );
    }
    let literal = "0".repeat(65_537);
    reject(
        &format!("fn main() -> i32 {{ return {literal}; }}"),
        "E0400",
        "lex",
        &literal,
    );
}
#[test]
fn i32_main_identity_and_boolean_branch_selection_are_source_order_independent() {
    for main_first in [false, true] {
        for (a, b, expected) in [
            (false, false, -2147483648),
            (false, true, -1),
            (true, false, 0),
            (true, true, 2147483647),
        ] {
            let main = format!("fn main() -> i32 {{ return select({a}, {b}); }}");
            let helpers = "fn decoy() -> i32 { return 123; } fn id(x: i32) -> i32 { return x; } fn select(a: bool, b: bool) -> i32 { if a { if b { return id(2147483647); } return 0; } else { if b { let x = -1; return id(x); } return (-2147483648); } }";
            let source = if main_first {
                format!("{main} {helpers}")
            } else {
                format!("{helpers} {main}")
            };
            let out = Fixture::new(&source).run(false);
            assert_eq!(out.status.code(), Some(0), "{out:?}");
            assert_eq!(out.stdout, format!("{expected}\n").as_bytes());
        }
    }
}

#[test]
fn cast_rejection_does_not_reserve_a_previously_valid_identifier() {
    let out = Fixture::new("fn as(x: bool) -> bool { return x; } fn main() -> bool { let i32 = true; return as(i32); }").run(false);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(out.stdout, b"true\n");
}
#[test]
fn signed_eof_and_numeric_diagnostic_caps_remain_bounded() {
    reject("fn main() -> i32 { return -", "E0101", "parse", "-");
    let incomplete = "fn main() -> i32 { return 1";
    let fixture = Fixture::new(incomplete);
    let out = fixture.run(true);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("\"code\":\"E0100\""));
    assert!(text.contains(&format!(
        "\"start\":{},\"end\":{}",
        incomplete.len(),
        incomplete.len()
    )));
    let source = (0..110)
        .map(|i| format!("fn f{i}() -> i32 {{ return 2147483648; }}"))
        .collect::<String>();
    let out = Fixture::new(&source).run(true);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(text.matches("\"code\":\"E0203\"").count(), 100);
    assert!(text.ends_with(&summary(false, 100, "null")));
}
