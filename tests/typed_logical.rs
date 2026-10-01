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
            "oxid-logical-{}-{}",
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
fn logical_truth_tables_return_exact_bool_text_and_json() {
    assert_bool("!true", false);
    assert_bool("!!false", false);
    for left in [false, true] {
        for right in [false, true] {
            assert_bool(&format!("{left} && {right}"), left && right);
            assert_bool(&format!("{left} || {right}"), left || right);
        }
    }
}

#[test]
fn precedence_associativity_trivia_and_adjacent_not_equal_are_preserved() {
    assert_bool(&format!("false{}", "||true&&false".repeat(30)), false);
    for (expression, expected) in [
        ("true || false && false", true),
        ("false && true || true", true),
        ("!false == true", true),
        ("1 < 2 && 3 >= 3", true),
        ("(1 < 2) && !(3 == 4)", true),
        ("1+2*3==7 && 20-4-3==13 || false", true),
        ("false || false || true", true),
        ("true && true && false", false),
        ("!true!=!false", true),
        ("!!true", true),
        ("! /* 雪 */ false && // 🦀\r\n true", true),
        ("false/*é*/||/*雪*/!true", false),
        ("(false || true) == (true && !false)", true),
        ("!(false || true) != !!false", false),
    ] {
        assert_bool(expression, expected);
    }
}

#[test]
fn skipped_overflows_are_dormant_and_nested_join_values_reach_every_context() {
    for (source, expected) in [
        ("fn main() -> bool { return false && 2147483647 + 1 == 0; }", "false\n"),
        ("fn main() -> bool { return true || -2147483648 * -1 == 0; }", "true\n"),
        ("fn main() -> bool { return !(false && (true || 2147483647 + 1 == 0)); }", "true\n"),
        ("fn main() -> bool { return (true || 2147483647 + 1 == 0) && (false || true); }", "true\n"),
        ("fn id(x: bool) -> bool { return x; } fn main() -> bool { let x = false || true; let y: bool = !x; return id(x && !y); }", "true\n"),
        ("fn main() -> () { false && 2147483647 + 1 == 0; true || -2147483648 - 1 == 0; let ignored = !false; return; }", "()\n"),
        ("fn main() -> i32 { if false || !false { return 42; } else { return 2147483647 + 1; } }", "42\n"),
        ("fn main() -> i32 { if true && !true { return -2147483648 - 1; } return 7; }", "7\n"),
        ("fn unused() -> bool { return true && 2147483647 + 1 == 0; } fn main() -> bool { return !false; }", "true\n"),
        ("fn id(x: bool) -> bool { return x; } fn main() -> bool { return id((1+2<4) && (5*6==30)) || id(false); }", "true\n"),
    ] {
        let fixture = Fixture::new(source);
        let check = fixture.command(&["check", "main.ox", "--edition=typed-preview"]);
        assert_eq!(check.status.code(), Some(0), "{source}: {check:?}");
        let output = fixture.run(false);
        assert_eq!(output.status.code(), Some(0), "{source}: {output:?}");
        assert_eq!(output.stdout, expected.as_bytes(), "{source}");
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn executed_errors_obey_left_first_argument_and_statement_order_with_exact_origins() {
    let helpers = "// 🦀\r\nfn later() -> bool { return -2147483648 - 1 == 0; }\r\nfn first() -> bool { return 2147483647 + 1 == 0; }\r\nfn take(a: bool, b: bool) -> bool { return b; }\r\n";
    for (body, marker) in [
        ("return first() && later();", "+ 1"),
        ("return later() || first();", "- 1"),
        ("return true && first();", "+ 1"),
        ("return false || later();", "- 1"),
        ("return (false && first()) || later();", "- 1"),
        ("return (true || later()) && first();", "+ 1"),
        ("return take(first(), false && later());", "+ 1"),
        ("return take(false && first(), later());", "- 1"),
        ("return take(true || later(), first());", "+ 1"),
        ("return take(later(), true || first());", "- 1"),
        ("return !(false || first());", "+ 1"),
        ("return (false || first()) == later();", "+ 1"),
        ("true && first(); return later();", "+ 1"),
        ("let ignored = false || later(); return first();", "- 1"),
        (
            "if false || first() { return true; } return later();",
            "+ 1",
        ),
    ] {
        let source = format!("{helpers}fn main() -> bool {{ {body} }}");
        let fixture = Fixture::new(&source);
        let checked = fixture.command(&["check", "main.ox", "--edition=typed-preview"]);
        assert_eq!(checked.status.code(), Some(0), "{source}: {checked:?}");
        let start = source.find(marker).unwrap();
        let output = fixture.run(true);
        assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        for expected in [
            "\"code\":\"E0604\"".to_owned(),
            "\"stage\":\"oir-run\"".to_owned(),
            format!("\"start\":{start},\"end\":{}", start + 1),
        ] {
            assert!(text.contains(&expected), "{source}: {text}");
        }
        assert!(text.ends_with(&summary(false, 1, "null")));
        let output = fixture.run(false);
        let line = source[..start]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1;
        let column = source[..start].rsplit('\n').next().unwrap().chars().count() + 1;
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, format!("error[E0604] (oir-run): checked i32 arithmetic overflow\n  --> main.ox:{line}:{column}\n").as_bytes());
    }
}

#[test]
fn all_non_bool_operands_reject_even_in_skipped_or_unused_source() {
    for operand in ["1", "()"] {
        for prefix in [
            "// 雪\r\nfn main() -> bool { return !",
            "fn main() -> bool { return true; } fn unused() -> bool { return !",
        ] {
            reject(
                &format!("{prefix}{operand}; }}"),
                "E0300",
                "type",
                prefix.len(),
                operand.len(),
            );
        }
    }
    for op in ["&&", "||"] {
        for left in ["1", "()", "true", "false"] {
            for right in ["2", "()", "true", "false"] {
                if ["true", "false"].contains(&left) && ["true", "false"].contains(&right) {
                    continue;
                }
                for prefix in [
                    "// 🦀\r\nfn main() -> bool { return ",
                    "fn main() -> bool { return true; } fn unused() -> bool { return ",
                    "fn main() -> bool { if true { return true; } else { return ",
                ] {
                    let expression = format!("{left} {op} {right}");
                    let (offset, width) = if ["true", "false"].contains(&left) {
                        (left.len() + op.len() + 2, right.len())
                    } else {
                        (0, left.len())
                    };
                    let closing = if prefix.contains("if true") {
                        "; } }"
                    } else {
                        "; }"
                    };
                    reject(
                        &format!("{prefix}{expression}{closing}"),
                        "E0300",
                        "type",
                        prefix.len() + offset,
                        width,
                    );
                }
            }
        }
    }
    for expression in ["!1 < 2", "!true + 1", "!true * 2"] {
        let prefix = "fn main() -> bool { return ";
        let (offset, width) = if expression == "!1 < 2" {
            (1, 1)
        } else {
            (0, 5)
        };
        reject(
            &format!("{prefix}{expression}; }}"),
            "E0300",
            "type",
            prefix.len() + offset,
            width,
        );
    }
}

#[test]
fn skipped_rhs_resolution_is_eager_and_preserves_phase_priority() {
    for (expression, missing) in [
        ("false && missing()", "missing"),
        ("true || missing()", "missing"),
        ("left() && right()", "left"),
        ("left() || right()", "left"),
        ("1 && missing()", "missing"),
    ] {
        let source = format!("fn main() -> bool {{ return {expression}; }}");
        reject(
            &source,
            "E0200",
            "resolve",
            source.find(missing).unwrap(),
            missing.len(),
        );
    }
}

#[test]
fn malformed_tokens_missing_operands_and_comparison_chains_stay_rejected() {
    let prefix = "// 🦀\r\nfn main() -> bool { return ";
    for (expression, marker, code) in [
        ("true & false", "&", "E0101"),
        ("true | false", "|", "E0101"),
        ("true & & false", "&", "E0101"),
        ("true | /* 雪 */ | false", "|", "E0101"),
        ("true ^ false", "^", "E0100"),
        ("~true", "~", "E0100"),
        ("true ! false", "!", "E0100"),
        ("true && || false", "||", "E0100"),
        ("true || && false", "&&", "E0100"),
        ("-(1)", "-", "E0101"),
    ] {
        reject(
            &format!("{prefix}{expression}; }}"),
            code,
            "parse",
            prefix.len() + expression.find(marker).unwrap(),
            marker.len(),
        );
    }
    for expression in ["true &&", "false ||", "!", "true && !"] {
        reject(
            &format!("{prefix}{expression}; }}"),
            "E0100",
            "parse",
            prefix.len() + expression.len(),
            1,
        );
    }
    for first in ["==", "!=", "<", "<=", ">", ">="] {
        for second in ["==", "!=", "<", "<=", ">", ">="] {
            let begin = format!("{prefix}true && 1 {first} 2 /* 雪 */ ");
            reject(
                &format!("{begin}{second} 3 || false; }}"),
                "E0100",
                "parse",
                begin.len(),
                second.len(),
            );
        }
    }
}

#[test]
fn logical_nodes_share_the_64_height_limit_with_calls_groups_and_comparisons() {
    for count in [62, 63, 64] {
        let nots = "!".repeat(count);
        for (expression, height) in [
            (format!("{nots}true"), count + 1),
            (format!("({nots}true)"), count + 2),
            (format!("id({nots}true)"), count + 2),
            (format!("{nots}true && false"), count + 2),
            (format!("{nots}true == false"), count + 2),
        ] {
            let source = format!("fn id(x: bool) -> bool {{ return x; }} fn main() -> bool {{ return {expression}; }}");
            let output = Fixture::new(&source).run(true);
            assert_eq!(
                output.status.code(),
                Some(if height <= 64 { 0 } else { 1 }),
                "height={height}, {source}: {output:?}"
            );
            if height > 64 {
                assert!(String::from_utf8(output.stdout)
                    .unwrap()
                    .contains("\"code\":\"E0400\""));
            }
        }
    }
    for terms in [62, 63, 64, 65] {
        for expression in [
            vec!["true"; terms].join("&&"),
            vec!["false"; terms].join("||"),
        ] {
            let output =
                Fixture::new(&format!("fn main() -> bool {{ return {expression}; }}")).run(true);
            assert_eq!(
                output.status.code(),
                Some(if terms <= 64 { 0 } else { 1 }),
                "{output:?}"
            );
            if terms > 64 {
                assert!(String::from_utf8(output.stdout)
                    .unwrap()
                    .contains("\"code\":\"E0400\""));
            }
        }
    }
    // These two nodes expose left associativity through the height bound:
    // ((63-high left) op right) op right has height 65, not 64.
    for op in ["&&", "||"] {
        let expression = format!("{}true {op} false {op} true", "!".repeat(62));
        let output =
            Fixture::new(&format!("fn main() -> bool {{ return {expression}; }}")).run(true);
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains("\"code\":\"E0400\""));
    }
    let output = Fixture::new(&format!(
        "fn main() -> bool {{ return {}true; }}",
        "!".repeat(5000)
    ))
    .run(true);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("\"code\":\"E0400\""));
}

#[test]
fn reference_recursion_is_skipped_only_when_logical_value_short_circuits() {
    let helper = "fn recurse() -> bool { return recurse(); } ";
    for (expression, result) in [
        ("false && recurse()", "false\n"),
        ("true || recurse()", "true\n"),
    ] {
        let output = Fixture::new(&format!(
            "{helper}fn main() -> bool {{ return {expression}; }}"
        ))
        .run(false);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, result.as_bytes());
        assert!(output.stderr.is_empty());
    }
    for expression in [
        "true && recurse()",
        "false || recurse()",
        "recurse() && false",
        "recurse() || true",
    ] {
        let output = Fixture::new(&format!(
            "{helper}fn main() -> bool {{ return {expression}; }}"
        ))
        .run(true);
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains("\"code\":\"E0602\""));
    }
}
