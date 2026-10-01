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
            "oxid-while-{}-{}",
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
fn while_runs_zero_one_many_and_nested_iterations() {
    for (source, result) in [
        ("fn main() -> i32 { let mut n = 7; while false { n = 9; } return n; }", "7"),
        ("fn main() -> i32 { let mut n = 0; while n < 3 { n = n + 1; } return n; }", "3"),
        ("fn main() -> i32 { let mut n=0; let mut sum=0; while n<4 { let mut j=0; while j<n { sum=sum+1; j=j+1; } n=n+1; } return sum; }", "6"),
        ("fn main() -> bool { let mut b=true; while b { b=false; } return b; }", "false"),
        ("fn main() -> () { let mut n=0; while n<2 { let mut u=(); u=(); n=n+1; } return; }", "()"),
    ] { assert_result(source, if result == "false" { "bool" } else if result == "()" { "unit" } else { "i32" }, result); }
}

#[test]
fn each_iteration_reexecutes_declarations_calls_and_snapshots() {
    assert_result("fn bump(x:i32)->i32 { return x+1; } fn main()->i32 { let mut n=0; let mut sum=0; while n<4 { let old=n; let mut fresh=10; fresh=fresh+old; n=bump(n); sum=sum+fresh+old; } return sum; }", "i32", "52");
    assert_result("fn yes(x:i32)->bool { return x<4; } fn main()->i32 { let mut n=0; while n<3 && yes(n) { n=n+1; } return n; }", "i32", "3");
    assert_result(
        "fn main()->i32 { while true { return 9; } return 2; }",
        "i32",
        "9",
    );
}

#[test]
fn loop_conditions_and_dead_bodies_are_fully_checked() {
    for (source, code) in [
        ("fn main()->() { while 1 {} return; }", "E0300"),
        ("fn main()->() { while () {} return; }", "E0300"),
        (
            "fn main()->() { while false { unknown; } return; }",
            "E0200",
        ),
        (
            "fn main()->() { while false { let mut x=1; x=true; } return; }",
            "E0300",
        ),
        ("fn main()->() { while true { return; } }", "E0302"),
        (
            "fn main()->() { while false { let x=1; } x; return; }",
            "E0200",
        ),
        (
            "fn main()->() { while false { break 1; } return; }",
            "E0100",
        ),
        (
            "fn main()->() { while false { continue 1; } return; }",
            "E0100",
        ),
        ("fn main()->() { return; while false {} }", "E0303"),
    ] {
        let fixture = Fixture::new(source);
        let output = fixture.command(&["check", "main.ox", "--edition=typed-preview"]);
        assert_eq!(output.status.code(), Some(1), "{source}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(code),
            "{source}: {output:?}"
        );
    }
}

#[test]
fn infinite_empty_loop_exhausts_shared_operation_budget() {
    let fixture = Fixture::new("fn main()->() { while true {} return; }");
    let output = fixture.run(false);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("error[E0601] (oir-run): execution fuel exhausted"),
        "{error}"
    );
}

#[test]
fn while_block_depth_and_expression_height_keep_existing_inclusive_limits() {
    for (count, accepted) in [(63, true), (64, false)] {
        let source = format!(
            "fn main()->() {{ {} {} return; }}",
            "while false {".repeat(count),
            "}".repeat(count)
        );
        let result =
            Fixture::new(&source).command(&["check", "main.ox", "--edition=typed-preview"]);
        assert_eq!(result.status.success(), accepted, "{result:?}");
        if !accepted {
            assert!(String::from_utf8_lossy(&result.stderr).contains("E0400"));
        }
    }
    for (count, accepted) in [(63, true), (64, false)] {
        let source = format!(
            "fn main()->() {{ while {}false {{}} return; }}",
            "!".repeat(count)
        );
        let result =
            Fixture::new(&source).command(&["check", "main.ox", "--edition=typed-preview"]);
        assert_eq!(result.status.success(), accepted, "{result:?}");
        if !accepted {
            assert!(String::from_utf8_lossy(&result.stderr).contains("E0400"));
        }
    }
}

#[test]
fn while_scopes_return_flow_and_lazy_effects_are_preserved() {
    assert_result(
        "fn main()->i32 { let mut n=0; while n<2 { let x=n; n=x+1; } let x=7; return x; }",
        "i32",
        "7",
    );
    assert_result("fn bad()->bool { 2147483647+1; return true; } fn main()->i32 { let mut n=0; while n<2 && (true || bad()) { n=n+1; } return n; }","i32","2");
    for (source, code) in [
        (
            "fn main()->() { while x { let x=false; } return; }",
            "E0200",
        ),
        (
            "fn main(x:bool)->() { while false { x=false; } return; }",
            "E0304",
        ),
        (
            "fn main()->() { let x=true; while false { let x=false; } return; }",
            "E0201",
        ),
        (
            "fn main()->() { while false { return; 1; } return; }",
            "E0303",
        ),
        ("fn main()->() { while false {} else {} return; }", "E0100"),
        ("fn main()->() { let x=while false {}; return; }", "E0100"),
    ] {
        let result = Fixture::new(source).command(&["check", "main.ox", "--edition=typed-preview"]);
        assert_eq!(result.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(code),
            "{result:?}"
        );
    }
}
