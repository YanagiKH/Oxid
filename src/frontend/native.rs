//! Trusted external LLVM tool boundary. No shell, source execution, cache, or
//! legacy runtime. All language admission happens before this module is entered.
use super::diagnostic::Diagnostic;
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

const LLVM_VERSION: &str = "19.1.7";
const TARGET: &str = "x86_64-unknown-linux-gnu";
const RUNTIME: &str = include_str!("../../native/typed_preview.c");
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
fn failure(message: impl Into<String>) -> Box<Diagnostic> {
    Diagnostic::new("E0701", "native-toolchain", message, None)
}
fn io(error: std::io::Error) -> Box<Diagnostic> {
    failure(format!("native artifact I/O failed: {error}"))
}
struct Workspace(PathBuf);
impl Workspace {
    fn create(parent: &Path) -> Result<Self, Box<Diagnostic>> {
        for _ in 0..100 {
            let path = parent.join(format!(
                ".oxid-native-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(io(e)),
            }
        }
        Err(failure(
            "cannot reserve a fresh private native build directory",
        ))
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn tool(name: &str) -> PathBuf {
    match std::env::var_os("OXID_LLVM_BIN") {
        Some(dir) => PathBuf::from(dir).join(name),
        None => PathBuf::from(format!("{name}-19")),
    }
}
fn run(tool: &Path, args: &[&OsStr], cwd: &Path) -> Result<Output, Box<Diagnostic>> {
    let result = Command::new(tool).args(args).current_dir(cwd)
        .env_remove("CCC_OVERRIDE_OPTIONS")
        .output().map_err(|e| failure(format!("cannot launch {}: {e}; install LLVM {LLVM_VERSION} and set OXID_LLVM_BIN to its bin directory", tool.display())))?;
    if !result.status.success() {
        // Keep external diagnostics bounded. The renderer escapes control bytes.
        let bytes = &result.stderr[..result.stderr.len().min(8192)];
        return Err(failure(format!(
            "{} failed ({}): {}",
            tool.display(),
            result.status,
            String::from_utf8_lossy(bytes)
        )));
    }
    Ok(result)
}
fn version(tool: &Path, marker: &str, cwd: &Path) -> Result<(), Box<Diagnostic>> {
    let output = run(tool, &[OsStr::new("--version")], cwd)?;
    let text = String::from_utf8_lossy(&output.stdout);
    let expected = format!("{marker} {LLVM_VERSION}");
    if !text.lines().any(|line| {
        line.trim()
            .split_once(&expected)
            .is_some_and(|(_, after)| after.is_empty() || after.starts_with([' ', '(']))
    }) {
        return Err(failure(format!(
            "{} must report {expected}",
            tool.display()
        )));
    }
    Ok(())
}

pub(super) fn compile(module: &str, output: &str) -> Result<(), Box<Diagnostic>> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err(failure("native preview requires a Linux x86_64 build host"));
    }
    // Path::file_name normalizes trailing separators and `.` components.
    // Preserve the caller's file-vs-directory intent before that normalization.
    if matches!(output.rsplit('/').next(), Some("" | "." | "..")) {
        return Err(failure(
            "native output must name a new file, not a directory path",
        ));
    }
    let requested = Path::new(output);
    let name = requested
        .file_name()
        .ok_or_else(|| failure("native output must name a new file"))?;
    let parent = requested
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent).map_err(io)?;
    let output = parent.join(name);
    match fs::symlink_metadata(&output) {
        Ok(_) => return Err(failure("native output already exists; choose a new path (existing files and symlinks are never overwritten)")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => return Err(io(e)),
    }
    // Resolve PATH before changing child cwd; preserve argv[0] symlink names
    // (ld.lld is a multi-call driver). Tool installations are trusted inputs.
    let resolve = |path: PathBuf| -> Result<PathBuf, Box<Diagnostic>> {
        let cwd = std::env::current_dir().map_err(io)?;
        if path.components().count() > 1 || path.is_absolute() {
            return Ok(cwd.join(path));
        }
        let search = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&search)
            .map(|dir| cwd.join(dir).join(&path))
            .find(|candidate| candidate.is_file())
            .ok_or_else(|| {
                failure(format!(
                    "cannot find {}; install LLVM {LLVM_VERSION} or set OXID_LLVM_BIN",
                    path.display()
                ))
            })
    };
    let clang = resolve(tool("clang"))?;
    let opt = resolve(tool("opt"))?;
    let lld = resolve(tool("ld.lld"))?;
    version(&clang, "clang version", &parent)?;
    version(&opt, "LLVM version", &parent)?;
    version(&lld, "LLD", &parent)?;
    let workspace = Workspace::create(&parent)?;
    fs::write(workspace.0.join("program.ll"), module).map_err(io)?;
    fs::write(workspace.0.join("runtime.c"), RUNTIME).map_err(io)?;
    let args = |values: &[&str]| values.iter().map(OsString::from).collect::<Vec<_>>();
    let invoke = |path: &Path, values: Vec<OsString>| {
        run(
            path,
            &values.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
            &workspace.0,
        )
        .map(|_| ())
    };
    invoke(
        &opt,
        args(&["-passes=verify", "-disable-output", "program.ll"]),
    )?;
    let target = format!("--target={TARGET}");
    invoke(
        &clang,
        args(&[
            "--no-default-config",
            &target,
            "-O0",
            "-fPIE",
            "-c",
            "-x",
            "ir",
            "program.ll",
            "-o",
            "program.o",
        ]),
    )?;
    invoke(
        &clang,
        args(&[
            "--no-default-config",
            &target,
            "-O0",
            "-fPIE",
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-c",
            "runtime.c",
            "-o",
            "runtime.o",
        ]),
    )?;
    let mut link = args(&[
        "--no-default-config",
        &target,
        "-pie",
        "-Wl,-z,noexecstack",
        "-Wl,-z,relro",
        "-Wl,-z,now",
        "program.o",
        "runtime.o",
        "-o",
        "executable",
    ]);
    let mut linker = OsString::from("--ld-path=");
    linker.push(lld);
    link.push(linker);
    invoke(&clang, link)?;
    // Atomic no-replace publication on the same filesystem. Unlike rename,
    // hard_link cannot clobber an output created concurrently after preflight.
    fs::hard_link(workspace.0.join("executable"), output).map_err(io)?;
    Ok(())
}
