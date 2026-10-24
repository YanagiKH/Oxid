//! Local typed packages are compiled together through the public project CLI.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const PROCESS_HOST: bool = cfg!(all(target_os = "linux", target_arch = "x86_64"));

struct Packages(PathBuf);
impl Packages {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-path-packages-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn write(&self, path: &str, body: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
    fn package(&self, directory: &str, name: &str, extra: &str, source: &str) {
        self.write(
            &format!("{directory}/oxid.toml"),
            &format!("[project]\nname = \"{name}\"\nedition = \"typed-preview\"\n{extra}"),
        );
        self.write(&format!("{directory}/src/lib.ox"), source);
    }
    fn app(&self, dependencies: &str, source: &str) {
        self.package(
            "app",
            "app",
            &format!("entry = \"src/main.ox\"\n[dependencies]\n{dependencies}"),
            "",
        );
        self.write("app/src/main.ox", source);
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(self.0.join("app"))
            .args(args)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env_remove("OXID_PATH")
            .stdin(Stdio::null());
        command
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }
    fn no_outputs(&self) {
        for path in ["app/.oxid", "cache", "app/oxid.lock", "app/sentinel"] {
            assert!(!self.0.join(path).exists(), "unexpected output: {path}");
        }
    }
}
impl Drop for Packages {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(output: Output) -> Output {
    assert!(output.status.success(), "{output:?}");
    output
}
fn failure(output: Output, message: &str) -> Output {
    assert!(!output.status.success(), "{output:?}");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains(message), "expected {message:?}: {text}");
    output
}
fn value(packages: &Packages, expected: i32) {
    let output = success(packages.run(&["run", "--entry-mode=result", "--message-format=json"]));
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains(&format!("\"value\":{expected}")), "{text}");
}
fn package_host(packages: &Packages) -> bool {
    if cfg!(target_os = "linux") {
        return true;
    }
    let output = failure(
        packages.run(&["check"]),
        "package source policy is qualified only on Linux",
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("E0006"),
        "{output:?}"
    );
    packages.no_outputs();
    false
}

#[test]
fn transitive_dependencies_use_each_packages_own_crate_root() {
    let packages = Packages::new();
    packages.app(
        "math = \"../math\"\n",
        "use crate::math::answer;\nfn main() -> i32 { return answer(); }\n",
    );
    packages.package("math", "math", "[dependencies]\nseed = \"../seed\"\n",
        "mod local;\nuse crate::local::increment;\nuse crate::seed::base;\npub fn answer() -> i32 { return base() + increment(); }\n");
    packages.write(
        "math/src/local.ox",
        "pub fn increment() -> i32 { return 2; }\n",
    );
    packages.package("seed", "seed", "", "pub fn base() -> i32 { return 40; }\n");
    if !package_host(&packages) {
        return;
    }
    success(packages.run(&["check"]));
    value(&packages, 42);
    if PROCESS_HOST {
        let output = packages.run(&["run"]);
        assert_eq!(output.status.code(), Some(42), "{output:?}");
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }
    packages.no_outputs();
}

#[test]
fn a_diamond_shares_nominal_types_at_one_canonical_package_root() {
    let packages = Packages::new();
    packages.app("left = \"../left\"\nright = \"../right\"\n",
        "use crate::left::make;\nuse crate::right::take;\nfn main() -> i32 { return take(make()); }\n");
    packages.package(
        "shared",
        "shared",
        "",
        "pub struct Token { pub value: i32 }\npub fn make() -> Token { return Token { value: 42 }; }\n",
    );
    packages.package("left", "side", "[dependencies]\nshared = \"../shared\"\n",
        "use crate::shared::Token;\nuse crate::shared::make as token;\npub fn make() -> Token { return token(); }\n");
    packages.package(
        "right",
        "side",
        "[dependencies]\nshared = \"../shared/../shared\"\n",
        "use crate::shared::Token;\npub fn take(token: Token) -> i32 { return token.value; }\n",
    );
    if !package_host(&packages) {
        return;
    }
    success(packages.run(&["check"]));
    value(&packages, 42);
    packages.no_outputs();
}

#[test]
fn separate_roots_with_identical_names_and_source_keep_distinct_nominal_types() {
    let packages = Packages::new();
    packages.app("first = \"../first\"\nsecond = \"../second\"\n",
        "use crate::first::make;\nuse crate::second::take;\nfn main() -> i32 { return take(make()); }\n");
    let source = "pub struct Token { value: i32 }\npub fn make() -> Token { return Token { value: 42 }; }\npub fn take(token: Token) -> i32 { return token.value; }\n";
    packages.package("first", "same_name", "", source);
    packages.package("second", "same_name", "", source);
    if !package_host(&packages) {
        return;
    }
    failure(packages.run(&["check"]), "call argument type");
    packages.no_outputs();
}

#[test]
fn declared_tests_retain_the_dependency_graph_and_report_nonzero_results() {
    if !PROCESS_HOST {
        return;
    }
    let packages = Packages::new();
    packages.app(
        "math = \"../math\"\n[tests]\nsmoke = \"tests/smoke.ox\"\n",
        "fn main() -> i32 { return 0; }\n",
    );
    packages.package(
        "math",
        "math",
        "",
        "pub fn answer() -> i32 { return 42; }\n",
    );
    packages.write("app/tests/smoke.ox", "use crate::math::answer;\nfn main() -> i32 { if answer() == 42 { return 0; } else { return 1; } }\n");
    packages.write("app/tests/helper.ox", "this is not a declared test root");
    let output = success(packages.run(&["test", "--message-format=json"]));
    assert!(output.stderr.is_empty());
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"passed\":1"));
    packages.write(
        "app/tests/smoke.ox",
        "use crate::math::answer;\nfn main() -> i32 { return answer(); }\n",
    );
    let output = failure(
        packages.run(&["test", "--message-format=json"]),
        "\"exit_status\":42",
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"failed\":1"));
    packages.no_outputs();
}

#[test]
fn invalid_dependency_graphs_fail_before_running_or_creating_outputs() {
    for (extra, expected) in [
        ("", "edition"),
        ("edition = \"legacy-0.9\"\n", "edition"),
        ("edition = \"typo\"\n", "edition"),
        (
            "edition = \"typed-preview\"\n[features]\nextra = true\n",
            "features",
        ),
    ] {
        let packages = Packages::new();
        packages.app(
            "dep = \"../dep\"\n",
            "write_text(\"sentinel\", \"executed\");\n",
        );
        packages.write(
            "dep/oxid.toml",
            &format!("[project]\nname = \"dep\"\n{extra}"),
        );
        packages.write("dep/src/lib.ox", "print 5 / 2;\n");
        failure(packages.run(&["check"]), expected);
        packages.no_outputs();
    }
    let packages = Packages::new();
    packages.app("dep = \"../dep\"\n", "fn main() -> i32 { return 0; }\n");
    packages.package(
        "dep",
        "dep",
        "[dependencies]\napp = \"../app\"\n",
        "pub fn value() -> i32 { return 0; }\n",
    );
    failure(packages.run(&["check"]), "cycle");
    packages.no_outputs();
}

#[test]
fn aliases_and_source_namespaces_cannot_collide_or_escape() {
    for declarations in [
        "\"../escape\" = \"../dep\"\n",
        "crate = \"../dep\"\n",
        "dep = \"../dep\"\nDep = \"../dep\"\n",
    ] {
        let packages = Packages::new();
        packages.app(declarations, "fn main() -> i32 { return 0; }\n");
        packages.package("dep", "dep", "", "pub fn value() -> i32 { return 0; }\n");
        failure(packages.run(&["check"]), "alias");
        packages.no_outputs();
    }
    let packages = Packages::new();
    packages.app(
        "dep = \"../dep\"\n",
        "mod dep;\nfn main() -> i32 { return 0; }\n",
    );
    packages.write("app/src/dep.ox", "pub fn local() -> i32 { return 1; }\n");
    packages.package("dep", "dep", "", "pub fn value() -> i32 { return 0; }\n");
    if !package_host(&packages) {
        return;
    }
    failure(packages.run(&["check"]), "dep");
    packages.no_outputs();
}

#[test]
fn dependency_alias_length_is_checked_before_opening_its_path() {
    let packages = Packages::new();
    let alias = "a".repeat(65);
    packages.app(
        &format!("{alias} = \"../absent\"\n"),
        "fn main() -> i32 { return 0; }\n",
    );
    failure(packages.run(&["check"]), "invalid typed dependency alias");
    packages.no_outputs();

    let alias = "a".repeat(64);
    packages.app(
        &format!("{alias} = \"../dep\"\n"),
        &format!("use crate::{alias}::value;\nfn main() -> i32 {{ return value(); }}\n"),
    );
    packages.package("dep", "dep", "", "pub fn value() -> i32 { return 4; }\n");
    if !package_host(&packages) {
        return;
    }
    value(&packages, 4);
    packages.no_outputs();
}

#[test]
fn explicit_file_routes_remain_independent_and_inferred_files_explain_project_scope() {
    let packages = Packages::new();
    packages.app(
        "missing = \"../absent\"\n",
        "fn main() -> i32 { return 3; }\n",
    );
    success(packages.run(&["check", "src/main.ox", "--edition=typed-preview"]));
    failure(packages.run(&["check", "src/main.ox"]), "project command");
    packages.no_outputs();
}

#[test]
fn explicit_dependency_entry_and_two_aliases_reuse_one_library_identity() {
    let packages = Packages::new();
    packages.app("first = \"../dep\"\nsecond = \"../dep/.\"\n",
        "use crate::first::Token;\nuse crate::second::make;\nfn main() -> i32 { let token: Token = make(); return token.value; }\n");
    packages.package(
        "dep",
        "dep",
        "entry = \"library.ox\"\n",
        "this default library must not be parsed",
    );
    packages.write("dep/library.ox", "pub struct Token { pub value: i32 }\npub fn make() -> Token { return Token { value: 23 }; }\nfn main() -> i32 { return 99; }\n");
    if !package_host(&packages) {
        return;
    }
    value(&packages, 23);
    packages.no_outputs();
}

#[test]
fn path_and_manifest_failures_are_precise_and_json_remains_framed() {
    for (path, setup, expected) in [
        ("../absent", "none", "local path"),
        ("../dep", "missing-manifest", "manifest"),
        ("../dep/src/lib.ox", "package", "directory"),
    ] {
        let packages = Packages::new();
        packages.app(
            &format!("dep = \"{path}\"\n"),
            "fn main() -> i32 { return 0; }\n",
        );
        if setup == "package" {
            packages.package("dep", "dep", "", "pub fn value() -> i32 { return 0; }\n");
        } else if setup == "missing-manifest" {
            packages.write("dep/src/lib.ox", "pub fn value() -> i32 { return 0; }\n");
        }
        let output = failure(packages.run(&["check", "--message-format=json"]), expected);
        assert!(output.stderr.is_empty(), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text
            .lines()
            .all(|line| line.starts_with('{') && line.ends_with('}')));
        assert!(text.contains("\"success\":false"));
        packages.no_outputs();
    }
}

#[test]
fn unsupported_dependency_forms_and_malicious_paths_cannot_be_ignored() {
    for declaration in [
        "[dependencies]\ndep = \"https://invalid.example/repo.git#main\"\n",
        "[dependencies]\ndep = \"git+file:///tmp/repo\"\n",
        "[dependencies]\ndep = \"registry:dep@1\"\n",
        "[dependencies]\ndep = \"../dep\\u0000\"\n",
        "[dependencies]\ndep = { path = \"../dep\" }\n",
        "[dependencies]\ndep \"../dep\"\n",
        "[dependencies.dep]\npath = \"../dep\"\n",
        "[[dependencies]]\npath = \"../dep\"\n",
        "[\"dependencies\".\"dep\"]\npath = \"../dep\"\n",
        "[features.extra]\nenabled = true\n",
    ] {
        let packages = Packages::new();
        packages.package("app", "app", &format!("entry = \"src/main.ox\"\n{declaration}"), "");
        packages.write("app/src/main.ox", "write_text(\"sentinel\", \"executed\");\n");
        failure(packages.run(&["check"]), if declaration.contains("features") { "features" } else { "dependencies" });
        packages.no_outputs();
    }
    for declaration in [
        "dependencies.dep = \"../dep\"\n",
        "dependencies = { dep = \"../dep\" }\n",
    ] {
        let packages = Packages::new();
        packages.write("app/oxid.toml", &format!("{declaration}[project]\nname = \"app\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n"));
        packages.write("app/src/main.ox", "fn main() -> i32 { return 0; }\n");
        failure(packages.run(&["check"]), "dependencies");
        packages.no_outputs();
    }
}

#[test]
fn invalid_library_names_entries_and_internal_test_options_fail_before_effects() {
    for extra in [
        "",
        "name = \"../escape\"\n",
        "name = \"dep\"\nentry = \"../outside.ox\"\n",
    ] {
        let packages = Packages::new();
        packages.app("dep = \"../dep\"\n", "fn main() -> i32 { return 0; }\n");
        packages.write(
            "dep/oxid.toml",
            &format!("[project]\nedition = \"typed-preview\"\n{extra}"),
        );
        packages.write("dep/src/lib.ox", "pub fn value() -> i32 { return 0; }\n");
        assert!(!packages.run(&["check"]).status.success());
        packages.no_outputs();
    }
    let packages = Packages::new();
    packages.app(
        "dep = \"../dep\"\n[tests]\nsmoke = \"tests/smoke.ox\"\n",
        "fn main() -> i32 { return 0; }\n",
    );
    packages.package("dep", "dep", "", "pub fn value() -> i32 { return 0; }\n");
    packages.write(
        "app/tests/smoke.ox",
        "write_text(\"sentinel\", \"executed\");\n",
    );
    for args in [
        vec!["run", "--project-test=smoke"],
        vec![
            "run",
            "src/main.ox",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--project-test=smoke",
        ],
        vec![
            "check",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--project-test=smoke",
        ],
        vec![
            "run",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--project-test=smoke",
            "--project-test=smoke",
        ],
        vec![
            "run",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--project-test=../outside",
        ],
        vec![
            "run",
            "--edition=typed-preview",
            "--entry-mode=process",
            "--project-test=smoke",
            "--message-format=json",
        ],
    ] {
        assert!(!packages.run(&args).status.success(), "{args:?}");
        packages.no_outputs();
    }
}

#[test]
fn shared_dependency_suffix_cannot_hide_a_too_deep_graph() {
    for (short, long) in [("ashort", "zlong"), ("zshort", "along")] {
        let packages = Packages::new();
        packages.app(
            &format!("{short} = \"../tail0\"\n{long} = \"../chain0\"\n"),
            "fn main() -> i32 { return 0; }\n",
        );
        for index in 0..30 {
            let next = if index == 29 {
                "tail0".to_owned()
            } else {
                format!("chain{}", index + 1)
            };
            packages.package(
                &format!("chain{index}"),
                "chain",
                &format!("[dependencies]\nnext = \"../{next}\"\n"),
                "pub fn value() -> i32 { return 0; }\n",
            );
        }
        for index in 0..3 {
            let extra = if index == 2 {
                String::new()
            } else {
                format!("[dependencies]\nnext = \"../tail{}\"\n", index + 1)
            };
            packages.package(
                &format!("tail{index}"),
                "tail",
                &extra,
                "pub fn value() -> i32 { return 0; }\n",
            );
        }
        failure(packages.run(&["check"]), "32");
        packages.no_outputs();
    }
}

#[cfg(unix)]
#[test]
fn directory_aliases_deduplicate_but_library_sources_cannot_escape_by_symlink() {
    use std::os::unix::fs::symlink;
    let packages = Packages::new();
    packages.app("first = \"../dep\"\nsecond = \"../alias\"\n",
        "use crate::first::Token;\nuse crate::second::make;\nfn main() -> i32 { let token: Token = make(); return token.value; }\n");
    packages.package(
        "dep",
        "dep",
        "",
        "pub struct Token { pub value: i32 }\npub fn make() -> Token { return Token { value: 9 }; }\n",
    );
    symlink(packages.0.join("dep"), packages.0.join("alias")).unwrap();
    if package_host(&packages) {
        value(&packages, 9);
    }
    fs::remove_file(packages.0.join("dep/src/lib.ox")).unwrap();
    packages.write("outside.ox", "pub fn make() -> i32 { return 1; }\n");
    symlink(
        packages.0.join("outside.ox"),
        packages.0.join("dep/src/lib.ox"),
    )
    .unwrap();
    failure(packages.run(&["check"]), "symbolic link");
    packages.no_outputs();
}

#[cfg(unix)]
#[test]
fn nonregular_dependency_manifest_is_rejected_without_waiting_for_input() {
    use std::ffi::CString;
    use std::os::unix::fs::symlink;
    use std::time::{Duration, Instant};
    let packages = Packages::new();
    packages.app("dep = \"../dep\"\n", "fn main() -> i32 { return 0; }\n");
    packages.write("dep/src/lib.ox", "pub fn value() -> i32 { return 0; }\n");
    let manifest = packages.0.join("dep/oxid.toml");
    let path = CString::new(manifest.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    let mut child = packages
        .command(&["check"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let finished = loop {
        if child.try_wait().unwrap().is_some() {
            break true;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            break false;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let output = child.wait_with_output().unwrap();
    assert!(finished, "dependency manifest read blocked: {output:?}");
    failure(output, "manifest must be a regular file");
    fs::remove_file(&manifest).unwrap();
    fs::create_dir(&manifest).unwrap();
    failure(packages.run(&["check"]), "manifest must be a regular file");
    fs::remove_dir(&manifest).unwrap();
    packages.write(
        "outside-manifest.toml",
        "[project]\nname = \"dep\"\nedition = \"typed-preview\"\n",
    );
    symlink(packages.0.join("outside-manifest.toml"), &manifest).unwrap();
    failure(packages.run(&["check"]), "manifest must be a regular file");
    packages.no_outputs();
}

#[test]
fn invalid_package_build_preserves_the_previous_managed_binary() {
    let packages = Packages::new();
    packages.app(
        "dep = \"../dep\"\n",
        "use crate::dep::value;\nfn main() -> i32 { return value(); }\n",
    );
    packages.package("dep", "dep", "", "pub fn value() -> i32 { return true; }\n");
    packages.write("app/.oxid/bin/app", "previous managed binary");
    failure(
        packages.run(&["build"]),
        if cfg!(target_os = "linux") {
            "E0300"
        } else {
            "package source policy is qualified only on Linux"
        },
    );
    assert_eq!(
        fs::read(packages.0.join("app/.oxid/bin/app")).unwrap(),
        b"previous managed binary"
    );
}

#[test]
#[ignore = "requires qualified Linux x86_64 with LLVM 19.1.7"]
fn package_native_build_runs_without_sources_and_rebuild_failure_preserves_it() {
    let packages = Packages::new();
    packages.app(
        "dep = \"../dep\"\n",
        "use crate::dep::value;\nfn main() -> i32 { return value(); }\n",
    );
    packages.package("dep", "dep", "", "pub fn value() -> i32 { return 7; }\n");
    success(packages.run(&["build"]));
    let binary = packages.0.join("app/.oxid/bin/app");
    assert_eq!(Command::new(&binary).status().unwrap().code(), Some(7));
    packages.write("dep/src/lib.ox", "pub fn value() -> i32 { return 9; }\n");
    success(packages.run(&["build"]));
    let previous = fs::read(&binary).unwrap();
    packages.write("dep/src/lib.ox", "pub fn value() -> i32 { return true; }\n");
    failure(packages.run(&["build"]), "E0300");
    assert_eq!(fs::read(&binary).unwrap(), previous);
    fs::remove_dir_all(packages.0.join("dep")).unwrap();
    fs::remove_dir_all(packages.0.join("app/src")).unwrap();
    fs::remove_file(packages.0.join("app/oxid.toml")).unwrap();
    let output = Command::new(binary)
        .current_dir(&packages.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(9), "{output:?}");
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
}
