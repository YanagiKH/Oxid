//! RFC 0029 contract edges through the public CLI, using bounded local fixtures.
//! Digest literals were computed independently from the RFC's framed bytes.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const FLAT_SHA: &str = "sha256:0e2f02766feab0246bcba853e4955a11953c6105d8ec8a48165889a7f8b1e99d";
const FLAT_FNV: &str = "fnv1a64:137966a53d847cbc";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-package-integrity-edges-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self(fs::canonicalize(root).unwrap());
        fs::create_dir(fixture.0.join("dep")).unwrap();
        fs::write(fixture.0.join("dep/value.ox"), b"const value = 1;\n").unwrap();
        fs::write(
            fixture.0.join("oxid.toml"),
            "[project]\nname = \"test\"\n[dependencies]\ndep = \"dep\"\n",
        )
        .unwrap();
        fixture
    }

    fn run(&self, args: &[&str]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(args)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // An erroneous attempt to read a FIFO must fail the test, never hang CI.
        // These path-only fixtures cannot launch Git or other child processes.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if child.try_wait().unwrap().is_some() {
                return child.wait_with_output().unwrap();
            }
            if Instant::now() >= deadline {
                let kill_result = child.kill();
                let output = child.wait_with_output().unwrap();
                panic!("CLI timed out for {args:?}; kill={kill_result:?}; {output:?}");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn ok(&self, args: &[&str], checksum: &str) {
        let output = self.run(args);
        assert_eq!(output.status.code(), Some(0), "{args:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{args:?}: {output:?}");
        assert_eq!(
            output.stdout,
            format!("dep path {checksum}\ndependencies resolved: 1\n").as_bytes(),
            "{args:?}: {output:?}"
        );
    }

    fn reject(&self, args: &[&str], reason: &str) {
        let output = self.run(args);
        assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
        assert_eq!(
            output.stderr,
            format!("error: {reason}\n").as_bytes(),
            "{args:?}: {output:?}"
        );
    }

    fn lock_bytes(checksum: &str) -> Vec<u8> {
        format!(
            "version = 1\n\n[[package]]\nname = \"dep\"\nsource = \"path+dep\"\n\
             revision = \"\"\nchecksum = \"{checksum}\"\ndependencies = []\n"
        )
        .into_bytes()
    }

    fn set_lock(&self, checksum: &str) -> Vec<u8> {
        let expected = Self::lock_bytes(checksum);
        fs::write(self.0.join("oxid.lock"), &expected).unwrap();
        expected
    }

    fn lock(&self) -> Vec<u8> {
        fs::read(self.0.join("oxid.lock")).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if thread::panicking() {
            eprintln!("preserving failed package fixture: {}", self.0.display());
        } else {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

#[test]
fn nested_exclusions_preserve_both_algorithms_but_included_changes_fail() {
    for (checksum, changed) in [
        (
            "sha256:480e64316fa341e364389ed5aee4282f272022857e02edc48e42e2f2b854ac21",
            "sha256:9655114556aaa60786769261bf622209433ef3901ecc63e239ec14336de67c50",
        ),
        ("fnv1a64:99ac38dc91f61e5e", "fnv1a64:c8f13a3fd64b7df7"),
    ] {
        let fixture = Fixture::new();
        let nested = fixture.0.join("dep/nested");
        fs::create_dir_all(nested.join("deeper")).unwrap();
        fs::write(nested.join("deeper/keep.bin"), [0, 255, 65]).unwrap();
        let original = fixture.set_lock(checksum);
        fixture.ok(&["lock", "--locked"], checksum);

        // Every excluded basename occurs at two non-root depths. Alternate
        // files and directories; excluded directories contain extra descendants.
        for (depth, parent) in [nested.clone(), nested.join("deeper")]
            .into_iter()
            .enumerate()
        {
            for (index, name) in [".git", ".oxid", "target", "oxid.lock"]
                .into_iter()
                .enumerate()
            {
                let path = parent.join(name);
                if (depth + index) % 2 == 0 {
                    fs::create_dir_all(path.join("child")).unwrap();
                    fs::write(path.join("child/ignored.bin"), [255, 0, 17]).unwrap();
                } else {
                    fs::write(path, b"excluded bytes").unwrap();
                }
            }
        }
        for args in [&["lock"][..], &["lock", "--offline"], &["lock", "--locked"]] {
            fixture.ok(args, checksum);
            assert_eq!(fixture.lock(), original);
        }
        fs::write(nested.join("deeper/keep.bin"), [0, 255, 66]).unwrap();
        fixture.reject(
            &["lock", "--offline"],
            &format!(
                "checksum mismatch for `dep`: expected {checksum}, found {changed}; \
                 use update mode only after verifying the source"
            ),
        );
        assert_eq!(fixture.lock(), original);
    }
}

#[cfg(unix)]
#[test]
fn included_fifo_is_rejected_before_open_without_creating_or_rewriting_lock() {
    use std::{ffi::CString, os::unix::ffi::OsStrExt, os::unix::fs::FileTypeExt};

    let fixture = Fixture::new();
    let pipe = fixture.0.join("dep/channel");
    let path = CString::new(pipe.as_os_str().as_bytes()).unwrap();
    // SAFETY: path is a live, NUL-terminated pathname; mkfifo retains no pointer.
    let result = unsafe { libc::mkfifo(path.as_ptr(), 0o600) };
    assert_eq!(
        result,
        0,
        "create FIFO: {}",
        std::io::Error::last_os_error()
    );
    assert!(fs::symlink_metadata(&pipe).unwrap().file_type().is_fifo());
    let reason = format!("unsupported package entry type: {}", pipe.display());

    fixture.reject(&["lock", "--offline"], &reason);
    assert!(!fixture.0.join("oxid.lock").exists());
    for checksum in [FLAT_SHA, FLAT_FNV] {
        let original = fixture.set_lock(checksum);
        for args in [
            &["lock"][..],
            &["lock", "--locked"],
            &["lock", "--offline"],
            &["lock", "--update"],
            &["update"],
        ] {
            fixture.reject(args, &reason);
            assert_eq!(fixture.lock(), original);
            assert!(fs::symlink_metadata(&pipe).unwrap().file_type().is_fifo());
        }
    }
    // Exclusion precedes entry-type rejection: this same FIFO is ignored when
    // it has an excluded basename, without opening it or changing the lock.
    fs::rename(pipe, fixture.0.join("dep/target")).unwrap();
    let original = fixture.set_lock(FLAT_SHA);
    fixture.ok(&["lock", "--locked"], FLAT_SHA);
    assert_eq!(fixture.lock(), original);
}

#[test]
fn lock_update_migrates_legacy_and_matches_update_with_or_without_content_change() {
    for (body, checksum) in [
        ("const value = 1;\n", FLAT_SHA),
        (
            "const value = 2;\n",
            "sha256:1739abc23c2297cba288fb03050eb566a002165851125dfe0697bfc68208afbd",
        ),
    ] {
        for args in [&["lock", "--update"][..], &["update"]] {
            let fixture = Fixture::new();
            fixture.set_lock(FLAT_FNV);
            fs::write(fixture.0.join("dep/value.ox"), body).unwrap();
            fixture.ok(args, checksum);
            let expected = Fixture::lock_bytes(checksum);
            assert_eq!(fixture.lock(), expected);
            fixture.ok(&["lock", "--locked"], checksum);
            assert_eq!(fixture.lock(), expected);
        }
    }
}
