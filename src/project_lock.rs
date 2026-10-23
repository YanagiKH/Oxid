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
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Edge {
    owner: String,
    alias: String,
    package: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Snapshot {
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

fn relative(base: &Path, target: &Path) -> Result<String, String> {
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
            if !valid_hash(&package.id)
                || package.id != package_id(&package.path)
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
            let mut text = format!(
                "version = 3\nkind = \"typed-workspace-v1\"\nmanifest = {}\n",
                quote(manifest)
            );
            for member in &self.members {
                text.push_str(&format!("\n[[member]]\npackage = {}\n", quote(member)));
            }
            text
        } else {
            format!(
                "version = 2\nkind = \"typed-local-v1\"\nroot = {}\n",
                quote(&self.root)
            )
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
        let workspace =
            version == Some("version = 3") && kind == Some("kind = \"typed-workspace-v1\"");
        if !workspace
            && !(version == Some("version = 2") && kind == Some("kind = \"typed-local-v1\""))
        {
            return Err("unsupported lock format: expected typed-local-v1 version 2 or typed-workspace-v1 version 3; old or unknown locks are never replaced".into());
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
            packages.push(Package {
                id: field(&mut lines, "id")?,
                path: field(&mut lines, "path")?,
                name: field(&mut lines, "name")?,
                version: field(&mut lines, "version")?,
                edition: field(&mut lines, "edition")?,
                entry: field(&mut lines, "entry")?,
                manifest: field(&mut lines, "manifest")?,
                sources: field(&mut lines, "sources")?,
            });
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
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("cannot inspect workspace lock ownership: {error}"))?;
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Ok(false);
    }
    let mut prefix = Vec::new();
    file.take(128)
        .read_to_end(&mut prefix)
        .map_err(|error| format!("cannot inspect workspace lock ownership: {error}"))?;
    let text = String::from_utf8_lossy(&prefix);
    let mut lines = text.lines();
    Ok(
        lines.next() == Some("version = 3")
            && lines.next() == Some("kind = \"typed-workspace-v1\""),
    )
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
        if existing.snapshot != *expected {
            return Err("typed lock is out of date: manifests, dependency sources, or graph changed; review changes and run `oxid lock --update`".into());
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
