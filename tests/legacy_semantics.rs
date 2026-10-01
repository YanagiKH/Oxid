//! Characterization of the existing 0.9 interpreter, not the proposed static core.
//! Every case runs as source and again through the production OXBC writer/reader.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);

impl Project {
    fn new(source: &str) -> Self {
        let sequence = NEXT_PROJECT.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("oxid-legacy-{}-{}", std::process::id(), sequence));
        fs::create_dir(&root).expect("create isolated legacy test project");
        fs::write(root.join("main.ox"), source).expect("write legacy source");
        Self(root)
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

    fn source_and_artifact(&self) -> [Output; 2] {
        let source = self.command(&["run", "main.ox"]);
        let compile = self.command(&["compile", "main.ox", "-o", "main.oxb"]);
        assert!(
            compile.status.success(),
            "artifact compilation failed: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        assert!(fs::read(self.0.join("main.oxb"))
            .expect("read OXBC")
            .starts_with(b"OXBC"));
        [source, self.command(&["run", "main.oxb"])]
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_stdout(source: &str, expected: &str) {
    let project = Project::new(source);
    for (route, output) in ["source", "OXBC"]
        .into_iter()
        .zip(project.source_and_artifact())
    {
        assert!(
            output.status.success(),
            "{route} execution failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            expected,
            "{route}"
        );
        assert!(output.stderr.is_empty(), "unexpected {route} diagnostics");
    }
}

#[test]
fn legacy_f64_arithmetic_and_epsilon_equality() {
    assert_stdout(
        r#"
print 5 / 2;
print 9007199254740992 + 1 == 9007199254740992;
print 0.1 + 0.2 == 0.3;
print 0 == 0.0000000000000001;
print 0 == 0.0000000000000002220446049250313;
print 0 < 0.0000000000000001;
print -5 % 2;
print true == 1;
"#,
        "2.5\ntrue\ntrue\ntrue\nfalse\ntrue\n-1\nfalse\n",
    );
}

#[test]
fn legacy_container_assignment_and_parameters_share_storage() {
    assert_stdout(
        r#"
fun change(items, record) {
    items[0] = 7;
    record.inner.value = 9;
}
const items = [1, 2];
let alias = items;
const record = {inner: {value: 3}};
let record_alias = record;
change(alias, record_alias);
print items[0];
print record.inner.value;
print items == [7, 2];
print record == {inner: {value: 9}};
print json_stringify({z: items, a: record.inner});
"#,
        "7\n9\ntrue\ntrue\n{\"a\":{\"value\":9},\"z\":[7,2]}\n",
    );
}

#[test]
fn legacy_logical_operators_short_circuit_and_return_operands() {
    assert_stdout(
        r#"
let calls = 0;
fun bump() { calls = calls + 1; return 42; }
print 0 and bump();
print "" or "fallback";
print [] or "empty array";
print {} or "empty record";
print 5 or bump();
print 1 and bump();
print calls;
print !null;
print ![0];
"#,
        "0\nfallback\nempty array\nempty record\n5\n42\n1\ntrue\nfalse\n",
    );
}

#[test]
fn legacy_evaluation_order_is_left_to_right() {
    assert_stdout(
        r#"
let trace = "";
fun mark(label, value) { trace = trace + label; return value; }
fun combine(left, right) { return left + right; }
fun callee() { trace = trace + "C"; return combine; }
print callee()(mark("L", 10), mark("R", 20));
let array = [mark("A", 1), mark("B", 2)];
let record = {z: mark("Z", 3), a: mark("Y", 4)};
print mark("D", 5) + mark("E", 6);
print trace;
print json_stringify(record);
"#,
        "30\n11\nCLRABZYDE\n{\"a\":4,\"z\":3}\n",
    );
}

#[test]
fn legacy_tasks_are_lazy_ordered_and_memoized() {
    assert_stdout(
        r#"
let trace = "";
async fun compute(label) { trace = trace + label; return label; }
let first = compute("A");
let second = spawn(compute, "B");
print task_status(first);
print len(trace);
print json_stringify(join_all([second, first]));
print trace;
print await first;
print trace;
print task_status(first);
"#,
        "pending\n0\n[\"B\",\"A\"]\nBA\nA\nBA\ncompleted\n",
    );
}

#[test]
fn legacy_runtime_failures_preserve_reason_and_source_location() {
    for (statement, reason) in [
        ("print 1 / 0;", "division by zero"),
        ("print 1 % 0;", "modulo by zero"),
        ("print [1][-1];", "index must be a non-negative integer"),
        ("print [1][1];", "array index 1 is out of bounds"),
        ("answer = 2;", "cannot reassign constant `answer`"),
    ] {
        let project = Project::new(&format!("const answer = 1;\n{statement}\n"));
        for output in project.source_and_artifact() {
            assert_eq!(output.status.code(), Some(1), "{statement}");
            assert!(output.stdout.is_empty(), "{statement}");
            let diagnostic = String::from_utf8(output.stderr).unwrap();
            assert!(diagnostic.starts_with("error: "), "{diagnostic}");
            assert!(diagnostic.contains("main.ox:2:1-2:"), "{diagnostic}");
            assert!(diagnostic.contains(reason), "{diagnostic}");
        }
    }
}

#[test]
fn legacy_module_initialization_runs_once_before_entry() {
    let project = Project::new(
        "import \"module.ox\";\nimport \"module.ox\";\nfun main() { print \"entry\"; }\n",
    );
    fs::write(
        project.0.join(Path::new("module.ox")),
        "print \"module\";\n",
    )
    .expect("write module");
    for output in project.source_and_artifact() {
        assert!(output.status.success(), "{:?}", output);
        assert_eq!(output.stdout, b"module\nentry\n");
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn legacy_module_initialization_order_differs_between_source_and_artifact() {
    let project = Project::new("print \"before\";\nimport \"module.ox\";\nprint \"after\";\n");
    fs::write(project.0.join("module.ox"), "print \"module\";\n")
        .expect("write module with an observable initialization effect");

    // Freeze the existing discrepancy rather than silently making either route
    // the semantic oracle for the future static core or changing legacy behavior.
    let [source, artifact] = project.source_and_artifact();
    for output in [&source, &artifact] {
        assert!(output.status.success(), "{:?}", output);
        assert!(output.stderr.is_empty());
    }
    assert_eq!(source.stdout, b"before\nmodule\nafter\n");
    assert_eq!(artifact.stdout, b"module\nbefore\nafter\n");
}
