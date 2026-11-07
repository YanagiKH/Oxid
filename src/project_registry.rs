//! Explicit, immutable private library releases. Registry work stays outside
//! compiler carriers; ordinary path/Git projects do not allocate this session.
mod resources;
#[cfg(test)]
mod tests;

use crate::{project_package as package, runtime};
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

const MARKER: &[u8] = b"version = 1\nkind = \"typed-private-registry-v1\"\n";
const MAX_METADATA: usize = 8 * 1024 * 1024;
const MAX_WIRE: u64 = 64 * 1024 * 1024;
const MAX_PAYLOAD: usize = 32 * 1024 * 1024;
const MAX_FILES: usize = 2047;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Dependency {
    pub source: String,
    pub name: String,
    pub version: String,
}

/// Immutable parsed declarations share both identity and the operation's
/// allowance through manifest clones. A prepaid parser has no independent
/// allowance: its caller has already reserved the complete parser envelope.
#[derive(Debug)]
pub(crate) struct Declarations {
    entries: Vec<(String, Dependency)>,
    pub(crate) allowance: Option<Allowance>,
}
impl std::ops::Deref for Declarations {
    type Target = Vec<(String, Dependency)>;
    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}
impl Declarations {
    pub(crate) fn new(
        context: Option<&mut Option<Allowance>>,
        prepaid: bool,
    ) -> Result<Rc<Self>, String> {
        let allowance = if prepaid {
            None
        } else {
            let allowance = match context {
                Some(context) => {
                    if context.is_none() {
                        *context = Some(operation_budget()?);
                    }
                    context.as_ref().expect("registry allowance").clone()
                }
                None => operation_budget()?,
            };
            allowance
                .borrow_mut()
                .allocation(resources::declaration_allocation_bytes())?;
            Some(allowance)
        };
        Ok(Rc::new(Self {
            entries: Vec::new(),
            allowance,
        }))
    }
    pub(crate) fn insert(&mut self, alias: &str, raw: &str) -> Result<(), String> {
        if self.entries.len() >= 1024 {
            return Err("registry dependencies exceed the 1024-edge limit".into());
        }
        if let Some(allowance) = &self.allowance {
            let mut budget = allowance.borrow_mut();
            // Plain descriptor fields copy at most the raw bytes. Its URI
            // decoder temporarily retains at most another raw-length buffer.
            let bound = raw
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(std::mem::size_of::<Dependency>() + 128))
                .ok_or("registry descriptor storage overflow")?;
            let dependency = budget.scoped(
                bound,
                || Dependency::parse(raw),
                |dependency| {
                    Ok(std::mem::size_of::<Dependency>()
                        + dependency.source.capacity()
                        + dependency.name.capacity()
                        + dependency.version.capacity())
                },
            )?;
            budget.reserve(&mut self.entries, 1)?;
            self.entries.push((budget.text(alias)?, dependency));
        } else {
            let dependency = Dependency::parse(raw)?;
            self.entries
                .try_reserve_exact(1)
                .map_err(|_| "cannot reserve prepaid registry descriptors")?;
            if self.entries.capacity() != self.entries.len() + 1 {
                return Err("registry descriptor capacity exceeds admission".into());
            }
            self.entries.push((alias.to_owned(), dependency));
        }
        Ok(())
    }
}

pub(crate) fn version(value: &str) -> Result<(), String> {
    let mut count = 0;
    for part in value.split('.') {
        count += 1;
        if part.is_empty()
            || part.len() > 10
            || part.len() > 1 && part.starts_with('0')
            || !part.bytes().all(|byte| byte.is_ascii_digit())
            || part.parse::<u32>().is_err()
        {
            return Err("registry versions require an exact canonical MAJOR.MINOR.PATCH (u32 components); ranges, prereleases and build metadata are unsupported".into());
        }
    }
    if count != 3 {
        return Err("registry versions require exactly three numeric components".into());
    }
    Ok(())
}

fn name(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 64
        || !value.as_bytes()[0].is_ascii_lowercase()
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
    {
        return Err("registry package names require at most 64 lowercase ASCII letters, digits, `_` or `-`, starting with a letter".into());
    }
    Ok(())
}

impl Dependency {
    /// A deliberately small typed-only inline table. Quoted path strings keep
    /// their historical meaning and are never routed through this parser.
    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        if raw.len() > 4096 {
            return Err("registry dependency descriptor exceeds 4096 bytes".into());
        }
        let bad = || {
            "registry dependencies require exactly { registry = \"file:///DIRECTORY\", package = \"name\", version = \"1.2.3\" } without unknown or duplicate fields".to_owned()
        };
        let mut rest = raw.trim().strip_prefix('{').ok_or_else(bad)?;
        let mut fields = [None; 3];
        for ordinal in 0..3 {
            rest = rest.trim_start();
            let (key, tail) = rest.split_once('=').ok_or_else(bad)?;
            let slot = match key.trim() {
                "registry" => 0,
                "package" => 1,
                "version" => 2,
                _ => return Err(bad()),
            };
            let tail = tail.trim_start().strip_prefix('"').ok_or_else(bad)?;
            let (value, tail) = tail.split_once('"').ok_or_else(bad)?;
            if value.is_empty()
                || value.len() > 4096
                || value.contains('\\')
                || value.chars().any(char::is_control)
                || fields[slot].replace(value).is_some()
            {
                return Err(bad());
            }
            rest = tail.trim_start();
            rest = if ordinal == 2 {
                rest.strip_prefix('}')
            } else {
                rest.strip_prefix(',')
            }
            .ok_or_else(bad)?;
        }
        if !rest.trim().is_empty() && !rest.trim_start().starts_with('#') {
            return Err(bad());
        }
        let source = fields[0].ok_or_else(bad)?;
        if source != "self" {
            uri_path(source)?;
        }
        name(fields[1].ok_or_else(bad)?)?;
        version(fields[2].ok_or_else(bad)?)?;
        Ok(Self {
            source: source.into(),
            name: fields[1].unwrap().into(),
            version: fields[2].unwrap().into(),
        })
    }

    pub(crate) fn encode(&self) -> String {
        format!(
            "{{ registry = \"{}\", package = \"{}\", version = \"{}\" }}",
            self.source, self.name, self.version
        )
    }
}

/// A source identity is lexical and bounded even when its registry is offline.
/// Filesystem admission additionally proves this canonical path has no links.
pub(crate) fn uri_path(uri: &str) -> Result<PathBuf, String> {
    let raw = uri
        .strip_prefix("file:///")
        .ok_or("registry source requires file:/// followed by an absolute directory")?;
    if raw.is_empty()
        || uri.len() > 4096
        || raw.contains(['?', '#', '\\', '"'])
        || raw.chars().any(char::is_control)
    {
        return Err("invalid private registry file URI".into());
    }
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(raw.len() + 1)
        .map_err(|_| "cannot reserve registry URI")?;
    decoded.push(b'/');
    let bytes = raw.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let byte = if bytes[at] == b'%' {
            let digits = bytes
                .get(at + 1..at + 3)
                .ok_or("truncated registry URI escape")?;
            let hex = |b: u8| {
                (b as char)
                    .to_digit(16)
                    .ok_or("invalid registry URI escape")
            };
            let value = (hex(digits[0])? * 16 + hex(digits[1])?) as u8;
            if matches!(value, b'/' | b'\\' | 0) {
                return Err("registry URI cannot escape path separators or NUL".into());
            }
            at += 3;
            value
        } else {
            let byte = bytes[at];
            at += 1;
            byte
        };
        decoded.push(byte);
    }
    let text = String::from_utf8(decoded).map_err(|_| "registry URI is not UTF-8")?;
    if text.chars().any(char::is_control)
        || text
            .split('/')
            .skip(1)
            .any(|p| matches!(p, "" | "." | ".."))
    {
        return Err("registry URI requires normalized nonempty directory components".into());
    }
    Ok(PathBuf::from(text))
}

pub(crate) fn normalized_uri(uri: &str) -> Result<String, String> {
    let path = uri_path(uri)?;
    let path = path.to_str().ok_or("registry directory must be UTF-8")?;
    let width = path
        .bytes()
        .try_fold(7usize, |sum, byte| {
            sum.checked_add(
                if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~')
                {
                    1
                } else {
                    3
                },
            )
        })
        .filter(|width| *width <= 4096)
        .ok_or("canonical registry URI exceeds 4096 bytes")?;
    let mut result = String::new();
    result
        .try_reserve_exact(width)
        .map_err(|_| "cannot reserve canonical registry URI")?;
    if result.capacity() != width {
        return Err("canonical registry URI capacity exceeds admission".into());
    }
    result.push_str("file://");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            result.push(byte as char);
        } else {
            use std::fmt::Write;
            write!(result, "%{byte:02X}").map_err(|_| "cannot encode registry URI")?;
        }
    }
    Ok(result)
}

fn ordinary(path: &Path, file: bool) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("registry paths must be absolute".into());
    }
    if path
        .components()
        .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
    {
        return Err("registry paths must be normalized".into());
    }
    for current in path.ancestors() {
        let metadata = fs::symlink_metadata(current)
            .map_err(|e| format!("cannot inspect registry path {}: {e}", current.display()))?;
        if !(if file && current == path {
            metadata.file_type().is_file()
        } else {
            metadata.file_type().is_dir()
        }) {
            return Err("registry paths require ordinary files/directories without symbolic links or special files".into());
        }
        if file && current == path {
            single_link(&metadata)?;
        }
    }
    Ok(())
}

fn single_link(_metadata: &fs::Metadata) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if _metadata.nlink() != 1 {
            return Err("registry files must have exactly one hard link".into());
        }
    }
    Ok(())
}

fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    let same = left.is_file()
        && right.is_file()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        same && left.dev() == right.dev()
            && left.ino() == right.ino()
            && left.mode() == right.mode()
            && left.nlink() == 1
            && right.nlink() == 1
            && left.ctime() == right.ctime()
            && left.ctime_nsec() == right.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        same
    }
}

pub(crate) fn open_regular(
    path: &Path,
    maximum: usize,
) -> Result<(fs::File, fs::Metadata), String> {
    ordinary(path, true)?;
    let before = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if before.len() > maximum as u64 {
        return Err(format!(
            "registry input exceeds its {maximum} byte allowance"
        ));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|error| error.to_string())?;
    let opened = file.metadata().map_err(|error| error.to_string())?;
    if !same_file(&before, &opened) {
        return Err("registry input changed while opening".into());
    }
    Ok((file, opened))
}

pub(crate) fn recheck(path: &Path, file: &fs::File, before: &fs::Metadata) -> Result<(), String> {
    let after = file.metadata().map_err(|error| error.to_string())?;
    let current = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !same_file(before, &after) || !same_file(&after, &current) {
        return Err("registry input changed during verification".into());
    }
    Ok(())
}

fn read_regular(path: &Path, maximum: usize, budget: &mut Budget) -> Result<Vec<u8>, String> {
    let (mut file, before) = open_regular(path, maximum)?;
    let length = usize::try_from(before.len()).map_err(|_| "registry input length overflow")?;
    let mut bytes = Vec::new();
    budget.reserve(&mut bytes, length)?;
    bytes.resize(length, 0);
    file.read_exact(&mut bytes)
        .map_err(|error| error.to_string())?;
    if file
        .read(&mut [0u8; 1])
        .map_err(|error| error.to_string())?
        != 0
    {
        return Err("registry input grew during reading".into());
    }
    recheck(path, &file, &before)?;
    Ok(bytes)
}

pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Pin {
    pub source: String,
    pub name: String,
    pub version: String,
    pub artifact: String,
    pub bytes: u64,
}

impl Pin {
    pub(crate) fn validate(&self) -> Result<(), String> {
        name(&self.name)?;
        version(&self.version)?;
        if normalized_uri(&self.source)? != self.source
            || !valid_hash(&self.artifact)
            || self.bytes > MAX_WIRE
            || self.bytes == 0
        {
            return Err("invalid immutable registry release identity".into());
        }
        Ok(())
    }
    pub(crate) fn id(&self) -> String {
        release_id(&self.source, &self.name, &self.version)
    }
}

fn release_id(source: &str, name: &str, version: &str) -> String {
    let mut hash = Sha256::new();
    for part in [
        b"oxid-typed-registry-package-v1".as_slice(),
        source.as_bytes(),
        name.as_bytes(),
        version.as_bytes(),
    ] {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    format!("{:x}", hash.finalize())
}

#[derive(Clone, Debug)]
struct Record {
    name: String,
    version: String,
    entry: String,
    manifest: String,
    artifact: String,
    bytes: u64,
}
impl Record {
    fn encode(&self, budget: &mut Budget) -> Result<String, String> {
        use std::fmt::Write as _;
        name_check(self)?;
        let digits = self.bytes.ilog10() as usize + 1;
        let length = "version = 1\nkind = \"typed-library-release-v1\"\nname = \"\"\npackage-version = \"\"\nedition = \"typed-preview\"\nentry = \"\"\nmanifest = \"\"\nartifact = \"\"\nbytes = \n".len()
            + self.name.len() + self.version.len() + self.entry.len() + self.manifest.len() + self.artifact.len() + digits;
        let mut output = budget.string(length)?;
        write!(output, "version = 1\nkind = \"typed-library-release-v1\"\nname = \"{}\"\npackage-version = \"{}\"\nedition = \"typed-preview\"\nentry = \"{}\"\nmanifest = \"{}\"\nartifact = \"{}\"\nbytes = {}\n", self.name, self.version, self.entry, self.manifest, self.artifact, self.bytes).map_err(|_| "cannot encode release record")?;
        if output.len() != length || output.capacity() != length {
            return Err("release record exceeded its exact admission".into());
        }
        Ok(output)
    }
    fn decode(bytes: &[u8], budget: &mut Budget) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|_| "release record is not UTF-8")?;
        let mut lines = text.lines();
        if lines.next() != Some("version = 1")
            || lines.next() != Some("kind = \"typed-library-release-v1\"")
        {
            return Err("unsupported private library release record".into());
        }
        let mut string = |key: &str| -> Result<String, String> {
            let raw = lines
                .next()
                .and_then(|line| line.strip_prefix(key))
                .and_then(|line| line.strip_prefix(" = \""))
                .and_then(|line| line.strip_suffix('"'))
                .ok_or("malformed release record")?;
            if raw.contains(['\\', '"']) || raw.chars().any(char::is_control) {
                return Err("malformed release string".into());
            }
            budget.text(raw)
        };
        let name = string("name")?;
        let version = string("package-version")?;
        if string("edition")? != "typed-preview" {
            return Err("release requires typed-preview edition".into());
        }
        let entry = string("entry")?;
        let manifest = string("manifest")?;
        let artifact = string("artifact")?;
        let count = lines
            .next()
            .and_then(|line| line.strip_prefix("bytes = "))
            .ok_or("missing release byte count")?;
        let record = Self {
            name,
            version,
            entry,
            manifest,
            artifact,
            bytes: count.parse().map_err(|_| "invalid release byte count")?,
        };
        name_check(&record)?;
        if lines.next().is_some() || record.encode(budget)?.as_bytes() != bytes {
            return Err("release record is not canonical".into());
        }
        Ok(record)
    }
}

fn name_check(record: &Record) -> Result<(), String> {
    name(&record.name)?;
    version(&record.version)?;
    if !valid_hash(&record.manifest)
        || !valid_hash(&record.artifact)
        || record.bytes == 0
        || record.bytes > MAX_WIRE
        || record.entry.len() > 4096
        || record
            .entry
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
        || record.entry.contains(['\\', '"', ':'])
        || record.entry.chars().any(char::is_control)
    {
        return Err("invalid release record fields".into());
    }
    Ok(())
}

#[derive(Debug)]
struct Input {
    path: String,
    bytes: usize,
    digest: [u8; 32],
}

#[derive(Debug)]
pub(crate) struct Origin {
    pub pin: Pin,
    pub root: PathBuf,
    manifest: String,
    inputs: Vec<Input>,
}
impl Origin {
    pub(crate) fn manifest(&self, digest: &str) -> Result<(), String> {
        if digest != self.manifest {
            return Err("registry manifest differs from its immutable release".into());
        }
        Ok(())
    }
    pub(crate) fn source(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        let input = self
            .inputs
            .iter()
            .find(|input| input.path == path)
            .ok_or("loaded registry source is outside its immutable release")?;
        if input.bytes != bytes.len() || input.digest != <[u8; 32]>::from(Sha256::digest(bytes)) {
            return Err("loaded registry source differs from its retained release bytes".into());
        }
        Ok(())
    }
}

/// Operation-wide logical retention and I/O envelopes, not a process RSS limit.
/// Each exact release is admitted once per session, including workspace reuse.
#[derive(Debug)]
pub(crate) struct Budget {
    metadata: usize,
    wire: u64,
    payload: usize,
    files: usize,
    metadata_limit: usize,
    source_files: usize,
    source_bytes: usize,
    #[cfg(test)]
    reserves: usize,
    #[cfg(test)]
    fail_reserve: Option<usize>,
    #[cfg(test)]
    peak_metadata: usize,
}
pub(crate) type Allowance = Rc<RefCell<Budget>>;
pub(crate) fn operation_budget() -> Result<Allowance, String> {
    Ok(Rc::new(RefCell::new(Budget::new()?)))
}

#[cfg(all(
    test,
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
impl Budget {
    pub(crate) fn test_observation(&self) -> (usize, usize, usize) {
        (self.metadata, self.peak_metadata, self.reserves)
    }
    pub(crate) fn test_limit(&mut self, bytes: usize) {
        self.metadata_limit = bytes;
    }
    pub(crate) fn test_deny_reservation(&mut self, ordinal: usize) {
        self.fail_reserve = Some(ordinal);
    }
    pub(crate) fn test_reset_peak(&mut self) {
        self.peak_metadata = self.metadata;
    }
}
impl Budget {
    pub(crate) fn new() -> Result<Self, String> {
        let mut result = Self {
            metadata: 0,
            wire: 0,
            payload: 0,
            files: 0,
            metadata_limit: MAX_METADATA,
            source_files: 0,
            source_bytes: 0,
            #[cfg(test)]
            reserves: 0,
            #[cfg(test)]
            fail_reserve: None,
            #[cfg(test)]
            peak_metadata: 0,
        };
        result.metadata(resources::control_bytes())?;
        Ok(result)
    }
    pub(crate) fn metadata(&mut self, bytes: usize) -> Result<(), String> {
        let next = self
            .metadata
            .checked_add(bytes)
            .filter(|n| *n <= self.metadata_limit)
            .ok_or("registry operation metadata exceeds 8 MiB")?;
        self.metadata = next;
        #[cfg(test)]
        {
            self.peak_metadata = self.peak_metadata.max(next);
        }
        Ok(())
    }
    /// Prepay an opaque, bounded legacy helper before it allocates. Retained
    /// capacity remains charged; a conservative temporary upper bound is
    /// released after all of that helper's temporaries have been destroyed.
    pub(crate) fn scoped<T, A, R>(
        &mut self,
        bound: usize,
        action: A,
        retained: R,
    ) -> Result<T, String>
    where
        A: FnOnce() -> Result<T, String>,
        R: FnOnce(&T) -> Result<usize, String>,
    {
        let charge = bound
            .checked_add(resources::scope_control_bytes::<T, A, R>())
            .ok_or("registry helper admission overflow")?;
        self.allocation(charge)?;
        let value = action()?;
        let actual = retained(&value)?;
        if actual > bound {
            return Err("registry helper retained capacity exceeds its pre-admission".into());
        }
        self.metadata -= charge - actual;
        Ok(value)
    }
    pub(crate) fn boxed<T>(&mut self) -> Result<(), String> {
        self.allocation(std::mem::size_of::<T>())
    }
    pub(crate) fn map_insert<K: std::hash::Hash + Eq, V>(
        &mut self,
        map: &mut std::collections::HashMap<K, V>,
        key: K,
        value: V,
    ) -> Result<(), String> {
        if !map.contains_key(&key) && map.len() == map.capacity() {
            let bound = resources::table::<(K, V)>(
                map.len()
                    .checked_add(1)
                    .ok_or("registry map count overflow")?,
            )?;
            self.allocation(bound)?;
            map.try_reserve(1)
                .map_err(|_| "cannot reserve registry export map")?;
            let actual = resources::table_capacity::<(K, V)>(map.capacity())?;
            if actual > bound {
                return Err("registry export map exceeds pre-admission".into());
            }
            self.metadata -= bound - actual;
        }
        map.insert(key, value);
        Ok(())
    }

    pub(crate) fn typed_file(
        &mut self,
        path: &Path,
        maximum: usize,
        kind: &str,
    ) -> Result<Vec<u8>, String> {
        let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
        if !metadata.is_file() || metadata.len() > maximum as u64 {
            return Err(format!(
                "{kind} requires a regular file of at most {maximum} bytes"
            ));
        }
        let length =
            usize::try_from(metadata.len()).map_err(|_| "registry input length overflow")?;
        // Rust 1.99's default read_to_end reserves 32 spare bytes, including
        // before its final EOF read. At growth, old capacity < len + 32;
        // RawVec requests max(2 * old capacity, len + 32, 8). Take bounds len
        // by length + 1, so 2 * (length + 1 + 32) covers even short reads whose
        // capacities are not powers of two. Keep the actual-capacity check.
        let bound = length
            .checked_add(1)
            .and_then(|n| n.checked_add(32))
            .and_then(|n| n.checked_mul(2))
            .ok_or("registry read capacity overflow")?;
        self.scoped(
            bound,
            || runtime::read_typed_regular_file(path, length, kind),
            |bytes| Ok(bytes.capacity()),
        )
    }

    pub(crate) fn uri_path(&mut self, uri: &str) -> Result<PathBuf, String> {
        let bytes = uri
            .strip_prefix("file:///")
            .ok_or("registry source requires file:/// followed by an absolute directory")?
            .len()
            .checked_add(1)
            .filter(|n| *n <= 4090)
            .ok_or("registry URI exceeds 4096 bytes")?;
        self.scoped(bytes, || uri_path(uri), |path| Ok(path.capacity()))
    }

    pub(crate) fn normalized_uri(&mut self, uri: &str) -> Result<String, String> {
        let path = self.uri_path(uri)?;
        let path = path.to_str().ok_or("registry directory must be UTF-8")?;
        let width = path
            .bytes()
            .try_fold(7usize, |sum, byte| {
                sum.checked_add(
                    if byte.is_ascii_alphanumeric()
                        || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~')
                    {
                        1
                    } else {
                        3
                    },
                )
            })
            .filter(|width| *width <= 4096)
            .ok_or("canonical registry URI exceeds 4096 bytes")?;
        let mut result = self.string(width)?;
        result.push_str("file://");
        for byte in path.bytes() {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
                result.push(byte as char);
            } else {
                use std::fmt::Write as _;
                write!(result, "%{byte:02X}").map_err(|_| "cannot encode registry URI")?;
            }
        }
        Ok(result)
    }
    fn allocation(&mut self, bytes: usize) -> Result<(), String> {
        self.metadata(bytes)?;
        #[cfg(test)]
        {
            let ordinal = self.reserves;
            self.reserves += 1;
            if self.fail_reserve == Some(ordinal) {
                return Err("injected registry reservation failure".into());
            }
        }
        Ok(())
    }
    pub(crate) fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), String> {
        let required = values
            .len()
            .checked_add(additional)
            .ok_or("registry capacity overflow")?;
        if required <= values.capacity() {
            return Ok(());
        }
        self.allocation(
            (required - values.capacity())
                .checked_mul(std::mem::size_of::<T>())
                .ok_or("registry capacity overflow")?,
        )?;
        values
            .try_reserve_exact(additional)
            .map_err(|_| "cannot reserve registry metadata")?;
        if values.capacity() != required {
            return Err("registry metadata capacity exceeds admission".into());
        }
        Ok(())
    }
    pub(crate) fn text(&mut self, value: &str) -> Result<String, String> {
        let mut bytes = Vec::new();
        self.reserve(&mut bytes, value.len())?;
        bytes.extend_from_slice(value.as_bytes());
        String::from_utf8(bytes).map_err(|_| "registry text is not UTF-8".into())
    }
    pub(crate) fn digest(&mut self, bytes: &[u8]) -> Result<String, String> {
        self.scoped(64, || Ok(hash(bytes)), |text| Ok(text.capacity()))
    }
    pub(crate) fn identity(
        &mut self,
        source: &str,
        name: &str,
        version: &str,
    ) -> Result<String, String> {
        self.scoped(
            64,
            || Ok(release_id(source, name, version)),
            |text| Ok(text.capacity()),
        )
    }
    pub(crate) fn string(&mut self, length: usize) -> Result<String, String> {
        self.allocation(length)?;
        let mut text = String::new();
        text.try_reserve_exact(length)
            .map_err(|_| "cannot reserve registry text")?;
        if text.capacity() != length {
            return Err("registry text capacity exceeds admission".into());
        }
        Ok(text)
    }
    pub(crate) fn path(&mut self, path: &Path) -> Result<PathBuf, String> {
        let text = path.to_str().ok_or("registry paths must be UTF-8")?;
        Ok(PathBuf::from(self.text(text)?))
    }
    pub(crate) fn canonical_path(&mut self, path: &Path) -> Result<PathBuf, String> {
        // The registry's existing path envelope is 8192 UTF-8 bytes. This is
        // a request bound for the standard path helper, not a libc/RSS limit.
        self.scoped(
            8192,
            || fs::canonicalize(path).map_err(|error| error.to_string()),
            |path| {
                if path.to_str().is_none_or(|text| text.len() > 8192) {
                    return Err("registry path exceeds 8192 UTF-8 bytes".into());
                }
                Ok(path.capacity())
            },
        )
    }
    pub(crate) fn join(&mut self, parent: &Path, child: &str) -> Result<PathBuf, String> {
        if Path::new(child).is_absolute() {
            return Err("registry child path must be relative".into());
        }
        let parent = parent.to_str().ok_or("registry paths must be UTF-8")?;
        let separator = usize::from(!parent.ends_with('/'));
        let length = parent
            .len()
            .checked_add(separator)
            .and_then(|n| n.checked_add(child.len()))
            .ok_or("registry path length overflow")?;
        if length > 8192 {
            return Err("registry path exceeds 8192 bytes".into());
        }
        let mut text = self.string(length)?;
        text.push_str(parent);
        if separator != 0 {
            text.push('/');
        }
        text.push_str(child);
        Ok(PathBuf::from(text))
    }
    fn wire(&mut self, bytes: u64) -> Result<(), String> {
        let next = self
            .wire
            .checked_add(bytes)
            .filter(|n| *n <= MAX_WIRE)
            .ok_or("registry operation archives exceed 64 MiB")?;
        self.wire = next;
        Ok(())
    }
    pub(crate) fn source_limits(&self) -> (usize, usize) {
        (
            1024 - self.source_files,
            8 * 1024 * 1024 - self.source_bytes,
        )
    }
    pub(crate) fn source_usage(&mut self, files: usize, bytes: usize) -> Result<(), String> {
        self.source_files = self
            .source_files
            .checked_add(files)
            .filter(|n| *n <= 1024)
            .ok_or("registry publication exceeds 1024 source visits")?;
        self.source_bytes = self
            .source_bytes
            .checked_add(bytes)
            .filter(|n| *n <= 8 * 1024 * 1024)
            .ok_or("registry publication exceeds 8 MiB of retained source visits")?;
        Ok(())
    }
    pub(crate) fn payload_admission(
        &self,
        count: usize,
        mut lengths: impl Iterator<Item = usize>,
    ) -> Result<(), String> {
        self.files
            .checked_add(count)
            .filter(|n| *n <= MAX_FILES)
            .ok_or("registry operation exceeds 2047 payload files")?;
        lengths
            .try_fold(self.payload, |sum, bytes| sum.checked_add(bytes))
            .filter(|n| *n <= MAX_PAYLOAD)
            .ok_or("registry operation payload exceeds 32 MiB")?;
        Ok(())
    }
    pub(crate) fn payload(&mut self, bytes: usize) -> Result<Vec<u8>, String> {
        let files = self
            .files
            .checked_add(1)
            .filter(|n| *n <= MAX_FILES)
            .ok_or("registry operation exceeds 2047 payload files")?;
        let total = self
            .payload
            .checked_add(bytes)
            .filter(|n| *n <= MAX_PAYLOAD)
            .ok_or("registry operation payload exceeds 32 MiB")?;
        #[cfg(test)]
        {
            let ordinal = self.reserves;
            self.reserves += 1;
            if self.fail_reserve == Some(ordinal) {
                return Err("injected registry reservation failure".into());
            }
        }
        let mut result = Vec::new();
        result
            .try_reserve_exact(bytes)
            .map_err(|_| "cannot reserve registry payload")?;
        if result.capacity() != bytes {
            return Err("registry payload capacity exceeds admission".into());
        }
        self.files = files;
        self.payload = total;
        Ok(result)
    }
}

pub(crate) struct GraphOrigins {
    pub entries: Vec<(usize, Arc<Origin>)>,
    pub allowance: Allowance,
}

pub(crate) struct Publication {
    pub owner: PathBuf,
    pub archive: package::LibraryArchive,
}

pub(crate) struct Session {
    owner: PathBuf,
    no_fetch: bool,
    origins: Vec<Arc<Origin>>,
    trusted: Vec<Pin>,
    budget: Allowance,
}
impl Session {
    pub(crate) fn boxed(self) -> Result<Box<Self>, String> {
        self.budget
            .borrow_mut()
            .allocation(std::mem::size_of::<Self>())?;
        Ok(Box::new(self))
    }
    pub(crate) fn with_budget(
        owner: &Path,
        no_fetch: bool,
        budget: Allowance,
    ) -> Result<Self, String> {
        qualified()?;
        let owner = budget.borrow_mut().path(owner)?;
        let trusted = crate::project_lock::registry_trust(&owner, &mut budget.borrow_mut())?;
        Ok(Self {
            owner,
            no_fetch,
            origins: Vec::new(),
            trusted,
            budget,
        })
    }
    pub(crate) fn graph(
        &mut self,
        packages: &[crate::frontend::PackageSpec],
    ) -> Result<Option<Box<GraphOrigins>>, String> {
        let count = packages
            .iter()
            .filter(|package| {
                self.origins
                    .iter()
                    .any(|origin| origin.root == package.root)
            })
            .count();
        if count == 0 {
            return Ok(None);
        }
        let mut budget = self.budget.borrow_mut();
        budget.allocation(std::mem::size_of::<GraphOrigins>())?;
        let mut entries = Vec::new();
        budget.reserve(&mut entries, count)?;
        for (index, package) in packages.iter().enumerate() {
            if let Some(origin) = self
                .origins
                .iter()
                .find(|origin| origin.root == package.root)
            {
                entries.push((index, Arc::clone(origin)));
            }
        }
        Ok(Some(Box::new(GraphOrigins {
            entries,
            allowance: self.budget.clone(),
        })))
    }
    pub(crate) fn origin(&self, root: &Path) -> Option<Arc<Origin>> {
        self.origins
            .iter()
            .find(|origin| origin.root == root)
            .cloned()
    }
    pub(crate) fn resolve(
        &mut self,
        declaring: &Path,
        dependency: &Dependency,
    ) -> Result<PathBuf, String> {
        let mut budget = self.budget.borrow_mut();
        let source = if dependency.source == "self" {
            budget.text(
                &self
                    .origin(declaring)
                    .ok_or(
                        "registry = self is valid only inside an independently admitted release",
                    )?
                    .pin
                    .source,
            )?
        } else {
            budget.normalized_uri(&dependency.source)?
        };
        let id = budget.identity(&source, &dependency.name, &dependency.version)?;
        if let Some(origin) = self.origins.iter().find(|origin| {
            origin.pin.source == source
                && origin.pin.name == dependency.name
                && origin.pin.version == dependency.version
        }) {
            return budget.path(&origin.root);
        }
        if self.origins.len() >= 256 {
            return Err("registry exceeds the 256-package graph limit".into());
        }
        let parent = budget.join(&self.owner, ".oxid/registry")?;
        let cache = budget.join(&parent, &id)?;
        let existing = match fs::symlink_metadata(&cache) {
            Ok(_) => {
                ordinary(&cache, false)?;
                true
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(format!("cannot inspect registry cache: {error}")),
        };
        if !existing && self.no_fetch {
            return Err("registry release is not cached; locked/offline/frozen commands never fetch, including file registries".into());
        }
        let release = if existing {
            budget.path(&cache)?
        } else {
            let root = budget.uri_path(&source)?;
            ordinary(&root, false)?;
            let marker = budget.join(&root, "registry.toml")?;
            if read_regular(&marker, MARKER.len(), &mut budget)? != MARKER {
                return Err("unsupported private registry marker".into());
            }
            let packages = budget.join(&root, "packages")?;
            let package = budget.join(&packages, &dependency.name)?;
            let release = budget.join(&package, &dependency.version)?;
            ordinary(&release, false)?;
            release
        };
        let record_path = budget.join(&release, "release.toml")?;
        let raw = read_regular(&record_path, 1024 * 1024, &mut budget)?;
        let record = Record::decode(&raw, &mut budget)?;
        if record.name != dependency.name || record.version != dependency.version {
            return Err("registry release name/version differs from the requested identity".into());
        }
        if self
            .trusted
            .iter()
            .find(|pin| {
                pin.source == source && pin.name == record.name && pin.version == record.version
            })
            .is_some_and(|pin| pin.artifact != record.artifact || pin.bytes != record.bytes)
        {
            return Err("immutable registry release differs from the trusted lock; choose a new version instead of updating the same tuple".into());
        }
        budget.wire(record.bytes)?;
        let artifact_path = budget.join(&release, "artifact.tar")?;
        ordinary(&artifact_path, true)?;
        if fs::symlink_metadata(&artifact_path)
            .map_err(|e| e.to_string())?
            .len()
            != record.bytes
        {
            return Err("registry artifact length differs from its release record".into());
        }
        let (archive, info) = package::read_library(&artifact_path, &mut budget)?;
        if info.bytes != record.bytes || info.sha256 != record.artifact {
            return Err(
                "registry artifact does not match its release record SHA-256/length".into(),
            );
        }
        validate_manifest(&archive, &record, &mut budget)?;
        if existing {
            verify_tree(&cache, &archive, &mut budget)?;
        } else {
            for directory in [budget.join(&self.owner, ".oxid")?, budget.path(&parent)?] {
                match fs::symlink_metadata(&directory) {
                    Ok(_) => ordinary(&directory, false)?,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        fs::create_dir(&directory)
                            .map_err(|e| format!("cannot create registry cache: {e}"))?;
                    }
                    Err(error) => return Err(format!("cannot inspect registry cache: {error}")),
                }
            }
            let mut stage = private_stage(&parent, &mut budget)?;
            materialize(&archive, &stage.path, &mut budget)?;
            write(&budget.join(&stage.path, "release.toml")?, &raw)?;
            let mut file = package::new_file(&budget.join(&stage.path, "artifact.tar")?)?;
            let copied = package::write_library(&archive, &mut file, &mut budget)?;
            file.sync_all().map_err(|e| e.to_string())?;
            if copied != info {
                return Err("library artifact changed during staging".into());
            }
            verify_tree(&stage.path, &archive, &mut budget)?;
            package::publish_directory(&stage.path, &cache)?;
            stage.published = true;
            fs::File::open(&parent)
                .and_then(|file| file.sync_all())
                .map_err(|e| format!("registry cache published but directory sync failed: {e}"))?;
        }
        budget.allocation(resources::origin_allocation_bytes())?;
        let mut inputs = Vec::new();
        budget.reserve(&mut inputs, archive.files.len())?;
        for file in archive.files {
            let path = budget.text(
                file.path
                    .strip_prefix("source/")
                    .ok_or("invalid release root")?,
            )?;
            inputs.push(Input {
                path,
                bytes: file.bytes.len(),
                digest: Sha256::digest(&file.bytes).into(),
            });
        }
        let root = budget.join(&cache, "source")?;
        let retained_root = budget.path(&root)?;
        let origin = Arc::new(Origin {
            pin: Pin {
                source,
                name: record.name,
                version: record.version,
                artifact: record.artifact,
                bytes: record.bytes,
            },
            root: retained_root,
            manifest: record.manifest,
            inputs,
        });
        budget.reserve(&mut self.origins, 1)?;
        self.origins.push(origin);
        Ok(root)
    }
}

pub(crate) fn parse_manifest(
    path: &Path,
    raw: &[u8],
    budget: &mut Budget,
) -> Result<runtime::ManifestSnapshot, String> {
    budget.scoped(
        resources::manifest_bound(raw)?,
        || runtime::parse_typed_manifest_bytes_registry(path, raw, &mut None, true),
        resources::manifest_capacity,
    )
}
pub(crate) fn load_manifest(
    path: &Path,
    allowance: &Allowance,
) -> Result<runtime::ManifestSnapshot, String> {
    let mut budget = allowance.borrow_mut();
    let raw = budget.typed_file(path, 1024 * 1024, "typed manifest")?;
    parse_manifest(path, &raw, &mut budget)
}
pub(crate) fn clone_manifest(
    snapshot: &runtime::ManifestSnapshot,
    budget: &mut Budget,
) -> Result<runtime::ManifestSnapshot, String> {
    budget.scoped(
        resources::manifest_clone_capacity(snapshot)?
            .checked_mul(2)
            .ok_or("registry manifest clone storage overflow")?,
        || Ok(snapshot.clone()),
        resources::manifest_clone_capacity,
    )
}
pub(crate) fn publication_replacements(
    entries: &[(String, Dependency)],
    source: &str,
    budget: &mut Budget,
) -> Result<std::collections::HashMap<String, String>, String> {
    budget.scoped(resources::replacement_bound(entries)?, || {
        let mut replacements = std::collections::HashMap::new();
        for (alias, dependency) in entries {
            if normalized_uri(&dependency.source)? != source { return Err("published libraries currently require every dependency in the same registry".into()); }
            let mut exported = dependency.clone(); exported.source = "self".into();
            replacements.insert(alias.clone(), exported.encode());
        }
        Ok(replacements)
    }, resources::string_map_capacity)
}
pub(crate) fn rewrite_manifest(
    path: &Path,
    raw: &[u8],
    replacements: &std::collections::HashMap<String, String>,
    registry: bool,
    budget: &mut Budget,
) -> Result<Vec<u8>, String> {
    budget.scoped(
        resources::rewrite_bound(raw, replacements)?,
        || runtime::rewrite_registry_prepaid(path, raw, replacements, registry),
        |bytes| Ok(bytes.capacity()),
    )
}

fn validate_manifest(
    archive: &package::LibraryArchive,
    record: &Record,
    budget: &mut Budget,
) -> Result<(), String> {
    let raw = &archive
        .files
        .iter()
        .find(|file| file.path == "source/oxid.toml")
        .ok_or("library manifest is missing")?
        .bytes;
    if budget.digest(raw)? != record.manifest {
        return Err("library manifest digest mismatch".into());
    }
    let snapshot = parse_manifest(Path::new("oxid.toml"), raw, budget)?;
    let manifest = &snapshot.manifest;
    if snapshot.workspace.is_some()
        || manifest.configuration_error.is_some()
        || manifest.duplicate_version
        || manifest.name.as_deref() != Some(record.name.as_str())
        || manifest.version.as_deref() != Some(record.version.as_str())
        || manifest.edition.as_deref() != Some("typed-preview")
        || manifest.kind.as_deref() != Some("lib")
        || manifest.entry.as_deref().unwrap_or("src/lib.ox") != record.entry
        || !manifest.features.is_empty()
    {
        return Err("release manifest does not match its immutable typed library metadata".into());
    }
    let registry = manifest
        .registry_dependencies
        .as_deref()
        .map_or(&[][..], |entries| entries.as_slice());
    if registry.len() != manifest.dependencies.len()
        || registry
            .iter()
            .any(|(_, dependency)| dependency.source != "self")
    {
        return Err("published libraries require only same-registry dependencies; external path/Git/registry sources are unsupported".into());
    }
    Ok(())
}

pub(crate) fn publication_manifest(manifest: &runtime::ProjectManifest) -> Result<(), String> {
    name(
        manifest
            .name
            .as_deref()
            .ok_or("publication requires a package name")?,
    )?;
    version(
        manifest
            .version
            .as_deref()
            .ok_or("publication requires an exact stable package version")?,
    )?;
    if manifest.kind.as_deref() != Some("lib")
        || manifest.edition.as_deref() != Some("typed-preview")
        || manifest.duplicate_version
        || !manifest.features.is_empty()
    {
        return Err(
            "publication requires a typed library, one version and no unsupported features".into(),
        );
    }
    if let Some(error) = &manifest.configuration_error {
        return Err(error.clone());
    }
    Ok(())
}

fn verify_tree(
    root: &Path,
    archive: &package::LibraryArchive,
    budget: &mut Budget,
) -> Result<(), String> {
    // Only admitted payloads are read. Extra names, aliases, links and special
    // files fail without altering them. The queue and every owned path are paid.
    let mut pending = Vec::new();
    budget.reserve(&mut pending, 1)?;
    pending.push(budget.path(root)?);
    let mut directories = 0usize;
    let mut files = 0usize;
    while let Some(directory) = pending.pop() {
        directories += 1;
        if directories > 4097 {
            return Err("registry cache exceeds its directory limit".into());
        }
        for entry in fs::read_dir(&directory).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            // Linux VFS limits one directory name to NAME_MAX=255 bytes. Reserve
            // the bound before requesting the owned name, then account the
            // actual returned capacity; unknown excess is rejected before use.
            budget.allocation(255)?;
            let name = entry.file_name();
            if name.capacity() > 255 {
                return Err("registry directory name capacity exceeds admission".into());
            }
            budget.metadata -= 255 - name.capacity();
            let path = budget.join(
                &directory,
                name.to_str().ok_or("registry cache paths must be UTF-8")?,
            )?;
            let relative = path
                .strip_prefix(root)
                .ok()
                .and_then(Path::to_str)
                .ok_or("registry cache path escaped its root")?;
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            if kind.is_dir() {
                if !archive.files.iter().any(|file| {
                    file.path
                        .strip_prefix(relative)
                        .is_some_and(|tail| tail.starts_with('/'))
                }) {
                    return Err("registry cache contains an unexpected directory; preserve local changes before restoring it".into());
                }
                if pending.len() >= 4096 {
                    return Err("registry cache directory queue exceeded".into());
                }
                budget.reserve(&mut pending, 1)?;
                pending.push(path);
            } else if kind.is_file() {
                files += 1;
                if files > MAX_FILES + 2 {
                    return Err("registry cache contains too many files".into());
                }
                // Even metadata entries must remain single-linked ordinary files.
                ordinary(&path, true)?;
                if matches!(relative, "artifact.tar" | "release.toml") {
                    continue;
                }
                let expected = archive.files.iter().find(|file| file.path == relative).ok_or("registry cache contains an unexpected file; preserve local changes before restoring it")?;
                compare_regular(&path, &expected.bytes)?;
            } else {
                return Err("registry cache contains a link or special file".into());
            }
        }
    }
    if files != archive.files.len() + 2 {
        return Err("registry cache is missing release inputs".into());
    }
    Ok(())
}

fn compare_regular(path: &Path, expected: &[u8]) -> Result<(), String> {
    let (mut file, before) = open_regular(path, expected.len())?;
    if before.len() != expected.len() as u64 {
        return Err(
            "registry cache was modified; preserve local changes before restoring it".into(),
        );
    }
    let mut scratch = [0; 8192];
    for chunk in expected.chunks(scratch.len()) {
        file.read_exact(&mut scratch[..chunk.len()])
            .map_err(|error| error.to_string())?;
        if scratch[..chunk.len()] != *chunk {
            return Err(
                "registry cache was modified; preserve local changes before restoring it".into(),
            );
        }
    }
    if file.read(&mut [0; 1]).map_err(|error| error.to_string())? != 0 {
        return Err("registry cache grew during verification".into());
    }
    recheck(path, &file, &before)
}

fn qualified() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("private registry publication and materialization are currently qualified only on Linux".into());
    }
    Ok(())
}

fn private_stage(parent: &Path, budget: &mut Budget) -> Result<package::PrivateDirectory, String> {
    use std::{
        fmt::Write as _,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    for _ in 0..32 {
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let digits = |n: u128| if n == 0 { 1 } else { n.ilog10() as usize + 1 };
        let length = ".oxid-registry---".len()
            + digits(u128::from(pid))
            + digits(stamp)
            + digits(u128::from(number));
        let mut name = budget.string(length)?;
        write!(name, ".oxid-registry-{pid}-{stamp}-{number}")
            .map_err(|_| "cannot name private registry storage")?;
        if name.len() != length || name.capacity() != length {
            return Err("registry stage name exceeded admission".into());
        }
        let path = budget.join(parent, &name)?;
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&path) {
            Ok(()) => {
                return Ok(package::PrivateDirectory {
                    path,
                    published: false,
                })
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("cannot create private registry storage: {error}")),
        }
    }
    Err("cannot allocate fresh private registry storage".into())
}

fn materialize(
    archive: &package::LibraryArchive,
    stage: &Path,
    budget: &mut Budget,
) -> Result<(), String> {
    let mut directories = Vec::new();
    // Complete directory admission precedes the first payload write.
    for member in &archive.files {
        let relative = Path::new(&member.path);
        if relative.components().count() > 32 {
            return Err("library source path exceeds 32 components".into());
        }
        for parent in relative
            .ancestors()
            .skip(1)
            .filter(|path| !path.as_os_str().is_empty())
        {
            let directory =
                budget.join(stage, parent.to_str().ok_or("library path is not UTF-8")?)?;
            if !directories.contains(&directory) {
                if directories.len() >= 4096 {
                    return Err("library source exceeds 4096 implicit directories".into());
                }
                budget.reserve(&mut directories, 1)?;
                directories.push(directory);
            }
        }
    }
    directories.sort_unstable();
    for directory in &directories {
        fs::create_dir(directory)
            .map_err(|error| format!("cannot materialize library directory: {error}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory, fs::Permissions::from_mode(0o755))
                .map_err(|error| error.to_string())?;
        }
    }
    for member in &archive.files {
        write(&budget.join(stage, &member.path)?, &member.bytes)?;
    }
    for directory in directories.iter().rev() {
        fs::File::open(directory)
            .and_then(|file| file.sync_all())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = package::new_file(path)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("cannot stage registry input: {e}"))
}

pub(crate) fn dispatch(args: &[String]) -> Option<i32> {
    if !matches!(
        args.first().map(String::as_str),
        Some("registry" | "publish")
    ) {
        return None;
    }
    let result = command(args);
    let (status, text) = match result {
        Ok(text) => (0, text),
        Err(error) => (1, format!("error: {error}\n")),
    };
    let written = if status == 0 {
        io::stdout().lock().write_all(text.as_bytes())
    } else {
        io::stderr().lock().write_all(text.as_bytes())
    };
    Some(if written.is_ok() { status } else { 74 })
}

fn command(args: &[String]) -> Result<String, String> {
    qualified()?;
    if args.first().is_some_and(|word| word == "registry") {
        if args.len() != 3 || args[1] != "init" || args[2].starts_with('-') {
            return Err("use oxid registry init NEW_DIRECTORY".into());
        }
        let mut budget = Budget::new()?;
        if args[2].len() > 8192 {
            return Err("registry path exceeds 8192 bytes".into());
        }
        let path = if Path::new(&args[2]).is_absolute() {
            budget.path(Path::new(&args[2]))?
        } else {
            let cwd = budget.scoped(
                8192,
                || std::env::current_dir().map_err(|error| error.to_string()),
                |path| Ok(path.capacity()),
            )?;
            budget.join(&cwd, &args[2])?
        };
        let parent =
            budget.canonical_path(path.parent().ok_or("registry directory has no parent")?)?;
        ordinary(&parent, false)?;
        let destination = budget.join(
            &parent,
            path.file_name()
                .and_then(|name| name.to_str())
                .ok_or("registry directory requires a UTF-8 name")?,
        )?;
        let mut stage = private_stage(&parent, &mut budget)?;
        write(&budget.join(&stage.path, "registry.toml")?, MARKER)?;
        fs::create_dir(budget.join(&stage.path, "packages")?).map_err(|error| error.to_string())?;
        let destination_text = destination.to_str().ok_or("registry path must be UTF-8")?;
        let mut message =
            budget.string("created private registry \n".len() + destination_text.len())?;
        message.push_str("created private registry ");
        message.push_str(destination_text);
        message.push('\n');
        package::publish_directory(&stage.path, &destination)?;
        stage.published = true;
        fs::File::open(&parent)
            .and_then(|file| file.sync_all())
            .map_err(|error| format!("registry created but parent sync failed: {error}"))?;
        return Ok(message);
    }
    let mut source = None;
    let mut selection = None;
    let mut iter = args.iter().skip(1);
    while let Some(flag) = iter.next() {
        let value = iter
            .next()
            .filter(|word| !word.starts_with('-'))
            .ok_or("publish option requires a value")?;
        match flag.as_str() {
            "--registry" if source.is_none() => source = Some(value.as_str()),
            "--package" | "-p" if selection.is_none() => selection = Some(value.as_str()),
            _ => {
                return Err(
                    "publish accepts one --registry URI and an optional --package MEMBER".into(),
                )
            }
        }
    }
    let allowance = operation_budget()?;
    let source = allowance
        .borrow_mut()
        .normalized_uri(source.ok_or("publish requires --registry file:///DIRECTORY")?)?;
    let registry = allowance.borrow_mut().uri_path(&source)?;
    ordinary(&registry, false)?;
    {
        let mut budget = allowance.borrow_mut();
        let marker = budget.join(&registry, "registry.toml")?;
        if read_regular(&marker, MARKER.len(), &mut budget)? != MARKER {
            return Err("unsupported private registry".into());
        }
    }
    let publication =
        crate::project_cli::registry_release_plan(&source, selection, allowance.clone())?;
    let raw = &publication
        .archive
        .files
        .iter()
        .find(|file| file.path == "source/oxid.toml")
        .ok_or("missing publication manifest")?
        .bytes;
    let mut budget = allowance.borrow_mut();
    let snapshot = parse_manifest(Path::new("oxid.toml"), raw, &mut budget)?;
    let manifest = &snapshot.manifest;
    let mut record = Record {
        name: budget.text(manifest.name.as_deref().ok_or("library name is required")?)?,
        version: budget.text(
            manifest
                .version
                .as_deref()
                .ok_or("library version is required")?,
        )?,
        entry: budget.text(manifest.entry.as_deref().unwrap_or("src/lib.ox"))?,
        manifest: budget.digest(raw)?,
        artifact: String::new(),
        bytes: 0,
    };
    let info = package::write_library(&publication.archive, &mut io::sink(), &mut budget)?;
    record.artifact = budget.text(&info.sha256)?;
    record.bytes = info.bytes;
    validate_manifest(&publication.archive, &record, &mut budget)?;
    let packages = budget.join(&registry, "packages")?;
    ordinary(&packages, false)?;
    let parent = budget.join(&packages, &record.name)?;
    match fs::symlink_metadata(&parent) {
        Ok(_) => ordinary(&parent, false)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(&parent).map_err(|error| error.to_string())?
        }
        Err(error) => return Err(error.to_string()),
    }
    let destination = budget.join(&parent, &record.version)?;
    match fs::symlink_metadata(&destination) {
        Ok(_) => return Err("registry versions are immutable; publish a new version".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect release destination: {error}")),
    }
    let mut stage = private_stage(&parent, &mut budget)?;
    let encoded_record = record.encode(&mut budget)?;
    write(
        &budget.join(&stage.path, "release.toml")?,
        encoded_record.as_bytes(),
    )?;
    let artifact = budget.join(&stage.path, "artifact.tar")?;
    let mut output = package::new_file(&artifact)?;
    let actual = package::write_library(&publication.archive, &mut output, &mut budget)?;
    output.sync_all().map_err(|error| error.to_string())?;
    drop(output);
    let decoded = package::read_library(&artifact, &mut budget)?;
    if actual != info || decoded.0 != publication.archive || decoded.1 != info {
        return Err("library publication changed during staging".into());
    }
    drop(decoded);
    let verification = private_stage(&parent, &mut budget)?;
    materialize(&publication.archive, &verification.path, &mut budget)?;
    let checked_root = budget.join(&verification.path, "source")?;
    drop(budget);
    crate::project_cli::registry_recheck_release(
        &source,
        &checked_root,
        &publication,
        allowance.clone(),
    )?;
    drop(verification);
    let mut message = allowance.borrow_mut().string(
        "published ".len()
            + record.name.len()
            + 1
            + record.version.len()
            + " to ".len()
            + source.len()
            + " (sha256:".len()
            + record.artifact.len()
            + 2,
    )?;
    {
        use std::fmt::Write as _;
        writeln!(
            message,
            "published {} {} to {} (sha256:{})",
            record.name, record.version, source, record.artifact
        )
        .map_err(|_| "cannot render registry publication result")?;
    }
    package::publish_directory(&stage.path, &destination)?;
    stage.published = true;
    fs::File::open(&parent)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("release published but parent sync failed: {error}"))?;
    Ok(message)
}
