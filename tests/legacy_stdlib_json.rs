//! Coverage of existing legacy JSON acceptance and malformed-input rejection.
//! Error text is characterized here, not promoted to a stable diagnostic schema.
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);

impl Project {
    fn new(source: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time follows Unix epoch")
            .as_nanos();
        for _ in 0..16 {
            let root = std::env::temp_dir().join(format!(
                "oxid-stdlib-json-{}-{stamp}-{}",
                std::process::id(),
                NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&root) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create isolated JSON project: {error}"),
            }
            let project = Self(root);
            fs::create_dir(project.0.join("stdlib")).expect("create stdlib directory");
            // The checkout's JSON module currently has no imported dependencies.
            fs::write(
                project.0.join("stdlib/json.ox"),
                include_str!("../stdlib/json.ox"),
            )
            .expect("write actual JSON module");
            fs::write(project.0.join("main.ox"), source).expect("write JSON fixture");
            return project;
        }
        panic!("could not reserve an exclusive JSON project directory");
    }

    fn command(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(arguments)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env_remove("OXID_PATH")
            .output()
            .expect("run Oxid CLI")
    }

    fn compile(&self) {
        let output = self.command(&["compile", "main.ox", "-o", "main.oxb"]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert!(fs::read(self.0.join("main.oxb"))
            .expect("read compiled artifact")
            .starts_with(b"OXBC"));
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_valid_object(output: &Output) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"{\"a\":1}\n");
    assert!(output.stderr.is_empty(), "{output:?}");
}

fn assert_trailing_comma_rejected(output: &Output) {
    assert_trailing_comma_rejected_at(output, 7);
}

fn assert_stdout(output: &Output, expected: &[u8]) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty(), "{output:?}");
}

fn assert_trailing_comma_rejected_at(output: &Output, offset: usize) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let diagnostic = std::str::from_utf8(&output.stderr).expect("UTF-8 diagnostic");
    assert!(diagnostic.starts_with("error: "), "{diagnostic}");
    // Pin the current reason and ASCII offset, not environment-dependent paths. The
    // offset is the existing parser's index of the closing delimiter after whitespace.
    let reason = format!("trailing commas are not valid JSON at byte {offset}");
    assert!(diagnostic.trim_end().ends_with(&reason), "{diagnostic}");
}

#[test]
fn json_decode_accepts_neighboring_valid_object_on_both_routes() {
    let project = Project::new(
        r#"use "stdlib/json.ox";
print json_encode(json_decode("{\"a\":1}"));
"#,
    );
    assert_valid_object(&project.command(&["run", "main.ox"]));
    project.compile();
    assert_valid_object(&project.command(&["run", "main.oxb"]));
}

#[test]
fn json_decode_rejects_object_trailing_comma_on_both_routes() {
    // docs/API.md documents rejection of malformed JSON input.
    let project = Project::new(
        r#"use "stdlib/json.ox";
print json_decode("{\"a\":1,}");
"#,
    );
    assert_trailing_comma_rejected(&project.command(&["run", "main.ox"]));
    project.compile();
    assert_trailing_comma_rejected(&project.command(&["run", "main.oxb"]));
}

#[test]
fn json_decode_accepts_neighboring_valid_array_and_spaced_object_on_both_routes() {
    let project = Project::new(
        r#"use "stdlib/json.ox";
print json_encode(json_decode("[1]"));
print json_encode(json_decode("{\"a\":1 }"));
"#,
    );
    let expected = b"[1]\n{\"a\":1}\n";
    assert_stdout(&project.command(&["run", "main.ox"]), expected);
    project.compile();
    assert_stdout(&project.command(&["run", "main.oxb"]), expected);
}

#[test]
fn json_decode_rejects_array_trailing_comma_on_both_routes() {
    // `[1,]`: the closing `]` is byte 3.
    let project = Project::new(
        r#"use "stdlib/json.ox";
print json_decode("[1,]");
"#,
    );
    assert_trailing_comma_rejected_at(&project.command(&["run", "main.ox"]), 3);
    project.compile();
    assert_trailing_comma_rejected_at(&project.command(&["run", "main.oxb"]), 3);
}

#[test]
fn json_decode_rejects_object_trailing_comma_before_whitespace_on_both_routes() {
    // `{"a":1, }`: whitespace after the comma is skipped; the closing `}` is byte 8.
    let project = Project::new(
        r#"use "stdlib/json.ox";
print json_decode("{\"a\":1, }");
"#,
    );
    assert_trailing_comma_rejected_at(&project.command(&["run", "main.ox"]), 8);
    project.compile();
    assert_trailing_comma_rejected_at(&project.command(&["run", "main.oxb"]), 8);
}

#[test]
fn json_decode_accepts_array_with_whitespace_before_close_on_both_routes() {
    // `[1 ]`: positive neighbor of `[1, ]`; whitespace before `]` without a comma is valid.
    let project = Project::new(
        r#"use "stdlib/json.ox";
print json_encode(json_decode("[1 ]"));
"#,
    );
    assert_stdout(&project.command(&["run", "main.ox"]), b"[1]\n");
    project.compile();
    assert_stdout(&project.command(&["run", "main.oxb"]), b"[1]\n");
}

#[test]
fn json_decode_rejects_array_trailing_comma_before_whitespace_on_both_routes() {
    // `[1, ]`: whitespace after the comma is skipped; the closing `]` is byte 4.
    let project = Project::new(
        r#"use "stdlib/json.ox";
print json_decode("[1, ]");
"#,
    );
    assert_trailing_comma_rejected_at(&project.command(&["run", "main.ox"]), 4);
    project.compile();
    assert_trailing_comma_rejected_at(&project.command(&["run", "main.oxb"]), 4);
}
