//! Manifest package identities and the single, aggregate source forest.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) struct PackageId(pub usize);

#[derive(Debug)]
pub(in crate::frontend) struct Dependency {
    pub alias: String,
    pub directory: String,
    pub target: PackageId,
    pub line: usize,
}

#[derive(Debug)]
pub(in crate::frontend) struct Package {
    pub manifest: String,
    pub canonical_manifest: PathBuf,
    pub entry: String,
    pub name: String,
    pub dependencies: Vec<Dependency>,
    pub root: ModuleId,
    pub end: usize,
}

#[derive(Debug)]
pub(in crate::frontend) struct PackageGraph {
    pub packages: Vec<Package>,
    pub retained_path_bytes: usize,
}

impl ProjectSources {
    /// Move the same source/AST owner through each package DFS. There are no
    /// separately parsed source maps, ID remapping, or synthetic declarations.
    // Test-only allocation traces enlarge LoadFailure; keep its owner transfer unboxed.
    #[cfg_attr(test, allow(clippy::result_large_err))]
    pub(in crate::frontend) fn load_forest(
        mut graph: PackageGraph,
        limits: ProjectLimits,
    ) -> Result<Self, LoadFailure> {
        let mut project = Self {
            sources: SourceMap::new(),
            programs: Vec::new(),
            modules: Vec::new(),
            canonical_root: None,
            graph: None,
            usage: SourceUsage {
                retained_path_bytes: graph.retained_path_bytes,
                ..SourceUsage::default()
            },
            syntax_flavor: SyntaxFlavor::OriginalSingleFile,
        };
        let mut allocator = Allocator::default();
        for package in &mut graph.packages {
            package.root = ModuleId(project.modules.len());
            let mut builder = SourceSetBuilder {
                entry: &package.entry,
                limits,
                allocator: &mut allocator,
                mode: parser::SourceMode::ProjectCandidate,
                arrays: parser::ArraySyntaxPolicy::Enabled,
                enums: ProjectEnumSyntax::Enabled,
                project,
            };
            if let Err(diagnostics) = builder.load_all() {
                return Err(LoadFailure {
                    sources: builder.project.sources,
                    diagnostics,
                    usage: builder.project.usage,
                    #[cfg(test)]
                    allocator,
                });
            }
            project = builder.project;
            package.end = project.modules.len();
        }
        if graph.packages.len() > 1 {
            project.syntax_flavor = SyntaxFlavor::ProjectSyntax;
        }
        project.graph = Some(graph);
        Ok(project)
    }

    pub(in crate::frontend) fn graph(&self) -> Option<&PackageGraph> {
        self.graph.as_ref()
    }

    pub(in crate::frontend) fn package_count(&self) -> usize {
        self.graph.as_ref().map_or(1, |g| g.packages.len())
    }

    pub(in crate::frontend) fn package_for_module(&self, module: ModuleId) -> Option<PackageId> {
        if module.0 >= self.modules.len() {
            return None;
        }
        let Some(graph) = &self.graph else {
            return Some(PackageId(0));
        };
        let index = graph
            .packages
            .partition_point(|p| p.root.0 <= module.0)
            .checked_sub(1)?;
        (module.0 < graph.packages[index].end).then_some(PackageId(index))
    }

    pub(in crate::frontend) fn package_root(&self, module: ModuleId) -> Option<ModuleId> {
        let id = self.package_for_module(module)?;
        Some(
            self.graph
                .as_ref()
                .map_or(ModuleId(0), |g| g.packages[id.0].root),
        )
    }
}
