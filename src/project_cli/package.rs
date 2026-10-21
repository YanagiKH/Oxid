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
            return Ok(snapshot.clone());
        }
        let bytes = self.raw(path)?;
        let snapshot = runtime::parse_typed_manifest_bytes(path, &bytes)?;
        if let Some(entry) = &snapshot.manifest.entry {
            relative_path(entry, false)?;
        }
        for path in snapshot.manifest.tests.values() {
            relative_path(path, false)?;
        }
        // Reject every portability violation before traversing the first edge.
        for path in snapshot.manifest.dependencies.values() {
            relative_path(path, true)?;
        }
        self.parsed.insert(path.to_owned(), snapshot.clone());
        Ok(snapshot)
    }

    fn dependency(&mut self, root: &Path, source: &str) -> Result<PathBuf, String> {
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
}

struct Roster {
    files: BTreeMap<PathBuf, Vec<u8>>,
    bytes: usize,
    source_files_left: usize,
    source_bytes_left: usize,
}
impl Roster {
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
fn reroot(graph: &PackageGraph, root: usize) -> PackageGraph {
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
    PackageGraph { specs, manifests }
}

/// Admission and source retention only. The caller materializes this plan in a
/// private boundary and compares a second admitted plan before publication.
pub(crate) fn package_source_plan(
    start: &Path,
    boundary: Option<&Path>,
    require_workspace: bool,
) -> Result<crate::project_package::SourcePlan, String> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err("typed source package creation requires Linux x86_64".into());
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
    let existing = if is_workspace {
        project_lock::read_workspace(&owner, true)?
    } else {
        project_lock::read(&owner, true)?
    }
    .ok_or("source packaging requires an existing typed lock")?;
    let mut graphs = Vec::new();
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
        session.source_package_preflight(entry)?;
        fingerprints.push(session.workspace_fingerprints(&graph.specs[0])?);
        roster.collect(&session, graph)?;
    }
    let expected = if is_workspace {
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
    };
    project_lock::verify(&expected, Some(&existing))?;
    for &(slot, index) in packages.values() {
        if graphs[slot].manifests[index].manifest.tests.is_empty() {
            continue;
        }
        let graph = reroot(&graphs[slot], index);
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
            session.source_package_preflight(Some(true))?;
            roster.collect(&session, &graph)?;
        }
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
    for (path, bytes) in roster.files {
        files.push((archive_path(&path)?, bytes));
        input_paths.insert(path);
    }
    files.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    Ok(crate::project_package::SourcePlan {
        owner,
        root,
        files,
        input_paths: input_paths.into_iter().collect(),
    })
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
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
}
