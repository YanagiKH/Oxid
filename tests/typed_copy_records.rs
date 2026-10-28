//! Explicit Copy records through the ordinary project commands.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new(source: &str, policy: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-copy-records-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        fs::write(root.join("oxid.toml"),format!("[project]\nname=\"copy-records\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[execution]\npolicy=\"{policy}\"\n")).unwrap();
        Self(root)
    }
    fn invoke(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .output()
            .unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn checked_copy_examples_run_with_both_execution_policies() {
    for policy in ["bounded", "hosted"] {
        for (source, expected) in [
            (
                include_str!("../examples/copy-records/src/main.ox"),
                b"42\n".as_slice(),
            ),
            (
                include_str!("../examples/copy-records/tests/copies.ox"),
                b"0\n".as_slice(),
            ),
        ] {
            let output = Project::new(source, policy).invoke(&["run"]);
            assert!(output.status.success(), "{policy}: {output:?}");
            assert_eq!(output.stdout, expected);
            assert!(output.stderr.is_empty(), "{output:?}");
        }
    }
}
#[test]
fn checked_copy_generics_preserve_wide_and_unicode_fields() {
    let source = include_str!("fixtures/copy_records/wide_unicode.ox");
    for policy in ["bounded", "hosted"] {
        let output = Project::new(source, policy).invoke(&["run"]);
        assert!(output.status.success(), "{policy}: {output:?}");
        assert_eq!(output.stdout, b"42\n");
        assert!(output.stderr.is_empty(), "{output:?}");
    }
}
#[test]
fn checked_copy_rejections_do_not_publish_artifacts_or_enter_the_backend() {
    for source in [
        "struct P{x:i32}fn main()->i32{let p=P{x:21};let second=p;return p.x+second.x;}",
        "#[derive(Copy)]struct P{items:[i32;1]}fn main()->i32{return 0;}",
        "use alloc::vec::Vec;#[derive(Copy)]struct P{items:Vec<i32>}fn main()->i32{return 0;}",
        "#[derive(Copy)]struct P{x:i32}fn read(p:&mut P,copy:P)->i32{return copy.x;}fn main()->i32{let mut p=P{x:21};return read(&mut p,p);}",
        "#[derive(Copy)]struct P{x:i32}use alloc::vec::new;fn main()->i32{let values=new::<P>();return 0;}",
    ]{
        let project=Project::new(source,"hosted");let output=project.invoke(&["check"]);
        assert_eq!(output.status.code(),Some(1),"{source}: {output:?}");assert!(output.stdout.is_empty(),"{output:?}");
        let error=String::from_utf8_lossy(&output.stderr);assert!(error.contains("error[E03")&&!error.contains("E0500"),"{source}: {output:?}");
        for path in [".oxid","cache","oxid.lock"]{assert!(!project.0.join(path).exists(),"{path}");}
    }
}
#[test]
fn checked_copy_formatting_preserves_the_attribute_and_behavior() {
    let project=Project::new("#[derive(Copy)]struct Point{x:i32}fn main()->i32{let p=Point{x:21};let q=p;return p.x+q.x;}","hosted");
    let output = project.invoke(&["fmt", "main.ox"]);
    assert!(output.status.success(), "{output:?}");
    let formatted = output.stdout;
    fs::write(project.0.join("main.ox"), &formatted).unwrap();
    let repeated = project.invoke(&["fmt", "main.ox"]);
    assert!(repeated.status.success(), "{repeated:?}");
    assert_eq!(repeated.stdout, formatted);
    let result = project.invoke(&["run"]);
    assert!(result.status.success(), "{result:?}");
    assert_eq!(result.stdout, b"42\n");
    assert!(result.stderr.is_empty(), "{result:?}");
}
