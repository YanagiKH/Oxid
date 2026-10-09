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
        Self::with_suffix("")
    }
    fn with_suffix(suffix: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-cache-admission-{}-{}{suffix}",
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
#[derive(Debug, Eq, PartialEq)]
enum Entry {
    Directory,
    File(Vec<u8>),
    Link(PathBuf),
}
fn redirected(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
// Never traverse links, including junctions; include empty directories.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Entry> {
    fn walk(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, Entry>) {
        let meta = fs::symlink_metadata(path).unwrap();
        let key = path.strip_prefix(root).unwrap().to_path_buf();
        if redirected(&meta) {
            out.insert(key, Entry::Link(fs::read_link(path).unwrap()));
        } else if meta.is_dir() {
            out.insert(key, Entry::Directory);
            for entry in fs::read_dir(path).unwrap() {
                walk(root, &entry.unwrap().path(), out);
            }
        } else {
            assert!(meta.is_file());
            out.insert(key, Entry::File(fs::read(path).unwrap()));
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}
fn directory_alias(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        let output = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Windows junction capability required: {output:?}"
        );
        assert!(redirected(&fs::symlink_metadata(link).unwrap()));
    }
}
fn rejected(f: &Fixture, watched: &[&Path]) {
    let before: Vec<_> = watched.iter().map(|p| snapshot(p)).collect();
    let lock = fs::read(f.root.join("oxid.lock")).unwrap();
    for args in [
        &["lock"][..],
        &["fetch", "--locked"],
        &["lock", "--offline"],
        &["fetch", "--frozen"],
        &["update"],
    ] {
        fs::write(f.root.join("git-trace"), "").unwrap();
        let output = f.run(args);
        for (path, expected) in watched.iter().zip(&before) {
            assert!(
                snapshot(path) == *expected,
                "refused resolution mutated {}",
                path.display()
            );
        }
        assert!(
            !output.status.success(),
            "unexpected admission {args:?}: {output:?}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("dependency cache location"),
            "wrong refusal {args:?}: {output:?}"
        );
        assert_eq!(fs::read(f.root.join("oxid.lock")).unwrap(), lock);
        let trace = fs::read_to_string(f.root.join("git-trace")).unwrap();
        for mutation in [
            "built-in: git init",
            "built-in: git config",
            "built-in: git fetch",
            "built-in: git checkout",
        ] {
            assert!(
                !trace.contains(mutation),
                "mutation before admission: {trace}"
            );
        }
    }
}
#[test]
fn redirected_cache_boundaries_preserve_external_targets_in_all_modes() {
    for boundary in [
        ".oxid",
        ".oxid/deps",
        ".oxid/deps/dep",
        ".oxid/deps/dep/.git",
    ] {
        let f = Fixture::new();
        let original = f.root.join(boundary);
        let external = f.root.join("external");
        fs::rename(&original, &external).unwrap();
        directory_alias(&external, &original);
        f.manifest(&f.second);
        rejected(&f, &[&external, &f.root.join(".oxid")]);
    }
}
#[test]
fn dangling_boundaries_and_regular_files_are_occupied() {
    for boundary in [
        ".oxid",
        ".oxid/deps",
        ".oxid/deps/dep",
        ".oxid/deps/dep/.git",
    ] {
        for link in [true, false] {
            let f = Fixture::new();
            let original = f.root.join(boundary);
            let saved = f.root.join("saved");
            fs::rename(&original, &saved).unwrap();
            let missing = f.root.join("must-not-exist");
            if link {
                directory_alias(&missing, &original);
            } else {
                fs::write(&original, b"sentinel").unwrap();
            }
            rejected(&f, &[&saved, &f.root.join(".oxid")]);
            assert!(!missing.exists());
        }
    }
}
#[test]
fn gitfiles_and_enclosing_repositories_are_not_cache_roots() {
    for gitfile in [true, false] {
        let f = Fixture::new();
        let cache = f.cache();
        let external = f.root.join("external-git");
        fs::rename(cache.join(".git"), &external).unwrap();
        if gitfile {
            fs::write(
                cache.join(".git"),
                format!("gitdir: {}\n", external.display()),
            )
            .unwrap();
        } else {
            git(&f.root, &["init", "--quiet"]);
            git(&f.root, &["config", "user.name", "Oxid Test"]);
            git(&f.root, &["config", "user.email", "oxid@example.invalid"]);
            let origin = git(&external, &["config", "--get", "remote.origin.url"]);
            git(&f.root, &["remote", "add", "origin", &origin]);
            fs::write(f.root.join(".gitignore"), "*\n").unwrap();
            git(
                &f.root,
                &["commit", "--allow-empty", "--quiet", "-m", "enclosing"],
            );
        }
        let mut watched = vec![external.as_path(), cache.as_path()];
        let enclosing = f.root.join(".git");
        if !gitfile {
            watched.push(&enclosing);
        }
        rejected(&f, &watched);
    }
}
#[test]
fn core_worktree_redirection_is_rejected_before_writes() {
    let f = Fixture::new();
    let external = f.root.join("external-worktree");
    git(
        &f.root,
        &[
            "clone",
            "--quiet",
            f.cache().to_str().unwrap(),
            external.to_str().unwrap(),
        ],
    );
    git(
        &f.cache(),
        &["config", "core.worktree", external.to_str().unwrap()],
    );
    rejected(&f, &[&external, &f.cache()]);
}
#[test]
fn ordinary_cache_and_external_path_dependencies_remain_supported() {
    let f = Fixture::new();
    for args in [
        &["lock"][..],
        &["fetch", "--locked"],
        &["lock", "--offline"],
        &["fetch", "--frozen"],
    ] {
        f.ok(args);
    }
    f.manifest(&f.second);
    f.ok(&["update"]);
    assert_eq!(git(&f.cache(), &["rev-parse", "HEAD"]), f.second);
    let project = f.root.join("project");
    fs::create_dir(&project).unwrap();
    for path in [
        "../origin".to_owned(),
        f.origin.to_str().unwrap().replace('\\', "/"),
    ] {
        fs::write(
            project.join("oxid.toml"),
            format!("[dependencies]\nshared = \"{path}\"\n"),
        )
        .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .arg("update")
            .current_dir(&project)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(!project.join(".oxid").exists());
    }
    let alias = f.root.join("project-alias");
    directory_alias(&project, &alias);
    let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
        .arg("lock")
        .arg("--frozen")
        .current_dir(&alias)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn shared_git_metadata_and_linked_worktrees_are_rejected() {
    for linked in [true, false] {
        let f = Fixture::new();
        let cache = f.cache();
        let saved = f.root.join("saved-cache");
        fs::rename(&cache, &saved).unwrap();
        if linked {
            git(
                &f.origin,
                &[
                    "worktree",
                    "add",
                    "--quiet",
                    "--detach",
                    cache.to_str().unwrap(),
                    &f.first,
                ],
            );
        } else {
            fs::create_dir(&cache).unwrap();
            fs::create_dir(cache.join(".git")).unwrap();
            fs::copy(saved.join(".git/HEAD"), cache.join(".git/HEAD")).unwrap();
            fs::write(
                cache.join(".git/commondir"),
                format!("{}\n", saved.join(".git").display()),
            )
            .unwrap();
            assert_eq!(git(&cache, &["rev-parse", "--is-inside-work-tree"]), "true");
        }
        rejected(&f, &[&f.origin, &saved, &cache]);
    }
}
#[test]
fn project_alias_with_git_cache_is_supported() {
    let f = Fixture::new();
    let alias = f.root.join("project-alias");
    directory_alias(&f.root, &alias);
    let output = Command::new(env!("CARGO_BIN_EXE_oxid"))
        .args(["fetch", "--frozen"])
        .current_dir(&alias)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}
#[test]
fn git_cache_in_project_with_spaces_is_supported() {
    #[cfg(unix)]
    let suffix = " with trailing spaces ";
    #[cfg(not(unix))]
    let suffix = " with spaces";
    let f = Fixture::with_suffix(suffix);
    f.ok(&["fetch", "--frozen"]);
}

#[test]
fn redirected_empty_parent_is_rejected_before_creating_descendants() {
    for boundary in [".oxid", ".oxid/deps"] {
        let f = Fixture::new();
        let original = f.root.join(boundary);
        let saved = f.root.join("saved");
        fs::rename(&original, &saved).unwrap();
        let external = f.root.join("external-empty");
        fs::create_dir(&external).unwrap();
        fs::write(external.join("sentinel"), b"external user data").unwrap();
        directory_alias(&external, &original);
        rejected(&f, &[&saved, &external, &f.root.join(".oxid")]);
    }
}

#[cfg(windows)]
#[test]
fn windows_directory_symlink_capability_and_rejection() {
    // Junction cases above are mandatory independently of symlink privilege.
    // This test must execute, not silently skip, to qualify symlink behavior.
    for boundary in [
        ".oxid",
        ".oxid/deps",
        ".oxid/deps/dep",
        ".oxid/deps/dep/.git",
    ] {
        let f = Fixture::new();
        let original = f.root.join(boundary);
        let external = f.root.join("external");
        fs::rename(&original, &external).unwrap();
        std::os::windows::fs::symlink_dir(&external, &original)
            .expect("Windows directory symlink creation capability required for qualification");
        rejected(&f, &[&external, &f.root.join(".oxid")]);
    }
}
