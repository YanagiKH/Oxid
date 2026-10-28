//! Lazy, source-bound member queries over the already admitted editor tokens.
//! These handles describe names; they cannot become executable syntax or types.
use super::{DeclarationKind, EditorFile};
use crate::frontend::{
    declaration_index::WorkMeter,
    diagnostic::Diagnostic,
    lexer::{Kind, Token},
    parser::{MAX_BLOCK_NESTING, MAX_PATH_SEGMENTS},
    source::{SourceFile, Span},
};

type Answer<T> = Result<T, Box<Diagnostic>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) struct TypePath {
    identity: u64,
    bytes: usize,
    span: Span,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::frontend) struct MemberQuery {
    path: TypePath,
    annotation: Option<TypePath>,
    constructor: Option<Span>,
    replacement: Span,
    cursor: usize,
}
impl MemberQuery {
    pub(in crate::frontend) fn path(&self) -> TypePath {
        self.path
    }
    pub(in crate::frontend) fn annotation(&self) -> Option<TypePath> {
        self.annotation
    }
    pub(in crate::frontend) fn constructor(&self) -> Option<Span> {
        self.constructor
    }
    pub(in crate::frontend) fn replacement(&self) -> Span {
        self.replacement
    }
    pub(in crate::frontend) fn cursor(&self) -> usize {
        self.cursor
    }
}

impl TypePath {
    pub(in crate::frontend) fn span(self) -> Span {
        self.span
    }
    fn belongs_to(self, source: &SourceFile) -> bool {
        self.identity == source.identity()
            && self.bytes == source.text().len()
            && self.span.file == source.span(0, 0).file
    }
}

fn invalid() -> Box<Diagnostic> {
    Diagnostic::new(
        "E0500",
        "editor",
        "editor member source identity changed",
        None,
    )
}
fn limit(at: Span) -> Box<Diagnostic> {
    Diagnostic::new(
        "E0400",
        "editor",
        "editor member nesting limit exceeded",
        Some(at),
    )
}

/// A cursor never owns or reparses a replacement source buffer. Every token
/// inspection, including trivia and failed lookahead, consumes query work.
#[derive(Clone, Copy)]
pub(in crate::frontend) struct Reader<'a> {
    file: &'a EditorFile,
    source: &'a SourceFile,
    work: &'a WorkMeter,
    position: usize,
    end: usize,
}
impl<'a> Reader<'a> {
    fn previous(&self, end: &mut usize) -> Answer<Option<Token>> {
        while *end > 0 {
            *end -= 1;
            let token = self.file.tokens[*end];
            self.work.debit(1, token.span, "editor member receiver")?;
            if token.kind != Kind::Trivia && token.kind != Kind::Eof {
                return Ok(Some(token));
            }
        }
        Ok(None)
    }
    fn peek(&mut self) -> Answer<Option<Token>> {
        while self.position < self.end {
            let token = self.file.tokens[self.position];
            self.work.debit(1, token.span, "editor member token")?;
            if token.kind != Kind::Trivia {
                return Ok(Some(token));
            }
            self.position += 1;
        }
        Ok(None)
    }
    fn next(&mut self) -> Answer<Option<Token>> {
        let token = self.peek()?;
        if token.is_some() {
            self.position += 1;
        }
        Ok(token)
    }
    fn take(&mut self, kind: Kind) -> Answer<bool> {
        if self.peek()?.is_some_and(|token| token.kind == kind) {
            self.position += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn same_name(&self, a: Span, b: Span) -> Answer<bool> {
        let (a, b) = (self.source.text_at(a), self.source.text_at(b));
        self.work.debit(
            (a.len().min(b.len()) + 1) as u64,
            self.source.span(0, 0),
            "editor member name",
        )?;
        Ok(a == b)
    }
    fn between(self, start: usize, end: usize) -> Self {
        Self {
            position: start,
            end,
            ..self
        }
    }

    /// Delimiters are inspected from canonical tokens. The fixed stack is
    /// bounded by the existing parser block nesting cap, not by input length.
    fn value_end(&mut self, terminator: Kind, type_syntax: bool) -> Answer<Option<usize>> {
        let mut closes = [Kind::Eof; MAX_BLOCK_NESTING];
        let mut depth = 0usize;
        let mut any = false;
        while let Some(token) = self.peek()? {
            if depth == 0 && (token.kind == terminator || token.kind == Kind::RBrace) {
                return Ok(any.then_some(self.position));
            }
            let text = self.source.text_at(token.span);
            let closing = match token.kind {
                Kind::LParen => Some(Kind::RParen),
                Kind::LBrace => Some(Kind::RBrace),
                Kind::Less if type_syntax => Some(Kind::Greater),
                Kind::Unsupported if text == "[" => Some(Kind::Unsupported),
                _ => None,
            };
            if let Some(close) = closing {
                if depth == closes.len() {
                    return Err(limit(token.span));
                }
                closes[depth] = close;
                depth += 1;
            } else if matches!(token.kind, Kind::RParen | Kind::RBrace)
                || (type_syntax && token.kind == Kind::Greater)
                || (token.kind == Kind::Unsupported && text == "]")
            {
                if depth == 0 || closes[depth - 1] != token.kind {
                    return Ok(None);
                }
                depth -= 1;
            }
            if !type_syntax
                && (matches!(
                    token.kind,
                    Kind::If
                        | Kind::While
                        | Kind::Else
                        | Kind::Fn
                        | Kind::Return
                        | Kind::Break
                        | Kind::Continue
                ) || (token.kind == Kind::Ident && matches!(text, "for" | "match" | "loop")))
            {
                return Ok(None);
            }
            any = true;
            self.position += 1;
        }
        Ok(None)
    }

    fn type_path(mut self, allow_reference: bool) -> Answer<Option<TypePath>> {
        if allow_reference && self.take(Kind::Ampersand)? {
            self.take(Kind::Mut)?;
        }
        let Some(first) = self.next()? else {
            return Ok(None);
        };
        if first.kind != Kind::Ident {
            return Ok(None);
        }
        let mut last = first;
        let mut count = 1usize;
        while let Some(colon) = self.next()? {
            if colon.kind != Kind::Colon {
                return Ok(None);
            }
            let Some(second) = self.next()? else {
                return Ok(None);
            };
            if second.kind != Kind::Colon || colon.span.end != second.span.start {
                return Ok(None);
            }
            let Some(name) = self.next()? else {
                return Ok(None);
            };
            if name.kind != Kind::Ident || count == MAX_PATH_SEGMENTS {
                return Ok(None);
            }
            count += 1;
            last = name;
        }
        if count > 1 && self.source.text_at(first.span) != "crate" {
            return Ok(None);
        }
        Ok(Some(TypePath {
            identity: self.source.identity(),
            bytes: self.source.text().len(),
            span: self.source.span(first.span.start, last.span.end),
        }))
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::frontend) struct FieldName {
    pub name: Span,
    pub public: bool,
}

/// Declaration types and initializer values have different delimiter rules;
/// neither iterator invents a field or changes the editor file's retained AST.
pub(in crate::frontend) struct Fields<'a> {
    reader: Reader<'a>,
    declaration: bool,
    done: bool,
}
impl Fields<'_> {
    fn malformed(&self) -> Box<Diagnostic> {
        if self.declaration {
            invalid()
        } else {
            Diagnostic::new(
                "E0101",
                "editor",
                "record initializer is outside member completion syntax",
                None,
            )
        }
    }
    pub(in crate::frontend) fn next(&mut self) -> Answer<Option<FieldName>> {
        if self.done {
            return Ok(None);
        }
        if self.reader.take(Kind::RBrace)? {
            self.done = true;
            return Ok(None);
        }
        let public = self.declaration && self.reader.take(Kind::Pub)?;
        let Some(name) = self.reader.next()? else {
            return Err(self.malformed());
        };
        if name.kind != Kind::Ident || !self.reader.take(Kind::Colon)? {
            return Err(self.malformed());
        }
        if self
            .reader
            .value_end(Kind::Comma, self.declaration)?
            .is_none()
        {
            return Err(self.malformed());
        }
        if !self.reader.take(Kind::Comma)? {
            if !self.reader.take(Kind::RBrace)? {
                return Err(self.malformed());
            }
            self.done = true;
        }
        Ok(Some(FieldName {
            name: name.span,
            public,
        }))
    }
}

pub(in crate::frontend) struct PathNames<'a> {
    reader: Reader<'a>,
}
impl PathNames<'_> {
    pub(in crate::frontend) fn next(&mut self) -> Answer<Option<Span>> {
        while let Some(token) = self.reader.next()? {
            if token.kind == Kind::Ident {
                return Ok(Some(token.span));
            }
            if token.kind != Kind::Colon {
                return Err(invalid());
            }
        }
        Ok(None)
    }
}

impl EditorFile {
    fn member_reader<'a>(
        &'a self,
        source: &'a SourceFile,
        work: &'a WorkMeter,
    ) -> Answer<Reader<'a>> {
        if !self.belongs_to(source) {
            return Err(invalid());
        }
        Ok(Reader {
            file: self,
            source,
            work,
            position: 0,
            end: self.tokens.len(),
        })
    }
    fn token_at(&self, at: Span, work: &WorkMeter) -> Answer<Option<usize>> {
        for (index, token) in self.tokens.iter().enumerate() {
            work.debit(1, token.span, "editor member anchor")?;
            if token.span == at {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }
    pub(in crate::frontend) fn member_path<'a>(
        &'a self,
        source: &'a SourceFile,
        path: TypePath,
        work: &'a WorkMeter,
    ) -> Answer<PathNames<'a>> {
        if !path.belongs_to(source) {
            return Err(invalid());
        }
        let mut reader = self.member_reader(source, work)?;
        while let Some(token) = reader.peek()? {
            if token.span.start >= path.span.start {
                break;
            }
            reader.position += 1;
        }
        let start = reader.position;
        while let Some(token) = reader.peek()? {
            if token.span.end > path.span.end {
                break;
            }
            reader.position += 1;
        }
        reader = reader.between(start, reader.position);
        if reader.type_path(false)? != Some(path) {
            return Err(invalid());
        }
        Ok(PathNames { reader })
    }
    pub(in crate::frontend) fn has_member_import(
        &self,
        source: &SourceFile,
        name: &str,
        work: &WorkMeter,
    ) -> Answer<bool> {
        if !self.belongs_to(source) {
            return Err(invalid());
        }
        for import in &self.imports {
            let spelling = source.text_at(*import);
            work.debit(
                (spelling.len().min(name.len()) + 1) as u64,
                *import,
                "editor member import",
            )?;
            if spelling == name {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(in crate::frontend) fn member_fields<'a>(
        &'a self,
        source: &'a SourceFile,
        name: Span,
        work: &'a WorkMeter,
    ) -> Answer<Option<Fields<'a>>> {
        let mut reader = self.member_reader(source, work)?;
        let mut found = false;
        for declaration in &self.declarations {
            work.debit(1, declaration.name, "editor member record")?;
            if declaration.name == name && declaration.kind == DeclarationKind::Record {
                found = true;
            }
        }
        if !found {
            return Err(invalid());
        }
        reader.position = self.token_at(name, work)?.ok_or_else(invalid)? + 1;
        // Applied and unapplied generic records are deliberately outside this query.
        if !reader.take(Kind::LBrace)? {
            return Ok(None);
        }
        Ok(Some(Fields {
            reader,
            declaration: true,
            done: false,
        }))
    }
    pub(in crate::frontend) fn constructor_fields<'a>(
        &'a self,
        source: &'a SourceFile,
        span: Span,
        work: &'a WorkMeter,
    ) -> Answer<Fields<'a>> {
        let mut reader = self.member_reader(source, work)?;
        reader.position = self
            .token_at(source.span(span.start, span.start + 1), work)?
            .ok_or_else(invalid)?;
        if !reader.take(Kind::LBrace)? {
            return Err(invalid());
        }
        let start = reader.position;
        while let Some(token) = reader.peek()? {
            if token.span.end > span.end {
                break;
            }
            reader.position += 1;
        }
        reader = reader.between(start, reader.position);
        Ok(Fields {
            reader,
            declaration: false,
            done: false,
        })
    }

    pub(in crate::frontend) fn member_query(
        &self,
        source: &SourceFile,
        cursor: usize,
        work: &WorkMeter,
    ) -> Answer<Option<MemberQuery>> {
        let all = self.member_reader(source, work)?;
        if cursor > source.text().len() || !source.text().is_char_boundary(cursor) {
            return Ok(None);
        }
        let mut replacement = source.span(cursor, cursor);
        let mut before = 0usize;
        for (index, token) in self.tokens.iter().enumerate() {
            work.debit(1, token.span, "editor member cursor")?;
            if token.span.start > cursor {
                break;
            }
            let text = source.text_at(token.span);
            if token.span.start <= cursor
                && (cursor < token.span.end || (text.starts_with("//") && cursor == token.span.end))
                && (matches!(
                    token.kind,
                    Kind::String | Kind::Number | Kind::Unsupported | Kind::Invalid
                ) || text.starts_with('\'')
                    || (token.kind == Kind::Trivia
                        && (text.starts_with("//") || text.starts_with("/*"))))
            {
                return Ok(None);
            }
            if token.kind == Kind::Ident && token.span.start <= cursor && cursor <= token.span.end {
                replacement = token.span;
                before = index;
                break;
            }
            if token.span.end <= cursor {
                before = index + 1;
            }
        }
        if !all
            .previous(&mut before)?
            .is_some_and(|token| token.kind == Kind::Dot)
        {
            return Ok(None);
        }
        let Some(receiver) = all.previous(&mut before)? else {
            return Ok(None);
        };
        if receiver.kind != Kind::Ident {
            return Ok(None);
        }
        let receiver_index = before;
        let mut opening = None;
        for region in &self.regions {
            work.debit(1, region.span, "editor member body")?;
            if region.span.start <= receiver.span.start && cursor <= region.span.end {
                for (index, token) in self.tokens.iter().enumerate() {
                    work.debit(1, token.span, "editor member body anchor")?;
                    if token.kind == Kind::LBrace && token.span.end == region.span.start {
                        opening = Some(index);
                        break;
                    }
                }
            }
        }
        let Some(opening) = opening else {
            return Ok(None);
        };
        let mut function = None;
        for declaration in &self.declarations {
            work.debit(1, declaration.name, "editor member function")?;
            if declaration.name.end < self.tokens[opening].span.start {
                function =
                    (declaration.kind == DeclarationKind::Function).then_some(declaration.name);
            }
        }
        let Some(function) = function else {
            return Ok(None);
        };
        let function = self.token_at(function, work)?.ok_or_else(invalid)?;
        let mut header = all.between(function + 1, opening);
        if !header.take(Kind::LParen)? {
            return Ok(None);
        }
        let mut bound = false;
        let mut path = None;
        while !header.take(Kind::RParen)? {
            let Some(name) = header.next()? else {
                return Ok(None);
            };
            if name.kind != Kind::Ident || !header.take(Kind::Colon)? {
                return Ok(None);
            }
            let start = header.position;
            // Function headers were parsed by the ordinary parser. Only commas
            // at the parameter level delimit a type; nested types remain opaque.
            let mut depth = 0usize;
            while let Some(token) = header.peek()? {
                let text = source.text_at(token.span);
                if depth == 0 && matches!(token.kind, Kind::Comma | Kind::RParen) {
                    break;
                }
                if matches!(token.kind, Kind::LParen | Kind::Less) || text == "[" {
                    depth += 1;
                }
                if matches!(token.kind, Kind::RParen | Kind::Greater) || text == "]" {
                    depth = depth.saturating_sub(1);
                }
                header.position += 1;
            }
            if all.same_name(name.span, receiver.span)? {
                if bound {
                    return Ok(None);
                }
                bound = true;
                path = all.between(start, header.position).type_path(true)?;
            }
            if !header.take(Kind::Comma)? {
                if !header.take(Kind::RParen)? {
                    return Ok(None);
                }
                break;
            }
        }
        let mut annotation = None;
        let mut constructor = None;
        let mut body = all.between(opening + 1, receiver_index);
        while let Some(token) = body.peek()? {
            if token.kind == Kind::Return {
                body.position += 1;
                if body.peek()?.is_some() {
                    return Ok(None);
                }
                break;
            }
            if !body.take(Kind::Let)? {
                return Ok(None);
            }
            body.take(Kind::Mut)?;
            let Some(name) = body.next()? else {
                return Ok(None);
            };
            if name.kind != Kind::Ident {
                return Ok(None);
            }
            let same = all.same_name(name.span, receiver.span)?;
            let mut declared = None;
            if body.take(Kind::Colon)? {
                let start = body.position;
                while let Some(token) = body.peek()? {
                    if token.kind == Kind::Equal {
                        break;
                    }
                    body.position += 1;
                }
                declared = all.between(start, body.position).type_path(false)?;
                if declared.is_none() && same {
                    return Ok(None);
                }
            }
            if !body.take(Kind::Equal)? {
                return Ok(None);
            }
            if body.peek()?.is_none() {
                // A new binding is not visible inside its own initializer.
                if same {
                    return Ok(None);
                }
                break;
            }
            let start = body.position;
            let Some(end) = body.value_end(Kind::Semi, false)? else {
                return Ok(None);
            };
            let mut value = all.between(start, end);
            while let Some(token) = value.next()? {
                if token.kind == Kind::Ident && all.same_name(token.span, receiver.span)? {
                    // No flow or borrow proof is fabricated for a preceding use.
                    return Ok(None);
                }
            }
            if same {
                if bound {
                    return Ok(None);
                }
                bound = true;
                let mut value = all.between(start, end);
                let path_start = value.position;
                while let Some(token) = value.peek()? {
                    if token.kind == Kind::LBrace {
                        break;
                    }
                    value.position += 1;
                }
                path = all.between(path_start, value.position).type_path(false)?;
                annotation = declared;
                let Some(open) = value.next()? else {
                    return Ok(None);
                };
                if open.kind != Kind::LBrace {
                    return Ok(None);
                }
                let mut depth = 1usize;
                let mut close = None;
                while let Some(token) = value.next()? {
                    if token.kind == Kind::LBrace {
                        depth += 1;
                    }
                    if token.kind == Kind::RBrace {
                        depth -= 1;
                    }
                    if depth == 0 {
                        close = Some(token);
                        break;
                    }
                }
                if depth != 0 || value.peek()?.is_some() {
                    return Ok(None);
                }
                constructor = close.map(|close| source.span(open.span.start, close.span.end));
            }
            if !body.take(Kind::Semi)? {
                return Ok(None);
            }
        }
        Ok(match path {
            Some(path) => Some(MemberQuery {
                path,
                annotation,
                constructor,
                replacement,
                cursor,
            }),
            None => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{lexer, project::budget::Allocator, source::SourceMap};

    #[test]
    fn member_sidecars_reject_equal_spelling_from_another_source_generation() {
        let text = "struct Point{value:i32}fn read(point:&Point)->i32{return point.;}";
        let mut first = SourceMap::new();
        let id = first.add("main.ox".into(), text.into());
        let source = first.get(id);
        let (file, _) = EditorFile::parse(
            source,
            lexer::lex(source).unwrap(),
            100_000,
            &mut Allocator::default(),
        )
        .unwrap();
        let offset = text.find("point.").unwrap() + 6;
        let work = WorkMeter::default();
        let query = file.member_query(source, offset, &work).unwrap().unwrap();
        let mut second = SourceMap::new();
        let other = second.add("main.ox".into(), text.into());
        assert_eq!(id, other);
        assert!(file
            .member_query(second.get(other), offset, &work)
            .err()
            .is_some_and(|error| error.code == "E0500"));
        assert!(file
            .member_path(second.get(other), query.path(), &work)
            .err()
            .is_some_and(|error| error.code == "E0500"));
        let (other_file, _) = EditorFile::parse(
            second.get(other),
            lexer::lex(second.get(other)).unwrap(),
            100_000,
            &mut Allocator::default(),
        )
        .unwrap();
        assert!(other_file
            .member_path(second.get(other), query.path(), &work)
            .err()
            .is_some_and(|error| error.code == "E0500"));
        let mut names = file.member_path(source, query.path(), &work).unwrap();
        assert_eq!(source.text_at(names.next().unwrap().unwrap()), "Point");
        assert!(names.next().unwrap().is_none());
    }
}
