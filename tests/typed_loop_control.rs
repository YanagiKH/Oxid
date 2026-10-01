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
            "oxid-loop-control-{}-{}",
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
fn loop_transfers_choose_innermost_target_and_recheck_condition() {
    for (source, expected) in [
        ("fn main()->i32 { while true { break; } return 7; }", "7"),
        ("fn main()->i32 { let mut n=0; while n<3 { n=n+1; continue; } return n; }", "3"),
        ("fn main()->i32 { let mut n=0; let mut sum=0; while n<4 { n=n+1; let mut j=0; while true { j=j+1; if j==2 { break; } sum=sum+n; } if n<3 { continue; } sum=sum+10; } return sum; }", "30"),
        ("fn main()->i32 { let mut n=0; while n<3 { n=n+1; if n<2 { continue; } else { break; } } return n; }", "2"),
        ("fn choose(b:bool)->i32 { while true { if b { return 4; } else { break; } } return 7; } fn main()->i32 { return choose(true)+choose(false); }", "11"),
    ] { assert_result(source, "i32", expected); }
}

#[test]
fn transfers_skip_effects_but_keep_dead_paths_checked() {
    for flag in [false, true] {
        let source=format!("fn bad()->i32 {{ return 2147483647+1; }} fn main()->i32 {{ let mut n=0; while n<3 {{ n=n+1; if {flag} {{ break; }} else {{ continue; }} }} return n; }}");
        assert_result(&source, "i32", if flag { "1" } else { "3" });
    }
    assert_result("fn bad()->bool { return (2147483647+1)==0; } fn main()->bool { let mut b=true; while b && (true || bad()) { b=false; continue; } return b; }","bool","false");
    assert_result(
        "fn main()->() { while true { break; } return; }",
        "unit",
        "()",
    );
    assert_result("fn main()->i32 { let mut n=0; let mut total=0; while n<4 { let old=n; let mut fresh=2; n=n+1; if n<3 { continue; } fresh=fresh+old; total=total+fresh; } let old=7; return total+old; }","i32","16");
    for (source,code,stage) in [
        ("fn main()->() { break; return; }","E0204","resolve"),
        ("fn main()->() { continue; return; }","E0204","resolve"),
        ("fn other()->() { break; return; } fn main()->() { while true { other(); break; } return; }","E0204","resolve"),
        ("fn main()->() { while false {} continue; return; }","E0204","resolve"),
        ("fn main()->() { while false { break; } if false { continue; } return; }","E0204","resolve"),
        ("fn main()->() { while false { if true { break; } else { absent; } } return; }","E0200","resolve"),
        ("fn main()->() { while false { if true { break; } else { let x:bool=1; } } return; }","E0300","type"),
        ("fn main()->() { while true { break; } }","E0302","type"),
        ("fn main()->() { while true { continue; } }","E0302","type"),
        ("fn main()->() { while true { break; (); } return; }","E0303","type"),
        ("fn main()->() { while false { continue; (); } return; }","E0303","type"),
        ("fn main()->() { while false { if true { break; } else { continue; } (); } return; }","E0303","type"),
        ("fn main()->() { while false { if true { return; } else { break; } (); } return; }","E0303","type"),
        ("fn main()->() { while false { if true { continue; } else { return; } (); } return; }","E0303","type"),
    ] { assert_error(source,code,stage); }
}

fn assert_error(source: &str, code: &str, stage: &str) {
    let fixture = Fixture::new(source);
    for operation in ["check", "run", "compile"] {
        let mut args = vec![
            operation,
            "main.ox",
            "--edition=typed-preview",
            "--message-format=json",
        ];
        if operation == "compile" {
            args.extend(["--backend=llvm", "--output", "must-not-exist"]);
        }
        let result = fixture.command(&args);
        assert_eq!(result.status.code(), Some(1), "{source}: {result:?}");
        assert!(result.stderr.is_empty(), "{result:?}");
        let text = String::from_utf8(result.stdout).unwrap();
        assert!(
            text.contains(&format!("\"code\":\"{code}\"")),
            "{source}: {text}"
        );
        assert!(
            text.contains(&format!("\"stage\":\"{stage}\"")),
            "{source}: {text}"
        );
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1, "{source}");
    }
}

#[test]
fn loop_transfers_are_semicolon_only_statements() {
    for body in [
        "break 1;",
        "continue true;",
        "break",
        "continue",
        "let x=break;",
        "return continue;",
        "while break {}",
        "if continue {}",
        "(break);",
        "break x;",
        "continue x;",
        "break();",
        "let break=1;",
        "let continue=1;",
    ] {
        assert_error(
            &format!("fn main()->() {{ while false {{ {body} }} return; }}"),
            "E0100",
            "parse",
        );
    }
    for body in ["break 'label;", "continue 'label;"] {
        assert_error(
            &format!("fn main()->() {{ while false {{ {body} }} return; }}"),
            "E0100",
            "parse",
        );
    }
}

#[test]
fn transfers_preserve_scope_depth_and_exact_full_statement_origins() {
    for (depth, code) in [(63, None), (64, Some("E0400"))] {
        let text = format!(
            "fn main()->() {{ {} {} return; }}",
            "while false {".repeat(depth),
            "break; }".repeat(depth)
        );
        if let Some(code) = code {
            assert_error(&text, code, "parse");
        } else {
            assert_result(&text, "unit", "()");
        }
    }
    for word in ["break", "continue"] {
        let statement = format!("{word} /* 雪🦀 */ ;");
        let text = format!("// 🦀\r\nfn main()->() {{ {statement} return; }}");
        let fixture = Fixture::new(&text);
        let result = fixture.command(&[
            "check",
            "main.ox",
            "--edition=typed-preview",
            "--message-format=json",
        ]);
        let diagnostic = String::from_utf8(result.stdout).unwrap();
        let start = text.find(&statement).unwrap();
        assert!(
            diagnostic.contains(&format!(
                "\"start\":{start},\"end\":{}",
                start + statement.len()
            )),
            "{diagnostic}"
        );
    }
}

#[test]
fn continue_infinite_loop_stops_at_shared_fuel_limit() {
    let fixture = Fixture::new("fn main()->() { while true { continue; } return; }");
    let result = fixture.run(false);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8(result.stderr)
        .unwrap()
        .contains("error[E0601] (oir-run): execution fuel exhausted"));
}
