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
        .map(|artifact| artifact.path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn refused(error: &Diagnostic, text: &str) {
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
            &err,
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
        refused(&s.publish(b"concurrent").unwrap_err(), "another build");
        fs::write(path, b"winner").unwrap();
        Ok(())
    })
    .unwrap();
    fs::create_dir(s.state().join("stage")).unwrap();
    fs::write(s.state().join("stage/partial"), b"interrupted").unwrap();
    refused(
        &s.publish(b"next").unwrap_err(),
        "interrupted build staging",
    );
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
    refused(&result.unwrap_err(), "ownership receipt");
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
    refused(&result.unwrap_err(), "unowned");
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
    .unwrap()
    .path;
    assert_ne!(result, process);
    assert_eq!(fs::read(result).unwrap(), b"result");
    assert_eq!(fs::read(process).unwrap(), b"process");
}

#[test]
fn typed_project_test_artifacts_preserve_app_outputs_receipts_and_pinned_handles() {
    use std::os::unix::fs::MetadataExt;
    let s = Scratch::new();
    let mut preserved = Vec::new();
    for policy in [EntryPolicy::Result, EntryPolicy::Process] {
        for bytes in [b"old app", b"new app"] {
            publish_with(&s.selection(), policy, |path| {
                fs::write(path, bytes).unwrap();
                Ok(())
            })
            .unwrap();
        }
        let directory = s.0.join(format!(".oxid/typed-preview/{}", mode(policy)));
        for relative in [
            "app",
            ".app.build/current",
            ".app.build/next",
            ".app.build/owner",
        ] {
            let path = directory.join(relative);
            let meta = fs::metadata(&path).unwrap();
            preserved.push((path.clone(), meta.ino(), fs::read(path).unwrap()));
        }
    }
    let publish_test = |program: &str| {
        linux::publish_named_with(&s.selection().parent, "case-0", "test", |path| {
            fs::copy(program, path).unwrap();
            Ok(())
        })
        .unwrap()
    };
    let passed = publish_test("/bin/true");
    let failed = publish_test("/bin/false");
    assert_eq!(passed.path, failed.path);
    assert!(passed.path.ends_with("/test/case-0"));
    assert!(passed.run_test().unwrap());
    assert!(!failed.run_test().unwrap());
    for (path, inode, bytes) in preserved {
        assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}

#[test]
fn typed_project_test_namespace_rejects_alias_to_application_directory() {
    let s = Scratch::new();
    let output = s.publish(b"application").unwrap();
    symlink("result", s.0.join(".oxid/typed-preview/test")).unwrap();
    assert!(
        linux::publish_named_with(&s.selection().parent, "case-0", "test", |_| {
            panic!("must refuse before compiling");
        })
        .is_err()
    );
    assert_eq!(fs::read(output).unwrap(), b"application");
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
    let error = linux::lock_state(&creator, true)
        .err()
        .expect("creator loses lock");
    refused(&error, "another build");
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
        refused(&error, "unrecognized managed build directory");
        assert!(!s.state().join("stage").exists());
        assert!(!s.mode().join("app").exists());
        if let Some(name) = marker {
            assert_eq!(fs::read(s.state().join(name)).unwrap(), b"oxid managed");
        }
    }
}

// These handoff tests copy existing host ELF tools; they do not simulate LLVM
// or claim that Oxid native compilation was exercised.
#[test]
fn managed_build_releases_lock_with_an_inherited_descriptor_still_open() {
    let s = Scratch::new();
    s.publish(b"old").unwrap();
    let state = fs::File::open(s.state()).unwrap();
    let lock = linux::lock_state(&state, false).unwrap();
    let inherited = state.try_clone().unwrap();
    refused(&s.publish(b"busy").unwrap_err(), "another build");
    drop(lock);
    drop(state);
    s.publish(b"new").unwrap();
    drop(inherited);
    assert_eq!(fs::read(s.mode().join("app")).unwrap(), b"new");
}

#[cfg(target_arch = "x86_64")]
fn host_artifact(s: &Scratch, tool: &str) -> Artifact {
    publish_with(&s.selection(), EntryPolicy::Result, |path| {
        fs::copy(tool, path).unwrap();
        Ok(())
    })
    .unwrap()
}

#[test]
#[cfg(target_arch = "x86_64")]
fn native_run_artifact_survives_two_replacements_and_receipt_retirement() {
    use std::os::unix::fs::MetadataExt;
    let s = Scratch::new();
    let first = host_artifact(&s, "/bin/true");
    for _ in 0..2 {
        let later = host_artifact(&s, "/bin/false");
        assert_eq!(later.run().unwrap(), 1);
    }
    // No pathname still owns the first inode, including either receipt.
    assert_eq!(first.executable.metadata().unwrap().nlink(), 0);
    assert_ne!(
        first.executable.metadata().unwrap().ino(),
        fs::metadata(&first.path).unwrap().ino()
    );
    assert_eq!(first.run().unwrap(), 0);
}

#[test]
#[cfg(target_arch = "x86_64")]
fn native_run_artifact_command_keeps_raw_streams_cwd_and_cloexec() {
    use std::{io::Write, os::fd::AsRawFd, process::Stdio};
    let s = Scratch::new();
    let artifact = host_artifact(&s, "/bin/sh");
    let fd = artifact.executable.as_raw_fd();
    assert_ne!(
        unsafe { libc::fcntl(fd, libc::F_GETFD) } & libc::FD_CLOEXEC,
        0
    );
    let mut command = artifact.command();
    assert!(command.get_args().next().is_none());
    assert!(command.get_current_dir().is_none());
    // Supply test commands via -c so stdin contains application data alone.
    let script = format!("test ! -e /proc/self/fd/{fd} || exit 99; read line; printf '%s\\n' \"$line\"; printf 'application stderr\\n' >&2; pwd; exit 7");
    let mut child = command
        .arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"application stdin\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(
        output.stdout,
        format!(
            "application stdin\n{}\n",
            std::env::current_dir().unwrap().display()
        )
        .as_bytes()
    );
    assert_eq!(output.stderr, b"application stderr\n");
}

#[test]
#[cfg(target_arch = "x86_64")]
fn native_run_launch_failure_keeps_the_published_artifact() {
    let s = Scratch::new();
    let artifact = publish_with(&s.selection(), EntryPolicy::Result, |path| {
        fs::write(path, b"not executable").unwrap();
        Ok(())
    })
    .unwrap();
    let path = artifact.path.clone();
    let error = artifact.run().unwrap_err();
    assert_eq!(error.code, "E0711");
    let text = error.render_human(&crate::frontend::source::SourceMap::new());
    assert!(text.contains("native project launch failed"));
    assert!(text.contains("executable was published"));
    assert!(text.contains("previous artifact was not restored"));
    assert_eq!(fs::read(path).unwrap(), b"not executable");
}
