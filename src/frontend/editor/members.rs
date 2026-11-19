//! Record fields are obtained from exact retained source declarations, never
//! from a spelling-only union of records or from a repaired executable AST.
use super::*;
use crate::frontend::{
    declaration_index::{
        editor_completion::{members::Record, Namespace},
        IndexLimits, WorkMeter,
    },
    parser::editor_query::members::{FieldName, MemberQuery},
    project::{editor_sources::QuerySources, ModuleId},
    source::{SourceFile, SourceFileId},
    PackageSpec,
};
use sha2::{Digest, Sha256};
use std::mem::size_of;

mod resources;
#[cfg(test)]
mod tests;

const MAX_CANDIDATES: usize = 4096;
const MAX_ITEMS: usize = 512;
const MAX_BYTES: usize = 1024 * 1024;
type Answer<T> = Result<T, Box<Diagnostic>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Identity([u8; 32]);

pub(crate) struct MemberCompletions {
    pub completions: EditorCompletions,
    pub identity: Option<Identity>,
    /// A graph that does not load this buffer supplies no opinion about it.
    /// A loaded but unresolved receiver must suppress a cross-graph result.
    pub present: bool,
    #[cfg(test)]
    charged: usize,
}

#[derive(Default)]
pub(crate) struct Merge {
    selected: Option<MemberCompletions>,
    refused: bool,
}
impl Merge {
    pub(crate) fn push(&mut self, next: MemberCompletions) {
        if !next.present {
            return;
        }
        let conflict = match &self.selected {
            Some(old) => old.identity != next.identity,
            None => false,
        };
        if next.identity.is_none() || conflict {
            self.refused = true;
            self.selected = None;
        } else if !self.refused && self.selected.is_none() {
            self.selected = Some(next);
        }
    }
    pub(crate) fn finish(self) -> Option<EditorCompletions> {
        self.selected.map(|selected| selected.completions)
    }
}

struct Budget {
    used: usize,
    limit: usize,
    at: Span,
}
impl Budget {
    fn charge(&mut self, bytes: usize) -> Answer<()> {
        let next = self
            .used
            .checked_add(bytes)
            .ok_or_else(|| failure("editor member size overflow", self.at))?;
        if next > self.limit {
            return Err(failure("editor member scratch limit exceeded", self.at));
        }
        self.used = next;
        Ok(())
    }
}
fn failure(message: &'static str, at: Span) -> Box<Diagnostic> {
    Diagnostic::new("E0400", "editor", message, Some(at))
}
fn invalid(at: Span) -> Box<Diagnostic> {
    Diagnostic::new(
        "E0500",
        "editor",
        "editor member source identity changed",
        Some(at),
    )
}

/// Route selection only. The lazy member parser validates tokens, body scope,
/// receiver binding and the complete source-generation association afterwards.
pub(crate) fn hint(text: &str, offset: usize) -> bool {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return false;
    }
    let bytes = text.as_bytes();
    let mut start = offset;
    while start > 0
        && (bytes[start - 1].is_ascii_alphanumeric()
            || bytes[start - 1] == b'_'
            || !bytes[start - 1].is_ascii())
    {
        start -= 1;
    }
    while start > 0 && bytes[start - 1].is_ascii_whitespace() {
        start -= 1;
    }
    start > 0 && bytes[start - 1] == b'.'
}

pub(crate) fn complete(
    packages: &[PackageSpec],
    entry: Option<&Path>,
    overlay: &SourceOverlay<'_>,
    path: &Path,
    offset: usize,
) -> Result<MemberCompletions, Vec<EditorDiagnostic>> {
    let session = QuerySources::load(packages, entry, overlay)
        .map_err(|error| diagnostics(&error.sources, &error.diagnostics))?;
    let work = WorkMeter::new(IndexLimits::default().work);
    complete_limited(
        &session,
        packages,
        path,
        offset,
        &work,
        &mut Allocator::default(),
        IndexLimits::default().scratch,
    )
    .map_err(|error| diagnostics(session.project().sources(), &[*error]))
}

fn empty(sources: &SourceMap, present: bool, charged: usize) -> MemberCompletions {
    #[cfg(not(test))]
    let _ = charged;
    MemberCompletions {
        completions: EditorCompletions {
            entries: Vec::new(),
            replacement: None,
            incomplete: false,
            files: sources.files().len(),
            bytes: sources.files().iter().map(|file| file.text().len()).sum(),
        },
        identity: None,
        present,
        #[cfg(test)]
        charged,
    }
}

fn compare(a: Span, b: Span, map: &SourceMap, work: &WorkMeter) -> Answer<std::cmp::Ordering> {
    let (a, b) = (map.text(a), map.text(b));
    work.debit(
        (a.len().min(b.len()) + 1) as u64,
        map.get(SourceFileId(0)).span(0, 0),
        "editor field comparison",
    )?;
    Ok(a.cmp(b))
}
fn sift(
    fields: &mut [FieldName],
    mut root: usize,
    end: usize,
    map: &SourceMap,
    work: &WorkMeter,
) -> Answer<()> {
    while let Some(left) = root
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .filter(|n| *n < end)
    {
        let mut child = left;
        if left + 1 < end && compare(fields[left].name, fields[left + 1].name, map, work)?.is_lt() {
            child += 1;
        }
        if !compare(fields[root].name, fields[child].name, map, work)?.is_lt() {
            break;
        }
        fields.swap(root, child);
        root = child;
    }
    Ok(())
}
fn sort(fields: &mut [FieldName], map: &SourceMap, work: &WorkMeter) -> Answer<()> {
    let length = fields.len();
    for root in (0..length / 2).rev() {
        sift(fields, root, length, map, work)?;
    }
    for end in (1..length).rev() {
        fields.swap(0, end);
        sift(fields, 0, end, map, work)?;
    }
    for pair in fields.windows(2) {
        if compare(pair[0].name, pair[1].name, map, work)?.is_eq() {
            return Err(Diagnostic::new(
                "E0201",
                "editor",
                "duplicate record field",
                Some(pair[1].name),
            ));
        }
    }
    Ok(())
}

fn matches(
    field: FieldName,
    private: bool,
    prefix: &str,
    map: &SourceMap,
    work: &WorkMeter,
) -> Answer<bool> {
    work.debit(1, field.name, "editor field candidate")?;
    if !field.public && !private {
        return Ok(false);
    }
    let name = map.text(field.name);
    work.debit(
        (name.len().min(prefix.len()) + 1) as u64,
        field.name,
        "editor field prefix",
    )?;
    Ok(name.starts_with(prefix))
}

fn constructor_matches(
    session: &QuerySources<'_>,
    query: &MemberQuery,
    fields: &[FieldName],
    private: bool,
    work: &WorkMeter,
) -> Answer<bool> {
    let Some(constructor) = query.constructor() else {
        return Ok(true);
    };
    let project = session.project();
    let file = &project.editor_files()[constructor.file.0];
    let source = project.sources().get(constructor.file);
    let mut values = file.constructor_fields(source, constructor, work)?;
    let mut count = 0usize;
    while values.next()?.is_some() {
        count += 1;
    }
    if count != fields.len() {
        return Ok(false);
    }
    for field in fields {
        if !field.public && !private {
            return Ok(false);
        }
        let mut values = file.constructor_fields(source, constructor, work)?;
        let mut found = 0usize;
        while let Some(value) = values.next()? {
            if compare(field.name, value.name, project.sources(), work)?.is_eq() {
                found += 1;
            }
        }
        if found != 1 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn digest_part(hash: &mut Sha256, bytes: &[u8], work: &WorkMeter, at: Span) -> Answer<()> {
    work.debit(
        (bytes.len() as u64)
            .checked_add(8)
            .ok_or_else(|| failure("editor identity size overflow", at))?,
        at,
        "editor record identity",
    )?;
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    Ok(())
}
fn identity(
    session: &QuerySources<'_>,
    packages: &[PackageSpec],
    record: Record<'_, '_>,
    fields: &[FieldName],
    private: bool,
    work: &WorkMeter,
) -> Answer<Identity> {
    let at = record.name();
    let root = session.root(record.module()).ok_or_else(|| invalid(at))?;
    let mut package = None;
    for (ordinal, spec) in packages.iter().enumerate() {
        work.debit(1, at, "editor record package identity")?;
        let target = if packages.len() == 1 {
            Some(ModuleId(0))
        } else {
            session.dependency_target(ordinal)
        };
        if target == Some(root) {
            package = Some(spec);
            break;
        }
    }
    let package = package.ok_or_else(|| invalid(at))?;
    let project = session.project();
    let source = project.sources().get(at.file);
    let mut hash = Sha256::new();
    digest_part(&mut hash, b"oxid-editor-record-v1", work, at)?;
    digest_part(
        &mut hash,
        package.root.to_str().ok_or_else(|| invalid(at))?.as_bytes(),
        work,
        at,
    )?;
    digest_part(&mut hash, source.text().as_bytes(), work, at)?;
    digest_part(&mut hash, &(at.start as u64).to_le_bytes(), work, at)?;
    digest_part(&mut hash, &(at.end as u64).to_le_bytes(), work, at)?;
    let mut module = Some(record.module());
    let mut depth = 0;
    while let Some(id) = module {
        work.debit(1, at, "editor record module identity")?;
        if depth > 32 {
            return Err(invalid(at));
        }
        let header = project
            .editor_modules()
            .get(id.0)
            .ok_or_else(|| invalid(at))?;
        let path = header
            .canonical_path
            .as_deref()
            .and_then(Path::to_str)
            .unwrap_or_else(|| project.sources().get(header.file).path());
        digest_part(&mut hash, path.as_bytes(), work, at)?;
        digest_part(&mut hash, &[u8::from(header.public.is_some())], work, at)?;
        module = header.parent;
        depth += 1;
    }
    for field in fields {
        work.debit(1, field.name, "editor record field identity")?;
        if field.public || private {
            digest_part(
                &mut hash,
                project.sources().text(field.name).as_bytes(),
                work,
                at,
            )?;
        }
    }
    Ok(Identity(hash.finalize().into()))
}

fn copy_string(
    text: &str,
    allocator: &mut Allocator,
    at: Span,
    work: &WorkMeter,
) -> Answer<String> {
    work.debit(text.len() as u64 + 1, at, "editor field result bytes")?;
    let mut result = String::new();
    allocator
        .string(&mut result, text.len(), "editor field result string")
        .map_err(|_| failure("cannot reserve editor field result", at))?;
    if result.capacity() != text.len() {
        return Err(failure(
            "editor field result capacity exceeded admission",
            at,
        ));
    }
    result.push_str(text);
    Ok(result)
}

type Positions = ((usize, usize), (usize, usize));
fn positions(source: &SourceFile, span: Span, work: &WorkMeter) -> Answer<Positions> {
    if span.start > span.end
        || span.end > source.text().len()
        || !source.text().is_char_boundary(span.start)
        || !source.text().is_char_boundary(span.end)
    {
        return Err(invalid(span));
    }
    // One exact retained prefix, scanned once. Charge its raw byte length before
    // visiting UTF-8 characters; an earlier non-BMP scalar counts as two columns.
    work.debit(span.end as u64 + 1, span, "editor field UTF-16 positions")?;
    let (mut line, mut column) = (0usize, 0usize);
    let mut start = None;
    for (offset, scalar) in source.text()[..span.end].char_indices() {
        if offset == span.start {
            start = Some((line, column));
        }
        if scalar == '\n' {
            line += 1;
            column = 0;
        } else {
            column += scalar.len_utf16();
        }
    }
    let end = (line, column);
    let start = if span.start == span.end {
        end
    } else {
        start.ok_or_else(|| invalid(span))?
    };
    Ok((start, end))
}

fn complete_limited(
    session: &QuerySources<'_>,
    packages: &[PackageSpec],
    path: &Path,
    offset: usize,
    work: &WorkMeter,
    allocator: &mut Allocator,
    limit: u64,
) -> Answer<MemberCompletions> {
    let project = session.project();
    let map = project.sources();
    let at = map.get(SourceFileId(0)).span(0, 0);
    let mut budget = Budget {
        used: 0,
        limit: limit.min(IndexLimits::default().scratch) as usize,
        at,
    };
    budget.charge(resources::fixed_bytes())?;
    let mut selected = None;
    for (ordinal, source) in map.files().iter().enumerate() {
        work.debit(1, at, "editor member file")?;
        // Both arguments come from the admitted canonical project graph. No
        // filesystem reopen or speculative path lookup is performed here.
        if Path::new(source.path()) == path
            || project.editor_modules()[ordinal].canonical_path.as_deref() == Some(path)
        {
            selected = Some((ordinal, source));
            break;
        }
    }
    let Some((ordinal, source)) = selected else {
        return Ok(empty(map, false, budget.used));
    };
    let Some(query) = project.editor_files()[ordinal].member_query(source, offset, work)? else {
        return Ok(empty(map, true, budget.used));
    };
    let namespace = Namespace::build_for_members(session, work, allocator, budget.used, limit)?;
    budget.charge(
        namespace
            .member_namespace_bytes()
            .ok_or_else(|| failure("editor namespace size overflow", at))?,
    )?;
    let requester = ModuleId(ordinal);
    let Some(record) = namespace.member_record(requester, query.path())? else {
        return Ok(empty(map, true, budget.used));
    };
    if let Some(annotation) = query.annotation() {
        let Some(declared) = namespace.member_record(requester, annotation)? else {
            return Ok(empty(map, true, budget.used));
        };
        if !record.same(declared) {
            return Ok(empty(map, true, budget.used));
        }
    }
    let Some(mut reader) = namespace.member_fields(record)? else {
        return Ok(empty(map, true, budget.used));
    };
    let mut count = 0usize;
    while reader.next()?.is_some() {
        count = count
            .checked_add(1)
            .ok_or_else(|| failure("editor field count overflow", at))?;
    }
    budget.charge(
        count
            .checked_mul(size_of::<FieldName>())
            .ok_or_else(|| failure("editor field size overflow", at))?,
    )?;
    let mut fields = Vec::new();
    allocator
        .vector_exact(&mut fields, count, "editor record fields")
        .map_err(|_| failure("cannot reserve editor record fields", at))?;
    if fields.capacity() != count {
        return Err(failure("editor field capacity exceeded admission", at));
    }
    let mut reader = namespace
        .member_fields(record)?
        .ok_or_else(|| invalid(at))?;
    while let Some(field) = reader.next()? {
        fields.push(field);
    }
    if fields.len() != count {
        return Err(invalid(at));
    }
    sort(&mut fields, map, work)?;
    let private = namespace.member_private_visible(record, requester)?;
    let constructor = match constructor_matches(session, &query, &fields, private, work) {
        Ok(matched) => matched,
        Err(error) if error.code == "E0101" => false,
        Err(error) => return Err(error),
    };
    if !constructor {
        return Ok(empty(map, true, budget.used));
    }
    let prefix = &source.text()[query.replacement().start..query.cursor()];
    let (mut candidates, mut names, mut output_bytes) = (0usize, 0usize, 0usize);
    for &field in &fields {
        if matches(field, private, prefix, map, work)? {
            candidates += 1;
            names = names
                .checked_add(map.text(field.name).len())
                .ok_or_else(|| failure("editor field size overflow", at))?;
            if candidates > MAX_CANDIDATES || names > MAX_BYTES {
                return Err(failure(
                    "too many completion candidates; enter a longer name",
                    at,
                ));
            }
            if candidates <= MAX_ITEMS {
                output_bytes = names;
            }
        }
    }
    let count = candidates.min(MAX_ITEMS);
    let payload = count
        .checked_mul(size_of::<EditorCompletionItem>())
        .and_then(|bytes| bytes.checked_add(output_bytes))
        .and_then(|bytes| bytes.checked_add(source.path().len()))
        .ok_or_else(|| failure("editor field result size overflow", at))?;
    budget.charge(payload)?;
    let identity = identity(session, packages, record, &fields, private, work)?;
    let replacement = query.replacement();
    let (start, end) = positions(source, replacement, work)?;
    let mut entries = Vec::new();
    allocator
        .vector_exact(&mut entries, count, "editor field results")
        .map_err(|_| failure("cannot reserve editor field results", at))?;
    if entries.capacity() != count {
        return Err(failure(
            "editor field result capacity exceeded admission",
            at,
        ));
    }
    for &field in &fields {
        if entries.len() == count {
            break;
        }
        if matches(field, private, prefix, map, work)? {
            entries.push(EditorCompletionItem {
                label: copy_string(map.text(field.name), allocator, at, work)?,
                kind: 5,
                detail: "record field",
            });
        }
    }
    let location = EditorLocation {
        path: copy_string(source.path(), allocator, at, work)?,
        start,
        end,
    };
    Ok(MemberCompletions {
        completions: EditorCompletions {
            entries,
            replacement: Some(location),
            incomplete: candidates > MAX_ITEMS,
            files: map.files().len(),
            bytes: map.files().iter().map(|source| source.text().len()).sum(),
        },
        identity: Some(identity),
        present: true,
        #[cfg(test)]
        charged: budget.used,
    })
}
