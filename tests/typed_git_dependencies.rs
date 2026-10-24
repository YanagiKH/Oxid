//! Pinned Git packages use the ordinary typed graph and lock checks.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::path::Path;

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-typed-git-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn write(&self, path: &str, value: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }

    fn git(&self, directory: &str, args: &[&str]) -> String {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Oxid tests",
                "-c",
                "user.email=tests@oxid.invalid",
            ])
            .args(args)
            .current_dir(self.0.join(directory))
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn repository(&self) -> (String, String) {
        self.write("remote/oxid.toml", "[project]\nname=\"numbers\"\nversion=\"1.0.0\"\nedition=\"typed-preview\"\nkind=\"lib\"\n");
        self.write("remote/.gitignore", "scratch\n");
        self.write("remote/src/lib.ox", "pub struct Number{pub value:i32}pub fn make()->Number{return Number{value:11};}pub fn value(n:&Number)->i32{return n.value;}");
        self.git("remote", &["init", "--quiet"]);
        self.git("remote", &["add", "."]);
        self.git("remote", &["commit", "--quiet", "-m", "First value"]);
        let first = self.git("remote", &["rev-parse", "HEAD"]);
        self.write("remote/src/lib.ox", "pub struct Number{pub value:i32}pub fn make()->Number{return Number{value:17};}pub fn value(n:&Number)->i32{return n.value;}");
        self.git("remote", &["add", "."]);
        self.git("remote", &["commit", "--quiet", "-m", "Second value"]);
        (first, self.git("remote", &["rev-parse", "HEAD"]))
    }

    fn source(&self, revision: &str) -> String {
        let path = self.0.join("remote").to_string_lossy().replace('\\', "/");
        format!(
            "git+file://{}{path}#{revision}",
            if path.starts_with('/') { "" } else { "/" }
        )
    }

    fn application(&self, dependencies: &[(&str, String)], source: &str) {
        let mut manifest = String::from("[project]\nname=\"git-app\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[tests]\nproof=\"test.ox\"\n[dependencies]\n");
        for (alias, source) in dependencies {
            manifest.push_str(&format!("{alias}={source:?}\n"));
        }
        self.write("app/oxid.toml", &manifest);
        self.write("app/main.ox", source);
        self.write("app/test.ox", "fn main()->i32{return 0;}");
    }

    fn invoke(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(self.0.join("app"))
            .env("OXID_CACHE_DIR", self.0.join("native-cache"))
            .output()
            .unwrap()
    }

    fn checkouts(&self) -> Vec<PathBuf> {
        let root = self.0.join("app/.oxid/deps");
        if !root.exists() {
            return Vec::new();
        }
        let mut paths = fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn success(output: Output) -> Output {
    assert!(output.status.success(), "{output:?}");
    output
}
fn failure(output: Output, message: &str) {
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(message),
        "{output:?}"
    );
}

const TWO_VERSIONS: &str = "use crate::old::make as old_make;use crate::new::make as new_make;use crate::old::value as old_value;use crate::new::value as new_value;fn main()->i32{let a=old_make();let b=new_make();return old_value(&a)+new_value(&b);}";

#[test]
fn revisions_coexist_and_a_locked_application_runs_without_its_remote() {
    let f = Fixture::new();
    let (a, b) = f.repository();
    f.application(
        &[("old", f.source(&a)), ("new", f.source(&b))],
        TWO_VERSIONS,
    );
    success(f.invoke(&["lock"]));
    assert_eq!(f.checkouts().len(), 2);
    let lock = fs::read(f.0.join("app/oxid.lock")).unwrap();
    fs::rename(f.0.join("remote"), f.0.join("remote-unavailable")).unwrap();
    for mode in ["--locked", "--offline", "--frozen"] {
        success(f.invoke(&["check", mode]));
        let output = success(f.invoke(&["run", mode]));
        assert_eq!(output.stdout, b"28\n");
        assert!(output.stderr.is_empty());
        success(f.invoke(&["test", mode]));
        assert_eq!(fs::read(f.0.join("app/oxid.lock")).unwrap(), lock);
    }
}

#[test]
fn aliases_share_one_revision_but_different_revisions_keep_distinct_types() {
    let f = Fixture::new();
    let (a, b) = f.repository();
    let source="use crate::first::make;use crate::second::value;fn main()->i32{let n=make();return value(&n);}";
    f.application(&[("first", f.source(&a)), ("second", f.source(&a))], source);
    assert_eq!(success(f.invoke(&["run"])).stdout, b"11\n");
    assert_eq!(f.checkouts().len(), 1);
    f.application(&[("first", f.source(&a)), ("second", f.source(&b))], source);
    failure(f.invoke(&["check"]), "E0300");
    assert_eq!(f.checkouts().len(), 2);
}

#[test]
fn a_nested_git_edge_reuses_the_root_revision_identity() {
    let f = Fixture::new();
    let (a, _) = f.repository();
    let manifest = fs::read_to_string(f.0.join("remote/oxid.toml")).unwrap();
    f.write(
        "remote/oxid.toml",
        &format!("{manifest}[dependencies]\nprevious={:?}\n", f.source(&a)),
    );
    f.write("remote/src/lib.ox","use crate::previous::Number;use crate::previous::value;pub fn read_previous(n:&Number)->i32{return value(&*n)+17;}");
    f.git("remote", &["add", "."]);
    f.git(
        "remote",
        &["commit", "--quiet", "-m", "Use previous revision"],
    );
    let b = f.git("remote", &["rev-parse", "HEAD"]);
    f.application(&[("old",f.source(&a)),("new",f.source(&b))],"use crate::old::make;use crate::new::read_previous;fn main()->i32{let n=make();return read_previous(&n);}");
    success(f.invoke(&["lock"]));
    assert_eq!(success(f.invoke(&["run", "--frozen"])).stdout, b"28\n");
    assert_eq!(f.checkouts().len(), 2);
}

#[test]
fn uncached_offline_and_locked_modes_never_create_dependency_caches() {
    let f = Fixture::new();
    let (a, _) = f.repository();
    f.application(&[("numbers", f.source(&a))], "fn main()->i32{return 0;}");
    failure(f.invoke(&["check", "--offline"]), "not cached");
    failure(f.invoke(&["check", "--locked"]), "lock");
    failure(f.invoke(&["lock", "--frozen"]), "lock");
    assert!(!f.0.join("app/.oxid").exists());
    assert!(!f.0.join("app/oxid.lock").exists());
}

#[test]
fn changing_a_pin_requires_an_explicit_lock_update() {
    let f = Fixture::new();
    let (a, b) = f.repository();
    let source="use crate::numbers::make;use crate::numbers::value;fn main()->i32{let n=make();return value(&n);}";
    f.application(&[("numbers", f.source(&a))], source);
    success(f.invoke(&["lock"]));
    let lock = fs::read(f.0.join("app/oxid.lock")).unwrap();
    f.application(&[("numbers", f.source(&b))], source);
    failure(f.invoke(&["run", "--locked"]), "not cached");
    assert_eq!(f.checkouts().len(), 1);
    failure(f.invoke(&["run"]), "lock");
    assert_eq!(fs::read(f.0.join("app/oxid.lock")).unwrap(), lock);
    failure(f.invoke(&["lock"]), "lock");
    success(f.invoke(&["lock", "--update"]));
    assert_ne!(fs::read(f.0.join("app/oxid.lock")).unwrap(), lock);
    assert_eq!(success(f.invoke(&["run", "--frozen"])).stdout, b"17\n");
}

#[test]
fn ignored_cache_contents_are_preserved_even_by_lock_update() {
    let f = Fixture::new();
    let (a, _) = f.repository();
    f.application(&[("numbers", f.source(&a))], "fn main()->i32{return 0;}");
    success(f.invoke(&["lock"]));
    let cache = f.checkouts().remove(0);
    fs::write(cache.join("scratch"), b"keep this ignored file").unwrap();
    let paths = [
        cache.join(".git/HEAD"),
        cache.join(".git/index"),
        cache.join(".git/config"),
        cache.join("scratch"),
        f.0.join("app/oxid.lock"),
    ];
    let before = paths
        .iter()
        .map(fs::read)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    for args in [
        &["check"][..],
        &["check", "--frozen"],
        &["lock", "--update"],
    ] {
        failure(f.invoke(args), "cache");
        for (path, bytes) in paths.iter().zip(&before) {
            assert_eq!(&fs::read(path).unwrap(), bytes);
        }
    }
}

#[test]
fn malformed_pins_are_rejected_before_any_cache_is_created() {
    let f = Fixture::new();
    for source in [
        "https://invalid.example/repository.git",
        "git+https://invalid.example/repository#main",
        "git+https://user:secret@invalid.example/repo#0123456789012345678901234567890123456789",
        "git+ssh://invalid.example/repo#0123456789012345678901234567890123456789",
    ] {
        f.application(&[("numbers", source.into())], "fn main()->i32{return 0;}");
        failure(f.invoke(&["check"]), "dependencies");
        assert!(!f.0.join("app/.oxid").exists());
    }
}

#[test]
fn workspace_locks_cover_cached_git_dependencies_of_unselected_members() {
    let f = Fixture::new();
    let (a, b) = f.repository();
    f.write(
        "app/oxid.toml",
        "[workspace]\nedition=\"typed-preview\"\nmembers=[\"left\",\"right\"]\n",
    );
    for (name, revision) in [("left", a), ("right", b)] {
        f.write(&format!("app/{name}/oxid.toml"), &format!("[project]\nname={name:?}\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n[dependencies]\nnumbers={:?}\n",f.source(&revision)));
        f.write(&format!("app/{name}/main.ox"),"use crate::numbers::make;use crate::numbers::value;fn main()->i32{let n=make();return value(&n);}");
    }
    success(f.invoke(&["lock"]));
    let lock = fs::read(f.0.join("app/oxid.lock")).unwrap();
    let right = fs::read_dir(f.0.join("app/right/.oxid/deps"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(right.join("scratch"), b"keep").unwrap();
    failure(f.invoke(&["check", "-p", "left", "--frozen"]), "cache");
    assert_eq!(fs::read(right.join("scratch")).unwrap(), b"keep");
    assert_eq!(fs::read(f.0.join("app/oxid.lock")).unwrap(), lock);
    fs::remove_file(right.join("scratch")).unwrap();
    fs::rename(f.0.join("remote"), f.0.join("remote-unavailable")).unwrap();
    assert_eq!(
        success(f.invoke(&["run", "-p", "left", "--frozen"])).stdout,
        b"11\n"
    );
    assert_eq!(
        success(f.invoke(&["run", "-p", "right", "--frozen"])).stdout,
        b"17\n"
    );
}

#[test]
fn source_bundles_keep_their_portable_local_dependency_contract() {
    let f = Fixture::new();
    let (a, _) = f.repository();
    f.application(&[("numbers", f.source(&a))], "fn main()->i32{return 0;}");
    success(f.invoke(&["lock"]));
    failure(
        f.invoke(&["package", "--frozen", "--output", "bundle.oxpkg.tar"]),
        "source package",
    );
    assert!(!f.0.join("app/bundle.oxpkg.tar").exists());
    assert_eq!(f.checkouts().len(), 1);
}

#[test]
fn local_dependencies_inside_a_git_checkout_work_and_cannot_escape_it() {
    let f = Fixture::new();
    let _ = f.repository();
    f.write(
        "remote/helper/oxid.toml",
        "[project]\nname=\"helper\"\nedition=\"typed-preview\"\nkind=\"lib\"\n",
    );
    f.write(
        "remote/helper/src/lib.ox",
        "pub fn answer()->i32{return 42;}",
    );
    let manifest = fs::read_to_string(f.0.join("remote/oxid.toml")).unwrap();
    f.write(
        "remote/oxid.toml",
        &format!("{manifest}[dependencies]\nhelper=\"helper\"\n"),
    );
    f.write(
        "remote/src/lib.ox",
        "use crate::helper::answer;pub fn result()->i32{return answer();}",
    );
    f.git("remote", &["add", "."]);
    f.git(
        "remote",
        &["commit", "--quiet", "-m", "Add contained local package"],
    );
    let a = f.git("remote", &["rev-parse", "HEAD"]);
    let source = "use crate::numbers::result;fn main()->i32{return result();}";
    f.application(&[("numbers", f.source(&a))], source);
    assert_eq!(success(f.invoke(&["run"])).stdout, b"42\n");
    f.write(
        "outside/oxid.toml",
        "[project]\nedition=\"typed-preview\"\nkind=\"lib\"\n",
    );
    f.write(
        "remote/oxid.toml",
        &format!(
            "{manifest}[dependencies]\nhelper={:?}\n",
            f.0.join("outside").to_str().unwrap()
        ),
    );
    f.git("remote", &["add", "."]);
    f.git(
        "remote",
        &["commit", "--quiet", "-m", "Request outside package"],
    );
    let b = f.git("remote", &["rev-parse", "HEAD"]);
    f.application(&[("numbers", f.source(&b))], source);
    failure(f.invoke(&["check"]), "inside its checkout");
}

#[cfg(unix)]
#[test]
fn a_symlinked_cache_root_is_refused_without_writing_its_target() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let (a, _) = f.repository();
    f.application(&[("numbers", f.source(&a))], "fn main()->i32{return 0;}");
    f.write("outside/sentinel", "keep");
    symlink(f.0.join("outside"), f.0.join("app/.oxid")).unwrap();
    failure(f.invoke(&["lock"]), "cache location");
    assert_eq!(fs::read(f.0.join("outside/sentinel")).unwrap(), b"keep");
    assert_eq!(fs::read_dir(f.0.join("outside")).unwrap().count(), 1);
}

#[test]
fn a_local_alias_cannot_bypass_a_pinned_packages_dependency_boundary() {
    let f = Fixture::new();
    let _ = f.repository();
    f.write(
        "outside/oxid.toml",
        "[project]\nname=\"outside\"\nedition=\"typed-preview\"\nkind=\"lib\"\n",
    );
    f.write("outside/src/lib.ox", "pub fn answer()->i32{return 42;}");
    let manifest = fs::read_to_string(f.0.join("remote/oxid.toml")).unwrap();
    f.write(
        "remote/oxid.toml",
        &format!(
            "{manifest}[dependencies]\nescape={:?}\n",
            f.0.join("outside").to_str().unwrap()
        ),
    );
    f.write(
        "remote/src/lib.ox",
        "use crate::escape::answer;pub fn read()->i32{return answer();}",
    );
    f.git("remote", &["add", "."]);
    f.git(
        "remote",
        &["commit", "--quiet", "-m", "Request outside package"],
    );
    let revision = f.git("remote", &["rev-parse", "HEAD"]);
    let source = f.source(&revision);
    f.application(&[("pinned", source.clone())], "fn main()->i32{return 0;}");
    failure(f.invoke(&["check"]), "inside its checkout");
    let cached = f
        .checkouts()
        .remove(0)
        .strip_prefix(f.0.join("app"))
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    for (local_alias, git_alias) in [("first_local", "last_git"), ("last_local", "first_git")] {
        f.application(
            &[(local_alias, cached.clone()), (git_alias, source.clone())],
            "fn main()->i32{return 0;}",
        );
        failure(f.invoke(&["check"]), "inside its checkout");
        assert!(!f.0.join("app/oxid.lock").exists());
        assert!(!f.0.join("app/.oxid/bin").exists());
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn deployed(path: &Path, cwd: &Path) -> Output {
    use std::os::unix::process::CommandExt;
    let mut command = Command::new(path);
    command.current_dir(cwd).env_clear();
    unsafe {
        command.pre_exec(|| {
            let limit = libc::rlimit {
                rlim_cur: 32 * 1024 * 1024,
                rlim_max: 32 * 1024 * 1024,
            };
            if libc::setrlimit(libc::RLIMIT_AS, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.output().unwrap()
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires LLVM, Clang and LLD 19.1.7"]
fn pinned_libraries_build_both_profiles_and_deploy_without_sources_or_git() {
    let f = Fixture::new();
    let (a, b) = f.repository();
    f.application(
        &[("old", f.source(&a)), ("new", f.source(&b))],
        TWO_VERSIONS,
    );
    success(f.invoke(&["lock"]));
    let deploy = f.0.join("deploy");
    fs::create_dir(&deploy).unwrap();
    for entry in ["result", "process"] {
        for profile in ["dev", "release"] {
            let args = [
                "build",
                "--frozen",
                "--entry-mode",
                entry,
                "--profile",
                profile,
                "--reproducible",
                "--incremental",
            ];
            success(f.invoke(&args));
            let destination = deploy.join(format!("{profile}-{entry}"));
            fs::copy(f.0.join("app/.oxid/bin/git-app"), &destination).unwrap();
            let warm = success(f.invoke(&args));
            assert!(
                String::from_utf8_lossy(&warm.stdout).contains("hit"),
                "{warm:?}"
            );
            assert_eq!(
                fs::read(destination).unwrap(),
                fs::read(f.0.join("app/.oxid/bin/git-app")).unwrap()
            );
        }
    }
    let previous = fs::read(f.0.join("app/.oxid/bin/git-app")).unwrap();
    let lock = fs::read(f.0.join("app/oxid.lock")).unwrap();
    f.application(
        &[("old", f.source(&a)), ("new", f.source(&"0".repeat(40)))],
        TWO_VERSIONS,
    );
    failure(
        f.invoke(&["build", "--frozen", "--profile", "dev"]),
        "not cached",
    );
    assert_eq!(
        fs::read(f.0.join("app/.oxid/bin/git-app")).unwrap(),
        previous
    );
    assert_eq!(fs::read(f.0.join("app/oxid.lock")).unwrap(), lock);
    assert_eq!(f.checkouts().len(), 2);
    fs::remove_dir_all(f.0.join("remote")).unwrap();
    fs::remove_dir_all(f.0.join("app")).unwrap();
    if f.0.join("native-cache").exists() {
        fs::remove_dir_all(f.0.join("native-cache")).unwrap();
    }
    for entry in ["result", "process"] {
        for profile in ["dev", "release"] {
            let result = deployed(&deploy.join(format!("{profile}-{entry}")), &deploy);
            assert_eq!(
                result.status.code(),
                Some(if entry == "result" { 0 } else { 28 }),
                "{result:?}"
            );
            assert_eq!(
                result.stdout,
                if entry == "result" {
                    b"28\n".as_slice()
                } else {
                    b""
                },
                "{result:?}"
            );
            assert!(result.stderr.is_empty(), "{result:?}");
        }
    }
}
