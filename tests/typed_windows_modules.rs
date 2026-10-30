//! Actual Windows local-NTFS module behavior through the public CLI.
#![cfg(all(
    target_os = "windows",
    target_arch = "x86_64",
    target_pointer_width = "64",
    target_endian = "little"
))]

use std::{
    fs,
    os::windows::fs::{symlink_file, OpenOptionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const MAIN: &str = "mod child; fn main()->i32{return crate::child::value();}\n";
const CHILD: &str = "pub fn value()->i32{return 42;}\n";

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-windows-\u{65e5}\u{672c}\u{8a9e}-\u{1f980}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, bytes: impl AsRef<[u8]>) {
        fs::write(self.0.join(name), bytes).unwrap();
    }
    fn program(&self) {
        self.write("main.ox", MAIN);
        self.write("child.ox", CHILD);
    }
    fn command(&self, operation: &str, entry: &Path, json: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(&self.0)
            .arg(operation)
            .arg(entry)
            .arg("--edition=typed-preview")
            .env("OXID_LLVM_BIN", self.0.join("no-native-tools"))
            .stdin(Stdio::null());
        if operation == "run" {
            command.arg("--entry-mode=result");
        }
        if json {
            command.arg("--message-format=json");
        }
        command
    }
    fn invoke(&self, operation: &str, entry: &Path, json: bool) -> Output {
        self.command(operation, entry, json).output().unwrap()
    }
    fn check(&self) -> Output {
        self.invoke("check", Path::new("main.ox"), true)
    }
    fn result(&self) {
        let output = self.invoke("run", Path::new("main.ox"), false);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert_eq!(output.stdout, b"42\n");
        assert!(output.stderr.is_empty(), "{output:?}");
        self.no_artifacts();
    }
    fn no_artifacts(&self) {
        for path in [
            ".oxid",
            "target",
            "program",
            "program.exe",
            "output",
            "oxid.lock",
        ] {
            assert!(!self.0.join(path).exists(), "unexpected {path}");
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn success(output: Output) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}
fn diagnostic(output: Output, code: &str, message: &str) -> String {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains(&format!("\"code\":\"{code}\"")), "{text}");
    assert!(text.contains(message), "{text}");
    assert!(text.contains("\"success\":false"), "{text}");
    assert!(!text.contains("E0500"), "{text}");
    text
}
fn junction(link: &Path, target: &Path) {
    let output = Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "actual Windows junction creation is required for qualification: {output:?}"
    );
}

fn local_packages() -> Fixture {
    let fixture = Fixture::new();
    for path in ["app/src", "math/src"] {
        fs::create_dir_all(fixture.0.join(path)).unwrap();
    }
    fs::write(fixture.0.join("app/oxid.toml"), "[project]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[dependencies]\nmath=\"../math\"\n").unwrap();
    fs::write(fixture.0.join("math/oxid.toml"), "[project]\nname=\"math\"\nversion=\"0.1.0\"\nedition=\"typed-preview\"\nkind=\"lib\"\nentry=\"src/lib.ox\"\n").unwrap();
    fs::write(
        fixture.0.join("app/src/main.ox"),
        "fn main()->i32{return crate::math::value();}\n",
    )
    .unwrap();
    fs::write(fixture.0.join("math/src/lib.ox"), CHILD).unwrap();
    fixture
}

fn package_command(fixture: &Fixture, directory: &str, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
    command
        .current_dir(fixture.0.join(directory))
        .args(args)
        .env("OXID_LLVM_BIN", fixture.0.join("no-native-tools"))
        .env("OXID_CACHE_DIR", fixture.0.join("cache"))
        .stdin(Stdio::null());
    command
}

fn package_output(fixture: &Fixture, directory: &str, args: &[&str]) -> Output {
    package_command(fixture, directory, args).output().unwrap()
}

fn package_refusal(output: Output) -> String {
    assert!(!output.status.success(), "{output:?}");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!text.contains("E0500"), "{text}");
    assert!(
        !text.contains("E0100"),
        "redirected bytes must not reach the parser: {text}"
    );
    text
}

fn no_package_outputs(fixture: &Fixture) {
    for base in ["", "app", "math"] {
        for path in [".oxid", "target", "cache", "download-cache"] {
            assert!(
                !fixture.0.join(base).join(path).exists(),
                "unexpected {base}/{path}"
            );
        }
        for entry in fs::read_dir(fixture.0.join(base)).unwrap() {
            assert!(!entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".oxid-lock-"));
        }
    }
}

#[test]
fn real_ntfs_local_packages_without_child_modules_lock_run_and_relocate() {
    let fixture = local_packages();
    success(package_output(&fixture, "app", &["check"]));
    assert!(!fixture.0.join("app/oxid.lock").exists());
    let result = package_output(&fixture, "app", &["run", "--entry-mode=result"]);
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert_eq!(result.stdout, b"42\n");
    assert!(result.stderr.is_empty(), "{result:?}");
    success(package_output(&fixture, "app", &["lock"]));
    let lock = fs::read(fixture.0.join("app/oxid.lock")).unwrap();
    success(package_output(&fixture, "app", &["check", "--frozen"]));
    let moved = fixture.0.with_extension("relocated");
    fs::rename(&fixture.0, &moved).unwrap();
    let relocated = Fixture(moved);
    success(package_output(&relocated, "app", &["lock"]));
    assert_eq!(fs::read(relocated.0.join("app/oxid.lock")).unwrap(), lock);
    success(package_output(&relocated, "app", &["check", "--frozen"]));
    no_package_outputs(&relocated);
}

#[test]
fn zero_dependency_package_entries_cannot_bypass_ntfs_verification() {
    let fixture = local_packages();
    fs::write(
        fixture.0.join("app/oxid.toml"),
        "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n",
    )
    .unwrap();
    fs::write(
        fixture.0.join("app/src/main.ox"),
        "fn main()->i32{return 42;}\n",
    )
    .unwrap();
    success(package_output(&fixture, "app", &["check"]));
    fs::rename(fixture.0.join("app/src"), fixture.0.join("app/ordinary")).unwrap();
    junction(&fixture.0.join("app/src"), &fixture.0.join("app/ordinary"));
    let text = package_refusal(package_output(&fixture, "app", &["check"]));
    assert!(
        text.contains("reparse") || text.contains("symbolic link"),
        "{text}"
    );
    assert!(!fixture.0.join("app/oxid.lock").exists());
    no_package_outputs(&fixture);
    fs::remove_dir(fixture.0.join("app/src")).unwrap();
    fs::rename(fixture.0.join("app/ordinary"), fixture.0.join("app/src")).unwrap();
    success(package_output(&fixture, "app", &["check"]));
    no_package_outputs(&fixture);
}

#[test]
fn package_entries_reject_actual_inside_and_outside_junctions_without_parsing_targets() {
    for inside in [true, false] {
        let fixture = local_packages();
        let external = Fixture::new();
        let target = if inside {
            let path = fixture.0.join("app/ordinary");
            fs::create_dir(&path).unwrap();
            path
        } else {
            external.0.clone()
        };
        fs::write(target.join("main.ox"), "invalid redirected source").unwrap();
        fs::remove_dir_all(fixture.0.join("app/src")).unwrap();
        junction(&fixture.0.join("app/src"), &target);
        let text = package_refusal(package_output(&fixture, "app", &["check"]));
        assert!(
            text.contains("reparse") || text.contains("symbolic link"),
            "{text}"
        );
        assert!(!fixture.0.join("app/oxid.lock").exists());
        no_package_outputs(&fixture);
        external.no_artifacts();
        fs::remove_dir(fixture.0.join("app/src")).unwrap();
        fs::create_dir(fixture.0.join("app/src")).unwrap();
        fs::write(
            fixture.0.join("app/src/main.ox"),
            "fn main()->i32{return crate::math::value();}\n",
        )
        .unwrap();
        success(package_output(&fixture, "app", &["check"]));
    }
}

#[test]
fn declared_package_root_aliases_keep_one_canonical_nominal_identity() {
    let fixture = local_packages();
    fs::write(fixture.0.join("app/oxid.toml"), "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[dependencies]\nfirst=\"../math\"\nsecond=\"../alias\"\n").unwrap();
    fs::write(
        fixture.0.join("math/src/lib.ox"),
        "pub struct Token{pub value:i32}pub fn make()->Token{return Token{value:42};}\n",
    )
    .unwrap();
    fs::write(fixture.0.join("app/src/main.ox"), "use crate::first::Token;fn main()->i32{let value:Token=crate::second::make();return value.value;}\n").unwrap();
    junction(&fixture.0.join("alias"), &fixture.0.join("math"));
    success(package_output(&fixture, "app", &["check"]));
    let result = package_output(&fixture, "app", &["run", "--entry-mode=result"]);
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert_eq!(result.stdout, b"42\n");
    assert!(result.stderr.is_empty(), "{result:?}");
    fs::remove_dir(fixture.0.join("alias")).unwrap();
    no_package_outputs(&fixture);
}

#[test]
fn package_entry_and_manifest_symlinks_preserve_targets_and_recover_after_refusal() {
    let fixture = local_packages();
    let external = Fixture::new();
    external.write("invalid.ox", "invalid redirected source");
    let entry = fixture.0.join("math/src/lib.ox");
    fs::remove_file(&entry).unwrap();
    symlink_file(external.0.join("invalid.ox"), &entry)
        .expect("actual Windows file symlinks are required for package qualification");
    let text = package_refusal(package_output(&fixture, "app", &["check"]));
    assert!(
        text.contains("reparse") || text.contains("symbolic link"),
        "{text}"
    );
    fs::remove_file(&entry).unwrap();
    fs::write(&entry, CHILD).unwrap();
    let manifest = fixture.0.join("math/oxid.toml");
    let original = fs::read(&manifest).unwrap();
    external.write("manifest.toml", &original);
    fs::remove_file(&manifest).unwrap();
    symlink_file(external.0.join("manifest.toml"), &manifest).unwrap();
    let text = package_refusal(package_output(&fixture, "app", &["check"]));
    assert!(
        text.contains("regular file") || text.contains("reparse"),
        "{text}"
    );
    assert_eq!(
        fs::read(external.0.join("manifest.toml")).unwrap(),
        original
    );
    fs::remove_file(&manifest).unwrap();
    fs::write(&manifest, &original).unwrap();
    success(package_output(&fixture, "app", &["check"]));
    no_package_outputs(&fixture);
    external.no_artifacts();
}

#[test]
fn package_lock_and_manifest_native_read_failures_do_not_hold_handles_or_publish_stages() {
    let fixture = local_packages();
    success(package_output(&fixture, "app", &["lock"]));
    let lock_path = fixture.0.join("app/oxid.lock");
    let original = fs::read(&lock_path).unwrap();
    let manifest_path = fixture.0.join("app/oxid.toml");
    for path in [&lock_path, &manifest_path] {
        let exclusive = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(path)
            .unwrap();
        let text = package_refusal(package_output(&fixture, "app", &["check", "--locked"]));
        assert!(text.contains("cannot"), "{text}");
        drop(exclusive);
        assert_eq!(fs::read(&lock_path).unwrap(), original);
        success(package_output(&fixture, "app", &["check", "--locked"]));
        no_package_outputs(&fixture);
    }
    let external = Fixture::new();
    external.write("lock", &original);
    fs::remove_file(&lock_path).unwrap();
    symlink_file(external.0.join("lock"), &lock_path).unwrap();
    package_refusal(package_output(&fixture, "app", &["lock", "--update"]));
    assert!(fs::symlink_metadata(&lock_path)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read(external.0.join("lock")).unwrap(), original);
    fs::remove_file(&lock_path).unwrap();
    fs::write(&lock_path, &original).unwrap();
    success(package_output(&fixture, "app", &["check", "--locked"]));
    no_package_outputs(&fixture);
}

#[test]
fn workspace_member_junctions_are_rejected_before_canonical_aliases_hide_them() {
    let fixture = local_packages();
    fixture.write(
        "oxid.toml",
        "[workspace]\nedition=\"typed-preview\"\nmembers=[\"app\"]\n",
    );
    success(package_output(&fixture, "", &["check", "--workspace"]));
    fs::rename(fixture.0.join("app"), fixture.0.join("ordinary")).unwrap();
    junction(&fixture.0.join("app"), &fixture.0.join("ordinary"));
    let text = package_refusal(package_output(&fixture, "", &["check", "--workspace"]));
    assert!(
        text.contains("reparse") || text.contains("symbolic link"),
        "{text}"
    );
    assert!(!fixture.0.join("oxid.lock").exists());
    fs::remove_dir(fixture.0.join("app")).unwrap();
    fs::rename(fixture.0.join("ordinary"), fixture.0.join("app")).unwrap();
    success(package_output(&fixture, "", &["check", "--workspace"]));
    no_package_outputs(&fixture);
}

#[test]
fn real_ntfs_module_check_and_result_are_source_only_and_preserve_unicode_roots() {
    let fixture = Fixture::new();
    fixture.program();
    let output = fixture.check();
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("\"functions\":2"),
        "{output:?}"
    );
    success(output);
    fixture.result();
}

#[test]
fn exact_case_is_required_and_missing_unused_children_still_fail() {
    let fixture = Fixture::new();
    fixture.write("main.ox", MAIN);
    fixture.write("Child.ox", CHILD);
    diagnostic(fixture.check(), "E0005", "case does not match");
    fs::remove_file(fixture.0.join("Child.ox")).unwrap();
    diagnostic(fixture.check(), "E0002", "component is missing");
    fixture.no_artifacts();
}

#[test]
fn an_exact_name_wins_over_a_folded_neighbor_in_a_real_case_sensitive_directory() {
    let fixture = Fixture::new();
    let output = Command::new("fsutil.exe")
        .args(["file", "setCaseSensitiveInfo"])
        .arg(&fixture.0)
        .arg("enable")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "actual NTFS case-sensitive-directory capability is required: {output:?}"
    );
    fixture.program();
    fixture.write(
        "Child.ox",
        "this differently cased neighbor must not be parsed",
    );
    assert_ne!(
        fs::read(fixture.0.join("child.ox")).unwrap(),
        fs::read(fixture.0.join("Child.ox")).unwrap()
    );
    success(fixture.check());
    fixture.result();
}

#[test]
fn declared_case_collisions_and_exact_duplicates_keep_their_distinct_diagnostics() {
    let fixture = Fixture::new();
    fixture.write("Child.ox", CHILD);
    fixture.write("main.ox", "mod Child;mod child;fn main()->i32{return 0;}\n");
    diagnostic(fixture.check(), "E0005", "collide under ASCII case folding");
    fixture.write("main.ox", "mod Child;mod Child;fn main()->i32{return 0;}\n");
    diagnostic(fixture.check(), "E0201", "duplicate module declaration");
    fixture.no_artifacts();
}

#[test]
fn final_and_dangling_file_symlinks_are_refused_before_target_parsing() {
    let fixture = Fixture::new();
    let external = Fixture::new();
    fixture.write("main.ox", MAIN);
    external.write("target.ox", "invalid target must not be parsed");
    symlink_file(external.0.join("target.ox"), fixture.0.join("child.ox"))
        .expect("actual Windows file-symlink capability is required for qualification");
    diagnostic(fixture.check(), "E0005", "Windows reparse point");
    fs::remove_file(external.0.join("target.ox")).unwrap();
    diagnostic(fixture.check(), "E0005", "Windows reparse point");
    fs::remove_file(fixture.0.join("child.ox")).unwrap();
    fixture.write("child.ox", CHILD);
    fixture.result();
    external.no_artifacts();
}

#[test]
fn intermediate_junctions_are_refused_even_when_their_target_is_inside_the_root() {
    for inside in [false, true] {
        let fixture = Fixture::new();
        let external = Fixture::new();
        fixture.write("main.ox", "mod branch;fn main()->i32{return 0;}\n");
        fixture.write("branch.ox", "mod leaf;\n");
        let target = if inside {
            let path = fixture.0.join("ordinary");
            fs::create_dir(&path).unwrap();
            path
        } else {
            external.0.clone()
        };
        fs::write(
            target.join("leaf.ox"),
            "invalid redirected bytes must not be parsed",
        )
        .unwrap();
        junction(&fixture.0.join("branch"), &target);
        let text = diagnostic(fixture.check(), "E0005", "Windows reparse point");
        assert!(text.contains("branch.ox"), "{text}");
        assert!(!text.contains("E0100"), "{text}");
        fs::remove_dir(fixture.0.join("branch")).unwrap();
        fs::create_dir(fixture.0.join("branch")).unwrap();
        fs::write(fixture.0.join("branch/leaf.ox"), "").unwrap();
        success(fixture.check());
        fixture.no_artifacts();
        external.no_artifacts();
    }
}

#[test]
fn hardlink_names_remain_distinct_modules_and_nominal_record_types() {
    let fixture = Fixture::new();
    fixture.write(
        "left.ox",
        "pub struct Item{pub n:i32}pub fn make()->Item{return Item{n:21};}\n",
    );
    fs::hard_link(fixture.0.join("left.ox"), fixture.0.join("right.ox")).unwrap();
    fixture.write("main.ox", "mod left;mod right;fn main()->i32{let a=crate::left::make();let b=crate::right::make();return a.n+b.n;}\n");
    success(fixture.check());
    fixture.result();
    fixture.write("main.ox", "mod left;mod right;fn only(x:crate::left::Item)->i32{return x.n;}fn main()->i32{return only(crate::right::make());}\n");
    diagnostic(fixture.check(), "E0300", "type");
    fixture.no_artifacts();
}

#[test]
fn symlink_entry_uses_the_invoked_parent_and_repeated_canonical_paths_are_refused() {
    let fixture = Fixture::new();
    let external = Fixture::new();
    external.write("entry.ox", MAIN);
    external.write("child.ox", "invalid child beside target must not be parsed");
    fixture.write("child.ox", CHILD);
    symlink_file(external.0.join("entry.ox"), fixture.0.join("main.ox"))
        .expect("actual Windows file-symlink capability is required for qualification");
    success(fixture.check());
    fixture.result();
    fs::remove_file(fixture.0.join("main.ox")).unwrap();
    fixture.write("child.ox", MAIN);
    symlink_file(fixture.0.join("child.ox"), fixture.0.join("main.ox")).unwrap();
    diagnostic(fixture.check(), "E0005", "already loaded");
    fixture.no_artifacts();
    external.no_artifacts();
}

#[test]
fn stream_and_device_entry_aliases_keep_an_explicit_source_policy_refusal() {
    let fixture = Fixture::new();
    fixture.program();
    let stream = fixture.0.join("main.ox:source");
    fs::write(&stream, MAIN).expect("actual NTFS named-stream fixture creation is required");
    diagnostic(
        fixture.invoke("check", &stream, true),
        "E0005",
        "ordinary local drive paths",
    );
    let canonical = fixture.0.canonicalize().unwrap();
    let text = canonical.to_str().unwrap().strip_prefix(r"\\?\").unwrap();
    let device = PathBuf::from(format!(r"\\.\{text}\main.ox"));
    assert_eq!(fs::read(&device).unwrap(), MAIN.as_bytes());
    diagnostic(
        fixture.invoke("check", &device, true),
        "E0005",
        "ordinary local drive paths",
    );
    fixture.no_artifacts();
}

#[test]
fn a_failed_child_open_releases_ancestors_and_retry_preserves_the_same_public_result() {
    let fixture = Fixture::new();
    fixture.program();
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(fixture.0.join("child.ox"))
        .unwrap();
    let path = fixture.0.join("child.ox");
    let ordinary = fs::File::open(&path)
        .err()
        .and_then(|error| error.raw_os_error());
    // Compare the loader's native flag combinations with an ordinary read
    // while the exclusive handle is still alive. Keep the original CLI
    // refusal assertion, and retain these probes if the platform disagrees.
    for flags in [
        0,
        0x0200_0000,
        0x0020_0000,
        0x0100_0000,
        0x0220_0000,
        0x0300_0000,
        0x0120_0000,
        0x0320_0000,
    ] {
        let result = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .custom_flags(flags)
            .open(&path);
        eprintln!("read-sharing probe {path:?}, flags={flags:#010x}: {result:?}");
    }
    assert_eq!(
        ordinary,
        Some(32),
        "exclusive handle must deny ordinary reads"
    );
    diagnostic(fixture.check(), "E0002", "cannot read");
    fixture.no_artifacts();
    drop(held);
    fixture.result();
    fs::rename(fixture.0.join("child.ox"), fixture.0.join("moved.ox")).unwrap();
    fs::rename(fixture.0.join("moved.ox"), fixture.0.join("child.ox")).unwrap();
    fixture.result();
}

#[test]
fn dfs_loading_errors_precede_later_siblings_and_type_checks() {
    let fixture = Fixture::new();
    fixture.write(
        "main.ox",
        "mod branch;mod later;fn main()->i32{return false;}\n",
    );
    fixture.write("branch.ox", "mod missing;\n");
    fixture.write("later.ox", "later parse error must not win");
    let text = diagnostic(fixture.check(), "E0002", "component is missing");
    assert!(text.contains("branch.ox"), "{text}");
    assert!(!text.contains("E0300"), "{text}");
    fixture.write("main.ox", "mod branch; fn main( {\n");
    let output = fixture.check();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("\"stage\":\"parse\""), "{text}");
    assert!(!text.contains("component is missing"), "{text}");
    fixture.no_artifacts();
}

#[test]
fn aggregate_source_bytes_keep_the_existing_inclusive_boundary() {
    let fixture = Fixture::new();
    fixture.program();
    let mut child = CHILD.as_bytes().to_vec();
    let limit = 1_048_576 - MAIN.len();
    // Bound each trivia token so this exercises aggregate source admission,
    // rather than the independent maximum length of a single lexer token.
    while limit - child.len() >= 4 {
        let padding = (limit - child.len() - 4).min(4096);
        child.extend_from_slice(b"/*");
        child.resize(child.len() + padding, b' ');
        child.extend_from_slice(b"*/");
    }
    child.resize(limit, b' ');
    assert_eq!(MAIN.len() + child.len(), 1_048_576);
    fixture.write("child.ox", &child);
    success(fixture.check());
    child.push(b' ');
    fixture.write("child.ox", &child);
    diagnostic(
        fixture.check(),
        "E0400",
        "aggregate source byte limit exceeded",
    );
    fixture.no_artifacts();
}

#[test]
fn a_canonical_drive_root_has_no_extra_native_join_separator() {
    let fixture = Fixture::new();
    let canonical = fixture.0.canonicalize().unwrap();
    let drive = canonical.ancestors().last().unwrap();
    let suffix = format!(
        "{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let entry = drive.join(format!("oxid_windows_entry_{suffix}.ox"));
    let module = format!("oxid_windows_child_{suffix}");
    let child = drive.join(format!("{module}.ox"));
    struct RootFiles(Option<PathBuf>, Option<PathBuf>);
    impl Drop for RootFiles {
        fn drop(&mut self) {
            for path in [&self.0, &self.1].into_iter().flatten() {
                let _ = fs::remove_file(path);
            }
        }
    }
    // Own only two exclusive files; never recursively clean a drive root.
    let mut root_entry = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&entry)
        .unwrap();
    let mut owned = RootFiles(Some(entry.clone()), None);
    let mut root_child = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&child)
        .unwrap();
    owned.1 = Some(child.clone());
    use std::io::Write;
    write!(
        root_entry,
        "mod {module};fn main()->i32{{return crate::{module}::value();}}\n"
    )
    .unwrap();
    root_child.write_all(CHILD.as_bytes()).unwrap();
    drop(root_entry);
    drop(root_child);
    success(fixture.invoke("check", &entry, true));
    let output = fixture.invoke("run", &entry, false);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    assert!(output.stderr.is_empty(), "{output:?}");
    fixture.no_artifacts();
}
