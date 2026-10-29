//! Library publication checks the complete author lock and retains only the
//! selected member's own source closure. Preflight never executes user code.
use super::*;
use crate::project_package::{LibraryArchive, LibraryMember};
use crate::project_registry::{Allowance, Publication};

pub(crate) fn registry_release_plan(
    source: &str,
    selection: Option<&str>,
    budget: Allowance,
) -> Result<Publication, String> {
    let (owner, owner_snapshot, graphs, selected, workspace, existing) = if let Some(workspace) =
        workspace::discover_with_registry_budget(Some(budget.clone()))?
    {
        let (owner, snapshot, graphs, selected, existing) =
            workspace.publication(selection, budget.clone())?;
        (owner, snapshot, graphs, selected, true, existing)
    } else {
        if selection.is_some() {
            return Err("publish --package requires an enclosing workspace".into());
        }
        let manifest = find_manifest(None)?.ok_or("publish requires a typed library project")?;
        let root = budget
            .borrow_mut()
            .canonical_path(manifest.parent().ok_or("manifest parent is absent")?)?;
        let snapshot = crate::project_registry::load_manifest(&manifest, &budget)?;
        crate::project_registry::publication_manifest(&snapshot.manifest)?;
        let existing = project_lock::read_registry(&root, true, false, &mut budget.borrow_mut())?
            .ok_or("publish requires the existing author lock")?;
        let mut reader = RegularProjectReader::new(budget.borrow_mut().path(&root)?, true, true);
        reader.registry_budget = Some(budget.clone());
        let graph = package_graph_with(&root, &snapshot, &mut reader)?;
        let mut graphs = Vec::new();
        budget.borrow_mut().reserve(&mut graphs, 1)?;
        graphs.push(graph);
        (root, snapshot, graphs, 0, false, existing)
    };
    let graph = &graphs[selected];
    let manifest = &graph.manifests[0].manifest;
    crate::project_registry::publication_manifest(manifest)?;
    let descriptors = manifest
        .registry_dependencies
        .as_deref()
        .map_or(&[][..], |entries| entries.as_slice());
    if descriptors.len() != manifest.dependencies.len() {
        return Err("published libraries cannot contain path or Git dependencies; publish dependencies to the same registry first".into());
    }
    let replacements = crate::project_registry::publication_replacements(
        descriptors,
        source,
        &mut budget.borrow_mut(),
    )?;
    let root = &graph.specs[0].root;
    let path = budget.borrow_mut().join(root, "oxid.toml")?;
    let raw = budget
        .borrow_mut()
        .typed_file(&path, 1024 * 1024, "publication manifest")?;
    if budget.borrow_mut().digest(&raw)? != graph.manifests[0].digest {
        return Err("publication manifest changed after graph admission".into());
    }
    let exported = crate::project_registry::rewrite_manifest(
        &path,
        &raw,
        &replacements,
        true,
        &mut budget.borrow_mut(),
    )?;
    let mut collector = Collector {
        files: Vec::new(),
        budget: budget.clone(),
    };
    collector.add("oxid.toml", &exported)?;
    let mut fingerprints = Vec::new();
    budget
        .borrow_mut()
        .reserve(&mut fingerprints, graphs.len())?;
    for (index, graph) in graphs.iter().enumerate() {
        let (files, bytes) = budget.borrow().source_limits();
        let session = frontend::prepare_package_project_bounded(&graph.specs, None, files, bytes)
            .map_err(|e| e.message())?;
        let (files, bytes) = session.input_usage();
        budget.borrow_mut().source_usage(files, bytes)?;
        graph.verify_registries(&session)?;
        if index == selected {
            session.source_package_preflight(None)?;
            collector.collect(&session, graph)?;
        }
        fingerprints.push(publication_fingerprints(&session, graph, &budget)?);
    }
    let expected = if workspace {
        let mut inputs = Vec::new();
        budget.borrow_mut().reserve(&mut inputs, graphs.len())?;
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
            &mut budget.borrow_mut(),
        )?
    } else {
        // workspace_fingerprints always places root zero first. Borrow the
        // remaining rows rather than clone a second checksum roster.
        let dependency = fingerprints[0]
            .get(1..)
            .ok_or("publication root fingerprint is absent")?;
        project_lock::expected_registry(
            &graph.specs,
            &graph.manifests,
            dependency,
            &mut budget.borrow_mut(),
        )?
    }
    .with_registries(&owner, graphs.iter().flat_map(PackageGraph::registry_pins))?;
    project_lock::verify(&expected, Some(&existing))?;
    if manifest.tests.len() > 256 {
        return Err("library publication exceeds 256 declared tests".into());
    }
    let mut tests = Vec::new();
    budget
        .borrow_mut()
        .reserve(&mut tests, manifest.tests.len())?;
    tests.extend(manifest.tests.iter());
    tests.sort_by_key(|(name, _)| *name);
    for (_, relative) in tests {
        let entry = publication_entry(root, relative, &budget)?;
        let (files, bytes) = budget.borrow().source_limits();
        let session =
            frontend::prepare_package_project_bounded(&graph.specs, Some(&entry), files, bytes)
                .map_err(|e| e.message())?;
        let (files, bytes) = session.input_usage();
        budget.borrow_mut().source_usage(files, bytes)?;
        graph.verify_registries(&session)?;
        session.source_package_preflight(Some(true))?;
        collector.collect(&session, graph)?;
    }
    collector
        .files
        .sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    Ok(Publication {
        owner,
        archive: LibraryArchive {
            root: budget.borrow_mut().text("source")?,
            files: collector.files,
        },
    })
}

fn publication_entry(
    root: &Path,
    relative: &str,
    allowance: &Allowance,
) -> Result<PathBuf, String> {
    // Preserve the existing source-path predicate, including its CurDir rule.
    // Only its newly allocated result belongs to this registry operation.
    let bound = root
        .as_os_str()
        .len()
        .checked_add(relative.len() + 1)
        .and_then(|n| n.checked_mul(2))
        .ok_or("publication entry storage overflow")?;
    allowance.borrow_mut().scoped(
        bound,
        || local_source(root, relative),
        |path| Ok(path.capacity()),
    )
}

pub(super) fn publication_fingerprints(
    session: &frontend::PreparedPackageProject<'_>,
    graph: &PackageGraph,
    allowance: &Allowance,
) -> Result<Vec<(usize, String)>, String> {
    let count = session.dependency_fingerprints().len() + 1;
    let bound = count
        .checked_mul(std::mem::size_of::<(usize, String)>() + 71)
        .ok_or("publication fingerprint storage overflow")?;
    allowance.borrow_mut().scoped(
        bound,
        || session.workspace_fingerprints(&graph.specs[0]),
        |rows| {
            let mut bytes = rows.capacity() * std::mem::size_of::<(usize, String)>();
            for (_, checksum) in rows {
                bytes += checksum.capacity();
            }
            Ok(bytes)
        },
    )
}

struct Collector {
    files: Vec<LibraryMember>,
    budget: Allowance,
}

#[repr(C)]
struct PublicationRoles {
    publication: Publication,
    publication_return: Result<Publication, String>,
    collector: Collector,
    own_source_capture: &'static mut Collector,
    source_ordinal: usize,
    source_path: &'static str,
    source_bytes: &'static [u8],
    source_return: Result<(), String>,
    recheck_publication: &'static Publication,
    recheck_allowance: &'static Allowance,
    recheck_seen: &'static mut Vec<String>,
    graphs: Vec<PackageGraph>,
    graph_return: Result<PackageGraph, String>,
    reader: RegularProjectReader,
    owner_snapshot: runtime::ManifestSnapshot,
    root_snapshot: runtime::ManifestSnapshot,
    fingerprints: Vec<Vec<(usize, String)>>,
    fingerprint_return: Result<Vec<(usize, String)>, String>,
    existing_lock: Option<project_lock::Existing>,
    expected_lock: project_lock::Snapshot,
    expected_return: Result<project_lock::Snapshot, String>,
    workspace: Option<workspace::Workspace>,
    workspace_return: Result<Option<workspace::Workspace>, String>,
    options: Options,
    selection: Vec<usize>,
    selection_return: Result<Vec<usize>, String>,
    publication_graph_return: Result<
        (
            PathBuf,
            runtime::ManifestSnapshot,
            Vec<PackageGraph>,
            usize,
            project_lock::Existing,
        ),
        String,
    >,
    declared_tests: Vec<(&'static String, &'static String)>,
    entries: Vec<Option<PathBuf>>,
    seen: Vec<String>,
}
pub(crate) fn control_bytes() -> usize {
    std::mem::size_of::<PublicationRoles>() + super::package::registry_control_bytes()
}

fn own_source(
    collector: &mut Collector,
) -> impl FnMut(usize, &str, &[u8]) -> Result<(), String> + '_ {
    move |index, path, bytes| {
        if index == 0 {
            collector.add(path, bytes)?;
        }
        Ok(())
    }
}

fn rechecked_source<'a>(
    publication: &'a Publication,
    allowance: &'a Allowance,
    seen: &'a mut Vec<String>,
) -> impl FnMut(usize, &str, &[u8]) -> Result<(), String> + 'a {
    move |index, path, bytes| {
        if index != 0 {
            return Ok(());
        }
        let expected = publication
            .archive
            .files
            .iter()
            .find(|file| file.path.strip_prefix("source/") == Some(path))
            .ok_or("private publication loaded a source outside its artifact")?;
        if expected.bytes != bytes {
            return Err("private publication source changed after retention".into());
        }
        if !seen.iter().any(|old| old == path) {
            let mut budget = allowance.borrow_mut();
            budget.reserve(seen, 1)?;
            seen.push(budget.text(path)?);
        }
        Ok(())
    }
}
impl Collector {
    fn add(&mut self, relative: &str, bytes: &[u8]) -> Result<(), String> {
        let mut budget = self.budget.borrow_mut();
        let mut path = budget.string(
            7usize
                .checked_add(relative.len())
                .ok_or("library path length overflow")?,
        )?;
        path.push_str("source/");
        path.push_str(relative);
        if let Some(previous) = self.files.iter().find(|file| file.path == path) {
            if previous.bytes != bytes {
                return Err("library source changed between retained publication sessions".into());
            }
            return Ok(());
        }
        budget.reserve(&mut self.files, 1)?;
        let mut retained = budget.payload(bytes.len())?;
        retained.extend_from_slice(bytes);
        self.files.push(LibraryMember {
            path,
            bytes: retained,
        });
        Ok(())
    }
    fn collect(
        &mut self,
        session: &frontend::PreparedPackageProject<'_>,
        graph: &PackageGraph,
    ) -> Result<(), String> {
        session.visit_package_sources(&graph.specs[0], own_source(self))
    }
}

/// Re-admit the exact rewritten artifact in private storage. Only this private
/// reader may bind the publishing root's `self` to its explicit target source;
/// ordinary consumer roots have no such authority.
pub(crate) fn registry_recheck_release(
    source: &str,
    stage: &Path,
    publication: &Publication,
    budget: Allowance,
) -> Result<(), String> {
    struct Reader<'a> {
        root: &'a Path,
        source: &'a str,
        inner: RegularProjectReader,
    }
    impl ProjectReader for Reader<'_> {
        fn manifest(&mut self, path: &Path) -> Result<runtime::ManifestSnapshot, String> {
            self.inner.manifest(path)
        }
        fn registry_dependency(
            &mut self,
            root: &Path,
            dependency: &crate::project_registry::Dependency,
        ) -> Result<PathBuf, String> {
            if root == self.root {
                if dependency.source != "self" {
                    return Err("published root may only bind same-registry dependencies".into());
                }
                let allowance = self
                    .inner
                    .registry_budget
                    .as_ref()
                    .ok_or("publication allowance is missing")?;
                let bound = {
                    let mut budget = allowance.borrow_mut();
                    crate::project_registry::Dependency {
                        source: budget.text(self.source)?,
                        name: budget.text(&dependency.name)?,
                        version: budget.text(&dependency.version)?,
                    }
                };
                self.inner.registry_dependency(root, &bound)
            } else {
                self.inner.registry_dependency(root, dependency)
            }
        }
        fn registry_graph(
            &mut self,
            packages: &[frontend::PackageSpec],
        ) -> Result<Option<Box<crate::project_registry::GraphOrigins>>, String> {
            self.inner.registry_graph(packages)
        }
        fn finish_graph(
            &self,
            packages: &[frontend::PackageSpec],
            manifests: &[runtime::ManifestSnapshot],
        ) -> Result<(), String> {
            self.inner.finish_graph(packages, manifests)
        }
    }
    let manifest_path = budget.borrow_mut().join(stage, "oxid.toml")?;
    let snapshot = crate::project_registry::load_manifest(&manifest_path, &budget)?;
    let manifest = &snapshot.manifest;
    crate::project_registry::publication_manifest(manifest)?;
    let mut inner =
        RegularProjectReader::new(budget.borrow_mut().path(&publication.owner)?, true, true);
    inner.registry_budget = Some(budget.clone());
    let mut reader = Reader {
        root: stage,
        source,
        inner,
    };
    let graph = package_graph_with(stage, &snapshot, &mut reader)?;
    let raw = publication
        .archive
        .files
        .iter()
        .find(|file| file.path == "source/oxid.toml")
        .ok_or("publication manifest is missing")?;
    if snapshot.digest != budget.borrow_mut().digest(&raw.bytes)? {
        return Err("publication manifest changed during private re-admission".into());
    }
    let mut entries = Vec::new();
    budget
        .borrow_mut()
        .reserve(&mut entries, manifest.tests.len() + 1)?;
    entries.push(None);
    let mut tests = Vec::new();
    budget
        .borrow_mut()
        .reserve(&mut tests, manifest.tests.len())?;
    tests.extend(manifest.tests.iter());
    tests.sort_by_key(|(name, _)| *name);
    for (_, relative) in tests {
        entries.push(Some(publication_entry(stage, relative, &budget)?));
    }
    let mut seen = Vec::<String>::new();
    for entry in entries {
        let (files, bytes) = budget.borrow().source_limits();
        let session =
            frontend::prepare_package_project_bounded(&graph.specs, entry.as_deref(), files, bytes)
                .map_err(|e| e.message())?;
        let (files, bytes) = session.input_usage();
        budget.borrow_mut().source_usage(files, bytes)?;
        graph.verify_registries(&session)?;
        session.source_package_preflight(entry.as_ref().map(|_| true))?;
        session.visit_package_sources(
            &graph.specs[0],
            rechecked_source(publication, &budget, &mut seen),
        )?;
    }
    if seen.len() + 1 != publication.archive.files.len() {
        return Err("library artifact includes a source outside its normal/test closure".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of, size_of_val};

    #[test]
    fn registry_publication_callbacks_are_actual_paid_captures_and_keep_bytes_exact() {
        let allowance = crate::project_registry::operation_budget().unwrap();
        let mut collector = Collector {
            files: Vec::new(),
            budget: allowance.clone(),
        };
        let mut visit = own_source(&mut collector);
        assert_eq!(size_of_val(&visit), size_of::<&mut Collector>());
        visit(1, "ignored.ox", b"never retained").unwrap();
        visit(0, "src/lib.ox", b"kept bytes").unwrap();
        visit(0, "src/lib.ox", b"kept bytes").unwrap();
        assert!(visit(0, "src/lib.ox", b"different").is_err());
        drop(visit);
        assert_eq!(collector.files.len(), 1);
        let publication = Publication {
            owner: PathBuf::new(),
            archive: LibraryArchive {
                root: "source".into(),
                files: collector.files,
            },
        };
        let mut seen = Vec::new();
        let mut check = rechecked_source(&publication, &allowance, &mut seen);
        assert_eq!(
            size_of_val(&check),
            size_of::<&Publication>() + size_of::<&Allowance>() + size_of::<&mut Vec<String>>()
        );
        check(0, "src/lib.ox", b"kept bytes").unwrap();
        check(0, "src/lib.ox", b"kept bytes").unwrap();
        assert!(check(0, "src/lib.ox", b"changed").is_err());
        assert!(check(0, "unknown.ox", b"kept bytes").is_err());
        drop(check);
        assert_eq!(seen, ["src/lib.ox"]);
    }

    #[test]
    fn registry_publication_carriers_have_independent_member_offsets() {
        macro_rules! layout {
            ($($field:ident : $ty:ty),+ $(,)?) => {{
                let mut bytes = 0usize; let mut alignment = 1usize;
                $(let align = align_of::<$ty>(); alignment = alignment.max(align);
                bytes = bytes.div_ceil(align) * align;
                assert_eq!(std::mem::offset_of!(PublicationRoles, $field), bytes);
                bytes += size_of::<$ty>();)+
                assert_eq!(align_of::<PublicationRoles>(), alignment);
                assert_eq!(size_of::<PublicationRoles>(), bytes.div_ceil(alignment) * alignment);
            }};
        }
        layout!(publication: Publication, publication_return: Result<Publication,String>, collector: Collector,
            own_source_capture: &'static mut Collector, source_ordinal: usize, source_path: &'static str,
            source_bytes: &'static [u8], source_return: Result<(),String>, recheck_publication: &'static Publication,
            recheck_allowance: &'static Allowance, recheck_seen: &'static mut Vec<String>, graphs: Vec<PackageGraph>,
            graph_return: Result<PackageGraph,String>, reader: RegularProjectReader, owner_snapshot: runtime::ManifestSnapshot,
            root_snapshot: runtime::ManifestSnapshot, fingerprints: Vec<Vec<(usize,String)>>,
            fingerprint_return: Result<Vec<(usize,String)>,String>, existing_lock: Option<project_lock::Existing>,
            expected_lock: project_lock::Snapshot, expected_return: Result<project_lock::Snapshot,String>,
            workspace: Option<workspace::Workspace>, workspace_return: Result<Option<workspace::Workspace>,String>,
            options: Options, selection: Vec<usize>, selection_return: Result<Vec<usize>,String>,
            publication_graph_return: Result<(PathBuf,runtime::ManifestSnapshot,Vec<PackageGraph>,usize,project_lock::Existing),String>,
            declared_tests: Vec<(&'static String,&'static String)>, entries: Vec<Option<PathBuf>>, seen: Vec<String>);
        assert_eq!(
            control_bytes(),
            size_of::<PublicationRoles>() + super::super::package::registry_control_bytes()
        );
    }
}
