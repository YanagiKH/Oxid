//! Package integrity is tested through the shipped CLI, using local content only.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-package-integrity-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("dep")).unwrap();
        fs::write(root.join("dep/value.ox"), b"const value = 1;\n").unwrap();
        fs::write(
            root.join("oxid.toml"),
            "[project]\nname = \"test\"\n[dependencies]\ndep = \"dep\"\n",
        )
        .unwrap();
        Self(root)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) {
        let output = self.run(args);
        assert!(output.status.success(), "{args:?}: {output:?}");
    }
    fn lock(&self) -> Vec<u8> {
        fs::read(self.0.join("oxid.lock")).unwrap()
    }
    fn legacy(&self) {
        fs::write(self.0.join("oxid.lock"), "version = 1\n\n[[package]]\nname = \"dep\"\nsource = \"path+dep\"\nrevision = \"\"\nchecksum = \"fnv1a64:137966a53d847cbc\"\ndependencies = []\n").unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn sha256_cli_locks_verifies_and_rejects_changes_without_rewrite() {
    let fixture = Fixture::new();
    fixture.ok(&["lock", "--offline"]); // Preserve missing-lock offline behavior.
    let original = fixture.lock();
    assert!(String::from_utf8_lossy(&original)
        .contains("sha256:0e2f02766feab0246bcba853e4955a11953c6105d8ec8a48165889a7f8b1e99d"));
    for args in [&["lock"][..], &["fetch", "--locked"], &["lock", "--frozen"]] {
        fixture.ok(args);
        assert_eq!(fixture.lock(), original);
    }
    fs::write(fixture.0.join("dep/value.ox"), "const value = 2;\n").unwrap();
    for args in [
        &["lock"][..],
        &["fetch", "--locked"],
        &["lock", "--offline"],
    ] {
        let output = fixture.run(args);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("checksum mismatch"),
            "{output:?}"
        );
        assert_eq!(fixture.lock(), original);
    }
    fixture.ok(&["update"]);
    assert_ne!(fixture.lock(), original);
    fixture.ok(&["lock", "--locked"]);
}

#[test]
fn legacy_mixed_lock_and_explicit_migration_preserve_offline_behavior() {
    let fixture = Fixture::new();
    fixture.legacy();
    let original = fixture.lock();
    for args in [
        &["lock"][..],
        &["lock", "--offline"],
        &["lock", "--locked"],
        &["fetch", "--frozen"],
    ] {
        fixture.ok(args);
        assert_eq!(fixture.lock(), original);
    }
    fs::create_dir(fixture.0.join("newdep")).unwrap();
    fs::write(fixture.0.join("newdep/value"), "new").unwrap();
    fs::write(
        fixture.0.join("oxid.toml"),
        "[dependencies]\ndep = \"dep\"\nnewdep = \"newdep\"\n",
    )
    .unwrap();
    fixture.ok(&["lock", "--offline"]);
    let mixed = String::from_utf8(fixture.lock()).unwrap();
    assert!(mixed.contains("fnv1a64:137966a53d847cbc"));
    assert!(mixed.contains("sha256:"));
    fixture.ok(&["lock", "--frozen"]);
    fs::write(
        fixture.0.join("oxid.toml"),
        "[dependencies]\ndep = \"dep\"\n",
    )
    .unwrap();
    fixture.ok(&["lock", "--offline"]);
    assert_eq!(fixture.lock(), original);
    fs::write(fixture.0.join("dep/value.ox"), "changed intentionally").unwrap();
    fixture.ok(&["update"]);
    let upgraded = String::from_utf8(fixture.lock()).unwrap();
    assert!(upgraded.contains("sha256:"));
    assert!(!upgraded.contains("fnv1a64:"));
    fixture.ok(&["lock", "--locked"]);
}

#[test]
fn malformed_digest_and_invalid_modes_fail_without_lock_writes() {
    let fixture = Fixture::new();
    assert!(!fixture.run(&["lock", "--locked"]).status.success());
    assert!(!fixture.0.join("oxid.lock").exists());
    fixture.legacy();
    let original = fixture.lock();
    for args in [
        &["lock", "--locked", "--update"][..],
        &["lock", "--offline", "--update"],
    ] {
        assert!(!fixture.run(args).status.success());
        assert_eq!(fixture.lock(), original);
    }
    for invalid in ["unknown:abcd", "sha256:abcd", "sha256:gggg"] {
        let text = String::from_utf8(original.clone())
            .unwrap()
            .replace("fnv1a64:137966a53d847cbc", invalid);
        fs::write(fixture.0.join("oxid.lock"), &text).unwrap();
        for args in [&["lock"][..], &["update"]] {
            assert!(!fixture.run(args).status.success());
            assert_eq!(fixture.lock(), text.as_bytes());
        }
    }
}

#[cfg(unix)]
#[test]
fn rejected_included_paths_do_not_rewrite_sha_or_legacy_locks() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    fixture.ok(&["lock"]);
    let original = fixture.lock();
    symlink("value.ox", fixture.0.join("dep/link")).unwrap();
    for args in [&["lock"][..], &["update"]] {
        let output = fixture.run(args);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("symbolic links"));
        assert_eq!(fixture.lock(), original);
    }
    fs::remove_file(fixture.0.join("dep/link")).unwrap();
    fixture.legacy();
    let legacy = fixture.lock();
    fs::write(fixture.0.join("dep/a\\b"), "ambiguous").unwrap();
    let output = fixture.run(&["update"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("rename"));
    assert_eq!(fixture.lock(), legacy);
}
