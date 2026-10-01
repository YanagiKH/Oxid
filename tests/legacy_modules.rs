//! Current legacy module behavior, including deliberately distinct route results.
//! These characterizations do not choose the future static edition's semantics.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);

impl Project {
    fn new(files: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-module-contract-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create isolated module project");
        let project = Self(root);
        for (path, source) in files {
            let file = project.path(path);
            fs::create_dir_all(file.parent().unwrap()).expect("create source directory");
            fs::write(file, source).expect("write source fixture");
        }
        project
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    fn command(&self, cwd: &Path) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(cwd)
            .env("OXID_CACHE_DIR", self.path("cache"))
            .env_remove("OXID_PATH");
        command
    }

    fn run(&self, path: &Path, cwd: &Path) -> Output {
        self.command(cwd)
            .arg("run")
            .arg(path)
            .output()
            .expect("run fixture through the public CLI")
    }

    fn compile(&self, cwd: &Path) -> Output {
        self.command(cwd)
            .arg("compile")
            .arg(self.path("main.ox"))
            .arg("-o")
            .arg(self.path("main.oxb"))
            .output()
            .expect("compile fixture through the production writer")
    }

    fn routes(&self, cwd: &Path) -> [Output; 2] {
        let source = self.run(&self.path("main.ox"), cwd);
        let compile = self.compile(cwd);
        assert!(compile.status.success(), "{compile:?}");
        assert!(compile.stderr.is_empty(), "{compile:?}");
        assert!(fs::read(self.path("main.oxb"))
            .expect("read artifact")
            .starts_with(b"OXBC"));
        [source, self.run(&self.path("main.oxb"), cwd)]
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_success(output: &Output, stdout: &str) {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(output.stdout, stdout.as_bytes());
}

fn assert_failure(output: &Output, reason: &str) -> String {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let diagnostic = String::from_utf8(output.stderr.clone()).expect("UTF-8 diagnostic");
    assert!(diagnostic.starts_with("error: "), "{diagnostic}");
    assert!(diagnostic.contains(reason), "{diagnostic}");
    diagnostic
}

#[test]
fn canonical_import_aliases_initialize_the_module_once() {
    let project = Project::new(&[
        (
            "main.ox",
            "import \"module.ox\";\nimport \"./module.ox\";\nprint \"main\";\n",
        ),
        ("module.ox", "print \"module\";\n"),
    ]);
    for output in project.routes(&project.0) {
        assert_success(&output, "module\nmain\n");
    }
}

#[test]
fn diamond_import_graph_initializes_shared_leaf_once() {
    let project = Project::new(&[
        (
            "main.ox",
            "import \"left.ox\";\nimport \"right.ox\";\nprint \"main\";\n",
        ),
        ("left.ox", "import \"leaf.ox\";\nprint \"left\";\n"),
        ("right.ox", "import \"leaf.ox\";\nprint \"right\";\n"),
        ("leaf.ox", "print \"leaf\";\n"),
    ]);
    for output in project.routes(&project.0) {
        assert_success(&output, "leaf\nleft\nright\nmain\n");
    }
}

#[test]
fn top_level_imports_resolve_from_the_importer_not_working_directory() {
    let project = Project::new(&[
        ("main.ox", "import \"nested/left.ox\";\nprint \"main\";\n"),
        (
            "nested/left.ox",
            "import \"../leaf.ox\";\nprint \"left\";\n",
        ),
        ("leaf.ox", "print \"leaf\";\n"),
        ("unrelated/leaf.ox", "print \"wrong leaf\";\n"),
    ]);
    let cwd = project.path("unrelated/cwd");
    fs::create_dir(&cwd).expect("create unrelated working directory");
    for output in project.routes(&cwd) {
        assert_success(&output, "leaf\nleft\nmain\n");
    }
}

#[test]
fn missing_module_fails_at_run_and_compile_without_writing_an_artifact() {
    let project = Project::new(&[("main.ox", "import \"missing.ox\";\n")]);
    let source = project.run(&project.path("main.ox"), &project.0);
    let source_error = assert_failure(&source, "cannot open module");
    assert!(source_error.contains("main.ox:1:1-1:"), "{source_error}");
    assert!(source_error.contains("missing.ox"), "{source_error}");

    let compile_error = assert_failure(&project.compile(&project.0), "cannot open source");
    assert!(compile_error.contains("missing.ox"), "{compile_error}");
    assert!(!project.path("main.oxb").exists());
}

#[test]
fn cyclic_imports_are_rejected_without_writing_an_artifact() {
    let project = Project::new(&[
        ("main.ox", "import \"left.ox\";\n"),
        ("left.ox", "import \"right.ox\";\n"),
        ("right.ox", "import \"left.ox\";\n"),
    ]);
    for output in [
        project.run(&project.path("main.ox"), &project.0),
        project.compile(&project.0),
    ] {
        let diagnostic = assert_failure(&output, "cyclic module import detected");
        assert!(diagnostic.contains("left.ox"), "{diagnostic}");
    }
    assert!(!project.path("main.oxb").exists());
}

#[test]
fn imported_runtime_error_keeps_the_responsible_module_line() {
    let project = Project::new(&[
        ("main.ox", "import \"module.ox\";\n"),
        ("module.ox", "// error on the next line\nprint 1 / 0;\n"),
    ]);
    for output in project.routes(&project.0) {
        let diagnostic = assert_failure(&output, "division by zero");
        assert!(diagnostic.contains("module.ox:2:1-2:"), "{diagnostic}");
    }
}

#[test]
fn preceding_importer_binding_is_available_only_in_the_source_route() {
    let project = Project::new(&[
        ("main.ox", "let answer = 42;\nimport \"module.ox\";\n"),
        ("module.ox", "print answer;\n"),
    ]);
    let [source, artifact] = project.routes(&project.0);
    assert_success(&source, "42\n");
    let diagnostic = assert_failure(&artifact, "undefined identifier: answer");
    assert!(diagnostic.contains("module.ox:1:1-1:"), "{diagnostic}");
}

#[test]
fn function_scoped_import_keeps_a_runtime_source_dependency() {
    let project = Project::new(&[
        (
            "main.ox",
            "fun main() { import \"module.ox\"; print answer; }\n",
        ),
        ("module.ox", "let answer = 42;\n"),
    ]);
    for output in project.routes(&project.0) {
        assert_success(&output, "42\n");
    }

    // Moving only the artifact demonstrates the remaining runtime dependency;
    // leave every source fixture intact so no destructive setup is needed.
    let artifact_only = project.path("artifact-only");
    fs::create_dir(&artifact_only).expect("create artifact-only directory");
    fs::copy(project.path("main.oxb"), artifact_only.join("main.oxb"))
        .expect("copy artifact without source modules");
    let output = project.run(&artifact_only.join("main.oxb"), &artifact_only);
    let diagnostic = assert_failure(&output, "cannot open module");
    assert!(diagnostic.contains("module.ox"), "{diagnostic}");
    assert!(diagnostic.contains("main.ox:1:14-1:"), "{diagnostic}");
}
