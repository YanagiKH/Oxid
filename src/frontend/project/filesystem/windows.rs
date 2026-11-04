//! Local NTFS module admission under the ordinary stable-filesystem contract.
//! Handles bind inspection to reads; this is not a concurrent-filesystem sandbox.
use super::{
    add, io_error, path_units, resource, scan_entries_at as scan_entries, Allocator, Diagnostic,
    ProjectLimits, SourceUsage, Span,
};
use std::{
    ffi::c_void,
    fs::File,
    io,
    mem::{align_of, offset_of, size_of},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, RawHandle},
    },
    path::{Component, Path, PathBuf, Prefix},
};

const MAX_ANCESTORS: usize = 33;
const MAX_WIDE_PATH: usize = 32_768;
const FILE_READ_ATTRIBUTES: u32 = 0x80;
const GENERIC_READ: u32 = 0x8000_0000;
const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;
const OPEN_EXISTING: u32 = 3;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_FLAG_POSIX_SEMANTICS: u32 = 0x0100_0000;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FILE_TYPE_DISK: u32 = 1;
const VOLUME_NAME_GUID: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct FileTime {
    low: u32,
    high: u32,
}

// FILETIME contains two DWORDs. Rust u64 would give the wrong Win64 alignment.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct FileInformation {
    attributes: u32,
    creation: FileTime,
    access: FileTime,
    write: FileTime,
    volume: u32,
    size_high: u32,
    size_low: u32,
    links: u32,
    index_high: u32,
    index_low: u32,
}

const _: () = {
    assert!(size_of::<FileTime>() == 8);
    assert!(align_of::<FileTime>() == 4);
    assert!(size_of::<FileInformation>() == 52);
    assert!(align_of::<FileInformation>() == 4);
    assert!(offset_of!(FileInformation, attributes) == 0);
    assert!(offset_of!(FileInformation, creation) == 4);
    assert!(offset_of!(FileInformation, access) == 12);
    assert!(offset_of!(FileInformation, write) == 20);
    assert!(offset_of!(FileInformation, volume) == 28);
    assert!(offset_of!(FileInformation, size_high) == 32);
    assert!(offset_of!(FileInformation, size_low) == 36);
    assert!(offset_of!(FileInformation, links) == 40);
    assert!(offset_of!(FileInformation, index_high) == 44);
    assert!(offset_of!(FileInformation, index_low) == 48);
};

#[link(name = "Kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        name: *const u16,
        access: u32,
        sharing: u32,
        security: *const c_void,
        disposition: u32,
        flags: u32,
        template: RawHandle,
    ) -> RawHandle;
    fn GetFileType(file: RawHandle) -> u32;
    fn SetLastError(code: u32);
    fn GetFileInformationByHandle(file: RawHandle, information: *mut FileInformation) -> i32;
    fn GetFinalPathNameByHandleW(file: RawHandle, path: *mut u16, capacity: u32, flags: u32)
        -> u32;
    fn GetVolumeInformationByHandleW(
        file: RawHandle,
        name: *mut u16,
        name_capacity: u32,
        serial: *mut u32,
        maximum_component: *mut u32,
        flags: *mut u32,
        filesystem: *mut u16,
        filesystem_capacity: u32,
    ) -> i32;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    volume: u32,
    index: u64,
}
impl FileInformation {
    fn identity(self) -> Identity {
        Identity {
            volume: self.volume,
            index: (u64::from(self.index_high) << 32) | u64::from(self.index_low),
        }
    }
}

#[derive(Debug)]
struct Ancestor {
    file: File,
    identity: Identity,
}

/// No heap-backed handle roster: the root plus depth-bounded parent chain.
/// File IDs bind these handles; module identity remains the canonical pathname.
#[derive(Debug)]
pub(in crate::frontend::project) struct VerifiedChild {
    pub(in crate::frontend::project) file: File,
    identity: Identity,
    parents: [Option<Ancestor>; MAX_ANCESTORS],
    parents_len: usize,
}
impl VerifiedChild {
    pub(in crate::frontend::project) fn validate(
        &self,
        origin: Span,
    ) -> Result<(), Box<Diagnostic>> {
        self.validate_at(Some(origin))
    }

    pub(in crate::frontend::project) fn validate_at(
        &self,
        origin: Option<Span>,
    ) -> Result<(), Box<Diagnostic>> {
        for parent in &self.parents[..self.parents_len] {
            let parent = parent.as_ref().ok_or_else(|| {
                Diagnostic::new(
                    "E0500",
                    "source-project",
                    "Windows module ancestor ownership invariant failed",
                    origin,
                )
            })?;
            let actual = information(&parent.file, true, origin)?.identity();
            if actual != parent.identity || actual.volume != self.identity.volume {
                return Err(policy("module ancestor changed during loading", origin));
            }
        }
        if information(&self.file, false, origin)?.identity() != self.identity {
            return Err(policy("module source changed during loading", origin));
        }
        Ok(())
    }
}

pub(in crate::frontend::project) fn fixed_scratch_bytes() -> Option<usize> {
    size_of::<VerifiedChild>()
        .checked_add(size_of::<Scratch>())?
        .checked_add(2 * size_of::<FileInformation>())?
        .checked_add(size_of::<[u16; 32]>())?
        .checked_add(3 * size_of::<u32>())
}

/// Each native wide buffer is transient. Its exact requested bytes, the probe
/// path and the fixed handle/query carriers fit the existing 4 MiB path ceiling.
struct Scratch {
    available: usize,
}
impl Scratch {
    fn new(
        probe_bytes: usize,
        limits: ProjectLimits,
        usage: SourceUsage,
        origin: Option<Span>,
    ) -> Result<Self, Box<Diagnostic>> {
        let fixed = fixed_scratch_bytes().ok_or_else(|| super::super::overflow(origin))?;
        let held = add(usage.retained_path_bytes, probe_bytes, origin)
            .and_then(|n| add(n, fixed, origin))?;
        let available = limits
            .path_bytes
            .min(4 * 1024 * 1024)
            .checked_sub(held)
            .ok_or_else(|| resource("Windows module path scratch limit exceeded", origin))?;
        Ok(Self { available })
    }

    fn wide(
        &self,
        units: usize,
        allocator: &mut Allocator,
        origin: Option<Span>,
        kind: &'static str,
    ) -> Result<Vec<u16>, Box<Diagnostic>> {
        let bytes = units
            .checked_mul(size_of::<u16>())
            .ok_or_else(|| super::super::overflow(origin))?;
        if units > MAX_WIDE_PATH || bytes > self.available {
            return Err(resource(
                "Windows module path scratch limit exceeded",
                origin,
            ));
        }
        let mut result = Vec::new();
        allocator
            .vector_exact(&mut result, units, kind)
            .map_err(|error| super::super::reserve_error(error, origin))?;
        Ok(result)
    }
}

pub(in crate::frontend::project) fn separator_units(path: &Path) -> usize {
    usize::from(!matches!(
        path.as_os_str().encode_wide().last(),
        Some(47 | 92)
    ))
}

/// Entry compatibility is preserved until a parsed declaration requests module
/// discovery. Drive-relative, device, stream and normalization aliases are not
/// part of this initial ordinary-path qualification.
pub(in crate::frontend::project) fn admit_invoked_entry(
    entry: &Path,
    origin: Span,
) -> Result<(), Box<Diagnostic>> {
    admit_invoked_entry_at(entry, Some(origin))
}

fn admit_invoked_entry_at(entry: &Path, origin: Option<Span>) -> Result<(), Box<Diagnostic>> {
    for component in entry.components() {
        match component {
            Component::Prefix(prefix) => match prefix.kind() {
                Prefix::Disk(_) | Prefix::VerbatimDisk(_) if entry.has_root() => {}
                _ => return Err(namespace(origin)),
            },
            Component::Normal(name) => {
                let mut last = None;
                for unit in name.encode_wide() {
                    if unit == 0 || unit == u16::from(b':') {
                        return Err(namespace(origin));
                    }
                    last = Some(unit);
                }
                if matches!(last, Some(32 | 46)) {
                    return Err(namespace(origin));
                }
            }
            Component::CurDir | Component::ParentDir | Component::RootDir => {}
        }
    }
    Ok(())
}

pub(in crate::frontend::project) fn admit_package_path(path: &Path) -> Result<(), Box<Diagnostic>> {
    admit_invoked_entry_at(path, None)
}

fn policy(message: &'static str, origin: Option<Span>) -> Box<Diagnostic> {
    super::super::owned_diagnostic::diagnostic("E0005", "source", format_args!("{message}"), origin)
}

fn namespace(origin: Option<Span>) -> Box<Diagnostic> {
    policy(
        "Windows module source policy requires ordinary local drive paths without device, network or stream aliases",
        origin,
    )
}

fn admit_root(root: &Path, origin: Option<Span>) -> Result<(), Box<Diagnostic>> {
    let mut components = root.components();
    if !matches!(
        components.next(),
        Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::VerbatimDisk(_) | Prefix::Disk(_))
    ) || !matches!(components.next(), Some(Component::RootDir))
        || components.any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(namespace(origin));
    }
    Ok(())
}

fn information(
    file: &File,
    directory: bool,
    origin: Option<Span>,
) -> Result<FileInformation, Box<Diagnostic>> {
    // SAFETY: These APIs borrow a live, owned file handle. The output has the
    // checked C layout above and stays writable for the complete call.
    let kind = unsafe {
        SetLastError(0);
        GetFileType(file.as_raw_handle())
    };
    if kind == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(0) {
            return Err(io_error(error, "", origin));
        }
    }
    if kind != FILE_TYPE_DISK {
        return Err(policy(
            "module source handle is not an ordinary disk object",
            origin,
        ));
    }
    let mut value = FileInformation::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut value) } == 0 {
        return Err(io_error(io::Error::last_os_error(), "", origin));
    }
    if value.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(policy(
            "module path contains a Windows reparse point",
            origin,
        ));
    }
    if (value.attributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory {
        return Err(policy(
            if directory {
                "module intermediate component is not a directory"
            } else {
                "module source is not a regular file"
            },
            origin,
        ));
    }
    Ok(value)
}

fn open(
    path: &Path,
    directory: bool,
    scratch: &Scratch,
    allocator: &mut Allocator,
    origin: Option<Span>,
) -> Result<(File, Identity), Box<Diagnostic>> {
    let count = path
        .as_os_str()
        .encode_wide()
        .try_fold(0usize, |count, unit| {
            if unit == 0 {
                return Err(namespace(origin));
            }
            add(count, 1, origin)
        })?;
    let units = add(count, 1, origin)?;
    let mut wide = scratch.wide(units, allocator, origin, "Windows module open path")?;
    wide.extend(path.as_os_str().encode_wide());
    wide.push(0);
    // SAFETY: The admitted buffer contains one terminated native pathname.
    // OPEN_EXISTING never creates/truncates. Null security means non-inheritable.
    // Below-root reparse points are opened, inspected and refused, not followed.
    let raw = unsafe {
        CreateFileW(
            wide.as_ptr(),
            if directory {
                FILE_READ_ATTRIBUTES
            } else {
                GENERIC_READ
            },
            if directory {
                FILE_SHARE_READ | FILE_SHARE_WRITE
            } else {
                FILE_SHARE_READ
            },
            std::ptr::null(),
            OPEN_EXISTING,
            // Directory handles need backup intent. Source files must retain
            // ordinary read-sharing checks, including an exclusive open's denial.
            FILE_FLAG_OPEN_REPARSE_POINT
                | FILE_FLAG_POSIX_SEMANTICS
                | if directory {
                    FILE_FLAG_BACKUP_SEMANTICS
                } else {
                    0
                },
            std::ptr::null_mut(),
        )
    };
    if raw as isize == -1 || raw.is_null() {
        return Err(io_error(io::Error::last_os_error(), "", origin));
    }
    // SAFETY: CreateFileW returned one valid uniquely owned handle. File closes
    // it on all later errors; no other owner receives the raw value.
    let file = unsafe { File::from_raw_handle(raw) };
    let identity = information(&file, directory, origin)?.identity();
    Ok((file, identity))
}

fn volume(file: &File, identity: Identity, origin: Option<Span>) -> Result<(), Box<Diagnostic>> {
    let mut serial = 0;
    let mut maximum_component = 0;
    let mut flags = 0;
    let mut filesystem = [0u16; 32];
    // SAFETY: All non-null output pointers cover their exact documented sizes.
    // The volume-name output is intentionally omitted. This queries the opened
    // root volume, not a drive name resolved independently of that root.
    if unsafe {
        GetVolumeInformationByHandleW(
            file.as_raw_handle(),
            std::ptr::null_mut(),
            0,
            &mut serial,
            &mut maximum_component,
            &mut flags,
            filesystem.as_mut_ptr(),
            filesystem.len() as u32,
        )
    } == 0
    {
        return Err(io_error(io::Error::last_os_error(), "", origin));
    }
    let end = filesystem
        .iter()
        .position(|&unit| unit == 0)
        .ok_or_else(|| {
            policy(
                "Windows module volume returned an invalid filesystem name",
                origin,
            )
        })?;
    if !qualified_volume(
        &filesystem[..end],
        serial,
        identity.volume,
        maximum_component,
    ) {
        return Err(policy(
            "Windows module source policy is qualified only on ordinary local NTFS volumes",
            origin,
        ));
    }
    Ok(())
}

fn qualified_volume(name: &[u16], serial: u32, expected: u32, maximum_component: u32) -> bool {
    name == [
        u16::from(b'N'),
        u16::from(b'T'),
        u16::from(b'F'),
        u16::from(b'S'),
    ] && serial == expected
        && maximum_component == 255
}

fn pathname_error(error: io::Error, flags: u32, origin: Option<Span>) -> Box<Diagnostic> {
    if flags == VOLUME_NAME_GUID && matches!(error.raw_os_error(), Some(1 | 3 | 50 | 87)) {
        return policy(
            "Windows module source policy requires an ordinary local NTFS volume identity",
            origin,
        );
    }
    io_error(error, "", origin)
}

fn query_name(
    file: &File,
    flags: u32,
    scratch: &Scratch,
    allocator: &mut Allocator,
    origin: Option<Span>,
    kind: &'static str,
) -> Result<Vec<u16>, Box<Diagnostic>> {
    // SAFETY: This first call borrows a live handle and requests only a length.
    let required =
        unsafe { GetFinalPathNameByHandleW(file.as_raw_handle(), std::ptr::null_mut(), 0, flags) };
    if required == 0 {
        return Err(pathname_error(io::Error::last_os_error(), flags, origin));
    }
    let units = usize::try_from(required).map_err(|_| super::super::overflow(origin))?;
    let mut wide = scratch.wide(units, allocator, origin, kind)?;
    wide.resize(units, 0);
    // SAFETY: required was bounded and fallibly reserved; resize initialized
    // the whole output range. No returned length is trusted before the checks.
    let count = unsafe {
        GetFinalPathNameByHandleW(file.as_raw_handle(), wide.as_mut_ptr(), required, flags)
    };
    if count == 0 {
        return Err(pathname_error(io::Error::last_os_error(), flags, origin));
    }
    let count = usize::try_from(count).map_err(|_| super::super::overflow(origin))?;
    validate_name_buffer(&wide, count, origin)?;
    wide.truncate(count);
    Ok(wide)
}

fn validate_name_buffer(
    wide: &[u16],
    count: usize,
    origin: Option<Span>,
) -> Result<(), Box<Diagnostic>> {
    if count >= wide.len() || wide[count] != 0 || wide[..count].contains(&0) {
        return Err(policy(
            "module canonical pathname changed during loading",
            origin,
        ));
    }
    if std::char::decode_utf16(wide[..count].iter().copied()).any(|scalar| scalar.is_err()) {
        return Err(policy(
            "Windows module canonical pathname is not valid Unicode",
            origin,
        ));
    }
    Ok(())
}

fn final_name(
    file: &File,
    expected: &Path,
    scratch: &Scratch,
    allocator: &mut Allocator,
    origin: Option<Span>,
) -> Result<(), Box<Diagnostic>> {
    // VOLUME_NAME_DOS | FILE_NAME_NORMALIZED matches std canonicalize. Validate
    // against the ordinary canonical root plus exact scanned components; do not
    // create a second lossy pathname or merge hardlink names by file ID.
    let wide = query_name(
        file,
        0,
        scratch,
        allocator,
        origin,
        "Windows module final pathname",
    )?;
    if !wide.iter().copied().eq(expected.as_os_str().encode_wide()) {
        return Err(policy(
            "module canonical pathname changed during loading",
            origin,
        ));
    }
    Ok(())
}

#[cfg(test)]
fn validate_final_name(
    wide: &[u16],
    count: usize,
    expected: &Path,
    origin: Span,
) -> Result<(), Box<Diagnostic>> {
    validate_name_buffer(wide, count, Some(origin))?;
    if !wide[..count]
        .iter()
        .copied()
        .eq(expected.as_os_str().encode_wide())
    {
        return Err(policy(
            "module canonical pathname changed during loading",
            Some(origin),
        ));
    }
    Ok(())
}

fn local_volume_name(
    file: &File,
    root: &Path,
    scratch: &Scratch,
    allocator: &mut Allocator,
    origin: Option<Span>,
) -> Result<(), Box<Diagnostic>> {
    // Network shares have no volume GUID. A drive-shaped pathname plus the
    // server's reported NTFS name must never be mistaken for local storage.
    let wide = query_name(
        file,
        VOLUME_NAME_GUID,
        scratch,
        allocator,
        origin,
        "Windows module volume pathname",
    )?;
    validate_volume_name(&wide, root, origin)
}

fn validate_volume_name(
    wide: &[u16],
    root: &Path,
    origin: Option<Span>,
) -> Result<(), Box<Diagnostic>> {
    let reject = || {
        policy(
            "Windows module source policy requires an ordinary local NTFS volume identity",
            origin,
        )
    };
    // \\?\Volume{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}\ is 49 UTF-16 units.
    // Check the entire closed prefix before any fixed-offset indexing.
    const PREFIX: [u16; 11] = [92, 92, 63, 92, 86, 111, 108, 117, 109, 101, 123];
    if wide.len() < 49 || !wide.starts_with(&PREFIX) {
        return Err(reject());
    }
    for (position, &unit) in wide[11..47].iter().enumerate() {
        let correct = if matches!(position, 8 | 13 | 18 | 23) {
            unit == 45
        } else {
            matches!(unit, 48..=57 | 65..=70 | 97..=102)
        };
        if !correct {
            return Err(reject());
        }
    }
    if wide[47] != 125 || wide[48] != 92 {
        return Err(reject());
    }
    let skip = match root.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::VerbatimDisk(_) => 7,
            Prefix::Disk(_) => 3,
            _ => return Err(reject()),
        },
        _ => return Err(reject()),
    };
    if !wide[49..]
        .iter()
        .copied()
        .eq(root.as_os_str().encode_wide().skip(skip))
    {
        return Err(reject());
    }
    Ok(())
}

pub(in crate::frontend::project) fn verify(
    root: &Path,
    relative: &str,
    limits: ProjectLimits,
    usage: &mut SourceUsage,
    allocator: &mut Allocator,
    origin: Span,
) -> Result<(PathBuf, VerifiedChild), Box<Diagnostic>> {
    verify_at(root, relative, limits, usage, allocator, Some(origin))
}

pub(in crate::frontend::project) fn verify_entry(
    root: &Path,
    relative: &str,
    limits: ProjectLimits,
    usage: &mut SourceUsage,
    allocator: &mut Allocator,
) -> Result<(PathBuf, VerifiedChild), Box<Diagnostic>> {
    if !cfg!(all(
        target_arch = "x86_64",
        target_pointer_width = "64",
        target_endian = "little"
    )) {
        return Err(policy(
            "package source policy is not qualified on this host",
            None,
        ));
    }
    admit_invoked_entry_at(Path::new(relative), None)?;
    if relative.is_empty()
        || relative.len() > 4096
        || relative.split('/').count() > MAX_ANCESTORS
        || Path::new(relative)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)) || part.as_os_str().len() > 255)
    {
        return Err(namespace(None));
    }
    verify_at(root, relative, limits, usage, allocator, None)
}

fn verify_at(
    root: &Path,
    relative: &str,
    limits: ProjectLimits,
    usage: &mut SourceUsage,
    allocator: &mut Allocator,
    origin: Option<Span>,
) -> Result<(PathBuf, VerifiedChild), Box<Diagnostic>> {
    admit_root(root, origin)?;
    let joined = add(path_units(root), separator_units(root), origin)
        .and_then(|n| add(n, relative.len(), origin))?;
    let scratch = Scratch::new(joined, limits, *usage, origin)?;
    let mut probe = PathBuf::new();
    allocator
        .path(&mut probe, joined, "module probe path")
        .map_err(|error| super::super::reserve_error(error, origin))?;
    probe.push(root);
    let (root_file, root_identity) = open(root, true, &scratch, allocator, origin)?;
    volume(&root_file, root_identity, origin)?;
    final_name(&root_file, root, &scratch, allocator, origin)?;
    local_volume_name(&root_file, root, &scratch, allocator, origin)?;
    let mut parents: [Option<Ancestor>; MAX_ANCESTORS] = std::array::from_fn(|_| None);
    parents[0] = Some(Ancestor {
        file: root_file,
        identity: root_identity,
    });
    let mut parents_len = 1;
    let mut components = relative.split('/').peekable();
    while let Some(component) = components.next() {
        usage.probes = add(usage.probes, 1, origin)?;
        if usage.probes > limits.probes.min(8448) {
            return Err(resource("module directory probe budget exceeded", origin));
        }
        let entries = std::fs::read_dir(&probe).map_err(|e| io_error(e, "", origin))?;
        let (exact, folded) = scan_entries(
            entries.map(|entry| entry.map(|entry| entry.file_name())),
            component,
            limits,
            usage,
            origin,
        )?;
        if !exact {
            return Err(if folded {
                policy("module path case does not match its declaration", origin)
            } else {
                super::super::owned_diagnostic::diagnostic(
                    "E0002",
                    "source",
                    format_args!("declared module path component is missing"),
                    origin,
                )
            });
        }
        probe.push(component);
        let directory = components.peek().is_some();
        let (file, identity) = open(&probe, directory, &scratch, allocator, origin)?;
        if identity.volume != root_identity.volume {
            return Err(policy(
                "module path leaves its admitted NTFS volume",
                origin,
            ));
        }
        final_name(&file, &probe, &scratch, allocator, origin)?;
        if directory {
            if parents_len == parents.len() {
                return Err(resource("module depth limit exceeded", origin));
            }
            parents[parents_len] = Some(Ancestor { file, identity });
            parents_len += 1;
        } else {
            if !probe.starts_with(root) {
                return Err(policy(
                    "module canonical pathname escapes project root",
                    origin,
                ));
            }
            let child = VerifiedChild {
                file,
                identity,
                parents,
                parents_len,
            };
            child.validate_at(origin)?;
            return Ok((probe, child));
        }
    }
    Err(policy("module source path is empty", origin))
}

/// Borrow the inspected file while its ancestor/identity ownership stays live.
/// Callers retain their existing byte limits and read the same native handle.
pub(crate) fn with_windows_regular_file<T>(
    path: &Path,
    kind: &str,
    read: impl FnOnce(&mut File) -> Result<T, String>,
) -> Result<T, String> {
    let describe = |message| format!("cannot read {kind} {}: {message}", path.display());
    if path_units(path) > 4096 {
        return Err(describe("pathname limit exceeded".to_owned()));
    }
    admit_invoked_entry_at(path, None).map_err(|error| describe(error.message))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let root = parent
        .canonicalize()
        .map_err(|error| describe(error.to_string()))?;
    if root.to_str().is_none_or(|text| text.len() > 4096) {
        return Err(describe(
            "canonical pathname must be bounded UTF-8".to_owned(),
        ));
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| describe("filename must be UTF-8".to_owned()))?;
    if path_units(&root)
        .checked_add(separator_units(&root))
        .and_then(|bytes| bytes.checked_add(name.len()))
        .is_none_or(|bytes| bytes > 4096)
    {
        return Err(describe("canonical pathname limit exceeded".to_owned()));
    }
    let mut usage = SourceUsage {
        retained_path_bytes: path_units(&root),
        ..Default::default()
    };
    let (_, mut child) = verify_entry(
        &root,
        name,
        ProjectLimits::default(),
        &mut usage,
        &mut Allocator::default(),
    )
    .map_err(|error| describe(error.message))?;
    child
        .validate_at(None)
        .map_err(|error| describe(error.message))?;
    let result = read(&mut child.file)?;
    child
        .validate_at(None)
        .map_err(|error| describe(error.message))?;
    Ok(result)
}

pub(crate) fn verify_windows_workspace_member(
    root: &Path,
    member: &str,
) -> Result<PathBuf, String> {
    let length = if member == "." {
        9
    } else {
        member
            .len()
            .checked_add(10)
            .ok_or("workspace member pathname overflow")?
    };
    if length > 4096 {
        return Err("workspace member pathname limit exceeded".into());
    }
    let mut allocator = Allocator::default();
    let mut relative = String::new();
    allocator
        .string(&mut relative, length, "workspace member relative path")
        .map_err(|error| super::super::reserve_error(error, None).message)?;
    if member != "." {
        relative.push_str(member);
        relative.push('/');
    }
    relative.push_str("oxid.toml");
    let mut usage = SourceUsage {
        retained_path_bytes: length,
        ..Default::default()
    };
    let (mut path, child) = verify_entry(
        root,
        &relative,
        ProjectLimits::default(),
        &mut usage,
        &mut allocator,
    )
    .map_err(|error| error.message)?;
    drop(child);
    path.pop();
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::frontend::source::SourceFileId;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "oxid-windows-module-policy-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("child.ox"), b"retained\0bytes\xff").unwrap();
            Self(path.canonicalize().unwrap())
        }
        fn moved(&self) {
            let moved = self.0.with_extension("moved");
            std::fs::rename(&self.0, &moved).unwrap();
            std::fs::rename(moved, &self.0).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn origin() -> Span {
        Span {
            file: SourceFileId(0),
            start: 0,
            end: 1,
        }
    }

    #[test]
    fn package_entry_native_failures_keep_no_invented_span_and_release_every_handle() {
        let fixture = Fixture::new();
        let mut allocator = Allocator::default();
        let (_, child) = verify_entry(
            &fixture.0,
            "child.ox",
            ProjectLimits::default(),
            &mut SourceUsage::default(),
            &mut allocator,
        )
        .unwrap();
        let attempts = allocator.attempts;
        assert_eq!(attempts, 6);
        child.validate_at(None).unwrap();
        drop(child);
        fixture.moved();
        for attempt in 1..=attempts {
            let mut allocator = Allocator {
                fail_at: Some(attempt),
                ..Default::default()
            };
            let error = verify_entry(
                &fixture.0,
                "child.ox",
                ProjectLimits::default(),
                &mut SourceUsage::default(),
                &mut allocator,
            )
            .unwrap_err();
            assert_eq!((error.code, error.stage), ("E0400", "source-project"));
            assert_eq!(error.primary, None);
            assert!(error.secondary.is_empty());
            assert_eq!(allocator.attempts, attempt);
            fixture.moved();
        }
        for relative in [
            "../child.ox",
            r"..\child.ox",
            "/child.ox",
            "child.ox:stream",
            "child.ox.",
        ] {
            let mut usage = SourceUsage::default();
            let mut allocator = Allocator::default();
            let error = verify_entry(
                &fixture.0,
                relative,
                ProjectLimits::default(),
                &mut usage,
                &mut allocator,
            )
            .unwrap_err();
            assert_eq!((error.code, error.primary), ("E0005", None));
            assert_eq!(usage, SourceUsage::default());
            assert_eq!(allocator.attempts, 0);
        }
    }

    #[test]
    fn package_entry_identity_validation_checks_actual_replacement_handles() {
        let fixture = Fixture::new();
        std::fs::write(fixture.0.join("other.ox"), b"other").unwrap();
        let (path, mut child) = verify_entry(
            &fixture.0,
            "child.ox",
            ProjectLimits::default(),
            &mut SourceUsage::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        assert!(std::fs::OpenOptions::new().write(true).open(&path).is_err());
        assert!(std::fs::rename(&path, fixture.0.join("moved.ox")).is_err());
        child.file = File::open(fixture.0.join("other.ox")).unwrap();
        let error = child.validate_at(None).unwrap_err();
        assert_eq!((error.code, error.primary), ("E0005", None));
        assert_eq!(error.message, "module source changed during loading");
        drop(child);
        fixture.moved();
    }

    #[test]
    fn metadata_reader_borrows_the_inspected_handle_and_releases_it_on_callback_failure() {
        use std::io::Read;
        let fixture = Fixture::new();
        let path = fixture.0.join("child.ox");
        let bytes = with_windows_regular_file(&path, "manifest", |file| {
            assert!(std::fs::OpenOptions::new().write(true).open(&path).is_err());
            assert!(std::fs::rename(&path, fixture.0.join("moved.ox")).is_err());
            let mut bytes = Vec::new();
            file.take(9)
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string())?;
            Ok(bytes)
        })
        .unwrap();
        assert_eq!(bytes, b"retained\0");
        fixture.moved();
        let failure = with_windows_regular_file::<()>(&path, "lock", |_| Err("read failed".into()));
        assert_eq!(failure.unwrap_err(), "read failed");
        fixture.moved();
    }

    #[test]
    fn wide_buffer_reservations_are_bounded_and_each_allocation_failure_releases_handles() {
        let fixture = Fixture::new();
        let mut usage = SourceUsage::default();
        let mut allocator = Allocator::default();
        let (path, child) = verify(
            &fixture.0,
            "child.ox",
            ProjectLimits::default(),
            &mut usage,
            &mut allocator,
            origin(),
        )
        .unwrap();
        assert_eq!(path, fixture.0.join("child.ox"));
        assert_eq!(usage.probes, 1);
        assert_eq!(usage.directory_entries, 1);
        let wide: Vec<_> = allocator
            .trace
            .iter()
            .filter(|event| event.kind.starts_with("Windows module "))
            .collect();
        assert_eq!(wide.len(), 5);
        for (event, expected) in wide.into_iter().zip([
            fixture.0.as_os_str().encode_wide().count() + 1,
            fixture.0.as_os_str().encode_wide().count() + 1,
            49 + fixture.0.as_os_str().encode_wide().count() - 7 + 1,
            path.as_os_str().encode_wide().count() + 1,
            path.as_os_str().encode_wide().count() + 1,
        ]) {
            assert_eq!(event.length, expected);
            assert_eq!(event.element_bytes, 2);
            assert!(event.success);
        }
        let attempts = allocator.attempts;
        assert_eq!(attempts, 6);
        drop(child);
        fixture.moved();
        for attempt in 1..=attempts {
            let mut allocator = Allocator {
                fail_at: Some(attempt),
                ..Default::default()
            };
            let error = verify(
                &fixture.0,
                "child.ox",
                ProjectLimits::default(),
                &mut SourceUsage::default(),
                &mut allocator,
                origin(),
            )
            .unwrap_err();
            assert_eq!((error.code, error.stage), ("E0400", "source-project"));
            assert_eq!(error.message, "project source allocation failed");
            assert_eq!(allocator.attempts, attempt);
            fixture.moved();
        }
    }

    #[test]
    fn inspected_child_handle_prevents_write_and_rename_and_keeps_exact_bytes() {
        use std::io::Read;
        let fixture = Fixture::new();
        let (path, mut child) = verify(
            &fixture.0,
            "child.ox",
            ProjectLimits::default(),
            &mut SourceUsage::default(),
            &mut Allocator::default(),
            origin(),
        )
        .unwrap();
        assert!(std::fs::OpenOptions::new().write(true).open(&path).is_err());
        assert!(std::fs::rename(&path, fixture.0.join("moved.ox")).is_err());
        child.validate(origin()).unwrap();
        let mut captured = Vec::new();
        child.file.read_to_end(&mut captured).unwrap();
        child.validate(origin()).unwrap();
        assert_eq!(captured, b"retained\0bytes\xff");
        drop(child);
        std::fs::write(&path, b"later bytes").unwrap();
        std::fs::rename(&path, fixture.0.join("moved.ox")).unwrap();
    }

    #[test]
    fn native_scratch_arithmetic_and_caps_precede_allocation_and_file_effects() {
        let mut allocator = Allocator::default();
        let scratch = Scratch::new(
            0,
            ProjectLimits {
                path_bytes: fixed_scratch_bytes().unwrap() + 2,
                ..Default::default()
            },
            SourceUsage::default(),
            Some(origin()),
        )
        .unwrap();
        assert_eq!(scratch.available, 2);
        for units in [2, MAX_WIDE_PATH + 1, usize::MAX] {
            let error = scratch
                .wide(units, &mut allocator, Some(origin()), "test native path")
                .unwrap_err();
            assert_eq!((error.code, error.stage), ("E0400", "source-project"));
        }
        assert_eq!(allocator.attempts, 0);
        let mut usage = SourceUsage::default();
        let error = verify(
            Path::new(r"\\?\C:\missing-root-must-not-be-opened"),
            "child.ox",
            ProjectLimits {
                path_bytes: 0,
                ..Default::default()
            },
            &mut usage,
            &mut allocator,
            origin(),
        )
        .unwrap_err();
        assert_eq!((error.code, error.stage), ("E0400", "source-project"));
        assert_eq!(allocator.attempts, 0);
        assert_eq!(usage.probes, 0);
        let wide = scratch
            .wide(1, &mut allocator, Some(origin()), "test native path")
            .unwrap();
        assert_eq!(wide.len(), 0);
        assert_eq!(allocator.trace[0].length, 1);
        assert_eq!(allocator.trace[0].element_bytes, 2);
        assert_eq!(
            fixed_scratch_bytes().unwrap(),
            size_of::<VerifiedChild>()
                + size_of::<Scratch>()
                + 2 * size_of::<FileInformation>()
                + size_of::<[u16; 32]>()
                + 3 * size_of::<u32>()
        );
        assert_eq!(
            size_of::<[Option<Ancestor>; MAX_ANCESTORS]>(),
            MAX_ANCESTORS * size_of::<Option<Ancestor>>()
        );
    }

    #[test]
    fn complete_native_directory_scans_keep_the_common_cap_error() {
        let fixture = Fixture::new();
        std::fs::write(fixture.0.join("unrelated.ox"), "must not be read").unwrap();
        for limits in [
            ProjectLimits {
                directory_entries: 1,
                ..Default::default()
            },
            ProjectLimits {
                directory_name_units: 0,
                ..Default::default()
            },
        ] {
            let mut usage = SourceUsage::default();
            let mut allocator = Allocator::default();
            let error = verify(
                &fixture.0,
                "child.ox",
                limits,
                &mut usage,
                &mut allocator,
                origin(),
            )
            .unwrap_err();
            assert_eq!((error.code, error.stage), ("E0400", "source-project"));
            assert_eq!(error.message, "module directory scan budget exceeded");
            assert_eq!(allocator.attempts, 4); // probe, root open, DOS/GUID names; no child open.
            fixture.moved();
        }
    }

    #[test]
    fn ordinary_entry_namespaces_and_native_separator_counts_are_closed() {
        for path in [
            r"C:\ordinary\entry.ox",
            r"\\?\C:\ordinary\entry.ox",
            r"relative\entry.ox",
            r"..\entry.ox",
        ] {
            admit_invoked_entry(Path::new(path), origin()).unwrap();
        }
        for path in [
            r"C:entry.ox",
            r"\\server\share\entry.ox",
            r"\\?\UNC\server\share\entry.ox",
            r"\\.\C:\entry.ox",
            r"C:\entry.ox:stream",
            r"C:\ordinary.\entry.ox",
            "C:\\a\0.ox",
        ] {
            let error = admit_invoked_entry(Path::new(path), origin()).unwrap_err();
            assert_eq!((error.code, error.stage), ("E0005", "source"));
        }
        for (path, units) in [
            (r"C:\", 0),
            (r"\\?\C:\", 0),
            (r"C:\ordinary", 1),
            ("C:/ordinary/", 0),
        ] {
            assert_eq!(separator_units(Path::new(path)), units, "{path}");
        }
    }

    #[test]
    fn native_query_lengths_nuls_and_unicode_are_checked_before_indexing() {
        let path = Path::new(r"\\?\C:\ordinary\child.ox");
        let mut wide: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let count = wide.len() - 1;
        validate_final_name(&wide, count, path, origin()).unwrap();
        for length in [wide.len(), usize::MAX] {
            assert!(validate_final_name(&wide, length, path, origin()).is_err());
        }
        wide[count] = 1;
        assert!(validate_final_name(&wide, count, path, origin()).is_err());
        wide[count] = 0;
        wide[7] = 0;
        assert!(validate_final_name(&wide, count, path, origin()).is_err());
        wide[7] = 0xd800;
        let error = validate_final_name(&wide, count, path, origin()).unwrap_err();
        assert!(error.message.contains("not valid Unicode"));
    }

    #[test]
    fn local_volume_query_accepts_only_a_closed_guid_root_with_the_same_opened_suffix() {
        let root = Path::new(r"\\?\C:\ordinary\child");
        let text = r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\ordinary\child";
        let good: Vec<_> = text.encode_utf16().collect();
        validate_volume_name(&good, root, Some(origin())).unwrap();
        for text in [
            r"\\?\UNC\server\share\ordinary\child",
            r"\\?\C:\ordinary\child",
            r"\\?\Volume{0123456789ab-cdef-0123-456789abcdef}\ordinary\child",
            r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdeg}\ordinary\child",
            r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\elsewhere\child",
        ] {
            let wide: Vec<_> = text.encode_utf16().collect();
            assert_eq!(
                validate_volume_name(&wide, root, Some(origin()))
                    .unwrap_err()
                    .code,
                "E0005"
            );
        }
        for count in 0..49 {
            assert!(validate_volume_name(&good[..count], root, Some(origin())).is_err());
        }
        for code in [1, 3, 50, 87] {
            let error = pathname_error(
                io::Error::from_raw_os_error(code),
                VOLUME_NAME_GUID,
                Some(origin()),
            );
            assert_eq!((error.code, error.stage), ("E0005", "source"));
        }
        for (code, flags) in [(5, VOLUME_NAME_GUID), (32, VOLUME_NAME_GUID), (3, 0)] {
            let error = pathname_error(io::Error::from_raw_os_error(code), flags, Some(origin()));
            assert_eq!((error.code, error.stage), ("E0002", "source"));
        }
    }

    #[test]
    fn volume_admission_requires_the_actual_ntfs_identity_and_component_contract() {
        let ntfs = [78, 84, 70, 83];
        assert!(qualified_volume(&ntfs, 7, 7, 255));
        for (name, serial, expected, maximum) in [
            (&[70, 65, 84, 51, 50][..], 7, 7, 255),
            (&[82, 101, 70, 83][..], 7, 7, 255),
            (&ntfs[..], 8, 7, 255),
            (&ntfs[..], 7, 7, 256),
            (&ntfs[..], 7, 7, 0),
        ] {
            assert!(!qualified_volume(name, serial, expected, maximum));
        }
    }
}
