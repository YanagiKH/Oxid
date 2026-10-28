//! Borrowed editor buffers for a single checked source-loading session.
//! Filesystem identities and module admission still belong to the ordinary loader.
use super::*;

pub(crate) const MAX_EDITOR_FILES: usize = 256;
pub(crate) const MAX_EDITOR_BYTES: usize = 16 * 1024 * 1024;

pub(crate) struct SourceOverlay<'a> {
    entries: &'a [(&'a Path, &'a str)],
}

impl<'a> SourceOverlay<'a> {
    pub(crate) fn new(entries: &'a [(&'a Path, &'a str)]) -> Result<Self, String> {
        if entries.len() > MAX_EDITOR_FILES {
            return Err("too many open Oxid source files".into());
        }
        let mut bytes = 0usize;
        for (index, (path, text)) in entries.iter().enumerate() {
            if !path.is_absolute()
                || path.to_str().is_none_or(|name| name.len() > 4096)
                || path.components().any(|part| {
                    matches!(
                        part,
                        std::path::Component::CurDir | std::path::Component::ParentDir
                    )
                })
            {
                return Err("editor source paths must be normalized absolute UTF-8 paths".into());
            }
            if entries[..index].iter().any(|(earlier, _)| earlier == path) {
                return Err("duplicate editor source identity".into());
            }
            bytes = bytes
                .checked_add(text.len())
                .ok_or("editor source size overflow")?;
            if text.len() > MAX_SOURCE_BYTES || bytes > MAX_EDITOR_BYTES {
                return Err("open Oxid source buffers exceed the editor byte limit".into());
            }
        }
        Ok(Self { entries })
    }

    pub(in crate::frontend) fn text(&self, path: &Path) -> Option<&str> {
        if let Some((_, text)) = self.entries.iter().find(|(key, _)| *key == path) {
            return Some(text);
        }
        let canonical = path.canonicalize().ok()?;
        self.entries
            .iter()
            .find(|(key, _)| *key == canonical)
            .map(|(_, text)| *text)
    }
}

/// Query syntax has its own concrete owner; there is no conversion to the
/// ordinary ProjectSources<ast::Program> required by compiler source owners.
/// Keep both bounded session forms inline: boxing would add an allocation and
/// failure boundary to editor queries without reducing their admitted storage.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub(in crate::frontend) enum QuerySources<'a> {
    Single(ProjectSources<parser::editor_query::EditorFile>),
    Packages(packages::PackageSources<'a, parser::editor_query::EditorFile>),
}
impl<'a> QuerySources<'a> {
    // Keep the loader's source and allocator failure evidence inline in tests.
    #[cfg_attr(test, allow(clippy::result_large_err))]
    pub(in crate::frontend) fn load(
        specs: &'a [packages::PackageSpec],
        entry: Option<&Path>,
        overlay: &SourceOverlay<'_>,
    ) -> Result<Self, LoadFailure> {
        packages::validate_graph(specs, entry).map_err(|error| LoadFailure {
            sources: SourceMap::new(),
            diagnostics: vec![*error],
            usage: Default::default(),
            #[cfg(test)]
            allocator: Allocator::default(),
        })?;
        if specs.len() == 1 {
            let entry = entry
                .unwrap_or(&specs[0].entry)
                .to_str()
                .expect("validated source path");
            ProjectSources::load_with_syntax_inputs(
                entry,
                ProjectLimits::default(),
                parser::SourceMode::ProjectCandidate,
                &mut Allocator::default(),
                parser::ArraySyntaxPolicy::Enabled,
                ProjectEnumSyntax::Enabled,
                None,
                Some(overlay),
            )
            .map(Self::Single)
        } else {
            packages::PackageSources::load_syntax_with_editor(
                specs,
                entry,
                ProjectLimits::default(),
                Some(overlay),
            )
            .map(Self::Packages)
        }
    }
    pub(in crate::frontend) fn project(&self) -> &ProjectSources<parser::editor_query::EditorFile> {
        match self {
            Self::Single(project) => project,
            Self::Packages(sources) => &sources.project,
        }
    }
    pub(in crate::frontend) fn root(&self, module: ModuleId) -> Option<ModuleId> {
        match self {
            Self::Single(_) => Some(ModuleId(0)),
            Self::Packages(sources) => sources.context.root(module),
        }
    }
    pub(in crate::frontend) fn dependencies(
        &self,
        module: ModuleId,
    ) -> &[packages::PackageDependency] {
        match self {
            Self::Single(_) => &[],
            Self::Packages(sources) => sources.context.dependencies(module).unwrap_or(&[]),
        }
    }
    pub(in crate::frontend) fn dependency_target(&self, package: usize) -> Option<ModuleId> {
        match self {
            Self::Single(_) => None,
            Self::Packages(sources) => sources.context.target(package),
        }
    }
}
impl ProjectSources<parser::editor_query::EditorFile> {
    pub(in crate::frontend) fn editor_files(&self) -> &[parser::editor_query::EditorFile] {
        &self.programs
    }
    pub(in crate::frontend) fn editor_modules(&self) -> &[ModuleHeader] {
        &self.modules
    }
}

impl<S: ModuleSyntax> SourceSetBuilder<'_, S> {
    #[cfg(windows)]
    pub(super) fn read_verified_child_input(
        &mut self,
        child: &mut filesystem::VerifiedChild,
        path: &Path,
        display: &str,
        origin: Span,
        overlay: Option<&SourceOverlay<'_>>,
    ) -> Result<String, Box<Diagnostic>> {
        child.validate(origin)?;
        let result = match overlay.and_then(|buffers| buffers.text(path)) {
            Some(text) => self.read_overlay_source(text, Some(origin)),
            None => self.read_open_source(&mut child.file, display, Some(origin)),
        }?;
        child.validate(origin)?;
        Ok(result)
    }

    pub(super) fn read_source_input(
        &mut self,
        path: &Path,
        display: &str,
        origin: Option<Span>,
        overlay: Option<&SourceOverlay<'_>>,
    ) -> Result<String, Box<Diagnostic>> {
        let Some(text) = overlay.and_then(|buffers| buffers.text(path)) else {
            return self.read_source(path, display, origin);
        };
        self.read_overlay_source(text, origin)
    }

    fn read_overlay_source(
        &mut self,
        text: &str,
        origin: Option<Span>,
    ) -> Result<String, Box<Diagnostic>> {
        let remaining = self
            .limits
            .source_bytes
            .min(MAX_SOURCE_BYTES)
            .checked_sub(self.project.usage.source_bytes)
            .ok_or_else(|| overflow(origin))?;
        if text.len() > remaining {
            return Err(resource("aggregate source byte limit exceeded", origin));
        }
        let mut bytes = Vec::new();
        self.allocator
            .vector(&mut bytes, text.len(), "source bytes")
            .map_err(|error| reserve_error(error, origin))?;
        bytes.extend_from_slice(text.as_bytes());
        self.project.usage.source_bytes =
            add(self.project.usage.source_bytes, bytes.len(), origin)?;
        if bytes.starts_with(b"OXBC") {
            return Err(Diagnostic::new(
                "E0004",
                "source",
                "legacy OXBC artifacts are unavailable in typed-preview",
                origin,
            ));
        }
        // The borrowed input was already a Rust UTF-8 string; copying its bytes
        // does not change their validity or normalize any source positions.
        Ok(String::from_utf8(bytes).expect("editor buffer is UTF-8"))
    }
}
