#![cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "oxid-test-filter-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        let project = Self(root);
        project.write(
            "app/oxid.toml",
            concat!(
                "[project]\nname=\"app\"\nedition=\"typed-preview\"\n",
                "[dependencies]\nmath=\"../math\"\n",
                "[tests]\nsmoke=\"tests/smoke.ox\"\nother=\"tests/other.ox\"\n",
            ),
        );
        project.write("app/src/main.ox", "fn main()->i32{return 0;}\n");
        project.write(
            "app/tests/smoke.ox",
            "use crate::math::answer;fn main()->i32{if answer()==42{return 0;}else{return 1;}}\n",
        );
        project.write("app/tests/other.ox", "fn incomplete(");
        project.write(
            "math/oxid.toml",
            "[project]\nname=\"math\"\nedition=\"typed-preview\"\nkind=\"lib\"\n",
        );
        project.write("math/src/lib.ox", "pub fn answer()->i32{return 42;}\n");
        project
    }

    fn write(&self, path: &str, bytes: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn run(&self, directory: &str, args: &[&str], status: i32) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(self.0.join(directory))
            .args(args)
            .env_remove("OXID_PATH")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(status), "{args:?}: {output:?}");
        String::from_utf8([output.stdout, output.stderr].concat()).unwrap()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn project_filter_keeps_lock_verification_and_runs_only_the_matching_test() {
    let project = Project::new();
    project.run("app", &["lock"], 0);
    let text = project.run(
        "app",
        &[
            "test",
            "--filter",
            "mok",
            "--jobs=2",
            "--frozen",
            "--message-format=json",
        ],
        0,
    );
    assert!(
        text.contains("\"name\":\"smoke\"") && text.contains("\"passed\":1"),
        "{text}"
    );
    assert_eq!(
        text.matches("\"kind\":\"test-result\"").count(),
        1,
        "{text}"
    );
    assert!(project
        .run("app", &["test", "--filter=absent"], 1)
        .contains("no declared test matches"));
    for args in [
        &["test", "--filter"][..],
        &["test", "--filter="],
        &["test", "--filter=smoke", "--filter=other"],
    ] {
        assert!(project
            .run("app", args, 1)
            .contains("--filter requires one nonempty test name"));
    }
    project.write("math/src/lib.ox", "pub fn answer()->i32{return 99;}\n");
    assert!(project
        .run("app", &["test", "--filter=smoke", "--frozen"], 1)
        .contains("lock is out of date"));
}

#[test]
fn qualified_workspace_filter_excludes_other_members_and_preserves_the_lock() {
    let project = Project::new();
    project.write(
        "oxid.toml",
        "[workspace]\nedition=\"typed-preview\"\nmembers=[\"app\",\"other\",\"math\"]\n",
    );
    project.write(
        "other/oxid.toml",
        "[project]\nname=\"other\"\nedition=\"typed-preview\"\n[tests]\nsmoke=\"tests/smoke.ox\"\n",
    );
    project.write("other/src/main.ox", "fn main()->i32{return 0;}\n");
    project.write("other/tests/smoke.ox", "fn incomplete(");
    project.run("", &["lock"], 0);
    let path = project.0.join("oxid.lock");
    let lock = fs::read(&path).unwrap();
    let text = project.run(
        "",
        &[
            "test",
            "--workspace",
            "--filter=app::smoke",
            "--frozen",
            "--message-format=json",
        ],
        0,
    );
    assert!(
        text.contains("\"member\":\"app\"") && text.contains("\"total\":1"),
        "{text}"
    );
    assert_eq!(
        text.matches("\"kind\":\"test-result\"").count(),
        1,
        "{text}"
    );
    assert_eq!(fs::read(path).unwrap(), lock);
    assert!(project
        .run("", &["test", "-p", "other", "--filter=app::smoke"], 1)
        .contains("no declared test matches"));
}
