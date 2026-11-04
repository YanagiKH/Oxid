//! A bounded package session around the existing source owner and parser.
//! Manifest interpretation belongs to the CLI. This owner validates the supplied
//! graph and retains package namespaces without changing any source text or AST ID.
use super::*;
use std::path::Component;

pub(crate) const MAX_PACKAGES: usize = 256;
const MAX_DEPENDENCIES: usize = 1024;
const MAX_GRAPH_DEPTH: usize = 32;

#[derive(Clone, Debug)]
pub(crate) struct PackageDependency {
    pub alias: String,
    pub package: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct PackageSpec {
    pub root: PathBuf,
    pub entry: PathBuf,
    pub dependencies: Vec<PackageDependency>,
}

/// Extra package state lives only on the package route. Ordinary loader,
/// declaration-index and observer carriers retain their existing layouts.
#[derive(Debug)]
pub(in crate::frontend) struct PackageContext<'a> {
    specs: &'a [PackageSpec],
    roots: [usize; MAX_PACKAGES],
    owners: [usize; 256],
    modules: usize,
}
impl<'a> PackageContext<'a> {
    pub fn root(&self, module: ModuleId) -> Option<ModuleId> {
        (module.0 < self.modules).then(|| ModuleId(self.roots[self.owners[module.0]]))
    }
    pub fn dependencies(&self, module: ModuleId) -> Option<&'a [PackageDependency]> {
        (module.0 < self.modules).then(|| self.specs[self.owners[module.0]].dependencies.as_slice())
    }
    pub fn target(&self, package: usize) -> Option<ModuleId> {
        (package < self.specs.len()).then(|| ModuleId(self.roots[package]))
    }
    pub fn roots(&self) -> usize {
        self.specs.len()
    }
}

#[derive(Debug)]
pub(in crate::frontend) struct PackageSources<'a, S = ast::Program> {
    pub project: ProjectSources<S>,
    pub context: PackageContext<'a>,
}

/// Borrowed input from an already loaded package. Paths are canonical source
/// paths relative to that package's canonical root; bytes are never reopened.
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::frontend) struct LoadedSourceInput<'s> {
    pub path: &'s str,
    pub bytes: &'s [u8],
}

fn failure(message: impl Into<String>) -> Box<Diagnostic> {
    Diagnostic::new("E0006", "package", message, None)
}

fn valid_alias(alias: &str) -> bool {
    let mut chars = alias.bytes();
    matches!(chars.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'_'))
        && alias.len() <= 64
        && chars.all(|c| c.is_ascii_alphanumeric() || c == b'_')
        && !matches!(
            alias,
            "crate"
                | "std"
                | "self"
                | "super"
                | "bool"
                | "i32"
                | "u8"
                | "fn"
                | "struct"
                | "mod"
                | "use"
                | "pub"
                | "let"
                | "mut"
                | "return"
                | "break"
                | "continue"
                | "if"
                | "while"
                | "else"
                | "true"
                | "false"
                | "import"
                | "macro"
                | "macro_rules"
                | "const"
                | "for"
                | "loop"
                | "match"
                | "async"
                | "await"
                | "move"
                | "ref"
                | "unsafe"
                | "extern"
                | "enum"
                | "trait"
                | "impl"
                | "type"
                | "null"
                | "and"
                | "or"
        )
}

fn entry_relative<'a>(root: &Path, entry: &'a Path) -> Result<&'a Path, Box<Diagnostic>> {
    let relative = entry
        .strip_prefix(root)
        .map_err(|_| failure("package entry escapes its root"))?;
    if relative.as_os_str().is_empty()
        || relative.components().count() > 33
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || relative.to_str().is_none_or(|path| path.len() > 4096)
        || relative
            .components()
            .any(|part| part.as_os_str().len() > 255)
    {
        return Err(failure(
            "package entry requires a bounded relative source path without `..`",
        ));
    }
    Ok(relative)
}

pub(in crate::frontend) fn validate_graph(
    specs: &[PackageSpec],
    entry_override: Option<&Path>,
) -> Result<(), Box<Diagnostic>> {
    if specs.is_empty() || specs.len() > MAX_PACKAGES {
        return Err(failure("package count must be between 1 and 256"));
    }
    let mut edges = 0usize;
    let mut path_bytes = 0usize;
    for (index, spec) in specs.iter().enumerate() {
        let entry = if index == 0 {
            entry_override.unwrap_or(&spec.entry)
        } else {
            &spec.entry
        };
        if !spec.root.is_absolute() || !entry.is_absolute() {
            return Err(failure("package roots and entries must be absolute paths"));
        }
        for path in [&spec.root, entry] {
            let text = path
                .to_str()
                .ok_or_else(|| failure("package paths must be UTF-8"))?;
            if text.len() > 4096 {
                return Err(failure("package pathname limit exceeded"));
            }
            path_bytes = path_bytes
                .checked_add(text.len())
                .ok_or_else(|| failure("package graph size overflow"))?;
        }
        if path_bytes > 4 * 1024 * 1024 {
            return Err(failure("package graph path budget exceeded"));
        }
        #[cfg(windows)]
        {
            filesystem::admit_package_path(&spec.root)?;
            filesystem::admit_package_path(entry)?;
        }
        let canonical = spec.root.canonicalize().map_err(|e| {
            failure(format!(
                "cannot open package root {}: {e}",
                spec.root.display()
            ))
        })?;
        if canonical != spec.root || !canonical.is_dir() {
            return Err(failure("package roots must be canonical directories"));
        }
        if specs[..index]
            .iter()
            .any(|previous| previous.root == canonical)
        {
            return Err(failure("duplicate canonical package root; shared dependencies must use one package identity"));
        }
        let relative = entry_relative(&canonical, entry)?;
        let mut probe = canonical;
        for component in relative.components() {
            probe.push(component);
            let metadata = std::fs::symlink_metadata(&probe).map_err(|e| {
                failure(format!(
                    "cannot open package entry {}: {e}",
                    probe.display()
                ))
            })?;
            if metadata.file_type().is_symlink() {
                return Err(failure("package entry path contains a symbolic link"));
            }
        }
        if !probe.is_file() {
            return Err(failure("package entry must be a regular source file"));
        }
        edges = edges
            .checked_add(spec.dependencies.len())
            .ok_or_else(|| failure("package dependency count overflow"))?;
        if edges > MAX_DEPENDENCIES {
            return Err(failure("package dependency limit exceeded"));
        }
        for (ordinal, dep) in spec.dependencies.iter().enumerate() {
            if !valid_alias(&dep.alias) {
                return Err(failure("invalid typed dependency alias: expected a non-reserved ASCII identifier of at most 64 bytes"));
            }
            if dep.package >= specs.len() {
                return Err(failure("dependency targets an unknown package"));
            }
            if spec.dependencies[..ordinal]
                .iter()
                .any(|other| other.alias.eq_ignore_ascii_case(&dep.alias))
            {
                return Err(failure(
                    "dependency aliases collide under ASCII case folding",
                ));
            }
        }
    }
    let mut states = [0u8; MAX_PACKAGES];
    let mut longest_suffix = [0u8; MAX_PACKAGES];
    let mut stack = [(0usize, 0usize); MAX_GRAPH_DEPTH + 1];
    let mut height = 1;
    states[0] = 1;
    while height != 0 {
        let (package, next) = stack[height - 1];
        if next == specs[package].dependencies.len() {
            // A completed shared dependency may be reached again through a
            // longer prefix. Its full suffix must still count toward the cap;
            // first-visit DFS height alone is dependent on edge ordering.
            let longest = specs[package]
                .dependencies
                .iter()
                .map(|dependency| usize::from(longest_suffix[dependency.package]) + 1)
                .max()
                .unwrap_or(0);
            if longest > MAX_GRAPH_DEPTH {
                return Err(failure("package dependency depth limit exceeded"));
            }
            longest_suffix[package] = longest as u8;
            states[package] = 2;
            height -= 1;
            continue;
        }
        stack[height - 1].1 += 1;
        let target = specs[package].dependencies[next].package;
        if states[target] == 1 {
            return Err(failure("package dependency cycle detected"));
        }
        if states[target] == 2 {
            continue;
        }
        if height == stack.len() {
            return Err(failure("package dependency depth limit exceeded"));
        }
        states[target] = 1;
        stack[height] = (target, 0);
        height += 1;
    }
    if states[..specs.len()].contains(&0) {
        return Err(failure("package graph contains an undeclared package"));
    }
    Ok(())
}

impl<'a> PackageSources<'a> {
    pub fn source_inputs(
        &self,
        package: usize,
    ) -> Result<
        impl ExactSizeIterator<Item = Result<LoadedSourceInput<'_>, Box<Diagnostic>>> + '_,
        Box<Diagnostic>,
    > {
        let spec = self
            .context
            .specs
            .get(package)
            .ok_or_else(|| failure("invalid loaded package identity"))?;
        let start = self.context.roots[package];
        let end = if package + 1 < self.context.specs.len() {
            self.context.roots[package + 1]
        } else {
            self.context.modules
        };
        if start >= end
            || end > self.project.modules.len()
            || end > self.context.owners.len()
            || self.context.owners[start..end]
                .iter()
                .any(|&owner| owner != package)
        {
            return Err(failure("invalid loaded package source range"));
        }
        Ok((start..end).map(move |module| {
            let header = &self.project.modules[module];
            let path = header
                .canonical_path
                .as_ref()
                .and_then(|path| path.strip_prefix(&spec.root).ok())
                .and_then(Path::to_str)
                .filter(|path| !path.is_empty())
                .ok_or_else(|| {
                    failure("loaded source identity must be relative to its package root and UTF-8")
                })?;
            let source = self
                .project
                .sources
                .files()
                .get(header.file.0)
                .ok_or_else(|| failure("invalid loaded package source identity"))?;
            Ok(LoadedSourceInput {
                path,
                bytes: source.text().as_bytes(),
            })
        }))
    }

    pub fn into_source_map(self) -> SourceMap {
        self.project.sources
    }

    #[allow(clippy::result_large_err)]
    pub fn load(
        specs: &'a [PackageSpec],
        entry_override: Option<&Path>,
        limits: ProjectLimits,
    ) -> Result<Self, LoadFailure> {
        Self::load_with_editor(specs, entry_override, limits, None)
    }

    #[allow(clippy::result_large_err)]
    pub(in crate::frontend) fn load_with_editor(
        specs: &'a [PackageSpec],
        entry_override: Option<&Path>,
        limits: ProjectLimits,
        overlay: Option<&SourceOverlay<'_>>,
    ) -> Result<Self, LoadFailure> {
        Self::load_syntax_with_editor(specs, entry_override, limits, overlay)
    }
}

impl<'a, S: ModuleSyntax> PackageSources<'a, S> {
    #[allow(clippy::result_large_err)]
    pub(in crate::frontend) fn load_syntax_with_editor(
        specs: &'a [PackageSpec],
        entry_override: Option<&Path>,
        limits: ProjectLimits,
        overlay: Option<&SourceOverlay<'_>>,
    ) -> Result<Self, LoadFailure> {
        let mut allocator = Allocator::default();
        let mut builder = SourceSetBuilder {
            entry: "",
            limits,
            allocator: &mut allocator,
            mode: parser::SourceMode::ProjectCandidate,
            arrays: parser::ArraySyntaxPolicy::Enabled,
            enums: ProjectEnumSyntax::Enabled,
            project: ProjectSources {
                sources: SourceMap::new(),
                programs: Vec::new(),
                modules: Vec::new(),
                canonical_root: None,
                usage: SourceUsage::default(),
                syntax_flavor: SyntaxFlavor::ProjectSyntax,
            },
        };
        let mut context = PackageContext {
            specs,
            roots: [0; MAX_PACKAGES],
            owners: [0; 256],
            modules: 0,
        };
        let result = (|| {
            validate_graph(specs, entry_override).map_err(one)?;
            if !cfg!(any(
                target_os = "linux",
                target_os = "macos",
                all(
                    target_os = "windows",
                    target_arch = "x86_64",
                    target_pointer_width = "64",
                    target_endian = "little"
                )
            )) {
                return Err(one(failure(
                    "package source policy is not qualified on this host",
                )));
            }
            for (package, spec) in specs.iter().enumerate() {
                let entry = if package == 0 {
                    entry_override.unwrap_or(&spec.entry)
                } else {
                    &spec.entry
                };
                builder.entry = entry.to_str().expect("validated UTF-8 entry");
                let start = builder.project.modules.len();
                if overlay.is_some() {
                    builder.load_package_root_with_editor(spec, overlay)?;
                } else {
                    builder.load_package_root(spec)?;
                }
                let end = builder.project.modules.len();
                context.roots[package] = start;
                context.owners[start..end].fill(package);
                context.modules = end;
            }
            Ok(())
        })();
        match result {
            Ok(()) => Ok(Self {
                project: builder.project,
                context,
            }),
            Err(diagnostics) => Err(LoadFailure {
                sources: builder.project.sources,
                diagnostics,
                usage: builder.project.usage,
                #[cfg(test)]
                allocator: std::mem::take(builder.allocator),
            }),
        }
    }
}

#[cfg(windows)]
#[cfg_attr(test, allow(clippy::result_large_err))]
impl<S: ModuleSyntax> ProjectSources<S> {
    pub(in crate::frontend) fn load_local_package_entry(
        spec: &PackageSpec,
        entry: &Path,
        limits: ProjectLimits,
        overlay: Option<&SourceOverlay<'_>>,
    ) -> Result<Self, LoadFailure> {
        let mut allocator = Allocator::default();
        let mut builder = SourceSetBuilder {
            entry: entry.to_str().expect("validated UTF-8 entry"),
            limits,
            allocator: &mut allocator,
            mode: parser::SourceMode::ProjectCandidate,
            arrays: parser::ArraySyntaxPolicy::Enabled,
            enums: ProjectEnumSyntax::Enabled,
            project: ProjectSources {
                sources: SourceMap::new(),
                programs: Vec::new(),
                modules: Vec::new(),
                canonical_root: None,
                usage: SourceUsage::default(),
                syntax_flavor: SyntaxFlavor::OriginalSingleFile,
            },
        };
        let result = if !spec.dependencies.is_empty() {
            Err(one(failure(
                "single-package entry cannot declare dependency capabilities",
            )))
        } else if cfg!(all(
            target_arch = "x86_64",
            target_pointer_width = "64",
            target_endian = "little"
        )) {
            builder.load_package_root_with_editor(spec, overlay)
        } else {
            Err(one(failure(
                "package source policy is not qualified on this host",
            )))
        };
        match result {
            Ok(()) => Ok(builder.project),
            Err(diagnostics) => Err(LoadFailure {
                sources: builder.project.sources,
                diagnostics,
                usage: builder.project.usage,
                #[cfg(test)]
                allocator: std::mem::take(builder.allocator),
            }),
        }
    }
}

impl<S: ModuleSyntax> SourceSetBuilder<'_, S> {
    #[cfg(windows)]
    fn windows_package_entry(
        &mut self,
        spec: &PackageSpec,
        entry: &Path,
    ) -> Result<(PathBuf, filesystem::VerifiedChild), Box<Diagnostic>> {
        let path = entry_relative(&spec.root, entry)?;
        let mut length = 0usize;
        for part in path.components() {
            length = add(length, usize::from(length != 0), None)
                .and_then(|n| add(n, part.as_os_str().len(), None))?;
        }
        let retained = self.project.usage.retained_path_bytes;
        let held = add(retained, length, None)?;
        if held > self.limits.path_bytes.min(4 * 1024 * 1024) {
            return Err(resource("package entry relative path limit exceeded", None));
        }
        let mut relative = String::new();
        self.allocator
            .string(&mut relative, length, "package entry relative path")
            .map_err(|e| reserve_error(e, None))?;
        for part in path.components() {
            if !relative.is_empty() {
                relative.push('/');
            }
            relative.push_str(
                part.as_os_str()
                    .to_str()
                    .ok_or_else(|| failure("package paths must be UTF-8"))?,
            );
        }
        // The serialization coexists with the native probe and wide scratch.
        // Restore only this transient charge; real scan/probe usage survives.
        self.project.usage.retained_path_bytes = held;
        let result = filesystem::verify_entry(
            &spec.root,
            &relative,
            self.limits,
            &mut self.project.usage,
            self.allocator,
        );
        self.project.usage.retained_path_bytes = retained;
        result
    }

    fn load_package_root(&mut self, spec: &PackageSpec) -> Result<(), Errors> {
        self.load_package_root_with_editor(spec, None)
    }

    fn load_package_root_with_editor(
        &mut self,
        spec: &PackageSpec,
        overlay: Option<&SourceOverlay<'_>>,
    ) -> Result<(), Errors> {
        let id = ModuleId(self.project.modules.len());
        if id.0 >= self.limits.modules.min(256) {
            return Err(one(resource("module count limit exceeded", None)));
        }
        let entry = Path::new(self.entry);
        #[cfg(not(windows))]
        let canonical = entry
            .canonicalize()
            .map_err(|e| one(io_error(e, self.entry, None)))?;
        #[cfg(windows)]
        let (canonical, mut verified_entry) =
            self.windows_package_entry(spec, entry).map_err(one)?;
        if self
            .project
            .modules
            .iter()
            .any(|header| header.canonical_path.as_ref() == Some(&canonical))
        {
            return Err(one(failure(
                "package entry source was already loaded by another module",
            )));
        }
        let root_path = canonical
            .parent()
            .expect("absolute entry has parent")
            .to_path_buf();
        #[cfg(not(windows))]
        let text = self
            .read_source_input(&canonical, self.entry, None, overlay)
            .map_err(one)?;
        #[cfg(windows)]
        let text = self
            .read_verified_child_input(&mut verified_entry, &canonical, self.entry, None, overlay)
            .map_err(one)?;
        #[cfg(windows)]
        drop(verified_entry);
        let mut display = String::new();
        self.allocator
            .string(&mut display, self.entry.len(), "package entry display")
            .map_err(|e| one(reserve_error(e, None)))?;
        display.push_str(self.entry);
        let program = self.parse_file(display, text, None)?;
        let at = self.project.sources.get(SourceFileId(id.0)).span(0, 0);
        // Manifest aliases are separate capabilities, never synthetic AST items.
        // Reject conflicts before following any ordinary module declaration.
        for dep in &spec.dependencies {
            let names = program.binding_names();
            for name in names {
                if self.project.sources.text(name) == dep.alias {
                    return Err(one(Diagnostic::new(
                        "E0201",
                        "resolve",
                        "dependency alias conflicts with a package-root binding",
                        Some(name),
                    )));
                }
            }
        }
        let retained = add(
            self.project.usage.retained_path_bytes,
            filesystem::path_units(&root_path),
            Some(at),
        )
        .and_then(|n| add(n, filesystem::path_units(&canonical), Some(at)))
        .map_err(one)?;
        if retained > self.limits.path_bytes {
            return Err(one(resource(
                "retained module path budget exceeded",
                Some(at),
            )));
        }
        self.project.usage.retained_path_bytes = retained;
        self.project.canonical_root = Some(root_path);
        self.reserve_file(Some(at)).map_err(one)?;
        self.project.programs.push(program);
        self.project.modules.push(ModuleHeader {
            file: SourceFileId(id.0),
            parent: None,
            declaration: None,
            public: None,
            depth: 0,
            relative_path: String::new(),
            canonical_path: Some(canonical),
        });
        self.project.usage.modules = self.project.modules.len();
        let mut stack = [Frame {
            module: id,
            next: 0,
        }; 33];
        let mut height = 1;
        while height != 0 {
            let frame = stack[height - 1];
            if frame.next == self.project.programs[frame.module.0].modules().len() {
                height -= 1;
                continue;
            }
            stack[height - 1].next += 1;
            let child = if overlay.is_some() {
                self.load_child_with_inputs(frame.module, frame.next, &mut None, overlay)?
            } else {
                self.load_child(frame.module, frame.next)?
            };
            if height == stack.len() {
                return Err(one(resource("module depth limit exceeded", Some(at))));
            }
            stack[height] = Frame {
                module: child,
                next: 0,
            };
            height += 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[allow(dead_code)]
mod editor_loader_layout_tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    struct BaselinePackageContext<'a> {
        specs: &'a [PackageSpec],
        roots: [usize; 256],
        owners: [usize; 256],
        modules: usize,
    }
    struct BaselinePackageSources<'a> {
        project: ProjectSources<ast::Program>,
        context: BaselinePackageContext<'a>,
    }
    #[test]
    fn editor_query_package_loader_keeps_ordinary_field_types_and_offsets() {
        let _package_types = |value: &PackageSources<'_>| {
            let PackageSources { project, context } = value;
            let _: &ProjectSources<ast::Program> = project;
            let _: &PackageContext<'_> = context;
        };
        let _context_types = |value: &PackageContext<'_>| {
            let PackageContext {
                specs,
                roots,
                owners,
                modules,
            } = value;
            let _: &&[PackageSpec] = specs;
            let _: &[usize; 256] = roots;
            let _: &[usize; 256] = owners;
            let _: &usize = modules;
        };
        assert_eq!(
            size_of::<PackageContext<'_>>(),
            size_of::<BaselinePackageContext<'_>>()
        );
        assert_eq!(
            align_of::<PackageContext<'_>>(),
            align_of::<BaselinePackageContext<'_>>()
        );
        assert_eq!(
            size_of::<PackageSources<'_>>(),
            size_of::<BaselinePackageSources<'_>>()
        );
        assert_eq!(
            align_of::<PackageSources<'_>>(),
            align_of::<BaselinePackageSources<'_>>()
        );
        macro_rules! same_offsets {
            ($current:ty, $previous:ty, $($field:ident),+ $(,)?) => {
                $(assert_eq!(offset_of!($current, $field), offset_of!($previous, $field), stringify!($field));)+
            };
        }
        same_offsets!(
            PackageContext<'_>,
            BaselinePackageContext<'_>,
            specs,
            roots,
            owners,
            modules
        );
        same_offsets!(
            PackageSources<'_>,
            BaselinePackageSources<'_>,
            project,
            context
        );
    }
}
