//! Typed local locks bind manifests and the compiler's actual dependency inputs.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-typed-lock-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self(root);
        fixture.write("app/oxid.toml", "[project]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n[dependencies]\nmath = \"../math\"\n[tests]\nsmoke = \"tests/smoke.ox\"\n");
        fixture.write(
            "app/src/main.ox",
            "use crate::math::answer;\nfn main() -> i32 { return answer(); }\n",
        );
        fixture.write("app/tests/smoke.ox", "use crate::math::answer;\nfn main() -> i32 { if answer() == 7 { return 0; } else { return 1; } }\n");
        fixture.write(
            "math/oxid.toml",
            "[project]\nname = \"math\"\nversion = \"0.1.0\"\nedition = \"typed-preview\"\n",
        );
        fixture.write("math/src/lib.ox", "pub fn answer() -> i32 { return 7; }\n");
        fixture
    }
    fn write(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
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
    fn lock(&self) -> Vec<u8> {
        fs::read(self.0.join("app/oxid.lock")).unwrap()
    }
    fn unchanged(&self, bytes: &[u8]) {
        assert_eq!(self.lock(), bytes);
        for relative in ["app/.oxid", "app/sentinel", "cache"] {
            assert!(!self.0.join(relative).exists(), "unexpected {relative}");
        }
        for entry in fs::read_dir(self.0.join("app")).unwrap() {
            assert!(!entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".oxid-lock-"));
        }
    }
    fn qualified(&self) -> bool {
        if cfg!(any(target_os = "linux", target_os = "macos")) {
            return true;
        }
        failure(
            self.run(&["lock"]),
            "package source policy is qualified only on Linux",
        );
        assert!(!self.0.join("app/oxid.lock").exists());
        false
    }
}
impl Drop for Fixture {
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

#[test]
fn local_lock_creation_verification_and_explicit_update() {
    let fixture = Fixture::new();
    if !fixture.qualified() {
        return;
    }
    success(fixture.run(&["check"]));
    assert!(!fixture.0.join("app/oxid.lock").exists());
    failure(fixture.run(&["check", "--locked"]), "requires oxid.lock");
    success(fixture.run(&["lock"]));
    let original = fixture.lock();
    let text = String::from_utf8(original.clone()).unwrap();
    assert!(
        text.starts_with("version = 2\nkind = \"typed-local-v1\"\n"),
        "{text}"
    );
    assert!(text.contains("path = \"../math\""), "{text}");
    success(fixture.run(&["lock"]));
    success(fixture.run(&["check", "--locked"]));
    fixture.unchanged(&original);
    fixture.write("math/src/lib.ox", "pub fn answer() -> i32 { return 9; }\n");
    for args in [&["check"][..], &["lock"], &["check", "--frozen"]] {
        failure(fixture.run(args), "lock is out of date");
        fixture.unchanged(&original);
    }
    success(fixture.run(&["lock", "--update"]));
    assert_ne!(fixture.lock(), original);
    success(fixture.run(&["check", "--frozen"]));
}

#[test]
fn application_sources_are_editable_but_every_manifest_is_pinned() {
    let fixture = Fixture::new();
    if !fixture.qualified() {
        return;
    }
    success(fixture.run(&["lock"]));
    let original = fixture.lock();
    fixture.write("app/src/main.ox", "fn main() -> i32 { return 42; }\n");
    let output = success(fixture.run(&[
        "run",
        "--locked",
        "--entry-mode=result",
        "--message-format=json",
    ]));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("\"value\":42"),
        "{output:?}"
    );
    fixture.write("app/tests/smoke.ox", "fn main() -> i32 { return 0; }\n");
    if cfg!(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        success(fixture.run(&["test", "--frozen"]));
        fs::remove_file(fixture.0.join("app/src/main.ox")).unwrap();
        success(fixture.run(&["test", "--locked"]));
        fixture.write("app/src/main.ox", "fn main() -> i32 { return 42; }\n");
    }
    fixture.unchanged(&original);
    for path in ["app/oxid.toml", "math/oxid.toml"] {
        let manifest = fs::read_to_string(fixture.0.join(path)).unwrap();
        fixture.write(
            path,
            &(manifest.clone() + "# raw manifest bytes are part of the lock\n"),
        );
        failure(fixture.run(&["check"]), "lock is out of date");
        fixture.unchanged(&original);
        fixture.write(path, &manifest);
    }
}

#[test]
fn locking_modes_are_validated_before_effects_and_offline_does_not_create_a_lock() {
    let fixture = Fixture::new();
    for args in [
        vec!["lock", "--update", "--locked"],
        vec!["lock", "--update", "--offline"],
        vec!["lock", "--update", "--frozen"],
        vec!["check", "--update"],
        vec!["build", "--locked", "--locked"],
        vec!["test", "--frozen=false"],
        vec!["lock", "--offline=1"],
        vec!["lock", "--update", "--update"],
    ] {
        let output = fixture.run(&args);
        assert!(!output.status.success(), "{args:?}: {output:?}");
        assert!(!fixture.0.join("app/oxid.lock").exists());
        assert!(!fixture.0.join("app/.oxid").exists());
    }
    for command in ["check", "run", "build", "test", "lock"] {
        for flag in ["--locked", "--frozen"] {
            failure(fixture.run(&[command, flag]), "requires oxid.lock");
        }
    }
    if !fixture.qualified() {
        return;
    }
    success(fixture.run(&["--offline", "check"]));
    assert!(!fixture.0.join("app/oxid.lock").exists());
    success(fixture.run(&["lock", "--offline"]));
    let original = fixture.lock();
    success(fixture.run(&["--locked", "--offline", "check"]));
    success(fixture.run(&["lock", "--frozen"]));
    fixture.unchanged(&original);
}

#[test]
fn zero_dependency_projects_lock_without_pinning_application_code() {
    let fixture = Fixture::new();
    fixture.write("app/oxid.toml", "[project]\nname = \"solo\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n[tests]\nsmoke = \"tests/smoke.ox\"\n");
    fixture.write("app/src/main.ox", "fn main() -> i32 { return 0; }\n");
    fixture.write("app/tests/smoke.ox", "fn main() -> i32 { return 0; }\n");
    success(fixture.run(&["lock"]));
    let original = fixture.lock();
    let text = String::from_utf8(original.clone()).unwrap();
    assert_eq!(text.matches("[[package]]").count(), 1);
    assert_eq!(text.matches("[[dependency]]").count(), 0);
    fixture.write("app/src/main.ox", "fn main() -> i32 { return 4; }\n");
    success(fixture.run(&["check", "--frozen"]));
    if cfg!(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        success(fixture.run(&["test", "--locked"]));
    }
    fixture.unchanged(&original);
}

#[test]
fn old_unknown_and_corrupt_locks_are_never_overwritten_even_with_update() {
    let fixture = Fixture::new();
    if !fixture.qualified() {
        return;
    }
    success(fixture.run(&["lock"]));
    let valid = String::from_utf8(fixture.lock()).unwrap();
    let corrupt = [
        "version = 1\n".into(),
        valid.replacen("version = 2", "version = 3", 1),
        valid.replacen("typed-local-v1", "unknown-kind", 1),
        valid.replacen("root =", "unknown =", 1),
        valid.replacen("path = \"../math\"", "path = \"../elsewhere\"", 1),
        valid.replacen("alias = \"math\"", "alias = \"crate\"", 1),
        valid.replacen("sources = \"root\"", "sources = \"sha256:00\"", 1),
        valid.clone() + "\nunknown = true\n",
        valid.clone() + "\n[[package]]\n",
    ];
    for text in corrupt {
        fixture.write("app/oxid.lock", &text);
        for args in [&["lock"][..], &["lock", "--update"], &["build", "--locked"]] {
            failure(fixture.run(args), "lock");
            fixture.unchanged(text.as_bytes());
        }
    }
    for bytes in [vec![0xff; 10], vec![b'x'; 2 * 1024 * 1024 + 1]] {
        fs::write(fixture.0.join("app/oxid.lock"), &bytes).unwrap();
        failure(fixture.run(&["lock", "--update"]), "lock");
        fixture.unchanged(&bytes);
    }
}

#[test]
fn relocation_and_diamonds_keep_canonical_package_identities() {
    let mut fixture = Fixture::new();
    fixture.write("app/oxid.toml", "[project]\nname = \"app\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n[dependencies]\nleft = \"../left\"\nright = \"../right\"\n");
    fixture.write("app/src/main.ox", "use crate::left::left_value;\nuse crate::right::right_value;\nfn main() -> i32 { return left_value() + right_value(); }\n");
    for (directory, function, path) in [
        ("left", "left_value", "../math"),
        ("right", "right_value", "../math/."),
    ] {
        fixture.write(&format!("{directory}/oxid.toml"), &format!("[project]\nname = \"same-name\"\nedition = \"typed-preview\"\n[dependencies]\nmath = \"{path}\"\n"));
        fixture.write(
            &format!("{directory}/src/lib.ox"),
            &format!(
                "use crate::math::answer;\npub fn {function}() -> i32 {{ return answer(); }}\n"
            ),
        );
    }
    if !fixture.qualified() {
        return;
    }
    success(fixture.run(&["lock"]));
    let original = fixture.lock();
    let text = String::from_utf8(original.clone()).unwrap();
    assert_eq!(text.matches("[[package]]").count(), 4, "{text}");
    assert_eq!(text.matches("[[dependency]]").count(), 4, "{text}");
    assert_eq!(text.matches("name = \"same-name\"").count(), 2);
    assert_eq!(text.matches("path = \"../math\"").count(), 1);
    assert!(!text.contains(fixture.0.to_str().unwrap()));
    let destination = fixture.0.with_extension("relocated");
    fs::rename(&fixture.0, &destination).unwrap();
    fixture.0 = destination;
    success(fixture.run(&["lock"]));
    let output = success(fixture.run(&[
        "run",
        "--frozen",
        "--entry-mode=result",
        "--message-format=json",
    ]));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("\"value\":14"),
        "{output:?}"
    );
    fixture.unchanged(&original);
}

#[test]
fn actual_loaded_sources_under_target_are_pinned_and_unloaded_files_are_irrelevant() {
    let fixture = Fixture::new();
    fixture.write(
        "math/oxid.toml",
        "[project]\nname = \"math\"\nedition = \"typed-preview\"\nentry = \"target/lib.ox\"\n",
    );
    fixture.write(
        "math/target/lib.ox",
        "mod helper;\nuse crate::helper::value;\npub fn answer() -> i32 { return value(); }\n",
    );
    fixture.write(
        "math/target/helper.ox",
        "pub fn value() -> i32 { return 7; }\n",
    );
    if !fixture.qualified() {
        return;
    }
    success(fixture.run(&["lock"]));
    let original = fixture.lock();
    fixture.write("math/src/lib.ox", "not even valid source\n");
    fixture.write("math/target/unloaded.ox", "not even valid source\n");
    success(fixture.run(&["check", "--locked"]));
    fixture.unchanged(&original);
    fixture.write(
        "math/target/helper.ox",
        "pub fn value() -> i32 { return 8; }\n",
    );
    failure(fixture.run(&["check", "--locked"]), "lock is out of date");
    fixture.unchanged(&original);
    success(fixture.run(&["lock", "--update"]));
    assert_ne!(fixture.lock(), original);
}

#[test]
fn source_errors_cannot_publish_a_lock_and_file_routes_ignore_project_locks() {
    let fixture = Fixture::new();
    fixture.write("app/src/main.ox", "print 5 / 2;\n");
    failure(fixture.run(&["lock", "--edition=typed-preview"]), "error");
    assert!(!fixture.0.join("app/oxid.lock").exists());
    fixture.write("app/src/main.ox", "fn main() -> i32 { return 4; }\n");
    fixture.write("app/oxid.lock", "version = 1\n");
    success(fixture.run(&["check", "src/main.ox", "--edition=typed-preview"]));
    failure(
        fixture.run(&[
            "check",
            "src/main.ox",
            "--edition=typed-preview",
            "--locked",
        ]),
        "--locked",
    );
    fixture.unchanged(b"version = 1\n");
    fixture.write("app/legacy.ox", "print 5 / 2;\n");
    let output = success(fixture.run(&["run", "legacy.ox", "--edition=legacy-0.9"]));
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "2.5");
    assert_eq!(fixture.lock(), b"version = 1\n");
}

#[test]
fn typed_manifests_have_bounded_strict_snapshots_without_changing_legacy_reads() {
    let fixture = Fixture::new();
    let manifest = fs::read_to_string(fixture.0.join("app/oxid.toml")).unwrap();
    fixture.write(
        "app/oxid.toml",
        &(manifest + "#" + &"x".repeat(1024 * 1024)),
    );
    failure(fixture.run(&["lock"]), "at most");
    assert!(!fixture.0.join("app/oxid.lock").exists());
    fixture.write("app/legacy.ox", "print 5 / 2;\n");
    let output = success(fixture.run(&["run", "legacy.ox", "--edition=legacy-0.9"]));
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "2.5");
}

#[cfg(unix)]
#[test]
fn nonregular_locks_are_rejected_without_blocking_or_changing_the_target() {
    use std::{
        ffi::CString,
        os::unix::{ffi::OsStrExt, fs::symlink},
        thread,
        time::{Duration, Instant},
    };
    let fixture = Fixture::new();
    let path = fixture.0.join("app/oxid.lock");
    fs::create_dir(&path).unwrap();
    failure(fixture.run(&["lock", "--update"]), "regular file");
    fs::remove_dir(&path).unwrap();
    fixture.write("outside", "unchanged");
    symlink(fixture.0.join("outside"), &path).unwrap();
    failure(fixture.run(&["lock", "--update"]), "regular file");
    assert_eq!(
        fs::read_to_string(fixture.0.join("outside")).unwrap(),
        "unchanged"
    );
    fs::remove_file(&path).unwrap();
    let cpath = CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    let mut child = fixture
        .command(&["lock", "--update"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    if child.try_wait().unwrap().is_none() {
        child.kill().unwrap();
    }
    failure(child.wait_with_output().unwrap(), "regular file");
}

#[cfg(unix)]
#[test]
fn failed_staging_writes_preserve_the_existing_lock_and_remove_partial_files() {
    use std::os::unix::process::CommandExt;
    let fixture = Fixture::new();
    if !fixture.qualified() {
        return;
    }
    success(fixture.run(&["lock"]));
    let original = fixture.lock();
    fixture.write("math/src/lib.ox", "pub fn answer() -> i32 { return 8; }\n");
    let mut command = fixture.command(&["lock", "--update"]);
    unsafe {
        command.pre_exec(|| {
            let limit = libc::rlimit {
                rlim_cur: 32,
                rlim_max: 32,
            };
            if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::signal(libc::SIGXFSZ, libc::SIG_IGN) == libc::SIG_ERR {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    failure(command.output().unwrap(), "cannot write typed lock");
    fixture.unchanged(&original);
}

#[test]
fn stale_locks_stop_run_test_and_build_before_effects_or_artifact_replacement() {
    let fixture = Fixture::new();
    if !fixture.qualified() {
        return;
    }
    success(fixture.run(&["lock"]));
    let original = fixture.lock();
    fixture.write("app/.oxid/bin/app", "keep previous executable");
    fixture.write("math/src/lib.ox", "pub fn answer() -> i32 { return 8; }\n");
    for args in [
        &["run", "--locked", "--entry-mode=result"][..],
        &["test", "--frozen"],
        &["build", "--locked"],
        &["build"],
    ] {
        let output = fixture
            .command(args)
            .env("OXID_LLVM_BIN", fixture.0.join("missing-toolchain"))
            .output()
            .unwrap();
        failure(output, "lock is out of date");
        assert_eq!(fixture.lock(), original);
        assert_eq!(
            fs::read(fixture.0.join("app/.oxid/bin/app")).unwrap(),
            b"keep previous executable"
        );
    }
    let output = failure(
        fixture.run(&["check", "--locked", "--message-format=json"]),
        "lock is out of date",
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.lines().count(), 2, "{text}");
    assert!(
        text.lines()
            .all(|line| line.starts_with('{') && line.ends_with('}')),
        "{text}"
    );
    assert!(text.contains("\"kind\":\"check-summary\""), "{text}");
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 in OXID_LLVM_BIN"]
fn locked_native_build_uses_retained_sources_and_preserves_a_stale_rebuild() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    success(fixture.run(&["lock"]));
    let original = fixture.lock();
    let real_tools =
        std::env::var_os("OXID_LLVM_BIN").expect("set OXID_LLVM_BIN to pinned LLVM 19.1.7");
    let tools = fixture.0.join("tools");
    fs::create_dir(&tools).unwrap();
    for name in ["clang", "opt", "ld.lld"] {
        let path = tools.join(name);
        fs::write(&path, "#!/bin/sh\nif [ \"$1\" = --version ]; then\n  printf '%s\\n' 'pub fn answer() -> i32 { return 99; }' > \"$OXID_LOCK_TEST_DEP\"\nfi\nexec \"$OXID_LOCK_TEST_REAL/${0##*/}\" \"$@\"\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    success(
        fixture
            .command(&["build", "--frozen"])
            .env("OXID_LLVM_BIN", &tools)
            .env("OXID_LOCK_TEST_REAL", real_tools)
            .env("OXID_LOCK_TEST_DEP", fixture.0.join("math/src/lib.ox"))
            .output()
            .unwrap(),
    );
    let executable = fixture.0.join("app/.oxid/bin/app");
    let binary = fs::read(&executable).unwrap();
    let output = Command::new(&executable).output().unwrap();
    assert_eq!(output.status.code(), Some(7), "{output:?}");
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert!(fs::read_to_string(fixture.0.join("math/src/lib.ox"))
        .unwrap()
        .contains("99"));
    failure(fixture.run(&["build", "--locked"]), "lock is out of date");
    assert_eq!(fs::read(&executable).unwrap(), binary);
    assert_eq!(fixture.lock(), original);
    fs::remove_dir_all(fixture.0.join("math")).unwrap();
    fs::remove_dir_all(fixture.0.join("app/src")).unwrap();
    let output = Command::new(executable).output().unwrap();
    assert_eq!(output.status.code(), Some(7), "{output:?}");
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
}
