//! Name completion uses original syntax scopes and unsaved source positions.
use super::*;
use crate::frontend::{
    ast,
    declaration_index::SourceOwner,
    lexer::{self, Kind, Token},
    project::ModuleId,
    source::SourceFile,
};

const MAX_CANDIDATES: usize = 4096;
const MAX_ITEMS: usize = 512;
const MAX_BYTES: usize = 1024 * 1024;
const WORDS: &[(&str, u8, &str)] = &[
    ("fn", 14, "keyword"),
    ("struct", 14, "keyword"),
    ("enum", 14, "keyword"),
    ("let", 14, "keyword"),
    ("mut", 14, "keyword"),
    ("return", 14, "keyword"),
    ("if", 14, "keyword"),
    ("else", 14, "keyword"),
    ("while", 14, "keyword"),
    ("for", 14, "keyword"),
    ("in", 14, "keyword"),
    ("break", 14, "keyword"),
    ("continue", 14, "keyword"),
    ("match", 14, "keyword"),
    ("mod", 14, "keyword"),
    ("use", 14, "keyword"),
    ("pub", 14, "keyword"),
    ("as", 14, "keyword"),
    ("true", 14, "boolean"),
    ("false", 14, "boolean"),
    ("bool", 25, "type"),
    ("char", 25, "type"),
    ("i8", 25, "type"),
    ("i16", 25, "type"),
    ("i32", 25, "type"),
    ("i64", 25, "type"),
    ("i128", 25, "type"),
    ("isize", 25, "type"),
    ("u8", 25, "type"),
    ("u16", 25, "type"),
    ("u32", 25, "type"),
    ("u64", 25, "type"),
    ("u128", 25, "type"),
    ("usize", 25, "type"),
    ("f32", 25, "type"),
    ("f64", 25, "type"),
];

fn failure(message: &'static str) -> Vec<Diagnostic> {
    vec![*Diagnostic::new("E0400", "editor", message, None)]
}
struct Candidate<'a> {
    label: &'a str,
    kind: u8,
    detail: &'static str,
    priority: (u8, usize),
}
struct Candidates<'a> {
    prefix: &'a str,
    entries: Vec<Candidate<'a>>,
    bytes: usize,
}
impl<'a> Candidates<'a> {
    fn add(
        &mut self,
        label: &'a str,
        kind: u8,
        detail: &'static str,
        priority: (u8, usize),
    ) -> Result<(), Vec<Diagnostic>> {
        if !label.starts_with(self.prefix) {
            return Ok(());
        }
        self.bytes = self
            .bytes
            .checked_add(label.len())
            .ok_or_else(|| failure("completion size overflow"))?;
        if self.entries.len() >= MAX_CANDIDATES || self.bytes > MAX_BYTES {
            return Err(failure(
                "too many completion candidates; enter a longer name",
            ));
        }
        self.entries
            .try_reserve(1)
            .map_err(|_| failure("cannot reserve completion candidates"))?;
        self.entries.push(Candidate {
            label,
            kind,
            detail,
            priority,
        });
        Ok(())
    }
    fn finish(mut self) -> Result<(Vec<EditorCompletionItem>, bool), Vec<Diagnostic>> {
        self.entries.sort_unstable_by(|a, b| {
            a.label
                .cmp(b.label)
                .then_with(|| b.priority.cmp(&a.priority))
        });
        self.entries.dedup_by(|a, b| a.label == b.label);
        let incomplete = self.entries.len() > MAX_ITEMS;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(self.entries.len().min(MAX_ITEMS))
            .map_err(|_| failure("cannot reserve completion results"))?;
        for candidate in self.entries.into_iter().take(MAX_ITEMS) {
            let mut label = String::new();
            label
                .try_reserve_exact(candidate.label.len())
                .map_err(|_| failure("cannot reserve completion name"))?;
            label.push_str(candidate.label);
            entries.push(EditorCompletionItem {
                label,
                kind: candidate.kind,
                detail: candidate.detail,
            });
        }
        Ok((entries, incomplete))
    }
}
fn same_path(source: &str, path: &Path) -> bool {
    Path::new(source) == path
        || Path::new(source)
            .canonicalize()
            .is_ok_and(|source| source == path)
}
fn within(block: &ast::BodyBlock, offset: usize) -> bool {
    block.span.start <= offset && offset <= block.end.start
}
fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// This only selects a query route over the current buffer; it grants no name
/// or source authority. The admitted query parser rechecks canonical tokens.
pub(in crate::frontend) fn qualified_hint(text: &str, offset: usize) -> bool {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return false;
    }
    let bytes = text.as_bytes();
    let mut start = offset;
    while start > 0 && word(bytes[start - 1]) {
        start -= 1;
    }
    while start > 0 && bytes[start - 1].is_ascii_whitespace() {
        start -= 1;
    }
    start >= 2 && &bytes[start - 2..start] == b"::"
}

pub(in crate::frontend) fn complete_qualified(
    session: &crate::frontend::project::editor_sources::QuerySources<'_>,
    path: &Path,
    offset: usize,
) -> Result<EditorCompletions, Vec<Diagnostic>> {
    use crate::frontend::declaration_index::{
        editor_completion::Namespace, IndexLimits, WorkMeter,
    };
    use crate::frontend::parser::editor_query::DeclarationKind;
    let project = session.project();
    let sources = project.sources();
    let Some((ordinal, source)) = sources
        .files()
        .iter()
        .enumerate()
        .find(|(_, source)| same_path(source.path(), path))
    else {
        return Ok(empty(sources));
    };
    let query = project.editor_files()[ordinal]
        .query(source, offset)
        .map_err(|error| vec![*error])?;
    let Some(query) = query else {
        return Ok(empty(sources));
    };
    let work = WorkMeter::new(IndexLimits::default().work);
    let mut allocator = Allocator::default();
    let namespace =
        Namespace::build(session, &work, &mut allocator).map_err(|error| vec![*error])?;
    let mut candidates = Candidates {
        prefix: &source.text()[query.replacement().start..query.cursor()],
        entries: Vec::new(),
        bytes: 0,
    };
    namespace
        .visit(ModuleId(ordinal), &query, |label, kind| {
            let (kind, detail) = match kind {
                DeclarationKind::Function => (3, "function"),
                DeclarationKind::Record => (22, "record"),
                DeclarationKind::Enumeration => (13, "enumeration"),
                DeclarationKind::Module => (9, "module"),
            };
            work.debit(
                (label.len().min(candidates.prefix.len()) + 1) as u64,
                query.replacement(),
                "editor candidate prefix",
            )?;
            candidates
                .add(label, kind, detail, (1, 0))
                .map_err(|errors| {
                    Box::new(
                        errors
                            .into_iter()
                            .next()
                            .expect("completion allocation error"),
                    )
                })
        })
        .map_err(|error| vec![*error])?;
    finish(sources, query.replacement(), candidates)
}
fn context(source: &SourceFile, tokens: &[Token], offset: usize) -> Option<Span> {
    let text = source.text();
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }
    for token in tokens {
        if token.span.start > offset {
            break;
        }
        let body = source.text_at(token.span);
        let comment =
            token.kind == Kind::Trivia && (body.starts_with("//") || body.starts_with("/*"));
        if token.span.start <= offset
            && (offset < token.span.end || (body.starts_with("//") && offset == token.span.end))
            && (comment
                || token.kind == Kind::String
                || token.kind == Kind::Number
                || body.starts_with('\''))
        {
            return None;
        }
    }
    let bytes = text.as_bytes();
    let mut start = offset;
    while start > 0 && word(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = offset;
    while end < bytes.len() && word(bytes[end]) {
        end += 1;
    }
    let mut before = tokens
        .iter()
        .rev()
        .filter(|token| token.span.end <= start && token.kind != Kind::Trivia);
    if let Some(previous) = before.next() {
        if previous.kind == Kind::Dot
            || (previous.kind == Kind::Colon
                && before.next().is_some_and(|token| token.kind == Kind::Colon))
        {
            return None;
        }
    }
    Some(source.span(start, end))
}
fn empty(sources: &SourceMap) -> EditorCompletions {
    EditorCompletions {
        entries: Vec::new(),
        replacement: None,
        incomplete: false,
        files: sources.files().len(),
        bytes: sources
            .files()
            .iter()
            .map(|source| source.text().len())
            .sum(),
    }
}
fn finish(
    sources: &SourceMap,
    replacement: Span,
    candidates: Candidates<'_>,
) -> Result<EditorCompletions, Vec<Diagnostic>> {
    let (entries, incomplete) = candidates.finish()?;
    let mut result = empty(sources);
    result.entries = entries;
    result.incomplete = incomplete;
    result.replacement = location(sources, replacement);
    Ok(result)
}
fn keywords<'a>(
    source: &'a SourceFile,
    replacement: Span,
    offset: usize,
) -> Result<Candidates<'a>, Vec<Diagnostic>> {
    let mut result = Candidates {
        prefix: &source.text()[replacement.start..offset],
        entries: Vec::new(),
        bytes: 0,
    };
    for &(label, kind, detail) in WORDS {
        result.add(label, kind, detail, (0, 0))?;
    }
    Ok(result)
}

pub(in crate::frontend) fn fallback(
    sources: &SourceMap,
    path: &Path,
    offset: usize,
) -> Result<EditorCompletions, Vec<Diagnostic>> {
    let Some(source) = sources
        .files()
        .iter()
        .find(|source| same_path(source.path(), path))
    else {
        return Ok(empty(sources));
    };
    let tokens = match lexer::lex_with_limit(source, lexer::MAX_TOKENS) {
        Ok(tokens) => tokens,
        Err(error) if error.code == "E0100" => return Ok(empty(sources)),
        Err(error) => return Err(vec![*error]),
    };
    let Some(replacement) = context(source, &tokens, offset) else {
        return Ok(empty(sources));
    };
    finish(sources, replacement, keywords(source, replacement, offset)?)
}

pub(in crate::frontend) fn complete(
    owner: SourceOwner<'_>,
    sources: &SourceMap,
    path: &Path,
    offset: usize,
) -> Result<EditorCompletions, Vec<Diagnostic>> {
    let found = (0..owner.count()).find_map(|id| {
        let module = ModuleId(id);
        let source = owner.file(module).ok()?;
        same_path(source.path(), path).then_some((module, source))
    });
    let Some((module, source)) = found else {
        return Ok(empty(sources));
    };
    let syntax = owner.ast(module).map_err(|error| vec![*error])?;
    let Some(replacement) = context(source, &syntax.tokens, offset) else {
        return Ok(empty(sources));
    };
    let mut candidates = keywords(source, replacement, offset)?;
    for declaration in &syntax.modules {
        candidates.add(
            source.text_at(declaration.name),
            9,
            "module",
            (1, declaration.name.start),
        )?;
    }
    for declaration in &syntax.imports {
        candidates.add(
            source.text_at(declaration.alias),
            18,
            "import",
            (1, declaration.alias.start),
        )?;
    }
    for function in &syntax.functions {
        candidates.add(
            source.text_at(function.name),
            3,
            "function",
            (1, function.name.start),
        )?;
    }
    for record in &syntax.records {
        candidates.add(
            source.text_at(record.name),
            22,
            "record",
            (1, record.name.start),
        )?;
    }
    for enumeration in &syntax.enums {
        candidates.add(
            source.text_at(enumeration.name),
            13,
            "enumeration",
            (1, enumeration.name.start),
        )?;
    }
    for (name, end) in syntax
        .records
        .iter()
        .map(|record| (record.name, record.end))
        .chain(
            syntax
                .enums
                .iter()
                .map(|enumeration| (enumeration.name, enumeration.end)),
        )
    {
        if offset >= name.end && offset <= end.start {
            for parameter in syntax.generic_parameters(name) {
                candidates.add(
                    source.text_at(parameter.name),
                    25,
                    "type parameter",
                    (2, parameter.name.start),
                )?;
            }
        }
    }
    for function in &syntax.functions {
        if offset < function.name.end || offset > function.end.start {
            continue;
        }
        for parameter in syntax.generic_parameters(function.name) {
            candidates.add(
                source.text_at(parameter.name),
                25,
                "type parameter",
                (2, parameter.name.start),
            )?;
        }
        let Some(body) = function.blocks.get(function.body.0) else {
            continue;
        };
        if !within(body, offset) {
            continue;
        }
        for parameter in &function.params {
            candidates.add(
                source.text_at(parameter.name),
                6,
                "parameter",
                (3, parameter.name.start),
            )?;
        }
        for block in &function.blocks {
            for statement in &block.body {
                match &statement.kind {
                    ast::StmtKind::Let { name, init, .. }
                        if within(block, offset)
                            && syntax.expressions[init.0].span.end <= replacement.start =>
                    {
                        candidates.add(
                            source.text_at(*name),
                            6,
                            "local variable",
                            (4, name.start),
                        )?
                    }
                    ast::StmtKind::DestructureRecord { fields, init, .. }
                        if within(block, offset)
                            && syntax.expressions[init.0].span.end <= replacement.start =>
                    {
                        for field in fields {
                            candidates.add(
                                source.text_at(field.binding),
                                6,
                                "local variable",
                                (4, field.binding.start),
                            )?;
                        }
                    }
                    ast::StmtKind::ForRange { name, body, .. }
                        if function
                            .blocks
                            .get(body.0)
                            .is_some_and(|scope| within(scope, offset)) =>
                    {
                        candidates.add(
                            source.text_at(*name),
                            6,
                            "loop variable",
                            (4, name.start),
                        )?
                    }
                    ast::StmtKind::Match { arms, .. } => {
                        for arm in arms {
                            if function
                                .blocks
                                .get(arm.body.0)
                                .is_some_and(|scope| within(scope, offset))
                            {
                                if let Some(name) = arm.binding {
                                    candidates.add(
                                        source.text_at(name),
                                        6,
                                        "pattern variable",
                                        (4, name.start),
                                    )?;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    finish(sources, replacement, candidates)
}
