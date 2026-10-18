//! Explicit typed scaffolding, separate from the default legacy template.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        for _ in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "oxid-typed-new-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("{error}"),
            }
        }
        panic!("scratch collisions");
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(out: Output) -> Vec<u8> {
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    out.stdout
}
#[test]
fn typed_new_generates_a_checkable_and_runnable_two_module_project() {
    let s = Scratch::new();
    assert_eq!(
        success(s.command(&["new", "--edition", "typed-preview", "app"])),
        b"created typed project: \"app\"\n"
    );
    let manifest = fs::read_to_string(s.0.join("app/oxid.toml")).unwrap();
    assert_eq!(manifest, "[project]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n");
    success(s.command(&["check", "--manifest-path", "app/oxid.toml"]));
    assert_eq!(
        success(s.command(&["run", "--manifest-path", "app/oxid.toml"])),
        b"7\n"
    );
    fs::write(
        s.0.join("app/src/answer.ox"),
        "pub fn value() -> i32 { return 9; }\n",
    )
    .unwrap();
    assert_eq!(
        success(s.command(&["run", "--manifest-path", "app/oxid.toml"])),
        b"9\n"
    );
    assert!(!s.0.join("app/.oxid").exists());
}
#[test]
fn selector_positions_and_separator_preserve_explicit_selection() {
    let s = Scratch::new();
    for args in [
        vec!["--edition=typed-preview", "new", "first"],
        vec!["new", "second", "--edition=typed-preview"],
        vec!["new", "--edition=typed-preview", "--", "third"],
    ] {
        success(s.command(&args));
    }
    for name in ["first", "second", "third"] {
        assert!(fs::read_to_string(s.0.join(name).join("oxid.toml"))
            .unwrap()
            .contains("edition = \"typed-preview\""));
    }
}
#[test]
fn invalid_cli_has_no_filesystem_effects() {
    let s = Scratch::new();
    for extra in [
        vec![],
        vec!["one", "two"],
        vec!["--backend=llvm", "app"],
        vec!["--engine=native", "app"],
        vec!["--entry-mode=process", "app"],
        vec!["--edition=typed-preview", "app"],
        vec!["--", ""],
    ] {
        let mut args = vec!["new", "--edition=typed-preview"];
        args.extend(extra);
        let out = s.command(&args);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(out.stdout.is_empty(), "{out:?}");
        assert!(!out.stderr.is_empty(), "{out:?}");
        assert_eq!(fs::read_dir(&s.0).unwrap().count(), 0);
    }
    let out = s.command(&[
        "new",
        "--edition=typed-preview",
        "--message-format=json",
        "app",
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stderr.is_empty());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("typed-preview new does not support --message-format=json"));
    assert_eq!(text.lines().count(), 2);
    assert_eq!(fs::read_dir(&s.0).unwrap().count(), 0);
}
#[test]
fn new_never_overwrites_a_destination_and_legacy_stays_default() {
    let s = Scratch::new();
    fs::write(s.0.join("owned"), b"keep").unwrap();
    let out = s.command(&["new", "--edition=typed-preview", "owned"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(fs::read(s.0.join("owned")).unwrap(), b"keep");
    success(s.command(&["new", "legacy"]));
    let manifest = fs::read_to_string(s.0.join("legacy/oxid.toml")).unwrap();
    assert!(!manifest.contains("typed-preview"));
    assert!(manifest.contains("[dependencies]"));
    assert!(fs::read_to_string(s.0.join("legacy/src/main.ox"))
        .unwrap()
        .contains("fun main()"));
}
