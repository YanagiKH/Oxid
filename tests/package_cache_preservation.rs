//! Existing Git caches are user data: rejected resolution must not mutate them.
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    origin: PathBuf,
    first: String,
    second: String,
}
fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-cache-preservation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let origin = root.join("origin");
        fs::create_dir_all(&origin).unwrap();
        git(&origin, &["init", "--quiet"]);
        git(&origin, &["config", "user.name", "Oxid Test"]);
        git(&origin, &["config", "user.email", "oxid@example.invalid"]);
        git(&origin, &["config", "core.autocrlf", "false"]);
        fs::write(origin.join("value.ox"), "const value = 1;\n").unwrap();
        fs::write(origin.join(".gitignore"), "ignored\nignored-dir/\n").unwrap();
        git(&origin, &["add", "."]);
        git(&origin, &["commit", "--quiet", "-m", "first"]);
        let first = git(&origin, &["rev-parse", "HEAD"]);
        let mut fixture = Self {
            root,
            origin,
            first,
            second: String::new(),
        };
        fixture.manifest(&fixture.first);
        fixture.ok(&["lock"]);
        fs::write(fixture.origin.join("value.ox"), "const value = 2;\n").unwrap();
        fs::write(fixture.origin.join("untracked"), "upstream\n").unwrap();
        fs::write(fixture.origin.join("ignored"), "upstream\n").unwrap();
        fs::create_dir(fixture.origin.join("ignored-dir")).unwrap();
        fs::write(fixture.origin.join("ignored-dir/value"), "upstream\n").unwrap();
        git(&fixture.origin, &["add", "--force", "."]);
        git(&fixture.origin, &["commit", "--quiet", "-m", "second"]);
        fixture.second = git(&fixture.origin, &["rev-parse", "HEAD"]);
        fixture
    }
    fn cache(&self) -> PathBuf {
        self.root.join(".oxid/deps/dep")
    }
    fn manifest(&self, revision: &str) {
        let path = fs::canonicalize(&self.origin)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('\\', "/");
        let path = path.strip_prefix("//?/").unwrap_or(&path);
        let url = if cfg!(windows) {
            format!("file:///{}", path.trim_start_matches('/'))
        } else {
            format!("file://{path}")
        };
        fs::write(
            self.root.join("oxid.toml"),
            format!("[dependencies]\ndep = \"git+{url}#{revision}\"\n"),
        )
        .unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.root)
            .env("OXID_CACHE_DIR", self.root.join("cache"))
            .env("GIT_TRACE", self.root.join("git-trace"))
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) {
        let output = self.run(args);
        assert!(output.status.success(), "{args:?}: {output:?}");
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, path: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}
#[test]
fn rejects_dirty_caches_before_any_mutation_in_every_resolution_mode() {
    for change in [
        "modified",
        "staged",
        "deleted",
        "staged-deleted",
        "untracked",
        "ignored",
        "ignored-dir",
    ] {
        let fixture = Fixture::new();
        let cache = fixture.cache();
        // A rejected attempt must not rewrite this setting or refresh the index.
        git(&cache, &["config", "core.autocrlf", "true"]);
        match change {
            "modified" | "staged" => {
                fs::write(cache.join("value.ox"), "local edits\n").unwrap();
                if change == "staged" {
                    git(&cache, &["add", "value.ox"]);
                }
            }
            "deleted" => fs::remove_file(cache.join("value.ox")).unwrap(),
            "staged-deleted" => {
                git(&cache, &["rm", "value.ox"]);
            }
            "ignored-dir" => {
                fs::create_dir(cache.join(change)).unwrap();
                fs::write(cache.join("ignored-dir/value"), "local edits\n").unwrap();
            }
            _ => fs::write(cache.join(change), "local edits\n").unwrap(),
        }
        let before = snapshot(&cache);
        let lock = fs::read(fixture.root.join("oxid.lock")).unwrap();
        for args in [
            &["lock"][..],
            &["fetch", "--locked"],
            &["lock", "--offline"],
            &["fetch", "--frozen"],
            &["update"],
        ] {
            fs::write(fixture.root.join("git-trace"), "").unwrap();
            let output = fixture.run(args);
            assert!(!output.status.success(), "{change} {args:?}: {output:?}");
            assert!(!fs::read_to_string(fixture.root.join("git-trace"))
                .unwrap()
                .contains("built-in: git fetch "));
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("contains modified, untracked or ignored files"),
                "{change} {args:?}: {output:?}"
            );
            assert_eq!(snapshot(&cache), before, "cache changed: {change} {args:?}");
            assert_eq!(fs::read(fixture.root.join("oxid.lock")).unwrap(), lock);
        }
        // The next revision would overwrite each additional path. It must not
        // even be fetched into the cache when local data is present.
        fixture.manifest(&fixture.second);
        fs::write(fixture.root.join("git-trace"), "").unwrap();
        let output = fixture.run(&["update"]);
        assert!(!output.status.success(), "{change}: {output:?}");
        assert!(!fs::read_to_string(fixture.root.join("git-trace"))
            .unwrap()
            .contains("built-in: git fetch "));
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("contains modified, untracked or ignored files"),
            "{output:?}"
        );
        assert_eq!(snapshot(&cache), before, "updated dirty cache: {change}");
        assert_eq!(fs::read(fixture.root.join("oxid.lock")).unwrap(), lock);
    }
}
#[test]
fn clean_cache_allows_cached_resolution_and_explicit_revision_update() {
    let fixture = Fixture::new();
    let lock = fs::read(fixture.root.join("oxid.lock")).unwrap();
    for args in [
        &["lock"][..],
        &["fetch", "--locked"],
        &["lock", "--offline"],
        &["fetch", "--frozen"],
    ] {
        fixture.ok(args);
        assert_eq!(fs::read(fixture.root.join("oxid.lock")).unwrap(), lock);
    }
    fixture.manifest(&fixture.second);
    fs::write(fixture.root.join("git-trace"), "").unwrap();
    fixture.ok(&["update"]);
    assert!(fs::read_to_string(fixture.root.join("git-trace"))
        .unwrap()
        .contains("built-in: git fetch "));
    assert_eq!(
        git(&fixture.cache(), &["rev-parse", "HEAD"]),
        fixture.second
    );
    assert_eq!(
        fs::read(fixture.cache().join("ignored")).unwrap(),
        b"upstream\n"
    );
    assert_ne!(fs::read(fixture.root.join("oxid.lock")).unwrap(), lock);
    fixture.ok(&["lock", "--frozen"]);
}

#[test]
fn index_flags_are_rejected_without_changing_hidden_edits_or_clean_flags() {
    for flag in ["--assume-unchanged", "--skip-worktree"] {
        for modified in [true, false] {
            let fixture = Fixture::new();
            let cache = fixture.cache();
            git(&cache, &["update-index", flag, "value.ox"]);
            if modified {
                fs::write(cache.join("value.ox"), "hidden local edits\n").unwrap();
            }
            assert_eq!(git(&cache, &["status", "--porcelain"]), "");
            fixture.manifest(&fixture.second);
            let before = snapshot(&cache);
            let lock = fs::read(fixture.root.join("oxid.lock")).unwrap();
            fs::write(fixture.root.join("git-trace"), "").unwrap();
            let output = fixture.run(&["update"]);
            assert!(
                !output.status.success(),
                "{flag} modified={modified}: {output:?}"
            );
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("unsupported assume-unchanged or skip-worktree"),
                "{output:?}"
            );
            assert_eq!(snapshot(&cache), before);
            assert_eq!(fs::read(fixture.root.join("oxid.lock")).unwrap(), lock);
            assert!(!fs::read_to_string(fixture.root.join("git-trace"))
                .unwrap()
                .contains("built-in: git fetch "));
        }
    }
}

#[test]
fn unsupported_submodule_caches_are_rejected_before_mutation_even_without_metadata() {
    for metadata in [true, false] {
        let fixture = Fixture::new();
        let cache = fixture.cache();
        git(&cache, &["config", "user.name", "Oxid Test"]);
        git(&cache, &["config", "user.email", "oxid@example.invalid"]);
        git(
            &cache,
            &[
                "clone",
                "--quiet",
                fixture.origin.to_str().unwrap(),
                "nested",
            ],
        );
        git(
            &cache.join("nested"),
            &["checkout", "--quiet", &fixture.first],
        );
        fs::write(cache.join("nested/ignored"), "local ignored data\n").unwrap();
        git(
            &cache,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("160000,{},nested", fixture.first),
            ],
        );
        if metadata {
            fs::write(
                cache.join(".gitmodules"),
                "[submodule \"nested\"]\n\tpath = nested\n\turl = ../origin\n",
            )
            .unwrap();
            git(&cache, &["add", ".gitmodules"]);
        }
        git(&cache, &["commit", "--quiet", "-m", "local submodule"]);
        assert_eq!(
            git(
                &cache,
                &[
                    "status",
                    "--porcelain",
                    "--ignored",
                    "--ignore-submodules=none"
                ]
            ),
            ""
        );
        git(&cache, &["config", "core.autocrlf", "true"]);
        let before = snapshot(&cache);
        let lock = fs::read(fixture.root.join("oxid.lock")).unwrap();
        fs::write(fixture.root.join("git-trace"), "").unwrap();
        let output = fixture.run(&["update"]);
        assert!(!output.status.success(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("unsupported submodules"),
            "{output:?}"
        );
        assert_eq!(snapshot(&cache), before);
        assert_eq!(fs::read(fixture.root.join("oxid.lock")).unwrap(), lock);
        assert!(!fs::read_to_string(fixture.root.join("git-trace"))
            .unwrap()
            .contains("built-in: git fetch "));
    }
}
