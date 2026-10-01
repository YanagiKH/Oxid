use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Project(PathBuf);

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn generated_api_describes_current_execution_boundaries() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let project = Project(std::env::temp_dir().join(format!(
        "oxid-generated-docs-{}-{nonce}",
        std::process::id()
    )));
    fs::create_dir(&project.0).expect("create isolated documentation project");
    let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
        .arg("doc")
        .current_dir(&project.0)
        .output()
        .expect("generate API documentation");
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let document = fs::read_to_string(project.0.join("docs/API.md"))
        .expect("read the existing generated API output path");
    for heading in [
        "# Oxid API",
        "## Built-ins",
        "## Commands",
        "## Language focus",
    ] {
        assert!(document.contains(heading), "missing heading: {heading}");
    }
    for claim in [
        "source interpretation",
        "lazy, memoized tasks",
        "join_all executes sequentially",
        "OXBC 1.0 serialized-AST artifacts",
        "artifact serialization round-trip, not compiler self-rebuild",
        "the production parser and binary artifact writer are Rust implementations",
    ] {
        assert!(
            document.contains(claim),
            "missing execution boundary: {claim}"
        );
    }
    for old_claim in [
        "fast script execution",
        "ergonomic async tasks",
        "deterministic versioned bytecode",
    ] {
        assert!(!document.contains(old_claim), "stale claim: {old_claim}");
    }
}
