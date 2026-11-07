//! Public source-bundle contracts. Python's independent tar oracle lives in
//! scripts/verify_typed_package_bundle.py; these tests exercise the actual CLI.
#![cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{symlink, FileTypeExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-source-package-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self(root);
        fixture.write("app/oxid.toml", "[project]\nname = \"app\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n[dependencies]\nmath = \"../math\"\n[tests]\nsmoke = \"tests/smoke.ox\"\n");
        fixture.write(
            "app/src/main.ox",
            "fn main() -> i32 { return crate::math::answer(); }\n",
        );
        fixture.write("app/tests/smoke.ox", "fn main() -> i32 { return 0; }\n");
        fixture.write("math/oxid.toml", "[project]\nname = \"math\"\nedition = \"typed-preview\"\nkind = \"lib\"\n[tests]\nsmoke = \"tests/smoke.ox\"\n");
        fixture.write("math/src/lib.ox", "pub fn answer() -> i32 { return 7; }\n");
        fixture.write("math/tests/smoke.ox", "fn main() -> i32 { return 0; }\n");
        fixture
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(cwd)
            .args(args)
            .stdin(Stdio::null())
            .env("OXID_LLVM_BIN", self.0.join("absent-tools"))
            .env("OXID_CACHE_DIR", self.0.join("legacy-cache"))
            .env_remove("OXID_PATH");
        command
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command(&self.0.join("app"), args).output().unwrap()
    }
    fn outside(&self, args: &[&str]) -> Output {
        self.command(&self.0, args).output().unwrap()
    }
    fn lock(&self) {
        good(self.run(&["lock"]));
    }
    fn archive(&self) -> PathBuf {
        self.0.join("application.oxpkg.tar")
    }
    fn package(&self) -> Output {
        self.run(&[
            "package",
            "--frozen",
            "--output",
            self.archive().to_str().unwrap(),
            "--message-format=json",
        ])
    }
    fn create(&self) -> Vec<u8> {
        summary(good(self.package()), "create");
        fs::read(self.archive()).unwrap()
    }
    fn workspace(&self) {
        self.write("oxid.toml", "[workspace]\nedition = \"typed-preview\"\nmembers = [\"app\", \"math\"]\ndefault-members = [\"app\"]\n");
        // Equal display names must retain their separate canonical identities.
        let path = self.0.join("math/oxid.toml");
        let text = fs::read_to_string(&path)
            .unwrap()
            .replace("name = \"math\"", "name = \"app\"");
        fs::write(path, text).unwrap();
        good(self.outside(&["lock"]));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn good(output: Output) -> Output {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    output
}
fn bad(output: Output) -> Output {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    output
}
fn summary(output: Output, operation: &str) -> String {
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.lines().count(), 1, "{text}");
    for field in [
        "\"kind\":\"package-summary\"",
        "\"schema\":\"oxid-typed-package-summary-v1\"",
        "\"schema_version\":1",
        "\"edition\":\"typed-preview\"",
    ] {
        assert!(text.contains(field), "{text}");
    }
    assert!(
        text.contains(&format!("\"operation\":\"{operation}\"")),
        "{text}"
    );
    assert!(
        text.contains(&format!("\"success\":{}", output.status.success())),
        "{text}"
    );
    text
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

// This small fixture reader only locates ordinary payloads for mutation tests.
// The separate Python oracle independently checks complete ustar correctness.
fn entries(bytes: &[u8]) -> Vec<(String, usize, usize, usize)> {
    let mut offset = 0;
    let mut entries = Vec::new();
    while bytes[offset..offset + 512].iter().any(|byte| *byte != 0) {
        let header = &bytes[offset..offset + 512];
        let field = |range: std::ops::Range<usize>| {
            let value = &header[range];
            String::from_utf8(
                value[..value
                    .iter()
                    .position(|byte| *byte == 0)
                    .unwrap_or(value.len())]
                    .to_vec(),
            )
            .unwrap()
        };
        let name = field(0..100);
        let prefix = field(345..500);
        let path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let size = usize::from_str_radix(field(124..136).trim(), 8).unwrap();
        entries.push((path, offset, offset + 512, size));
        offset += 512 + size.div_ceil(512) * 512;
    }
    entries
}
fn files(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    entries(bytes)
        .into_iter()
        .map(|(path, _, start, size)| (path, bytes[start..start + size].to_vec()))
        .collect()
}
fn checksum(header: &mut [u8]) {
    header[148..156].fill(b' ');
    let sum: u32 = header.iter().map(|byte| u32::from(*byte)).sum();
    header[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
}
fn bounded(mut command: Command) -> Output {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!("package command exceeded its bounded fixture deadline: {output:?}");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn creation_always_requires_a_matching_existing_lock_and_reports_the_archive_digest() {
    let fixture = Fixture::new();
    for flag in [None, Some("--offline"), Some("--locked"), Some("--frozen")] {
        let archive = fixture.archive();
        let mut args = vec!["package", "--output", archive.to_str().unwrap()];
        if let Some(flag) = flag {
            args.push(flag);
        }
        bad(fixture.run(&args));
        assert!(!fixture.archive().exists());
        assert!(!fixture.0.join("app/oxid.lock").exists());
    }
    fixture.lock();
    let lock = fs::read(fixture.0.join("app/oxid.lock")).unwrap();
    let output = summary(good(fixture.package()), "create");
    let archive = fs::read(fixture.archive()).unwrap();
    assert!(
        output.contains(&format!("\"sha256\":\"{}\"", hash(&archive))),
        "{output}"
    );
    assert!(output.contains("\"root\":\"source/app\""), "{output}");
    assert!(
        output.contains(&format!("\"bytes\":{}", archive.len())),
        "{output}"
    );
    assert_eq!(fs::read(fixture.0.join("app/oxid.lock")).unwrap(), lock);
    assert!(!fixture.0.join("legacy-cache").exists());
    assert!(!fixture.0.join("app/.oxid/bin").exists());
}

#[test]
fn ordinary_graph_checks_release_manifest_bytes_and_packaging_enforces_its_own_cap() {
    use std::os::unix::process::CommandExt;

    fn constrained(mut command: Command) -> Output {
        // Limit only the exec'd compiler; other concurrently running Rust tests
        // and their fixtures keep the test runner's ordinary resource limits.
        unsafe {
            command.pre_exec(|| {
                let address = libc::rlimit {
                    rlim_cur: 48 * 1024 * 1024,
                    rlim_max: 48 * 1024 * 1024,
                };
                let core = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                if libc::setrlimit(libc::RLIMIT_AS, &address) != 0
                    || libc::setrlimit(libc::RLIMIT_CORE, &core) != 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        command.output().unwrap()
    }

    let fixture = Fixture::new();
    fixture.lock();
    let previous_archive = fixture.create();
    let mut root =
        String::from("[project]\nname = \"app\"\nedition = \"typed-preview\"\n[dependencies]\n");
    // The graph is within ordinary package/source limits. Its raw manifests
    // total 48 MiB, so retaining all of them defeats the 48 MiB process limit;
    // their tiny parsed metadata and source programs fit comfortably.
    for index in 0..64 {
        let name = format!("lib{index:02}");
        root.push_str(&format!("{name} = \"../{name}\"\n"));
        let mut manifest =
            format!("[project]\nname = \"{name}\"\nedition = \"typed-preview\"\nkind = \"lib\"\n#");
        manifest.push_str(&"x".repeat(768 * 1024 - manifest.len() - 1));
        manifest.push('\n');
        assert_eq!(manifest.len(), 768 * 1024);
        fixture.write(&format!("{name}/oxid.toml"), &manifest);
        fixture.write(
            &format!("{name}/src/lib.ox"),
            "pub fn value() -> i32 { return 0; }\n",
        );
    }
    fixture.write("app/oxid.toml", &root);
    fixture.write("app/src/main.ox", "fn main() -> i32 { return 0; }\n");
    let cwd = fixture.0.join("app");
    fs::remove_file(cwd.join("oxid.lock")).unwrap();
    let checked = good(constrained(
        fixture.command(&cwd, &["check", "--message-format=json"]),
    ));
    assert!(checked.stderr.is_empty(), "{checked:?}");
    assert!(
        String::from_utf8_lossy(&checked.stdout).contains("\"functions\":65"),
        "the complete graph must be checked: {checked:?}"
    );
    good(constrained(fixture.command(&cwd, &["lock", "--update"])));
    let lock = fs::read(cwd.join("oxid.lock")).unwrap();
    let archive = fixture.archive();
    let rejected = summary(
        bad(constrained(fixture.command(
            &cwd,
            &[
                "package",
                "--frozen",
                "--output",
                archive.to_str().unwrap(),
                "--message-format=json",
            ],
        ))),
        "create",
    );
    assert!(rejected.contains("manifest"), "{rejected}");
    assert!(rejected.contains("8 MiB"), "{rejected}");
    assert_eq!(fs::read(archive).unwrap(), previous_archive);
    assert_eq!(fs::read(cwd.join("oxid.lock")).unwrap(), lock);
}

#[test]
fn source_roster_includes_normal_and_dependency_test_modules_but_no_incidental_files() {
    let fixture = Fixture::new();
    fixture.write(
        "app/tests/smoke.ox",
        "mod helper;\nfn main() -> i32 { return crate::helper::status(); }\n",
    );
    fixture.write(
        "app/tests/helper.ox",
        "pub fn status() -> i32 { return 99; }\n",
    );
    fixture.write("math/oxid.toml", "[project]\nname = \"math\"\nedition = \"typed-preview\"\nkind = \"lib\"\nentry = \".oxid/source/lib.ox\"\n[tests]\nsmoke = \"target/tests/smoke.ox\"\n");
    fixture.write(
        "math/.oxid/source/lib.ox",
        "mod value;\npub fn answer() -> i32 { return crate::value::get(); }\n",
    );
    fixture.write(
        "math/.oxid/source/value.ox",
        "pub fn get() -> i32 { return 7; }\n",
    );
    fixture.write(
        "math/target/tests/smoke.ox",
        "mod helper;\nfn main() -> i32 { return crate::helper::status(); }\n",
    );
    fixture.write(
        "math/target/tests/helper.ox",
        "pub fn status() -> i32 { return 88; }\n",
    );
    for path in [
        "app/.env",
        "app/.git/config",
        "app/.oxid/bin/old",
        "math/unloaded.ox",
        "math/target/cache",
    ] {
        fixture.write(path, "not a source input; do not archive\n");
    }
    fixture.lock();
    let archived = files(&fixture.create());
    let expected = [
        "app/oxid.toml",
        "app/oxid.lock",
        "app/src/main.ox",
        "app/tests/smoke.ox",
        "app/tests/helper.ox",
        "math/oxid.toml",
        "math/.oxid/source/lib.ox",
        "math/.oxid/source/value.ox",
        "math/target/tests/smoke.ox",
        "math/target/tests/helper.ox",
    ];
    assert_eq!(archived.len(), expected.len() + 1);
    for path in expected {
        assert_eq!(
            archived[&format!("source/{path}")],
            fs::read(fixture.0.join(path)).unwrap()
        );
    }
    // Both test entry bodies return nonzero: successful creation cannot have
    // executed them. The selected native tool directory is deliberately absent.
    assert!(!fixture.0.join("legacy-cache").exists());
}

#[test]
fn default_output_and_repeated_relocated_creation_are_deterministic() {
    let mut fixture = Fixture::new();
    fixture.lock();
    good(fixture.run(&["package"]));
    let default = fs::read(fixture.0.join("app/.oxid/package/source.oxpkg.tar")).unwrap();
    assert_eq!(fixture.create(), default);
    fs::set_permissions(
        fixture.0.join("math/src/lib.ox"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let source = fs::read(fixture.0.join("app/src/main.ox")).unwrap();
    fs::write(fixture.0.join("app/src/main.ox"), source).unwrap();
    assert_eq!(fixture.create(), default);
    let relocated = fixture.0.with_extension("relocated");
    fs::rename(&fixture.0, &relocated).unwrap();
    fixture.0 = relocated;
    assert_eq!(fixture.create(), default);
}

#[test]
fn verification_and_extraction_work_after_original_source_deletion() {
    let fixture = Fixture::new();
    fixture.lock();
    let bytes = fixture.create();
    fs::remove_dir_all(fixture.0.join("app")).unwrap();
    fs::remove_dir_all(fixture.0.join("math")).unwrap();
    let archive = fixture.archive();
    let hex = hash(&bytes).to_ascii_uppercase();
    let verified = summary(
        good(fixture.outside(&[
            "package",
            "--verify",
            archive.to_str().unwrap(),
            "--sha256",
            &hex,
            "--message-format=json",
        ])),
        "verify",
    );
    assert!(verified.contains("\"output\":null"), "{verified}");
    let destination = fixture.0.join("recipient");
    summary(
        good(fixture.outside(&[
            "package",
            "--extract",
            archive.to_str().unwrap(),
            "--output",
            destination.to_str().unwrap(),
            "--sha256",
            &hex,
            "--message-format=json",
        ])),
        "extract",
    );
    let root = destination.join("source/app");
    good(
        fixture
            .command(&root, &["check", "--frozen", "--offline"])
            .output()
            .unwrap(),
    );
    good(
        fixture
            .command(&root, &["test", "--frozen", "--offline"])
            .output()
            .unwrap(),
    );
    let ran = good(
        fixture
            .command(
                &root,
                &[
                    "run",
                    "--frozen",
                    "--entry-mode=result",
                    "--message-format=json",
                ],
            )
            .output()
            .unwrap(),
    );
    assert!(
        String::from_utf8_lossy(&ran.stdout).contains("\"value\":7"),
        "{ran:?}"
    );
}

#[test]
fn stale_locks_manifest_changes_and_invalid_sources_preserve_the_previous_archive() {
    let fixture = Fixture::new();
    fixture.lock();
    let original = fixture.create();
    for (path, invalid) in [
        ("app/src/main.ox", "fn broken(\n"),
        ("app/tests/smoke.ox", "fn main() -> i32 { return false; }\n"),
        (
            "math/tests/smoke.ox",
            "fn main(value: i32) -> i32 { return value; }\n",
        ),
        ("math/src/lib.ox", "pub fn answer() -> i32 { return 8; }\n"),
    ] {
        let saved = fs::read(fixture.0.join(path)).unwrap();
        fixture.write(path, invalid);
        summary(bad(fixture.package()), "create");
        assert_eq!(fs::read(fixture.archive()).unwrap(), original, "{path}");
        fs::write(fixture.0.join(path), saved).unwrap();
    }
    for path in ["app/oxid.toml", "math/oxid.toml"] {
        let saved = fs::read_to_string(fixture.0.join(path)).unwrap();
        fixture.write(path, &(saved.clone() + "# changed raw manifest\n"));
        bad(fixture.package());
        assert_eq!(fs::read(fixture.archive()).unwrap(), original);
        fixture.write(path, &saved);
    }
    let lock = fs::read(fixture.0.join("app/oxid.lock")).unwrap();
    for bytes in [b"version = 1\n".as_slice(), &lock[..lock.len() / 2]] {
        fs::write(fixture.0.join("app/oxid.lock"), bytes).unwrap();
        bad(fixture.package());
        assert_eq!(fs::read(fixture.archive()).unwrap(), original);
        assert_eq!(fs::read(fixture.0.join("app/oxid.lock")).unwrap(), bytes);
    }
}

#[test]
fn editable_root_sources_and_explicit_dependency_update_produce_new_valid_archives() {
    let fixture = Fixture::new();
    fixture.lock();
    let first = fixture.create();
    let lock = fs::read(fixture.0.join("app/oxid.lock")).unwrap();
    fixture.write(
        "app/src/main.ox",
        "fn main() -> i32 { return crate::math::answer() + 1; }\n",
    );
    let root_changed = fixture.create();
    assert_ne!(root_changed, first);
    assert_eq!(fs::read(fixture.0.join("app/oxid.lock")).unwrap(), lock);
    fixture.write("math/src/lib.ox", "pub fn answer() -> i32 { return 9; }\n");
    bad(fixture.package());
    assert_eq!(fs::read(fixture.archive()).unwrap(), root_changed);
    good(fixture.run(&["lock", "--update"]));
    assert_ne!(fixture.create(), root_changed);
}

#[test]
fn workspace_packages_every_member_even_from_a_library_or_with_defaults() {
    let fixture = Fixture::new();
    fixture.workspace();
    let archive = fixture.archive();
    let from_root = summary(
        good(fixture.outside(&[
            "package",
            "--workspace",
            "--output",
            archive.to_str().unwrap(),
            "--message-format=json",
        ])),
        "create",
    );
    assert!(from_root.contains("\"root\":\"source\""));
    let bytes = fs::read(&archive).unwrap();
    let archived = files(&bytes);
    for name in [
        "source/oxid.toml",
        "source/oxid.lock",
        "source/app/src/main.ox",
        "source/math/src/lib.ox",
        "source/math/tests/smoke.ox",
    ] {
        assert!(archived.contains_key(name), "{name}");
    }
    good(
        fixture
            .command(
                &fixture.0.join("math"),
                &["package", "--output", archive.to_str().unwrap()],
            )
            .output()
            .unwrap(),
    );
    assert_eq!(fs::read(&archive).unwrap(), bytes);
    fixture.write("app/src/main.ox", "fn main() -> i32 { return false; }\n");
    bad(fixture
        .command(
            &fixture.0.join("math"),
            &["package", "--output", archive.to_str().unwrap()],
        )
        .output()
        .unwrap());
    assert_eq!(fs::read(archive).unwrap(), bytes);
}

#[test]
fn strict_package_options_fail_before_output_or_project_work() {
    let fixture = Fixture::new();
    for args in [
        vec!["package", "--update"],
        vec!["package", "--incremental"],
        vec!["package", "--package", "app"],
        vec!["package", "-p", "app"],
        vec!["package", "--entry-mode=result"],
        vec!["package", "--edition=legacy-0.9"],
        vec!["package", "--verify"],
        vec!["package", "--extract", "missing"],
        vec!["package", "--verify", "missing", "--frozen"],
        vec!["package", "--verify", "missing", "--output", "new"],
        vec!["package", "--extract", "missing", "--verify", "missing"],
        vec!["package", "--sha256", "00"],
        vec!["package", "--locked", "--locked"],
        vec!["package", "--output", "first", "--output", "second"],
    ] {
        bad(fixture.outside(&args));
        for name in [
            "new",
            "first",
            "second",
            ".oxid",
            "app/.oxid",
            "legacy-cache",
        ] {
            assert!(!fixture.0.join(name).exists(), "{args:?}: {name}");
        }
    }
}

#[test]
fn package_operands_do_not_become_a_package_command() {
    let fixture = Fixture::new();
    fixture.lock();
    let bytes = fixture.create();
    fixture.write("package", "operand sentinel");
    // A valid archive named `package` would make an accidental extract/verify
    // dispatch succeed; testing just a nonexistent operand would miss it.
    fs::write(fixture.0.join("package"), &bytes).unwrap();
    for args in [
        vec!["--extract", "package", "--output", "recipient"],
        vec!["--verify", "package"],
        vec!["--output", "package"],
        vec![
            "check",
            "package",
            "--edition=typed-preview",
            "--message-format=json",
        ],
    ] {
        let output = fixture.outside(&args);
        assert!(!output.status.success(), "{args:?}: {output:?}");
        assert!(
            !String::from_utf8_lossy(&output.stdout).contains("package-summary"),
            "operand was reinterpreted as a package command: {args:?}: {output:?}"
        );
        assert_eq!(fs::read(fixture.0.join("package")).unwrap(), bytes);
        assert!(!fixture.0.join("recipient").exists());
    }
    summary(
        good(fixture.outside(&[
            "--edition=typed-preview",
            "--message-format",
            "json",
            "package",
            "--verify",
            "package",
        ])),
        "verify",
    );
}

#[test]
fn input_collisions_and_special_output_paths_cannot_destroy_user_files() {
    let fixture = Fixture::new();
    fixture.lock();
    for relative in [
        "app/oxid.toml",
        "app/oxid.lock",
        "app/src/main.ox",
        "math/src/lib.ox",
    ] {
        let path = fixture.0.join(relative);
        let before = fs::read(&path).unwrap();
        bad(fixture.run(&["package", "--output", path.to_str().unwrap()]));
        assert_eq!(fs::read(&path).unwrap(), before);
        let alias = fixture.0.join("hardlinked-output.tar");
        fs::hard_link(&path, &alias).unwrap();
        bad(fixture.run(&["package", "--output", alias.to_str().unwrap()]));
        assert_eq!(fs::read(&alias).unwrap(), before);
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_file(alias).unwrap();
    }
    fixture.write("sentinel", "preserve me");
    let output = fixture.archive();
    symlink(fixture.0.join("sentinel"), &output).unwrap();
    bad(fixture.package());
    assert_eq!(
        fs::read(fixture.0.join("sentinel")).unwrap(),
        b"preserve me"
    );
    fs::remove_file(&output).unwrap();
    fs::create_dir(&output).unwrap();
    bad(fixture.package());
    assert!(output.is_dir());
    fs::remove_dir(&output).unwrap();
    use std::os::unix::ffi::OsStrExt;
    let encoded = std::ffi::CString::new(output.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(encoded.as_ptr(), 0o600) }, 0);
    bad(bounded(fixture.command(
        &fixture.0.join("app"),
        &["package", "--output", output.to_str().unwrap()],
    )));
    assert!(fs::symlink_metadata(output).unwrap().file_type().is_fifo());
}

#[test]
fn unbundled_ancestor_discovery_inputs_and_hardlink_aliases_are_preserved() {
    let fixture = Fixture::new();
    fixture.lock();
    fixture.write("oxid.toml", "[project]\nname = \"unrelated ancestor\"\n");
    fixture.write("oxid.lock", "version = 1\n");
    let archive = fixture.create();
    let archived = files(&archive);
    assert!(!archived.contains_key("source/oxid.toml"));
    assert!(!archived.contains_key("source/oxid.lock"));
    for name in ["oxid.toml", "oxid.lock"] {
        let path = fixture.0.join(name);
        let before = fs::read(&path).unwrap();
        bad(fixture.run(&["package", "--output", &format!("../{name}")]));
        assert_eq!(fs::read(&path).unwrap(), before);
        let alias = fixture.0.join("ancestor-hardlink.tar");
        fs::hard_link(&path, &alias).unwrap();
        bad(fixture.run(&["package", "--output", alias.to_str().unwrap()]));
        assert_eq!(fs::read(&alias).unwrap(), before);
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_file(alias).unwrap();
    }
    assert_eq!(fs::read(fixture.archive()).unwrap(), archive);
}

#[test]
fn existing_extraction_paths_and_wrong_digest_are_never_replaced() {
    let fixture = Fixture::new();
    fixture.lock();
    fixture.create();
    let archive = fixture.archive();
    let destination = fixture.0.join("recipient");
    let wrong = "0".repeat(64);
    summary(
        bad(fixture.outside(&[
            "package",
            "--extract",
            archive.to_str().unwrap(),
            "--output",
            destination.to_str().unwrap(),
            "--sha256",
            &wrong,
            "--message-format=json",
        ])),
        "extract",
    );
    assert!(!destination.exists());
    fs::create_dir(&destination).unwrap();
    bad(fixture.outside(&[
        "package",
        "--extract",
        archive.to_str().unwrap(),
        "--output",
        destination.to_str().unwrap(),
    ]));
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
    fs::remove_dir(&destination).unwrap();
    fs::write(&destination, b"existing file").unwrap();
    bad(fixture.outside(&[
        "package",
        "--extract",
        archive.to_str().unwrap(),
        "--output",
        destination.to_str().unwrap(),
    ]));
    assert_eq!(fs::read(&destination).unwrap(), b"existing file");
    fs::remove_file(&destination).unwrap();
    symlink(fixture.0.join("missing-target"), &destination).unwrap();
    bad(fixture.outside(&[
        "package",
        "--extract",
        archive.to_str().unwrap(),
        "--output",
        destination.to_str().unwrap(),
    ]));
    assert!(fs::symlink_metadata(destination)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(!fixture.0.join("missing-target").exists());
}

#[test]
fn archive_corruption_unsafe_headers_overflow_and_trailing_data_fail_without_extraction() {
    let fixture = Fixture::new();
    fixture.lock();
    let valid = fixture.create();
    let (_, header, data, size) = entries(&valid)[1].clone();
    let mut mutations = Vec::new();
    for offset in [148, data + size - 1] {
        let mut changed = valid.clone();
        changed[offset] ^= 1;
        mutations.push(changed);
    }
    mutations.push(valid[..valid.len() - 1].to_vec());
    let mut trailing = valid.clone();
    trailing.extend_from_slice(&[0; 512]);
    mutations.push(trailing);
    for path in [
        "../escape",
        "/absolute",
        "source/../escape",
        "source\\escape",
    ] {
        let mut changed = valid.clone();
        changed[header..header + 100].fill(0);
        changed[header..header + path.len()].copy_from_slice(path.as_bytes());
        changed[header + 345..header + 500].fill(0);
        checksum(&mut changed[header..header + 512]);
        mutations.push(changed);
    }
    for kind in *b"1235xL" {
        let mut changed = valid.clone();
        changed[header + 156] = kind;
        checksum(&mut changed[header..header + 512]);
        mutations.push(changed);
    }
    let mut huge = valid.clone();
    huge[header + 124..header + 136].copy_from_slice(b"77777777777\0");
    checksum(&mut huge[header..header + 512]);
    mutations.push(huge);
    for (index, bytes) in mutations.into_iter().enumerate() {
        let archive = fixture.0.join(format!("corrupt-{index}.tar"));
        let output = fixture.0.join(format!("recipient-{index}"));
        fs::write(&archive, bytes).unwrap();
        summary(
            bad(bounded(fixture.command(
                &fixture.0,
                &[
                    "package",
                    "--verify",
                    archive.to_str().unwrap(),
                    "--message-format=json",
                ],
            ))),
            "verify",
        );
        summary(
            bad(bounded(fixture.command(
                &fixture.0,
                &[
                    "package",
                    "--extract",
                    archive.to_str().unwrap(),
                    "--output",
                    output.to_str().unwrap(),
                    "--message-format=json",
                ],
            ))),
            "extract",
        );
        assert!(!output.exists());
    }
    assert!(!fixture.0.join("escape").exists());
}

#[test]
fn format_verification_does_not_execute_or_replace_staged_source_readmission() {
    let fixture = Fixture::new();
    fixture.lock();
    let mut bytes = fixture.create();
    let roster = entries(&bytes);
    let (_, _, start, size) = roster
        .iter()
        .find(|(path, _, _, _)| path == "source/app/src/main.ox")
        .unwrap();
    let old = hash(&bytes[*start..*start + *size]);
    bytes[*start] = b'!';
    let new = hash(&bytes[*start..*start + *size]);
    let (_, _, inventory_start, inventory_size) = &roster[0];
    let inventory =
        String::from_utf8(bytes[*inventory_start..*inventory_start + *inventory_size].to_vec())
            .unwrap();
    assert_eq!(inventory.matches(&old).count(), 1);
    let inventory = inventory.replace(&old, &new);
    bytes[*inventory_start..*inventory_start + *inventory_size]
        .copy_from_slice(inventory.as_bytes());
    let archive = fixture.0.join("coherent-but-invalid-source.tar");
    fs::write(&archive, bytes).unwrap();
    summary(
        good(fixture.outside(&[
            "package",
            "--verify",
            archive.to_str().unwrap(),
            "--message-format=json",
        ])),
        "verify",
    );
    let destination = fixture.0.join("recipient");
    summary(
        bad(fixture.outside(&[
            "package",
            "--extract",
            archive.to_str().unwrap(),
            "--output",
            destination.to_str().unwrap(),
            "--message-format=json",
        ])),
        "extract",
    );
    assert!(!destination.exists());
}

#[test]
fn nonportable_dependency_and_source_path_aliases_are_refused() {
    let fixture = Fixture::new();
    let manifest = fixture.0.join("app/oxid.toml");
    let original = fs::read_to_string(&manifest).unwrap();
    fs::create_dir(fixture.0.join("hop")).unwrap();
    symlink(fixture.0.join("math"), fixture.0.join("alias")).unwrap();
    for path in [
        fixture.0.join("math").to_str().unwrap().to_owned(),
        "./../math".into(),
        "../hop/../math".into(),
        "../alias".into(),
    ] {
        fs::write(&manifest, original.replace("../math", &path)).unwrap();
        let lock = fixture.0.join("app/oxid.lock");
        if lock.exists() {
            fs::remove_file(lock).unwrap();
        }
        fixture.lock();
        bad(fixture.package());
        assert!(!fixture.archive().exists());
    }
    fs::write(&manifest, original.replace("src/main.ox", "./src/main.ox")).unwrap();
    fs::remove_file(fixture.0.join("app/oxid.lock")).unwrap();
    fixture.lock();
    bad(fixture.package());
    assert!(!fixture.archive().exists());
}

#[test]
fn concurrent_creation_publishes_only_a_complete_archive() {
    let fixture = Fixture::new();
    fixture.lock();
    let expected = fixture.create();
    // Existing regular output may be replaced, but no writer may expose its
    // unfinished archive or accidentally remove another writer's staging.
    fs::write(fixture.archive(), b"old regular archive").unwrap();
    let archive = fixture.archive();
    let args = ["package", "--output", archive.to_str().unwrap()];
    let first = fixture
        .command(&fixture.0.join("app"), &args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let second = fixture
        .command(&fixture.0.join("app"), &args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    good(first.wait_with_output().unwrap());
    good(second.wait_with_output().unwrap());
    assert_eq!(fs::read(&archive).unwrap(), expected);
    good(fixture.outside(&["package", "--verify", archive.to_str().unwrap()]));
}

#[test]
fn concurrent_extraction_has_one_winner_and_no_partial_destination() {
    let fixture = Fixture::new();
    fixture.lock();
    let bytes = fixture.create();
    let archive = fixture.archive();
    let output = fixture.0.join("recipient");
    let args = [
        "package",
        "--extract",
        archive.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ];
    let first = fixture
        .command(&fixture.0, &args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let second = fixture
        .command(&fixture.0, &args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut statuses = [
        first.wait_with_output().unwrap().status.code().unwrap(),
        second.wait_with_output().unwrap().status.code().unwrap(),
    ];
    statuses.sort();
    assert_eq!(statuses, [0, 1]);
    for (path, contents) in files(&bytes) {
        if path != "OXID-PACKAGE.toml" {
            assert_eq!(fs::read(output.join(path)).unwrap(), contents);
        }
    }
}
