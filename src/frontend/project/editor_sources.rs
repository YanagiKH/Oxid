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

    fn text(&self, path: &Path) -> Option<&str> {
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

impl SourceSetBuilder<'_> {
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
