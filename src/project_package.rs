//! Retained typed source bundles, with bounded format verification and atomic
//! publication. This is not a general archive extractor or a build-script hook.
mod archive;
pub(crate) use archive::{Archive as LibraryArchive, Info as LibraryInfo, Member as LibraryMember};

pub(crate) fn write_library(
    archive: &LibraryArchive,
    output: &mut impl io::Write,
    budget: &mut crate::project_registry::Budget,
) -> Result<LibraryInfo, String> {
    archive::write_library(archive, output, budget)
        .map_err(|error| format!("invalid library source artifact: {error}"))
}
pub(crate) fn read_library(
    path: &Path,
    budget: &mut crate::project_registry::Budget,
) -> Result<(LibraryArchive, LibraryInfo), String> {
    let (mut file, before) =
        crate::project_registry::open_regular(path, archive::MAX_ARCHIVE_BYTES as usize)?;
    let result = archive::read_library(&mut file, budget)
        .map_err(|error| format!("invalid library source artifact: {error}"))?;
    crate::project_registry::recheck(path, &file, &before)?;
    if result.1.bytes != before.len() {
        return Err("library artifact changed during reading".into());
    }
    Ok(result)
}
pub(crate) fn library_control_bytes() -> usize {
    archive::library_control_bytes()
}

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

/// Produced from admitted compiler/manifest/lock buffers, never filesystem
/// recursion. Paths in `files` include the canonical `source/` archive prefix.
pub(crate) struct SourcePlan {
    pub owner: PathBuf,
    pub root: String,
    pub files: Vec<(String, Vec<u8>)>,
    pub input_paths: Vec<PathBuf>,
}

/// A detached local graph, not a replacement source for the original lock.
pub(crate) struct VendorSummary {
    pub output: PathBuf,
    pub root: PathBuf,
    pub sha256: String,
    pub bytes: u64,
    pub files: usize,
}

const VENDOR_PATH_BYTES: usize = 8192;
const VENDOR_PATH_CAPACITY: usize = 2 * VENDOR_PATH_BYTES;
// This additional bridge has its own stricter ceiling. Existing collector,
// archive, staging and compiler ceilings are unchanged.
const VENDOR_BRIDGE_BYTES: usize = 512 * 1024;
type VendorPayload = Vec<(String, Vec<u8>)>;
type VendorPayloadIterator = std::vec::IntoIter<(String, Vec<u8>)>;
type VendorMembersResult = Result<Vec<archive::Member>, String>;
type VendorReservation = Result<(), std::collections::TryReserveError>;
type VendorResult = Result<VendorSummary, String>;
type VendorBound = Result<usize, &'static str>;
type VendorAdmissionResult = Result<VendorAdmission, String>;
type VendorPathResult = Result<PathBuf, String>;
type VendorInfoResult = Result<archive::Info, String>;
type VendorWorkspaceResult = Result<Option<crate::runtime::WorkspaceManifest>, String>;
type VendorLocationResult = Result<bool, String>;

#[repr(C)]
struct VendorLocationRoles {
    ancestors: std::iter::Enumerate<std::path::Ancestors<'static>>,
    next: Option<(usize, &'static Path)>,
    directory: &'static Path,
    depth: usize,
    candidate_return: VendorPathResult,
    candidate: PathBuf,
    metadata_return: io::Result<fs::Metadata>,
    before: fs::Metadata,
    opened: fs::Metadata,
    after: fs::Metadata,
    file_return: io::Result<File>,
    file: File,
    options: OpenOptions,
    raw: Vec<u8>,
    reservation: VendorReservation,
    mapped_reservation: Result<(), String>,
    read_return: io::Result<()>,
    witness_return: io::Result<usize>,
    declaration_construction: VendorWorkspaceResult,
    declaration_return: VendorWorkspaceResult,
    declaration_caller: VendorWorkspaceResult,
    declaration: Option<crate::runtime::WorkspaceManifest>,
    lossy: std::borrow::Cow<'static, str>,
    pending: Option<(String, String)>,
    key: String,
    value: String,
    decoded: String,
    arrays: [Vec<String>; 2],
    location_construction: VendorLocationResult,
    location_return: VendorLocationResult,
    location_caller: VendorLocationResult,
    input: &'static [u8],
    file_length: usize,
    scope: usize,
    saved_admission: usize,
    checked: Option<usize>,
    file_length_result: Result<usize, std::num::TryFromIntError>,
    files: &'static [archive::Member],
    files_iter: std::slice::Iter<'static, archive::Member>,
    member: &'static archive::Member,
    relative: &'static Path,
    components: std::path::Components<'static>,
    base_depth: usize,
    relative_depth: usize,
}

/// Complete new bridge transports, conservatively summed across its phases.
/// Existing source collection/re-admission and materialization helpers retain
/// their inherited baselines. Payload allocations move between owners; they
/// are governed by the existing 32 MiB ceiling, not copied into this fee.
#[repr(C)]
struct VendorBridgeRoles {
    source_argument: VendorPayload,
    source_iterator: VendorPayloadIterator,
    source_next: Option<(String, Vec<u8>)>,
    source_selected: (String, Vec<u8>),
    member: archive::Member,
    members: Vec<archive::Member>,
    members_construction: VendorMembersResult,
    members_return: VendorMembersResult,
    members_caller: VendorMembersResult,
    reservation: VendorReservation,
    mapped_reservation: Result<(), String>,
    plan_return: Result<SourcePlan, String>,
    retained_plan: SourcePlan,
    archive: archive::Archive,
    destination_return: Result<PathBuf, String>,
    destination: PathBuf,
    path_construction: VendorPathResult,
    path_return: VendorPathResult,
    path_caller: VendorPathResult,
    stage_return: Result<PrivateDirectory, String>,
    stage: PrivateDirectory,
    exported_root: PathBuf,
    info_return: io::Result<archive::Info>,
    info_mapped: VendorInfoResult,
    info: archive::Info,
    summary: VendorSummary,
    summary_construction: VendorResult,
    summary_return: VendorResult,
    summary_caller: VendorResult,
    admission: VendorAdmission,
    admission_construction: VendorAdmissionResult,
    admission_return: VendorAdmissionResult,
    admission_caller: VendorAdmissionResult,
    parent: Option<&'static Path>,
    check_return: Result<(), String>,
    requested_slots: usize,
    requested_bytes: Option<usize>,
    bound_construction: VendorBound,
    bound_return: VendorBound,
    bound_caller: VendorBound,
    selected_bound: usize,
    path_input: &'static Path,
    path_output: PathBuf,
    path_reservation: VendorReservation,
    location: VendorLocationRoles,
    requested_name: Option<&'static std::ffi::OsStr>,
}

struct VendorAdmission {
    used: usize,
    limit: usize,
}

impl VendorAdmission {
    fn new(limit: usize) -> VendorAdmissionResult {
        let mut admission = Self { used: 0, limit };
        admission.charge(std::mem::size_of::<VendorBridgeRoles>())?;
        admission.charge(crate::project_vendor::control_bytes())?;
        // Eight bridge/caller path buffers, including destination temporaries
        // and the cwd owner. Existing PathBuf helpers can double a buffer while
        // appending, so prepay twice the admitted path length for each role.
        // Compiler collector paths keep their inherited baseline.
        admission.charge(8 * VENDOR_PATH_CAPACITY)?;
        Ok(admission)
    }

    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        let next = self
            .used
            .checked_add(bytes)
            .ok_or("vendor metadata overflow")?;
        if next > self.limit {
            return Err("offline export metadata is too large; use a smaller project or a different output parent".into());
        }
        self.used = next;
        Ok(())
    }
}

fn vendor_member_slots(capacity: usize, count: usize) -> VendorBound {
    capacity
        .checked_add(count)
        .and_then(|count| count.checked_mul(std::mem::size_of::<archive::Member>()))
        .ok_or("vendor member metadata overflow")
}

fn vendor_members(files: VendorPayload, admission: &mut VendorAdmission) -> VendorMembersResult {
    if files.len() > archive::MAX_FILES {
        return Err("vendor exceeds the existing 2047 payload-file limit".into());
    }
    let slots = vendor_member_slots(files.capacity(), files.len())?;
    // Both the consumed tuple buffer and destination member buffer exist while
    // transferring. Strings and payload Vecs themselves retain their allocation.
    admission.charge(slots)?;
    let mut members = Vec::new();
    #[cfg(test)]
    vendor_test_reservation()?;
    let reservation: VendorReservation = members.try_reserve_exact(files.len());
    let mapped: Result<(), String> =
        reservation.map_err(|_| "cannot reserve vendor members".into());
    mapped?;
    if members.capacity() != files.len() {
        return Err("offline file list needs too much memory; reduce the number of files".into());
    }
    let source: VendorPayloadIterator = files.into_iter();
    for (path, bytes) in source {
        members.push(archive::Member { path, bytes });
    }
    Ok(members)
}

fn vendor_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().len() > VENDOR_PATH_BYTES
        || path.to_str().is_none()
        || path
            .as_os_str()
            .as_encoded_bytes()
            .iter()
            .any(|byte| byte.is_ascii_control())
    {
        return Err("vendor paths require UTF-8 without controls and at most 8192 bytes".into());
    }
    Ok(())
}

fn vendor_join_length(base: &Path, relative: &str) -> VendorBound {
    base.as_os_str()
        .len()
        .checked_add(relative.len())
        .and_then(|length| length.checked_add(1))
        .ok_or("vendor path overflow")
}

fn vendor_join(base: &Path, relative: &str) -> VendorPathResult {
    let length = vendor_join_length(base, relative)?;
    if length > VENDOR_PATH_BYTES {
        return Err("offline project path exceeds 8192 bytes; choose a shorter output path".into());
    }
    let mut path = PathBuf::new();
    let reservation: VendorReservation = path.try_reserve_exact(length);
    let mapped: Result<(), String> =
        reservation.map_err(|_| "cannot reserve exported vendor root".into());
    mapped?;
    path.push(base);
    path.push(relative);
    if path.capacity() > VENDOR_PATH_BYTES {
        return Err(
            "offline project path needs too much memory; choose a shorter output path".into(),
        );
    }
    Ok(path)
}

fn vendor_workspace_scope(length: usize) -> VendorBound {
    // The shared discovery parser may retain a lossy UTF-8 copy (3n), pending
    // text, decoded strings and a temporary key/error. String growth can double
    // each request. 32n covers those overlapping text roles including this raw
    // n+1 buffer; the two workspace arrays can each reach 64 String slots before
    // their 32-member refusal. Fixed parser/I/O carriers are charged above.
    length
        .checked_add(1)
        .and_then(|n| n.checked_mul(32))
        .and_then(|n| n.checked_add(2 * 64 * std::mem::size_of::<String>() + 4096))
        .ok_or("vendor destination discovery storage overflow")
}

fn vendor_workspace_file(
    path: &Path,
    before: &fs::Metadata,
    length: usize,
) -> VendorLocationResult {
    use std::io::Read;
    let mut bytes = Vec::new();
    #[cfg(test)]
    vendor_test_reservation()?;
    let reservation: VendorReservation = bytes.try_reserve_exact(length + 1);
    let mapped: Result<(), String> =
        reservation.map_err(|_| "cannot reserve vendor destination discovery bytes".into());
    mapped?;
    if bytes.capacity() != length + 1 {
        return Err(
            "offline output parent needs too much memory to inspect; choose another parent".into(),
        );
    }
    bytes.resize(length + 1, 0);
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("cannot inspect vendor destination manifest: {error}"))?;
    let opened = file.metadata().map_err(|error| error.to_string())?;
    if !opened.is_file() || !same_file(before, &opened) || opened.len() != length as u64 {
        return Err("vendor destination manifest changed during admission".into());
    }
    file.read_exact(&mut bytes[..length])
        .map_err(|error| error.to_string())?;
    if file
        .read(&mut bytes[length..])
        .map_err(|error| error.to_string())?
        != 0
    {
        return Err("vendor destination manifest grew during admission".into());
    }
    let after = file.metadata().map_err(|error| error.to_string())?;
    if after.len() != length as u64 || !same_file(before, &after) {
        return Err("vendor destination manifest changed during admission".into());
    }
    bytes.truncate(length);
    let parsed: VendorWorkspaceResult = crate::runtime::workspace_declaration_bytes(&bytes);
    let declaration = parsed?;
    Ok(declaration.is_some())
}

fn vendor_destination_context(
    output: &Path,
    admission: &mut VendorAdmission,
) -> Result<(), String> {
    let parent = output.parent().ok_or("vendor output has no parent")?;
    for (depth, directory) in parent.ancestors().enumerate() {
        if depth > 64 {
            return Err("vendor destination exceeds the existing 64-ancestor discovery limit; choose a shallower output parent".into());
        }
        let candidate = vendor_join(directory, "oxid.toml")?;
        let declared = match fs::symlink_metadata(&candidate) {
            Ok(before) => {
                if !before.file_type().is_file() || before.len() > 1024 * 1024 {
                    return Err(
                        "vendor destination ancestors require ordinary manifests of at most 1 MiB"
                            .into(),
                    );
                }
                let length = usize::try_from(before.len())
                    .map_err(|_| "vendor discovery length overflow")?;
                let saved = admission.used;
                admission.charge(vendor_workspace_scope(length)?)?;
                let result = vendor_workspace_file(&candidate, &before, length);
                admission.used = saved;
                result?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => {
                return Err(format!(
                    "cannot inspect vendor destination ancestor: {error}"
                ))
            }
        };
        if declared {
            return Err(format!("offline checkout destination is inside the workspace at {}; choose an output parent outside that workspace", directory.display()));
        }
        // Match ordinary workspace and source-package discovery when a deleted
        // declaration leaves an authoritative v3/v4 workspace lock behind.
        let saved = admission.used;
        admission.charge(1024)?;
        let marker: VendorLocationResult = crate::project_lock::has_workspace_marker(directory);
        admission.used = saved;
        if marker? {
            return Err(format!("offline checkout destination is beneath a workspace lock at {}; restore its workspace declaration or choose an output parent outside it", directory.display()));
        }
    }
    Ok(())
}

fn vendor_final_depth(output: &Path, archive: &archive::Archive) -> Result<(), String> {
    let base = output.ancestors().count();
    for member in &archive.files {
        if Path::new(&member.path)
            .file_name()
            .is_some_and(|name| name == "oxid.toml")
        {
            let nested = Path::new(&member.path).components().count() - 1;
            if base.checked_add(nested).is_none_or(|count| count > 65) {
                return Err("exported project roots exceed the existing 64-ancestor discovery limit; choose a shallower output parent".into());
            }
        }
    }
    Ok(())
}

pub(crate) fn vendor_directory(start: &Path, requested: &Path, workspace: bool) -> VendorResult {
    vendor_directory_with_limit(start, requested, workspace, VENDOR_BRIDGE_BYTES)
}

fn vendor_directory_with_limit(
    start: &Path,
    requested: &Path,
    workspace: bool,
    limit: usize,
) -> VendorResult {
    if !cfg!(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        return Err("offline typed checkouts require Linux x86_64 or ARM64".into());
    }
    let mut admission = VendorAdmission::new(limit)?;
    vendor_path(start)?;
    vendor_path(requested)?;
    let requested_name = requested.file_name();
    if requested_name == Some(std::ffi::OsStr::new("oxid.toml")) {
        return Err(
            "offline checkout output cannot be named oxid.toml; choose a different directory name"
                .into(),
        );
    }
    // Do not create parent directories, follow a final symlink, or overwrite a
    // destination while discovering/validating the complete input graph.
    let output = destination(requested, false)?;
    vendor_path(&output)?;
    if output.capacity() > VENDOR_PATH_BYTES {
        return Err("offline output path needs too much memory; choose a shorter path".into());
    }
    require_absent(&output)?;
    vendor_destination_context(&output, &mut admission)?;
    let SourcePlan {
        owner: _,
        root,
        files,
        input_paths,
    } = crate::project_cli::package_source_plan(start, None, workspace)?;
    validate_archive_destination(&output, &input_paths)?;
    let exported_root = vendor_join(&output, &root)?;
    let archive = archive::Archive {
        root,
        files: vendor_members(files, &mut admission)?,
    };
    // Exact same canonical source-package validator/hash as `package`, before
    // creating staging files. No intermediate archive or payload clone.
    let formatted: VendorInfoResult = archive::write(&archive, &mut io::sink())
        .map_err(|error| format!("cannot format offline project: {error}"));
    let info = formatted?;
    vendor_final_depth(&output, &archive)?;
    vendor_destination_context(&output, &mut admission)?;
    let parent = output.parent().ok_or("vendor output has no parent")?;
    // PrivateDirectory's generated component is at most 78 bytes (u32 PID,
    // u128 timestamp and u64 counter). Reserve 128, plus a separator, before
    // invoking that inherited helper or creating its directory.
    if parent.as_os_str().len() > VENDOR_PATH_BYTES - 129 {
        return Err("vendor output parent leaves insufficient private staging path space".into());
    }
    let mut stage = PrivateDirectory::create(parent)?;
    vendor_path(&stage.path)?;
    if stage.path.capacity() > VENDOR_PATH_CAPACITY {
        return Err(
            "offline export needs too much temporary path storage; choose a shorter output path"
                .into(),
        );
    }
    materialize(&archive, &stage.path)?;
    validate_materialized(&archive, &stage.path)?;
    validate_archive_destination(&output, &input_paths)?;
    sync_directory(&stage.path).map_err(|error| format!("cannot sync offline project: {error}"))?;
    #[cfg(test)]
    vendor_test_fault(VendorFault::BeforePublish)?;
    vendor_destination_context(&output, &mut admission)?;
    publish_directory(&stage.path, &output)?;
    stage.published = true;
    let synced = vendor_sync_parent(parent);
    if let Err(error) = synced {
        return Err(format!("offline project was published at {}, but its parent directory could not be synced: {error}", exported_root.display()));
    }
    Ok(VendorSummary {
        output,
        root: exported_root,
        sha256: info.sha256,
        bytes: info.bytes,
        files: info.file_count,
    })
}

fn vendor_sync_parent(parent: &Path) -> io::Result<()> {
    #[cfg(test)]
    if vendor_test_fault(VendorFault::ParentSync).is_err() {
        return Err(io::Error::other("injected vendor parent sync failure"));
    }
    sync_directory(parent)
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum VendorFault {
    Reservation,
    BeforePublish,
    ParentSync,
}

#[cfg(test)]
thread_local! {
    static VENDOR_FAULT: std::cell::Cell<Option<VendorFault>> = const { std::cell::Cell::new(None) };
    static VENDOR_RESERVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn vendor_test_fault(point: VendorFault) -> Result<(), String> {
    if VENDOR_FAULT.with(|fault| fault.get() == Some(point)) {
        Err("injected vendor failure".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
fn vendor_test_reservation() -> Result<(), String> {
    VENDOR_RESERVATIONS.with(|count| count.set(count.get() + 1));
    vendor_test_fault(VendorFault::Reservation)
}

#[cfg(test)]
#[path = "project_package/vendor_tests.rs"]
mod vendor_tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operation {
    Create,
    Verify,
    Extract,
}

impl Operation {
    fn name(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Verify => "verify",
            Self::Extract => "extract",
        }
    }
}

struct Options {
    operation: Operation,
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    sha256: Option<String>,
    workspace: bool,
    json: bool,
}

struct Summary {
    operation: Operation,
    archive: PathBuf,
    output: Option<PathBuf>,
    root: String,
    info: archive::Info,
}

const HELP: &str = "Typed source packages (experimental):\n  oxid package [--frozen|--locked|--offline] [--workspace] [--output ARCHIVE]\n  oxid package --verify ARCHIVE [--sha256 HEX]\n  oxid package --extract ARCHIVE --output NEW_DIRECTORY [--sha256 HEX]\n\nCreation requires a matching existing typed lock and includes the whole workspace.\nIt includes cached Git libraries as local sources and never fetches.\nIt checks retained source and declared tests without executing them. Verification\nchecks archive integrity; extraction also checks the isolated project and lock.\n--edition=typed-preview and --message-format=text|json are accepted.\n";

/// Own `package` before project discovery so verify/extract need no local
/// manifest. A source filename or argument of another command is never stolen.
pub(crate) fn dispatch(args: &[String]) -> Option<i32> {
    let mut index = 0;
    let command = loop {
        let argument = args.get(index)?;
        if argument == "--" {
            return None;
        }
        if matches!(argument.as_str(), "--edition" | "--message-format") {
            index += 2;
        } else if argument.starts_with("--edition=") || argument.starts_with("--message-format=") {
            index += 1;
        } else if argument.starts_with('-') {
            return None;
        } else {
            break index;
        }
    };
    if args[command] != "package" {
        return None;
    }
    if args.len() == 2 && command == 0 && matches!(args[1].as_str(), "--help" | "-h") {
        return Some(if io::stdout().lock().write_all(HELP.as_bytes()).is_ok() {
            0
        } else {
            74
        });
    }
    let json = args.iter().any(|value| value == "--message-format=json")
        || args
            .windows(2)
            .any(|values| values == ["--message-format", "json"]);
    let guessed_operation = if args
        .iter()
        .any(|value| value == "--extract" || value.starts_with("--extract="))
    {
        Operation::Extract
    } else if args
        .iter()
        .any(|value| value == "--verify" || value.starts_with("--verify="))
    {
        Operation::Verify
    } else {
        Operation::Create
    };
    let options = match parse_options(args, command) {
        Ok(options) => options,
        Err(error) => return Some(report(guessed_operation, json, Err(error))),
    };
    let result = match options.operation {
        Operation::Create => create(&options),
        Operation::Verify | Operation::Extract => verify_or_extract(&options),
    };
    Some(report(options.operation, options.json, result))
}

fn parse_options(args: &[String], command: usize) -> Result<Options, String> {
    let mut options = Options {
        operation: Operation::Create,
        input: None,
        output: None,
        sha256: None,
        workspace: false,
        json: false,
    };
    let mut seen = std::collections::HashSet::new();
    let mut index = 0;
    while index < args.len() {
        if index == command {
            index += 1;
            continue;
        }
        let (name, inline) = args[index]
            .split_once('=')
            .map_or((args[index].as_str(), None), |(name, value)| {
                (name, Some(value))
            });
        if !seen.insert(name) {
            return Err(format!("{name} may only be specified once"));
        }
        if matches!(name, "--locked" | "--offline" | "--frozen" | "--workspace") {
            if inline.is_some() {
                return Err(format!("{name} is a flag and does not take a value"));
            }
            options.workspace |= name == "--workspace";
            index += 1;
            continue;
        }
        if !matches!(
            name,
            "--edition" | "--message-format" | "--output" | "--verify" | "--extract" | "--sha256"
        ) {
            return Err(format!("unsupported package option: {name}"));
        }
        let value = if let Some(value) = inline {
            value
        } else {
            index += 1;
            args.get(index)
                .filter(|value| !value.starts_with('-'))
                .map(String::as_str)
                .ok_or_else(|| format!("{name} requires a value"))?
        };
        if value.is_empty() {
            return Err(format!("{name} requires a nonempty value"));
        }
        match name {
            "--edition" if value == "typed-preview" => {}
            "--message-format" if matches!(value, "text" | "json") => {
                options.json = value == "json"
            }
            "--output" => options.output = Some(PathBuf::from(value)),
            "--verify" | "--extract" => {
                if options.input.is_some() {
                    return Err("--verify and --extract are mutually exclusive".into());
                }
                options.operation = if name == "--verify" {
                    Operation::Verify
                } else {
                    Operation::Extract
                };
                options.input = Some(PathBuf::from(value));
            }
            "--sha256"
                if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) =>
            {
                options.sha256 = Some(value.to_ascii_lowercase());
            }
            _ => return Err(format!("unsupported value for {name}")),
        }
        index += 1;
    }
    if options.operation == Operation::Create {
        if options.sha256.is_some() {
            return Err("--sha256 requires --verify or --extract".into());
        }
    } else {
        if seen
            .iter()
            .any(|name| matches!(*name, "--locked" | "--offline" | "--frozen" | "--workspace"))
        {
            return Err("locking and workspace flags apply only to package creation".into());
        }
        if options.operation == Operation::Verify && options.output.is_some() {
            return Err("--output requires package creation or --extract".into());
        }
        if options.operation == Operation::Extract && options.output.is_none() {
            return Err("--extract requires --output NEW_DIRECTORY".into());
        }
    }
    Ok(options)
}

fn create(options: &Options) -> Result<Summary, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("cannot find current directory: {error}"))?;
    let plan = crate::project_cli::package_source_plan(&cwd, None, options.workspace)?;
    let requested_output = options
        .output
        .clone()
        .unwrap_or_else(|| plan.owner.join(".oxid/package/source.oxpkg.tar"));
    let output = destination(&requested_output, true)?;
    validate_archive_destination(&output, &plan.input_paths)?;
    let input_paths = plan.input_paths;
    let archive = archive::Archive {
        root: plan.root,
        files: plan
            .files
            .into_iter()
            .map(|(path, bytes)| archive::Member { path, bytes })
            .collect(),
    };
    // The format validator runs before any archive path is materialized.
    let expected_info = archive::write(&archive, &mut io::sink())
        .map_err(|error| format!("cannot format source package: {error}"))?;
    let parent = output.parent().ok_or("package output has no parent")?;
    let stage = PrivateDirectory::create(parent)?;
    let tree = stage.path.join("tree");
    private_directory(&tree)?;
    materialize(&archive, &tree)?;
    validate_materialized(&archive, &tree)?;

    let temporary = stage.path.join("archive.tmp");
    let mut file = new_file(&temporary)?;
    let info = archive::write(&archive, &mut file)
        .map_err(|error| format!("cannot write source package: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("cannot sync source package: {error}"))?;
    drop(file);
    let (checked_archive, checked_info) = read_archive(&temporary)?;
    if info != expected_info || checked_info != info || checked_archive != archive {
        return Err("source package changed during private publication".into());
    }
    validate_archive_destination(&output, &input_paths)?;
    fs::rename(&temporary, &output)
        .map_err(|error| format!("cannot publish source package: {error}"))?;
    sync_directory(parent).map_err(|error| {
        format!("source package was published but its directory could not be synced: {error}")
    })?;
    Ok(Summary {
        operation: Operation::Create,
        archive: output.clone(),
        output: Some(output),
        root: archive.root,
        info,
    })
}

fn verify_or_extract(options: &Options) -> Result<Summary, String> {
    let input = options
        .input
        .as_deref()
        .ok_or("package operation requires an archive")?;
    let input = absolute_path(input)?;
    let (archive, info) = read_archive(&input)?;
    if options
        .sha256
        .as_ref()
        .is_some_and(|expected| expected != &info.sha256)
    {
        return Err("package archive does not match the supplied SHA-256".into());
    }
    let output = if options.operation == Operation::Extract {
        let output = destination(
            options
                .output
                .as_deref()
                .ok_or("extraction requires an output directory")?,
            false,
        )?;
        require_absent(&output)?;
        let parent = output.parent().ok_or("package destination has no parent")?;
        let mut stage = PrivateDirectory::create(parent)?;
        materialize(&archive, &stage.path)?;
        validate_materialized(&archive, &stage.path)?;
        sync_directory(&stage.path)
            .map_err(|error| format!("cannot sync extracted package: {error}"))?;
        publish_directory(&stage.path, &output)?;
        stage.published = true;
        sync_directory(parent).map_err(|error| {
            format!("package was extracted but its parent could not be synced: {error}")
        })?;
        Some(output)
    } else {
        None
    };
    Ok(Summary {
        operation: options.operation,
        archive: input,
        output,
        root: archive.root,
        info,
    })
}

fn validate_materialized(archive: &archive::Archive, stage: &Path) -> Result<(), String> {
    let plan =
        crate::project_cli::package_source_plan(&stage.join(&archive.root), Some(stage), false)?;
    if plan.root != archive.root
        || plan.files.len() != archive.files.len()
        || plan
            .files
            .iter()
            .zip(&archive.files)
            .any(|((path, bytes), file)| path != &file.path || bytes != &file.bytes)
    {
        return Err(
            "materialized package does not match its complete retained source and lock closure"
                .into(),
        );
    }
    Ok(())
}

pub(crate) fn materialize(archive: &archive::Archive, stage: &Path) -> Result<(), String> {
    // Bound implicit directories too: a tiny file can otherwise carry a large
    // chain of parents without contributing to the payload byte budget.
    let mut directories = std::collections::BTreeSet::new();
    for member in &archive.files {
        let relative = Path::new(&member.path);
        if relative.components().count() > 32 {
            return Err("source package path exceeds 32 components".into());
        }
        for parent in relative
            .ancestors()
            .skip(1)
            .filter(|path| !path.as_os_str().is_empty())
        {
            directories.insert(stage.join(parent));
            if directories.len() > 4096 {
                return Err("source package exceeds 4096 implicit directories".into());
            }
        }
    }
    for member in &archive.files {
        let path = stage.join(&member.path);
        let parent = path.parent().ok_or("package file has no parent")?;
        ensure_directories(stage, parent)?;
        let mut file = new_file(&path)?;
        file.write_all(&member.bytes)
            .map_err(|error| format!("cannot materialize source package: {error}"))?;
        // Every member is an ordinary 0644 file irrespective of the caller's
        // umask. The enclosing fresh private directory remains 0700.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o644))
                .map_err(|error| format!("cannot set package member permissions: {error}"))?;
        }
        file.sync_all()
            .map_err(|error| format!("cannot sync package member: {error}"))?;
    }
    for directory in directories.iter().rev() {
        sync_directory(directory)
            .map_err(|error| format!("cannot sync package directory: {error}"))?;
    }
    Ok(())
}

pub(crate) fn new_file(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    options
        .open(path)
        .map_err(|error| format!("cannot create package file {}: {error}", path.display()))
}

fn read_archive(path: &Path) -> Result<(archive::Archive, archive::Info), String> {
    let before = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect package archive {}: {error}", path.display()))?;
    if !before.is_file() || before.len() > archive::MAX_ARCHIVE_BYTES {
        return Err("package archive must be an ordinary regular file of at most 64 MiB".into());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("cannot open package archive: {error}"))?;
    let admitted = file
        .metadata()
        .map_err(|error| format!("cannot inspect opened package archive: {error}"))?;
    if !same_file(&before, &admitted) || !admitted.is_file() {
        return Err("package archive changed while opening".into());
    }
    let result =
        archive::read(&mut file).map_err(|error| format!("invalid source package: {error}"))?;
    let after = file
        .metadata()
        .map_err(|error| format!("cannot recheck package archive: {error}"))?;
    let current = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot recheck package archive path: {error}"))?;
    if !same_file(&admitted, &after)
        || !same_file(&after, &current)
        || result.1.bytes != after.len()
    {
        return Err("package archive changed during verification".into());
    }
    Ok(result)
}

fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    let same = left.is_file() == right.is_file()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        same && left.dev() == right.dev()
            && left.ino() == right.ino()
            && left.mode() == right.mode()
            && left.ctime() == right.ctime()
            && left.ctime_nsec() == right.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        same
    }
}

fn validate_archive_destination(path: &Path, inputs: &[PathBuf]) -> Result<(), String> {
    if inputs.iter().any(|input| path.starts_with(input)) {
        return Err("package output collides with an admitted input".into());
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err("package output must be absent or a non-symlink regular file".into());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                for input in inputs {
                    if let Ok(input_metadata) = fs::symlink_metadata(input) {
                        if input_metadata.dev() == metadata.dev()
                            && input_metadata.ino() == metadata.ino()
                        {
                            return Err("package output aliases an admitted input".into());
                        }
                    }
                }
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot inspect package output: {error}")),
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|error| format!("cannot find current directory: {error}"))
    }
}

/// Resolve only the parent. The final component must remain unfollowed so a
/// dangling symlink is distinguishable from an absent destination.
fn destination(path: &Path, create_parents: bool) -> Result<PathBuf, String> {
    let absolute = absolute_path(path)?;
    let name = absolute
        .file_name()
        .ok_or("package output requires a file or directory name")?;
    if absolute
        .components()
        .next_back()
        .is_none_or(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("package output requires a final ordinary path component".into());
    }
    let parent = absolute.parent().ok_or("package output has no parent")?;
    let mut walked = PathBuf::new();
    for component in parent.components() {
        match component {
            Component::RootDir | Component::Prefix(_) => walked.push(component),
            Component::CurDir => {}
            Component::ParentDir => {
                walked.pop();
            }
            Component::Normal(part) => {
                walked.push(part);
                match fs::symlink_metadata(&walked) {
                    Ok(metadata) if metadata.is_dir() => {}
                    Ok(_) => {
                        return Err(format!(
                            "package output parent is not an ordinary directory: {}",
                            walked.display()
                        ))
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound && create_parents => {
                        match fs::create_dir(&walked) {
                            Ok(()) => {}
                            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                                require_directory(&walked)?
                            }
                            Err(error) => {
                                return Err(format!("cannot create package output parent: {error}"))
                            }
                        }
                        // Persist each new directory entry before descending;
                        // syncing only the final archive parent is insufficient
                        // for a newly created chain of output ancestors.
                        sync_directory(&walked)
                            .and_then(|()| {
                                sync_directory(walked.parent().unwrap_or(Path::new("/")))
                            })
                            .map_err(|error| {
                                format!("cannot sync package output parent: {error}")
                            })?;
                    }
                    Err(error) => {
                        return Err(format!("cannot inspect package output parent: {error}"))
                    }
                }
            }
        }
    }
    Ok(walked.join(name))
}

pub(crate) fn require_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect package directory: {error}"))?;
    if metadata.is_dir() {
        Ok(())
    } else {
        Err("package path is not an ordinary directory".into())
    }
}

fn ensure_directories(root: &Path, parent: &Path) -> Result<(), String> {
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| "package directory escapes private staging")?;
    let mut path = root.to_owned();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err("invalid private package directory".into());
        };
        path.push(name);
        match fs::create_dir(&path) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).map_err(
                        |error| format!("cannot set package directory permissions: {error}"),
                    )?;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => require_directory(&path)?,
            Err(error) => return Err(format!("cannot create package directory: {error}")),
        }
    }
    Ok(())
}

fn private_directory(path: &Path) -> Result<(), String> {
    private_mkdir(path).map_err(|error| format!("cannot create private package directory: {error}"))
}

fn private_mkdir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(path)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir(path)
    }
}

pub(crate) struct PrivateDirectory {
    pub(crate) path: PathBuf,
    pub(crate) published: bool,
}

impl PrivateDirectory {
    pub(crate) fn create(parent: &Path) -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("cannot name private package storage: {error}"))?
            .as_nanos();
        for _ in 0..32 {
            let path = parent.join(format!(
                ".oxid-package-{}-{stamp:x}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match private_mkdir(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        published: false,
                    })
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!("cannot create private package storage: {error}"))
                }
            }
        }
        Err("cannot allocate fresh private package storage".into())
    }
}

impl Drop for PrivateDirectory {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn require_absent(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot inspect extraction destination: {error}")),
        Ok(_) => Err("package extraction destination already exists".into()),
    }
}

pub(crate) fn publish_directory(source: &Path, destination: &Path) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let source = CString::new(source.as_os_str().as_bytes())
            .map_err(|_| "package stage contains NUL")?;
        let destination = CString::new(destination.as_os_str().as_bytes())
            .map_err(|_| "package destination contains NUL")?;
        // RENAME_NOREPLACE preserves all existing destinations, including
        // empty directories and symlinks, even between concurrent extractors.
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                destination.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(format!(
                "cannot publish extracted package without replacement: {}",
                io::Error::last_os_error()
            ))
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (source, destination);
        Err("atomic package extraction is currently qualified only on Linux".into())
    }
}

fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

fn json_string(text: &str) -> String {
    let mut output = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            character if character.is_control() => {
                output.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn report(operation: Operation, json: bool, result: Result<Summary, String>) -> i32 {
    let success = result.is_ok();
    let message = match result {
        Ok(summary) if json => format!("{{\"schema\":\"oxid-typed-package-summary-v1\",\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"package-summary\",\"operation\":{},\"success\":true,\"archive\":{},\"sha256\":{},\"bytes\":{},\"files\":{},\"root\":{},\"output\":{}}}\n",
            json_string(summary.operation.name()), json_string(&summary.archive.to_string_lossy()), json_string(&summary.info.sha256), summary.info.bytes, summary.info.file_count,
            json_string(&summary.root), summary.output.as_ref().map_or("null".into(), |path| json_string(&path.to_string_lossy()))),
        Ok(summary) => {
            let location = if summary.operation == Operation::Extract {
                summary.output.as_ref().map_or_else(|| summary.archive.clone(), |path| path.join(&summary.root))
            } else { summary.archive.clone() };
            format!("package {}: {}\nSHA-256: {}\nproject root: {}\n", summary.operation.name(), location.display(), summary.info.sha256, summary.root)
        }
        Err(error) if json => format!("{{\"schema\":\"oxid-typed-package-summary-v1\",\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"package-summary\",\"operation\":{},\"success\":false,\"error\":{}}}\n", json_string(operation.name()), json_string(&error)),
        Err(error) => format!("error[E0001] (package): {error}\n"),
    };
    let written = if json || success {
        io::stdout().lock().write_all(message.as_bytes())
    } else {
        io::stderr().lock().write_all(message.as_bytes())
    };
    if written.is_err() {
        74
    } else if success {
        0
    } else {
        1
    }
}
