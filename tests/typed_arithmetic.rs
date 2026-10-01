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
            "oxid-arithmetic-{}-{}",
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
fn arithmetic_precedence_associativity_and_exact_i32_results() {
    for (expr, expected) in [
        ("1 + 2 * 3", 7),
        ("(1 + 2) * 3", 9),
        ("20 - 4 - 3", 13),
        ("20 - (4 - 3)", 19),
        ("2 * 3 * 4 - 5 + 6", 25),
        ("1--2", 3),
        ("-2147483648 + 2147483647", -1),
        ("2147483647 - 2147483647", 0),
        ("-2147483648 * 1", -2147483648),
        ("-2147483648 * 0", 0),
        ("0 - 2147483647 - 1", -2147483648),
        ("46340 * 46340", 2147395600),
    ] {
        let fixture = Fixture::new(&format!("fn id(x: i32) -> i32 {{ return x; }} fn main() -> i32 {{ let x: i32 = {expr}; return id(x); }}"));
        for _ in 0..2 {
            let out = fixture.run(false);
            assert_eq!(out.status.code(), Some(0), "{expr}: {out:?}");
            assert_eq!(out.stdout, format!("{expected}\n").as_bytes());
            assert!(out.stderr.is_empty());
            let out = fixture.run(true);
            assert_eq!(out.status.code(), Some(0), "{expr}: {out:?}");
            assert_eq!(
                out.stdout,
                summary(
                    true,
                    0,
                    &format!("{{\"type\":\"i32\",\"value\":{expected}}}")
                )
                .as_bytes()
            );
            assert!(out.stderr.is_empty());
        }
    }
}
#[test]
fn overflow_is_runtime_only_with_exact_operator_origin() {
    for (expr, operator) in [
        ("2147483647 + 1", "+"),
        ("-2147483648 - 1", "- 1"),
        ("2147483647 - -1", "- -1"),
        ("-2147483648 + -1", "+"),
        ("46341 * 46341", "*"),
        ("-2147483648 * -1", "*"),
        ("2147483647 + 1 - 1", "+"),
        ("0 * (2147483647 + 1)", "+"),
    ] {
        let source = format!("// 雪\r\nfn main() -> i32 {{ return {expr}; }}");
        let fixture = Fixture::new(&source);
        let check = fixture.command(&[
            "check",
            "main.ox",
            "--edition=typed-preview",
            "--message-format=json",
        ]);
        assert_eq!(check.status.code(), Some(0), "{check:?}");
        let out = fixture.run(true);
        assert_eq!(out.status.code(), Some(1), "{expr}: {out:?}");
        assert!(out.stderr.is_empty());
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains("\"code\":\"E0604\""), "{text}");
        assert!(text.contains("\"stage\":\"oir-run\""), "{text}");
        let start = source.find("return ").unwrap() + 7 + expr.find(operator).unwrap();
        assert!(
            text.contains(&format!("\"start\":{start},\"end\":{}", start + 1)),
            "{text}"
        );
        assert!(text.ends_with(&summary(false, 1, "null")));
        let out = fixture.run(false);
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stdout.is_empty());
        assert!(String::from_utf8(out.stderr).unwrap().contains("E0604"));
    }
}
#[test]
fn operand_calls_run_left_to_right_and_unchosen_arms_do_not_overflow() {
    let helpers = "fn left() -> i32 { return 2147483647 + 1; } fn right() -> i32 { return -2147483648 - 1; } fn recurse() -> i32 { return recurse(); }";
    for expr in [
        "left() + right()",
        "left() * recurse()",
        "left() - (right() * recurse())",
    ] {
        let source = format!("{helpers} fn main() -> i32 {{ return {expr}; }}");
        let out = Fixture::new(&source).run(true);
        assert_eq!(out.status.code(), Some(1));
        let text = String::from_utf8(out.stdout).unwrap();
        let start = source.find("+ 1").unwrap();
        assert!(text.contains("\"code\":\"E0604\""), "{text}");
        assert!(
            text.contains(&format!("\"start\":{start},\"end\":{}", start + 1)),
            "{text}"
        );
    }
    let out = Fixture::new(&format!(
        "{helpers} fn main() -> i32 {{ return recurse() + left(); }}"
    ))
    .run(true);
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("\"code\":\"E0602\""));
    for source in [
        "fn main() -> i32 { if true { return 6 * 7; } else { return 2147483647 + 1; } }",
        "fn main() -> i32 { if false { return -2147483648 * -1; } return 40 + 2; }",
        "fn main() -> bool { let ignored = 40 + 2; return true; }",
        "fn main() -> () { 40 + 2; return; }",
    ] {
        let out = Fixture::new(source).run(false);
        assert_eq!(out.status.code(), Some(0), "{out:?}");
    }
    let out = Fixture::new("fn main() -> () { 2147483647 + 1; return; }").run(true);
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("\"code\":\"E0604\""));
}
#[test]
fn both_operands_are_strict_i32_including_statically_checked_dead_code() {
    for expr in [
        "true + 1",
        "1 + false",
        "() - 1",
        "1 - ()",
        "true * false",
        "1 * true",
    ] {
        for source in [
            format!("fn main() -> i32 {{ return {expr}; }}"),
            format!("fn main() -> i32 {{ if true {{ return 1; }} else {{ return {expr}; }} }}"),
        ] {
            for operation in ["check", "run"] {
                let out = Fixture::new(&source).command(&[
                    operation,
                    "main.ox",
                    "--edition=typed-preview",
                    "--message-format=json",
                ]);
                assert_eq!(out.status.code(), Some(1));
                let text = String::from_utf8(out.stdout).unwrap();
                assert!(text.contains("\"code\":\"E0300\""), "{text}");
                assert!(text.contains("\"stage\":\"type\""));
            }
        }
    }
}
#[test]
fn arithmetic_expression_depth_is_bounded_including_flat_left_chains() {
    for (terms, success) in [(64, true), (65, false), (20_000, false)] {
        let expr = vec!["0"; terms].join("+");
        let out = Fixture::new(&format!("fn main() -> i32 {{ return {expr}; }}")).run(true);
        assert_eq!(
            out.status.code(),
            Some(if success { 0 } else { 1 }),
            "{out:?}"
        );
        if !success {
            assert!(String::from_utf8(out.stdout)
                .unwrap()
                .contains("\"code\":\"E0400\""));
        }
    }
}

#[test]
fn arithmetic_grammar_and_mixed_depth_fail_cleanly_without_changing_literal_rules() {
    for (expr, expected) in [
        ("1 +", "E0100"),
        ("1 *", "E0100"),
        ("1 -", "E0100"),
        ("1 ** 2", "E0100"),
        ("1 + +2", "E0101"),
        ("1 - -x", "E0101"),
        ("1 + -(2)", "E0101"),
        ("1 + 2147483648", "E0203"),
        ("1 * -2147483649", "E0203"),
    ] {
        let source = format!("fn main() -> i32 {{ return {expr}; }}");
        let out = Fixture::new(&source).run(true);
        assert_eq!(out.status.code(), Some(1), "{expr}: {out:?}");
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains(&format!("\"code\":\"{expected}\"")), "{text}");
        assert!(text.ends_with(&summary(false, 1, "null")));
    }
    for (terms, success) in [(62, true), (63, true), (64, false)] {
        let chain = vec!["0"; terms].join("+");
        for expr in [
            format!("({chain})"),
            format!("id({chain})"),
            format!("1*({chain})"),
        ] {
            // The last form adds both a group and multiply to tree height.
            let expected = success && (!expr.starts_with("1*") || terms <= 62);
            let out = Fixture::new(&format!(
                "fn id(x: i32) -> i32 {{ return x; }} fn main() -> i32 {{ return {expr}; }}"
            ))
            .run(true);
            assert_eq!(
                out.status.code(),
                Some(if expected { 0 } else { 1 }),
                "{expr}: {out:?}"
            );
            if !expected {
                assert!(String::from_utf8(out.stdout)
                    .unwrap()
                    .contains("\"code\":\"E0400\""));
            }
        }
    }
    let source = "fn compute(flag: bool, x: i32) -> i32 { if flag { let inner = compute(false, x * 2); return x + inner; } return x - 1; } fn main() -> i32 { return compute(true, 10); }";
    let out = Fixture::new(source).run(false);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(out.stdout, b"29\n");
}
