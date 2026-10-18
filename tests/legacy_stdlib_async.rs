//! Coverage of preexisting legacy task behavior through the actual stdlib wrapper.
//! This does not establish concurrent scheduling or typed-preview semantics.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

struct Project(PathBuf);

impl Project {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!("oxid-stdlib-async-{}", std::process::id()));
        fs::create_dir(&root).expect("create isolated async project");
        let project = Self(root);
        fs::create_dir(project.0.join("stdlib")).expect("create stdlib directory");
        // The checkout's async module currently has no imported dependencies.
        fs::write(
            project.0.join("stdlib/async.ox"),
            include_str!("../stdlib/async.ox"),
        )
        .expect("write actual async module");
        fs::write(project.0.join("main.ox"), source).expect("write async fixture");
        project
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
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_stdout(output: &Output, expected: &str) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn await_all_is_lazy_ordered_and_memoizes_shared_tasks_on_both_routes() {
    // docs/ASYNC.md and spec/legacy-0.9.md characterize this existing behavior.
    let project = Project::new(
        r#"use "stdlib/async.ox";
let trace = "";
fn compute(label) { trace = trace + label; return label; }
let first = spawn_task(compute, "A");
let second = spawn_task(compute, "B");
print status(first);
print len(trace);
print json_stringify(await_all([second, first, second]));
print trace;
print json_stringify(await_all([first, second]));
print trace;
print status(first);
print status(second);
"#,
    );
    let expected = "pending\n0\n[\"B\",\"A\",\"B\"]\nBA\n[\"A\",\"B\"]\nBA\ncompleted\ncompleted\n";

    assert_stdout(&project.command(&["run", "main.ox"]), expected);
    let compile = project.command(&["compile", "main.ox", "-o", "main.oxb"]);
    assert_eq!(compile.status.code(), Some(0), "{compile:?}");
    assert!(compile.stderr.is_empty(), "{compile:?}");
    assert!(fs::read(project.0.join("main.oxb"))
        .expect("read compiled artifact")
        .starts_with(b"OXBC"));
    assert_stdout(&project.command(&["run", "main.oxb"]), expected);
}
