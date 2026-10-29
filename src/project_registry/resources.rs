//! Logical request storage, separate from compiler scratch and VM heap limits.
//! Payload/inventory bytes and retained dynamic slots are charged separately.
use super::*;
use std::mem::size_of;

/// Actual monomorphized callbacks and complete by-value return transports.
/// These rows are paid before invoking either opaque helper, then released
/// when both callbacks/temporary results have finished. No guessed capture size.
#[repr(C)]
struct ScopeRoles<'a, T, A, R> {
    receiver: &'a mut Budget,
    action: A,
    retained: R,
    returned: Result<T, String>,
    value: T,
    retained_input: &'a T,
    capacity_return: Result<usize, String>,
    scope_return: Result<T, String>,
    bound: usize,
    actual: usize,
    charge: Option<usize>,
}
pub(super) fn scope_control_bytes<T, A, R>() -> usize {
    size_of::<ScopeRoles<'_, T, A, R>>()
}

// Rc/Arc allocations contain both complete atomic/non-atomic reference counts,
// followed by the properly aligned value. Handle slots are charged separately.
#[repr(C)]
struct OriginAllocation {
    strong: std::sync::atomic::AtomicUsize,
    weak: std::sync::atomic::AtomicUsize,
    value: Origin,
}
#[repr(C)]
struct DeclarationAllocation {
    strong: std::cell::Cell<usize>,
    weak: std::cell::Cell<usize>,
    value: Declarations,
}
pub(super) fn declaration_allocation_bytes() -> usize {
    size_of::<DeclarationAllocation>()
}

#[repr(C)]
struct BudgetAllocation {
    strong: std::cell::Cell<usize>,
    weak: std::cell::Cell<usize>,
    value: RefCell<Budget>,
}
pub(super) fn origin_allocation_bytes() -> usize {
    size_of::<OriginAllocation>()
}

#[repr(C)]
struct ResolutionRoles {
    session: Session,
    dependency: Dependency,
    selected_source: String,
    selected_id: String,
    release_path: PathBuf,
    cache_path: PathBuf,
    parent_path: PathBuf,
    record_path: PathBuf,
    artifact_path: PathBuf,
    record_bytes: Vec<u8>,
    record: Record,
    decoded: Result<(package::LibraryArchive, package::LibraryInfo), String>,
    archive: package::LibraryArchive,
    info: package::LibraryInfo,
    staged_info: Result<package::LibraryInfo, String>,
    stage: package::PrivateDirectory,
    origin: Origin,
    origin_return: Option<Arc<Origin>>,
    resolution_return: Result<PathBuf, String>,
    file: fs::File,
    metadata: fs::Metadata,
    sha256: Sha256,
    digest: [u8; 32],
}

#[repr(C)]
struct TraversalRoles {
    pending: Vec<PathBuf>,
    directory: PathBuf,
    entry_path: PathBuf,
    entries: fs::ReadDir,
    entry_return: Option<io::Result<fs::DirEntry>>,
    entry: fs::DirEntry,
    entry_name: std::ffi::OsString,
    kind_return: io::Result<fs::FileType>,
    kind: fs::FileType,
    raw_return: Result<Vec<u8>, String>,
    raw: Vec<u8>,
    expected: Option<&'static package::LibraryMember>,
    traversal_return: Result<(), String>,
    counts: [usize; 4],
    compare_buffer: [u8; 8192],
    end_byte: [u8; 1],
}

#[repr(C)]
struct ParserTransportRoles {
    path: &'static Path,
    input: &'static [u8],
    context: Option<Allowance>,
    context_borrow: &'static mut Option<Allowance>,
    manifest: runtime::ManifestSnapshot,
    manifest_return: Result<runtime::ManifestSnapshot, String>,
    declaration_return: Result<Rc<Declarations>, String>,
    replacement_map: std::collections::HashMap<String, String>,
    replacement_return: Result<std::collections::HashMap<String, String>, String>,
    exported_updates: std::collections::HashMap<PathBuf, String>,
    exported_root: PathBuf,
    cloned_manifest_return: Result<runtime::ManifestSnapshot, String>,
    output: Vec<u8>,
    output_return: Result<Vec<u8>, String>,
    permit_return: Result<usize, String>,
    measured: [usize; 6],
}

pub(super) fn control_bytes() -> usize {
    size_of::<ResolutionRoles>()
        + size_of::<TraversalRoles>()
        + size_of::<BudgetAllocation>()
        + size_of::<ParserTransportRoles>()
        + crate::project_cli::registry_control_bytes()
        + crate::project_lock::registry_control_bytes()
        + package::library_control_bytes()
}

fn add(total: &mut usize, value: usize) -> Result<(), String> {
    *total = total
        .checked_add(value)
        .ok_or("registry parser storage overflow")?;
    Ok(())
}
pub(super) fn table<T>(entries: usize) -> Result<usize, String> {
    if entries == 0 {
        return Ok(0);
    }
    let buckets = entries
        .checked_mul(2)
        .and_then(|count| count.max(4).checked_next_power_of_two())
        .ok_or("registry parser table overflow")?;
    buckets
        .checked_mul(size_of::<T>() + 1)
        .and_then(|bytes| bytes.checked_add(16 + std::mem::align_of::<T>()))
        .ok_or("registry parser table overflow".into())
}
pub(super) fn table_capacity<T>(capacity: usize) -> Result<usize, String> {
    if capacity == 0 {
        return Ok(0);
    }
    let buckets = capacity
        .checked_add(1)
        .and_then(usize::checked_next_power_of_two)
        .ok_or("registry parser capacity overflow")?;
    buckets
        .checked_mul(size_of::<T>() + 1)
        .and_then(|bytes| bytes.checked_add(16 + std::mem::align_of::<T>()))
        .ok_or("registry parser capacity overflow".into())
}

/// Conservative bound for the existing manifest parser's named roles. It does
/// not change that parser's grammar or any ordinary-command limit. Noncomment
/// bytes bound decoded key/value text; header dots bound section components;
/// all possible maps and both workspace arrays are covered before parsing.
pub(super) fn manifest_bound(raw: &[u8]) -> Result<usize, String> {
    if raw.len() > 1024 * 1024 {
        return Err("registry manifest exceeds 1 MiB".into());
    }
    let text = std::str::from_utf8(raw).map_err(|_| "registry manifest is not UTF-8")?;
    let mut syntax = 0usize;
    let mut headers = 0usize;
    let mut components = 0usize;
    let mut fields = 0usize;
    let mut inline = 0usize;
    let mut inline_bytes = 0usize;
    for line in text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        add(&mut syntax, line.len())?;
        if line.starts_with('[') {
            add(&mut headers, line.len())?;
            components += line.bytes().filter(|byte| *byte == b'.').count() + 1;
        }
        if let Some((_, value)) = line.split_once('=') {
            fields += 1;
            if value.trim_start().starts_with('{') {
                inline += 1;
                add(&mut inline_bytes, line.len())?;
            }
        }
    }
    let mut bound = size_of::<runtime::ManifestSnapshot>()
        + size_of::<Result<runtime::ManifestSnapshot, String>>()
        + size_of::<Vec<String>>() * 3
        + size_of::<Option<(String, String)>>()
        + 256;
    for amount in [
        // Growing parsed keys/values; pending workspace value plus decoded arrays.
        syntax
            .checked_mul(6)
            .ok_or("registry manifest text overflow")?
            + fields * 32,
        headers * 4 + components * (2 * size_of::<String>() + 16),
        // Descriptor field copies, URI decode temporary, and owned alias keys.
        inline_bytes * 3 + inline * size_of::<(String, Dependency)>(),
        size_of::<Declarations>() + 2 * size_of::<usize>(),
        table::<(String, String)>(fields)? * 3,
        table::<(String, bool)>(fields)?,
        2 * 64 * size_of::<String>(),
    ] {
        add(&mut bound, amount)?;
    }
    Ok(bound)
}

pub(super) fn manifest_capacity(snapshot: &runtime::ManifestSnapshot) -> Result<usize, String> {
    manifest_capacity_kind(snapshot, true)
}
pub(super) fn manifest_clone_capacity(
    snapshot: &runtime::ManifestSnapshot,
) -> Result<usize, String> {
    manifest_capacity_kind(snapshot, false)
}
fn manifest_capacity_kind(
    snapshot: &runtime::ManifestSnapshot,
    declarations: bool,
) -> Result<usize, String> {
    let manifest = &snapshot.manifest;
    let mut bytes = size_of::<runtime::ManifestSnapshot>() + snapshot.digest.capacity();
    for value in [
        &manifest.name,
        &manifest.version,
        &manifest.entry,
        &manifest.kind,
        &manifest.edition,
        &manifest.backend,
        &manifest.target,
        &manifest.profile,
        &manifest.entry_mode,
        &manifest.execution_policy,
        &manifest.configuration_error,
    ]
    .into_iter()
    .flatten()
    {
        add(&mut bytes, value.capacity())?;
    }
    for map in [&manifest.scripts, &manifest.dependencies, &manifest.tests] {
        add(&mut bytes, string_map_capacity(map)?)?;
    }
    add(
        &mut bytes,
        table_capacity::<(String, bool)>(manifest.features.capacity())?,
    )?;
    for key in manifest.features.keys() {
        add(&mut bytes, key.capacity())?;
    }
    if let Some(entries) = snapshot
        .manifest
        .registry_dependencies
        .as_ref()
        .filter(|_| declarations)
    {
        add(
            &mut bytes,
            2 * size_of::<usize>()
                + size_of::<Declarations>()
                + entries.capacity() * size_of::<(String, Dependency)>(),
        )?;
        for (alias, dependency) in entries.iter() {
            add(
                &mut bytes,
                alias.capacity()
                    + dependency.source.capacity()
                    + dependency.name.capacity()
                    + dependency.version.capacity(),
            )?;
        }
    }
    if let Some(workspace) = &snapshot.workspace {
        if let Some(edition) = &workspace.edition {
            add(&mut bytes, edition.capacity())?;
        }
        for array in [&workspace.members, &workspace.default_members]
            .into_iter()
            .flatten()
        {
            add(&mut bytes, array.capacity() * size_of::<String>())?;
            for member in array {
                add(&mut bytes, member.capacity())?;
            }
        }
    }
    Ok(bytes)
}

pub(super) fn string_map_capacity(
    map: &std::collections::HashMap<String, String>,
) -> Result<usize, String> {
    let mut bytes = table_capacity::<(String, String)>(map.capacity())?;
    for (key, value) in map {
        add(&mut bytes, key.capacity() + value.capacity())?;
    }
    Ok(bytes)
}

pub(super) fn replacement_bound(entries: &[(String, Dependency)]) -> Result<usize, String> {
    let mut bytes = table::<(String, String)>(entries.len())?
        + size_of::<std::collections::HashMap<String, String>>()
        + 256;
    for (alias, dependency) in entries {
        add(
            &mut bytes,
            alias.len() * 2
                + dependency.source.len() * 2
                + 4096
                + dependency.name.len() * 2
                + dependency.version.len() * 2
                + 256,
        )?;
    }
    Ok(bytes)
}

pub(super) fn rewrite_bound(
    raw: &[u8],
    replacements: &std::collections::HashMap<String, String>,
) -> Result<usize, String> {
    let replacement_bytes = replacements
        .values()
        .try_fold(0usize, |sum, value| {
            value
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(2))
                .and_then(|n| sum.checked_add(n))
        })
        .ok_or("registry rewrite size overflow")?;
    let maximum = raw
        .len()
        .checked_add(replacement_bytes)
        .ok_or("registry rewrite size overflow")?;
    if maximum > 1024 * 1024 {
        return Err("registry rewritten manifest admission exceeds 1 MiB".into());
    }
    let fields = raw.iter().filter(|byte| **byte == b'=').count();
    let mut bytes = manifest_bound(raw)?
        .checked_mul(2)
        .ok_or("registry rewrite admission overflow")?;
    // Original parse, output/reparse, dependency map clone and span/key roster.
    for amount in [
        maximum * 4,
        // Replaced values may be longer than their original spans. The second
        // parser's six text copies and three descriptor copies are admitted
        // from those additional bytes too; field/header counts do not grow.
        replacement_bytes * 9 + replacements.len() * std::mem::size_of::<(String, Dependency)>(),
        table::<(String, String)>(fields)?,
        fields * (2 * size_of::<(String, std::ops::Range<usize>)>() + 32),
        raw.len() * 2,
        string_map_capacity(replacements)?,
    ] {
        add(&mut bytes, amount)?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::align_of;
    #[test]
    fn registry_control_roles_have_independent_member_sum_and_alignment() {
        macro_rules! layout {
            ($ty:ty; $($field:ident : $field_ty:ty),+ $(,)?) => {{
                let mut bytes = 0usize;
                let mut alignment = 1usize;
                $(let align = align_of::<$field_ty>(); alignment = alignment.max(align);
                  bytes = bytes.div_ceil(align) * align;
                  assert_eq!(std::mem::offset_of!($ty, $field), bytes);
                  bytes += size_of::<$field_ty>();)+
                assert_eq!(align_of::<$ty>(), alignment);
                assert_eq!(size_of::<$ty>(), bytes.div_ceil(alignment) * alignment);
            }};
        }
        layout!(TraversalRoles;
            pending: Vec<PathBuf>, directory: PathBuf, entry_path: PathBuf,
            entries: fs::ReadDir, entry_return: Option<io::Result<fs::DirEntry>>, entry: fs::DirEntry, entry_name: std::ffi::OsString,
            kind_return: io::Result<fs::FileType>, kind: fs::FileType,
            raw_return: Result<Vec<u8>, String>, raw: Vec<u8>, expected: Option<&'static package::LibraryMember>,
            traversal_return: Result<(), String>, counts: [usize;4], compare_buffer: [u8;8192], end_byte: [u8;1]);
        layout!(OriginAllocation; strong: std::sync::atomic::AtomicUsize, weak: std::sync::atomic::AtomicUsize, value: Origin);
        layout!(ResolutionRoles; session: Session, dependency: Dependency, selected_source: String, selected_id: String,
            release_path: PathBuf, cache_path: PathBuf, parent_path: PathBuf, record_path: PathBuf, artifact_path: PathBuf,
            record_bytes: Vec<u8>, record: Record, decoded: Result<(package::LibraryArchive,package::LibraryInfo),String>,
            archive: package::LibraryArchive, info: package::LibraryInfo, staged_info: Result<package::LibraryInfo,String>,
            stage: package::PrivateDirectory, origin: Origin, origin_return: Option<Arc<Origin>>,
            resolution_return: Result<PathBuf,String>, file: fs::File, metadata: fs::Metadata, sha256: Sha256, digest: [u8;32]);
        layout!(BudgetAllocation; strong: std::cell::Cell<usize>, weak: std::cell::Cell<usize>, value: RefCell<Budget>);
        layout!(ScopeRoles<'_,u32,[u8;7],(u64,bool)>; receiver: &mut Budget, action: [u8;7], retained: (u64,bool), returned: Result<u32,String>,
            value: u32, retained_input: &u32, capacity_return: Result<usize,String>, scope_return: Result<u32,String>, bound: usize, actual: usize, charge: Option<usize>);
        layout!(DeclarationAllocation; strong: std::cell::Cell<usize>, weak: std::cell::Cell<usize>, value: Declarations);
        layout!(ParserTransportRoles; path: &'static Path, input: &'static [u8], context: Option<Allowance>, context_borrow: &'static mut Option<Allowance>, manifest: runtime::ManifestSnapshot, manifest_return: Result<runtime::ManifestSnapshot,String>, declaration_return: Result<Rc<Declarations>,String>, replacement_map: std::collections::HashMap<String,String>, replacement_return: Result<std::collections::HashMap<String,String>,String>, exported_updates: std::collections::HashMap<PathBuf,String>, exported_root: PathBuf, cloned_manifest_return: Result<runtime::ManifestSnapshot,String>, output: Vec<u8>, output_return: Result<Vec<u8>,String>, permit_return: Result<usize,String>, measured: [usize;6]);
        assert_eq!(
            control_bytes(),
            size_of::<ResolutionRoles>()
                + size_of::<TraversalRoles>()
                + size_of::<BudgetAllocation>()
                + size_of::<ParserTransportRoles>()
                + crate::project_cli::registry_control_bytes()
                + crate::project_lock::registry_control_bytes()
                + package::library_control_bytes()
        );
        let mut budget = Budget::new().unwrap();
        let remaining = MAX_METADATA - budget.metadata;
        budget.metadata(remaining).unwrap();
        assert!(budget.metadata(1).is_err());
    }
}
