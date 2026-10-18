//! Managed native publication for explicit manifest build (Linux x86_64).
//!
//! Each output has a persistent private `.NAME.build` directory. Its directory
//! lock excludes cooperating builds; never unlink it as part of normal cleanup.
//! `current`/`next` are hard-link ownership receipts, not copies of executable
//! bytes. Preparing `next` precedes the single output rename (the commit point).
//! A later build accepts either receipt, so interruption immediately after the
//! rename does not lose ownership. No crash-durability guarantee is made.
//!
//! A stale `stage` directory causes an explicit refusal, never automatic replay.
//! After confirming no build is active, the user may remove ONLY that stage
//! directory and retry. Normal unwind removes it best-effort. Directory/receipt
//! tampering by other processes is unsupported; managed path components are
//! opened without following symlinks and operations use held directory handles.
use super::{diagnostic::Diagnostic, manifest::BuildSelection, options::EntryPolicy};

/// The open executable pins this invocation's inode across later publications.
/// Cooperating builds replace names, never the contents of published inodes.
#[derive(Debug)]
pub(super) struct Artifact {
    pub path: String,
    #[cfg(target_os = "linux")]
    executable: std::fs::File,
}
impl Artifact {
    fn run_error(&self, action: &str, error: impl std::fmt::Display) -> Box<Diagnostic> {
        Diagnostic::new(
            "E0711",
            "project-run",
            format!(
            "{action}: {error}; executable was published at {}; previous artifact was not restored",
            super::diagnostic::json_string(&self.path)
        ),
            None,
        )
    }

    #[cfg(target_os = "linux")]
    fn command(&self) -> std::process::Command {
        use std::{
            os::{fd::AsRawFd, unix::process::CommandExt},
            process::{Command, Stdio},
        };
        // Keep self alive through spawn: exec resolves the pinned ELF before
        // CLOEXEC closes the descriptor. No fallback to the mutable output path.
        // Missing/inaccessible procfs is an explicit launch failure.
        let mut command = Command::new(format!("/proc/self/fd/{}", self.executable.as_raw_fd()));
        command
            .arg0(&self.path)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        command
    }

    pub fn run(self) -> Result<i32, Box<Diagnostic>> {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            let mut child = self
                .command()
                .spawn()
                .map_err(|error| self.run_error("native project launch failed", error))?;
            let status = child.wait().map_err(|error| {
                self.run_error(
                    "native project wait failed; application completion is unknown",
                    error,
                )
            })?;
            // Preserve normal application statuses. Match the existing subprocess
            // convention for signal termination; do not install global handlers.
            Ok(status.code().unwrap_or(1))
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        Err(self.run_error("native project launch failed", "requires Linux x86_64"))
    }
}

pub(super) fn mode(policy: EntryPolicy) -> &'static str {
    match policy {
        EntryPolicy::Result => "result",
        EntryPolicy::Process => "process",
    }
}

pub(super) fn publish(
    module: &str,
    selection: &BuildSelection,
    policy: EntryPolicy,
) -> Result<Artifact, Box<Diagnostic>> {
    // Result compile historically reaches host validation after native emission.
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err(Diagnostic::new(
            "E0701",
            "native-toolchain",
            "native preview requires a Linux x86_64 build host",
            None,
        ));
    }
    #[cfg(target_os = "linux")]
    return linux::publish_with(selection, policy, |staged| {
        super::native::compile(module, staged)
    });
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (module, selection, policy);
        unreachable!("host checked above")
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::{
        ffi::CString,
        fs::{self, File},
        io::{Read, Write},
        os::fd::{AsRawFd, FromRawFd},
        path::Path,
    };
    const MARKER: &[u8] = b"oxid managed native output v1\n";
    fn failure(message: impl Into<String>) -> Box<Diagnostic> {
        Diagnostic::new("E0710", "project-build", message, None)
    }
    fn io(error: std::io::Error) -> Box<Diagnostic> {
        failure(format!("managed native output I/O failed: {error}"))
    }
    fn c(name: &str) -> Result<CString, Box<Diagnostic>> {
        CString::new(name).map_err(|_| failure("managed path contains a NUL byte"))
    }
    fn open(parent: &File, name: &str, flags: i32) -> Result<File, Box<Diagnostic>> {
        let name = c(name)?;
        // All callers request CLOEXEC and NOFOLLOW; descriptors own their fd.
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            )
        };
        if fd < 0 {
            Err(io(std::io::Error::last_os_error()))
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }
    fn mkdir(parent: &File, name: &str) -> Result<bool, Box<Diagnostic>> {
        let name = c(name)?;
        if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } == 0 {
            return Ok(true);
        }
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Ok(false)
        } else {
            Err(io(error))
        }
    }
    fn directory(parent: &File, name: &str) -> Result<File, Box<Diagnostic>> {
        mkdir(parent, name)?;
        open(parent, name, libc::O_RDONLY | libc::O_DIRECTORY)
    }
    fn stat(parent: &File, name: &str) -> Result<Option<libc::stat>, Box<Diagnostic>> {
        let name = c(name)?;
        let mut value = std::mem::MaybeUninit::uninit();
        if unsafe {
            libc::fstatat(
                parent.as_raw_fd(),
                name.as_ptr(),
                value.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } == 0
        {
            Ok(Some(unsafe { value.assume_init() }))
        } else {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(io(error))
            }
        }
    }
    fn regular(value: &libc::stat) -> bool {
        value.st_mode & libc::S_IFMT == libc::S_IFREG
    }
    fn same(a: &libc::stat, b: &libc::stat) -> bool {
        a.st_dev == b.st_dev && a.st_ino == b.st_ino
    }
    fn rename(from: &File, old: &str, to: &File, new: &str) -> Result<(), Box<Diagnostic>> {
        let (old, new) = (c(old)?, c(new)?);
        if unsafe { libc::renameat(from.as_raw_fd(), old.as_ptr(), to.as_raw_fd(), new.as_ptr()) }
            == 0
        {
            Ok(())
        } else {
            Err(io(std::io::Error::last_os_error()))
        }
    }
    fn unlink(parent: &File, name: &str) -> Result<(), Box<Diagnostic>> {
        let name = c(name)?;
        if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) } == 0 {
            Ok(())
        } else {
            Err(io(std::io::Error::last_os_error()))
        }
    }
    fn receipt(state: &File, name: &str) -> Result<Option<libc::stat>, Box<Diagnostic>> {
        let value = stat(state, name)?;
        if value.as_ref().is_some_and(|s| !regular(s)) {
            return Err(failure("managed ownership receipt must be a regular file"));
        }
        Ok(value)
    }
    fn validate_output(
        root: &File,
        name: &str,
        state: &File,
    ) -> Result<Option<libc::stat>, Box<Diagnostic>> {
        let current = receipt(state, "current")?;
        let next = receipt(state, "next")?;
        let output = stat(root, name)?;
        if let Some(output) = &output {
            if !regular(output) {
                return Err(failure(
                    "managed output must be a regular file; symlinks are rejected",
                ));
            }
            if !current.as_ref().is_some_and(|r| same(output, r))
                && !next.as_ref().is_some_and(|r| same(output, r))
            {
                return Err(failure("refusing to replace an unowned managed output"));
            }
        }
        Ok(output)
    }
    struct Stage<'a>(&'a File);
    impl Drop for Stage<'_> {
        fn drop(&mut self) {
            // Never remove ownership receipts or the locked persistent directory.
            let _ = fs::remove_dir_all(format!("/proc/self/fd/{}/stage", self.0.as_raw_fd()));
        }
    }
    pub(super) struct StateLock<'a>(&'a File);
    impl Drop for StateLock<'_> {
        fn drop(&mut self) {
            // Release explicitly: a concurrent fork can briefly retain a copy
            // of this open description even after our descriptor is closed.
            unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
        }
    }
    pub(super) fn lock_state(state: &File, fresh: bool) -> Result<StateLock<'_>, Box<Diagnostic>> {
        // Only the mkdir winner initializes. Finish before contending for the
        // lock: another opener may already hold it and must not strand us.
        if fresh {
            open(
                state,
                "owner.pending",
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            )?
            .write_all(MARKER)
            .map_err(io)?;
            // Publish complete bytes atomically, without replacing an owner.
            let (pending, owner) = (c("owner.pending")?, c("owner")?);
            if unsafe {
                libc::linkat(
                    state.as_raw_fd(),
                    pending.as_ptr(),
                    state.as_raw_fd(),
                    owner.as_ptr(),
                    0,
                )
            } != 0
            {
                return Err(io(std::io::Error::last_os_error()));
            }
            unlink(state, "owner.pending")?;
        }
        if unsafe { libc::flock(state.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = std::io::Error::last_os_error();
            return Err(if error.kind() == std::io::ErrorKind::WouldBlock {
                failure("another build owns this output; retry after it finishes")
            } else {
                io(error)
            });
        }
        let lock = StateLock(state);
        // Kernel also releases locks on exit. The directory
        // persists so later processes always lock the same inode. An opener that
        // arrives before marker publication refuses; it never initializes state.
        let mut marker = Vec::new();
        if !stat(state, "owner")?.as_ref().is_some_and(regular) {
            return Err(failure(
                "unrecognized managed build directory; inspect it before retrying",
            ));
        }
        open(state, "owner", libc::O_RDONLY)?
            .take(MARKER.len() as u64 + 1)
            .read_to_end(&mut marker)
            .map_err(io)?;
        if marker != MARKER {
            return Err(failure(
                "unrecognized managed build directory; inspect it before retrying",
            ));
        }
        Ok(lock)
    }
    pub(super) fn publish_with(
        selection: &BuildSelection,
        policy: EntryPolicy,
        compile: impl FnOnce(&str) -> Result<(), Box<Diagnostic>>,
    ) -> Result<Artifact, Box<Diagnostic>> {
        let parent = if selection.parent.is_empty() {
            "."
        } else {
            &selection.parent
        };
        // The manifest's lexical parent may itself be a symlink, as in CHECK/RUN.
        // Only the managed components below it must be real directories.
        let parent = File::open(Path::new(parent)).map_err(io)?;
        let oxid = directory(&parent, ".oxid")?;
        let typed = directory(&oxid, "typed-preview")?;
        let root = directory(&typed, mode(policy))?;
        let state_name = format!(".{}.build", selection.name);
        let fresh = mkdir(&root, &state_name)?;
        let state = open(&root, &state_name, libc::O_RDONLY | libc::O_DIRECTORY)?;
        let _lock = lock_state(&state, fresh)?;
        let old = validate_output(&root, &selection.name, &state)?;
        // Normalize a previous committed next receipt before reusing that slot.
        if let (Some(old), Some(next)) = (&old, receipt(&state, "next")?) {
            if same(old, &next) {
                rename(&state, "next", &state, "current")?;
            }
        }
        if !mkdir(&state, "stage")? {
            return Err(failure("interrupted build staging exists; after confirming no build is active, remove only the output's .build/stage directory and retry"));
        }
        let _cleanup = Stage(&state);
        let stage = open(&state, "stage", libc::O_RDONLY | libc::O_DIRECTORY)?;
        let staged = format!("/proc/self/fd/{}/stage/executable", state.as_raw_fd());
        compile(&staged)?;
        if !stat(&stage, "executable")?.as_ref().is_some_and(regular) {
            return Err(failure(
                "native compiler did not produce a regular staged executable",
            ));
        }
        // Acquire before the commit point while the stage is private and locked.
        // Failure leaves the old output intact; the descriptor outlives all names.
        let executable = open(&stage, "executable", libc::O_RDONLY)?;
        let now = validate_output(&root, &selection.name, &state)?;
        if match (&old, &now) {
            (None, None) => false,
            (Some(a), Some(b)) => !same(a, b),
            _ => true,
        } {
            return Err(failure(
                "managed output changed during build; publication refused",
            ));
        }
        if receipt(&state, "next")?.is_some() {
            unlink(&state, "next")?;
        }
        let (exe, next) = (c("executable")?, c("next")?);
        if unsafe {
            libc::linkat(
                stage.as_raw_fd(),
                exe.as_ptr(),
                state.as_raw_fd(),
                next.as_ptr(),
                0,
            )
        } != 0
        {
            return Err(io(std::io::Error::last_os_error()));
        }
        // The only commit point. Every failure above leaves old output bytes intact.
        // next already owns the new inode; nothing fallible is required afterward.
        rename(&stage, "executable", &root, &selection.name)?;
        Ok(Artifact {
            executable,
            path: format!(
                "{}.oxid/typed-preview/{}/{}",
                selection.parent,
                mode(policy),
                selection.name
            ),
        })
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "project_build_tests.rs"]
mod tests;
