//! Offline graph admission for editor checks; source buffers belong to the frontend.
use super::*;

pub(crate) struct EditorProject {
    pub packages: Vec<frontend::PackageSpec>,
    pub entry: Option<PathBuf>,
}

pub(crate) fn source_edition(path: &Path, explicit_typed: bool) -> Result<(), String> {
    if explicit_typed {
        return Ok(());
    }
    let parent = path.parent().ok_or("source has no parent directory")?;
    for (depth, directory) in parent.ancestors().enumerate() {
        if depth > 64 {
            return Err("editor edition discovery exceeds 64 ancestor directories".into());
        }
        let manifest = directory.join("oxid.toml");
        if manifest.is_file() {
            return if runtime::manifest_edition(&manifest)?.as_deref() == Some("typed-preview") {
                Ok(())
            } else {
                Err("editor formatting requires a typed project or --edition typed-preview".into())
            };
        }
    }
    Err("editor formatting requires a typed project or --edition typed-preview".into())
}

pub(crate) fn project_graphs(
    start: &Path,
    explicit_typed: bool,
) -> Result<Vec<EditorProject>, String> {
    let start = start
        .canonicalize()
        .map_err(|error| format!("cannot locate editor source: {error}"))?;
    let cwd = if start.is_dir() {
        start.as_path()
    } else {
        start.parent().ok_or("source has no parent directory")?
    };
    let mut nearest = None;
    let mut workspace_root = None;
    for (depth, directory) in cwd.ancestors().enumerate() {
        if depth > 64 {
            return Err("editor project discovery exceeds 64 ancestor directories".into());
        }
        let path = directory.join("oxid.toml");
        if path.is_file() {
            if nearest.is_none() {
                nearest = Some(path.clone());
            }
            if runtime::workspace_declaration(&path)?.is_some() {
                if workspace_root.is_some() {
                    return Err("nested workspaces are unsupported".into());
                }
                workspace_root = Some(directory.to_owned());
            }
        } else if project_lock::has_workspace_marker(directory)? {
            return Err("workspace lock requires its workspace manifest declaration".into());
        }
    }
    let mut graphs = Vec::new();
    if let Some(root) = workspace_root {
        let snapshot = runtime::load_typed_manifest(&root.join("oxid.toml"))?;
        let mut reader = RegularProjectReader {
            root: root.clone(),
            locked: true,
            offline: true,
            git_roots: Vec::new(),
        };
        let workspace = workspace::load_roster(root.clone(), cwd, snapshot, &mut reader)?;
        for (member, snapshot) in workspace.source_members() {
            append(
                &mut graphs,
                package_graph_cache_owner(member, snapshot, true, true, &root)?,
                &snapshot.manifest,
            )?;
            budget(&graphs)?;
        }
    } else if let Some(manifest) = nearest {
        let snapshot = runtime::load_typed_manifest(&manifest)?;
        if !explicit_typed && snapshot.manifest.edition.as_deref() != Some("typed-preview") {
            return Err("editor checking requires edition = \"typed-preview\" in oxid.toml, or an explicit typed edition for the server".into());
        }
        let root = manifest
            .parent()
            .ok_or("manifest has no parent directory")?;
        append(
            &mut graphs,
            package_graph_cache_owner(root, &snapshot, true, true, root)?,
            &snapshot.manifest,
        )?;
    } else if explicit_typed && start.is_file() {
        graphs.push(EditorProject {
            packages: vec![frontend::PackageSpec {
                root: cwd.to_owned(),
                entry: start.clone(),
                dependencies: Vec::new(),
            }],
            entry: None,
        });
    } else {
        return Err("open a typed Oxid project, or select --edition typed-preview for standalone source files".into());
    }
    budget(&graphs)?;
    Ok(graphs)
}

fn append(
    graphs: &mut Vec<EditorProject>,
    graph: PackageGraph,
    manifest: &runtime::ProjectManifest,
) -> Result<(), String> {
    for path in manifest.tests.values() {
        graphs.push(EditorProject {
            entry: Some(local_source(&graph.specs[0].root, path)?),
            packages: graph.specs.clone(),
        });
        budget(graphs)?;
    }
    graphs.push(EditorProject {
        packages: graph.specs,
        entry: None,
    });
    budget(graphs)
}

fn budget(graphs: &[EditorProject]) -> Result<(), String> {
    let nodes: usize = graphs.iter().map(|graph| graph.packages.len()).sum();
    let edges: usize = graphs
        .iter()
        .flat_map(|graph| &graph.packages)
        .map(|package| package.dependencies.len())
        .sum();
    if nodes > 256 || edges > 1024 {
        return Err(
            "editor checks exceed the aggregate 256-package or 1024-edge session limit".into(),
        );
    }
    Ok(())
}
