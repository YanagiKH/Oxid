//! Real LLVM acceptance for opt-in program-object reuse. These tests observe
//! execve calls as well as executing the resulting ELF; a cache status alone
//! cannot prove that verification, runtime compilation or linking still ran.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs,
    os::unix::fs::{symlink, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex, MutexGuard,
    },
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
static FIXTURE_EXECUTION: Mutex<()> = Mutex::new(());
const HEADER: usize = 88;

struct Fixture {
    _execution: MutexGuard<'static, ()>,
    root: PathBuf,
    compiler: PathBuf,
    app: PathBuf,
    tools: PathBuf,
    real_tools: PathBuf,
    environment: Vec<(OsString, OsString)>,
}

impl Fixture {
    fn new() -> Self {
        // Keep copied compiler descriptors out of concurrent fixture children.
        let execution = FIXTURE_EXECUTION
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        assert!(
            Path::new("/usr/bin/strace").is_file(),
            "install strace for native cache acceptance"
        );
        let real_tools = fs::canonicalize(
            std::env::var_os("OXID_LLVM_BIN").expect("set OXID_LLVM_BIN to real LLVM 19.1.7 tools"),
        )
        .unwrap();
        let root = std::env::temp_dir().join(format!(
            "oxid-native-incremental-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        // Cargo can replace target/debug/oxid while another test process runs.
        // Use one retained compiler file for this fixture's entire cache history.
        let compiler = root.join("oxid");
        fs::copy(env!("CARGO_BIN_EXE_oxid"), &compiler).unwrap();
        let app = root.join("app");
        let tools = root.join("tools");
        for path in [
            app.join("src"),
            root.join("math/src"),
            tools.clone(),
            root.join("traces"),
        ] {
            fs::create_dir_all(path).unwrap();
        }
        for name in ["clang", "opt", "ld.lld"] {
            symlink(real_tools.join(name), tools.join(name)).unwrap();
        }
        fs::write(
            app.join("oxid.toml"),
            "[project]\nname = \"app\"\nedition = \"typed-preview\"\nentry = \"src/main.ox\"\n[dependencies]\nmath = \"../math\"\n",
        )
        .unwrap();
        fs::write(
            app.join("src/main.ox"),
            "use crate::math::answer;\nfn main() -> i32 { return answer(); }\n",
        )
        .unwrap();
        fs::write(
            root.join("math/oxid.toml"),
            "[project]\nname = \"math\"\nedition = \"typed-preview\"\n",
        )
        .unwrap();
        fs::write(
            root.join("math/src/lib.ox"),
            "pub fn answer() -> i32 { return 7; }\n",
        )
        .unwrap();
        // Snapshot once. Trace names are arguments, never environment changes:
        // an environment-sensitive key must see identical inputs on warm runs.
        let mut environment: Vec<_> = std::env::vars_os()
            .filter(|(name, _)| {
                !matches!(
                    name.to_str(),
                    Some("OXID_LLVM_BIN" | "LD_PRELOAD" | "LD_AUDIT" | "LD_TRACE_LOADED_OBJECTS")
                )
            })
            .collect();
        environment.push(("OXID_LLVM_BIN".into(), tools.clone().into_os_string()));
        Self {
            _execution: execution,
            root,
            compiler,
            app,
            tools,
            real_tools,
            environment,
        }
    }

    fn cli(&self, args: &[&str]) -> Command {
        let mut command = Command::new(&self.compiler);
        command
            .current_dir(&self.app)
            .args(args)
            .env_clear()
            .envs(self.environment.iter().cloned())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn traced_command(&self, name: &str, args: &[&str]) -> Command {
        let mut command = Command::new("/usr/bin/strace");
        command
            .args(["-qq", "-f", "-s", "65536", "-e", "trace=execve", "-o"])
            .arg(self.root.join("traces").join(name))
            .arg("--")
            .arg(&self.compiler)
            .args(args)
            .current_dir(&self.app)
            .env_clear()
            .envs(self.environment.iter().cloned())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn trace(&self, name: &str) -> String {
        fs::read_to_string(self.root.join("traces").join(name)).unwrap()
    }

    fn build(&self, name: &str, extra: &[&str]) -> (Output, String) {
        let mut args = vec!["build", "--message-format=json"];
        args.extend_from_slice(extra);
        let output = self.traced_command(name, &args).output().unwrap();
        (output, self.trace(name))
    }

    fn phases(&self, trace: &str, codegen: usize) {
        let clang = format!("execve({:?}, ", self.tools.join("clang"));
        let opt = format!("execve({:?}, ", self.tools.join("opt"));
        let drivers: Vec<_> = trace
            .lines()
            .filter(|line| line.contains(&clang) && line.contains("\"--no-default-config\""))
            .collect();
        let actual = [
            drivers
                .iter()
                .filter(|line| line.contains("\"-x\", \"ir\"") && line.contains("\"program.ll\""))
                .count(),
            trace
                .lines()
                .filter(|line| line.contains(&opt) && line.contains("\"-passes=verify\""))
                .count(),
            drivers
                .iter()
                .filter(|line| line.contains("\"-c\"") && line.contains("\"runtime.c\""))
                .count(),
            drivers
                .iter()
                .filter(|line| {
                    line.contains("\"-pie\"")
                        && line.contains("\"program.o\"")
                        && line.contains("\"runtime.o\"")
                })
                .count(),
        ];
        assert_eq!(
            actual,
            [codegen, 1, 1, 1],
            "expected IR/verify/runtime/link counts; full trace:\n{trace}"
        );
    }

    fn no_tools(&self, trace: &str) {
        for name in ["clang", "opt", "ld.lld"] {
            assert!(
                !trace.contains(&format!("execve({:?}, ", self.tools.join(name))),
                "unexpected tool execution:\n{trace}"
            );
        }
    }

    fn executable(&self) -> PathBuf {
        self.app.join(".oxid/bin/app")
    }

    fn runs(&self, status: i32, stdout: &[u8]) {
        let output = Command::new(self.executable())
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(status), "{output:?}");
        assert_eq!(output.stdout, stdout, "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
    }

    fn entries(&self) -> Vec<PathBuf> {
        let mut entries: Vec<_> = fs::read_dir(self.app.join(".oxid/cache/native-object-v1"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for entry in &entries {
            let name = entry.file_name().unwrap().to_str().unwrap();
            assert!(
                name.len() == 70
                    && name.ends_with(".oxobj")
                    && name[..64].bytes().all(|c| c.is_ascii_hexdigit()),
                "unexpected cache member: {entry:?}"
            );
        }
        entries
    }

    fn one_entry(&self) -> PathBuf {
        let entries = self.entries();
        assert_eq!(entries.len(), 1, "{entries:?}");
        entries[0].clone()
    }

    fn restore_tool(&self, name: &str) {
        fs::remove_file(self.tools.join(name)).unwrap();
        symlink(self.real_tools.join(name), self.tools.join(name)).unwrap();
    }

    fn script_tool(&self, name: &str, text: &str) {
        fs::remove_file(self.tools.join(name)).unwrap();
        fs::write(self.tools.join(name), text).unwrap();
        fs::set_permissions(self.tools.join(name), fs::Permissions::from_mode(0o700)).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "retained failed native-cache fixture: {}",
                self.root.display()
            );
        } else {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn success(output: &Output, cache: Option<&str>) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = std::str::from_utf8(&output.stdout).unwrap();
    let events: Vec<_> = text
        .lines()
        .filter(|line| line.contains("\"kind\":\"build-cache\""))
        .collect();
    match cache {
        Some(status) => {
            assert_eq!(events.len(), 1, "{text}");
            assert!(
                events[0].contains("\"stage\":\"native-object\"")
                    && events[0].contains(&format!("\"status\":\"{status}\"")),
                "{text}"
            );
            assert_eq!(text.lines().count(), 2, "{text}");
        }
        None => {
            assert!(events.is_empty(), "{text}");
            assert_eq!(text.lines().count(), 1, "{text}");
        }
    }
    assert!(
        text.lines()
            .any(|line| line.contains("\"kind\":\"compile-summary\"")
                && line.contains("\"success\":true")),
        "{text}"
    );
}

fn rejected(output: &Output, message: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = std::str::from_utf8(&output.stdout).unwrap();
    assert!(
        text.contains(message)
            && text.contains("\"kind\":\"diagnostic\"")
            && text.contains("\"success\":false"),
        "{text}"
    );
    assert!(
        !text.contains("\"kind\":\"build-cache\""),
        "failed build claimed a cache outcome: {text}"
    );
}

fn valid_blob(path: &Path) -> Vec<u8> {
    let bytes = fs::read(path).unwrap();
    assert!(bytes.len() > HEADER + 64);
    assert_eq!(&bytes[..16], b"OXID-OBJECT-v1\0\0");
    assert_eq!(
        u64::from_le_bytes(bytes[48..56].try_into().unwrap()) as usize,
        bytes.len() - HEADER
    );
    assert_eq!(&bytes[56..88], &Sha256::digest(&bytes[HEADER..])[..]);
    assert_eq!(&bytes[HEADER..HEADER + 7], b"\x7fELF\x02\x01\x01");
    assert_eq!(&bytes[HEADER + 16..HEADER + 20], &[1, 0, 62, 0]);
    bytes
}

#[test]
#[ignore = "requires real LLVM 19.1.7 in OXID_LLVM_BIN and /usr/bin/strace"]
fn incremental_native_reuses_only_codegen_and_tracks_application_mode_and_dependencies() {
    let fixture = Fixture::new();
    let lock = fixture.cli(&["lock"]).output().unwrap();
    assert_eq!(lock.status.code(), Some(0), "{lock:?}");
    let (plain, trace) = fixture.build("plain", &[]);
    success(&plain, None);
    fixture.phases(&trace, 1);
    fixture.runs(7, b"");
    assert!(!fixture.app.join(".oxid/cache").exists());
    let (cold, trace) = fixture.build("cold", &["--incremental", "--frozen"]);
    success(&cold, Some("miss"));
    fixture.phases(&trace, 1);
    let entry = fixture.one_entry();
    let cache = valid_blob(&entry);
    let (warm, trace) = fixture.build("warm", &["--incremental", "--frozen"]);
    success(&warm, Some("hit"));
    fixture.phases(&trace, 0);
    fixture.runs(7, b"");
    let cache_meta = fs::metadata(&entry).unwrap();
    let output_meta = fs::metadata(fixture.executable()).unwrap();
    assert_eq!(cache_meta.nlink(), 1);
    assert_eq!(output_meta.nlink(), 1);
    assert_ne!(
        (cache_meta.dev(), cache_meta.ino()),
        (output_meta.dev(), output_meta.ino())
    );
    fs::write(
        fixture.executable(),
        b"damaged output must not damage the cached object",
    )
    .unwrap();
    assert_eq!(fs::read(&entry).unwrap(), cache);
    let (repaired, trace) = fixture.build("repair-output", &["--incremental", "--frozen"]);
    success(&repaired, Some("hit"));
    fixture.phases(&trace, 0);
    fixture.runs(7, b"");

    fs::write(
        fixture.app.join("src/main.ox"),
        "use crate::math::answer;\nfn main() -> i32 { return answer() + 1; }\n",
    )
    .unwrap();
    let (edited, trace) = fixture.build("edited-app", &["--incremental", "--frozen"]);
    success(&edited, Some("miss"));
    fixture.phases(&trace, 1);
    fixture.runs(8, b"");
    let (result, trace) = fixture.build(
        "result-mode",
        &["--incremental", "--frozen", "--entry-mode=result"],
    );
    success(&result, Some("miss"));
    fixture.phases(&trace, 1);
    fixture.runs(0, b"8\n");
    let before = fs::read(fixture.executable()).unwrap();
    let old_lock = fs::read(fixture.app.join("oxid.lock")).unwrap();
    let old_entries = fixture.entries();
    fs::write(
        fixture.root.join("math/src/lib.ox"),
        "pub fn answer() -> i32 { return 9; }\n",
    )
    .unwrap();
    let (stale, trace) = fixture.build("stale", &["--incremental", "--frozen"]);
    rejected(&stale, "lock is out of date");
    fixture.no_tools(&trace);
    assert_eq!(fs::read(fixture.executable()).unwrap(), before);
    assert_eq!(fs::read(fixture.app.join("oxid.lock")).unwrap(), old_lock);
    assert_eq!(fixture.entries(), old_entries);
    let update = fixture.cli(&["lock", "--update"]).output().unwrap();
    assert_eq!(update.status.code(), Some(0), "{update:?}");
    let (updated, trace) = fixture.build("updated-dependency", &["--incremental", "--frozen"]);
    success(&updated, Some("miss"));
    fixture.phases(&trace, 1);
    fixture.runs(10, b"");
    let before = fs::read(fixture.executable()).unwrap();
    fs::write(fixture.app.join("src/main.ox"), "fn broken(\n").unwrap();
    let (invalid, trace) = fixture.build("invalid-source", &["--incremental", "--frozen"]);
    rejected(&invalid, "E0100");
    fixture.no_tools(&trace);
    assert_eq!(fs::read(fixture.executable()).unwrap(), before);
}

#[test]
#[ignore = "requires real LLVM 19.1.7 in OXID_LLVM_BIN and /usr/bin/strace"]
fn incremental_native_hit_still_links_and_script_codegen_tools_are_uncached() {
    let fixture = Fixture::new();
    let (cold, trace) = fixture.build("cold", &["--incremental"]);
    success(&cold, Some("miss"));
    fixture.phases(&trace, 1);
    fixture.runs(7, b"");
    let executable = fs::read(fixture.executable()).unwrap();
    let entry = fixture.one_entry();
    let cache = valid_blob(&entry);
    // Linking is outside the cached stage. Keeping its selected path constant
    // while replacing only lld must reuse the object and still observe failure.
    fixture.script_tool("ld.lld", "#!/bin/sh\nif [ \"$1\" = --version ]; then printf '%s\\n' 'LLD 19.1.7'; exit 0; fi\nprintf '%s\\n' 'forced cache link failure' >&2\nexit 9\n");
    let (failed, trace) = fixture.build("failed-link", &["--incremental"]);
    rejected(&failed, "forced cache link failure");
    fixture.phases(&trace, 0);
    assert_eq!(fs::read(fixture.executable()).unwrap(), executable);
    assert_eq!(fs::read(&entry).unwrap(), cache);
    fixture.runs(7, b"");
    fixture.restore_tool("ld.lld");
    for name in ["clang", "opt"] {
        let real = fixture.real_tools.join(name);
        let quoted = real.to_str().unwrap().replace('\'', "'\"'\"'");
        fixture.script_tool(name, &format!("#!/bin/sh\nexec '{quoted}' \"$@\"\n"));
        let (uncached, trace) = fixture.build(&format!("wrapper-{name}"), &["--incremental"]);
        success(&uncached, Some("unavailable"));
        fixture.phases(&trace, 1);
        fixture.runs(7, b"");
        assert_eq!(fs::read(&entry).unwrap(), cache);
        fixture.restore_tool(name);
    }
}

#[test]
#[ignore = "requires real LLVM 19.1.7 in OXID_LLVM_BIN and /usr/bin/strace"]
fn incremental_native_corruption_misses_and_concurrent_writers_publish_complete_objects() {
    let fixture = Fixture::new();
    let (cold, trace) = fixture.build("cold", &["--incremental"]);
    success(&cold, Some("miss"));
    fixture.phases(&trace, 1);
    let entry = fixture.one_entry();
    let mut corrupt = valid_blob(&entry);
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    fs::write(&entry, &corrupt).unwrap();
    let (rebuilt, trace) = fixture.build("corrupt-object", &["--incremental"]);
    success(&rebuilt, Some("miss"));
    fixture.phases(&trace, 1);
    fixture.runs(7, b"");
    assert_eq!(
        fs::read(&entry).unwrap(),
        corrupt,
        "immutable publication must not replace the existing entry"
    );
    // Both processes must also tolerate concurrent first creation of the
    // managed output/cache directories, not just publishing into a warm tree.
    fs::remove_dir_all(fixture.app.join(".oxid")).unwrap();

    let args = ["build", "--incremental", "--message-format=json"];
    let first = fixture
        .traced_command("concurrent-first", &args)
        .spawn()
        .unwrap();
    let second = fixture
        .traced_command("concurrent-second", &args)
        .spawn()
        .unwrap();
    let outputs = [
        first.wait_with_output().unwrap(),
        second.wait_with_output().unwrap(),
    ];
    let mut misses = 0;
    for (name, output) in ["concurrent-first", "concurrent-second"]
        .into_iter()
        .zip(outputs)
    {
        let text = std::str::from_utf8(&output.stdout).unwrap();
        let miss = text.contains("\"status\":\"miss\"");
        misses += usize::from(miss);
        success(&output, Some(if miss { "miss" } else { "hit" }));
        fixture.phases(&fixture.trace(name), usize::from(miss));
    }
    assert!(misses >= 1, "an empty cache cannot serve two hits");
    valid_blob(&fixture.one_entry());
    fixture.runs(7, b"");
    let (warm, trace) = fixture.build("after-race", &["--incremental"]);
    success(&warm, Some("hit"));
    fixture.phases(&trace, 0);
    fixture.runs(7, b"");
    let bin_entries: Vec<_> = fs::read_dir(fixture.app.join(".oxid/bin"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(
        bin_entries,
        [OsString::from("app")],
        "private native workspaces must be removed"
    );
}
