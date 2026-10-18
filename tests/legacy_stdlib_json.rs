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
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let diagnostic = std::str::from_utf8(&output.stderr).expect("UTF-8 diagnostic");
    assert!(diagnostic.starts_with("error: "), "{diagnostic}");
    // Pin the current reason and ASCII offset, not environment-dependent paths.
    assert!(
        diagnostic.contains("trailing commas are not valid JSON at byte 7"),
        "{diagnostic}"
    );
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
