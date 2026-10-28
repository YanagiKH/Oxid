//! Linux scalar shared-library linkage and complete no-replace publication.
//! No runtime.c, executable adapter, object cache or source reopening is used.
use super::*;
use crate::frontend::{native_library::Options, project::ProjectSources};
use serde::ser::{Serialize, SerializeSeq, SerializeStruct, Serializer};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, mem::size_of};

const LIBRARY: &str = "liboxid_library.so";
const HEADER: &str = "oxid_library.h";
const EXPORTS: &str = "{ global: oxid_library_abi_version; oxid_library_call_v1; local: *; };\n";

#[allow(dead_code)]
struct PublicationBank {
    file_buffer: [u8; 8192],
    hashes: [[u8; 64]; 5],
    digests: [[u8; 32]; 5],
    hashers: [Sha256; 2],
    files: [File; 3],
    file_metadata: [fs::Metadata; 3],
    paths: [PathBuf; 10],
    path_returns: [Result<PathBuf, std::io::Error>; 3],
    parameters: [(
        &'static str,
        &'static str,
        &'static ProjectSources,
        usize,
        &'static Options,
    ); 2],
    version_arguments: [&'static OsStr; 1],
    verify_arguments: [&'static OsStr; 3],
    compile_arguments: [&'static OsStr; 10],
    link_arguments: [&'static OsStr; 14],
    tool_results: [Result<Output, Box<Diagnostic>>; 2],
    workspace: [Workspace; 2],
    target: String,
    linker: OsString,
    publication_paths: [std::ffi::CString; 2],
    serializer: serde_json::Serializer<&'static mut File>,
    metadata: Metadata<'static>,
    source_row: SourceRow<'static>,
    source_sequence: Sources<'static>,
    status: Statuses,
    status_values: [i32; 5],
    counts: [usize; 8],
}
pub(super) const CONTROL_BYTES: usize = size_of::<PublicationBank>();

pub(in crate::frontend) fn carrier_bytes() -> usize {
    let path = 2 * crate::frontend::project::ProjectLimits::default().relative_bytes;
    // Ten named PathBuf roles, two copied publication C strings, the linker
    // option and target. All IR/header/source bytes are borrowed or streamed.
    CONTROL_BYTES
        + 10 * path
        + 2 * (path + 1)
        + path
        + "--ld-path=".len()
        + 2 * ("--target=".len() + NativeBuildSettings::new(false).target_triple().len())
}

fn path_admitted(path: &PathBuf) -> Result<(), Box<Diagnostic>> {
    let limit = crate::frontend::project::ProjectLimits::default().relative_bytes;
    if path.as_os_str().len() <= limit && path.capacity() <= 2 * limit {
        Ok(())
    } else {
        Err(reject("library path capacity exceeds paid path payload"))
    }
}

fn child(parent: &Path, name: &str) -> Result<PathBuf, Box<Diagnostic>> {
    let path = parent.join(name);
    path_admitted(&path)?;
    Ok(path)
}

fn reject(message: &'static str) -> Box<Diagnostic> {
    failure(message)
}

fn digest_file(path: &Path) -> Result<[u8; 64], Box<Diagnostic>> {
    let mut file = File::open(path).map_err(io)?;
    let before = file.metadata().map_err(io)?;
    if !before.is_file() {
        return Err(reject("library provenance input must be a regular file"));
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 8192];
    let mut bytes = 0u64;
    loop {
        let read = file.read(&mut buffer).map_err(io)?;
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(read as u64)
            .ok_or_else(|| reject("library provenance length overflow"))?;
        digest.update(&buffer[..read]);
    }
    let after = file.metadata().map_err(io)?;
    if bytes != before.len()
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err(reject("library provenance input changed during hashing"));
    }
    Ok(hex(&digest.finalize().into()))
}

fn hex(bytes: &[u8; 32]) -> [u8; 64] {
    let mut result = [0; 64];
    for (i, &byte) in bytes.iter().enumerate() {
        result[2 * i] = b"0123456789abcdef"[(byte >> 4) as usize];
        result[2 * i + 1] = b"0123456789abcdef"[(byte & 15) as usize];
    }
    result
}
fn ascii(bytes: &[u8; 64]) -> &str {
    std::str::from_utf8(bytes).expect("hexadecimal ASCII")
}

struct Parameters(usize);
impl Serialize for Parameters {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0))?;
        for _ in 0..self.0 {
            sequence.serialize_element("i32")?;
        }
        sequence.end()
    }
}
struct Statuses;
impl Serialize for Statuses {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut row = serializer.serialize_struct("Statuses", 5)?;
        row.serialize_field("success", &0)?;
        row.serialize_field("overflow", &1)?;
        row.serialize_field("division_by_zero", &2)?;
        row.serialize_field("fuel", &3)?;
        row.serialize_field("invalid_output", &4)?;
        row.end()
    }
}
struct SourceRow<'a> {
    path: &'a str,
    sha256: &'a str,
}
impl Serialize for SourceRow<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut row = serializer.serialize_struct("Source", 2)?;
        row.serialize_field("path", self.path)?;
        row.serialize_field("sha256", self.sha256)?;
        row.end()
    }
}
struct Sources<'a>(&'a ProjectSources);
impl Serialize for Sources<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.modules().len()))?;
        for module in self.0.modules() {
            let bytes = self.0.sources().get(module.file).text().as_bytes();
            let hash = hex(&Sha256::digest(bytes).into());
            sequence.serialize_element(&SourceRow {
                path: &module.relative_path,
                sha256: ascii(&hash),
            })?;
        }
        sequence.end()
    }
}
struct Metadata<'a> {
    options: &'a Options,
    project: &'a ProjectSources,
    parameters: usize,
    target: &'a str,
    library_sha256: &'a str,
    header_sha256: &'a str,
    compiler_sha256: &'a str,
    ir_sha256: &'a str,
}
impl Serialize for Metadata<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut row = serializer.serialize_struct("ScalarLibrary", 17)?;
        row.serialize_field("schema", "oxid-scalar-native-library-v1")?;
        row.serialize_field("abi_version", &1)?;
        row.serialize_field("export", &self.options.export)?;
        row.serialize_field("symbol", "oxid_library_call_v1")?;
        row.serialize_field("parameters", &Parameters(self.parameters))?;
        row.serialize_field("result", "i32")?;
        row.serialize_field("statuses", &Statuses)?;
        row.serialize_field("target", self.target)?;
        row.serialize_field(
            "profile",
            if self.options.release {
                "release"
            } else {
                "dev"
            },
        )?;
        row.serialize_field("execution_policy", "bounded")?;
        row.serialize_field("fuel", &self.options.fuel)?;
        row.serialize_field("library_sha256", self.library_sha256)?;
        row.serialize_field("header_sha256", self.header_sha256)?;
        row.serialize_field("compiler_package_version", env!("CARGO_PKG_VERSION"))?;
        row.serialize_field("compiler_sha256", self.compiler_sha256)?;
        row.serialize_field("ir_sha256", self.ir_sha256)?;
        row.serialize_field("sources", &Sources(self.project))?;
        row.end()
    }
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn publish(source: &Path, output: &Path) -> Result<(), Box<Diagnostic>> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| reject("library path contains a NUL byte"))?;
    let output = CString::new(output.as_os_str().as_bytes())
        .map_err(|_| reject("library path contains a NUL byte"))?;
    // Same-filesystem atomic no-replace publication of the complete directory.
    // An existing file, directory or symlink is preserved even after preflight.
    let status = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            output.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(io(std::io::Error::last_os_error()))
    }
}
#[cfg(not(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
fn publish(_: &Path, _: &Path) -> Result<(), Box<Diagnostic>> {
    Err(reject(
        "scalar native libraries currently require Linux x86_64 or ARM64",
    ))
}

pub(in crate::frontend) fn compile(
    module: &str,
    header: &str,
    project: &ProjectSources,
    parameters: usize,
    options: &Options,
) -> Result<(), Box<Diagnostic>> {
    crate::frontend::native_library::require_host()?;
    // Publication/hash/serializer control is a separate real carrier bank.
    // LLVM and tool-process allocations remain at the existing trusted boundary.
    if CONTROL_BYTES as u64 > crate::frontend::declaration_index::IndexLimits::default().scratch {
        return Err(reject("library publication carrier budget exceeded"));
    }
    let requested = Path::new(&options.output);
    let name = requested
        .file_name()
        .filter(|_| {
            !options.output.ends_with('/')
                && !options.output.ends_with("/.")
                && !options.output.ends_with("/..")
        })
        .ok_or_else(|| reject("library output must name a new version directory"))?;
    let parent = requested
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent).map_err(io)?;
    let output = parent.join(name);
    path_admitted(&parent)?;
    path_admitted(&output)?;
    match fs::symlink_metadata(&output) {
        Ok(_) => {
            return Err(reject(
                "native library output already exists; choose a new version directory",
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(io(e)),
    }
    let clang = resolve_tool(tool("clang"))?;
    let opt = resolve_tool(tool("opt"))?;
    let lld = resolve_tool(tool("ld.lld"))?;
    for path in [&clang, &opt, &lld] {
        path_admitted(path)?;
    }
    version(&clang, "clang version", &parent, None)?;
    version(&opt, "LLVM version", &parent, None)?;
    version(&lld, "LLD", &parent, None)?;
    let workspace = Workspace::create(&parent)?;
    let complete = child(&workspace.0, "complete")?;
    path_admitted(&workspace.0)?;
    path_admitted(&complete)?;
    fs::create_dir(&complete).map_err(io)?;
    fs::write(child(&workspace.0, "program.ll")?, module).map_err(io)?;
    fs::write(child(&workspace.0, "exports.map")?, EXPORTS).map_err(io)?;
    let target = format!(
        "--target={}",
        NativeBuildSettings::new(false).target_triple()
    );
    let optimization = if options.release { "-O2" } else { "-O0" };
    run(
        &opt,
        &[
            OsStr::new("-passes=verify"),
            OsStr::new("-disable-output"),
            OsStr::new("program.ll"),
        ],
        &workspace.0,
        None,
    )?;
    run(
        &clang,
        &[
            OsStr::new("--no-default-config"),
            OsStr::new(&target),
            OsStr::new(optimization),
            OsStr::new("-fPIC"),
            OsStr::new("-c"),
            OsStr::new("-x"),
            OsStr::new("ir"),
            OsStr::new("program.ll"),
            OsStr::new("-o"),
            OsStr::new("program.o"),
        ],
        &workspace.0,
        None,
    )?;
    let mut linker = OsString::from("--ld-path=");
    linker.push(lld);
    run(
        &clang,
        &[
            OsStr::new("--no-default-config"),
            OsStr::new(&target),
            OsStr::new("-shared"),
            OsStr::new("-nostdlib"),
            OsStr::new("-Wl,-z,noexecstack"),
            OsStr::new("-Wl,-z,relro"),
            OsStr::new("-Wl,-z,now"),
            OsStr::new("-Wl,-z,defs"),
            OsStr::new("-Wl,--version-script=exports.map"),
            OsStr::new("-Wl,-soname,liboxid_library.so"),
            OsStr::new("program.o"),
            OsStr::new("-o"),
            OsStr::new("complete/liboxid_library.so"),
            &linker,
        ],
        &workspace.0,
        None,
    )?;
    let library = child(&complete, LIBRARY)?;
    path_admitted(&library)?;
    let mut identity = [0u8; 64];
    File::open(&library)
        .map_err(io)?
        .read_exact(&mut identity)
        .map_err(io)?;
    if &identity[..7] != b"\x7fELF\x02\x01\x01"
        || identity[16..18] != [3, 0]
        || identity[18..20] != NativeBuildSettings::elf_machine().to_le_bytes()
    {
        return Err(reject(
            "library linker did not produce a native ELF64 shared object",
        ));
    }
    fs::write(child(&complete, HEADER)?, header).map_err(io)?;
    let library_hash = digest_file(&library)?;
    let header_hash = digest_file(&child(&complete, HEADER)?)?;
    let compiler = std::env::current_exe().map_err(io)?;
    path_admitted(&compiler)?;
    let compiler_hash = digest_file(&compiler)?;
    let ir_hash = hex(&Sha256::digest(module.as_bytes()).into());
    let metadata = Metadata {
        options,
        project,
        parameters,
        target: NativeBuildSettings::new(false).target_triple(),
        library_sha256: ascii(&library_hash),
        header_sha256: ascii(&header_hash),
        compiler_sha256: ascii(&compiler_hash),
        ir_sha256: ascii(&ir_hash),
    };
    let mut file = File::create(child(&complete, "metadata.json")?).map_err(io)?;
    serde_json::to_writer_pretty(&mut file, &metadata)
        .map_err(|e| failure(format!("cannot serialize native library metadata: {e}")))?;
    drop(file);
    publish(&complete, &output)
}
