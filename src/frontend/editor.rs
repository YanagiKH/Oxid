//! Read-only editor operations over retained typed sources.
use super::{
    diagnostic::Diagnostic,
    project::budget::Allocator,
    source::{SourceMap, Span, MAX_SOURCE_BYTES},
};
use std::path::Path;

pub(super) mod navigation;
mod positions;

pub(crate) use super::project::editor_sources::SourceOverlay;

#[derive(Clone, Debug)]
pub(crate) struct EditorLocation {
    pub path: String,
    pub start: (usize, usize),
    pub end: (usize, usize),
}

#[derive(Clone, Debug)]
pub(crate) struct EditorDiagnostic {
    pub code: &'static str,
    pub message: String,
    pub location: Option<EditorLocation>,
    pub related: Vec<(EditorLocation, String)>,
}

pub(crate) struct EditorSymbol {
    pub name: String,
    pub kind: u8,
    pub location: EditorLocation,
    pub container: Option<String>,
}

pub(crate) struct EditorSymbols {
    pub entries: Vec<EditorSymbol>,
    pub files: usize,
    pub bytes: usize,
}

pub(crate) struct EditorDefinition {
    pub location: Option<EditorLocation>,
    pub files: usize,
    pub bytes: usize,
}

#[derive(Debug)]
pub(crate) struct EditorCheck {
    pub diagnostics: Vec<EditorDiagnostic>,
    pub files: Vec<String>,
    pub source_bytes: usize,
    pub sources_complete: bool,
}

impl std::ops::Deref for EditorCheck {
    type Target = [EditorDiagnostic];
    fn deref(&self) -> &Self::Target {
        &self.diagnostics
    }
}

pub(super) fn checked_sources(
    sources: &SourceMap,
    errors: &[Diagnostic],
    complete: bool,
) -> EditorCheck {
    EditorCheck {
        diagnostics: diagnostics(sources, errors),
        files: sources
            .files()
            .iter()
            .map(|source| source.path().into())
            .collect(),
        source_bytes: sources
            .files()
            .iter()
            .map(|source| source.text().len())
            .sum(),
        sources_complete: complete,
    }
}

pub(super) fn location(sources: &SourceMap, span: Span) -> Option<EditorLocation> {
    if !sources.is_valid_span(span) {
        return None;
    }
    let source = sources.get(span.file);
    Some(EditorLocation {
        path: source.path().into(),
        start: source.editor_position(span.start),
        end: source.editor_position(span.end),
    })
}

pub(super) fn diagnostics(sources: &SourceMap, errors: &[Diagnostic]) -> Vec<EditorDiagnostic> {
    errors
        .iter()
        .take(256)
        .map(|error| {
            let mut message = clipped(&error.message, 8192).to_owned();
            for note in &error.notes {
                if message.len() >= 8192 {
                    break;
                }
                message.push('\n');
                message.push_str(clipped(note, 8192 - message.len()));
            }
            EditorDiagnostic {
                code: error.code,
                message,
                location: error.primary.and_then(|span| location(sources, span)),
                related: error
                    .secondary
                    .iter()
                    .take(8)
                    .filter_map(|(span, message)| {
                        Some((location(sources, *span)?, clipped(message, 2048).to_owned()))
                    })
                    .collect(),
            }
        })
        .collect()
}

fn clipped(text: &str, maximum: usize) -> &str {
    let mut end = text.len().min(maximum);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
pub(crate) fn check(packages: &[super::PackageSpec], overlay: &SourceOverlay<'_>) -> EditorCheck {
    check_entry(packages, None, overlay)
}

pub(crate) fn check_entry(
    packages: &[super::PackageSpec],
    entry: Option<&Path>,
    overlay: &SourceOverlay<'_>,
) -> EditorCheck {
    super::driver::editor_check(packages, entry, overlay)
}

pub(crate) fn definition(
    packages: &[super::PackageSpec],
    entry: Option<&Path>,
    overlay: &SourceOverlay<'_>,
    path: &Path,
    offset: usize,
) -> Result<EditorDefinition, Vec<EditorDiagnostic>> {
    super::driver::editor_definition(packages, entry, overlay, path, offset)
}

pub(crate) fn symbols(
    packages: &[super::PackageSpec],
    entry: Option<&Path>,
    overlay: &SourceOverlay<'_>,
    path: Option<&Path>,
    query: &str,
) -> Result<EditorSymbols, Vec<EditorDiagnostic>> {
    super::driver::editor_symbols(packages, entry, overlay, path, query)
}

pub(crate) fn format(path: &Path, text: &str) -> Result<String, Vec<EditorDiagnostic>> {
    let mut sources = SourceMap::new();
    let mut allocator = Allocator::default();
    let fail = |message| {
        diagnostics(
            &SourceMap::new(),
            &[*Diagnostic::new("E0400", "editor", message, None)],
        )
    };
    if text.len() > MAX_SOURCE_BYTES {
        return Err(fail("source exceeds editor byte limit"));
    }
    let path = path
        .to_str()
        .ok_or_else(|| fail("source path must be UTF-8"))?;
    if path.len() > 4096 {
        return Err(fail("source path exceeds editor byte limit"));
    }
    let mut display = String::new();
    allocator
        .string(&mut display, path.len(), "editor source path")
        .map_err(|_| fail("cannot reserve editor path"))?;
    display.push_str(path);
    let mut body = String::new();
    allocator
        .string(&mut body, text.len(), "editor source text")
        .map_err(|_| fail("cannot reserve editor source"))?;
    body.push_str(text);
    let id = sources
        .try_add(display, body, &mut allocator)
        .map_err(|_| fail("cannot reserve editor source index"))?;
    super::format::format_source(sources.get(id)).map_err(|errors| diagnostics(&sources, &errors))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "oxid-editor-雪-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root.canonicalize().unwrap())
        }
        fn write(&self, path: &str, text: &str) -> PathBuf {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, text).unwrap();
            path
        }
        fn package(
            &self,
            root: &str,
            entry: &str,
            dependencies: Vec<super::super::PackageDependency>,
        ) -> super::super::PackageSpec {
            super::super::PackageSpec {
                root: self.0.join(root).canonicalize().unwrap(),
                entry: self.0.join(root).join(entry).canonicalize().unwrap(),
                dependencies,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if std::thread::panicking() {
                eprintln!("retained editor fixture: {}", self.0.display());
            } else {
                fs::remove_dir_all(&self.0).unwrap();
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unsaved_module_replaces_bad_saved_source_without_execution_or_writes() {
        let fixture = Fixture::new();
        fixture.write(
            "app/main.ox",
            "mod helper;fn main()->i32{return crate::helper::answer()+1/0;}",
        );
        let disk = "pub fn answer()->i32{return missing;}";
        let helper = fixture.write("app/helper.ox", disk);
        let packages = [fixture.package("app", "main.ox", vec![])];
        let entries = [(helper.as_path(), "pub fn answer()->i32{return 42;}")];
        let overlay = SourceOverlay::new(&entries).unwrap();
        assert!(check(&packages, &overlay).is_empty());
        assert_eq!(fs::read_to_string(helper).unwrap(), disk);
        assert!(!fixture.0.join("app/.oxid").exists());
        assert!(!fixture.0.join("app/oxid.lock").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn dependency_buffers_share_the_ordinary_type_and_namespace_checker() {
        let fixture = Fixture::new();
        fixture.write(
            "app/src/main.ox",
            "use crate::math::answer;fn main()->i32{return answer();}",
        );
        let library = fixture.write("math/src/lib.ox", "pub fn answer()->bool{return true;}");
        let packages = [
            fixture.package(
                "app",
                "src/main.ox",
                vec![super::super::PackageDependency {
                    alias: "math".into(),
                    package: 1,
                }],
            ),
            fixture.package("math", "src/lib.ox", vec![]),
        ];
        let good = [(library.as_path(), "pub fn answer()->i32{return 42;}")];
        assert!(check(&packages, &SourceOverlay::new(&good).unwrap()).is_empty());
        let bad = [(library.as_path(), "pub fn answer()->bool{return true;}")];
        let errors = check(&packages, &SourceOverlay::new(&bad).unwrap());
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|error| error
            .location
            .as_ref()
            .is_some_and(|location| location.path.ends_with("app/src/main.ox"))));
        assert_eq!(
            fs::read_to_string(library).unwrap(),
            "pub fn answer()->bool{return true;}"
        );
    }

    #[test]
    fn diagnostics_use_utf16_positions_from_the_unsaved_buffer() {
        let fixture = Fixture::new();
        let path = fixture.write("app/main.ox", "fn main()->i32{return 42;}");
        let text = "fn main()->i32{let c='🦀';return missing;}";
        let entries = [(path.as_path(), text)];
        let packages = [fixture.package("app", "main.ox", vec![])];
        let errors = check(&packages, &SourceOverlay::new(&entries).unwrap());
        let location = errors
            .iter()
            .find_map(|error| error.location.as_ref())
            .unwrap();
        let offset = text.find("missing").unwrap();
        assert_eq!(location.start, (0, text[..offset].encode_utf16().count()));
        assert_eq!(location.end, (0, text[..offset + 7].encode_utf16().count()));
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "fn main()->i32{return 42;}"
        );
    }

    #[test]
    fn formatting_uses_buffer_text_is_idempotent_and_preserves_saved_source() {
        let fixture = Fixture::new();
        let path = fixture.write("main.ox", "saved source remains unchanged");
        let text = "fn main()->i32{let c='🦀';return 42;}\r\n";
        let formatted = format(&path, text).unwrap();
        assert!(formatted.contains("fn main() -> i32 {"));
        assert_eq!(format(&path, &formatted).unwrap(), formatted);
        assert!(format(&path, "fn main()->i32{let n=;}").is_err());
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "saved source remains unchanged"
        );
    }

    #[test]
    fn buffer_limits_and_duplicate_identities_fail_before_source_loading() {
        let fixture = Fixture::new();
        let path = fixture.0.join("main.ox");
        let duplicated = [(path.as_path(), "a"), (path.as_path(), "b")];
        assert!(SourceOverlay::new(&duplicated).is_err());
        let large = "x".repeat(MAX_SOURCE_BYTES + 1);
        let entries = [(path.as_path(), large.as_str())];
        assert!(SourceOverlay::new(&entries).is_err());
        let one = "x".repeat(MAX_SOURCE_BYTES);
        let names: Vec<_> = (0..17)
            .map(|index| fixture.0.join(format!("source{index}.ox")))
            .collect();
        let entries: Vec<_> = names
            .iter()
            .map(|path| (path.as_path(), one.as_str()))
            .collect();
        assert!(SourceOverlay::new(&entries).is_err());
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
    }
}
