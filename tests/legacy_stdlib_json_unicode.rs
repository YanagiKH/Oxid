//! Regression coverage of `json_decode` Unicode escapes through the real stdlib
//! wrapper on both the source and OXBC routes. Expectations mirror the existing
//! runtime parser (`src/runtime/data.rs`); error text is characterized here, not
//! promoted to a stable diagnostic schema.
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
                "oxid-stdlib-json-unicode-{}-{stamp}-{}",
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

    /// Runs the source route, then compiles and runs the OXBC route.
    fn both_routes(&self) -> [Output; 2] {
        let source = self.command(&["run", "main.ox"]);
        self.compile();
        [source, self.command(&["run", "main.oxb"])]
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_stdout(output: &Output, expected: &[u8]) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, expected);
    assert!(output.stderr.is_empty(), "{output:?}");
}

fn assert_rejected(output: &Output, reason: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let diagnostic = std::str::from_utf8(&output.stderr).expect("UTF-8 diagnostic");
    assert!(diagnostic.starts_with("error: "), "{diagnostic}");
    // Pin the current reason and ASCII offset, not environment-dependent paths.
    assert!(diagnostic.trim_end().ends_with(reason), "{diagnostic}");
}

#[test]
fn json_decode_accepts_bmp_unicode_escape_on_both_routes() {
    // JSON text `"\u00e9"` decodes to U+00E9; `print` writes its raw UTF-8 bytes.
    let project = Project::new(
        r#"use "stdlib/json.ox";
print json_decode("\"\\u00e9\"");
"#,
    );
    for output in project.both_routes() {
        assert_stdout(&output, b"\xc3\xa9\n");
    }
}

#[test]
fn json_decode_accepts_surrogate_pair_escape_on_both_routes() {
    // JSON text `"\uD83D\uDE80"` combines into U+1F680 (UTF-8 F0 9F 9A 80).
    let project = Project::new(
        r#"use "stdlib/json.ox";
print json_decode("\"\\uD83D\\uDE80\"");
"#,
    );
    for output in project.both_routes() {
        assert_stdout(&output, b"\xf0\x9f\x9a\x80\n");
    }
}

#[test]
fn json_decode_rejects_lone_high_surrogate_on_both_routes() {
    // `"\uD800"`: the parser checks for a following `\u` after the quad, at byte 7.
    let project = Project::new(
        r#"use "stdlib/json.ox";
print json_decode("\"\\uD800\"");
"#,
    );
    for output in project.both_routes() {
        assert_rejected(
            &output,
            "high surrogate must be followed by a low surrogate at byte 7",
        );
    }
}

#[test]
fn json_decode_rejects_lone_low_surrogate_on_both_routes() {
    // `"\uDC00"`: a low surrogate without a preceding high one fails after the quad.
    let project = Project::new(
        r#"use "stdlib/json.ox";
print json_decode("\"\\uDC00\"");
"#,
    );
    for output in project.both_routes() {
        assert_rejected(&output, "unexpected low surrogate in JSON string at byte 7");
    }
}
