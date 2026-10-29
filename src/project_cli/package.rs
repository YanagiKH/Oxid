//! Source-package collection admits one complete lock owner and copies only
//! bytes already retained by manifest, lock and compiler source owners.
use super::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const MANIFEST_BYTES: usize = 8 * 1024 * 1024;
const SOURCE_BYTES: usize = 8 * 1024 * 1024;
const SOURCE_FILES: usize = 1024;
const PAYLOAD_BYTES: usize = 32 * 1024 * 1024;
// The format's 2048-entry cap also includes its generated inventory.
const PAYLOAD_FILES: usize = 2047;

struct Reader {
    boundary: Option<PathBuf>,
    remaining: usize,
    raw: HashMap<PathBuf, Arc<[u8]>>,
    parsed: HashMap<PathBuf, runtime::ManifestSnapshot>,
    graph_nodes_left: usize,
    graph_edges_left: usize,
    git: Option<RegularProjectReader>,
    cache_roots: BTreeSet<PathBuf>,
    registry_budget: Option<crate::project_registry::Allowance>,
}

fn component(value: &str) -> Result<(), String> {
    let stem = value.split('.').next().unwrap_or("").to_ascii_uppercase();
    if value.is_empty()
        || value.len() > 255
        || matches!(value, "." | "..")
        || value.ends_with(['.', ' '])
        || value.chars().any(char::is_control)
        || value.contains(['\\', ':', '*', '?', '<', '>', '"', '|'])
        || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err("source package paths require portable normal components".into());
    }
    Ok(())
}

fn relative_path(value: &str, parents: bool) -> Result<(), String> {
    if value.is_empty() || value.len() > 4096 || Path::new(value).is_absolute() {
        return Err(
            "source package paths must be bounded relative paths, not absolute paths".into(),
        );
    }
    let mut normal = false;
    for part in value.split('/') {
        if part == ".." && parents && !normal {
            continue;
        }
        component(part)?;
        normal = true;
    }
    if !normal {
        return Err(
            "source package dependency paths need a normal directory after leading ../ components"
                .into(),
        );
    }
    Ok(())
}

fn ordinary_path(path: &Path, file: bool, boundary: Option<&Path>) -> Result<(), String> {
    if boundary.is_some_and(|boundary| !path.starts_with(boundary)) {
        return Err("source package path escapes the private staging boundary".into());
    }
    if !path.is_absolute() {
        return Err("source package admission requires an absolute location".into());
    }
    let mut current = PathBuf::new();
    for part in path.components() {
        match part {
            Component::RootDir | Component::Prefix(_) => current.push(part.as_os_str()),
            Component::Normal(_) => current.push(part.as_os_str()),
            _ => return Err("source package paths must be normalized".into()),
        }
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            format!(
                "cannot inspect source package path {}: {error}",
                current.display()
            )
        })?;
        let expected_file = file && current == path;
        if !(if expected_file {
            metadata.file_type().is_file()
        } else {
            metadata.file_type().is_dir()
        }) {
            return Err(format!("source package inputs require ordinary {} without symbolic links or special files: {}", if expected_file { "files" } else { "directories" }, current.display()));
        }
    }
    Ok(())
}

fn registry_ordinary_path(
    path: &Path,
    budget: &mut crate::project_registry::Budget,
) -> Result<(), String> {
    let bound = path
        .as_os_str()
        .len()
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(8))
        .ok_or("registry collector path storage overflow")?;
    // One temporary PathBuf grows while checking the path. The scoped request
    // covers its growth and the actual closure/return types; nothing is retained.
    budget.scoped(bound, || ordinary_path(path, false, None), |_| Ok(0))
}

impl Reader {
    fn raw(&mut self, path: &Path) -> Result<Arc<[u8]>, String> {
        if let Some(bytes) = self.raw.get(path) {
            return Ok(bytes.clone());
        }
        ordinary_path(path, true, self.boundary.as_deref())?;
        if self.raw.len() >= 320 {
            return Err("source package exceeds the manifest/discovery count limit".into());
        }
        let bytes =
            runtime::read_typed_manifest_bytes(path, &mut self.remaining).map_err(|error| {
                format!("source package manifest admission (8 MiB aggregate limit): {error}")
            })?;
        self.raw.insert(path.to_owned(), bytes.clone());
        Ok(bytes)
    }

    /// Raw ownership belongs only to this bounded collector. Cached parse and
    /// digest identity bind a snapshot to its first retained read, even after
    /// the original file is changed or removed. Never reopen a cache miss.
    fn retained_manifest(
        &self,
        path: &Path,
        snapshot: &runtime::ManifestSnapshot,
    ) -> Result<&[u8], String> {
        let parsed = self
            .parsed
            .get(path)
            .ok_or("source package manifest was not admitted by this reader")?;
        if parsed.digest != snapshot.digest {
            return Err(
                "source package manifest snapshot differs from its retained reader identity".into(),
            );
        }
        self.raw
            .get(path)
            .map(AsRef::as_ref)
            .ok_or_else(|| "source package retained manifest bytes are missing".into())
    }

    fn source(&self, root: &Path, value: &str) -> Result<PathBuf, String> {
        relative_path(value, false)?;
        let path = root.join(value);
        ordinary_path(&path, true, self.boundary.as_deref())?;
        Ok(path)
    }
}

impl ProjectReader for Reader {
    fn registry_manifest(&mut self, snapshot: &runtime::ManifestSnapshot) -> Result<(), String> {
        if let Some(allowance) = snapshot
            .manifest
            .registry_dependencies
            .as_ref()
            .and_then(|entries| entries.allowance.as_ref())
        {
            if self
                .registry_budget
                .as_ref()
                .is_some_and(|old| !std::rc::Rc::ptr_eq(old, allowance))
            {
                return Err("source bundle registry manifest allowance changed".into());
            }
            self.registry_budget = Some(allowance.clone());
        }
        if let Some(inner) = &mut self.git {
            inner.registry_budget = self.registry_budget.clone();
            inner.registry_manifest(snapshot)?;
        }
        Ok(())
    }
    fn registry_allowance(&self) -> Option<crate::project_registry::Allowance> {
        self.registry_budget.clone()
    }
    fn registry_dependency(
        &mut self,
        root: &Path,
        dependency: &crate::project_registry::Dependency,
    ) -> Result<PathBuf, String> {
        if self.boundary.is_some() {
            return Err("source package payloads require portable local dependencies".into());
        }
        let inner = self
            .git
            .as_mut()
            .ok_or("source package resolver is missing")?;
        inner.registry_budget = self.registry_budget.clone();
        let path = inner.registry_dependency(root, dependency)?;
        self.registry_budget = inner.registry_budget.clone();
        let allowance = self
            .registry_budget
            .as_ref()
            .ok_or("registry allowance is missing")?;
        registry_ordinary_path(&path, &mut allowance.borrow_mut())?;
        // The already retained GraphOrigins supplies this cache parent when
        // constructing the final collision roster. No second tree owns it here.
        Ok(path)
    }
    fn registry_origin(
        &self,
        root: &Path,
    ) -> Option<std::sync::Arc<crate::project_registry::Origin>> {
        self.git
            .as_ref()
            .and_then(|reader| reader.registry_origin(root))
    }
    fn registry_graph(
        &mut self,
        packages: &[frontend::PackageSpec],
    ) -> Result<Option<Box<crate::project_registry::GraphOrigins>>, String> {
        match self.git.as_mut() {
            Some(reader) => reader.registry_graph(packages),
            None => Ok(None),
        }
    }
    fn package(&mut self, edges: usize) -> Result<(), String> {
        let nodes = self.graph_nodes_left.checked_sub(1);
        let edges = self.graph_edges_left.checked_sub(edges);
        let (Some(nodes), Some(edges)) = (nodes, edges) else {
            return Err(
                "source package exceeds the aggregate 256-package or 1024-edge session limit"
                    .into(),
            );
        };
        self.graph_nodes_left = nodes;
        self.graph_edges_left = edges;
        Ok(())
    }

    fn manifest(&mut self, path: &Path) -> Result<runtime::ManifestSnapshot, String> {
        if let Some(snapshot) = self.parsed.get(path) {
            return match &self.registry_budget {
                Some(budget) => {
                    crate::project_registry::clone_manifest(snapshot, &mut budget.borrow_mut())
                }
                None => Ok(snapshot.clone()),
            };
        }
        let bytes = self.raw(path)?;
        let snapshot = if let Some(budget) = &self.registry_budget {
            crate::project_registry::parse_manifest(path, &bytes, &mut budget.borrow_mut())?
        } else {
            runtime::parse_typed_manifest_bytes_registry(
                path,
                &bytes,
                &mut self.registry_budget,
                false,
            )?
        };
        if let Some(entry) = &snapshot.manifest.entry {
            relative_path(entry, false)?;
        }
        for path in snapshot.manifest.tests.values() {
            relative_path(path, false)?;
        }
        // Reject every portability violation before traversing the first edge.
        for (alias, path) in &snapshot.manifest.dependencies {
            let registry = snapshot
                .manifest
                .registry_dependencies
                .as_deref()
                .is_some_and(|entries| entries.iter().any(|(name, _)| name == alias));
            if !(self.boundary.is_none() && (git_source(path) || registry)) {
                relative_path(path, true)?;
            }
        }
        if let Some(allowance) = &self.registry_budget {
            let mut budget = allowance.borrow_mut();
            let retained = crate::project_registry::clone_manifest(&snapshot, &mut budget)?;
            let key = budget.path(path)?;
            budget.map_insert(&mut self.parsed, key, retained)?;
        } else {
            self.parsed.insert(path.to_owned(), snapshot.clone());
        }
        Ok(snapshot)
    }

    fn dependency(&mut self, root: &Path, source: &str) -> Result<PathBuf, String> {
        if git_source(source) {
            if self.boundary.is_some() {
                return Err("source package payloads require portable local dependencies".into());
            }
            let path = self
                .git
                .as_mut()
                .ok_or("source package Git session is missing")?
                .dependency(root, source)?;
            ordinary_path(&path, false, None)?;
            self.cache_roots.insert(path.clone());
            return Ok(path);
        }
        relative_path(source, true)?;
        let mut path = root.to_owned();
        for part in source.split('/') {
            if part == ".." {
                if !path.pop() {
                    return Err("source package dependency escapes its filesystem root".into());
                }
            } else {
                path.push(part);
            }
        }
        ordinary_path(&path, false, self.boundary.as_deref())?;
        Ok(path)
    }

    fn finish_graph(
        &self,
        packages: &[frontend::PackageSpec],
        manifests: &[runtime::ManifestSnapshot],
    ) -> Result<(), String> {
        if let Some(git) = &self.git {
            git.finish_graph(packages, manifests)?;
        }
        Ok(())
    }
}

fn git_source(value: &str) -> bool {
    value.starts_with("git+") || value.starts_with("https://")
}

struct Roster {
    files: BTreeMap<PathBuf, Vec<u8>>,
    bytes: usize,
    source_files_left: usize,
    source_bytes_left: usize,
}
impl Roster {
    fn replace(&mut self, path: &Path, previous: &[u8], bytes: Vec<u8>) -> Result<(), String> {
        if self.files.get(path).map(Vec::as_slice) != Some(previous) {
            return Err("exported source package input differs from its retained owner".into());
        }
        let next = self
            .bytes
            .checked_sub(previous.len())
            .and_then(|count| count.checked_add(bytes.len()))
            .ok_or("exported source package size overflow")?;
        if next > PAYLOAD_BYTES {
            return Err("source package exceeds the 32 MiB payload limit".into());
        }
        self.files.insert(path.to_owned(), bytes);
        self.bytes = next;
        Ok(())
    }
    fn insert(&mut self, path: PathBuf, bytes: &[u8]) -> Result<(), String> {
        if let Some(previous) = self.files.get(&path) {
            if previous != bytes {
                return Err(format!(
                    "source package input changed between retained sessions: {}",
                    path.display()
                ));
            }
            return Ok(());
        }
        let next = self
            .bytes
            .checked_add(bytes.len())
            .ok_or("source package payload size overflow")?;
        if next > PAYLOAD_BYTES || self.files.len() >= PAYLOAD_FILES {
            return Err("source package exceeds the 32 MiB payload or 2047-file limit".into());
        }
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(bytes.len())
            .map_err(|_| "cannot reserve source package payload")?;
        retained.extend_from_slice(bytes);
        self.files.insert(path, retained);
        self.bytes = next;
        Ok(())
    }

    fn collect(
        &mut self,
        session: &frontend::PreparedPackageProject<'_>,
        graph: &PackageGraph,
    ) -> Result<(), String> {
        let (files, bytes) = session.input_usage();
        self.source_files_left = self
            .source_files_left
            .checked_sub(files)
            .ok_or("source package exceeds the aggregate 1024 retained source-file limit")?;
        self.source_bytes_left = self
            .source_bytes_left
            .checked_sub(bytes)
            .ok_or("source package exceeds the aggregate 8 MiB retained source limit")?;
        session.visit_package_sources(&graph.specs[0], |package, relative, bytes| {
            relative_path(relative, false)?;
            let root = graph
                .specs
                .get(package)
                .ok_or("invalid retained source package identity")?;
            self.insert(root.root.join(relative), bytes)
        })
    }
}

fn graph_budget(graphs: &[PackageGraph]) -> Result<(), String> {
    let nodes: usize = graphs.iter().map(|graph| graph.specs.len()).sum();
    let edges: usize = graphs
        .iter()
        .flat_map(|graph| &graph.specs)
        .map(|spec| spec.dependencies.len())
        .sum();
    if nodes > 256 || edges > 1024 {
        return Err(
            "source package exceeds the aggregate 256-package or 1024-edge session limit".into(),
        );
    }
    Ok(())
}

/// Re-root an already admitted graph for a dependency's declared tests. Only
/// reachable package identities remain; no manifest or resolver runs again.
fn reroot(
    graph: &PackageGraph,
    root: usize,
    reader: &mut dyn ProjectReader,
) -> Result<PackageGraph, String> {
    fn visit(graph: &PackageGraph, index: usize, order: &mut Vec<usize>) {
        if order.contains(&index) {
            return;
        }
        order.push(index);
        for edge in &graph.specs[index].dependencies {
            visit(graph, edge.package, order);
        }
    }
    let mut order = Vec::new();
    visit(graph, root, &mut order);
    let mut specs = Vec::with_capacity(order.len());
    let mut manifests = Vec::with_capacity(order.len());
    for &index in &order {
        let mut spec = graph.specs[index].clone();
        for edge in &mut spec.dependencies {
            edge.package = order
                .iter()
                .position(|&old| old == edge.package)
                .expect("reachable edge");
        }
        specs.push(spec);
        manifests.push(graph.manifests[index].clone());
    }
    let registries = reader.registry_graph(&specs)?;
    Ok(PackageGraph {
        specs,
        manifests,
        registries,
    })
}

/// Admission and source retention only. The caller materializes this plan in a
/// private boundary and compares a second admitted plan before publication.
pub(crate) fn package_source_plan(
    start: &Path,
    boundary: Option<&Path>,
    require_workspace: bool,
) -> Result<crate::project_package::SourcePlan, String> {
    if !cfg!(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        return Err("typed source package creation requires Linux x86_64 or ARM64".into());
    }
    let cwd = fs::canonicalize(start)
        .map_err(|error| format!("cannot locate source package start: {error}"))?;
    let boundary = boundary
        .map(fs::canonicalize)
        .transpose()
        .map_err(|error| format!("cannot locate source package staging boundary: {error}"))?;
    ordinary_path(&cwd, false, boundary.as_deref())?;
    let mut reader = Reader {
        boundary,
        remaining: MANIFEST_BYTES,
        raw: HashMap::new(),
        parsed: HashMap::new(),
        graph_nodes_left: 256,
        graph_edges_left: 1024,
        git: None,
        cache_roots: BTreeSet::new(),
        registry_budget: None,
    };
    let mut workspace_root = None;
    let mut nearest = None;
    let mut discovery_locks = BTreeSet::new();
    for (depth, directory) in cwd.ancestors().enumerate() {
        if reader
            .boundary
            .as_deref()
            .is_some_and(|boundary| !directory.starts_with(boundary))
        {
            break;
        }
        if depth > 64 {
            return Err("source package discovery exceeds 64 ancestor directories".into());
        }
        let path = directory.join("oxid.toml");
        let declaration = match fs::symlink_metadata(&path) {
            Ok(_) => {
                if nearest.is_none() {
                    nearest = Some(directory.to_owned());
                }
                runtime::workspace_declaration_bytes(&reader.raw(&path)?)?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(format!("cannot discover source package manifest: {error}")),
        };
        if declaration.is_some() {
            if workspace_root.is_some() {
                return Err("nested workspaces are unsupported".into());
            }
            workspace_root = Some(directory.to_owned());
        } else {
            let lock = directory.join("oxid.lock");
            if fs::symlink_metadata(&lock).is_ok_and(|metadata| metadata.file_type().is_file()) {
                discovery_locks.insert(lock);
            }
            if project_lock::has_workspace_marker(directory)? {
                return Err(
                    "workspace lock requires its workspace declaration before source packaging"
                        .into(),
                );
            }
        }
    }
    if require_workspace && workspace_root.is_none() {
        return Err("source package --workspace requires an enclosing typed workspace".into());
    }
    let is_workspace = workspace_root.is_some();
    let owner = workspace_root
        .or(nearest)
        .ok_or("source packaging requires an oxid.toml project or workspace")?;
    let owner_snapshot = reader.manifest(&owner.join("oxid.toml"))?;
    let members = if is_workspace {
        let workspace =
            workspace::load_roster(owner.clone(), &cwd, owner_snapshot.clone(), &mut reader)?;
        workspace
            .source_members()
            .map(|(root, snapshot)| (root.to_owned(), snapshot.clone()))
            .collect::<Vec<_>>()
    } else {
        if owner_snapshot.manifest.edition.as_deref() != Some("typed-preview") {
            return Err("source packaging requires explicit edition = typed-preview".into());
        }
        vec![(owner.clone(), owner_snapshot.clone())]
    };
    let existing =
        project_lock::read_with_registry(&owner, true, is_workspace, &mut reader.registry_budget)?
            .ok_or("source packaging requires an existing typed lock")?;
    let mut graphs = Vec::new();
    reader.git = Some(RegularProjectReader::new(owner.clone(), true, true));
    reader.git.as_mut().expect("package reader").registry_budget = reader.registry_budget.clone();
    for (root, snapshot) in &members {
        graphs.push(package_graph_with(root, snapshot, &mut reader)?);
        graph_budget(&graphs)?;
    }
    // The workspace binary/library rule remains authoritative for every graph.
    for graph in &graphs {
        for package in graph.specs.iter().skip(1) {
            if members.iter().any(|(root, snapshot)| {
                root == &package.root && snapshot.manifest.kind.as_deref() != Some("lib")
            }) {
                return Err("binary workspace members cannot be imported as library dependencies; declare kind = lib".into());
            }
        }
    }
    let mut packages: BTreeMap<PathBuf, (usize, usize)> = BTreeMap::new();
    for (graph_index, graph) in graphs.iter().enumerate() {
        for (index, spec) in graph.specs.iter().enumerate() {
            let relative = spec
                .entry
                .strip_prefix(&spec.root)
                .ok()
                .and_then(Path::to_str)
                .ok_or("source package entry escapes its package")?;
            reader.source(&spec.root, relative)?;
            if let Some(&(old_graph, old_index)) = packages.get(&spec.root) {
                let previous: &PackageGraph = &graphs[old_graph];
                if previous.specs[old_index].entry != spec.entry
                    || previous.manifests[old_index].digest != graph.manifests[index].digest
                {
                    return Err(
                        "shared package metadata differs across source package sessions".into(),
                    );
                }
            } else {
                packages.insert(spec.root.clone(), (graph_index, index));
            }
        }
    }
    let mut tests = 0usize;
    for &(slot, index) in packages.values() {
        let manifest = &graphs[slot].manifests[index].manifest;
        tests = tests
            .checked_add(manifest.tests.len())
            .ok_or("source package test count overflow")?;
        if tests > 256 {
            return Err("source package exceeds the 256 declared-test limit".into());
        }
        for name in manifest.tests.keys() {
            if name.is_empty() || name.len() > 255 || name.chars().any(char::is_control) {
                return Err(
                    "source package test names must be bounded nonempty text without controls"
                        .into(),
                );
            }
        }
    }
    let mut roster = Roster {
        files: BTreeMap::new(),
        bytes: 0,
        source_files_left: SOURCE_FILES,
        source_bytes_left: SOURCE_BYTES,
    };
    roster.insert(
        owner.join("oxid.toml"),
        reader.retained_manifest(&owner.join("oxid.toml"), &owner_snapshot)?,
    )?;
    roster.insert(owner.join("oxid.lock"), existing.bytes())?;
    for (root, &(slot, index)) in &packages {
        roster.insert(
            root.join("oxid.toml"),
            reader.retained_manifest(&root.join("oxid.toml"), &graphs[slot].manifests[index])?,
        )?;
    }
    let mut fingerprints = Vec::new();
    if let Some(allowance) = &reader.registry_budget {
        allowance
            .borrow_mut()
            .reserve(&mut fingerprints, graphs.len())?;
    }
    for graph in &graphs {
        let session = frontend::prepare_package_project_bounded(
            &graph.specs,
            None,
            roster.source_files_left,
            roster.source_bytes_left,
        )
        .map_err(|error| error.message())?;
        let manifest = &graph.manifests[0].manifest;
        let entry = if manifest.kind.as_deref() == Some("lib") {
            None
        } else {
            Some(manifest.entry_mode.as_deref().unwrap_or("process") == "process")
        };
        graph.verify_registries(&session)?;
        session.source_package_preflight(entry)?;
        fingerprints.push(if let Some(allowance) = &reader.registry_budget {
            registry::publication_fingerprints(&session, graph, allowance)?
        } else {
            session.workspace_fingerprints(&graph.specs[0])?
        });
        roster.collect(&session, graph)?;
    }
    let expected = if is_workspace {
        if let Some(allowance) = &reader.registry_budget {
            let mut inputs = Vec::new();
            allowance.borrow_mut().reserve(&mut inputs, graphs.len())?;
            for (graph, fingerprints) in graphs.iter().zip(&fingerprints) {
                inputs.push(project_lock::WorkspaceInputs {
                    graph: &graph.specs,
                    manifests: &graph.manifests,
                    fingerprints,
                });
            }
            project_lock::expected_registry_workspace(
                &owner,
                &owner_snapshot.digest,
                &inputs,
                &mut allowance.borrow_mut(),
            )?
        } else {
            let inputs = graphs
                .iter()
                .zip(&fingerprints)
                .map(|(graph, fingerprints)| project_lock::WorkspaceInputs {
                    graph: &graph.specs,
                    manifests: &graph.manifests,
                    fingerprints,
                })
                .collect::<Vec<_>>();
            project_lock::expected_workspace(&owner, &owner_snapshot.digest, &inputs)?
        }
    } else {
        if let Some(allowance) = &reader.registry_budget {
            project_lock::expected_registry(
                &graphs[0].specs,
                &graphs[0].manifests,
                fingerprints[0]
                    .get(1..)
                    .ok_or("source package root fingerprint is missing")?,
                &mut allowance.borrow_mut(),
            )?
        } else {
            let dependency_fingerprints = fingerprints[0]
                .iter()
                .filter(|(index, _)| *index != 0)
                .cloned()
                .collect::<Vec<_>>();
            project_lock::expected(
                &graphs[0].specs,
                &graphs[0].manifests,
                &dependency_fingerprints,
            )?
        }
    };
    let expected =
        expected.with_registries(&owner, graphs.iter().flat_map(PackageGraph::registry_pins))?;
    project_lock::verify(&expected, Some(&existing))?;
    for &(slot, index) in packages.values() {
        if graphs[slot].manifests[index].manifest.tests.is_empty() {
            continue;
        }
        let graph = reroot(&graphs[slot], index, &mut reader)?;
        let mut declared = graph.manifests[0].manifest.tests.iter().collect::<Vec<_>>();
        declared.sort_by_key(|(name, _)| *name);
        for (_, relative) in declared {
            let entry = reader.source(&graph.specs[0].root, relative)?;
            let session = frontend::prepare_package_project_bounded(
                &graph.specs,
                Some(&entry),
                roster.source_files_left,
                roster.source_bytes_left,
            )
            .map_err(|error| error.message())?;
            graph.verify_registries(&session)?;
            session.source_package_preflight(Some(true))?;
            roster.collect(&session, &graph)?;
        }
    }
    // The original lock is verified above. Change only the exported manifest
    // dependency values and matching digests; user files stay untouched.
    let mut updates = HashMap::new();
    let mut manifest_bytes = MANIFEST_BYTES - reader.remaining;
    for (root, &(slot, index)) in &packages {
        let graph = &graphs[slot];
        let manifest = &graph.manifests[index];
        let mut replacements = HashMap::new();
        for edge in &graph.specs[index].dependencies {
            if git_source(&manifest.manifest.dependencies[&edge.alias])
                || manifest
                    .manifest
                    .registry_dependencies
                    .as_deref()
                    .is_some_and(|entries| entries.iter().any(|(name, _)| name == &edge.alias))
            {
                if let Some(allowance) = &reader.registry_budget {
                    let mut budget = allowance.borrow_mut();
                    let path = project_lock::relative_registry(
                        root,
                        &graph.specs[edge.package].root,
                        &mut budget,
                    )?;
                    relative_path(&path, true)?;
                    let alias = budget.text(&edge.alias)?;
                    budget.map_insert(&mut replacements, alias, path)?;
                } else {
                    let path = project_lock::relative(root, &graph.specs[edge.package].root)?;
                    relative_path(&path, true)?;
                    replacements.insert(edge.alias.clone(), path);
                }
            }
        }
        if replacements.is_empty() {
            continue;
        }
        let path = root.join("oxid.toml");
        let previous = reader.retained_manifest(&path, manifest)?;
        let bytes = if let Some(budget) = &reader.registry_budget {
            crate::project_registry::rewrite_manifest(
                &path,
                previous,
                &replacements,
                false,
                &mut budget.borrow_mut(),
            )?
        } else {
            runtime::rewrite_dependency_sources(&path, previous, &replacements)?
        };
        manifest_bytes = manifest_bytes
            .checked_sub(previous.len())
            .and_then(|count| count.checked_add(bytes.len()))
            .ok_or("exported manifest size overflow")?;
        if manifest_bytes > MANIFEST_BYTES {
            return Err("exported manifests exceed the 8 MiB aggregate limit".into());
        }
        let snapshot = if let Some(budget) = &reader.registry_budget {
            crate::project_registry::parse_manifest(&path, &bytes, &mut budget.borrow_mut())?
        } else {
            runtime::parse_typed_manifest_bytes(&path, &bytes)?
        };
        if let Some(allowance) = &reader.registry_budget {
            let mut budget = allowance.borrow_mut();
            let root = budget.path(root)?;
            budget.map_insert(&mut updates, root, snapshot.digest)?;
        } else {
            updates.insert(root.clone(), snapshot.digest);
        }
        roster.replace(&path, previous, bytes)?;
    }
    if !updates.is_empty() {
        let bytes = if let Some(budget) = &reader.registry_budget {
            expected.export_registry_with_manifests(&owner, &updates, &mut budget.borrow_mut())?
        } else {
            expected.export_with_manifests(&owner, &updates)?
        };
        roster.replace(&owner.join("oxid.lock"), existing.bytes(), bytes)?;
    }
    let mut anchor = owner.as_path();
    for root in packages.keys() {
        while !root.starts_with(anchor) {
            anchor = anchor
                .parent()
                .ok_or("source package locations must share a filesystem root")?;
        }
    }
    let archive_path = |path: &Path| -> Result<String, String> {
        let relative = path
            .strip_prefix(anchor)
            .ok()
            .and_then(Path::to_str)
            .ok_or("source package layout must be bounded UTF-8")?;
        if relative.is_empty() {
            return Ok("source".into());
        }
        relative_path(relative, false)?;
        Ok(format!("source/{relative}"))
    };
    let root = archive_path(&owner)?;
    let mut files = Vec::new();
    // Discovery inputs are read dependencies even when they are unrelated to
    // the archive payload. Output publication must preserve them as well.
    let mut input_paths = reader.raw.keys().cloned().collect::<BTreeSet<_>>();
    input_paths.extend(discovery_locks);
    input_paths.extend(reader.cache_roots);
    for (path, bytes) in roster.files {
        files.push((archive_path(&path)?, bytes));
        input_paths.insert(path);
    }
    files.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let mut input_paths = input_paths.into_iter().collect::<Vec<_>>();
    append_registry_input_paths(&mut input_paths, &graphs)?;
    Ok(crate::project_package::SourcePlan {
        owner,
        root,
        files,
        input_paths,
    })
}

fn append_registry_input_paths(
    input_paths: &mut Vec<PathBuf>,
    graphs: &[PackageGraph],
) -> Result<(), String> {
    let mut added = false;
    for graph in graphs {
        if let Some(origins) = &graph.registries {
            for (_, origin) in &origins.entries {
                let parent = origin
                    .root
                    .parent()
                    .ok_or("registry cache root is missing")?;
                let mut present = false;
                for path in input_paths.iter() {
                    if path == parent {
                        present = true;
                        break;
                    }
                }
                if present {
                    continue;
                }
                let mut budget = origins.allowance.borrow_mut();
                budget.reserve(input_paths, 1)?;
                let path = budget.path(parent)?;
                input_paths.push(path);
                added = true;
            }
        }
    }
    if added {
        input_paths.sort_unstable();
        input_paths.dedup();
    }
    Ok(())
}

#[repr(C)]
struct RegistryCollectorRoles {
    reader: Reader,
    snapshot_input: &'static runtime::ManifestSnapshot,
    snapshot_return: Result<runtime::ManifestSnapshot, String>,
    retained_snapshot: runtime::ManifestSnapshot,
    retained_key: PathBuf,
    old_input_roster: BTreeSet<PathBuf>,
    input_paths: Vec<PathBuf>,
    graphs: &'static [PackageGraph],
    graphs_iter: std::slice::Iter<'static, PackageGraph>,
    graph: &'static PackageGraph,
    origins: &'static crate::project_registry::GraphOrigins,
    origins_iter: std::slice::Iter<'static, (usize, Arc<crate::project_registry::Origin>)>,
    entry: &'static (usize, Arc<crate::project_registry::Origin>),
    parent: Option<&'static Path>,
    parent_return: Result<&'static Path, &'static str>,
    collision_iter: std::slice::Iter<'static, PathBuf>,
    collision_path: &'static PathBuf,
    present: bool,
    allowance: &'static crate::project_registry::Allowance,
    budget: std::cell::RefMut<'static, crate::project_registry::Budget>,
    output_path: PathBuf,
    output_path_return: Result<PathBuf, String>,
    reservation_return: Result<(), String>,
    added: bool,
    append_return: Result<(), String>,
    path_input: &'static Path,
    boundary: Option<&'static Path>,
    temporary_path: PathBuf,
    components: std::path::Components<'static>,
    component_return: Option<Component<'static>>,
    component: Component<'static>,
    path_metadata_return: std::io::Result<fs::Metadata>,
    path_metadata: fs::Metadata,
    path_file_type: fs::FileType,
    file: bool,
    expected_file: bool,
    path_bound: Option<usize>,
    admitted_bound: Result<usize, &'static str>,
}

pub(super) fn registry_control_bytes() -> usize {
    std::mem::size_of::<RegistryCollectorRoles>()
}

#[cfg(all(
    test,
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "oxid-source-collector-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            let fixture = Self(path);
            fixture.write("app/oxid.toml", "[project]\nname=\"app\"\nedition=\"typed-preview\"\n[dependencies]\nlib=\"../lib\"\n[tests]\nsmoke=\"tests/check.ox\"\n");
            fixture.write(
                "app/src/main.ox",
                "fn main()->i32{return crate::lib::value();}",
            );
            fixture.write(
                "app/tests/check.ox",
                "mod helper; fn main()->i32{return crate::helper::value();}",
            );
            fixture.write("app/tests/helper.ox", "pub fn value()->i32{return 7;}");
            fixture.write("lib/oxid.toml", "[project]\nname=\"lib\"\nedition=\"typed-preview\"\nkind=\"lib\"\nentry=\"target/lib.ox\"\n[tests]\nsmoke=\".oxid/smoke.ox\"\n");
            fixture.write(
                "lib/target/lib.ox",
                "mod helper; pub fn value()->i32{return crate::helper::value();}",
            );
            fixture.write("lib/target/helper.ox", "pub fn value()->i32{return 11;}");
            fixture.write(
                "lib/.oxid/smoke.ox",
                "mod helper; fn main()->i32{return crate::helper::value();}",
            );
            fixture.write("lib/.oxid/helper.ox", "pub fn value()->i32{return 9;}");
            fixture.write("lib/.oxid/cache/private", "incidental data");
            fixture.lock("app");
            fixture
        }
        fn write(&self, path: &str, value: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, value).unwrap();
        }
        fn lock(&self, path: &str) {
            let root = self.0.join(path);
            let snapshot = runtime::load_typed_manifest(&root.join("oxid.toml")).unwrap();
            let graph = package_graph(&root, &snapshot).unwrap();
            let session = frontend::prepare_package_project(&graph.specs, None).unwrap();
            let fingerprints = session
                .dependency_fingerprints()
                .iter()
                .map(|input| (input.package, input.checksum.clone()))
                .collect::<Vec<_>>();
            let expected =
                project_lock::expected(&graph.specs, &graph.manifests, &fingerprints).unwrap();
            project_lock::publish(&root, &expected, None, false).unwrap();
        }
        fn plan(&self) -> Result<crate::project_package::SourcePlan, String> {
            package_source_plan(&self.0.join("app"), None, false)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn collector_retains_dependency_test_closures_and_survives_source_tree_removal() {
        let fixture = Fixture::new();
        let plan = fixture.plan().unwrap();
        assert_eq!(plan.root, "source/app");
        assert_eq!(plan.files.len(), 10);
        assert!(plan
            .files
            .iter()
            .any(|(path, bytes)| path == "source/lib/.oxid/helper.ox"
                && bytes.ends_with(b"return 9;}")));
        assert!(plan
            .files
            .iter()
            .any(|(path, _)| path == "source/lib/target/helper.ox"));
        assert!(!plan.files.iter().any(|(path, _)| path.contains("cache")));
        assert!(plan.input_paths.iter().all(|path| path.is_absolute()));
        // Nonzero main/test bodies were checked without being executed.
        let stage = fixture.0.join("stage");
        for (path, bytes) in &plan.files {
            let path = stage.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        fs::remove_dir_all(fixture.0.join("app")).unwrap();
        fs::remove_dir_all(fixture.0.join("lib")).unwrap();
        // An invalid enclosing workspace must never participate in staging.
        fixture.write(
            "oxid.toml",
            "[workspace]\nedition=\"typed-preview\"\nmembers=[\"missing\"]\n",
        );
        let relocated = package_source_plan(&stage.join(&plan.root), Some(&stage), false).unwrap();
        assert_eq!(relocated.root, plan.root);
        assert_eq!(relocated.files, plan.files);
    }

    #[test]
    fn collector_rejects_stale_dependencies_and_type_invalid_unexecuted_tests() {
        let fixture = Fixture::new();
        fixture.write("lib/target/helper.ox", "pub fn value()->i32{return 12;}");
        assert!(fixture.plan().err().unwrap().contains("out of date"));
        fixture.write("lib/target/helper.ox", "pub fn value()->i32{return 11;}");
        fixture.write("lib/.oxid/smoke.ox", "fn main()->i32{return true;}");
        let error = fixture.plan().err().unwrap();
        assert!(error.contains("E0300"), "{error}");
    }

    #[test]
    fn collector_stage_boundary_rejects_external_dependency_before_inspecting_it() {
        let fixture = Fixture::new();
        fixture.write("stage/app/oxid.toml", "[project]\nname=\"staged\"\nedition=\"typed-preview\"\n[dependencies]\nlib=\"../../lib\"\n");
        fixture.write(
            "stage/app/src/main.ox",
            "fn main()->i32{return crate::lib::value();}",
        );
        fixture.lock("stage/app");
        fs::remove_dir_all(fixture.0.join("lib")).unwrap();
        let stage = fixture.0.join("stage");
        let error = package_source_plan(&stage.join("app"), Some(&stage), false)
            .err()
            .unwrap();
        assert!(error.contains("private staging boundary"), "{error}");
    }

    #[test]
    fn collector_refuses_portability_aliases_and_symlink_entries() {
        let fixture = Fixture::new();
        let manifest = fs::read_to_string(fixture.0.join("app/oxid.toml")).unwrap();
        for path in ["../lib//", "../unused/../lib", "./../lib"] {
            fixture.write("app/oxid.toml", &manifest.replace("../lib", path));
            let error = fixture.plan().err().unwrap();
            assert!(error.contains("normal components"), "{path}: {error}");
        }
        fixture.write(
            "app/oxid.toml",
            &manifest.replace("../lib", fixture.0.join("lib").to_str().unwrap()),
        );
        assert!(fixture.plan().err().unwrap().contains("not absolute paths"));
        fixture.write("app/oxid.toml", &manifest);
        fixture.write("outside.ox", "fn main()->i32{return 0;}");
        fs::remove_file(fixture.0.join("app/src/main.ox")).unwrap();
        std::os::unix::fs::symlink(
            fixture.0.join("outside.ox"),
            fixture.0.join("app/src/main.ox"),
        )
        .unwrap();
        assert!(fixture.plan().err().unwrap().contains("symbolic links"));
    }

    #[test]
    fn collector_protects_unbundled_ancestor_discovery_inputs() {
        let fixture = Fixture::new();
        fixture.write("oxid.toml", "[project]\nname=\"unrelated ancestor\"\n");
        fixture.write("oxid.lock", "version = 1\n");
        let plan = fixture.plan().unwrap();
        assert!(plan.input_paths.contains(&fixture.0.join("oxid.toml")));
        assert!(plan.input_paths.contains(&fixture.0.join("oxid.lock")));
        assert!(!plan
            .files
            .iter()
            .any(|(path, _)| matches!(path.as_str(), "source/oxid.toml" | "source/oxid.lock")));
        assert!(plan.input_paths.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn manifest_and_payload_admission_preserve_budgets_before_retention() {
        let fixture = Fixture::new();
        let mut remaining = 4;
        let error =
            runtime::read_typed_manifest_bytes(&fixture.0.join("app/oxid.toml"), &mut remaining)
                .err()
                .unwrap();
        assert!(error.contains("at most 4 bytes"), "{error}");
        assert_eq!(remaining, 4);
        let mut reader = Reader {
            boundary: None,
            remaining: MANIFEST_BYTES,
            raw: HashMap::new(),
            parsed: HashMap::new(),
            graph_nodes_left: 256,
            graph_edges_left: 1024,
            git: None,
            cache_roots: BTreeSet::new(),
            registry_budget: None,
        };
        let path = fixture.0.join("app/oxid.toml");
        let snapshot = reader.manifest(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(reader
            .retained_manifest(&path, &snapshot)
            .unwrap()
            .starts_with(b"[project]"));
        let mut changed = snapshot.clone();
        changed.digest = "incorrect digest".into();
        assert!(reader
            .retained_manifest(&path, &changed)
            .unwrap_err()
            .contains("reader identity"));
        reader.parsed.remove(&path);
        assert!(reader
            .retained_manifest(&path, &snapshot)
            .unwrap_err()
            .contains("not admitted"));
        let mut roster = Roster {
            files: BTreeMap::new(),
            bytes: PAYLOAD_BYTES,
            source_files_left: SOURCE_FILES,
            source_bytes_left: SOURCE_BYTES,
        };
        assert!(roster.insert(fixture.0.join("payload"), b"x").is_err());
        assert!(roster.files.is_empty());
        assert_eq!(roster.bytes, PAYLOAD_BYTES);
    }

    fn manifest_fixture() -> Fixture {
        let path = std::env::temp_dir().join(format!(
            "oxid-registry-collector-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let fixture = Fixture(path);
        fixture.write("app/oxid.toml", "[project]\nname=\"app\"\nedition=\"typed-preview\"\n[dependencies]\ncounts={ registry=\"file:///unread/registry\", package=\"counts\", version=\"1.0.0\" }\n[tests]\nsmoke=\"tests/smoke.ox\"\n");
        fixture
    }

    fn manifest_reader(allowance: Option<crate::project_registry::Allowance>) -> Reader {
        Reader {
            boundary: None,
            remaining: MANIFEST_BYTES,
            raw: HashMap::new(),
            parsed: HashMap::new(),
            graph_nodes_left: 256,
            graph_edges_left: 1024,
            git: None,
            cache_roots: BTreeSet::new(),
            registry_budget: allowance,
        }
    }

    #[test]
    fn registry_collector_initial_snapshot_and_map_have_exact_and_fault_admission() {
        let fixture = manifest_fixture();
        let path = fixture.0.join("app/oxid.toml");
        let original = fs::read(&path).unwrap();
        let measured = crate::project_registry::operation_budget().unwrap();
        let mut reader = manifest_reader(Some(measured.clone()));
        let snapshot = reader.manifest(&path).unwrap();
        let (_, peak, reservations) = measured.borrow().test_observation();
        assert!(
            reservations >= 4,
            "parser, clone, key and map are separately admitted"
        );
        assert_eq!(reader.parsed.len(), 1);
        assert_eq!(
            reader.retained_manifest(&path, &snapshot).unwrap(),
            original
        );
        assert!(std::rc::Rc::ptr_eq(
            snapshot.manifest.registry_dependencies.as_ref().unwrap(),
            reader.parsed[&path]
                .manifest
                .registry_dependencies
                .as_ref()
                .unwrap()
        ));

        let exact = crate::project_registry::operation_budget().unwrap();
        exact.borrow_mut().test_limit(peak);
        let repeated = manifest_reader(Some(exact.clone()))
            .manifest(&path)
            .unwrap();
        assert_eq!(repeated.digest, snapshot.digest);
        assert_eq!(exact.borrow().test_observation().1, peak);
        let short = crate::project_registry::operation_budget().unwrap();
        short.borrow_mut().test_limit(peak - 1);
        let mut denied = manifest_reader(Some(short));
        assert!(denied.manifest(&path).unwrap_err().contains("metadata"));
        assert!(denied.parsed.is_empty());
        for ordinal in 0..reservations {
            let allowance = crate::project_registry::operation_budget().unwrap();
            allowance.borrow_mut().test_deny_reservation(ordinal);
            let mut denied = manifest_reader(Some(allowance));
            assert!(
                denied.manifest(&path).unwrap_err().contains("injected"),
                "reservation {ordinal}"
            );
            assert!(denied.parsed.is_empty(), "reservation {ordinal}");
            assert_eq!(fs::read(&path).unwrap(), original);
        }

        // Only a real registry descriptor creates the lazy allowance. The
        // predecessor path-only branch retains its original reader behavior.
        let mut lazy = manifest_reader(None);
        lazy.manifest(&path).unwrap();
        assert!(lazy.registry_budget.is_some());
        fixture.write(
            "plain.toml",
            "[project]\nedition=\"typed-preview\"\n[dependencies]\nlocal=\"../local\"\n",
        );
        let mut ordinary = manifest_reader(None);
        ordinary.manifest(&fixture.0.join("plain.toml")).unwrap();
        assert!(ordinary.registry_budget.is_none());
    }

    #[test]
    fn registry_collector_cached_snapshot_is_paid_without_reopening_inputs() {
        let fixture = manifest_fixture();
        let path = fixture.0.join("app/oxid.toml");
        let warm = || {
            let allowance = crate::project_registry::operation_budget().unwrap();
            let mut reader = manifest_reader(Some(allowance.clone()));
            reader.manifest(&path).unwrap();
            allowance.borrow_mut().test_reset_peak();
            (reader, allowance)
        };
        let (mut reader, measured) = warm();
        let (before, _, first_reservation) = measured.borrow().test_observation();
        let remaining = reader.remaining;
        let snapshot = reader.manifest(&path).unwrap();
        let (retained, peak, after_reservation) = measured.borrow().test_observation();
        assert!(retained > before);
        assert!(after_reservation > first_reservation);
        assert_eq!(reader.remaining, remaining);
        assert!(std::rc::Rc::ptr_eq(
            snapshot.manifest.registry_dependencies.as_ref().unwrap(),
            reader.parsed[&path]
                .manifest
                .registry_dependencies
                .as_ref()
                .unwrap()
        ));
        assert_ne!(
            snapshot.manifest.dependencies["counts"].as_ptr(),
            reader.parsed[&path].manifest.dependencies["counts"].as_ptr()
        );

        let (mut exact_reader, exact) = warm();
        exact.borrow_mut().test_limit(peak);
        assert_eq!(
            exact_reader.manifest(&path).unwrap().digest,
            snapshot.digest
        );
        assert_eq!(exact.borrow().test_observation().1, peak);
        let (mut short_reader, short) = warm();
        short.borrow_mut().test_limit(peak - 1);
        assert!(short_reader
            .manifest(&path)
            .unwrap_err()
            .contains("metadata"));
        assert_eq!(short_reader.parsed[&path].digest, snapshot.digest);
        for ordinal in first_reservation..after_reservation {
            let (mut denied, allowance) = warm();
            allowance.borrow_mut().test_deny_reservation(ordinal);
            assert!(denied.manifest(&path).unwrap_err().contains("injected"));
            assert_eq!(denied.parsed[&path].digest, snapshot.digest);
            assert_eq!(denied.remaining, remaining);
        }
        let original = fs::read(&path).unwrap();
        fs::remove_file(&path).unwrap();
        let cached = reader.manifest(&path).unwrap();
        assert_eq!(cached.digest, snapshot.digest);
        assert_eq!(reader.retained_manifest(&path, &cached).unwrap(), original);
    }

    #[test]
    fn registry_collector_path_check_precedes_temporary_allocation() {
        let fixture = manifest_fixture();
        let path = fixture.0.join("app");
        let original = fs::read(path.join("oxid.toml")).unwrap();
        let mut measured = crate::project_registry::Budget::new().unwrap();
        let before = measured.test_observation().0;
        registry_ordinary_path(&path, &mut measured).unwrap();
        let (retained, peak, reservations) = measured.test_observation();
        assert_eq!(retained, before);
        assert_eq!(reservations, 1);
        let mut exact = crate::project_registry::Budget::new().unwrap();
        exact.test_limit(peak);
        registry_ordinary_path(&path, &mut exact).unwrap();
        assert_eq!(exact.test_observation().1, peak);
        let mut short = crate::project_registry::Budget::new().unwrap();
        short.test_limit(peak - 1);
        assert!(registry_ordinary_path(&path, &mut short)
            .unwrap_err()
            .contains("metadata"));
        assert_eq!(short.test_observation().2, 0);
        let mut denied = crate::project_registry::Budget::new().unwrap();
        denied.test_deny_reservation(0);
        // This missing input would produce an inspection error if the callback
        // ran. The allocator refusal must take precedence over that read.
        assert!(registry_ordinary_path(&path.join("missing"), &mut denied)
            .unwrap_err()
            .contains("injected"));
        assert_eq!(fs::read(path.join("oxid.toml")).unwrap(), original);
    }

    fn registry_graph_fixture() -> Fixture {
        use crate::project_package::{LibraryArchive, LibraryMember};
        let fixture = manifest_fixture();
        fixture.write(
            "registry/registry.toml",
            "version = 1\nkind = \"typed-private-registry-v1\"\n",
        );
        let release = fixture.0.join("registry/packages/counts/1.0.0");
        fs::create_dir_all(&release).unwrap();
        let manifest = b"[project]\nname=\"counts\"\nversion=\"1.0.0\"\nedition=\"typed-preview\"\nkind=\"lib\"\n";
        let archive = LibraryArchive {
            root: "source".into(),
            files: vec![
                LibraryMember {
                    path: "source/oxid.toml".into(),
                    bytes: manifest.to_vec(),
                },
                LibraryMember {
                    path: "source/src/lib.ox".into(),
                    bytes: b"pub fn answer()->i32{return 7;}".to_vec(),
                },
            ],
        };
        let info = crate::project_package::write_library(
            &archive,
            &mut fs::File::create(release.join("artifact.tar")).unwrap(),
            &mut crate::project_registry::Budget::new().unwrap(),
        )
        .unwrap();
        fs::write(release.join("release.toml"), format!(
            "version = 1\nkind = \"typed-library-release-v1\"\nname = \"counts\"\npackage-version = \"1.0.0\"\nedition = \"typed-preview\"\nentry = \"src/lib.ox\"\nmanifest = \"{}\"\nartifact = \"{}\"\nbytes = {}\n",
            crate::project_registry::hash(manifest), info.sha256, info.bytes
        )).unwrap();
        fixture.write("app/oxid.toml", &format!(
            "[project]\nname=\"app\"\nedition=\"typed-preview\"\n[dependencies]\nfirst={{ registry=\"file://{}\", package=\"counts\", version=\"1.0.0\" }}\nsecond={{ registry=\"file://{}\", package=\"counts\", version=\"1.0.0\" }}\n",
            fixture.0.join("registry").display(), fixture.0.join("registry").display()
        ));
        fixture.write("app/src/main.ox", "fn main()->i32{return 0;}");
        fixture
    }

    fn admitted_registry_graphs(
        fixture: &Fixture,
    ) -> (Vec<PackageGraph>, crate::project_registry::Allowance) {
        let root = fixture.0.join("app");
        let mut reader = manifest_reader(None);
        reader.git = Some(RegularProjectReader {
            root: root.clone(),
            locked: false,
            offline: false,
            git_roots: Vec::new(),
            registry: None,
            registry_budget: None,
            registry_no_fetch: false,
        });
        let snapshot = reader.manifest(&root.join("oxid.toml")).unwrap();
        let graphs = vec![
            package_graph_with(&root, &snapshot, &mut reader).unwrap(),
            package_graph_with(&root, &snapshot, &mut reader).unwrap(),
        ];
        assert!(
            reader.cache_roots.is_empty(),
            "registry parents are borrowed from graph origins"
        );
        assert_eq!(graphs[0].specs.len(), 2);
        assert_eq!(
            graphs[0].specs[0].dependencies[0].package,
            graphs[0].specs[0].dependencies[1].package
        );
        let allowance = reader.registry_budget.unwrap();
        allowance.borrow_mut().test_reset_peak();
        (graphs, allowance)
    }

    #[test]
    fn registry_collector_collision_roster_preserves_exact_parents_and_admission() {
        let fixture = registry_graph_fixture();
        // First establish an authentic immutable cache. Every measured replay
        // thereafter uses the same complete cache and actual graph origins.
        admitted_registry_graphs(&fixture);
        let (graphs, measured) = admitted_registry_graphs(&fixture);
        let original = vec![fixture.0.join("app/oxid.toml")];
        let parent = graphs[0].registries.as_ref().unwrap().entries[0]
            .1
            .root
            .parent()
            .unwrap();
        let mut expected = original.iter().cloned().collect::<BTreeSet<_>>();
        expected.insert(parent.to_owned());
        let expected = expected.into_iter().collect::<Vec<_>>();
        let mut paths = original.clone();
        let first_reservation = measured.borrow().test_observation().2;
        append_registry_input_paths(&mut paths, &graphs).unwrap();
        assert_eq!(paths, expected);
        assert_eq!(
            paths.len(),
            original.len() + 1,
            "duplicate graph parents appear once"
        );
        let (_, peak, after_reservation) = measured.borrow().test_observation();
        assert_eq!(
            after_reservation - first_reservation,
            2,
            "one Vec growth and one owned path"
        );
        let capacity = paths.capacity();
        let observation = measured.borrow().test_observation();
        append_registry_input_paths(&mut paths, &graphs).unwrap();
        assert_eq!(paths, expected);
        assert_eq!(paths.capacity(), capacity);
        assert_eq!(measured.borrow().test_observation(), observation);

        let (exact_graphs, exact) = admitted_registry_graphs(&fixture);
        exact.borrow_mut().test_limit(peak);
        let mut exact_paths = original.clone();
        append_registry_input_paths(&mut exact_paths, &exact_graphs).unwrap();
        assert_eq!(exact_paths, expected);
        assert_eq!(exact.borrow().test_observation().1, peak);
        let (short_graphs, short) = admitted_registry_graphs(&fixture);
        short.borrow_mut().test_limit(peak - 1);
        let mut short_paths = original.clone();
        assert!(append_registry_input_paths(&mut short_paths, &short_graphs)
            .unwrap_err()
            .contains("metadata"));
        assert_eq!(short_paths, original);
        for ordinal in first_reservation..after_reservation {
            let (denied_graphs, denied) = admitted_registry_graphs(&fixture);
            denied.borrow_mut().test_deny_reservation(ordinal);
            let mut paths = original.clone();
            assert!(append_registry_input_paths(&mut paths, &denied_graphs)
                .unwrap_err()
                .contains("injected"));
            assert_eq!(paths, original);
        }
        let before = original.as_ptr();
        let mut unchanged = original;
        let capacity = unchanged.capacity();
        append_registry_input_paths(
            &mut unchanged,
            &[PackageGraph {
                specs: Vec::new(),
                manifests: Vec::new(),
                registries: None,
            }],
        )
        .unwrap();
        assert_eq!(unchanged.as_ptr(), before);
        assert_eq!(unchanged.capacity(), capacity);
        assert_eq!(unchanged, vec![fixture.0.join("app/oxid.toml")]);
    }

    #[test]
    fn registry_collector_control_roles_have_independent_offsets_and_alignment() {
        use std::mem::{align_of, size_of};
        macro_rules! layout {
            ($($field:ident : $ty:ty),+ $(,)?) => {{
                let mut bytes = 0usize;
                let mut alignment = 1usize;
                $(let align = align_of::<$ty>(); alignment = alignment.max(align);
                  bytes = bytes.div_ceil(align) * align;
                  assert_eq!(std::mem::offset_of!(RegistryCollectorRoles, $field), bytes);
                  bytes += size_of::<$ty>();)+
                assert_eq!(align_of::<RegistryCollectorRoles>(), alignment);
                assert_eq!(registry_control_bytes(), bytes.div_ceil(alignment) * alignment);
            }};
        }
        layout!(reader: Reader, snapshot_input: &'static runtime::ManifestSnapshot,
            snapshot_return: Result<runtime::ManifestSnapshot,String>, retained_snapshot: runtime::ManifestSnapshot,
            retained_key: PathBuf, old_input_roster: BTreeSet<PathBuf>, input_paths: Vec<PathBuf>,
            graphs: &'static [PackageGraph], graphs_iter: std::slice::Iter<'static,PackageGraph>, graph: &'static PackageGraph,
            origins: &'static crate::project_registry::GraphOrigins,
            origins_iter: std::slice::Iter<'static,(usize,Arc<crate::project_registry::Origin>)>,
            entry: &'static (usize,Arc<crate::project_registry::Origin>), parent: Option<&'static Path>,
            parent_return: Result<&'static Path,&'static str>, collision_iter: std::slice::Iter<'static,PathBuf>,
            collision_path: &'static PathBuf, present: bool, allowance: &'static crate::project_registry::Allowance,
            budget: std::cell::RefMut<'static,crate::project_registry::Budget>, output_path: PathBuf,
            output_path_return: Result<PathBuf,String>, reservation_return: Result<(),String>, added: bool,
            append_return: Result<(),String>, path_input: &'static Path, boundary: Option<&'static Path>,
            temporary_path: PathBuf, components: std::path::Components<'static>, component_return: Option<Component<'static>>,
            component: Component<'static>, path_metadata_return: std::io::Result<fs::Metadata>, path_metadata: fs::Metadata,
            path_file_type: fs::FileType, file: bool, expected_file: bool, path_bound: Option<usize>,
            admitted_bound: Result<usize,&'static str>);
    }
}
