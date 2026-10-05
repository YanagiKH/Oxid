use super::{
    ast::*,
    diagnostic::Diagnostic,
    lexer::{Kind, Token},
    project::budget::{Allocator, ReserveFailure},
    source::SourceFile,
};
/// Only parsing can mint this association; AST consumers cannot clone or
/// manufacture it for a different source owner.
pub(super) struct SourceProvenance {
    identity: u64,
    text_len: usize,
    file: super::source::SourceFileId,
}
impl std::fmt::Debug for SourceProvenance {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The generation binds ownership, not syntax. Keep structural AST
        // observations deterministic across identical source allocations.
        formatter
            .debug_struct("SourceProvenance")
            .field("text_len", &self.text_len)
            .field("file", &self.file)
            .finish_non_exhaustive()
    }
}
impl SourceProvenance {
    fn new(source: &SourceFile) -> Self {
        Self {
            identity: source.identity(),
            text_len: source.text().len(),
            file: source.span(0, 0).file,
        }
    }
    pub(super) fn belongs_to(&self, source: &SourceFile) -> bool {
        self.identity == source.identity()
            && self.text_len == source.text().len()
            && self.file == source.span(0, 0).file
    }
}
pub const MAX_NODES: usize = 100_000;
pub const MAX_NESTING: usize = 64;
/// Active statement blocks, including the outer function body; independent of expressions.
pub const MAX_BLOCK_NESTING: usize = 64;
pub const MAX_PARAMS: usize = 256;
pub(super) const MAX_ARRAY_ELEMENTS: usize = 1024;
pub(super) const MAX_PATH_SEGMENTS: usize = 34;
pub(super) const MAX_ENUM_VARIANTS: usize = 256;
pub const MAX_DIAGNOSTICS: usize = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // Preserve privately qualified predecessor entrypoint names.
pub(super) enum SourceMode {
    #[cfg(test)]
    ScalarOnly,
    OwnedCandidate,
    ModuleCandidate,
    ProjectCandidate,
}
impl SourceMode {
    fn owned(self) -> bool {
        #[cfg(test)]
        if self == Self::ScalarOnly {
            return false;
        }
        true
    }
}
/// Independent of module routing; explicit typed routes opt into array syntax.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ArraySyntaxPolicy {
    Closed,
    Enabled,
    #[cfg(test)]
    Candidate,
}
impl ArraySyntaxPolicy {
    fn enabled(self) -> bool {
        match self {
            Self::Closed => false,
            Self::Enabled => true,
            #[cfg(test)]
            Self::Candidate => true,
        }
    }
}
/// A private parser candidate only; no production entrypoint accepts this policy.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EnumSyntaxPolicy {
    Closed,
    #[cfg(test)]
    Candidate,
}
impl EnumSyntaxPolicy {
    fn enabled(self) -> bool {
        match self {
            Self::Closed => false,
            #[cfg(test)]
            Self::Candidate => true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LiteralContext {
    Allowed,
    ConditionRoot,
}
enum PrimaryStart {
    Boolean,
    Number,
    Name,
    Group,
    Array,
}
#[cfg(test)]
pub fn parse(source: &SourceFile, tokens: Vec<Token>) -> Result<Program, Vec<Diagnostic>> {
    parse_with_mode(source, tokens, SourceMode::OwnedCandidate)
}
#[cfg(test)]
pub(super) fn parse_with_mode(
    source: &SourceFile,
    tokens: Vec<Token>,
    mode: SourceMode,
) -> Result<Program, Vec<Diagnostic>> {
    parse_with_node_limit(source, tokens, mode, MAX_NODES)
}
#[cfg(test)]
fn parse_with_node_limit(
    source: &SourceFile,
    tokens: Vec<Token>,
    mode: SourceMode,
    node_limit: usize,
) -> Result<Program, Vec<Diagnostic>> {
    parse_counted(source, tokens, mode, node_limit, &mut Allocator::default())
        .map(|(program, _)| program)
}

pub(super) fn parse_counted(
    source: &SourceFile,
    tokens: Vec<Token>,
    mode: SourceMode,
    node_limit: usize,
    allocator: &mut Allocator,
) -> Result<(Program, usize), Vec<Diagnostic>> {
    parse_counted_with_arrays(
        source,
        tokens,
        mode,
        node_limit,
        allocator,
        ArraySyntaxPolicy::Closed,
    )
}

pub(super) fn parse_counted_with_arrays(
    source: &SourceFile,
    tokens: Vec<Token>,
    mode: SourceMode,
    node_limit: usize,
    allocator: &mut Allocator,
    arrays: ArraySyntaxPolicy,
) -> Result<(Program, usize), Vec<Diagnostic>> {
    parse_counted_with_policies(
        source,
        tokens,
        mode,
        node_limit,
        allocator,
        arrays,
        EnumSyntaxPolicy::Closed,
        &mut enums::SyntaxStorage::default(),
    )
}

#[cfg(test)]
pub(super) fn parse_enum_candidate_counted(
    source: &SourceFile,
    tokens: Vec<Token>,
    mode: SourceMode,
    node_limit: usize,
    allocator: &mut Allocator,
    storage: &mut enums::SyntaxStorage,
) -> Result<(Program, usize), Vec<Diagnostic>> {
    parse_counted_with_policies(
        source,
        tokens,
        mode,
        node_limit,
        allocator,
        ArraySyntaxPolicy::Enabled,
        EnumSyntaxPolicy::Candidate,
        storage,
    )
}

#[allow(clippy::too_many_arguments)]
fn parse_counted_with_policies(
    source: &SourceFile,
    tokens: Vec<Token>,
    mode: SourceMode,
    node_limit: usize,
    allocator: &mut Allocator,
    arrays: ArraySyntaxPolicy,
    enums: EnumSyntaxPolicy,
    storage: &mut enums::SyntaxStorage,
) -> Result<(Program, usize), Vec<Diagnostic>> {
    *storage = enums::SyntaxStorage::default();
    let mut parser = Parser {
        source,
        allocator,
        mode,
        arrays,
        enums,
        storage: enums::SyntaxStorage::default(),
        project_recovery: false,
        tokens,
        cursor: 0,
        expressions: Vec::new(),
        paths: Vec::new(),
        path_segments: Vec::new(),
        heights: Vec::new(),
        nodes: 0,
        node_limit: node_limit.min(MAX_NODES),
    };
    parser.skip();
    let mut functions = Vec::new();
    let mut records = Vec::new();
    let mut enumerations = Vec::new();
    let mut modules = Vec::new();
    let mut imports = Vec::new();
    let mut items = Vec::new();
    let mut diagnostics = Vec::new();
    while parser.peek().kind != Kind::Eof && diagnostics.len() < MAX_DIAGNOSTICS {
        let before = parser.cursor;
        let result = if (mode == SourceMode::ModuleCandidate
            && matches!(parser.peek().kind, Kind::Mod | Kind::Pub))
            || (mode == SourceMode::ProjectCandidate
                && (parser.peek().kind == Kind::Mod
                    || (parser.peek().kind == Kind::Pub && parser.next_kind() == Kind::Mod)))
        {
            parser.module().and_then(|module| {
                let at = module.name;
                if parser.enums_enabled() {
                    parser.candidate_reserve(&mut modules, MAX_NODES, "syntax modules", at)?;
                    parser.candidate_reserve(&mut items, MAX_NODES, "syntax items", at)?;
                } else {
                    parser
                        .allocator
                        .vector(&mut modules, 1, "module declarations")
                        .and_then(|()| parser.allocator.vector(&mut items, 1, "module items"))
                        .map_err(|error| {
                            parser.diagnostic(
                                "E0400",
                                "parse",
                                match error {
                                    ReserveFailure::Overflow => "module syntax count overflow",
                                    ReserveFailure::Allocation => "module syntax allocation failed",
                                },
                                Some(at),
                            )
                        })?;
                }
                items.push(ItemId::Module(modules.len()));
                modules.push(module);
                Ok(())
            })
        } else if mode == SourceMode::ProjectCandidate && parser.peek().kind == Kind::Use {
            parser.import().and_then(|import| {
                if parser.enums_enabled() {
                    parser.candidate_reserve(
                        &mut imports,
                        MAX_NODES,
                        "syntax imports",
                        import.span,
                    )?;
                    parser.candidate_reserve(&mut items, MAX_NODES, "syntax items", import.span)?;
                } else {
                    parser
                        .allocator
                        .vector(&mut imports, 1, "import declarations")
                        .and_then(|()| parser.allocator.vector(&mut items, 1, "import items"))
                        .map_err(|error| parser.project_reserve_error(error, import.span))?;
                }
                items.push(ItemId::Import(imports.len()));
                imports.push(import);
                Ok(())
            })
        } else {
            let public = if mode == SourceMode::ProjectCandidate
                && parser.peek().kind == Kind::Pub
                && (matches!(parser.next_kind(), Kind::Fn | Kind::Struct)
                    || parser.next_enum_keyword("enum"))
            {
                parser.take(Kind::Pub).map(|token| {
                    parser.project_recovery = true;
                    token.span
                })
            } else {
                None
            };
            if parser.enum_keyword("enum") {
                parser.enumeration(public).and_then(|enumeration| {
                    parser.candidate_reserve(
                        &mut enumerations,
                        MAX_NODES,
                        "enum declarations",
                        enumeration.name,
                    )?;
                    parser.candidate_reserve(
                        &mut items,
                        MAX_NODES,
                        "syntax items",
                        enumeration.name,
                    )?;
                    items.push(ItemId::Enum(enumerations.len()));
                    enumerations.push(enumeration);
                    Ok(())
                })
            } else if mode.owned() && parser.peek().kind == Kind::Struct {
                parser.record(public).and_then(|record| {
                    parser.candidate_reserve(
                        &mut records,
                        MAX_NODES,
                        "syntax records",
                        record.name,
                    )?;
                    parser.candidate_reserve(&mut items, MAX_NODES, "syntax items", record.name)?;
                    items.push(ItemId::Struct(records.len()));
                    records.push(record);
                    Ok(())
                })
            } else {
                parser.function(public).and_then(|function| {
                    parser.candidate_reserve(
                        &mut functions,
                        MAX_NODES,
                        "syntax functions",
                        function.name,
                    )?;
                    parser.candidate_reserve(
                        &mut items,
                        MAX_NODES,
                        "syntax items",
                        function.name,
                    )?;
                    items.push(ItemId::Function(functions.len()));
                    functions.push(function);
                    Ok(())
                })
            }
        };
        if let Err(error) = result {
            let resource_failure = parser.enums_enabled() && error.code == "E0400";
            diagnostics.push(*error);
            if resource_failure {
                break;
            }
            // Recovery must consume the failing keyword before synchronizing.
            if parser.cursor == before {
                parser.bump();
            }
            while !matches!(parser.peek().kind, Kind::Fn | Kind::Eof)
                && !(mode.owned() && parser.peek().kind == Kind::Struct)
                && !parser.enum_keyword("enum")
                && !(mode == SourceMode::ModuleCandidate
                    && matches!(parser.peek().kind, Kind::Mod | Kind::Pub))
                && !(mode == SourceMode::ProjectCandidate
                    && parser.project_recovery
                    && matches!(parser.peek().kind, Kind::Mod | Kind::Pub | Kind::Use))
            {
                parser.bump();
            }
        }
    }
    if parser.enums_enabled() {
        parser.storage.scratch_capacity -= parser.heights.capacity() * std::mem::size_of::<usize>();
    }
    *storage = parser.storage;
    if diagnostics.is_empty() {
        Ok((
            Program::parsed(
                SourceProvenance::new(source),
                parser.tokens,
                functions,
                parser.expressions,
                records,
                enumerations,
                items,
                modules,
                parser.paths,
                parser.path_segments,
                imports,
            ),
            parser.nodes,
        ))
    } else {
        Err(diagnostics)
    }
}
struct Parser<'a> {
    source: &'a SourceFile,
    allocator: &'a mut Allocator,
    mode: SourceMode,
    arrays: ArraySyntaxPolicy,
    enums: EnumSyntaxPolicy,
    storage: enums::SyntaxStorage,
    // Sticky only after ordinary parsing enters project grammar. Recovery
    // scans never set it; successful source flavor comes from the AST instead.
    project_recovery: bool,
    tokens: Vec<Token>,
    cursor: usize,
    expressions: Vec<Expr>,
    paths: Vec<AbsolutePath>,
    path_segments: Vec<super::source::Span>,
    heights: Vec<usize>,
    nodes: usize,
    node_limit: usize,
}
impl Parser<'_> {
    fn skip(&mut self) {
        while self.tokens[self.cursor].kind == Kind::Trivia {
            self.cursor += 1;
        }
    }
    fn peek(&self) -> Token {
        self.tokens[self.cursor]
    }
    fn bump(&mut self) -> Token {
        let token = self.peek();
        if token.kind != Kind::Eof {
            self.cursor += 1;
            self.skip();
        }
        token
    }
    fn take(&mut self, kind: Kind) -> Option<Token> {
        if self.peek().kind == kind {
            Some(self.bump())
        } else {
            None
        }
    }
    fn next_kind(&self) -> Kind {
        self.tokens[self.cursor + 1..]
            .iter()
            .find(|token| token.kind != Kind::Trivia)
            .map_or(Kind::Eof, |token| token.kind)
    }
    fn double_colon(&self) -> bool {
        self.peek().kind == Kind::Colon
            && self.tokens.get(self.cursor + 1).is_some_and(|next| {
                next.kind == Kind::Colon && self.peek().span.end == next.span.start
            })
    }
    fn project_reserve_error(
        &self,
        error: ReserveFailure,
        span: super::source::Span,
    ) -> Box<Diagnostic> {
        self.diagnostic(
            "E0400",
            "parse",
            match error {
                ReserveFailure::Overflow => "project syntax count overflow",
                ReserveFailure::Allocation => "project syntax allocation failed",
            },
            Some(span),
        )
    }
    fn item_path(
        &mut self,
        message: &str,
        paths: bool,
    ) -> Result<(ItemPath, super::source::Span), Box<Diagnostic>> {
        let first = self.expect(Kind::Ident, message)?.span;
        if self.mode != SourceMode::ProjectCandidate || !self.double_colon() {
            return Ok((ItemPath::Unqualified(first), first));
        }
        // Recognition precedes both denied field paths and denied prefixes.
        self.project_recovery = true;
        if !paths {
            return Err(self.diagnostic(
                "E0101",
                "parse",
                "qualified paths are unavailable in scalar record fields",
                Some(first),
            ));
        }
        if self.source.text_at(first) != "crate" {
            return Err(self.diagnostic(
                "E0101",
                "parse",
                "only absolute item paths beginning with `crate::` are supported",
                Some(first),
            ));
        }
        let id = self.absolute_path(first)?;
        Ok((ItemPath::Absolute(id), self.paths[id.0].span))
    }
    fn path_segment(
        &mut self,
        count: &mut usize,
        segment: super::source::Span,
    ) -> Result<(), Box<Diagnostic>> {
        let next = count
            .checked_add(1)
            .ok_or_else(|| self.project_reserve_error(ReserveFailure::Overflow, segment))?;
        self.path_segments
            .len()
            .checked_add(1)
            .and_then(|length| length.checked_mul(std::mem::size_of::<super::source::Span>()))
            .ok_or_else(|| self.project_reserve_error(ReserveFailure::Overflow, segment))?;
        // Q precedes the aggregate node gate at this exact segment.
        if next > MAX_PATH_SEGMENTS {
            return Err(self.diagnostic(
                "E0400",
                "parse",
                "qualified path segment limit exceeded",
                Some(segment),
            ));
        }
        if self.nodes >= self.node_limit {
            return Err(self.diagnostic(
                "E0400",
                "parse",
                "syntax node limit exceeded",
                Some(segment),
            ));
        }
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or_else(|| self.project_reserve_error(ReserveFailure::Overflow, segment))?;
        if self.enums_enabled() {
            enums::reserve(
                self.allocator,
                &mut self.storage,
                &mut self.path_segments,
                MAX_NODES,
                "qualified path segments",
                false,
                segment,
            )?;
        } else {
            self.allocator
                .vector(&mut self.path_segments, 1, "absolute path segments")
                .map_err(|error| self.project_reserve_error(error, segment))?;
        }
        self.path_segments.push(segment);
        *count = next;
        Ok(())
    }
    fn absolute_path(&mut self, first: super::source::Span) -> Result<PathId, Box<Diagnostic>> {
        let segment_start = self.path_segments.len();
        let mut segment_len = 0;
        self.path_segment(&mut segment_len, first)?;
        let mut end;
        loop {
            self.expect(Kind::Colon, "absolute item path requires `::`")?;
            self.expect(Kind::Colon, "absolute item path requires `::`")?;
            if matches!(self.peek().kind, Kind::Star | Kind::LBrace) {
                return Err(self.diagnostic(
                    "E0101",
                    "parse",
                    "group and glob imports are unavailable in typed-preview",
                    Some(self.peek().span),
                ));
            }
            let segment = self.expect(Kind::Ident, "expected item path segment")?.span;
            self.path_segment(&mut segment_len, segment)?;
            end = segment.end;
            if !self.double_colon() {
                break;
            }
        }
        let span = self.source.span(first.start, end);
        let segment_len = u8::try_from(segment_len)
            .map_err(|_| self.project_reserve_error(ReserveFailure::Overflow, span))?;
        if self.enums_enabled() {
            enums::reserve(
                self.allocator,
                &mut self.storage,
                &mut self.paths,
                MAX_NODES,
                "qualified paths",
                false,
                span,
            )?;
        } else {
            self.allocator
                .vector(&mut self.paths, 1, "absolute paths")
                .map_err(|error| self.project_reserve_error(error, span))?;
        }
        let id = PathId(self.paths.len());
        self.paths.push(AbsolutePath {
            span,
            segment_start,
            segment_len,
            root: PathRoot::Crate,
        });
        Ok(id)
    }
    fn import(&mut self) -> Result<ImportDecl, Box<Diagnostic>> {
        self.node()?;
        let start = self.expect(Kind::Use, "expected import declaration")?.span;
        self.project_recovery = true;
        if matches!(self.peek().kind, Kind::Star | Kind::LBrace) {
            return Err(self.diagnostic(
                "E0101",
                "parse",
                "group and glob imports are unavailable in typed-preview",
                Some(self.peek().span),
            ));
        }
        let (path, _) = self.item_path("expected absolute import path", true)?;
        let ItemPath::Absolute(path) = path else {
            return Err(self.error("import requires an absolute `crate::` item path"));
        };
        let alias =
            if self.peek().kind == Kind::Ident && self.source.text_at(self.peek().span) == "as" {
                self.bump();
                self.expect(Kind::Ident, "expected import alias")?.span
            } else {
                *self
                    .path_segments
                    .last()
                    .expect("absolute path has segments")
            };
        let end = self
            .expect(Kind::Semi, "import declaration requires `;`")?
            .span
            .end;
        Ok(ImportDecl {
            path,
            alias,
            span: self.source.span(start.start, end),
        })
    }
    fn diagnostic(
        &self,
        code: &'static str,
        stage: &'static str,
        message: impl std::fmt::Display,
        primary: Option<super::source::Span>,
    ) -> Box<Diagnostic> {
        if self.mode.owned() {
            super::owned_diagnostic::diagnostic(code, stage, format_args!("{message}"), primary)
        } else {
            Diagnostic::new(code, stage, message.to_string(), primary)
        }
    }
    fn unsupported_token(&self) -> bool {
        let token = self.peek();
        matches!(
            token.kind,
            Kind::Unsupported
                | Kind::Mod
                | Kind::Use
                | Kind::Pub
                | Kind::Number
                | Kind::Minus
                | Kind::Plus
                | Kind::String
                | Kind::Struct
                | Kind::Ampersand
                | Kind::Dot
        ) || (token.kind == Kind::Ident && self.source.text_at(token.span) == "as")
    }
    fn error(&self, message: &str) -> Box<Diagnostic> {
        let token = self.peek();
        let unsupported = self.unsupported_token();
        // Report an unsupported cast in an already-invalid grammar position,
        // without reserving `as` as a declaration or expression identifier.
        // Shared parser diagnostics retain the original scalar formatting. The
        // new owned-only retained-text limit excludes this token-bounded path.
        Diagnostic::new(
            if unsupported { "E0101" } else { "E0100" },
            "parse",
            if unsupported {
                format!(
                    "unsupported typed-preview construct `{}`",
                    self.source.text_at(token.span)
                )
            } else {
                message.to_string()
            },
            Some(token.span),
        )
    }
    fn expect(&mut self, kind: Kind, message: &str) -> Result<Token, Box<Diagnostic>> {
        self.take(kind).ok_or_else(|| self.error(message))
    }
    fn node(&mut self) -> Result<(), Box<Diagnostic>> {
        if self.nodes >= self.node_limit {
            return Err(self.diagnostic(
                "E0400",
                "parse",
                "syntax node limit exceeded",
                Some(self.peek().span),
            ));
        }
        self.nodes += 1;
        Ok(())
    }
    fn module(&mut self) -> Result<ModuleDecl, Box<Diagnostic>> {
        self.node()?;
        let start = self.peek().span.start;
        if self.peek().kind == Kind::Pub
            && !self.tokens[self.cursor + 1..]
                .iter()
                .find(|token| token.kind != Kind::Trivia)
                .is_some_and(|token| token.kind == Kind::Mod)
        {
            return Err(self.error("expected module declaration"));
        }
        let public = self.take(Kind::Pub).map(|token| {
            if self.mode == SourceMode::ProjectCandidate {
                self.project_recovery = true;
            }
            token.span
        });
        self.expect(Kind::Mod, "expected module declaration")?;
        if self.mode == SourceMode::ProjectCandidate {
            self.project_recovery = true;
        }
        let name = self.expect(Kind::Ident, "expected module name")?.span;
        if self.peek().kind == Kind::LBrace {
            return Err(self.diagnostic(
                "E0101",
                "parse",
                "inline modules are unavailable in typed-preview",
                Some(self.peek().span),
            ));
        }
        let end = self
            .expect(Kind::Semi, "module declaration requires `;`")?
            .span
            .end;
        Ok(ModuleDecl {
            name,
            public,
            span: self.source.span(start, end),
        })
    }

    fn function(
        &mut self,
        public: Option<super::source::Span>,
    ) -> Result<Function, Box<Diagnostic>> {
        self.node()?;
        self.expect(Kind::Fn, "expected a top-level function declaration")?;
        let name = self.expect(Kind::Ident, "expected function name")?.span;
        self.expect(Kind::LParen, "expected `(`")?;
        let mut params = Vec::new();
        if self.peek().kind != Kind::RParen {
            loop {
                if params.len() >= MAX_PARAMS {
                    return Err(self.diagnostic(
                        "E0400",
                        "parse",
                        "parameter limit exceeded",
                        Some(self.peek().span),
                    ));
                }
                self.node()?;
                let name = self.expect(Kind::Ident, "expected parameter name")?.span;
                self.expect(Kind::Colon, "parameter requires an explicit type")?;
                let ty = self.parameter_ty()?;
                self.candidate_reserve(&mut params, MAX_PARAMS, "syntax parameters", name)?;
                params.push(Param { name, ty });
                if self.take(Kind::Comma).is_none() {
                    break;
                }
            }
        }
        self.expect(Kind::RParen, "expected `)`")?;
        self.expect(
            Kind::Arrow,
            "function requires an explicit return type after `->`",
        )?;
        let result = self.ty()?;
        let mut blocks = Vec::new();
        let body = self.block(&mut blocks, 0, "expected function body `{`")?;
        let end = blocks[body.0].end;
        Ok(Function {
            public,
            name,
            params,
            result,
            body,
            blocks,
            end,
        })
    }
    fn block(
        &mut self,
        blocks: &mut Vec<BodyBlock>,
        depth: usize,
        opening_message: &str,
    ) -> Result<BodyBlockId, Box<Diagnostic>> {
        // Nested callers check depth before entry. Arena edges prevent a
        // recursive drop chain, including for malformed syntax.
        let start = self.expect(Kind::LBrace, opening_message)?.span;
        let id = BodyBlockId(blocks.len());
        self.candidate_reserve(blocks, 2 * MAX_NODES + 1, "syntax blocks", start)?;
        blocks.push(BodyBlock {
            body: Vec::new(),
            span: start,
            end: start,
        });
        let mut body = Vec::new();
        while !matches!(self.peek().kind, Kind::RBrace | Kind::Eof) {
            let statement = self.statement(blocks, depth + 1)?;
            self.candidate_reserve(&mut body, MAX_NODES, "syntax statements", statement.span)?;
            body.push(statement);
        }
        let end = self
            .expect(Kind::RBrace, "expected `}` before end of file")?
            .span;
        blocks[id.0] = BodyBlock {
            body,
            span: self.source.span(start.start, end.end),
            end,
        };
        Ok(id)
    }
    fn ty(&mut self) -> Result<TypeSyntax, Box<Diagnostic>> {
        self.ty_with_paths(true)
    }
    fn ty_with_paths(&mut self, paths: bool) -> Result<TypeSyntax, Box<Diagnostic>> {
        let token = self.peek();
        let (kind, end) = if self.arrays_enabled() && self.array_punctuation("[") {
            if !paths {
                return Err(self.array_unsupported(token.span));
            }
            let (array, end) = self.array_type()?;
            (TypeSyntaxKind::Array(array), end)
        } else if self.take(Kind::LParen).is_some() {
            (
                TypeSyntaxKind::Unit,
                self.expect(Kind::RParen, "only the unit type `()` is supported here")?
                    .span
                    .end,
            )
        } else {
            let (name, span) = self.item_path("expected `bool`, `i32` or `()` type", paths)?;
            (TypeSyntaxKind::Name(name), span.end)
        };
        Ok(TypeSyntax {
            span: self.source.span(token.span.start, end),
            kind,
        })
    }
    fn parameter_ty(&mut self) -> Result<TypeSyntax, Box<Diagnostic>> {
        if self.mode.owned() && self.peek().kind == Kind::Ampersand {
            let start = self.bump().span.start;
            let mutable = self.take(Kind::Mut).is_some();
            if self.arrays_enabled() && self.array_punctuation("[") {
                let (kind, end) = self.array_parameter_type(mutable)?;
                return Ok(TypeSyntax {
                    span: self.source.span(start, end),
                    kind,
                });
            }
            let (referent, span) =
                self.item_path("reference parameter requires a record name", true)?;
            return Ok(TypeSyntax {
                span: self.source.span(start, span.end),
                kind: TypeSyntaxKind::Reference { mutable, referent },
            });
        }
        self.ty()
    }
    fn record(
        &mut self,
        public: Option<super::source::Span>,
    ) -> Result<StructDecl, Box<Diagnostic>> {
        self.node()?;
        let start = self
            .expect(Kind::Struct, "expected struct declaration")?
            .span;
        let name = self.expect(Kind::Ident, "expected record name")?.span;
        self.expect(Kind::LBrace, "expected record fields `{`")?;
        let mut fields = Vec::new();
        while self.peek().kind != Kind::RBrace {
            self.node()?;
            let public = if self.mode == SourceMode::ProjectCandidate
                && self.peek().kind == Kind::Pub
                && self.next_kind() == Kind::Ident
            {
                self.take(Kind::Pub).map(|token| {
                    self.project_recovery = true;
                    token.span
                })
            } else {
                None
            };
            let name = self.expect(Kind::Ident, "expected field name")?.span;
            self.expect(Kind::Colon, "field requires an explicit value type")?;
            let ty = self.ty_with_paths(true)?;
            self.candidate_reserve(&mut fields, MAX_NODES, "syntax record fields", name)?;
            fields.push(StructField {
                public,
                name,
                ty,
                span: self.source.span(public.unwrap_or(name).start, ty.span.end),
            });
            if self.take(Kind::Comma).is_none() {
                break;
            }
        }
        let end = self
            .expect(Kind::RBrace, "expected record fields `}`")?
            .span;
        Ok(StructDecl {
            public,
            name,
            fields,
            span: self.source.span(public.unwrap_or(start).start, end.end),
            end,
        })
    }
    fn argument(&mut self, depth: usize) -> Result<Argument, Box<Diagnostic>> {
        if self.mode.owned() && self.peek().kind == Kind::Ampersand {
            if depth >= MAX_NESTING {
                return Err(self.diagnostic(
                    "E0400",
                    "parse",
                    "expression nesting limit exceeded",
                    Some(self.peek().span),
                ));
            }
            self.node()?;
            let start = self.bump().span.start;
            let mutable = self.take(Kind::Mut).is_some();
            let star = self.take(Kind::Star);
            let mut name = self
                .expect(Kind::Ident, "borrow argument requires a binding name")?
                .span;
            let mut hops = 0;
            while self.take(Kind::Dot).is_some() {
                if hops >= 64 {
                    return Err(self.diagnostic(
                        "E0400",
                        "parse",
                        "record access path depth limit exceeded",
                        Some(self.peek().span),
                    ));
                }
                self.node()?;
                name.end = self
                    .expect(Kind::Ident, "expected field name after `.`")?
                    .span
                    .end;
                hops += 1;
            }
            if self.mode == SourceMode::ProjectCandidate && self.double_colon() {
                self.project_recovery = true;
                return Err(self.diagnostic(
                    "E0101",
                    "parse",
                    "qualified borrow places are unavailable in typed-preview",
                    Some(self.peek().span),
                ));
            }
            let place = star.map_or(BorrowPlace::OwnerName(name), |star| {
                BorrowPlace::ForwardedParameter {
                    name,
                    star_span: star.span,
                }
            });
            if !matches!(self.peek().kind, Kind::Comma | Kind::RParen) {
                return Err(self.error("borrow must be the complete call argument"));
            }
            return Ok(Argument::Borrow {
                mutable,
                place,
                span: self.source.span(start, name.end),
            });
        }
        Ok(Argument::Value(
            self.expression(depth, LiteralContext::Allowed)?,
        ))
    }
    fn statement(
        &mut self,
        blocks: &mut Vec<BodyBlock>,
        depth: usize,
    ) -> Result<Stmt, Box<Diagnostic>> {
        self.node()?;
        let start = self.peek().span.start;
        if self.enum_keyword("match") {
            return self.match_statement(blocks, depth, start);
        }
        if self.take(Kind::While).is_some() {
            let condition = self.expression(0, LiteralContext::ConditionRoot)?;
            if depth >= MAX_BLOCK_NESTING {
                return Err(self.diagnostic(
                    "E0400",
                    "parse",
                    "statement block nesting limit exceeded",
                    Some(self.peek().span),
                ));
            }
            let body = self.block(blocks, depth, "expected while body `{`")?;
            return Ok(Stmt {
                kind: StmtKind::While { condition, body },
                span: self.source.span(start, blocks[body.0].end.end),
            });
        }
        if self.take(Kind::If).is_some() {
            let condition = self.expression(0, LiteralContext::ConditionRoot)?;
            // `depth` is the number of active statement blocks. Check before
            // recursive entry or arena allocation for either arm.
            if depth >= MAX_BLOCK_NESTING {
                return Err(self.diagnostic(
                    "E0400",
                    "parse",
                    "statement block nesting limit exceeded",
                    Some(self.peek().span),
                ));
            }
            let then_block = self.block(blocks, depth, "expected if body `{`")?;
            let else_block = if self.take(Kind::Else).is_some() {
                Some(self.block(blocks, depth, "expected else body `{`")?)
            } else {
                None
            };
            let end = blocks[else_block.unwrap_or(then_block).0].end.end;
            return Ok(Stmt {
                kind: StmtKind::If {
                    condition,
                    then_block,
                    else_block,
                },
                span: self.source.span(start, end),
            });
        }
        let kind = if self.take(Kind::Let).is_some() {
            let mutable = self.take(Kind::Mut).is_some();
            let name = self.expect(Kind::Ident, "expected binding name")?.span;
            let annotation = if self.take(Kind::Colon).is_some() {
                Some(self.ty()?)
            } else {
                None
            };
            self.expect(Kind::Equal, "binding requires an initializer")?;
            StmtKind::Let {
                mutable,
                name,
                annotation,
                init: self.expression(0, LiteralContext::Allowed)?,
            }
        } else if self.take(Kind::Break).is_some() {
            StmtKind::Break
        } else if self.take(Kind::Continue).is_some() {
            StmtKind::Continue
        } else if self.take(Kind::Return).is_some() {
            StmtKind::Return(if self.peek().kind == Kind::Semi {
                None
            } else {
                Some(self.expression(0, LiteralContext::Allowed)?)
            })
        } else if self.mode.owned()
            && self.peek().kind == Kind::Ident
            && self.field_assignment_ahead()
        {
            let base = self.bump().span;
            self.bump();
            let first = self.bump().span;
            let mut field = first;
            let mut hops = 1;
            while self.take(Kind::Dot).is_some() {
                if hops >= 64 {
                    return Err(self.diagnostic(
                        "E0400",
                        "parse",
                        "record access path depth limit exceeded",
                        Some(self.peek().span),
                    ));
                }
                field.end = self
                    .expect(Kind::Ident, "expected field name after `.`")?
                    .span
                    .end;
                hops += 1;
            }
            let operator_span = self.expect(Kind::Equal, "expected assignment `=`")?.span;
            StmtKind::FieldAssign {
                base,
                field,
                target_span: self.source.span(base.start, field.end),
                operator_span,
                value: self.expression(0, LiteralContext::Allowed)?,
            }
        } else if self.peek().kind == Kind::Ident
            && self.tokens[self.cursor + 1..]
                .iter()
                .find(|token| token.kind != Kind::Trivia)
                .is_some_and(|token| token.kind == Kind::Equal)
        {
            let name = self.bump().span;
            let operator_span = self.expect(Kind::Equal, "expected assignment `=`")?.span;
            StmtKind::Assign {
                name,
                operator_span,
                value: self.expression(0, LiteralContext::Allowed)?,
            }
        } else {
            let target = self.expression(0, LiteralContext::Allowed)?;
            if self.arrays_enabled()
                && self.peek().kind == Kind::Equal
                && self.array_assignment_expression(target)
            {
                if !matches!(self.expressions[target.0].kind, ExprKind::IndexRead { .. }) {
                    return Err(self.array_unsupported(self.expressions[target.0].span));
                }
                let operator_span = self.bump().span;
                StmtKind::IndexAssign {
                    target,
                    operator_span,
                    value: self.expression(0, LiteralContext::Allowed)?,
                }
            } else {
                StmtKind::Expr(target)
            }
        };
        if matches!(kind, StmtKind::Break | StmtKind::Continue) && self.peek().kind != Kind::Semi {
            return Err(self.diagnostic(
                "E0100",
                "parse",
                "loop transfer requires `;`; values and labels are unavailable",
                Some(self.peek().span),
            ));
        }
        let end = self.expect(Kind::Semi, "statement requires `;`")?.span.end;
        Ok(Stmt {
            kind,
            span: self.source.span(start, end),
        })
    }
    fn field_assignment_ahead(&self) -> bool {
        let mut tokens = self.tokens[self.cursor + 1..]
            .iter()
            .filter(|token| token.kind != Kind::Trivia);
        let mut hops = 0;
        loop {
            match tokens.next().map(|token| token.kind) {
                Some(Kind::Dot) if hops <= 64 => {
                    if tokens.next().map(|token| token.kind) != Some(Kind::Ident) {
                        return false;
                    }
                    hops += 1;
                }
                Some(Kind::Equal) => return hops != 0,
                _ => return false,
            }
        }
    }
    fn comparison_op(&self) -> Option<ComparisonOp> {
        Some(match self.peek().kind {
            Kind::EqualEqual => ComparisonOp::Equal,
            Kind::NotEqual => ComparisonOp::NotEqual,
            Kind::Less => ComparisonOp::Less,
            Kind::LessEqual => ComparisonOp::LessEqual,
            Kind::Greater => ComparisonOp::Greater,
            Kind::GreaterEqual => ComparisonOp::GreaterEqual,
            _ => return None,
        })
    }
    fn expression(
        &mut self,
        depth: usize,
        context: LiteralContext,
    ) -> Result<ExprId, Box<Diagnostic>> {
        self.logical(depth, LogicalOp::Or, context)
    }
    /// Canonical primary dispatch, also used before admitting a literal element.
    fn primary_start(&self) -> Option<PrimaryStart> {
        Some(match self.peek().kind {
            Kind::True | Kind::False => PrimaryStart::Boolean,
            Kind::Number | Kind::Minus => PrimaryStart::Number,
            Kind::Ident => PrimaryStart::Name,
            Kind::LParen => PrimaryStart::Group,
            Kind::Unsupported if self.arrays_enabled() && self.array_punctuation("[") => {
                PrimaryStart::Array
            }
            _ => return None,
        })
    }
    fn expression_starter(&self) -> bool {
        self.primary_start().is_some()
            || self.peek().kind == Kind::Not // unary() prefix
            || (self.mode.owned() && self.peek().kind == Kind::Ampersand) // argument() prefix
    }
    fn logical(
        &mut self,
        depth: usize,
        op: LogicalOp,
        context: LiteralContext,
    ) -> Result<ExprId, Box<Diagnostic>> {
        let child = |parser: &mut Self| match op {
            LogicalOp::Or => parser.logical(depth, LogicalOp::And, context),
            LogicalOp::And => parser.comparison(depth, context),
        };
        let mut left = child(self)?;
        let token = match op {
            LogicalOp::And => Kind::AndAnd,
            LogicalOp::Or => Kind::OrOr,
        };
        while let Some(operator) = self.take(token) {
            self.node()?;
            let right = child(self)?;
            let span = self.source.span(
                self.expressions[left.0].span.start,
                self.expressions[right.0].span.end,
            );
            left = self.push_expr(
                ExprKind::Logical {
                    op,
                    left,
                    right,
                    operator_span: operator.span,
                },
                span,
            )?;
        }
        Ok(left)
    }
    fn comparison(
        &mut self,
        depth: usize,
        context: LiteralContext,
    ) -> Result<ExprId, Box<Diagnostic>> {
        let left = self.sum(depth, context)?;
        let Some(op) = self.comparison_op() else {
            return Ok(left);
        };
        let operator_span = self.bump().span;
        self.node()?;
        let right = self.sum(depth, context)?;
        if self.comparison_op().is_some() {
            return Err(self.error("comparison operators cannot be chained; use parentheses"));
        }
        self.push_expr(
            ExprKind::Comparison {
                op,
                left,
                right,
                operator_span,
            },
            self.source.span(
                self.expressions[left.0].span.start,
                self.expressions[right.0].span.end,
            ),
        )
    }
    fn sum(&mut self, depth: usize, context: LiteralContext) -> Result<ExprId, Box<Diagnostic>> {
        let mut left = self.product(depth, context)?;
        loop {
            let op = match self.peek().kind {
                Kind::Plus => ArithmeticOp::Add,
                Kind::Minus => ArithmeticOp::Subtract,
                _ => return Ok(left),
            };
            let operator_span = self.bump().span;
            self.node()?;
            let right = self.product(depth, context)?;
            left = self.binary(op, left, right, operator_span)?;
        }
    }
    fn product(
        &mut self,
        depth: usize,
        context: LiteralContext,
    ) -> Result<ExprId, Box<Diagnostic>> {
        let mut left = self.unary(depth, context)?;
        loop {
            let op = match self.peek().kind {
                Kind::Star => ArithmeticOp::Multiply,
                Kind::Slash => ArithmeticOp::Divide,
                Kind::Percent => ArithmeticOp::Remainder,
                _ => return Ok(left),
            };
            let operator_span = self.bump().span;
            self.node()?;
            let right = self.unary(depth, context)?;
            left = self.binary(op, left, right, operator_span)?;
        }
    }
    fn unary(&mut self, depth: usize, context: LiteralContext) -> Result<ExprId, Box<Diagnostic>> {
        let mut prefixes = Vec::new();
        let result = self.unary_with_prefixes(depth, context, &mut prefixes);
        if self.enums_enabled() {
            self.storage.scratch_capacity -=
                prefixes.capacity() * std::mem::size_of::<super::source::Span>();
        }
        result
    }
    fn unary_with_prefixes(
        &mut self,
        depth: usize,
        context: LiteralContext,
        prefixes: &mut Vec<super::source::Span>,
    ) -> Result<ExprId, Box<Diagnostic>> {
        // Keep minus + decimal on the historical signed-literal path, including
        // intervening trivia, so MIN conversion, origins and fuel stay unchanged.
        // The prefix stack retains only spans; validated one-byte spelling tells
        // us which operator to construct when unwinding without larger scratch.
        while self.peek().kind == Kind::Not
            || (self.peek().kind == Kind::Minus
                && self.tokens[self.cursor + 1..]
                    .iter()
                    .find(|token| token.kind != Kind::Trivia)
                    .is_some_and(|token| token.kind != Kind::Number))
        {
            if depth + prefixes.len() >= MAX_NESTING {
                return Err(self.diagnostic(
                    "E0400",
                    "parse",
                    "expression nesting limit exceeded",
                    Some(self.peek().span),
                ));
            }
            self.node()?;
            if self.enums_enabled() {
                let at = self.peek().span;
                enums::reserve(
                    self.allocator,
                    &mut self.storage,
                    prefixes,
                    MAX_NESTING,
                    "syntax unary prefixes",
                    true,
                    at,
                )?;
            }
            prefixes.push(self.bump().span);
        }
        let mut operand = self.primary(depth + prefixes.len(), context)?;
        while let Some(operator_span) = prefixes.pop() {
            let span = self
                .source
                .span(operator_span.start, self.expressions[operand.0].span.end);
            operand = self.push_expr(
                if self.source.text_at(operator_span) == "-" {
                    ExprKind::Negate {
                        operand,
                        operator_span,
                    }
                } else {
                    ExprKind::Not {
                        operand,
                        operator_span,
                    }
                },
                span,
            )?;
        }
        Ok(operand)
    }
    fn binary(
        &mut self,
        op: ArithmeticOp,
        left: ExprId,
        right: ExprId,
        operator_span: super::source::Span,
    ) -> Result<ExprId, Box<Diagnostic>> {
        let span = self.source.span(
            self.expressions[left.0].span.start,
            self.expressions[right.0].span.end,
        );
        self.push_expr(
            ExprKind::Arithmetic {
                op,
                left,
                right,
                operator_span,
            },
            span,
        )
    }
    fn push_expr(
        &mut self,
        kind: ExprKind,
        span: super::source::Span,
    ) -> Result<ExprId, Box<Diagnostic>> {
        // A flat left-associative chain is a deep tree too. Bound total tree
        // height before resolution, whose recursive visits now remain <= 64.
        let height = 1 + match &kind {
            ExprKind::Group(inner)
            | ExprKind::Negate { operand: inner, .. }
            | ExprKind::Not { operand: inner, .. } => self.heights[inner.0],
            ExprKind::Call { args, .. } => args
                .iter()
                .map(|arg| match arg {
                    Argument::Value(id) => self.heights[id.0],
                    Argument::Borrow { .. } => 1,
                })
                .max()
                .unwrap_or(0),
            ExprKind::QualifiedValue { args, .. } => args
                .iter()
                .flatten()
                .map(|arg| match arg {
                    Argument::Value(id) => self.heights[id.0],
                    Argument::Borrow { .. } => 1,
                })
                .max()
                .unwrap_or(0),
            ExprKind::StructLiteral { fields, .. } => fields
                .iter()
                .map(|f| self.heights[f.value.0])
                .max()
                .unwrap_or(0),
            ExprKind::ArrayLiteral { elements } => elements
                .iter()
                .map(|element| self.heights[element.0])
                .max()
                .unwrap_or(0),
            ExprKind::IndexRead { index, .. } => self.heights[index.0],
            ExprKind::Arithmetic { left, right, .. }
            | ExprKind::Comparison { left, right, .. }
            | ExprKind::Logical { left, right, .. } => {
                self.heights[left.0].max(self.heights[right.0])
            }
            _ => 0,
        };
        if height > MAX_NESTING {
            return Err(self.diagnostic(
                "E0400",
                "parse",
                "expression nesting limit exceeded",
                Some(span),
            ));
        }
        if self.enums_enabled() {
            enums::reserve(
                self.allocator,
                &mut self.storage,
                &mut self.expressions,
                MAX_NODES,
                "syntax expressions",
                false,
                span,
            )?;
            enums::reserve(
                self.allocator,
                &mut self.storage,
                &mut self.heights,
                MAX_NODES,
                "syntax expression heights",
                true,
                span,
            )?;
        }
        let id = ExprId(self.expressions.len());
        self.expressions.push(Expr { kind, span });
        self.heights.push(height);
        Ok(id)
    }
    fn primary(
        &mut self,
        depth: usize,
        context: LiteralContext,
    ) -> Result<ExprId, Box<Diagnostic>> {
        if depth >= MAX_NESTING {
            return Err(self.diagnostic(
                "E0400",
                "parse",
                "expression nesting limit exceeded",
                Some(self.peek().span),
            ));
        }
        self.node()?;
        let token = self.peek();
        let mut end = token.span.end;
        let kind = match self.primary_start() {
            Some(PrimaryStart::Boolean) => {
                self.bump();
                ExprKind::Bool(token.kind == Kind::True)
            }
            Some(PrimaryStart::Number) => {
                let negative = token.kind == Kind::Minus;
                if negative {
                    self.bump();
                    if self.peek().kind != Kind::Number {
                        return Err(self.diagnostic(
                            "E0101",
                            "parse",
                            "only a minus followed by decimal literal digits is supported",
                            Some(token.span),
                        ));
                    }
                }
                let digits = self.bump().span;
                end = digits.end;
                if !self
                    .source
                    .text_at(digits)
                    .bytes()
                    .all(|byte| byte.is_ascii_digit())
                {
                    return Err(self.diagnostic(
                        "E0101",
                        "parse",
                        "unsupported numeric spelling; expected ASCII decimal literal digits",
                        Some(self.source.span(token.span.start, end)),
                    ));
                }
                ExprKind::Number { digits, negative }
            }
            Some(PrimaryStart::Name) => {
                let qualified = if self.enum_qualified_ahead() {
                    Some(self.enum_path()?)
                } else {
                    None
                };
                let (path, path_span) = match qualified {
                    Some(id) => (ItemPath::Absolute(id), self.paths[id.0].span),
                    None => self.item_path("expected item name", true)?,
                };
                end = path_span.end;
                if self.take(Kind::LParen).is_some() {
                    let mut args = Vec::new();
                    if self.peek().kind != Kind::RParen {
                        loop {
                            if args.len() >= MAX_PARAMS {
                                return Err(self.diagnostic(
                                    "E0400",
                                    "parse",
                                    "argument limit exceeded",
                                    Some(self.peek().span),
                                ));
                            }
                            let argument = self.argument(depth + 1)?;
                            let label = if qualified.is_some() {
                                "qualified value arguments"
                            } else {
                                "syntax call arguments"
                            };
                            self.candidate_reserve(&mut args, MAX_PARAMS, label, token.span)?;
                            args.push(argument);
                            if self.take(Kind::Comma).is_none() {
                                break;
                            }
                        }
                    }
                    end = self.expect(Kind::RParen, "call requires `)`")?.span.end;
                    if let Some(path) = qualified {
                        ExprKind::QualifiedValue {
                            path,
                            args: Some(args),
                        }
                    } else {
                        ExprKind::Call { callee: path, args }
                    }
                } else if self.arrays_enabled()
                    && matches!(path, ItemPath::Unqualified(_))
                    && self.array_punctuation("[")
                {
                    self.bump();
                    if self.array_punctuation("]") {
                        return Err(self.array_missing("array index requires an expression"));
                    }
                    let index = self.expression(depth + 1, LiteralContext::Allowed)?;
                    end = self.array_close("array index requires `]`")?;
                    ExprKind::IndexRead {
                        base: token.span,
                        index,
                    }
                } else if self.mode.owned()
                    && matches!(path, ItemPath::Unqualified(_))
                    && self.take(Kind::Dot).is_some()
                {
                    let first = self
                        .expect(Kind::Ident, "expected field name after `.`")?
                        .span;
                    let mut field = first;
                    let mut receiver_end = token.span.end;
                    let mut hops = 1usize;
                    loop {
                        if self.mode == SourceMode::ProjectCandidate && self.double_colon() {
                            self.project_recovery = true;
                            return Err(self.diagnostic(
                                "E0101",
                                "parse",
                                "qualified field names are unavailable in typed-preview",
                                Some(self.peek().span),
                            ));
                        }
                        if self.peek().kind != Kind::Dot {
                            break;
                        }
                        if hops >= 65 {
                            return Err(self.diagnostic(
                                "E0400",
                                "parse",
                                "record access path depth limit exceeded",
                                Some(self.peek().span),
                            ));
                        }
                        receiver_end = field.end;
                        self.bump();
                        field = self
                            .expect(Kind::Ident, "expected field name after `.`")?
                            .span;
                        hops += 1;
                    }
                    let is_length = self.arrays_enabled()
                        && self.peek().kind == Kind::LParen
                        && self.source.text_at(field) == "len";
                    if hops - usize::from(is_length) > 64 {
                        return Err(self.diagnostic(
                            "E0400",
                            "parse",
                            "record access path depth limit exceeded",
                            Some(field),
                        ));
                    }
                    if self.arrays_enabled() && self.peek().kind == Kind::LParen {
                        if self.source.text_at(field) != "len" {
                            return Err(self.array_unsupported(self.peek().span));
                        }
                        self.bump();
                        end = self.array_length_close()?;
                        ExprKind::ArrayLength {
                            base: self.source.span(token.span.start, receiver_end),
                        }
                    } else if self.arrays_enabled() && self.array_punctuation("[") {
                        self.bump();
                        if self.array_punctuation("]") {
                            return Err(self.array_missing("array index requires an expression"));
                        }
                        let index = self.expression(depth + 1, LiteralContext::Allowed)?;
                        end = self.array_close("array index requires `]`")?;
                        ExprKind::IndexRead {
                            base: self.source.span(token.span.start, field.end),
                            index,
                        }
                    } else {
                        end = field.end;
                        ExprKind::FieldRead {
                            base: token.span,
                            field: self.source.span(first.start, field.end),
                        }
                    }
                } else if self.mode.owned()
                    && context == LiteralContext::Allowed
                    && self.take(Kind::LBrace).is_some()
                {
                    if qualified.is_some_and(|id| self.paths[id.0].root == PathRoot::LocalType) {
                        return Err(self.diagnostic(
                            "E0100",
                            "parse",
                            "enum variants do not use record literal syntax",
                            Some(path_span),
                        ));
                    }
                    let mut fields = Vec::new();
                    while self.peek().kind != Kind::RBrace {
                        self.node()?;
                        let name = self
                            .expect(Kind::Ident, "expected literal field name")?
                            .span;
                        self.expect(Kind::Colon, "literal field requires `:` and a value")?;
                        let value = self.expression(depth + 1, LiteralContext::Allowed)?;
                        self.candidate_reserve(
                            &mut fields,
                            MAX_NODES,
                            "syntax literal fields",
                            name,
                        )?;
                        fields.push(FieldInit {
                            name,
                            value,
                            span: self
                                .source
                                .span(name.start, self.expressions[value.0].span.end),
                        });
                        if self.take(Kind::Comma).is_none() {
                            break;
                        }
                    }
                    end = self.expect(Kind::RBrace, "expected literal `}`")?.span.end;
                    ExprKind::StructLiteral {
                        record: path,
                        fields,
                    }
                } else if let Some(path) = qualified {
                    ExprKind::QualifiedValue { path, args: None }
                } else if matches!(path, ItemPath::Absolute(_)) {
                    return Err(self.diagnostic(
                        "E0101",
                        "parse",
                        "qualified item paths require a direct call or constructor",
                        Some(path_span),
                    ));
                } else {
                    ExprKind::Name(token.span)
                }
            }
            Some(PrimaryStart::Group) => {
                self.bump();
                if let Some(close) = self.take(Kind::RParen) {
                    end = close.span.end;
                    ExprKind::Unit
                } else {
                    let inner = self.expression(depth + 1, LiteralContext::Allowed)?;
                    end = self.expect(Kind::RParen, "grouping requires `)`")?.span.end;
                    ExprKind::Group(inner)
                }
            }
            Some(PrimaryStart::Array) => {
                let (elements, close) = self.array_literal(depth)?;
                end = close;
                ExprKind::ArrayLiteral { elements }
            }
            None => return Err(self.error("expected a bool, i32 or unit expression")),
        };
        if self.arrays_enabled() && (self.array_punctuation("[") || self.peek().kind == Kind::Dot) {
            return Err(self.array_unsupported(self.peek().span));
        }
        self.push_expr(kind, self.source.span(token.span.start, end))
    }
}

#[path = "parser/arrays.rs"]
mod arrays;
#[path = "parser/enums.rs"]
mod enums;

#[cfg(test)]
#[path = "parser/array_syntax_tests.rs"]
mod array_syntax_tests;
#[cfg(test)]
#[path = "parser/enum_syntax_tests.rs"]
mod enum_syntax_tests;

#[cfg(test)]
pub(super) fn parse_with_lowered_node_limit(
    source: &SourceFile,
    tokens: Vec<Token>,
    mode: SourceMode,
    limit: usize,
) -> Result<Program, Vec<Diagnostic>> {
    parse_with_node_limit(source, tokens, mode, limit)
}

#[cfg(test)]
#[path = "parser/project_tests.rs"]
mod project_tests;

#[cfg(test)]
#[path = "parser/activation_tests.rs"]
mod activation_tests;
