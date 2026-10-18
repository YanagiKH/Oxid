//! Ordinary artifact lifecycle tests; synthetic bytes isolate publication from LLVM.
use super::{linux::publish_with, *};
use std::{
    fs,
    os::unix::fs::symlink,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-build-core-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn selection(&self) -> BuildSelection {
        BuildSelection {
            entry: "unused.ox".into(),
            parent: format!("{}/", self.0.display()),
            name: "app".into(),
        }
    }
    fn mode(&self) -> PathBuf {
        self.0.join(".oxid/typed-preview/result")
    }
    fn state(&self) -> PathBuf {
        self.mode().join(".app.build")
    }
    fn publish(&self, bytes: &[u8]) -> Result<String, Box<Diagnostic>> {
        publish_with(&self.selection(), EntryPolicy::Result, |path| {
            fs::write(path, bytes).unwrap();
            Ok(())
        })
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn refused(error: Box<Diagnostic>, text: &str) {
    assert_eq!(error.code, "E0710");
    assert!(error
        .render_human(&crate::frontend::source::SourceMap::new())
        .contains(text));
}
#[test]
fn managed_build_replaces_owned_output_and_retains_old_bytes_on_compile_failure() {
    let s = Scratch::new();
    let output = s.publish(b"first").unwrap();
    assert_eq!(fs::read(&output).unwrap(), b"first");
    let failed = publish_with(&s.selection(), EntryPolicy::Result, |path| {
        fs::write(path, b"incomplete").unwrap();
        Err(Diagnostic::new(
            "E0701",
            "native-toolchain",
            "injected ordinary compilation failure",
            None,
        ))
    });
    assert_eq!(failed.unwrap_err().code, "E0701");
    assert_eq!(fs::read(&output).unwrap(), b"first");
    assert!(!s.state().join("stage").exists());
    s.publish(b"second").unwrap();
    assert_eq!(fs::read(&output).unwrap(), b"second");
    s.publish(b"third").unwrap();
    assert_eq!(fs::read(&output).unwrap(), b"third");
}
#[test]
fn managed_build_rejects_unowned_files_symlinks_and_symlink_directories() {
    for kind in ["file", "symlink", "directory"] {
        let s = Scratch::new();
        fs::create_dir_all(s.mode()).unwrap();
        let output = s.mode().join("app");
        fs::write(s.0.join("unrelated"), b"keep").unwrap();
        match kind {
            "file" => fs::write(&output, b"unowned").unwrap(),
            "symlink" => symlink(s.0.join("unrelated"), &output).unwrap(),
            _ => fs::create_dir(&output).unwrap(),
        }
        let err = publish_with(&s.selection(), EntryPolicy::Result, |_| {
            panic!("must refuse before compiler")
        })
        .unwrap_err();
        refused(
            err,
            if kind == "file" {
                "unowned"
            } else {
                "regular file"
            },
        );
        assert_eq!(fs::read(s.0.join("unrelated")).unwrap(), b"keep");
    }
    let s = Scratch::new();
    let elsewhere = Scratch::new();
    symlink(&elsewhere.0, s.0.join(".oxid")).unwrap();
    assert!(s.publish(b"new").is_err());
    assert_eq!(fs::read_dir(&elsewhere.0).unwrap().count(), 0);
}
#[test]
fn managed_build_excludes_simultaneous_build_and_rejects_stale_stage() {
    let s = Scratch::new();
    publish_with(&s.selection(), EntryPolicy::Result, |path| {
        refused(s.publish(b"concurrent").unwrap_err(), "another build");
        fs::write(path, b"winner").unwrap();
        Ok(())
    })
    .unwrap();
    fs::create_dir(s.state().join("stage")).unwrap();
    fs::write(s.state().join("stage/partial"), b"interrupted").unwrap();
    refused(s.publish(b"next").unwrap_err(), "interrupted build staging");
    assert_eq!(fs::read(s.mode().join("app")).unwrap(), b"winner");
    fs::remove_dir_all(s.state().join("stage")).unwrap();
    s.publish(b"retry").unwrap();
}
#[test]
fn managed_build_keeps_old_output_when_receipt_publication_fails() {
    let s = Scratch::new();
    let output = s.publish(b"old").unwrap();
    // During compilation, make the pre-commit ownership slot inappropriate.
    let result = publish_with(&s.selection(), EntryPolicy::Result, |path| {
        fs::write(path, b"new").unwrap();
        fs::create_dir(s.state().join("next")).unwrap();
        Ok(())
    });
    refused(result.unwrap_err(), "ownership receipt");
    assert_eq!(fs::read(output).unwrap(), b"old");
}
#[test]
fn managed_build_detects_external_output_replacement() {
    let s = Scratch::new();
    let output = s.publish(b"old").unwrap();
    let result = publish_with(&s.selection(), EntryPolicy::Result, |path| {
        fs::write(path, b"new").unwrap();
        fs::write(s.0.join("external"), b"external").unwrap();
        fs::rename(s.0.join("external"), &output).unwrap();
        Ok(())
    });
    refused(result.unwrap_err(), "unowned");
    assert_eq!(fs::read(output).unwrap(), b"external");
}
#[test]
fn managed_build_modes_have_separate_outputs_and_receipts() {
    let s = Scratch::new();
    let result = s.publish(b"result").unwrap();
    let process = publish_with(&s.selection(), EntryPolicy::Process, |path| {
        fs::write(path, b"process").unwrap();
        Ok(())
    })
    .unwrap();
    assert_ne!(result, process);
    assert_eq!(fs::read(result).unwrap(), b"result");
    assert_eq!(fs::read(process).unwrap(), b"process");
}

#[test]
fn managed_build_rename_failure_keeps_previous_executable() {
    use std::os::unix::fs::PermissionsExt;
    // Root bypasses directory write permissions; exercise this on ordinary users.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let s = Scratch::new();
    let output = s.publish(b"old").unwrap();
    let result = publish_with(&s.selection(), EntryPolicy::Result, |path| {
        fs::write(path, b"new").unwrap();
        fs::set_permissions(s.mode(), fs::Permissions::from_mode(0o500)).unwrap();
        Ok(())
    });
    fs::set_permissions(s.mode(), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(result.unwrap_err().code, "E0710");
    assert_eq!(fs::read(&output).unwrap(), b"old");
    s.publish(b"retry").unwrap();
    assert_eq!(fs::read(output).unwrap(), b"retry");
}

#[test]
fn managed_build_first_creator_initializes_despite_losing_lock() {
    use std::os::fd::AsRawFd;
    let s = Scratch::new();
    // A creates the directory, but B opens and locks it before A can lock it.
    fs::create_dir_all(s.state()).unwrap();
    let creator = fs::File::open(s.state()).unwrap();
    let contender = fs::File::open(s.state()).unwrap();
    assert_eq!(
        unsafe { libc::flock(contender.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    assert!(!s.state().join("owner").exists());
    refused(
        linux::lock_state(&creator, true).unwrap_err(),
        "another build",
    );
    // B sees the complete marker, even though A lost the lock and exits.
    drop(creator);
    linux::lock_state(&contender, false).unwrap();
    assert!(!s.state().join("owner.pending").exists());
    drop(contender);
    let output = s.publish(b"retry without cleanup").unwrap();
    assert_eq!(fs::read(output).unwrap(), b"retry without cleanup");
}

#[test]
fn managed_build_rejects_uninitialized_and_incomplete_markers() {
    for marker in [None, Some("owner"), Some("owner.pending")] {
        let s = Scratch::new();
        fs::create_dir_all(s.state()).unwrap();
        if let Some(name) = marker {
            fs::write(s.state().join(name), b"oxid managed").unwrap();
        }
        let error = publish_with(&s.selection(), EntryPolicy::Result, |_| {
            panic!("unrecognized state must refuse before compilation")
        })
        .unwrap_err();
        refused(error, "unrecognized managed build directory");
        assert!(!s.state().join("stage").exists());
        assert!(!s.mode().join("app").exists());
        if let Some(name) = marker {
            assert_eq!(fs::read(s.state().join(name)).unwrap(), b"oxid managed");
        }
    }
}
