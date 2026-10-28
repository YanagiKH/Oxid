//! Trusted external LLVM tool boundary with optional verified object reuse.
//! All language admission happens before this module is entered.
use super::build_config::NativeBuildSettings;
use super::diagnostic::Diagnostic;
use super::native_cache::{self, Environment, Outcome};
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

const LLVM_VERSION: &str = "19.1.7";
const TARGET: &str = NativeBuildSettings::new(false).target_triple();
const RUNTIME: &str = include_str!("../../native/typed_preview.c");
const CHARACTER_RUNTIME: &str = include_str!("../../native/typed_character.c");
const FLOAT_RUNTIME: &str = include_str!("../../native/typed_float.c");
#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
const FLOAT_FORMAT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/float_format.a"));
#[cfg(not(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
const FLOAT_FORMAT: &[u8] = &[];
const INTEGER128_RUNTIME: &str = include_str!("../../native/typed_integer128.c");
const BYTEBUF_RUNTIME: &str = include_str!("../../native/typed_bytebuf.c");
const BYTEBUF_CORE: &str = include_str!("../../native/experimental_bytebuf.c");
const FILE_CORE: &str = include_str!("../../native/experimental_files.c");
const ARGUMENT_CORE: &str = include_str!("../../native/experimental_arguments.c");
const BYTEBUF_HEADER: &str = include_str!("../../native/experimental_bytebuf.h");
const VECTOR_CORE: &str = include_str!("../../native/experimental_vector.c");
const VECTOR_HEADER: &str = include_str!("../../native/experimental_vector.h");
const VECTOR_ADAPTER: &str = include_str!("../../native/typed_vector.c");
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
pub(crate) mod doctor;
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
// Preserve argv[0] symlink names: ld.lld is a multi-call driver. Relative
// selections are resolved before any child changes its working directory.
fn resolve_tool(path: PathBuf) -> Result<PathBuf, Box<Diagnostic>> {
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
}

fn run(
    tool: &Path,
    args: &[&OsStr],
    cwd: &Path,
    environment: Option<&Environment>,
) -> Result<Output, Box<Diagnostic>> {
    let mut command = Command::new(tool);
    command.args(args).current_dir(cwd);
    if let Some(environment) = environment {
        command.env_clear().envs(environment.iter().cloned());
    }
    let result = command.env_remove("CCC_OVERRIDE_OPTIONS")
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
fn version(
    tool: &Path,
    marker: &str,
    cwd: &Path,
    environment: Option<&Environment>,
) -> Result<(), Box<Diagnostic>> {
    let output = run(tool, &[OsStr::new("--version")], cwd, environment)?;
    validate_version_output(tool, marker, &output.stdout)
}
fn validate_version_output(
    tool: &Path,
    marker: &str,
    stdout: &[u8],
) -> Result<(), Box<Diagnostic>> {
    let text = String::from_utf8_lossy(stdout);
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

fn runtime_compile_args(settings: &NativeBuildSettings) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = [
        "--no-default-config",
        &format!("--target={}", settings.target_triple()),
        settings.optimization_flag(),
        "-fPIE",
        "-std=c11",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-c",
        "runtime.c",
        "-o",
        "runtime.o",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    if cfg!(target_os = "macos") {
        arguments.push(OsString::from("-mmacosx-version-min=11.0"));
        arguments.push(OsString::from("-D_DARWIN_C_SOURCE"));
    }
    if settings.execution().heap_limit() != super::build_config::ExecutionPolicy::DEFAULT_HEAP_LIMIT
    {
        arguments.push(OsString::from(format!(
            "-DOXID_BYTEBUF_LIMIT={}ULL",
            settings.execution().heap_limit()
        )));
    }
    arguments
}

/// Publish a project-owned native output without destroying the previous build
/// on a compile/link failure. Rename replaces a regular file atomically on
/// the supported host filesystems.
pub(super) fn compile_managed(module: &str, output: &str) -> Result<(), Box<Diagnostic>> {
    managed(module, output, false, None).map(|_| ())
}

pub(super) fn compile_managed_incremental(
    module: &str,
    output: &str,
) -> Result<Outcome, Box<Diagnostic>> {
    managed(module, output, true, None)
}

pub(super) fn compile_managed_with_settings(
    module: &str,
    output: &str,
    settings: &NativeBuildSettings,
) -> Result<(), Box<Diagnostic>> {
    managed(module, output, false, Some(settings)).map(|_| ())
}

pub(super) fn compile_managed_incremental_with_settings(
    module: &str,
    output: &str,
    settings: &NativeBuildSettings,
) -> Result<Outcome, Box<Diagnostic>> {
    managed(module, output, true, Some(settings))
}

fn managed(
    module: &str,
    output: &str,
    incremental: bool,
    settings: Option<&NativeBuildSettings>,
) -> Result<Outcome, Box<Diagnostic>> {
    admit_settings(settings)?;
    let output = Path::new(output);
    let parent = output
        .parent()
        .filter(|path| path.file_name() == Some(OsStr::new("bin")))
        .ok_or_else(|| failure("managed native output must be under .oxid/bin"))?;
    let build_root = parent
        .parent()
        .filter(|path| path.file_name() == Some(OsStr::new(".oxid")))
        .ok_or_else(|| failure("managed native output must be under .oxid/bin"))?;
    let name = output
        .file_name()
        .and_then(OsStr::to_str)
        .filter(|name| {
            !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        })
        .ok_or_else(|| {
            failure("managed executable name must contain only letters, digits, _ or -")
        })?;
    // Do not follow symlinks in the two directories owned by the build tool.
    // The project filesystem is trusted and stable, as for source loading; this
    // is not a sandbox against concurrent hostile directory replacement.
    for path in [build_root, parent] {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(failure(
                    "managed build directories must be real directories",
                ))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match fs::create_dir(path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        let meta = fs::symlink_metadata(path).map_err(io)?;
                        if !meta.is_dir() || meta.file_type().is_symlink() {
                            return Err(failure(
                                "managed build directories must be real directories",
                            ));
                        }
                    }
                    Err(error) => return Err(io(error)),
                }
            }
            Err(error) => return Err(io(error)),
        }
    }
    let output = parent.join(name);
    match fs::symlink_metadata(&output) {
        Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {}
        Ok(_) => return Err(failure("managed native output must be a regular file")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io(error)),
    }
    let stage = Workspace::create(parent)?;
    let staged_output = stage.0.join("program");
    let outcome = compile_inner(
        module,
        staged_output
            .to_str()
            .ok_or_else(|| failure("managed output path must be UTF-8"))?,
        incremental.then_some(build_root),
        settings,
    )?;
    fs::rename(staged_output, output).map_err(io)?;
    Ok(outcome)
}

pub(super) fn compile(module: &str, output: &str) -> Result<(), Box<Diagnostic>> {
    compile_inner(module, output, None, None).map(|_| ())
}

pub(super) fn compile_with_settings(
    module: &str,
    output: &str,
    settings: &NativeBuildSettings,
) -> Result<(), Box<Diagnostic>> {
    compile_inner(module, output, None, Some(settings)).map(|_| ())
}

fn admit_settings(settings: Option<&NativeBuildSettings>) -> Result<(), Box<Diagnostic>> {
    if !NativeBuildSettings::host_supported() {
        return Err(failure(
            "native builds require Linux or macOS on x86_64 or ARM64",
        ));
    }
    if settings.is_some_and(|settings| settings.debug_info())
        && !NativeBuildSettings::debug_info_supported()
    {
        return Err(failure("native debug information currently requires Linux"));
    }
    Ok(())
}

fn compile_inner(
    module: &str,
    output: &str,
    cache_root: Option<&Path>,
    settings: Option<&NativeBuildSettings>,
) -> Result<Outcome, Box<Diagnostic>> {
    admit_settings(settings)?;
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
    // A typed project build uses one captured environment whether or not object
    // reuse is requested. Historical file entrypoints keep their original path.
    let environment = (cache_root.is_some() || settings.is_some()).then(native_cache::environment);
    let settings = settings
        .copied()
        .unwrap_or_else(|| NativeBuildSettings::new(false));
    let env_value = |name: &str| {
        environment.as_ref().map_or_else(
            || std::env::var_os(name),
            |values| {
                values
                    .iter()
                    .find(|(key, _)| key == name)
                    .map(|(_, value)| value.clone())
            },
        )
    };
    // Resolve PATH before changing child cwd; preserve argv[0] symlink names
    // (ld.lld is a multi-call driver). Tool installations are trusted inputs.
    let resolve = |path: PathBuf| -> Result<PathBuf, Box<Diagnostic>> {
        if environment.is_none() {
            return resolve_tool(path);
        }
        let cwd = std::env::current_dir().map_err(io)?;
        if path.components().count() > 1 || path.is_absolute() {
            return Ok(cwd.join(path));
        }
        let search = env_value("PATH").unwrap_or_default();
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
    let selected = |name| {
        if environment.is_none() {
            return tool(name);
        }
        env_value("OXID_LLVM_BIN").map_or_else(
            || PathBuf::from(format!("{name}-19")),
            |dir| PathBuf::from(dir).join(name),
        )
    };
    let clang = resolve(selected("clang"))?;
    let opt = resolve(selected("opt"))?;
    let lld = resolve(selected(NativeBuildSettings::linker()))?;
    version(&clang, "clang version", &parent, environment.as_ref())?;
    version(&opt, "LLVM version", &parent, environment.as_ref())?;
    version(&lld, "LLD", &parent, environment.as_ref())?;
    let workspace = Workspace::create(&parent)?;
    fs::write(workspace.0.join("program.ll"), module).map_err(io)?;
    fs::write(workspace.0.join("runtime.c"), RUNTIME).map_err(io)?;
    if module.contains("declare void @__oxid_bytebuf_start()") {
        // The sealed emitter selects the runtime. The complete source is
        // embedded in the compiler; the linked program needs no checkout.
        fs::write(workspace.0.join("experimental_bytebuf.h"), BYTEBUF_HEADER).map_err(io)?;
        fs::write(workspace.0.join("experimental_bytebuf.c"), BYTEBUF_CORE).map_err(io)?;
        fs::write(workspace.0.join("experimental_vector.c"), VECTOR_CORE).map_err(io)?;
        fs::write(workspace.0.join("experimental_vector.h"), VECTOR_HEADER).map_err(io)?;
        fs::write(workspace.0.join("typed_vector.c"), VECTOR_ADAPTER).map_err(io)?;
        fs::write(workspace.0.join("experimental_files.c"), FILE_CORE).map_err(io)?;
        fs::write(workspace.0.join("experimental_arguments.c"), ARGUMENT_CORE).map_err(io)?;
        fs::write(workspace.0.join("typed_bytebuf.c"), BYTEBUF_RUNTIME).map_err(io)?;
        let arguments = if module.contains("declare void @__oxid_arguments_start(i32, ptr)") {
            "#include \"experimental_arguments.c\"\n"
        } else {
            ""
        };
        let record_schemas = if module.contains("@__oxid_record_vector_schema_count = constant") {
            "#define OXID_RECORD_VECTOR_SCHEMAS 1\n"
        } else {
            ""
        };
        fs::write(
            workspace.0.join("runtime.c"),
            format!("{record_schemas}{RUNTIME}\n#include \"typed_bytebuf.c\"\n{arguments}"),
        )
        .map_err(io)?;
    }
    if module.contains("declare i32 @__oxid_print_i128_parts(i64, i64)") {
        let runtime_path = workspace.0.join("runtime.c");
        let mut runtime = fs::read_to_string(&runtime_path).map_err(io)?;
        runtime.push('\n');
        runtime.push_str(INTEGER128_RUNTIME);
        fs::write(runtime_path, runtime).map_err(io)?;
    }
    if module
        .lines()
        .any(|line| line == "declare i32 @__oxid_print_char(i32)")
    {
        let path = workspace.0.join("runtime.c");
        let mut runtime = fs::read_to_string(&path).map_err(io)?;
        runtime.push('\n');
        runtime.push_str(CHARACTER_RUNTIME);
        fs::write(path, runtime).map_err(io)?;
    }
    let float_output = module.contains("declare i32 @__oxid_print_float_bits(i64, i32)");
    let float_operations = module
        .lines()
        .any(|line| line == "declare void @__oxid_float_runtime()");
    if float_operations {
        fs::write(workspace.0.join("float_format.a"), FLOAT_FORMAT).map_err(io)?;
    }
    if float_output {
        let path = workspace.0.join("runtime.c");
        let mut runtime = fs::read_to_string(&path).map_err(io)?;
        runtime.push('\n');
        runtime.push_str(FLOAT_RUNTIME);
        fs::write(path, runtime).map_err(io)?;
    }
    let args = |values: &[&str]| values.iter().map(OsString::from).collect::<Vec<_>>();
    let invoke = |path: &Path, values: Vec<OsString>| {
        run(
            path,
            &values.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
            &workspace.0,
            environment.as_ref(),
        )
        .map(|_| ())
    };
    invoke(
        &opt,
        args(&["-passes=verify", "-disable-output", "program.ll"]),
    )?;
    let target = format!("--target={}", settings.target_triple());
    let mut program_args = args(&[
        "--no-default-config",
        &target,
        settings.optimization_flag(),
        "-fPIE",
        "-c",
        "-x",
        "ir",
        "program.ll",
        "-o",
        "program.o",
    ]);
    if cfg!(target_os = "macos") {
        program_args.push(OsString::from("-mmacosx-version-min=11.0"));
    }
    let key = || {
        cache_root
            .and(environment.as_ref())
            .and_then(|environment| {
                native_cache::fingerprint(
                    module,
                    &[&clang, &opt],
                    &program_args,
                    environment,
                    &workspace.0,
                )
            })
    };
    let initial_key = key();
    let mut cache = match (cache_root, initial_key) {
        (Some(root), Some(key)) => Some(native_cache::Cache::new(root, key).map_err(io)?),
        _ => None,
    };
    let object = workspace.0.join("program.o");
    let mut hit = cache.as_ref().is_some_and(|cache| cache.restore(&object));
    if hit && key() != initial_key {
        fs::remove_file(&object).map_err(io)?;
        hit = false;
        cache = None;
    }
    let mut outcome = if hit {
        Outcome::Hit
    } else if cache.is_some() {
        Outcome::Miss
    } else {
        Outcome::Unavailable
    };
    if !hit {
        invoke(&clang, program_args.clone())?;
        if let Some(cache) = cache {
            if key() == initial_key {
                // Cache storage failure cannot turn a valid build into a failure.
                if cache
                    .save(&object, &workspace.0.join("object-cache-stage"))
                    .is_err()
                {
                    outcome = Outcome::Unavailable;
                }
            } else {
                outcome = Outcome::Unavailable;
            }
        }
    }
    invoke(&clang, runtime_compile_args(&settings))?;
    let mut link = if cfg!(target_os = "macos") {
        args(&[
            "--no-default-config",
            &target,
            "-mmacosx-version-min=11.0",
            "-Wl,-pie",
            "program.o",
            "runtime.o",
            "-o",
            "executable",
        ])
    } else {
        args(&[
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
        ])
    };
    if float_operations {
        link.push(OsString::from("float_format.a"));
        link.push(OsString::from(if cfg!(target_os = "macos") {
            "-Wl,-dead_strip"
        } else {
            "-Wl,--gc-sections"
        }));
    }
    if float_operations {
        link.push(OsString::from("-lm"));
    }
    let mut linker = OsString::from("--ld-path=");
    linker.push(lld);
    link.push(linker);
    invoke(&clang, link)?;
    // Atomic no-replace publication on the same filesystem. Unlike rename,
    // hard_link cannot clobber an output created concurrently after preflight.
    fs::hard_link(workspace.0.join("executable"), output).map_err(io)?;
    Ok(outcome)
}
