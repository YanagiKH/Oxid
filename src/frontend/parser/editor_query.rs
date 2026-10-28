//! Declaration-only editor syntax. This type cannot become an executable AST.
use super::*;
use crate::frontend::source::{SourceFileId, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum DeclarationKind {
    Function,
    Record,
    Enumeration,
    Module,
}
#[derive(Clone, Copy, Debug)]
pub(in crate::frontend) struct Declaration {
    pub name: Span,
    pub public: bool,
    pub kind: DeclarationKind,
}
#[derive(Debug)]
pub(in crate::frontend) struct EditorFile {
    identity: u64,
    file: SourceFileId,
    bytes: usize,
    tokens: Vec<Token>,
    modules: Vec<ModuleDecl>,
    declarations: Vec<Declaration>,
    imports: Vec<Span>,
    regions: Vec<QueryRegion>,
}
#[derive(Debug)]
struct QueryRegion {
    span: Span,
    import: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub(in crate::frontend) struct PathQuery {
    identity: u64,
    bytes: usize,
    segments: [Span; MAX_PATH_SEGMENTS],
    prefix_len: usize,
    replacement: Span,
    cursor: usize,
}

impl PathQuery {
    pub(in crate::frontend) fn segments(&self) -> &[Span] {
        &self.segments[..self.prefix_len]
    }
    pub(in crate::frontend) fn replacement(&self) -> Span {
        self.replacement
    }
    pub(in crate::frontend) fn cursor(&self) -> usize {
        self.cursor
    }
    pub(in crate::frontend) fn belongs_to(&self, source: &SourceFile) -> bool {
        self.identity == source.identity()
            && self.bytes == source.text().len()
            && self.replacement.file == source.span(0, 0).file
    }
}

impl EditorFile {
    pub(in crate::frontend) fn belongs_to(&self, source: &SourceFile) -> bool {
        self.identity == source.identity()
            && self.file == source.span(0, 0).file
            && self.bytes == source.text().len()
    }
    pub(in crate::frontend) fn modules(&self) -> &[ModuleDecl] {
        &self.modules
    }
    pub(in crate::frontend) fn declarations(&self) -> &[Declaration] {
        &self.declarations
    }
    pub(in crate::frontend) fn binding_names(&self) -> impl Iterator<Item = Span> + '_ {
        self.declarations
            .iter()
            .map(|item| item.name)
            .chain(self.imports.iter().copied())
    }
    pub(in crate::frontend) fn parse(
        source: &SourceFile,
        tokens: Vec<Token>,
        limit: usize,
        allocator: &mut Allocator,
    ) -> Result<(Self, usize), Vec<Diagnostic>> {
        let mut parser = Parser {
            source,
            allocator,
            mode: SourceMode::ProjectCandidate,
            arrays: ArraySyntaxPolicy::Enabled,
            enums: EnumSyntaxPolicy::Enabled,
            std_imports: StdImportPolicy::Enabled,
            storage: Default::default(),
            project_recovery: false,
            parsed_has_named_slice: false,
            parsed_has_copy_records: false,
            tokens,
            cursor: 0,
            pending_equal: None,
            expressions: Vec::new(),
            paths: Vec::new(),
            path_segments: Vec::new(),
            generic_headers: Vec::new(),
            generic_calls: Vec::new(),
            heights: Vec::new(),
            nodes: 0,
            node_limit: limit.min(MAX_NODES),
        };
        parser.skip();
        let mut modules = Vec::new();
        let mut declarations = Vec::new();
        let mut imports = Vec::new();
        let mut regions = Vec::new();
        let parsed = (|| -> Result<(), Box<Diagnostic>> {
            while parser.peek().kind != Kind::Eof {
                if parser.peek().kind == Kind::Mod
                    || (parser.peek().kind == Kind::Pub && parser.next_kind() == Kind::Mod)
                {
                    let module = parser.module()?;
                    parser.candidate_reserve(
                        &mut modules,
                        MAX_NODES,
                        "editor module declarations",
                        module.name,
                    )?;
                    parser.candidate_reserve(
                        &mut declarations,
                        MAX_NODES,
                        "editor declarations",
                        module.name,
                    )?;
                    declarations.push(Declaration {
                        name: module.name,
                        public: module.public.is_some(),
                        kind: DeclarationKind::Module,
                    });
                    modules.push(module);
                    continue;
                }
                if parser.peek().kind == Kind::Use {
                    let start = parser.peek().span;
                    // Standard imports retain the actual closed catalog grammar.
                    let first = parser.tokens[parser.cursor + 1..]
                        .iter()
                        .find(|t| t.kind != Kind::Trivia);
                    if first.is_none_or(|token| source.text_at(token.span) != "crate") {
                        let import = parser.import()?;
                        parser.candidate_reserve(
                            &mut imports,
                            MAX_NODES,
                            "editor import names",
                            import.alias,
                        )?;
                        imports.push(import.alias);
                    } else {
                        parser.bump();
                        let mut count = 1usize;
                        parser.expect(Kind::Ident, "expected crate path")?;
                        let mut endpoint: Option<Span>;
                        loop {
                            parser.expect(Kind::Colon, "absolute item path requires `::`")?;
                            parser.expect(Kind::Colon, "absolute item path requires `::`")?;
                            if count >= MAX_PATH_SEGMENTS {
                                return Err(parser.diagnostic(
                                    "E0400",
                                    "editor",
                                    "qualified path segment limit exceeded",
                                    Some(parser.peek().span),
                                ));
                            }
                            count += 1;
                            if matches!(parser.peek().kind, Kind::Semi | Kind::Eof)
                                || (parser.peek().kind == Kind::Ident
                                    && source.text_at(parser.peek().span) == "as")
                            {
                                endpoint = None;
                                break;
                            }
                            endpoint = Some(
                                parser
                                    .expect(Kind::Ident, "expected item path segment")?
                                    .span,
                            );
                            if !parser.double_colon() {
                                break;
                            }
                        }
                        if parser.peek().kind == Kind::Ident
                            && source.text_at(parser.peek().span) == "as"
                        {
                            parser.bump();
                            endpoint =
                                Some(parser.expect(Kind::Ident, "expected import alias")?.span);
                        }
                        let end = if parser.peek().kind == Kind::Eof {
                            parser.peek().span
                        } else {
                            parser
                                .expect(Kind::Semi, "import declaration requires `;`")?
                                .span
                        };
                        if let Some(alias) = endpoint {
                            parser.candidate_reserve(
                                &mut imports,
                                MAX_NODES,
                                "editor import names",
                                alias,
                            )?;
                            imports.push(alias);
                        }
                        parser.candidate_reserve(
                            &mut regions,
                            MAX_NODES,
                            "editor query regions",
                            start,
                        )?;
                        regions.push(QueryRegion {
                            span: source.span(start.end, end.start),
                            import: true,
                        });
                    }
                    continue;
                }
                let copy = parser.copy_attribute()?;
                let public = parser.take(Kind::Pub).is_some();
                let (name, kind) = if parser.peek().kind == Kind::Struct {
                    let item = parser.record(None, copy)?;
                    (item.name, DeclarationKind::Record)
                } else if parser.enum_keyword("enum") {
                    if copy.is_some() {
                        return Err(parser.error("Copy derivation requires a record declaration"));
                    }
                    let item = parser.enumeration(None)?;
                    (item.name, DeclarationKind::Enumeration)
                } else {
                    if copy.is_some() {
                        return Err(parser.error("Copy derivation requires a record declaration"));
                    }
                    let (name, _, _) = parser.function_header()?;
                    let opening = parser
                        .expect(Kind::LBrace, "expected function body `{`")?
                        .span;
                    let mut depth = 1usize;
                    while depth != 0 && parser.peek().kind != Kind::Eof {
                        let token = parser.peek();
                        // Opaque bodies are real token regions, not recovered AST values.
                        parser.node()?;
                        match token.kind {
                            Kind::LBrace => {
                                depth += 1;
                                if depth > MAX_BLOCK_NESTING {
                                    return Err(parser.diagnostic(
                                        "E0400",
                                        "editor",
                                        "editor body nesting limit exceeded",
                                        Some(token.span),
                                    ));
                                }
                            }
                            Kind::RBrace => depth -= 1,
                            _ => {}
                        }
                        if depth == 0 {
                            break;
                        }
                        parser.bump();
                    }
                    let end = parser.peek().span;
                    if depth == 0 {
                        parser.bump();
                    }
                    parser.candidate_reserve(
                        &mut regions,
                        MAX_NODES,
                        "editor query regions",
                        name,
                    )?;
                    regions.push(QueryRegion {
                        span: source.span(opening.end, end.start),
                        import: false,
                    });
                    (name, DeclarationKind::Function)
                };
                parser.candidate_reserve(
                    &mut declarations,
                    MAX_NODES,
                    "editor declarations",
                    name,
                )?;
                declarations.push(Declaration { name, public, kind });
            }
            Ok(())
        })();
        if let Err(error) = parsed {
            return Err(vec![*error]);
        }
        Ok((
            Self {
                identity: source.identity(),
                file: source.span(0, 0).file,
                bytes: source.text().len(),
                tokens: parser.tokens,
                modules,
                declarations,
                imports,
                regions,
            },
            parser.nodes,
        ))
    }

    pub(in crate::frontend) fn query(
        &self,
        source: &SourceFile,
        cursor: usize,
    ) -> Result<Option<PathQuery>, Box<Diagnostic>> {
        if !self.belongs_to(source) {
            return Err(Diagnostic::new(
                "E0500",
                "editor",
                "editor source identity changed",
                None,
            ));
        }
        if cursor > source.text().len() || !source.text().is_char_boundary(cursor) {
            return Ok(None);
        }
        for token in &self.tokens {
            if token.span.start > cursor {
                break;
            }
            if token.span.start <= cursor
                && (cursor < token.span.end
                    || (source.text_at(token.span).starts_with("//") && cursor == token.span.end))
            {
                let text = source.text_at(token.span);
                if matches!(token.kind, Kind::String | Kind::Number) || text.starts_with('\'') {
                    return Ok(None);
                }
                if token.kind == Kind::Trivia && (text.starts_with("//") || text.starts_with("/*"))
                {
                    return Ok(None);
                }
            }
        }
        let in_import = self.regions.iter().any(|region| {
            region.import && region.span.start <= cursor && cursor <= region.span.end
        });
        if in_import
            && self.tokens.iter().any(|token| {
                token.kind == Kind::Ident
                    && source.text_at(token.span) == "as"
                    && token.span.start < cursor
                    && cursor <= token.span.end
            })
        {
            return Ok(None);
        }
        let replacement = self
            .tokens
            .iter()
            .find(|token| {
                token.kind == Kind::Ident
                    && token.span.start <= cursor
                    && cursor <= token.span.end
                    && !(in_import && source.text_at(token.span) == "as")
            })
            .map_or(source.span(cursor, cursor), |token| token.span);
        let mut before = self.tokens.iter().rev().filter(|token| {
            token.kind != Kind::Trivia
                && token.kind != Kind::Eof
                && token.span.end <= replacement.start
        });
        let mut segments = [source.span(0, 0); MAX_PATH_SEGMENTS];
        let mut length = 0usize;
        loop {
            let (Some(second), Some(first)) = (before.next(), before.next()) else {
                return Ok(None);
            };
            if first.kind != Kind::Colon
                || second.kind != Kind::Colon
                || first.span.end != second.span.start
            {
                return Ok(None);
            }
            let Some(name) = before.next() else {
                return Ok(None);
            };
            if name.kind != Kind::Ident {
                return Ok(None);
            }
            if length + 1 >= MAX_PATH_SEGMENTS {
                return Err(Diagnostic::new(
                    "E0400",
                    "editor",
                    "qualified path segment limit exceeded",
                    Some(name.span),
                ));
            }
            segments[length] = name.span;
            length += 1;
            if source.text_at(name.span) == "crate" {
                break;
            }
        }
        segments[..length].reverse();
        if before
            .next()
            .is_some_and(|token| matches!(token.kind, Kind::Colon | Kind::Dot))
        {
            return Ok(None);
        }
        if !self.regions.iter().any(|region| {
            region.span.start <= segments[0].start && replacement.end <= region.span.end
        }) {
            return Ok(None);
        }
        Ok(Some(PathQuery {
            identity: self.identity,
            bytes: self.bytes,
            segments,
            prefix_len: length,
            replacement,
            cursor,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{lexer, source::SourceMap};

    fn parsed(text: &str) -> (SourceMap, EditorFile) {
        let mut map = SourceMap::new();
        let id = map.add("editor-query.ox".into(), text.into());
        let source = map.get(id);
        let mut allocator = Allocator::default();
        let tokens =
            lexer::lex_current_with_allocator(source, lexer::MAX_TOKENS, &mut allocator).unwrap();
        let (parsed, _) = EditorFile::parse(source, tokens, MAX_NODES, &mut allocator).unwrap();
        (map, parsed)
    }
    #[test]
    fn editor_query_holes_bind_the_actual_source_generation_without_ordinary_ast_authority() {
        let text = "fn answer()->i32{return 42;}fn main()->i32{return crate::";
        let (map, parsed) = parsed(text);
        let source = map.get(SourceFileId(0));
        let query = parsed.query(source, text.len()).unwrap().unwrap();
        assert_eq!(
            query
                .segments()
                .iter()
                .map(|&span| map.text(span))
                .collect::<Vec<_>>(),
            ["crate"]
        );
        assert_eq!(query.replacement(), source.span(text.len(), text.len()));
        assert!(query.belongs_to(source));
        let mut foreign = SourceMap::new();
        let id = foreign.add("editor-query.ox".into(), text.into());
        assert!(!parsed.belongs_to(foreign.get(id)));
        assert!(!query.belongs_to(foreign.get(id)));
        assert_eq!(
            parsed.query(foreign.get(id), text.len()).unwrap_err().code,
            "E0500"
        );
        let mut allocator = Allocator::default();
        let tokens =
            lexer::lex_current_with_allocator(source, lexer::MAX_TOKENS, &mut allocator).unwrap();
        assert!(parse_typed_counted(
            source,
            tokens,
            SourceMode::ProjectCandidate,
            MAX_NODES,
            &mut allocator,
            &mut Default::default()
        )
        .is_err());
    }
    #[test]
    fn editor_query_headers_share_ordinary_grammar_and_do_not_promote_body_tokens() {
        let text = "#[derive(Copy)]struct Point{x:i32}enum Choice<T>{Some(T),None}fn generic<T>(x:T)->T{return x;}pub fn answer()->i32{return 42;}fn main()->i32{return crate::answer();}";
        let (map, query) = parsed(text);
        let source = map.get(SourceFileId(0));
        let mut allocator = Allocator::default();
        let tokens =
            lexer::lex_current_with_allocator(source, lexer::MAX_TOKENS, &mut allocator).unwrap();
        let (ordinary, _) = parse_typed_counted(
            source,
            tokens,
            SourceMode::ProjectCandidate,
            MAX_NODES,
            &mut allocator,
            &mut Default::default(),
        )
        .unwrap();
        let expected: Vec<_> = ordinary
            .items
            .iter()
            .map(|item| match item {
                ItemId::Function(i) => (
                    ordinary.functions[*i].name,
                    ordinary.functions[*i].public.is_some(),
                    DeclarationKind::Function,
                ),
                ItemId::Struct(i) => (
                    ordinary.records[*i].name,
                    ordinary.records[*i].public.is_some(),
                    DeclarationKind::Record,
                ),
                ItemId::Enum(i) => (
                    ordinary.enums[*i].name,
                    ordinary.enums[*i].public.is_some(),
                    DeclarationKind::Enumeration,
                ),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(
            query
                .declarations()
                .iter()
                .map(|item| (item.name, item.public, item.kind))
                .collect::<Vec<_>>(),
            expected
        );
        let text = "fn outer()->i32{fn phantom()->i32{return 0;}mod nonexistent;/*fn comment()*/return crate::;}fn later()->i32{return 1;}";
        let (map, query) = parsed(text);
        assert_eq!(
            query
                .declarations()
                .iter()
                .map(|item| map.text(item.name))
                .collect::<Vec<_>>(),
            ["outer", "later"]
        );
        assert!(query.modules().is_empty());
    }
    #[test]
    fn editor_query_counts_the_absent_endpoint_in_the_existing_path_limit() {
        for modules in [31, 32, 33] {
            let text = format!("fn main()->i32{{return crate::{};}}", "m::".repeat(modules));
            let offset = text.find(';').unwrap();
            let (map, parsed) = parsed(&text);
            let result = parsed.query(map.get(SourceFileId(0)), offset);
            if modules == 33 {
                assert_eq!(result.unwrap_err().code, "E0400");
            } else {
                assert_eq!(result.unwrap().unwrap().segments().len(), modules + 1);
            }
        }
    }
}
