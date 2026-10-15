//! Root and nested dependency alias validation through the CLI, with local paths only.
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const HEADER: &str = "[project]\nname = \"test\"\nentry = \"main.ox\"\n[scripts]\nprobe = \"oxid-manifest-validation-must-not-launch\"\n";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-manifest-validation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        for dep in ["dep", "other"] {
            fs::create_dir(root.join(dep)).unwrap();
            fs::write(root.join(dep).join("value.ox"), "const value = 1;\n").unwrap();
        }
        fs::write(root.join("main.ox"), "const value = 1;\n").unwrap();
        let fixture = Self(root);
        fixture.manifest("[dependencies]\ndep = \"dep\"\n");
        fixture
    }
    fn manifest(&self, dependencies: &str) {
        fs::write(self.0.join("oxid.toml"), format!("{HEADER}{dependencies}")).unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) -> Output {
        let output = self.run(args);
        assert!(output.status.success(), "{args:?}: {output:?}");
        output
    }
    fn reject(&self, args: &[&str], expected: &str) {
        let output = self.run(args);
        assert!(!output.status.success(), "{args:?}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{args:?}: expected {expected:?}: {output:?}"
        );
    }
    fn lock(&self) -> Vec<u8> {
        fs::read(self.0.join("oxid.lock")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "preserving failed manifest-validation fixture: {}",
                self.0.display()
            );
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

// Fixtures contain regular files and directories only. Include empty directories
// so rejected commands cannot silently create cache or output directories.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn walk(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(!metadata.file_type().is_symlink());
            let value = if metadata.is_dir() {
                walk(root, &path, entries);
                None
            } else {
                Some(fs::read(&path).unwrap())
            };
            entries.insert(path.strip_prefix(root).unwrap().to_path_buf(), value);
        }
    }
    let mut entries = BTreeMap::new();
    walk(root, root, &mut entries);
    entries
}

fn root_duplicate() -> String {
    format!(
        "duplicate dependency `dep` in {}",
        Path::new(".").join("oxid.toml").display()
    )
}

#[test]
fn equivalent_alias_spellings_and_repeated_sections_are_duplicates() {
    let fixture = Fixture::new();
    for declarations in [
        "dep = \"dep\"\ndep = \"dep\"\n",
        "dep = \"dep\"\ndep = \"other\"\n",
        "dep = \"dep\"\n\"dep\" = \"other\"\n",
        "\"dep\" = \"dep\"\n\"d\\u0065p\" = \"dep\"\n",
        "\"d\\u0065p\" = \"dep\"\n[dependencies]\ndep = \"other\"\n",
    ] {
        fixture.manifest(&format!("[dependencies]\n{declarations}"));
        let before = snapshot(&fixture.0);
        for args in [&["lock"][..], &["list"]] {
            fixture.reject(args, &root_duplicate());
            assert_eq!(snapshot(&fixture.0), before, "{declarations}: {args:?}");
        }
    }
}

#[test]
fn root_rejection_preserves_missing_or_existing_lock_cache_and_outputs() {
    // Four local fixtures cover both lock states and both output/cache states.
    for existing_lock in [false, true] {
        for existing_outputs in [false, true] {
            let fixture = Fixture::new();
            if existing_lock {
                fixture.ok(&["lock", "--offline"]);
            }
            if existing_outputs {
                for path in [
                    "cache/sentinel",
                    ".oxid/deps/sentinel",
                    ".oxid/bin/test.oxb",
                    ".oxid/build-report.txt",
                ] {
                    let path = fixture.0.join(path);
                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                    fs::write(path, "preserve existing bytes\n").unwrap();
                }
            }
            fixture.manifest(
                "[dependencies]\ndep = \"dep\"\n[dependencies]\n\"d\\u0065p\" = \"other\"\n",
            );
            let before = snapshot(&fixture.0);
            for command in ["lock", "fetch", "build", "install"] {
                for flags in [
                    &[][..],
                    &["--locked"],
                    &["--offline"],
                    &["--frozen"],
                    &["--update"],
                ] {
                    let mut args = vec![command];
                    args.extend_from_slice(flags);
                    fixture.reject(&args, &root_duplicate());
                    assert_eq!(snapshot(&fixture.0), before, "{args:?}");
                }
            }
            for args in [
                &["update"][..],
                &["list"],
                &["script", "probe"],
                &["doctor"],
            ] {
                fixture.reject(args, &root_duplicate());
                assert_eq!(snapshot(&fixture.0), before, "{args:?}");
            }
        }
    }
}

#[test]
fn malformed_duplicate_value_keeps_value_error_precedence() {
    let fixture = Fixture::new();
    fixture.manifest("[dependencies]\ndep = \"dep\"\n\"dep\" = \"unterminated\n");
    let before = snapshot(&fixture.0);
    for args in [&["lock"][..], &["build"], &["script", "probe"], &["doctor"]] {
        let output = fixture.run(args);
        assert!(!output.status.success(), "{args:?}: {output:?}");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("invalid manifest value")
                && error.contains("unterminated manifest string"),
            "{args:?}: {output:?}"
        );
        assert!(
            !error.contains("duplicate dependency"),
            "{args:?}: {output:?}"
        );
        assert_eq!(snapshot(&fixture.0), before);
    }
}

#[test]
fn nested_manifest_still_rejects_duplicate_aliases() {
    let fixture = Fixture::new();
    fs::write(
        fixture.0.join("dep/oxid.toml"),
        "[dependencies]\nchild = \"../other\"\n[dependencies]\n\"ch\\u0069ld\" = \"../other\"\n",
    )
    .unwrap();
    let nested = fs::canonicalize(fixture.0.join("dep/oxid.toml")).unwrap();
    for args in [
        &["lock", "--offline"][..],
        &["fetch"],
        &["update"],
        &["build"],
        &["install"],
    ] {
        fixture.reject(
            args,
            &format!("duplicate dependency `child` in {}", nested.display()),
        );
        assert!(!fixture.0.join("oxid.lock").exists());
    }
}

#[test]
fn unique_aliases_across_sections_produce_deterministic_locks() {
    let fixture = Fixture::new();
    fixture
        .manifest("[dependencies]\n\"oth\\u0065r\" = \"other\"\n[dependencies]\ndep = \"dep\"\n");
    fixture.ok(&["lock", "--offline"]);
    let original = fixture.lock();
    let list = fixture.ok(&["list"]);
    assert_eq!(
        String::from_utf8(list.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        "dep = dep\nother = other\n"
    );
    fixture.manifest("[dependencies]\n\"dep\" = \"dep\"\nother = \"other\"\n");
    fs::remove_file(fixture.0.join("oxid.lock")).unwrap();
    fixture.ok(&["lock", "--offline"]);
    assert_eq!(fixture.lock(), original, "fresh lock after alias reorder");
    for args in [
        &["lock"][..],
        &["fetch", "--locked"],
        &["lock", "--offline"],
        &["fetch", "--frozen"],
        &["update"],
        &["build", "--frozen"],
        &["install", "--frozen"],
        &["doctor"],
    ] {
        fixture.ok(args);
        assert_eq!(fixture.lock(), original, "{args:?}");
    }
    assert!(fixture.0.join(".oxid/bin/test.oxb").is_file());
}
