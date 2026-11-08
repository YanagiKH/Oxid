//! Typed local locks are deliberately separate from the legacy v1 resolver.
//! All identities are relative to the canonical application directory, so an
//! entire checkout (including sibling packages) can move without changing a lock.
use crate::{frontend, runtime};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, Write},
    path::{Component, Path},
    sync::atomic::{AtomicUsize, Ordering},
};

const MAX_LOCK_BYTES: usize = 2 * 1024 * 1024;
static NEXT_STAGE: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
struct Package {
    id: String,
    path: String,
    name: String,
    version: String,
    edition: String,
    entry: String,
    manifest: String,
    sources: String,
    registry: Option<Box<crate::project_registry::Pin>>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Edge {
    owner: String,
    alias: String,
    package: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Snapshot {
    registry_format: bool,
    root: String,
    workspace: Option<String>,
    members: Vec<String>,
    packages: Vec<Package>,
    dependencies: Vec<Edge>,
}

pub(crate) struct Existing {
    snapshot: Snapshot,
    bytes: Vec<u8>,
}

impl Existing {
    /// Exact bytes admitted and verified with this lock; never reopen its path
    /// to construct a source package after checking a different read.
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

fn framed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// ID = SHA256(lp(domain) || lp(relative canonical directory)), with
/// lp(bytes) = u64 little-endian byte length followed by those exact bytes.
fn package_id(path: &str) -> String {
    let mut hasher = Sha256::new();
    framed(&mut hasher, b"oxid-typed-local-package-v1");
    framed(&mut hasher, path.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) fn relative(base: &Path, target: &Path) -> Result<String, String> {
    let base: Vec<_> = base.components().collect();
    let target: Vec<_> = target.components().collect();
    let shared = base.iter().zip(&target).take_while(|(a, b)| a == b).count();
    if shared == 0
        || target[shared..]
            .iter()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("typed lock package paths must share one filesystem root".into());
    }
    let mut parts = vec!["..".to_owned(); base.len() - shared];
    for part in &target[shared..] {
        parts.push(
            part.as_os_str()
                .to_str()
                .ok_or("typed lock paths must be UTF-8")?
                .to_owned(),
        );
    }
    Ok(if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    })
}

// These bounds apply only when the new registry operation calls an unchanged
// parser/validator. They are logical reservation envelopes, not allocator RSS.
fn add_bytes(total: &mut usize, bytes: usize) -> Result<(), String> {
    *total = total
        .checked_add(bytes)
        .ok_or("registry lock storage overflow")?;
    Ok(())
}
fn grow_slots(count: usize) -> Result<usize, String> {
    if count == 0 {
        Ok(0)
    } else {
        count
            .max(4)
            .checked_next_power_of_two()
            .ok_or("registry lock capacity overflow".into())
    }
}
fn table_bytes<T>(count: usize) -> Result<usize, String> {
    if count == 0 {
        return Ok(0);
    }
    // A <= 7/8-full table uses at most the next power of two of 2*n
    // buckets; include the complete key/value row, control bytes and alignment.
    let buckets = count
        .checked_mul(2)
        .and_then(|n| n.max(4).checked_next_power_of_two())
        .ok_or("registry table capacity overflow")?;
    buckets
        .checked_mul(std::mem::size_of::<T>() + 1)
        .and_then(|n| n.checked_add(16 + std::mem::align_of::<T>()))
        .ok_or("registry table storage overflow".into())
}
pub(crate) fn relative_registry(
    base: &Path,
    target: &Path,
    budget: &mut crate::project_registry::Budget,
) -> Result<String, String> {
    let shared = base
        .components()
        .zip(target.components())
        .take_while(|(a, b)| a == b)
        .count();
    if shared == 0 {
        return Err("registry lock paths must share one filesystem root".into());
    }
    let parents = base.components().count() - shared;
    let mut length = parents
        .checked_mul(2)
        .ok_or("registry relative path overflow")?;
    let mut parts = parents;
    for part in target.components().skip(shared) {
        let Component::Normal(value) = part else {
            return Err("registry paths must be normalized".into());
        };
        add_bytes(
            &mut length,
            value.to_str().ok_or("registry path is not UTF-8")?.len(),
        )?;
        parts += 1;
    }
    if parts == 0 {
        return budget.text(".");
    }
    add_bytes(&mut length, parts - 1)?;
    let mut output = budget.string(length)?;
    for _ in 0..parents {
        if !output.is_empty() {
            output.push('/');
        }
        output.push_str("..");
    }
    for part in target.components().skip(shared) {
        if !output.is_empty() {
            output.push('/');
        }
        output.push_str(
            part.as_os_str()
                .to_str()
                .ok_or("registry path is not UTF-8")?,
        );
    }
    Ok(output)
}
fn registry_pin_validation_bound(pin: &crate::project_registry::Pin) -> Result<usize, String> {
    // Decoded URI buffer, canonical URI buffer and the 64-byte framed ID.
    pin.source
        .len()
        .checked_add(4096 + 1 + 64 + 256)
        .ok_or("registry pin validation overflow".into())
}
fn validation_shape(
    packages: usize,
    edges: usize,
    members: usize,
    text: usize,
) -> Result<usize, String> {
    let mut bytes = std::mem::size_of::<Snapshot>() + std::mem::size_of::<Result<(), String>>();
    for amount in [
        table_bytes::<&String>(edges)?,
        table_bytes::<(&String, &Package)>(packages)?,
        table_bytes::<(&String, String)>(edges)?,
        table_bytes::<&str>(packages)?,
        table_bytes::<(&str, usize)>(packages)?,
        // One complete visitor frame per admitted package in the worst path.
        packages
            * (std::mem::size_of::<&str>()
                + std::mem::size_of::<&[Edge]>()
                + 2 * std::mem::size_of::<usize>()
                + std::mem::size_of::<Option<usize>>()
                + std::mem::size_of::<Result<usize, String>>()
                + 2 * std::mem::size_of::<&mut HashSet<&str>>()),
        members * std::mem::size_of::<&String>(),
        text.checked_mul(3)
            .ok_or("registry validation text overflow")?,
        packages * (4096 + 64 + 256),
    ] {
        add_bytes(&mut bytes, amount)?;
    }
    Ok(bytes)
}
fn registry_validation_bound(snapshot: &Snapshot) -> Result<usize, String> {
    let mut text = snapshot
        .dependencies
        .iter()
        .map(|edge| edge.alias.len())
        .sum::<usize>();
    for package in &snapshot.packages {
        if let Some(pin) = &package.registry {
            add_bytes(&mut text, pin.source.len())?;
        }
    }
    validation_shape(
        snapshot.packages.len(),
        snapshot.dependencies.len(),
        snapshot.members.len(),
        text,
    )
}
fn registry_encoding_bound(snapshot: &Snapshot) -> Result<usize, String> {
    let quote_length = |value: &str| {
        value.len()
            + value
                .bytes()
                .filter(|byte| matches!(byte, b'"' | b'\\'))
                .count()
            + 2
    };
    let mut bytes = 128usize;
    let mut longest = 128usize;
    let mut field = |key: &str, value: &str| -> Result<(), String> {
        let length = key.len() + 4 + quote_length(value);
        add_bytes(&mut bytes, length)?;
        longest = longest.max(length);
        Ok(())
    };
    if let Some(manifest) = &snapshot.workspace {
        field("manifest", manifest)?;
    } else {
        field("root", &snapshot.root)?;
    }
    for member in &snapshot.members {
        field("package", member)?;
    }
    for package in &snapshot.packages {
        for (key, value) in [
            ("id", &package.id),
            ("path", &package.path),
            ("name", &package.name),
            ("version", &package.version),
            ("edition", &package.edition),
            ("entry", &package.entry),
            ("manifest", &package.manifest),
            ("sources", &package.sources),
        ] {
            field(key, value)?;
        }
        if snapshot.registry_format {
            let (source, artifact) = package
                .registry
                .as_ref()
                .map_or(("", ""), |pin| (pin.source.as_str(), pin.artifact.as_str()));
            field("registry", source)?;
            field("artifact", artifact)?;
            field("artifact-bytes", "18446744073709551615")?;
        }
    }
    for edge in &snapshot.dependencies {
        field("owner", &edge.owner)?;
        field("alias", &edge.alias)?;
        field("package", &edge.package)?;
    }
    add_bytes(
        &mut bytes,
        (snapshot.packages.len() + snapshot.dependencies.len() + snapshot.members.len()) * 20,
    )?;
    // Geometric output growth plus one complete quoted/formatted field and
    // string/Result transports. The returned encoded text is immediately dropped.
    bytes
        .checked_mul(2)
        .and_then(|n| n.checked_add(longest * 4 + 256))
        .ok_or("registry lock encoding overflow".into())
}
fn registry_decode_bound(bytes: &[u8]) -> Result<usize, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "malformed typed lock: file is not UTF-8")?;
    let packages = text.lines().filter(|line| *line == "[[package]]").count();
    let edges = text
        .lines()
        .filter(|line| *line == "[[dependency]]")
        .count();
    let members = text.lines().filter(|line| *line == "[[member]]").count();
    if packages > 256 || edges > 1024 || members > 32 {
        return Err("registry lock exceeds its existing graph limits".into());
    }
    let mut value_bytes = 0usize;
    let mut fields = 0usize;
    for value in text
        .lines()
        .filter_map(|line| line.split_once(" = ").map(|(_, value)| value))
    {
        add_bytes(&mut value_bytes, value.len())?;
        fields += 1;
    }
    let mut bound =
        std::mem::size_of::<Snapshot>() + std::mem::size_of::<Result<Snapshot, String>>();
    for amount in [
        grow_slots(packages)? * std::mem::size_of::<Package>(),
        grow_slots(edges)? * std::mem::size_of::<Edge>(),
        grow_slots(members)? * std::mem::size_of::<String>(),
        packages * std::mem::size_of::<crate::project_registry::Pin>(),
        // Unquoted strings grow by doubling; pins separately clone name/version.
        value_bytes
            .checked_mul(3)
            .ok_or("registry lock strings overflow")?
            + fields * 16,
        validation_shape(packages, edges, members, value_bytes)?,
    ] {
        add_bytes(&mut bound, amount)?;
    }
    Ok(bound)
}
fn registry_snapshot_capacity(snapshot: &Snapshot) -> Result<usize, String> {
    let mut bytes = std::mem::size_of::<Snapshot>()
        + snapshot.root.capacity()
        + snapshot.workspace.as_ref().map_or(0, String::capacity)
        + snapshot.packages.capacity() * std::mem::size_of::<Package>()
        + snapshot.dependencies.capacity() * std::mem::size_of::<Edge>()
        + snapshot.members.capacity() * std::mem::size_of::<String>();
    for member in &snapshot.members {
        add_bytes(&mut bytes, member.capacity())?;
    }
    for package in &snapshot.packages {
        for value in [
            &package.id,
            &package.path,
            &package.name,
            &package.version,
            &package.edition,
            &package.entry,
            &package.manifest,
            &package.sources,
        ] {
            add_bytes(&mut bytes, value.capacity())?;
        }
        if let Some(pin) = &package.registry {
            add_bytes(
                &mut bytes,
                std::mem::size_of::<crate::project_registry::Pin>()
                    + pin.source.capacity()
                    + pin.name.capacity()
                    + pin.version.capacity()
                    + pin.artifact.capacity(),
            )?;
        }
    }
    for edge in &snapshot.dependencies {
        add_bytes(
            &mut bytes,
            edge.owner.capacity() + edge.alias.capacity() + edge.package.capacity(),
        )?;
    }
    Ok(bytes)
}

// Publication is a new caller of the local-lock builders. Keep the local
// implementation unchanged, but prepay its complete bounded construction and
// validation before calling it. The result keeps its measured capacities.
fn relative_storage_bound(base: &Path, target: &Path) -> Result<usize, String> {
    let components = base.components().count() + target.components().count();
    let text = base.as_os_str().len() + target.as_os_str().len();
    let mut bytes = grow_slots(components)?
        * (2 * std::mem::size_of::<Component<'_>>() + std::mem::size_of::<String>());
    add_bytes(&mut bytes, text * 4 + components * 8 + 64)?;
    Ok(bytes)
}

fn expected_registry_bound(
    graph: &[frontend::PackageSpec],
    manifests: &[runtime::ManifestSnapshot],
) -> Result<usize, String> {
    if graph.is_empty() || graph.len() > 256 || graph.len() != manifests.len() {
        return Err("invalid registry publication lock graph".into());
    }
    let edges = graph
        .iter()
        .map(|package| package.dependencies.len())
        .sum::<usize>();
    if edges > 1024 {
        return Err("registry publication exceeds 1024 edges".into());
    }
    let mut bytes = std::mem::size_of::<Snapshot>()
        + graph.len() * (std::mem::size_of::<Package>() + 2 * std::mem::size_of::<String>())
        + grow_slots(edges)? * std::mem::size_of::<Edge>()
        + table_bytes::<(usize, &String)>(graph.len())?;
    let mut text = 0usize;
    for (package, snapshot) in graph.iter().zip(manifests) {
        add_bytes(
            &mut bytes,
            relative_storage_bound(&graph[0].root, &package.root)?,
        )?;
        add_bytes(
            &mut bytes,
            relative_storage_bound(&package.root, &package.entry)?,
        )?;
        // IDs, source checksum, edition, manifest digest, root/entry relative
        // text and copied display metadata. Absolute text bounds relative text.
        add_bytes(
            &mut text,
            4 * 64
                + 71
                + 13
                + graph[0].root.as_os_str().len()
                + package.root.as_os_str().len()
                + package.entry.as_os_str().len()
                + snapshot.manifest.name.as_ref().map_or(0, String::len)
                + snapshot.manifest.version.as_ref().map_or(0, String::len),
        )?;
        for edge in &package.dependencies {
            add_bytes(&mut text, 128 + edge.alias.len())?;
        }
    }
    add_bytes(&mut bytes, text * 2)?;
    add_bytes(&mut bytes, validation_shape(graph.len(), edges, 0, text)?)?;
    // Every encoded field can double through quoting; output growth doubles
    // once more. Include a complete formatted/quoted field at the same time.
    add_bytes(
        &mut bytes,
        (text * 2 + graph.len() * 256 + edges * 64 + 128) * 6,
    )?;
    Ok(bytes)
}

pub(crate) fn expected_registry(
    graph: &[frontend::PackageSpec],
    manifests: &[runtime::ManifestSnapshot],
    fingerprints: &[(usize, String)],
    budget: &mut crate::project_registry::Budget,
) -> Result<Snapshot, String> {
    budget.scoped(
        expected_registry_bound(graph, manifests)?,
        || expected(graph, manifests, fingerprints),
        registry_snapshot_capacity,
    )
}

pub(crate) fn expected_registry_workspace(
    root: &Path,
    manifest: &str,
    inputs: &[WorkspaceInputs<'_>],
    budget: &mut crate::project_registry::Budget,
) -> Result<Snapshot, String> {
    let nodes = inputs.iter().map(|input| input.graph.len()).sum::<usize>();
    let edges = inputs
        .iter()
        .flat_map(|input| input.graph)
        .map(|spec| spec.dependencies.len())
        .sum::<usize>();
    if nodes > 256 || edges > 1024 || inputs.len() > 32 {
        return Err("registry workspace exceeds its existing aggregate graph limits".into());
    }
    let mut bound = std::mem::size_of::<Snapshot>();
    // A BTree node contains at most eleven key/value slots, twelve child
    // pointers and its parent/index/length header. Charging one whole node per
    // inserted item bounds every split without depending on allocator behavior.
    let map_node = 11 * std::mem::size_of::<(String, Package)>()
        + 12 * std::mem::size_of::<usize>()
        + 3 * std::mem::size_of::<usize>();
    let set_node = 11 * std::mem::size_of::<Edge>()
        + 12 * std::mem::size_of::<usize>()
        + 3 * std::mem::size_of::<usize>();
    add_bytes(&mut bound, nodes * map_node + edges * set_node)?;
    add_bytes(&mut bound, table_bytes::<(String, String)>(nodes)?)?;
    add_bytes(
        &mut bound,
        grow_slots(inputs.len())? * (std::mem::size_of::<String>() + 64),
    )?;
    let mut text = manifest.len();
    for input in inputs {
        add_bytes(
            &mut bound,
            expected_registry_bound(input.graph, input.manifests)?,
        )?;
        add_bytes(
            &mut bound,
            table_bytes::<(String, (String, String))>(input.graph.len())?,
        )?;
        add_bytes(
            &mut bound,
            grow_slots(input.fingerprints.len())? * (std::mem::size_of::<(usize, String)>() + 71),
        )?;
        for package in input.graph {
            add_bytes(&mut bound, relative_storage_bound(root, &package.root)? * 3)?;
            add_bytes(
                &mut text,
                root.as_os_str().len() + package.root.as_os_str().len() + 4 * 64 + 71,
            )?;
        }
    }
    add_bytes(
        &mut bound,
        text * 4 + nodes * std::mem::size_of::<Package>() + edges * std::mem::size_of::<Edge>(),
    )?;
    add_bytes(
        &mut bound,
        validation_shape(nodes, edges, inputs.len(), text)?,
    )?;
    // Local builders already paid their field encodings; the union can only
    // retain a subset. A second full local envelope covers union encoding too.
    for input in inputs {
        add_bytes(
            &mut bound,
            expected_registry_bound(input.graph, input.manifests)?,
        )?;
    }
    budget.scoped(
        bound,
        || expected_workspace(root, manifest, inputs),
        registry_snapshot_capacity,
    )
}

pub(crate) fn read_registry(
    root: &Path,
    required: bool,
    workspace: bool,
    budget: &mut crate::project_registry::Budget,
) -> Result<Option<Existing>, String> {
    let path = budget.join(root, "oxid.lock")?;
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return if required {
                Err("this command requires oxid.lock; run `oxid lock` first".into())
            } else {
                Ok(None)
            }
        }
        Err(error) => return Err(format!("cannot inspect typed lock: {error}")),
        Ok(_) => {}
    }
    let bytes = budget.typed_file(&path, MAX_LOCK_BYTES, "typed lock")?;
    let snapshot = budget.scoped(
        registry_decode_bound(&bytes)?,
        || Snapshot::decode(&bytes),
        registry_snapshot_capacity,
    )?;
    if snapshot.workspace.is_some() != workspace {
        return Err("lock format does not match workspace membership; existing locks are never implicitly converted or overwritten".into());
    }
    Ok(Some(Existing { snapshot, bytes }))
}

fn registry_lock_prefix(file: &mut fs::File) -> io::Result<bool> {
    use std::io::Read;
    let mut input = [0; 4096];
    let mut line = [0; 12];
    let mut length = 0usize;
    let mut remaining = MAX_LOCK_BYTES;
    while remaining != 0 {
        let size = input.len().min(remaining);
        let read = file.read(&mut input[..size])?;
        if read == 0 {
            break;
        }
        remaining -= read;
        for &byte in &input[..read] {
            if byte == b'\n' {
                let content = &line[..length];
                let content = content.strip_suffix(b"\r").unwrap_or(content);
                if !content.is_empty() {
                    return Ok(content == b"version = 4");
                }
                length = 0;
            } else {
                if length == line.len() {
                    return Ok(false);
                }
                line[length] = byte;
                length += 1;
            }
        }
    }
    Ok(&line[..length] == b"version = 4")
}

/// Select the registry-only ledger before decoding any new v4 Pin. A project
/// without registry declarations and with a v2/v3 lock keeps the old decoder
/// and does not allocate an allowance. The fixed prefix is not lock authority:
/// the complete regular-file read and strict decoder still decide acceptance.
pub(crate) fn read_with_registry(
    root: &Path,
    required: bool,
    workspace: bool,
    allowance: &mut Option<crate::project_registry::Allowance>,
) -> Result<Option<Existing>, String> {
    if allowance.is_none() {
        let path = root.join("oxid.lock");
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() && metadata.len() <= MAX_LOCK_BYTES as u64 => {
                #[cfg(windows)]
                let registry = frontend::with_windows_regular_file(&path, "typed lock", |file| {
                    registry_lock_prefix(file)
                        .map_err(|error| format!("cannot inspect typed lock: {error}"))
                })?;
                #[cfg(not(windows))]
                let registry = {
                    let mut options = fs::OpenOptions::new();
                    options.read(true);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::OpenOptionsExt;
                        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
                    }
                    let mut file = options
                        .open(&path)
                        .map_err(|error| format!("cannot inspect typed lock: {error}"))?;
                    if !file
                        .metadata()
                        .map_err(|error| error.to_string())?
                        .is_file()
                    {
                        return Err("typed lock requires a regular file".into());
                    }
                    registry_lock_prefix(&mut file)
                        .map_err(|error| format!("cannot inspect typed lock: {error}"))?
                };
                if registry {
                    *allowance = Some(crate::project_registry::operation_budget()?);
                }
            }
            Ok(_) => {} // The unchanged full reader reports its exact refusal.
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot inspect typed lock: {error}")),
        }
    }
    match allowance {
        Some(allowance) => read_registry(root, required, workspace, &mut allowance.borrow_mut()),
        None => {
            if workspace {
                read_workspace(root, required)
            } else {
                read(root, required)
            }
        }
    }
}

pub(crate) fn publish_registry(
    root: &Path,
    snapshot: &Snapshot,
    existing: Option<&Existing>,
    update: bool,
    budget: &mut crate::project_registry::Budget,
) -> Result<bool, String> {
    let mut bound = registry_encoding_bound(snapshot)?;
    add_bytes(&mut bound, 4 * root.as_os_str().len() + 256)?;
    if let Some(existing) = existing {
        add_bytes(
            &mut bound,
            registry_decode_bound(&existing.bytes)? + existing.bytes.len() * 2 + 32,
        )?;
    }
    // Includes the unchanged publication helper's update re-read and paths.
    // Admission occurs before it creates its staging file.
    budget.scoped(
        bound,
        || publish(root, snapshot, existing, update),
        |_| Ok(0),
    )
}

#[repr(C)]
struct RegistryLockRoles {
    snapshot: Snapshot,
    snapshot_return: Result<Snapshot, String>,
    existing: Existing,
    existing_return: Result<Option<Existing>, String>,
    pins: Vec<crate::project_registry::Pin>,
    pins_return: Result<Vec<crate::project_registry::Pin>, String>,
    mapping: Vec<(String, String)>,
    origins: Vec<(String, &'static crate::project_registry::Pin)>,
    path: std::path::PathBuf,
    raw: Vec<u8>,
    raw_return: Result<Vec<u8>, String>,
    bound_return: Result<usize, String>,
    checked_bound: Option<usize>,
    allowance: &'static crate::project_registry::Allowance,
    budget: std::cell::RefMut<'static, crate::project_registry::Budget>,
    validation_return: Result<(), String>,
    lock_prefix: [u8; 12],
    prefix_input: [u8; 4096],
    prefix_counts: [usize; 4],
    prefix_file: fs::File,
    prefix_metadata: fs::Metadata,
    prefix_read_return: io::Result<usize>,
    prefix_return: io::Result<bool>,
}
pub(crate) fn registry_control_bytes() -> usize {
    std::mem::size_of::<RegistryLockRoles>()
}

fn entry_relative(package: &frontend::PackageSpec) -> Result<String, String> {
    let path = package
        .entry
        .strip_prefix(&package.root)
        .map_err(|_| "typed lock entry escapes its package")?;
    let parts = path
        .components()
        .map(|part| match part {
            Component::Normal(value) => value
                .to_str()
                .map(str::to_owned)
                .ok_or("typed lock entry must be UTF-8"),
            _ => Err("typed lock entry must be a normalized relative path"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parts.is_empty() {
        return Err("typed lock entry must be nonempty".into());
    }
    Ok(parts.join("/"))
}

pub(crate) fn expected(
    graph: &[frontend::PackageSpec],
    manifests: &[runtime::ManifestSnapshot],
    fingerprints: &[(usize, String)],
) -> Result<Snapshot, String> {
    if graph.is_empty() || graph.len() != manifests.len() {
        return Err("invalid typed lock package graph".into());
    }
    let paths = graph
        .iter()
        .map(|package| relative(&graph[0].root, &package.root))
        .collect::<Result<Vec<_>, _>>()?;
    let ids: Vec<_> = paths.iter().map(|path| package_id(path)).collect();
    let mut sources = HashMap::new();
    for (package, checksum) in fingerprints {
        if *package == 0 || *package >= graph.len() || sources.insert(*package, checksum).is_some()
        {
            return Err("invalid prepared dependency fingerprint inventory".into());
        }
    }
    let mut packages = Vec::with_capacity(graph.len());
    let mut dependencies = Vec::new();
    for (index, (package, snapshot)) in graph.iter().zip(manifests).enumerate() {
        let manifest = &snapshot.manifest;
        packages.push(Package {
            id: ids[index].clone(),
            path: paths[index].clone(),
            name: manifest.name.clone().unwrap_or_default(),
            version: manifest.version.clone().unwrap_or_default(),
            edition: "typed-preview".into(),
            entry: entry_relative(package)?,
            manifest: snapshot.digest.clone(),
            sources: if index == 0 {
                "root".into()
            } else {
                (*sources
                    .get(&index)
                    .ok_or("prepared dependency fingerprint is missing")?)
                .clone()
            },
            registry: None,
        });
        for edge in &package.dependencies {
            dependencies.push(Edge {
                owner: ids[index].clone(),
                alias: edge.alias.clone(),
                package: ids[edge.package].clone(),
            });
        }
    }
    packages.sort_by(|a, b| a.id.cmp(&b.id));
    dependencies.sort();
    let snapshot = Snapshot {
        registry_format: false,
        root: ids[0].clone(),
        workspace: None,
        members: Vec::new(),
        packages,
        dependencies,
    };
    snapshot.validate()?;
    if snapshot.encode().len() > MAX_LOCK_BYTES {
        return Err("typed lock exceeds the 2 MiB limit".into());
    }
    Ok(snapshot)
}

pub(crate) struct WorkspaceInputs<'a> {
    pub graph: &'a [frontend::PackageSpec],
    pub manifests: &'a [runtime::ManifestSnapshot],
    pub fingerprints: &'a [(usize, String)],
}

pub(crate) fn expected_workspace(
    root: &Path,
    manifest: &str,
    inputs: &[WorkspaceInputs<'_>],
) -> Result<Snapshot, String> {
    let mut packages = std::collections::BTreeMap::<String, Package>::new();
    let mut dependencies = std::collections::BTreeSet::new();
    let mut members = Vec::new();
    let mut retained = HashMap::new();
    for input in inputs {
        let dependency_inputs = input
            .fingerprints
            .iter()
            .filter(|(index, _)| *index != 0)
            .cloned()
            .collect::<Vec<_>>();
        let local = expected(input.graph, input.manifests, &dependency_inputs)?;
        let mut mapping = HashMap::new();
        for package in input.graph {
            let old_path = relative(&input.graph[0].root, &package.root)?;
            let path = relative(root, &package.root)?;
            mapping.insert(package_id(&old_path), (package_id(&path), path));
        }
        members.push(mapping[&local.root].0.clone());
        for (index, checksum) in input.fingerprints {
            let spec = input
                .graph
                .get(*index)
                .ok_or("invalid workspace fingerprint identity")?;
            let id = package_id(&relative(root, &spec.root)?);
            if retained
                .insert(id, checksum.clone())
                .is_some_and(|previous| previous != *checksum)
            {
                return Err(
                    "workspace source changed between prepared member sessions; retry the command"
                        .into(),
                );
            }
        }
        for mut package in local.packages {
            let (id, path) = &mapping[&package.id];
            package.id = id.clone();
            package.path = path.clone();
            if let Some(previous) = packages.get_mut(id) {
                let source = if package.sources == "root" {
                    previous.sources.clone()
                } else {
                    package.sources.clone()
                };
                let mut comparable = previous.clone();
                comparable.sources = package.sources.clone();
                if comparable != package {
                    return Err("workspace package manifest or entry changed between member sessions; retry the command".into());
                }
                previous.sources = source;
            } else {
                packages.insert(id.clone(), package);
            }
        }
        for edge in local.dependencies {
            dependencies.insert(Edge {
                owner: mapping[&edge.owner].0.clone(),
                alias: edge.alias,
                package: mapping[&edge.package].0.clone(),
            });
        }
    }
    members.sort();
    let snapshot = Snapshot {
        registry_format: false,
        root: package_id("."),
        workspace: Some(manifest.to_owned()),
        members,
        packages: packages.into_values().collect(),
        dependencies: dependencies.into_iter().collect(),
    };
    snapshot.validate()?;
    if snapshot.encode().len() > MAX_LOCK_BYTES {
        return Err("typed workspace lock exceeds the 2 MiB limit".into());
    }
    Ok(snapshot)
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_path(value: &str, parents: bool) -> bool {
    if value.is_empty()
        || value.len() > 4096
        || value.starts_with('/')
        || value.chars().any(char::is_control)
    {
        return false;
    }
    if value == "." {
        return parents;
    }
    let mut normal = false;
    for part in value.split('/') {
        match part {
            "" | "." => return false,
            ".." if !parents || normal => return false,
            ".." => {}
            _ => normal = true,
        }
    }
    true
}

impl Snapshot {
    pub(crate) fn export_registry_with_manifests(
        self,
        owner: &Path,
        updates: &HashMap<std::path::PathBuf, String>,
        budget: &mut crate::project_registry::Budget,
    ) -> Result<Vec<u8>, String> {
        let mut bound = registry_validation_bound(&self)? + registry_encoding_bound(&self)?;
        add_bytes(
            &mut bound,
            table_bytes::<(String, String)>(self.packages.len())?
                + table_bytes::<(String, String)>(updates.len())?,
        )?;
        add_bytes(
            &mut bound,
            self.packages.len() * 192
                + self.dependencies.len() * 128
                + self.members.len() * 64
                + 64,
        )?;
        for root in updates.keys() {
            add_bytes(&mut bound, relative_storage_bound(owner, root)? + 64)?;
        }
        budget.scoped(
            bound,
            || self.export_with_manifests(owner, updates),
            |bytes| Ok(bytes.capacity()),
        )
    }

    pub(crate) fn with_registries<'a>(
        mut self,
        owner: &Path,
        origins: impl Iterator<
            Item = (
                &'a Path,
                &'a crate::project_registry::Pin,
                &'a crate::project_registry::Allowance,
            ),
        >,
    ) -> Result<Self, String> {
        let mut origins = origins.peekable();
        let Some((_, _, allowance)) = origins.peek().copied() else {
            return Ok(self);
        };
        let mut budget = allowance.borrow_mut();
        let mut pins = Vec::<(String, &crate::project_registry::Pin)>::new();
        for (root, pin, associated) in origins {
            if !std::rc::Rc::ptr_eq(allowance, associated) {
                return Err("registry graph uses inconsistent operation allowances".into());
            }
            budget.scoped(
                registry_pin_validation_bound(pin)?,
                || pin.validate(),
                |_| Ok(0),
            )?;
            let path = relative_registry(owner, root, &mut budget)?;
            if let Some((_, previous)) = pins.iter().find(|(old, _)| old == &path) {
                if *previous != pin {
                    return Err("registry identity changed between prepared sessions".into());
                }
            } else {
                budget.reserve(&mut pins, 1)?;
                pins.push((path, pin));
            }
        }
        let mut mapping = Vec::new();
        budget.reserve(&mut mapping, self.packages.len())?;
        for package in &mut self.packages {
            let old = std::mem::take(&mut package.id);
            if let Some(index) = pins.iter().position(|(path, _)| path == &package.path) {
                let (_, pin) = pins.swap_remove(index);
                if old == self.root || package.name != pin.name || package.version != pin.version {
                    return Err("registry lock metadata differs from its release".into());
                }
                package.id = budget.identity(&pin.source, &pin.name, &pin.version)?;
                budget.boxed::<crate::project_registry::Pin>()?;
                package.registry = Some(Box::new(crate::project_registry::Pin {
                    source: budget.text(&pin.source)?,
                    name: budget.text(&pin.name)?,
                    version: budget.text(&pin.version)?,
                    artifact: budget.text(&pin.artifact)?,
                    bytes: pin.bytes,
                }));
            } else {
                package.id = budget.text(&old)?;
            }
            mapping.push((old, budget.text(&package.id)?));
        }
        if !pins.is_empty() {
            return Err("registry origin is outside its prepared graph".into());
        }
        for edge in &mut self.dependencies {
            edge.owner = budget.text(
                &mapping
                    .iter()
                    .find(|(id, _)| id == &edge.owner)
                    .ok_or("registry edge owner is missing")?
                    .1,
            )?;
            edge.package = budget.text(
                &mapping
                    .iter()
                    .find(|(id, _)| id == &edge.package)
                    .ok_or("registry edge target is missing")?
                    .1,
            )?;
        }
        self.packages.sort_by(|a, b| a.id.cmp(&b.id));
        self.dependencies.sort();
        self.registry_format = true;
        budget.scoped(
            registry_validation_bound(&self)?,
            || self.validate(),
            |_| Ok(0),
        )?;
        budget.scoped(
            registry_encoding_bound(&self)?,
            || {
                if self.encode().len() > MAX_LOCK_BYTES {
                    return Err("registry lock exceeds 2 MiB".into());
                }
                Ok(())
            },
            |_| Ok(0),
        )?;
        Ok(self)
    }

    /// Export the same graph and sources after its retained manifest values are
    /// converted to portable local dependencies. This never writes a user lock.
    pub(crate) fn export_with_manifests(
        mut self,
        owner: &Path,
        updates: &HashMap<std::path::PathBuf, String>,
    ) -> Result<Vec<u8>, String> {
        // Export is a deliberate local graph, not a registry trust transition.
        // Every copy retains its own location; shared releases remain shared.
        if self.registry_format {
            let mapping = self
                .packages
                .iter()
                .map(|package| (package.id.clone(), package_id(&package.path)))
                .collect::<HashMap<_, _>>();
            for package in &mut self.packages {
                package.id = mapping[&package.id].clone();
                package.registry = None;
            }
            for edge in &mut self.dependencies {
                edge.owner = mapping[&edge.owner].clone();
                edge.package = mapping[&edge.package].clone();
            }
            for member in &mut self.members {
                *member = mapping[member].clone();
            }
            self.root = package_id(".");
            self.registry_format = false;
            self.packages.sort_by(|a, b| a.id.cmp(&b.id));
            self.dependencies.sort();
            self.members.sort();
        }
        let mut pending = HashMap::new();
        for (root, digest) in updates {
            if !valid_hash(digest)
                || pending
                    .insert(relative(owner, root)?, digest.clone())
                    .is_some()
            {
                return Err("invalid exported manifest inventory".into());
            }
        }
        for package in &mut self.packages {
            if let Some(digest) = pending.remove(&package.path) {
                package.manifest = digest;
            }
        }
        if let Some(workspace) = &mut self.workspace {
            if let Some(digest) = updates.get(owner) {
                *workspace = digest.clone();
                pending.remove(".");
            }
        }
        if !pending.is_empty() {
            return Err("exported manifest is outside its retained lock graph".into());
        }
        self.validate()?;
        let bytes = self.encode().into_bytes();
        if bytes.len() > MAX_LOCK_BYTES {
            return Err("exported lock exceeds the 2 MiB limit".into());
        }
        Ok(bytes)
    }

    fn validate(&self) -> Result<(), String> {
        let invalid = || "malformed typed lock: invalid package or dependency record".to_owned();
        if self.packages.is_empty()
            || self.packages.len() > 256
            || self.dependencies.len() > 1024
            || self.root != package_id(".")
        {
            return Err(invalid());
        }
        let workspace = self.workspace.is_some();
        if workspace
            && (!self.workspace.as_deref().is_some_and(valid_hash)
                || self.members.is_empty()
                || self.members.len() > 32
                || self.members.windows(2).any(|pair| pair[0] >= pair[1]))
        {
            return Err(invalid());
        }
        if !workspace && !self.members.is_empty() {
            return Err(invalid());
        }
        let imported = self
            .dependencies
            .iter()
            .map(|edge| &edge.package)
            .collect::<HashSet<_>>();
        let mut ids = HashMap::new();
        let mut last = None;
        for package in &self.packages {
            if let Some(pin) = &package.registry {
                pin.validate()?;
                if !self.registry_format
                    || pin.name != package.name
                    || pin.version != package.version
                    || package.id == self.root
                {
                    return Err(invalid());
                }
            }
            if !valid_hash(&package.id)
                || package.id
                    != package
                        .registry
                        .as_ref()
                        .map_or_else(|| package_id(&package.path), |pin| pin.id())
                || !valid_path(&package.path, true)
                || !valid_path(&package.entry, false)
                || package.edition != "typed-preview"
                || !valid_hash(&package.manifest)
                || package.name.len() > 4096
                || package.version.len() > 4096
                || package.name.chars().any(char::is_control)
                || package.version.chars().any(char::is_control)
                || !package
                    .name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                || ((workspace || package.id != self.root) && package.name.is_empty())
                || if workspace {
                    if self.members.contains(&package.id) && !imported.contains(&package.id) {
                        package.sources != "root"
                    } else {
                        !package
                            .sources
                            .strip_prefix("sha256:")
                            .is_some_and(valid_hash)
                    }
                } else if package.id == self.root {
                    package.path != "." || package.sources != "root"
                } else {
                    !package
                        .sources
                        .strip_prefix("sha256:")
                        .is_some_and(valid_hash)
                }
                || last.is_some_and(|id: &String| id >= &package.id)
                || ids.insert(&package.id, package).is_some()
            {
                return Err(invalid());
            }
            last = Some(&package.id);
        }
        if (!workspace && !ids.contains_key(&self.root))
            || self.members.iter().any(|member| !ids.contains_key(member))
        {
            return Err(invalid());
        }
        let mut aliases = HashSet::new();
        let mut last = None;
        for edge in &self.dependencies {
            if !ids.contains_key(&edge.owner)
                || !ids.contains_key(&edge.package)
                || edge.owner == edge.package
                || crate::project_cli::validate_alias(&edge.alias).is_err()
                || !aliases.insert((&edge.owner, edge.alias.to_ascii_lowercase()))
                || last.is_some_and(|previous: &Edge| previous >= edge)
            {
                return Err(invalid());
            }
            last = Some(edge);
        }
        fn visit<'a>(
            id: &'a str,
            edges: &'a [Edge],
            active: &mut HashSet<&'a str>,
            lengths: &mut HashMap<&'a str, usize>,
        ) -> Result<usize, String> {
            if let Some(depth) = lengths.get(id) {
                return Ok(*depth);
            }
            if !active.insert(id) {
                return Err("malformed typed lock: dependency cycle".into());
            }
            let mut depth = 0;
            for edge in edges.iter().filter(|edge| edge.owner == id) {
                depth = depth.max(1 + visit(&edge.package, edges, active, lengths)?);
            }
            active.remove(id);
            lengths.insert(id, depth);
            Ok(depth)
        }
        let mut lengths = HashMap::new();
        for member in if workspace {
            self.members.as_slice()
        } else {
            std::slice::from_ref(&self.root)
        } {
            if visit(
                member,
                &self.dependencies,
                &mut HashSet::new(),
                &mut lengths,
            )? > 32
            {
                return Err(invalid());
            }
        }
        if lengths.len() != self.packages.len() {
            return Err(invalid());
        }
        Ok(())
    }

    fn encode(&self) -> String {
        let mut text = if let Some(manifest) = &self.workspace {
            let header = if self.registry_format {
                "version = 4\nkind = \"typed-registry-workspace-v1\""
            } else {
                "version = 3\nkind = \"typed-workspace-v1\""
            };
            let mut text = format!("{header}\nmanifest = {}\n", quote(manifest));
            for member in &self.members {
                text.push_str(&format!("\n[[member]]\npackage = {}\n", quote(member)));
            }
            text
        } else {
            let header = if self.registry_format {
                "version = 4\nkind = \"typed-registry-project-v1\""
            } else {
                "version = 2\nkind = \"typed-local-v1\""
            };
            format!("{header}\nroot = {}\n", quote(&self.root))
        };
        for package in &self.packages {
            text.push_str("\n[[package]]\n");
            for (name, value) in [
                ("id", &package.id),
                ("path", &package.path),
                ("name", &package.name),
                ("version", &package.version),
                ("edition", &package.edition),
                ("entry", &package.entry),
                ("manifest", &package.manifest),
                ("sources", &package.sources),
            ] {
                text.push_str(&format!("{name} = {}\n", quote(value)));
            }
            if self.registry_format {
                let (source, artifact, bytes) =
                    package.registry.as_ref().map_or(("", "", 0), |pin| {
                        (pin.source.as_str(), pin.artifact.as_str(), pin.bytes)
                    });
                text.push_str(&format!(
                    "registry = {}\nartifact = {}\nartifact-bytes = {bytes}\n",
                    quote(source),
                    quote(artifact)
                ));
            }
        }
        for edge in &self.dependencies {
            text.push_str(&format!(
                "\n[[dependency]]\nowner = {}\nalias = {}\npackage = {}\n",
                quote(&edge.owner),
                quote(&edge.alias),
                quote(&edge.package)
            ));
        }
        text
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let text =
            std::str::from_utf8(bytes).map_err(|_| "malformed typed lock: file is not UTF-8")?;
        let mut lines = text.lines().filter(|line| !line.is_empty()).peekable();
        let version = lines.next();
        let kind = lines.next();
        let registry_format = version == Some("version = 4")
            && matches!(
                kind,
                Some(
                    "kind = \"typed-registry-workspace-v1\""
                        | "kind = \"typed-registry-project-v1\""
                )
            );
        let workspace = version == Some("version = 3")
            && kind == Some("kind = \"typed-workspace-v1\"")
            || registry_format && kind == Some("kind = \"typed-registry-workspace-v1\"");
        if !workspace
            && !registry_format
            && !(version == Some("version = 2") && kind == Some("kind = \"typed-local-v1\""))
        {
            return Err("unsupported lock format: expected typed-local-v1 version 2, typed-workspace-v1 version 3, or a typed registry version 4 lock; old or unknown locks are never replaced".into());
        }
        fn field<'a>(
            lines: &mut impl Iterator<Item = &'a str>,
            key: &str,
        ) -> Result<String, String> {
            let line = lines.next().ok_or("malformed typed lock: missing field")?;
            let raw = line
                .strip_prefix(&format!("{key} = "))
                .ok_or_else(|| format!("malformed typed lock: expected field `{key}`"))?;
            unquote(raw)
        }
        let workspace = if workspace {
            Some(field(&mut lines, "manifest")?)
        } else {
            None
        };
        let root = if workspace.is_some() {
            package_id(".")
        } else {
            field(&mut lines, "root")?
        };
        let mut members = Vec::new();
        if workspace.is_some() {
            while lines.peek() == Some(&"[[member]]") {
                lines.next();
                if members.len() >= 32 {
                    return Err("malformed typed lock: too many workspace members".into());
                }
                members.push(field(&mut lines, "package")?);
            }
        }
        let mut packages = Vec::new();
        let mut dependencies = Vec::new();
        while lines.peek() == Some(&"[[package]]") {
            lines.next();
            if packages.len() >= 256 {
                return Err("malformed typed lock: too many packages".into());
            }
            let mut package = Package {
                id: field(&mut lines, "id")?,
                path: field(&mut lines, "path")?,
                name: field(&mut lines, "name")?,
                version: field(&mut lines, "version")?,
                edition: field(&mut lines, "edition")?,
                entry: field(&mut lines, "entry")?,
                manifest: field(&mut lines, "manifest")?,
                sources: field(&mut lines, "sources")?,
                registry: None,
            };
            if registry_format {
                let source = field(&mut lines, "registry")?;
                let artifact = field(&mut lines, "artifact")?;
                let raw = lines
                    .next()
                    .and_then(|line| line.strip_prefix("artifact-bytes = "))
                    .ok_or("malformed registry lock byte count")?;
                let bytes: u64 = raw
                    .parse()
                    .map_err(|_| "malformed registry lock byte count")?;
                if bytes.to_string() != raw {
                    return Err("noncanonical registry lock byte count".into());
                }
                if source.is_empty() {
                    if !artifact.is_empty() || bytes != 0 {
                        return Err("local lock package has registry artifact fields".into());
                    }
                } else {
                    package.registry = Some(Box::new(crate::project_registry::Pin {
                        source,
                        name: package.name.clone(),
                        version: package.version.clone(),
                        artifact,
                        bytes,
                    }));
                }
            }
            packages.push(package);
        }
        while lines.peek() == Some(&"[[dependency]]") {
            lines.next();
            if dependencies.len() >= 1024 {
                return Err("malformed typed lock: too many dependency edges".into());
            }
            dependencies.push(Edge {
                owner: field(&mut lines, "owner")?,
                alias: field(&mut lines, "alias")?,
                package: field(&mut lines, "package")?,
            });
        }
        if lines.next().is_some() {
            return Err("malformed typed lock: unexpected field or table".into());
        }
        let snapshot = Self {
            registry_format,
            root,
            workspace,
            members,
            packages,
            dependencies,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
}

fn quote(value: &str) -> String {
    let mut text = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => text.push_str("\\\\"),
            '"' => text.push_str("\\\""),
            character => text.push(character),
        }
    }
    text.push('"');
    text
}

fn unquote(raw: &str) -> Result<String, String> {
    let raw = raw
        .strip_prefix('"')
        .and_then(|raw| raw.strip_suffix('"'))
        .ok_or("malformed typed lock: expected quoted string")?;
    if raw.len() > 8192 {
        return Err("malformed typed lock: string exceeds limit".into());
    }
    let mut chars = raw.chars();
    let mut value = String::new();
    while let Some(character) = chars.next() {
        match character {
            '\\' => match chars.next() {
                Some(character @ ('\\' | '"')) => value.push(character),
                _ => return Err("malformed typed lock: invalid escape".into()),
            },
            '"' => return Err("malformed typed lock: unescaped quote".into()),
            character if character.is_control() => {
                return Err("malformed typed lock: control character".into())
            }
            character => value.push(character),
        }
    }
    Ok(value)
}

pub(crate) fn read(root: &Path, required: bool) -> Result<Option<Existing>, String> {
    read_kind(root, required, false)
}

pub(crate) fn read_workspace(root: &Path, required: bool) -> Result<Option<Existing>, String> {
    read_kind(root, required, true)
}

/// Existing authority is checked before a resolver can publish a missing cache
/// entry. Ordinary callers never invoke this registry-only read.
pub(crate) fn registry_trust(
    root: &Path,
    budget: &mut crate::project_registry::Budget,
) -> Result<Vec<crate::project_registry::Pin>, String> {
    let path = budget.join(root, "oxid.lock")?;
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("cannot inspect typed lock: {error}")),
        Ok(_) => {}
    }
    let bytes = budget.typed_file(&path, MAX_LOCK_BYTES, "typed lock")?;
    let snapshot = budget.scoped(
        registry_decode_bound(&bytes)?,
        || Snapshot::decode(&bytes),
        registry_snapshot_capacity,
    )?;
    let count = snapshot
        .packages
        .iter()
        .filter(|package| package.registry.is_some())
        .count();
    let mut pins = Vec::new();
    budget.reserve(&mut pins, count)?;
    for package in snapshot.packages {
        if let Some(pin) = package.registry {
            pins.push(*pin);
        }
    }
    Ok(pins)
}

/// Discovery only: a generated shared lock still owns the directory when its
/// manifest declaration is accidentally removed. Unrelated legacy/local locks
/// remain outside workspace discovery. Full admission always uses read_kind.
pub(crate) fn has_workspace_marker(root: &Path) -> Result<bool, String> {
    use std::io::Read;
    let path = root.join("oxid.lock");
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("cannot inspect workspace lock ownership: {error}")),
    }
    let read = |file: &mut fs::File| -> Result<Vec<u8>, String> {
        let mut prefix = Vec::new();
        file.take(128)
            .read_to_end(&mut prefix)
            .map_err(|error| format!("cannot inspect workspace lock ownership: {error}"))?;
        Ok(prefix)
    };
    #[cfg(windows)]
    let prefix = frontend::with_windows_regular_file(&path, "workspace lock ownership", read)?;
    #[cfg(not(windows))]
    let prefix = {
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let mut file = options
            .open(path)
            .map_err(|error| format!("cannot inspect workspace lock ownership: {error}"))?;
        if !file
            .metadata()
            .map_err(|error| error.to_string())?
            .is_file()
        {
            return Ok(false);
        }
        read(&mut file)?
    };
    let text = String::from_utf8_lossy(&prefix);
    let mut lines = text.lines();
    Ok(matches!(
        (lines.next(), lines.next()),
        (Some("version = 3"), Some("kind = \"typed-workspace-v1\""))
            | (
                Some("version = 4"),
                Some("kind = \"typed-registry-workspace-v1\"")
            )
    ))
}

fn read_kind(root: &Path, required: bool, workspace: bool) -> Result<Option<Existing>, String> {
    let path = root.join("oxid.lock");
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return if required {
                Err("this command requires oxid.lock; run `oxid lock` first".into())
            } else {
                Ok(None)
            }
        }
        Err(error) => return Err(format!("cannot inspect typed lock: {error}")),
        Ok(_) => {}
    }
    let bytes = runtime::read_typed_regular_file(&path, MAX_LOCK_BYTES, "typed lock")?;
    let snapshot = Snapshot::decode(&bytes)?;
    if snapshot.workspace.is_some() != workspace {
        return Err("lock format does not match workspace membership; existing locks are never implicitly converted or overwritten".into());
    }
    Ok(Some(Existing { snapshot, bytes }))
}

pub(crate) fn verify(expected: &Snapshot, existing: Option<&Existing>) -> Result<(), String> {
    if let Some(existing) = existing {
        verify_immutable(expected, &existing.snapshot)?;
        if existing.snapshot != *expected {
            return Err("typed lock is out of date: manifests, dependency sources, or graph changed; review changes and run `oxid lock --update`".into());
        }
    }
    Ok(())
}

fn verify_immutable(expected: &Snapshot, previous: &Snapshot) -> Result<(), String> {
    for package in &expected.packages {
        if let Some(pin) = &package.registry {
            if previous
                .packages
                .iter()
                .find(|old| old.id == package.id)
                .and_then(|old| old.registry.as_ref())
                .is_some_and(|old| old != pin)
            {
                return Err("registry release changed under an already trusted name/version/source; lock --update cannot accept a republished version".into());
            }
        }
    }
    Ok(())
}

/// Complete a staging write before publishing. Missing locks use an exclusive
/// link; updates revalidate the original bytes before atomic replacement.
pub(crate) fn publish(
    root: &Path,
    snapshot: &Snapshot,
    existing: Option<&Existing>,
    update: bool,
) -> Result<bool, String> {
    if let Some(existing) = existing {
        verify_immutable(snapshot, &existing.snapshot)?;
        if existing.snapshot == *snapshot {
            return Ok(false);
        }
        if !update {
            verify(snapshot, Some(existing))?;
        }
    }
    let destination = root.join("oxid.lock");
    let stage = root.join(format!(
        ".oxid-lock-{}-{}.tmp",
        std::process::id(),
        NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stage)
        .map_err(|error| format!("cannot stage typed lock: {error}"))?;
    let result = (|| {
        file.write_all(snapshot.encode().as_bytes())
            .map_err(|error| format!("cannot write typed lock: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("cannot synchronize typed lock: {error}"))?;
        drop(file);
        if let Some(existing) = existing {
            let current = read_kind(root, true, snapshot.workspace.is_some())?
                .ok_or("typed lock disappeared during update")?;
            if current.bytes != existing.bytes {
                return Err("typed lock changed during update; retry after reviewing it".into());
            }
            fs::rename(&stage, &destination)
                .map_err(|error| format!("cannot replace typed lock: {error}"))?;
        } else {
            fs::hard_link(&stage, &destination).map_err(|error| {
                format!("cannot publish typed lock without replacing an existing file: {error}")
            })?;
        }
        Ok(true)
    })();
    let _ = fs::remove_file(&stage);
    result
}

#[cfg(test)]
mod registry_carrier_tests {
    use super::*;
    use std::mem::{align_of, size_of};
    #[test]
    fn registry_lock_carriers_have_independent_member_offsets() {
        macro_rules! layout {
            ($($field:ident : $ty:ty),+ $(,)?) => {{
                let mut bytes = 0usize; let mut alignment = 1usize;
                $(let align = align_of::<$ty>(); alignment = alignment.max(align);
                  bytes = bytes.div_ceil(align) * align;
                  assert_eq!(std::mem::offset_of!(RegistryLockRoles,$field), bytes);
                  bytes += size_of::<$ty>();)+
                assert_eq!(align_of::<RegistryLockRoles>(),alignment);
                assert_eq!(registry_control_bytes(), bytes.div_ceil(alignment) * alignment);
            }};
        }
        layout!(snapshot: Snapshot, snapshot_return: Result<Snapshot,String>, existing: Existing,
            existing_return: Result<Option<Existing>,String>, pins: Vec<crate::project_registry::Pin>,
            pins_return: Result<Vec<crate::project_registry::Pin>,String>, mapping: Vec<(String,String)>,
            origins: Vec<(String,&'static crate::project_registry::Pin)>, path: std::path::PathBuf,
            raw: Vec<u8>, raw_return: Result<Vec<u8>,String>, bound_return: Result<usize,String>,
            checked_bound: Option<usize>, allowance: &'static crate::project_registry::Allowance,
            budget: std::cell::RefMut<'static,crate::project_registry::Budget>, validation_return: Result<(),String>,
            lock_prefix: [u8;12], prefix_input: [u8;4096], prefix_counts: [usize;4], prefix_file: fs::File,
            prefix_metadata: fs::Metadata, prefix_read_return: io::Result<usize>, prefix_return: io::Result<bool>);
    }
}
