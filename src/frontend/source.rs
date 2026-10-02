//! Immutable source files and half-open, UTF-8 byte spans.

pub const MAX_SOURCE_BYTES: usize = 1_048_576;

// Never reuse a parser/source association while an independently owned AST
// might still exist. This counter is not a declaration or language identity.
static NEXT_SOURCE_IDENTITY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
fn next_source_identity() -> Option<u64> {
    allocate_source_identity(&NEXT_SOURCE_IDENTITY)
}
fn allocate_source_identity(counter: &std::sync::atomic::AtomicU64) -> Option<u64> {
    use std::sync::atomic::Ordering::Relaxed;
    let mut current = counter.load(Relaxed);
    loop {
        let next = current.checked_add(1)?;
        match counter.compare_exchange_weak(current, next, Relaxed, Relaxed) {
            Ok(previous) => return Some(previous),
            Err(observed) => current = observed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceFileId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub file: SourceFileId,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

#[derive(Debug)]
pub struct SourceFile {
    identity: u64,
    id: SourceFileId,
    path: String,
    text: String,
    line_starts: Vec<usize>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a source without normalizing its bytes or display path.
    /// The driver enforces `MAX_SOURCE_BYTES` before adding source text.
    #[cfg(test)]
    pub fn add(&mut self, path: String, text: String) -> SourceFileId {
        let id = SourceFileId(self.files.len());
        let mut line_starts = vec![0];
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter_map(|(offset, byte)| (byte == b'\n').then_some(offset + 1)),
        );
        self.files.push(SourceFile {
            identity: next_source_identity().expect("source identity space exhausted"),
            id,
            path,
            text,
            line_starts,
        });
        id
    }

    /// The loader counts line entries before any new source table allocation.
    pub(super) fn try_add(
        &mut self,
        path: String,
        text: String,
        allocator: &mut super::project::budget::Allocator,
    ) -> Result<SourceFileId, super::project::budget::ReserveFailure> {
        use super::project::budget::ReserveFailure;
        let count = text
            .bytes()
            .filter(|&byte| byte == b'\n')
            .count()
            .checked_add(1)
            .ok_or(ReserveFailure::Overflow)?;
        let identity = next_source_identity().ok_or(ReserveFailure::Overflow)?;
        count
            .checked_mul(size_of::<usize>())
            .ok_or(ReserveFailure::Overflow)?;
        self.files
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_mul(size_of::<SourceFile>()))
            .ok_or(ReserveFailure::Overflow)?;
        let mut line_starts = Vec::new();
        allocator.vector(&mut line_starts, count, "line starts")?;
        allocator.vector(&mut self.files, 1, "source files")?;
        line_starts.push(0);
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter_map(|(offset, byte)| (byte == b'\n').then_some(offset + 1)),
        );
        let id = SourceFileId(self.files.len());
        self.files.push(SourceFile {
            identity,
            id,
            path,
            text,
            line_starts,
        });
        Ok(id)
    }

    pub(super) fn try_text(&self, span: Span) -> Option<&str> {
        self.files.get(span.file.0)?.try_text(span)
    }

    /// Trusted frontend origins still validate their file and UTF-8 boundaries.
    pub(super) fn text(&self, span: Span) -> &str {
        self.try_text(span).expect("invalid source-map span")
    }

    pub(super) fn files(&self) -> &[SourceFile] {
        &self.files
    }

    /// Fallible validation for untrusted internal IR; never index or render first.
    pub(super) fn is_valid_span(&self, span: Span) -> bool {
        self.files.get(span.file.0).is_some_and(|file| {
            file.id == span.file
                && span.start <= span.end
                && span.end <= file.text.len()
                && file.text.is_char_boundary(span.start)
                && file.text.is_char_boundary(span.end)
        })
    }

    pub fn get(&self, id: SourceFileId) -> &SourceFile {
        &self.files[id.0]
    }
}

impl SourceFile {
    pub(super) fn identity(&self) -> u64 {
        self.identity
    }
    pub(super) fn try_text(&self, span: Span) -> Option<&str> {
        if span.file != self.id || span.start > span.end {
            return None;
        }
        self.text.get(span.start..span.end)
    }

    /// A file-local parser/resolver view cannot reinterpret a foreign span.
    pub(super) fn text_at(&self, span: Span) -> &str {
        self.try_text(span)
            .expect("invalid or foreign source-file span")
    }

    pub(super) fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Construct a half-open span, including an empty span at EOF.
    /// Invalid offsets indicate an internal frontend bug.
    pub fn span(&self, start: usize, end: usize) -> Span {
        assert!(start <= end, "span start exceeds end");
        self.validate_offset(start);
        self.validate_offset(end);
        Span {
            file: self.id,
            start,
            end,
        }
    }

    /// Convert a byte offset to one-based line and Unicode scalar column.
    /// LF starts a new line, including in CRLF. At the LF byte of a CRLF pair,
    /// the preceding CR counts as a scalar on the old line. Tabs, combining
    /// marks, and wide characters each count as one scalar, not display cells.
    pub fn location(&self, offset: usize) -> (usize, usize) {
        self.validate_offset(offset);
        let line = self.line_starts.partition_point(|&start| start <= offset) - 1;
        let column = self.text[self.line_starts[line]..offset].chars().count() + 1;
        (line + 1, column)
    }

    fn validate_offset(&self, offset: usize) {
        assert!(offset <= self.text.len(), "source offset out of bounds");
        assert!(
            self.text.is_char_boundary(offset),
            "source offset is not a UTF-8 boundary"
        );
    }
}

/// Borrowed immutable source owner. Single is an explicit compatibility adapter.
#[derive(Clone, Copy, Debug)]
pub(super) enum SourceView<'a> {
    #[allow(dead_code)] // Existing private one-file qualification adapters.
    Single(&'a SourceFile),
    Map(&'a SourceMap),
}
impl<'a> SourceView<'a> {
    pub(super) fn text(self, span: Span) -> &'a str {
        match self {
            Self::Single(file) => file.text_at(span),
            Self::Map(sources) => sources.text(span),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_identity_exhaustion_never_wraps_or_reuses() {
        let counter = std::sync::atomic::AtomicU64::new(u64::MAX - 1);
        assert_eq!(allocate_source_identity(&counter), Some(u64::MAX - 1));
        assert_eq!(allocate_source_identity(&counter), None);
        assert_eq!(allocate_source_identity(&counter), None);
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn concurrent_source_identities_remain_unique_through_exhaustion() {
        use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
        for (initial, successes) in [(1, 256), (u64::MAX - 4, 4)] {
            let counter = AtomicU64::new(initial);
            let mut identities = std::thread::scope(|scope| {
                let workers: Vec<_> = (0..4)
                    .map(|_| {
                        scope.spawn(|| {
                            (0..64)
                                .filter_map(|_| allocate_source_identity(&counter))
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                workers
                    .into_iter()
                    .flat_map(|worker| worker.join().unwrap())
                    .collect::<Vec<_>>()
            });
            identities.sort_unstable();
            assert_eq!(
                identities,
                (initial..initial + successes).collect::<Vec<_>>()
            );
            assert_eq!(counter.load(Relaxed), initial + successes);
        }
    }

    fn file(text: &str) -> SourceMap {
        let mut sources = SourceMap::new();
        sources.add("example.ox".into(), text.into());
        sources
    }

    #[test]
    fn source_map_preserves_text_paths_and_distinct_file_identity() {
        let mut sources = SourceMap::new();
        let first = sources.add("same.ox".into(), "first".into());
        let second = sources.add("same.ox".into(), "second".into());
        assert_eq!(first, SourceFileId(0));
        assert_eq!(second, SourceFileId(1));
        assert_eq!(sources.get(first).text(), "first");
        assert_eq!(sources.get(second).text(), "second");
        assert_eq!(sources.get(second).path(), "same.ox");
        assert_eq!(sources.get(second).span(0, 6).file, second);
    }

    #[test]
    fn locations_are_one_based_unicode_scalar_columns_with_byte_offsets() {
        let sources = file("aé🦀e\u{301}\nz");
        let source = sources.get(SourceFileId(0));
        for (offset, expected) in [
            (0, (1, 1)),
            (1, (1, 2)),
            (3, (1, 3)),
            (7, (1, 4)),
            (8, (1, 5)),
            (10, (1, 6)),
            (11, (2, 1)),
            (12, (2, 2)),
        ] {
            assert_eq!(source.location(offset), expected, "byte offset {offset}");
        }
    }

    #[test]
    fn crlf_is_one_line_break_and_eof_after_it_is_next_line() {
        let sources = file("é\r\nx\r\n");
        let source = sources.get(SourceFileId(0));
        assert_eq!(source.location(2), (1, 2));
        assert_eq!(source.location(3), (1, 3));
        assert_eq!(source.location(4), (2, 1));
        assert_eq!(source.location(7), (3, 1));
        assert_eq!(
            source.span(7, 7),
            Span {
                file: SourceFileId(0),
                start: 7,
                end: 7
            }
        );
    }

    #[test]
    fn empty_source_and_eof_spans_are_valid() {
        let sources = file("");
        let source = sources.get(SourceFileId(0));
        assert_eq!(source.location(0), (1, 1));
        assert_eq!(
            source.span(0, 0),
            Span {
                file: SourceFileId(0),
                start: 0,
                end: 0
            }
        );
    }

    #[test]
    fn span_preserves_half_open_byte_range() {
        let sources = file("a🦀b");
        let source = sources.get(SourceFileId(0));
        let span = source.span(1, 5);
        assert_eq!(&source.text()[span.start..span.end], "🦀");
    }

    #[test]
    #[should_panic(expected = "span start exceeds end")]
    fn rejects_reversed_span() {
        file("abc").get(SourceFileId(0)).span(2, 1);
    }

    #[test]
    #[should_panic(expected = "source offset out of bounds")]
    fn rejects_span_after_eof() {
        file("abc").get(SourceFileId(0)).span(0, 4);
    }

    #[test]
    #[should_panic(expected = "source offset is not a UTF-8 boundary")]
    fn rejects_span_start_inside_codepoint() {
        file("é").get(SourceFileId(0)).span(1, 2);
    }

    #[test]
    #[should_panic(expected = "source offset is not a UTF-8 boundary")]
    fn rejects_span_end_inside_codepoint() {
        file("é").get(SourceFileId(0)).span(0, 1);
    }

    #[test]
    #[should_panic(expected = "source offset out of bounds")]
    fn rejects_location_after_eof() {
        file("abc").get(SourceFileId(0)).location(4);
    }

    #[test]
    #[should_panic(expected = "source offset is not a UTF-8 boundary")]
    fn rejects_location_inside_codepoint() {
        file("é").get(SourceFileId(0)).location(1);
    }

    #[test]
    #[should_panic]
    fn rejects_unknown_file_id() {
        file("abc").get(SourceFileId(1));
    }
}
