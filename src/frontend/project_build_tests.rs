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
fn native_profiles_separate_all_output_and_receipt_namespaces() {
    use std::os::unix::fs::MetadataExt;
    let s = Scratch::new();
    let mut artifacts = Vec::new();
    for (profile, result, process, test) in [
        (BuildProfile::Dev, "result", "process", "test"),
        (
            BuildProfile::Release,
            "release-result",
            "release-process",
            "release-test",
        ),
    ] {
        for (policy, expected) in [
            (EntryPolicy::Result, result),
            (EntryPolicy::Process, process),
        ] {
            assert_eq!(namespace(policy, profile), expected);
            let artifact = linux::publish_with_profile(&s.selection(), policy, profile, |path| {
                fs::write(path, expected).unwrap();
                Ok(())
            })
            .unwrap();
            assert_eq!(artifact.profile(), profile);
            assert_eq!(
                artifact.path,
                s.0.join(format!(".oxid/typed-preview/{expected}/app"))
                    .to_string_lossy()
                    .into_owned()
            );
            artifacts.push((artifact, expected, "app"));
        }
        assert_eq!(test_namespace(profile), test);
        let artifact = linux::publish_named_with(
            &s.selection().parent,
            "case-0",
            test_namespace(profile),
            profile,
            |path| {
                fs::write(path, test).unwrap();
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(artifact.profile(), profile);
        artifacts.push((artifact, test, "case-0"));
    }
    let mut inodes = std::collections::HashSet::new();
    for (artifact, namespace, name) in artifacts {
        assert_eq!(fs::read(&artifact.path).unwrap(), namespace.as_bytes());
        let directory = s.0.join(format!(".oxid/typed-preview/{namespace}"));
        let state = directory.join(format!(".{name}.build"));
        let output = fs::metadata(&artifact.path).unwrap();
        assert!(inodes.insert(output.ino()));
        assert_eq!(
            fs::metadata(state.join("next")).unwrap().ino(),
            output.ino()
        );
        assert!(state.join("owner").is_file());
        assert!(!state.join("stage").exists());
    }
}

#[test]
fn failed_release_build_preserves_release_and_dev_artifacts_and_receipts() {
    use std::os::unix::fs::MetadataExt;
    let s = Scratch::new();
    let dev = s.publish(b"dev").unwrap();
    let release = linux::publish_with_profile(
        &s.selection(),
        EntryPolicy::Result,
        BuildProfile::Release,
        |path| {
            fs::write(path, b"release").unwrap();
            Ok(())
        },
    )
    .unwrap();
    let mut preserved = Vec::new();
    for namespace in ["result", "release-result"] {
        for name in ["app", ".app.build/next", ".app.build/owner"] {
            let path = s.0.join(format!(".oxid/typed-preview/{namespace}/{name}"));
            preserved.push((
                path.clone(),
                fs::metadata(&path).unwrap().ino(),
                fs::read(path).unwrap(),
            ));
        }
    }
    let error = linux::publish_with_profile(
        &s.selection(),
        EntryPolicy::Result,
        BuildProfile::Release,
        |path| {
            fs::write(path, b"partial release").unwrap();
            Err(Diagnostic::new(
                "E0701",
                "native-toolchain",
                "injected failure",
                None,
            ))
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "E0701");
    assert_eq!(fs::read(dev).unwrap(), b"dev");
    assert_eq!(release.package_bytes().unwrap(), b"release");
    for (path, inode, bytes) in preserved {
        // A committed next receipt is normalized before the next compile. Its
        // ownership inode and bytes are preserved, even when its name changes.
        let path = if path.ends_with("release-result/.app.build/next") {
            path.with_file_name("current")
        } else {
            path
        };
        assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    assert!(!s
        .0
        .join(".oxid/typed-preview/release-result/.app.build/stage")
        .exists());
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
        linux::publish_named_with(
            &s.selection().parent,
            "case-0",
            "test",
            BuildProfile::Dev,
            |path| {
                fs::copy(program, path).unwrap();
                Ok(())
            },
        )
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
    assert!(linux::publish_named_with(
        &s.selection().parent,
        "case-0",
        "test",
        BuildProfile::Dev,
        |_| {
            panic!("must refuse before compiling");
        },
    )
    .is_err());
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

fn package_elf(tag: u8) -> Vec<u8> {
    let mut bytes = vec![0; 65];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16..24].copy_from_slice(&[3, 0, 62, 0, 1, 0, 0, 0]);
    bytes[52] = 64;
    bytes[64] = tag;
    bytes
}
fn package_session(s: &Scratch) -> crate::frontend::session::CompilerSession {
    use crate::frontend::{manifest::Command, options, session::*};
    fs::write(
        s.0.join("oxid.toml"),
        "[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n",
    )
    .unwrap();
    fs::write(
        s.0.join("main.ox"),
        "mod helper; fn main()->i32{return crate::helper::answer();}",
    )
    .unwrap();
    fs::write(s.0.join("helper.ox"), "pub fn answer()->i32{return 7;}").unwrap();
    let args = vec![
        "package".into(),
        format!("--manifest-path={}/oxid.toml", s.0.display()),
        "--output=unused.tar".into(),
        "--message-format=json".into(),
    ];
    let options::Route::ManifestPackage {
        request,
        entry_policy,
        profile,
        ..
    } = options::route(&args)
    else {
        panic!("package route");
    };
    CompilerSession::load(
        request,
        Configuration::new(Command::Package, Engine::Native, entry_policy, true)
            .with_profile(profile),
    )
    .unwrap_or_else(|(_, errors)| panic!("{errors:?}"))
}
fn package_artifact(s: &Scratch, bytes: &[u8]) -> Artifact {
    publish_with(&s.selection(), EntryPolicy::Result, |path| {
        fs::write(path, bytes).unwrap();
        Ok(())
    })
    .unwrap()
}
#[test]
fn package_descriptor_keeps_a_after_b_c_publication_and_repeated_reads() {
    use std::io::{Read, Seek, SeekFrom};
    let s = Scratch::new();
    let a_bytes = package_elf(b'A');
    let a = package_artifact(&s, &a_bytes);
    let b = package_artifact(&s, &package_elf(b'B'));
    let c = package_artifact(&s, &package_elf(b'C'));
    assert_eq!(fs::read(&a.path).unwrap(), c.package_bytes().unwrap());
    assert_eq!(
        fs::read(s.state().join("current")).unwrap(),
        package_elf(b'B')
    );
    assert_eq!(fs::read(s.state().join("next")).unwrap(), package_elf(b'C'));
    // A cloned descriptor shares its cursor: positional reads must leave it alone.
    let mut shared = a.executable.try_clone().unwrap();
    shared.seek(SeekFrom::Start(8)).unwrap();
    for _ in 0..2 {
        assert_eq!(a.package_bytes().unwrap(), a_bytes);
    }
    assert_eq!(shared.stream_position().unwrap(), 8);
    let mut one = [0];
    shared.read_exact(&mut one).unwrap();
    assert_eq!(one[0], a_bytes[8]);
    assert_eq!(b.package_bytes().unwrap(), package_elf(b'B'));
    assert!(a
        .package_bytes_limit(64)
        .unwrap_err()
        .contains("size limit"));
}
#[test]
fn package_descriptor_rejects_nonfiles_and_u64_oversize_before_reading() {
    let s = Scratch::new();
    let directory = Artifact {
        path: "unused".into(),
        profile: BuildProfile::Dev,
        executable: fs::File::open(&s.0).unwrap(),
    };
    assert!(directory
        .package_bytes()
        .unwrap_err()
        .contains("regular file"));
    let path = s.0.join("sparse");
    let file = fs::File::create(&path).unwrap();
    file.set_len(crate::frontend::native_bundle::MAX_EXECUTABLE as u64 + 1)
        .unwrap();
    // The unreadable descriptor makes an accidental read fail differently.
    let oversize = Artifact {
        path: path.to_string_lossy().into_owned(),
        profile: BuildProfile::Dev,
        executable: file,
    };
    assert!(oversize.package_bytes().unwrap_err().contains("size limit"));
}
#[test]
fn package_finish_uses_session_name_and_retained_a_not_current_c() {
    use crate::frontend::project_package::finish;
    let s = Scratch::new();
    let session = package_session(&s);
    let module = session.check().unwrap().native_module().unwrap();
    fs::write(s.0.join("oxid.toml"), "not a manifest").unwrap();
    fs::remove_file(s.0.join("main.ox")).unwrap();
    fs::remove_file(s.0.join("helper.ox")).unwrap();
    assert_eq!(session.selected_name(), "app");
    assert_eq!(session.check().unwrap().native_module().unwrap(), module);
    let a = package_artifact(&s, &package_elf(b'A'));
    package_artifact(&s, &package_elf(b'B'));
    package_artifact(&s, &package_elf(b'C'));
    let output = s.0.join("different-name.tar");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    assert_eq!(
        finish(
            &session,
            &a,
            output.to_str().unwrap(),
            &mut stdout,
            &mut stderr
        ),
        0
    );
    let archive = fs::read(&output).unwrap();
    assert_eq!(&archive[..7], b"bin/app");
    assert_eq!(&archive[512..577], package_elf(b'A'));
    assert!(String::from_utf8_lossy(&archive).contains("\"name\":\"app\""));
    assert_eq!(fs::read(&a.path).unwrap(), package_elf(b'C'));
    let report = String::from_utf8(stdout).unwrap();
    assert_eq!(report.matches("package-summary").count(), 1);
    assert!(!report.contains("build-summary"));
    assert!(report.contains("\"managed_published\":true"));
    assert!(report.contains("\"archive_published\":true"));
    assert!(stderr.is_empty());
}

#[test]
fn package_metadata_uses_retained_profile_not_path_or_session_default() {
    use crate::frontend::project_package::finish;
    use std::os::unix::fs::MetadataExt;
    let s = Scratch::new();
    let session = package_session(&s);
    assert_eq!(session.configuration().profile, BuildProfile::Dev);
    let publish = |tag| {
        linux::publish_with_profile(
            &s.selection(),
            EntryPolicy::Result,
            BuildProfile::Release,
            |path| {
                fs::write(path, package_elf(tag)).unwrap();
                Ok(())
            },
        )
        .unwrap()
    };
    let mut artifact = publish(b'A');
    publish(b'B');
    publish(b'C');
    assert_eq!(artifact.executable.metadata().unwrap().nlink(), 0);
    // The reporting path is deliberately misleading. Neither it nor the
    // session's default profile may override this retained inode's identity.
    artifact.path = format!("{}/.oxid/typed-preview/result/app", s.0.display());
    let output = s.0.join("dev-name.tar");
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        finish(
            &session,
            &artifact,
            output.to_str().unwrap(),
            &mut out,
            &mut err
        ),
        0
    );
    let archive = fs::read(output).unwrap();
    assert_eq!(&archive[512..577], package_elf(b'A'));
    let text = String::from_utf8_lossy(&archive);
    assert!(text.contains("\"profile\":\"native-preview-O2\""));
    assert!(!text.contains("native-preview-O0"));
    assert_eq!(artifact.profile(), BuildProfile::Release);
    assert!(err.is_empty());
}
#[test]
fn package_finish_failure_keeps_managed_fact_and_does_not_replace_archive() {
    use crate::frontend::project_package::finish;
    for collision in [false, true] {
        let s = Scratch::new();
        let session = package_session(&s);
        let bytes = if collision {
            package_elf(b'A')
        } else {
            b"not ELF".to_vec()
        };
        let artifact = package_artifact(&s, &bytes);
        let output = s.0.join("out.tar");
        if collision {
            fs::write(&output, b"racing winner").unwrap();
        }
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(
            finish(
                &session,
                &artifact,
                output.to_str().unwrap(),
                &mut out,
                &mut err
            ),
            1
        );
        let report = String::from_utf8(out).unwrap();
        assert!(report.contains("\"code\":\"E0720\""));
        assert!(report.contains("\"success\":false"));
        assert!(report.contains("\"managed_published\":true"));
        assert!(report.contains("\"archive_published\":false"));
        assert!(report.contains(&crate::frontend::diagnostic::json_string(&artifact.path)));
        assert_eq!(fs::read(&artifact.path).unwrap(), bytes);
        if collision {
            assert_eq!(fs::read(output).unwrap(), b"racing winner");
        } else {
            assert!(!output.exists());
        }
        assert!(fs::read_dir(&s.0).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".oxid-package-")));
    }
}
#[test]
fn package_reporting_failure_retains_both_published_files() {
    use crate::frontend::project_package::finish;
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let s = Scratch::new();
    let session = package_session(&s);
    let artifact = package_artifact(&s, &package_elf(b'A'));
    let output = s.0.join("out.tar");
    let mut err = Vec::new();
    assert_eq!(
        finish(
            &session,
            &artifact,
            output.to_str().unwrap(),
            &mut Broken,
            &mut err
        ),
        1
    );
    assert_eq!(fs::read(&artifact.path).unwrap(), package_elf(b'A'));
    assert_eq!(&fs::read(&output).unwrap()[512..577], package_elf(b'A'));
    let diagnostic = String::from_utf8(err).unwrap();
    assert!(diagnostic.contains("E0721"));
    assert!(diagnostic.contains(&artifact.path));
    assert!(diagnostic.contains(output.to_str().unwrap()));
    assert!(diagnostic.contains("published files were retained"));
}
