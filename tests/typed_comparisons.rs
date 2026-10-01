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
            "oxid-comparison-{}-{}",
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

fn assert_bool(expression: &str, expected: bool) {
    let source = format!("fn id(x: bool) -> bool {{ return x; }} fn main() -> bool {{ let value: bool = {expression}; return id(value); }}");
    let fixture = Fixture::new(&source);
    let check = fixture.command(&["check", "main.ox", "--edition=typed-preview"]);
    assert_eq!(check.status.code(), Some(0), "{source}: {check:?}");
    for json in [false, true] {
        let out = fixture.run(json);
        assert_eq!(out.status.code(), Some(0), "{source}: {out:?}");
        assert!(out.stderr.is_empty());
        let expected = if json {
            summary(
                true,
                0,
                &format!("{{\"type\":\"bool\",\"value\":{expected}}}"),
            )
        } else {
            format!("{expected}\n")
        };
        assert_eq!(out.stdout, expected.as_bytes(), "{source}");
    }
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
}

fn reject(source: &str, code: &str, stage: &str, start: usize, width: usize) {
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
        if operation == "run" {
            assert!(text.ends_with(&summary(false, 1, "null")), "{text}");
        }
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
}

#[test]
fn signed_i32_comparisons_return_exact_bool_text_json_and_exit_zero() {
    for (left, right) in [
        (i32::MIN, i32::MAX),
        (i32::MAX, i32::MIN),
        (i32::MIN, i32::MIN),
        (i32::MAX, i32::MAX),
        (-1, 0),
        (0, -1),
        (0, 0),
        (1, 1),
        (-1, -1),
    ] {
        for (op, expected) in [
            ("==", left == right),
            ("!=", left != right),
            ("<", left < right),
            ("<=", left <= right),
            (">", left > right),
            (">=", left >= right),
        ] {
            assert_bool(&format!("{left}{op}{right}"), expected);
        }
    }
}

#[test]
fn bool_equality_has_complete_truth_tables_and_no_integer_conversion() {
    for left in [false, true] {
        for right in [false, true] {
            assert_bool(&format!("{left}=={right}"), left == right);
            assert_bool(&format!("{left}!={right}"), left != right);
        }
    }
}

#[test]
fn arithmetic_binds_more_tightly_and_groups_reopen_the_comparison_tier() {
    for (expression, expected) in [
        ("1 + 2 < 4 * 2", true),
        ("1 + 2 * 3 == 7", true),
        ("20 - 4 - 3 >= 13", true),
        ("1--2<=3", true),
        ("-2147483648 < -1 + 0", true),
        ("(1 < 2) == true", true),
        ("false == (2 < 1)", true),
        ("(1 == 1) != (2 >= 2)", false),
        ("((1 < 2) == true) != false", true),
        ("1 /* 雪 */ <= /* 🦀 */ 2", true),
        ("1 != // é\r\n 2", true),
    ] {
        assert_bool(expression, expected);
    }
}

#[test]
fn every_unparenthesized_comparison_pair_rejects_at_the_second_operator() {
    for first in ["==", "!=", "<", "<=", ">", ">="] {
        for second in ["==", "!=", "<", "<=", ">", ">="] {
            let prefix = format!("// 🦀\r\nfn main() -> bool {{ return 1 {first} 2 /* 雪 */ ");
            let source = format!("{prefix}{second} 3; }}");
            reject(&source, "E0100", "parse", prefix.len(), second.len());
        }
    }
}

#[test]
fn all_invalid_scalar_pairs_reject_the_first_invalid_operand_even_in_dead_code() {
    for op in ["==", "!=", "<", "<=", ">", ">="] {
        for (left_ty, left) in [("i32", "1"), ("bool", "true"), ("unit", "()")] {
            for (right_ty, right) in [("i32", "2"), ("bool", "false"), ("unit", "()")] {
                let equality = op == "==" || op == "!=";
                if left_ty == right_ty && (left_ty == "i32" || (equality && left_ty == "bool")) {
                    continue;
                }
                let expression = format!("{left} {op} {right}");
                let invalid_left = left_ty == "unit" || (!equality && left_ty != "i32");
                let (offset, width) = if invalid_left {
                    (0, left.len())
                } else {
                    (left.len() + op.len() + 2, right.len())
                };
                for (prefix, suffix) in [
                    ("// 雪\r\nfn main() -> bool { return ", "; }"),
                    (
                        "fn main() -> bool { return true; } fn unused() -> bool { return ",
                        "; }",
                    ),
                    (
                        "fn main() -> bool { if true { return true; } else { return ",
                        "; } }",
                    ),
                ] {
                    let source = format!("{prefix}{expression}{suffix}");
                    reject(&source, "E0300", "type", prefix.len() + offset, width);
                }
            }
        }
    }
    for (expr, origin) in [
        ("(1 < 2) + 3", "(1 < 2)"),
        ("(1 < 2) < 3", "(1 < 2)"),
        ("1 < (2 < 3)", "(2 < 3)"),
    ] {
        let source = format!("fn main() -> bool {{ return {expr}; }}");
        reject(
            &source,
            "E0300",
            "type",
            source.find(origin).unwrap(),
            origin.len(),
        );
    }
}

#[test]
fn comparisons_feed_calls_bindings_conditions_and_discarded_results() {
    for (source, expected) in [
        ("fn main() -> bool { return later(1 < 2); } fn later(x: bool) -> bool { return x != false; }", "true\n"),
        ("fn main() -> bool { let inferred = -1 < 0; let explicit: bool = inferred; return explicit == false; }", "false\n"),
        ("fn main() -> i32 { if -2147483648 < 2147483647 { return 42; } else { return 2147483647 + 1; } }", "42\n"),
        ("fn main() -> i32 { if 1 > 2 { return -2147483648 - 1; } return 7; }", "7\n"),
        ("fn main() -> () { 1 < 2; false != true; let ignored = 2 <= 1; return; }", "()\n"),
        ("fn unused() -> bool { return 2147483647 + 1 == 0; } fn main() -> bool { return 0 != 0; }", "false\n"),
    ] {
        let out = Fixture::new(source).run(false);
        assert_eq!(out.status.code(), Some(0), "{source}: {out:?}");
        assert_eq!(out.stdout, expected.as_bytes());
        assert!(out.stderr.is_empty());
    }
}

#[test]
fn both_operand_subtrees_and_arguments_run_left_first_with_unchanged_overflow_origins() {
    let helpers = "// 🦀\r\nfn later() -> i32 { return -2147483648 - 1; }\r\nfn first() -> i32 { return 2147483647 + 1; }\r\nfn take(a: bool, b: bool) -> bool { return b; }\r\n";
    for (body, marker) in [
        ("return first() < later();", "+ 1"),
        ("return later() >= first();", "- 1"),
        ("return 0 == first();", "+ 1"),
        ("return first() != 0;", "+ 1"),
        ("return (first() < 1) == (later() > 2);", "+ 1"),
        ("return take(first() < 0, later() > 0);", "+ 1"),
        ("return take(later() < 0, first() > 0);", "- 1"),
        ("first() < 1; return later() == 0;", "+ 1"),
        ("let ignored = later() != 0; return first() < 1;", "- 1"),
        ("if first() < 0 { return true; } return false;", "+ 1"),
    ] {
        let source = format!("{helpers}fn main() -> bool {{ {body} }}");
        let fixture = Fixture::new(&source);
        let check = fixture.command(&["check", "main.ox", "--edition=typed-preview"]);
        assert_eq!(check.status.code(), Some(0), "{check:?}");
        let out = fixture.run(true);
        assert_eq!(out.status.code(), Some(1), "{source}: {out:?}");
        assert!(out.stderr.is_empty());
        let text = String::from_utf8(out.stdout).unwrap();
        let start = source.find(marker).unwrap();
        assert!(text.contains("\"code\":\"E0604\""), "{text}");
        assert!(text.contains("\"stage\":\"oir-run\""), "{text}");
        assert!(
            text.contains(&format!("\"start\":{start},\"end\":{}", start + 1)),
            "{text}"
        );
        assert!(text.ends_with(&summary(false, 1, "null")));
        let out = fixture.run(false);
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stdout.is_empty());
        assert!(String::from_utf8(out.stderr)
            .unwrap()
            .starts_with("error[E0604] (oir-run): checked i32 arithmetic overflow\n"));
    }
}

#[test]
fn comparisons_preserve_left_first_resolution_and_do_not_short_circuit() {
    for expr in ["left() < right()", "(left() == 0) == (right() != 0)"] {
        let source = format!("fn main() -> bool {{ return {expr}; }}");
        reject(&source, "E0200", "resolve", source.find("left").unwrap(), 4);
    }
    let helpers =
        "fn recurse() -> i32 { return recurse(); } fn fail() -> i32 { return 2147483647 + 1; }";
    for (expression, code) in [
        ("fail() < recurse()", "E0604"),
        ("recurse() < fail()", "E0602"),
        ("0 == recurse()", "E0602"),
    ] {
        let out = Fixture::new(&format!(
            "{helpers} fn main() -> bool {{ return {expression}; }}"
        ))
        .run(true);
        assert_eq!(out.status.code(), Some(1));
        assert!(String::from_utf8(out.stdout)
            .unwrap()
            .contains(&format!("\"code\":\"{code}\"")));
    }
}

#[test]
fn comparison_and_grouping_nodes_share_the_existing_total_height_bound() {
    for terms in [61, 62, 63, 64] {
        let sum = vec!["0"; terms].join("+");
        for (expression, height) in [
            (format!("{sum} == 0"), terms + 1),
            (format!("({sum} == 0)"), terms + 2),
            (format!("id({sum} == 0)"), terms + 2),
            (format!("({sum} == 0) == true"), terms + 3),
        ] {
            let source = format!("fn id(x: bool) -> bool {{ return x; }} fn main() -> bool {{ return {expression}; }}");
            let out = Fixture::new(&source).run(true);
            assert_eq!(
                out.status.code(),
                Some(if height <= 64 { 0 } else { 1 }),
                "height {height}: {source}: {out:?}"
            );
            if height > 64 {
                assert!(String::from_utf8(out.stdout)
                    .unwrap()
                    .contains("\"code\":\"E0400\""));
            }
        }
    }
    let mut expression = "true".to_owned();
    for _ in 0..31 {
        expression = format!("({expression}) == true");
    }
    assert_bool(&format!("({expression})"), true);
    let out = Fixture::new(&format!(
        "fn main() -> bool {{ return ({expression}) == true; }}"
    ))
    .run(true);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("\"code\":\"E0400\""));
}

#[test]
fn adjacent_tokens_are_required_and_unrelated_operators_remain_unsupported() {
    for (expression, origin) in [
        ("1 = = 1", "="),
        ("1 ! = 2", "!"),
        ("1 < = 2", "="),
        ("1 > /* 雪 */ = 2", "="),
        ("1 =/* 雪 */= 2", "="),
        ("1 !/* 雪 */=2", "!"),
        ("true & false", "&"),
        ("true | false", "|"),
        ("true or false", "or"),
        ("1 / 2 == 0", "/"),
        ("1 as bool", "as"),
    ] {
        let prefix = "// 🦀\r\nfn main() -> bool { return ";
        let source = format!("{prefix}{expression}; }}");
        reject(
            &source,
            if matches!(origin, "!" | "=") {
                "E0100"
            } else {
                "E0101"
            },
            "parse",
            prefix.len() + expression.find(origin).unwrap(),
            origin.len(),
        );
    }
    for expression in ["1 <", "true ==", "1 <=", "1 < < 2"] {
        let out = Fixture::new(&format!("fn main() -> bool {{ return {expression}; }}")).run(true);
        assert_eq!(out.status.code(), Some(1));
        assert!(String::from_utf8(out.stdout)
            .unwrap()
            .contains("\"code\":\"E0100\""));
    }
}
