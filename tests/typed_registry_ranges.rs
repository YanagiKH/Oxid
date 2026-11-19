//! Lockfile selection of compatible releases through the normal project commands.
#![cfg(target_os = "linux")]

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "oxid-registry-ranges-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(fs::canonicalize(root).unwrap())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn oxid(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oxid"))
        .args(args)
        .current_dir(root)
        .env_remove("OXID_PATH")
        .output()
        .unwrap()
}

fn success(root: &Path, args: &[&str]) -> Output {
    let output = oxid(root, args);
    assert!(
        output.status.success() && output.stderr.is_empty(),
        "{args:?}: {output:?}"
    );
    output
}

fn publish(root: &Path, uri: &str, version: &str, value: i32) {
    let library = root.join(format!("numbers-{version}"));
    fs::create_dir_all(library.join("src")).unwrap();
    fs::write(
        library.join("oxid.toml"),
        format!("[project]\nname=\"numbers\"\nversion=\"{version}\"\nedition=\"typed-preview\"\nkind=\"lib\"\nentry=\"src/lib.ox\"\n"),
    )
    .unwrap();
    fs::write(
        library.join("src/lib.ox"),
        format!("pub fn value()->i32 {{ return {value}; }}\n"),
    )
    .unwrap();
    success(&library, &["lock"]);
    success(&library, &["publish", "--registry", uri]);
}

#[test]
fn compatible_registry_version_updates_explicitly_and_runs_offline() {
    let project = Fixture::new();
    let root = &project.0;
    let registry = root.join("registry");
    success(root, &["registry", "init", registry.to_str().unwrap()]);
    let uri = format!("file://{}", registry.display());
    publish(root, &uri, "1.2.9", 9);
    publish(root, &uri, "2.0.0", 200);
    let bridge = root.join("bridge");
    fs::create_dir_all(bridge.join("src")).unwrap();
    fs::write(
        bridge.join("oxid.toml"),
        format!("[project]\nname=\"bridge\"\nversion=\"1.0.0\"\nedition=\"typed-preview\"\nkind=\"lib\"\nentry=\"src/lib.ox\"\n[dependencies]\nnumbers={{registry=\"{uri}\",package=\"numbers\",version=\"^1.2.0\"}}\n"),
    ).unwrap();
    fs::write(
        bridge.join("src/lib.ox"),
        "pub fn value()->i32 { return crate::numbers::value(); }\n",
    )
    .unwrap();
    success(&bridge, &["lock"]);
    success(&bridge, &["publish", "--registry", &uri]);

    let app = root.join("app");
    fs::create_dir_all(app.join("src")).unwrap();
    let manifest = format!("[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n[dependencies]\ncompatible={{registry=\"{uri}\",package=\"numbers\",version=\"^1.2.0\"}}\nexact={{registry=\"{uri}\",package=\"numbers\",version=\"2.0.0\"}}\nbridge={{registry=\"{uri}\",package=\"bridge\",version=\"1.0.0\"}}\n");
    fs::write(app.join("oxid.toml"), &manifest).unwrap();
    fs::write(
        app.join("src/main.ox"),
        "fn main()->i32 { return crate::compatible::value()+crate::exact::value()+crate::bridge::value(); }\n",
    )
    .unwrap();

    success(&app, &["lock"]);
    let lock = app.join("oxid.lock");
    let initial = fs::read_to_string(&lock).unwrap();
    assert!(initial.contains("version = \"1.2.9\""));
    assert!(initial.contains("version = \"2.0.0\""));
    assert_eq!(success(&app, &["run", "--locked"]).stdout, b"218\n");

    publish(root, &uri, "1.10.1", 101);
    assert_eq!(success(&app, &["run"]).stdout, b"218\n");
    assert_eq!(fs::read_to_string(&lock).unwrap(), initial);
    success(&app, &["lock", "--update"]);
    let updated = fs::read_to_string(&lock).unwrap();
    assert!(updated.contains("version = \"1.10.1\""));
    assert!(
        updated.contains("version = \"1.2.9\""),
        "published bridge must retain its resolved dependency"
    );
    assert!(updated.contains("version = \"2.0.0\""));

    fs::rename(&registry, root.join("offline-registry")).unwrap();
    assert_eq!(success(&app, &["run", "--frozen"]).stdout, b"310\n");
    assert_eq!(fs::read_to_string(&lock).unwrap(), updated);

    fs::write(app.join("oxid.toml"), manifest.replace("^1.2.0", "^1.11.0")).unwrap();
    let stale = oxid(&app, &["run", "--frozen"]);
    assert!(
        !stale.status.success() && stale.stdout.is_empty(),
        "{stale:?}"
    );
    assert!(
        String::from_utf8_lossy(&stale.stderr).contains("matching locked release"),
        "{stale:?}"
    );
    assert_eq!(fs::read_to_string(&lock).unwrap(), updated);
}
