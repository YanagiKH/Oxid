//! Public CLI coverage for exact, project-owned source-data classification.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-test-discovery-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create isolated project");
        Self(root)
    }

    fn write(&self, path: &str, text: &str) {
        let file = self.0.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }

    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env_remove("OXID_PATH")
            .output()
            .expect("run public Oxid CLI")
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_failure(output: &Output, reason: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains(reason), "{error}");
}

#[test]
fn exact_fixture_is_excluded_and_recursive_language_tests_are_retained() {
    let project = Project::new();
    project.write(
        "oxid.toml",
        "[test-fixtures]\n\"tests/fixtures/data.ox\" = true # compiler input\n",
    );
    project.write("tests/fixtures/data.ox", "fn main() -> i32 { return 0; }\n");
    for (path, marker) in [
        ("tests/root.ox", "root test"),
        ("tests/nested/deeper/case.ox", "nested test"),
        ("tests/fixtures/neighbor.ox", "fixture neighbor"),
        ("examples/fixtures/data.ox", "same name example"),
        ("examples/nested/case.ox", "nested example"),
    ] {
        project.write(path, &format!("print \"{marker}\";\n"));
    }
    let output = project.command(&["test"]);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("test: "))
            .count(),
        5
    );
    for marker in [
        "root test",
        "nested test",
        "fixture neighbor",
        "same name example",
        "nested example",
    ] {
        assert!(text.lines().any(|line| line == marker), "{text}");
    }
    assert!(!text.contains("test: ./tests/fixtures/data.ox"));
    // Classification affects discovery only, not explicit compiler operations.
    assert_failure(
        &project.command(&["run", "tests/fixtures/data.ox"]),
        "function body requires",
    );
}

#[test]
fn unlisted_failing_neighbor_is_still_executed() {
    let project = Project::new();
    project.write(
        "oxid.toml",
        "[test-fixtures]\n\"tests/fixtures/data.ox\" = true\n",
    );
    project.write("tests/fixtures/data.ox", "fn main() -> i32 { return 0; }\n");
    project.write("tests/fixtures/neighbor.ox", "print 1 / 0;\n");
    let output = project.command(&["test"]);
    assert_failure(&output, "division by zero");
    assert!(String::from_utf8_lossy(&output.stdout).contains("neighbor.ox"));
}

#[test]
fn default_discovery_has_no_repository_specific_exclusions() {
    for manifest in [None, Some("[project]\nname = \"ordinary\"\n")] {
        let project = Project::new();
        if let Some(manifest) = manifest {
            project.write("oxid.toml", manifest);
        }
        project.write("tests/fixtures/data.ox", "print \"ordinary fixture\";\n");
        let output = project.command(&["test"]);
        assert!(output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("ordinary fixture"));
    }
}

#[test]
fn malformed_fixture_configuration_fails_before_running_tests() {
    for entry in [
        "\"../outside.ox\" = true",
        "\"/absolute.ox\" = true",
        "\"C:/absolute.ox\" = true",
        "\"tests/../outside.ox\" = true",
        "\"tests//data.ox\" = true",
        "\"tests/./data.ox\" = true",
        "\"tests\\\\data.ox\" = true",
        "\"tests/*.ox\" = true",
        "\"tests/data.txt\" = true",
        "\"src/main.ox\" = true",
        "\"tests/missing.ox\" = true",
        "\"tests/directory.ox\" = true",
        "\"tests/data.ox\" = false",
        "\"tests/data.ox\" = \"true\"",
        "\"tests/data.ox\" = true,",
        "\"tests/data.ox\" = true\n\"tests/data.ox\" = true",
        "\"tests/data.ox\"",
    ] {
        let project = Project::new();
        project.write("oxid.toml", &format!("[test-fixtures]\n{entry}\n"));
        project.write("tests/data.ox", "print \"must not run\";\n");
        fs::create_dir(project.0.join("tests/directory.ox")).unwrap();
        let output = project.command(&["test"]);
        assert_failure(&output, "test fixture");
        assert!(output.stdout.is_empty(), "{entry}: {output:?}");
    }
}

#[cfg(unix)]
#[test]
fn fixture_symlink_cannot_classify_an_outside_source() {
    use std::os::unix::fs::symlink;
    let outside = Project::new();
    outside.write("outside.ox", "print \"outside\";\n");
    let project = Project::new();
    project.write("oxid.toml", "[test-fixtures]\n\"tests/data.ox\" = true\n");
    fs::create_dir(project.0.join("tests")).unwrap();
    symlink(
        outside.0.join("outside.ox"),
        project.0.join("tests/data.ox"),
    )
    .unwrap();
    let output = project.command(&["test"]);
    assert_failure(&output, "test fixture");
    assert!(output.stdout.is_empty(), "{output:?}");
}

#[test]
fn fixture_only_project_does_not_report_a_passing_suite() {
    let project = Project::new();
    project.write("oxid.toml", "[test-fixtures]\n\"tests/data.ox\" = true\n");
    project.write("tests/data.ox", "print \"fixture\";\n");
    assert_failure(
        &project.command(&["test"]),
        "no test or runnable example Oxid files found",
    );
}

#[test]
fn standard_fixture_table_spellings_never_execute_registered_data() {
    for header in [
        "[test-fixtures] # compiler inputs",
        "[ test-fixtures ]",
        "[\"test-fixtures\"]",
        "[ 'test-fixtures' ] # literal table name",
    ] {
        let project = Project::new();
        project.write(
            "oxid.toml",
            &format!("{header}\n\"tests/data.ox\" = true\n"),
        );
        project.write("tests/data.ox", "fn main() -> i32 { return 0; }\n");
        project.write("tests/real.ox", "print \"real test\";\n");
        let output = project.command(&["test"]);
        assert!(output.status.success(), "{header}: {output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("real test"), "{text}");
        assert!(!text.contains("data.ox"), "{text}");
    }
}

#[test]
fn commented_headers_preserve_fixture_table_ownership() {
    for position in [0, 1, 3] {
        let project = Project::new();
        let mut sections = vec![
            "[project] # project metadata\nname = \"ordinary\"\n",
            "[features] # features remain separate\nffi = true\n",
            "[metadata] # unrelated table\n\"tests/neighbor.ox\" = true\n",
        ];
        sections.insert(position, "[test-fixtures]\n\"tests/data.ox\" = true\n");
        project.write("oxid.toml", &sections.concat());
        project.write("tests/data.ox", "fn main() -> i32 { return 0; }\n");
        project.write("tests/neighbor.ox", "print \"neighbor test\";\n");
        let output = project.command(&["test"]);
        assert!(output.status.success(), "position {position}: {output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("neighbor test"));
    }
}

#[test]
fn malformed_table_headers_fail_before_any_test_execution() {
    for header in [
        "[test-fixtures",
        "[test-fixtures] trailing",
        "[test-fixtures]]",
        "[[test-fixtures]]",
        "[\"test-fixtures]",
        "[]",
        "[\"test-fixtures\"x]",
        "[\"test-fixtures\"]oops",
    ] {
        let project = Project::new();
        project.write(
            "oxid.toml",
            &format!("{header}\n\"tests/data.ox\" = true\n"),
        );
        project.write("tests/data.ox", "print \"must not run\";\n");
        let output = project.command(&["test"]);
        assert_failure(&output, "invalid manifest section");
        assert!(output.stdout.is_empty(), "{header}: {output:?}");
    }
}

#[test]
fn equals_inside_an_exact_quoted_fixture_path_is_not_an_assignment_separator() {
    let project = Project::new();
    project.write("oxid.toml", "[test-fixtures]\n\"tests/a=b.ox\" = true\n");
    project.write("tests/a=b.ox", "fn main() -> i32 { return 0; }\n");
    project.write("tests/a.ox", "print \"retained\";\n");
    let output = project.command(&["test"]);
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("retained"));
}

#[test]
fn trailing_comma_in_a_fixture_key_is_rejected_before_execution() {
    let project = Project::new();
    project.write("oxid.toml", "[test-fixtures]\n\"tests/data.ox\", = true\n");
    project.write("tests/data.ox", "print \"must not run\";\n");
    let output = project.command(&["test"]);
    assert_failure(&output, "invalid manifest key");
    assert!(output.stdout.is_empty(), "{output:?}");
}

#[test]
fn escaped_fixture_paths_share_identity_with_their_plain_spelling() {
    let project = Project::new();
    let manifest = "[test-fixtures]\n\"tests/da\\u0074a.ox\" = true\n";
    project.write("oxid.toml", manifest);
    project.write("tests/data.ox", "fn main() -> i32 { return 0; }\n");
    project.write("tests/real.ox", "print \"real\";\n");
    let output = project.command(&["test"]);
    assert!(output.status.success(), "{output:?}");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("data.ox"));
    project.write(
        "oxid.toml",
        &format!("{manifest}\"tests/data.ox\" = true\n"),
    );
    let output = project.command(&["test"]);
    assert_failure(&output, "duplicate test fixture");
    assert!(output.stdout.is_empty(), "{output:?}");
}

#[test]
fn unrelated_valid_manifest_sections_and_values_keep_default_discovery() {
    for document in [
        "[project]\nname = \"ordinary\"\n[[metadata]]\nname = \"any\"\n",
        "[tool.\"my-custom-tool\"]\nvalue = { nested = [1, 2] }\n",
        "[metadata]\ntext = \"\"\"\n[test-fixtures]\n\"tests/data.ox\" = true\n\"\"\"\n",
        "[metadata]\ntext = '''\n[test-fixtures]\n\"tests/data.ox\" = true\n'''\n",
        "[metadata]\nmatrix = [\n[\"test-fixtures\"],\n[\"another\"],\n]\n",
    ] {
        let project = Project::new();
        project.write("oxid.toml", document);
        project.write("tests/data.ox", "print \"ordinary data path\";\n");
        let output = project.command(&["test"]);
        assert!(output.status.success(), "{document}: {output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("ordinary data path"));
    }
}

#[test]
fn real_fixture_section_after_unrelated_multiline_data_is_recognized() {
    let project = Project::new();
    project.write(
        "oxid.toml",
        "[tool.\"my-custom-tool\"]\ntext = '''\n[test-fixtures]\n\"tests/real.ox\" = true\n'''\n\
         [[metadata]]\nname = \"any\"\n\
         [test-fixtures] # the actual classification\n\"tests/data.ox\" = true\n",
    );
    project.write("tests/data.ox", "fn main() -> i32 { return 0; }\n");
    project.write("tests/real.ox", "print \"real test\";\n");
    let output = project.command(&["test"]);
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("real test"), "{text}");
    assert!(!text.contains("data.ox"), "{text}");
}
