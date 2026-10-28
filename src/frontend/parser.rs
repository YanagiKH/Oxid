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
    text_len: u32,
    file: u32,
    generics: Option<Box<[GenericSyntax; 1]>>,
}
impl std::fmt::Debug for SourceProvenance {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The generation binds ownership, not syntax. Keep structural AST
        // observations deterministic across identical source allocations.
        formatter
            .debug_struct("SourceProvenance")
            .field("text_len", &(self.text_len as usize))
            .field("file", &super::source::SourceFileId(self.file as usize))
            .finish_non_exhaustive()
    }
}
impl SourceProvenance {
    fn new(
        source: &SourceFile,
        headers: Vec<GenericHeader>,
        calls: Vec<GenericArguments>,
        allocator: &mut Allocator,
    ) -> Result<Self, Box<Diagnostic>> {
        let failure = || {
            Diagnostic::new(
                "E0400",
                "parse",
                "generic source metadata allocation or compact source limit exceeded",
                Some(source.span(0, 0)),
            )
        };
        let text_len = u32::try_from(source.text().len()).map_err(|_| failure())?;
        let file = u32::try_from(source.span(0, 0).file.0).map_err(|_| failure())?;
        let generics = if headers.is_empty() && calls.is_empty() {
            None
        } else {
            Some(
                allocator
                    .boxed_carrier(GenericSyntax { headers, calls }, "generic syntax owner")
                    .map_err(|_| failure())?,
            )
        };
        Ok(Self {
            identity: source.identity(),
            text_len,
            file,
            generics,
        })
    }
    pub(super) fn generic_headers(&self) -> &[GenericHeader] {
        self.generics
            .as_ref()
            .map_or(&[], |owner| owner[0].headers.as_slice())
    }
    pub(super) fn generic_calls(&self) -> &[GenericArguments] {
        self.generics
            .as_ref()
            .map_or(&[], |owner| owner[0].calls.as_slice())
    }
    pub(super) fn has_generics(&self) -> bool {
        self.generics.is_some()
    }
    pub(super) fn belongs_to(&self, source: &SourceFile) -> bool {
        self.identity == source.identity()
            && self.text_len as usize == source.text().len()
            && self.file as usize == source.span(0, 0).file.0
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
/// Typed syntax admission stays distinct from private qualification policy.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EnumSyntaxPolicy {
    Closed,
    Enabled,
    #[cfg(test)]
    Candidate,
}
impl EnumSyntaxPolicy {
    fn enabled(self) -> bool {
        match self {
            Self::Closed => false,
            Self::Enabled => true,
            #[cfg(test)]
            Self::Candidate => true,
        }
    }
}

/// Std is recognized only at an import edge. Current source admits the closed
/// catalog; the candidate policy retains its separate qualification route.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StdImportPolicy {
    #[cfg(test)]
    OutputCandidate,
    Closed,
    Enabled,
    #[cfg(test)]
    Candidate,
}
impl StdImportPolicy {
    pub(super) fn enabled(self) -> bool {
        match self {
            Self::Closed => false,
            Self::Enabled => true,
            #[cfg(test)]
            Self::Candidate | Self::OutputCandidate => true,
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
    Character,
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
        StdImportPolicy::Closed,
        &mut enums::SyntaxStorage::default(),
    )
}

/// Bounded production typed grammar; callers select this explicitly.
#[allow(dead_code)]
pub(super) fn parse_typed_counted(
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
        EnumSyntaxPolicy::Enabled,
        StdImportPolicy::Enabled,
        storage,
    )
}

/// Preserve the pre-stdin typed grammar for negative qualification controls.
#[cfg(test)]
pub(super) fn parse_typed_closed_std_counted(
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
        EnumSyntaxPolicy::Enabled,
        StdImportPolicy::Closed,
        storage,
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
        StdImportPolicy::Closed,
        storage,
    )
}

#[cfg(test)]
pub(super) fn parse_builtin_candidate_counted(
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
        EnumSyntaxPolicy::Enabled,
        StdImportPolicy::Candidate,
        storage,
    )
}

#[cfg(test)]
pub(super) fn parse_output_candidate_counted(
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
        EnumSyntaxPolicy::Enabled,
        StdImportPolicy::OutputCandidate,
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
    std_imports: StdImportPolicy,
    storage: &mut enums::SyntaxStorage,
) -> Result<(Program, usize), Vec<Diagnostic>> {
    *storage = enums::SyntaxStorage::default();
    let mut parser = Parser {
        source,
        allocator,
        mode,
        arrays,
        enums,
        std_imports,
        storage: enums::SyntaxStorage::default(),
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
            (|| {
                let derive_copy = parser.copy_attribute()?;
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
                if derive_copy.is_some() && parser.peek().kind != Kind::Struct {
                    return Err(parser.diagnostic(
                        "E0202",
                        "parse",
                        "Copy derivation requires a record declaration",
                        derive_copy,
                    ));
                }
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
                    parser.record(public, derive_copy).and_then(|record| {
                        parser.candidate_reserve(
                            &mut records,
                            MAX_NODES,
                            "syntax records",
                            record.name,
                        )?;
                        parser.candidate_reserve(
                            &mut items,
                            MAX_NODES,
                            "syntax items",
                            record.name,
                        )?;
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
            })()
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
        let has_generics = !parser.generic_headers.is_empty() || !parser.generic_calls.is_empty();
        let provenance = SourceProvenance::new(
            source,
            parser.generic_headers,
            parser.generic_calls,
            parser.allocator,
        )
        .map_err(|error| vec![*error])?;
        if has_generics && parser.enums.enabled() {
            let bytes = std::mem::size_of::<GenericSyntax>();
            storage.retained_capacity =
                storage
                    .retained_capacity
                    .checked_add(bytes)
                    .ok_or_else(|| {
                        vec![*Diagnostic::new(
                            "E0400",
                            "parse",
                            "generic syntax storage overflow",
                            Some(source.span(0, 0)),
                        )]
                    })?;
            storage.peak_capacity_bound = storage
                .peak_capacity_bound
                .max(storage.retained_capacity + storage.scratch_capacity);
        }
        Ok((
            Program::parsed(
                provenance,
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
                parser.parsed_has_named_slice,
                parser.parsed_has_copy_records,
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
    std_imports: StdImportPolicy,
    storage: enums::SyntaxStorage,
    // Sticky only after ordinary parsing enters project grammar. Recovery
    // scans never set it; successful source flavor comes from the AST instead.
    project_recovery: bool,
    // Written in the already bounded named-slice parse branch, with no AST scan.
    parsed_has_named_slice: bool,
    parsed_has_copy_records: bool,
    tokens: Vec<Token>,
    cursor: usize,
    pending_equal: Option<Token>,
    expressions: Vec<Expr>,
    paths: Vec<AbsolutePath>,
    path_segments: Vec<super::source::Span>,
    generic_headers: Vec<GenericHeader>,
    generic_calls: Vec<GenericArguments>,
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
        self.pending_equal.unwrap_or(self.tokens[self.cursor])
    }
    fn bump(&mut self) -> Token {
        if let Some(token) = self.pending_equal.take() {
            return token;
        }
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
        if self.pending_equal.is_some() {
            return self.tokens[self.cursor].kind;
        }
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
    fn type_arguments_ahead(&self) -> bool {
        self.mode.owned()
            && self.double_colon()
            && self.tokens[self.cursor + 2..]
                .iter()
                .find(|token| token.kind != Kind::Trivia)
                .is_some_and(|token| token.kind == Kind::Less)
    }
    fn double_dot(&self) -> bool {
        self.peek().kind == Kind::Dot
            && self.tokens.get(self.cursor + 1).is_some_and(|next| {
                next.kind == Kind::Dot && self.peek().span.end == next.span.start
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
        if self.mode != SourceMode::ProjectCandidate
            || !self.double_colon()
            || self.type_arguments_ahead()
        {
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
    fn import_path(&mut self) -> Result<PathId, Box<Diagnostic>> {
        let first = self
            .expect(Kind::Ident, "expected absolute import path")?
            .span;
        if !self.double_colon() {
            return Err(self.error("import requires an absolute `crate::` item path"));
        }
        let root = match self.source.text_at(first) {
            "crate" => PathRoot::Crate,
            "std" if self.std_imports.enabled() => PathRoot::Std,
            "alloc" if self.std_imports == StdImportPolicy::Enabled => PathRoot::Std,
            _ => {
                return Err(self.diagnostic(
                    "E0101",
                    "parse",
                    "only absolute item paths beginning with `crate::` are supported",
                    Some(first),
                ))
            }
        };
        let id = self.qualified_segments(first, root)?;
        if root == PathRoot::Std && self.std_imports == StdImportPolicy::Enabled {
            use super::builtin_catalog::{experimental, vector, BuiltinEnum, BuiltinFunction};
            let path = &self.paths[id.0];
            let segments = &self.path_segments[path.segment_start..];
            let experimental = segments.len() == 3
                && experimental::Item::ALL.into_iter().any(|item| {
                    item.path()
                        .into_iter()
                        .zip(segments)
                        .all(|(name, span)| self.source.text_at(*span) == name)
                });
            let vector = segments.len() == 3
                && (vector::Item::ALL.into_iter().any(|item| {
                    item.path()
                        .into_iter()
                        .zip(segments)
                        .all(|(name, span)| self.source.text_at(*span) == name)
                }) || ["alloc", "vec", "GrowStatus"]
                    .into_iter()
                    .zip(segments)
                    .all(|(name, span)| self.source.text_at(*span) == name));
            if !experimental
                && !vector
                && (segments.len() != 3
                    || self.source.text_at(segments[0]) != "std"
                    || self.source.text_at(segments[1]) != "io"
                    || (self.source.text_at(segments[2]) != BuiltinEnum::ReadStatus.name()
                        && self.source.text_at(segments[2]) != BuiltinFunction::ReadStdin.name()
                        && self.source.text_at(segments[2]) != BuiltinEnum::WriteStatus.name()
                        && self.source.text_at(segments[2]) != BuiltinFunction::WriteStdout.name()))
            {
                return Err(self.diagnostic(
                    "E0101",
                    "parse",
                    "unsupported standard library import",
                    Some(path.span),
                ));
            }
        }
        Ok(id)
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
        self.qualified_segments(first, PathRoot::Crate)
    }
    fn qualified_segments(
        &mut self,
        first: super::source::Span,
        root: PathRoot,
    ) -> Result<PathId, Box<Diagnostic>> {
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
            if !self.double_colon() || self.type_arguments_ahead() {
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
            root,
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
        let path = self.import_path()?;
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

    fn generic_parameters(
        &mut self,
        declaration: super::source::Span,
    ) -> Result<(), Box<Diagnostic>> {
        let mut parameters = Vec::new();
        if self.peek().kind != Kind::Less {
            return Ok(());
        }
        if !self.mode.owned() {
            return Err(self.diagnostic(
                "E0002",
                "parse",
                "generic parameters require typed project syntax",
                Some(self.peek().span),
            ));
        }
        self.bump();
        if self.peek().kind == Kind::Greater {
            return Err(self.diagnostic(
                "E0002",
                "parse",
                "generic parameter list cannot be empty",
                Some(self.peek().span),
            ));
        }
        loop {
            self.node()?;
            let name = self
                .expect(Kind::Ident, "expected type parameter name")?
                .span;
            let mut bounds = Vec::new();
            let mut end = name.end;
            if self.take(Kind::Colon).is_some() {
                loop {
                    let (bound, span) = self.item_path("expected trait bound", true)?;
                    self.candidate_reserve(&mut bounds, MAX_PARAMS, "type parameter bounds", span)?;
                    bounds.push(bound);
                    end = span.end;
                    if self.take(Kind::Plus).is_none() {
                        break;
                    }
                }
            }
            self.candidate_reserve(&mut parameters, MAX_PARAMS, "generic parameters", name)?;
            parameters.push(GenericParam {
                name,
                bounds,
                span: self.source.span(name.start, end),
            });
            if self.take(Kind::Comma).is_none() || self.peek().kind == Kind::Greater {
                break;
            }
        }
        self.expect(Kind::Greater, "expected `>` after type parameters")?;
        let mut headers = std::mem::take(&mut self.generic_headers);
        self.candidate_reserve(
            &mut headers,
            MAX_NODES,
            "generic declaration headers",
            declaration,
        )?;
        headers.push(GenericHeader {
            declaration,
            parameters,
        });
        self.generic_headers = headers;
        Ok(())
    }
    fn generic_call_arguments(
        &mut self,
        path: ItemPath,
        callee: super::source::Span,
    ) -> Result<GenericApplicationId, Box<Diagnostic>> {
        let start = self.bump().span.start;
        self.expect(Kind::Colon, "type arguments require `::`")?;
        let id = self.type_arguments(path, callee, GenericArgumentContext::Expression, start, 0)?;
        if !self.wide_scalars_enabled() && self.peek().kind != Kind::LParen {
            return Err(self.error("explicit function type arguments require a call"));
        }
        if !matches!(self.peek().kind, Kind::LParen | Kind::LBrace) && !self.double_colon() {
            return Err(self.error("explicit type arguments require a call or constructor"));
        }
        Ok(id)
    }
    fn type_arguments(
        &mut self,
        path: ItemPath,
        callee: super::source::Span,
        context: GenericArgumentContext,
        start: usize,
        depth: usize,
    ) -> Result<GenericApplicationId, Box<Diagnostic>> {
        if !self.mode.owned() {
            return Err(self.error("generic type arguments require typed project syntax"));
        }
        if context == GenericArgumentContext::Type && !self.wide_scalars_enabled() {
            return Err(self.error("generic nominal types require current typed project syntax"));
        }
        if depth >= MAX_NESTING {
            return Err(self.diagnostic(
                "E0400",
                "parse",
                "type application nesting limit exceeded",
                Some(self.peek().span),
            ));
        }
        self.expect(Kind::Less, "type arguments require `<`")?;
        if matches!(self.peek().kind, Kind::Greater | Kind::GreaterEqual) {
            return Err(self.error("type argument list cannot be empty"));
        }
        let mut calls = std::mem::take(&mut self.generic_calls);
        self.candidate_reserve(&mut calls, MAX_NODES, "generic application syntax", callee)?;
        let id = GenericApplicationId(calls.len());
        calls.push(GenericArguments {
            callee,
            path,
            arguments: Vec::new(),
            span: self.source.span(start, start),
            context,
        });
        self.generic_calls = calls;
        let mut arguments = Vec::new();
        loop {
            self.node()?;
            let argument = self.ty_with_paths_at(true, depth + 1)?;
            self.candidate_reserve(
                &mut arguments,
                MAX_PARAMS,
                "explicit type arguments",
                argument.span,
            )?;
            arguments.push(argument);
            if self.take(Kind::Comma).is_none()
                || matches!(self.peek().kind, Kind::Greater | Kind::GreaterEqual)
            {
                break;
            }
        }
        let end = if self.peek().kind == Kind::GreaterEqual {
            let token = self.bump();
            let middle = token.span.start + 1;
            self.pending_equal = Some(Token {
                kind: Kind::Equal,
                span: self.source.span(middle, token.span.end),
            });
            middle
        } else {
            self.expect(Kind::Greater, "expected `>` after type arguments")?
                .span
                .end
        };
        self.generic_calls[id.0].arguments = arguments;
        self.generic_calls[id.0].span = self.source.span(start, end);
        Ok(id)
    }
    fn function(
        &mut self,
        public: Option<super::source::Span>,
    ) -> Result<Function, Box<Diagnostic>> {
        self.node()?;
        self.expect(Kind::Fn, "expected a top-level function declaration")?;
        let name = self.expect(Kind::Ident, "expected function name")?.span;
        self.generic_parameters(name)?;
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
        self.ty_with_paths_at(paths, 0)
    }
    fn ty_with_paths_at(
        &mut self,
        paths: bool,
        depth: usize,
    ) -> Result<TypeSyntax, Box<Diagnostic>> {
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
            if self.peek().kind == Kind::Less {
                let id = self.type_arguments(
                    name,
                    span,
                    GenericArgumentContext::Type,
                    self.peek().span.start,
                    depth,
                )?;
                (
                    TypeSyntaxKind::Applied(id),
                    self.generic_calls[id.0].span.end,
                )
            } else {
                (TypeSyntaxKind::Name(name), span.end)
            }
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
            if self.peek().kind == Kind::Less {
                let application = self.type_arguments(
                    referent,
                    span,
                    GenericArgumentContext::Type,
                    self.peek().span.start,
                    0,
                )?;
                return Ok(TypeSyntax {
                    span: self
                        .source
                        .span(start, self.generic_calls[application.0].span.end),
                    kind: TypeSyntaxKind::AppliedReference {
                        mutable,
                        application,
                    },
                });
            }
            return Ok(TypeSyntax {
                span: self.source.span(start, span.end),
                kind: TypeSyntaxKind::Reference { mutable, referent },
            });
        }
        self.ty()
    }
    fn copy_attribute(&mut self) -> Result<Option<super::source::Span>, Box<Diagnostic>> {
        if !self.wide_scalars_enabled() || self.source.text_at(self.peek().span) != "#" {
            return Ok(None);
        }
        self.node()?;
        let start = self.bump().span;
        let mut end = start.end;
        for spelling in ["[", "derive", "(", "Copy", ")", "]"] {
            if self.source.text_at(self.peek().span) != spelling {
                return Err(self.diagnostic(
                    "E0202",
                    "parse",
                    "only #[derive(Copy)] on records is supported",
                    Some(self.peek().span),
                ));
            }
            end = self.bump().span.end;
        }
        self.parsed_has_copy_records = true;
        Ok(Some(self.source.span(start.start, end)))
    }
    fn record(
        &mut self,
        public: Option<super::source::Span>,
        derive_copy: Option<super::source::Span>,
    ) -> Result<StructDecl, Box<Diagnostic>> {
        self.node()?;
        let start = self
            .expect(Kind::Struct, "expected struct declaration")?
            .span;
        let name = self.expect(Kind::Ident, "expected record name")?.span;
        self.generic_parameters(name)?;
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
            derive_copy,
            public,
            name,
            fields,
            span: self
                .source
                .span(derive_copy.or(public).unwrap_or(start).start, end.end),
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
            let mut place = star.map_or(BorrowPlace::OwnerName(name), |star| {
                BorrowPlace::ForwardedParameter {
                    name,
                    star_span: star.span,
                }
            });
            let mut end = name.end;
            if self.arrays_enabled() && self.array_punctuation("[") {
                let bracket = self.bump();
                let range_start = self.expression(depth + 1, LiteralContext::Allowed)?;
                if !self.double_dot() {
                    return Err(self.array_unsupported(bracket.span));
                }
                self.bump();
                self.bump();
                let range_end = self.expression(depth + 1, LiteralContext::Allowed)?;
                end = self.array_close("byte view range requires `]`")?;
                let star = star
                    .map(|token| {
                        u32::try_from(token.span.start).map_err(|_| {
                            self.project_reserve_error(ReserveFailure::Overflow, token.span)
                        })
                    })
                    .transpose()?;
                place = BorrowPlace::Range {
                    name,
                    start: range_start,
                    end: range_end,
                    star,
                };
            }
            if !matches!(self.peek().kind, Kind::Comma | Kind::RParen) {
                return Err(self.error("borrow must be the complete call argument"));
            }
            return Ok(Argument::Borrow {
                mutable,
                place,
                span: self.source.span(start, end),
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
        if self.enum_keyword("for")
            && self.tokens[self.cursor + 1..]
                .iter()
                .find(|token| token.kind != Kind::Trivia)
                .is_some_and(|token| token.kind == Kind::Ident)
        {
            self.bump();
            let name = self
                .expect(Kind::Ident, "expected range loop binding")?
                .span;
            let keyword = self.expect(Kind::Ident, "range loop requires `in`")?;
            if self.source.text_at(keyword.span) != "in" {
                return Err(self.diagnostic(
                    "E0100",
                    "parse",
                    "range loop requires `in`",
                    Some(keyword.span),
                ));
            }
            let first = self.expression(0, LiteralContext::ConditionRoot)?;
            if !self.double_dot() {
                return Err(self.error("range loop requires an exclusive `start..end` range"));
            }
            self.bump();
            self.bump();
            let end = self.expression(0, LiteralContext::ConditionRoot)?;
            if depth >= MAX_BLOCK_NESTING {
                return Err(self.diagnostic(
                    "E0400",
                    "parse",
                    "statement block nesting limit exceeded",
                    Some(self.peek().span),
                ));
            }
            let body = self.block(blocks, depth, "expected range loop body `{`")?;
            return Ok(Stmt {
                kind: StmtKind::ForRange {
                    name,
                    start: first,
                    end,
                    body,
                },
                span: self.source.span(start, blocks[body.0].end.end),
            });
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
            let (path, at) = if self.wide_scalars_enabled() {
                self.item_path("expected binding name or record pattern", true)?
            } else {
                let name = self.expect(Kind::Ident, "expected binding name")?.span;
                (ItemPath::Unqualified(name), name)
            };
            if self.wide_scalars_enabled() && self.take(Kind::LBrace).is_some() {
                if mutable {
                    return Err(self.error("record patterns declare immutable bindings"));
                }
                let mut fields = Vec::new();
                while self.peek().kind != Kind::RBrace {
                    self.node()?;
                    let field = self
                        .expect(Kind::Ident, "expected record pattern field")?
                        .span;
                    let binding = if self.take(Kind::Colon).is_some() {
                        self.expect(Kind::Ident, "expected field binding name")?
                            .span
                    } else {
                        field
                    };
                    self.candidate_reserve(&mut fields, MAX_NODES, "record pattern fields", field)?;
                    fields.push(RecordPatternField {
                        field,
                        binding,
                        span: self.source.span(field.start, binding.end),
                    });
                    if self.take(Kind::Comma).is_none() {
                        break;
                    }
                }
                self.expect(Kind::RBrace, "expected record pattern `}`")?;
                self.expect(Kind::Equal, "record pattern requires an initializer")?;
                StmtKind::DestructureRecord {
                    record: path,
                    fields,
                    init: self.expression(0, LiteralContext::Allowed)?,
                }
            } else {
                let ItemPath::Unqualified(name) = path else {
                    return Err(self.diagnostic(
                        "E0100",
                        "parse",
                        "qualified binding requires a record pattern",
                        Some(at),
                    ));
                };
                let annotation = if self.take(Kind::Colon).is_some() {
                    let ty = self.parameter_ty()?;
                    if matches!(ty.kind, TypeSyntaxKind::GenericSliceReference { .. }) {
                        return Err(self.diagnostic(
                            "E0101",
                            "parse",
                            "named slice references are only supported as parameters",
                            Some(ty.span),
                        ));
                    }
                    Some(ty)
                } else {
                    None
                };
                self.expect(Kind::Equal, "binding requires an initializer")?;
                StmtKind::Let {
                    mutable,
                    name,
                    annotation,
                    init: if self.mode.owned() && self.peek().kind == Kind::Ampersand {
                        self.local_reference_initializer()?
                    } else {
                        self.expression(0, LiteralContext::Allowed)?
                    },
                }
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
    fn local_reference_initializer(&mut self) -> Result<ExprId, Box<Diagnostic>> {
        self.node()?;
        let start = self
            .expect(Kind::Ampersand, "reference requires `&`")?
            .span
            .start;
        let mutable = self.take(Kind::Mut).is_some();
        let forwarded = self.take(Kind::Star).is_some();
        let name = self
            .expect(Kind::Ident, "reference requires a binding name")?
            .span;
        if self.peek().kind != Kind::Semi {
            return Err(self.error("local reference initializer must borrow a whole binding"));
        }
        self.push_expr(
            ExprKind::Borrow {
                mutable,
                name,
                forwarded,
            },
            self.source.span(start, name.end),
        )
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
            Kind::Unsupported
                if self.wide_scalars_enabled()
                    && self.source.text_at(self.peek().span).starts_with('\'') =>
            {
                PrimaryStart::Character
            }
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
            || (self.wide_scalars_enabled() && self.peek().kind == Kind::Invalid && self.source.text_at(self.peek().span) == "~")
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
        let left = self.bitwise(depth, context, 0)?;
        let Some(op) = self.comparison_op() else {
            return Ok(left);
        };
        let operator_span = self.bump().span;
        self.node()?;
        let right = self.bitwise(depth, context, 0)?;
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
    // Fixed precedence levels reuse the original immutable token tape. In
    // particular, a shift is two adjacent angle tokens; lexical certificates
    // and byte-for-byte token classifications remain unchanged.
    fn bitwise(
        &mut self,
        depth: usize,
        context: LiteralContext,
        level: u8,
    ) -> Result<ExprId, Box<Diagnostic>> {
        if !self.wide_scalars_enabled() || level == 4 {
            return self.sum(depth, context);
        }
        let mut left = self.bitwise(depth, context, level + 1)?;
        loop {
            let first = self.peek();
            let spelling = self.source.text_at(first.span);
            let op = match (level, first.kind, spelling) {
                (0, Kind::Unsupported, "|") => BitwiseOp::Or,
                (1, Kind::Invalid, "^") => BitwiseOp::Xor,
                (2, Kind::Ampersand, "&") => BitwiseOp::And,
                (3, Kind::Less, "<") | (3, Kind::Greater, ">") => {
                    let next = self.tokens[self.cursor + 1..]
                        .iter()
                        .find(|token| token.kind != Kind::Trivia);
                    if !next.is_some_and(|next| {
                        next.kind == first.kind && first.span.end == next.span.start
                    }) {
                        break;
                    }
                    if first.kind == Kind::Less {
                        BitwiseOp::ShiftLeft
                    } else {
                        BitwiseOp::ShiftRight
                    }
                }
                _ => break,
            };
            let mut operator_span = self.bump().span;
            if op.shift() {
                operator_span.end = self.bump().span.end;
            }
            self.node()?;
            let right = self.bitwise(depth, context, level + 1)?;
            let span = self.source.span(
                self.expressions[left.0].span.start,
                self.expressions[right.0].span.end,
            );
            left = self.push_expr(
                ExprKind::Bitwise {
                    op,
                    left,
                    right,
                    operator_span,
                },
                span,
            )?;
        }
        Ok(left)
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
            || (self.wide_scalars_enabled()
                && self.peek().kind == Kind::Invalid
                && self.source.text_at(self.peek().span) == "~")
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
                } else if self.source.text_at(operator_span) == "~" {
                    ExprKind::BitNot {
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
            | ExprKind::Not { operand: inner, .. }
            | ExprKind::BitNot { operand: inner, .. }
            | ExprKind::Conversion { operand: inner, .. } => self.heights[inner.0],
            ExprKind::Call { args, .. } => args
                .iter()
                .map(|arg| match arg {
                    Argument::Value(id) => self.heights[id.0],
                    Argument::Borrow {
                        place: BorrowPlace::Range { start, end, .. },
                        ..
                    } => self.heights[start.0].max(self.heights[end.0]),
                    Argument::Borrow { .. } => 1,
                })
                .max()
                .unwrap_or(0),
            ExprKind::QualifiedValue { args, .. } => args
                .iter()
                .flatten()
                .map(|arg| match arg {
                    Argument::Value(id) => self.heights[id.0],
                    Argument::Borrow {
                        place: BorrowPlace::Range { start, end, .. },
                        ..
                    } => self.heights[start.0].max(self.heights[end.0]),
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
            ExprKind::Bitwise { left, right, .. }
            | ExprKind::Arithmetic { left, right, .. }
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
    fn float_literal(
        &mut self,
        literal: super::source::Span,
        negative: bool,
        end: &mut usize,
    ) -> Result<Option<ExprKind>, Box<Diagnostic>> {
        if !self.wide_scalars_enabled() {
            return Ok(None);
        }
        let mut candidate = literal;
        let first = self.source.text_at(candidate);
        if first.contains(".to_") {
            return Ok(None);
        }
        // Pointer-sized integer suffixes end in `e`; select every existing
        // integer spelling before looking for a scientific exponent sign.
        if first.ends_with("i64")
            || first.ends_with("i32")
            || IntegerTypeSyntax::ADDITIONAL_LITERAL_SUFFIXES
                .into_iter()
                .any(|kind| first.ends_with(kind.spelling()))
        {
            return Ok(None);
        }
        if first.ends_with(['e', 'E'])
            && matches!(self.peek().kind, Kind::Plus | Kind::Minus)
            && self.peek().span.start == literal.end
        {
            let sign = self.bump();
            if self.peek().kind != Kind::Number || self.peek().span.start != sign.span.end {
                return Err(self.diagnostic(
                    "E0101",
                    "parse",
                    "expected decimal exponent digits",
                    Some(sign.span),
                ));
            }
            candidate = self.source.span(literal.start, self.bump().span.end);
            *end = candidate.end;
        }
        let text = self.source.text_at(candidate);
        let kind = if text.ends_with("f32") {
            Some(FloatTypeSyntax::F32)
        } else if text.ends_with("f64") {
            Some(FloatTypeSyntax::F64)
        } else {
            None
        };
        if kind.is_none() && !text.bytes().any(|b| matches!(b, b'.' | b'e' | b'E')) {
            return Ok(None);
        }
        let digits = self.source.span(
            candidate.start,
            candidate.end - if kind.is_some() { 3 } else { 0 },
        );
        let value = self.source.text_at(digits).as_bytes();
        let mut cursor = 0;
        while value.get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
        }
        let valid_integer = cursor != 0;
        if value.get(cursor) == Some(&b'.') {
            cursor += 1;
            while value.get(cursor).is_some_and(u8::is_ascii_digit) {
                cursor += 1;
            }
        }
        let mut valid_exponent = true;
        if value.get(cursor).is_some_and(|b| matches!(b, b'e' | b'E')) {
            cursor += 1;
            if value.get(cursor).is_some_and(|b| matches!(b, b'+' | b'-')) {
                cursor += 1;
            }
            let start = cursor;
            while value.get(cursor).is_some_and(u8::is_ascii_digit) {
                cursor += 1;
            }
            valid_exponent = cursor > start;
        }
        if !valid_integer || !valid_exponent || cursor != value.len() {
            return Err(self.diagnostic("E0101", "parse", "unsupported floating-point literal; expected decimal digits and an optional exponent", Some(candidate)));
        }
        Ok(Some(ExprKind::FloatNumber {
            digits,
            negative,
            kind: kind.unwrap_or(FloatTypeSyntax::F64),
        }))
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
            Some(PrimaryStart::Character) => {
                self.bump();
                ExprKind::Character(character_value(self.source.text_at(token.span)).ok_or_else(
                    || {
                        self.diagnostic(
                            "E0101",
                            "parse",
                            "character literal requires one Unicode scalar or a valid escape",
                            Some(token.span),
                        )
                    },
                )?)
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
                let literal = self.bump().span;
                end = literal.end;
                if let Some(kind) = self.float_literal(literal, negative, &mut end)? {
                    kind
                } else {
                    let wide = self.wide_scalars_enabled()
                        && self.source.text_at(literal).ends_with("i64");
                    let integer = if self.wide_scalars_enabled() {
                        IntegerTypeSyntax::ADDITIONAL_LITERAL_SUFFIXES
                            .into_iter()
                            .find(|kind| self.source.text_at(literal).ends_with(kind.spelling()))
                    } else {
                        None
                    };
                    let digits = if wide {
                        self.source.span(literal.start, literal.end - 3)
                    } else if let Some(kind) = integer {
                        self.source
                            .span(literal.start, literal.end - kind.spelling().len())
                    } else {
                        literal
                    };
                    if !self
                        .source
                        .text_at(digits)
                        .bytes()
                        .all(|byte| byte.is_ascii_digit())
                        || self.source.text_at(digits).is_empty()
                    {
                        // The unchanged lexer retains a numeric candidate as one
                        // token. Still blame the excluded postfix punctuation for
                        // these exact new spellings, never a fake numeric receiver.
                        if self.peek().kind == Kind::LParen {
                            if let Some((number, name)) =
                                self.source.text_at(digits).split_once('.')
                            {
                                if number.bytes().all(|byte| byte.is_ascii_digit())
                                    && matches!(name, "to_u8_checked" | "to_i32")
                                {
                                    let dot = digits.start + number.len();
                                    return Err(
                                        self.array_unsupported(self.source.span(dot, dot + 1))
                                    );
                                }
                            }
                        }
                        return Err(self.diagnostic(
                            "E0101",
                            "parse",
                            "unsupported numeric spelling; expected ASCII decimal literal digits",
                            Some(self.source.span(token.span.start, end)),
                        ));
                    }
                    if wide {
                        ExprKind::I64Number { digits, negative }
                    } else if let Some(kind) = integer {
                        ExprKind::IntegerNumber {
                            digits,
                            negative,
                            kind,
                        }
                    } else {
                        ExprKind::Number { digits, negative }
                    }
                }
            }
            Some(PrimaryStart::Name)
                if self.wide_scalars_enabled()
                    && FloatTypeSyntax::from_spelling(self.source.text_at(token.span))
                        .is_some()
                    && self.enum_qualified_ahead() =>
            {
                let selected =
                    FloatTypeSyntax::from_spelling(self.source.text_at(token.span)).unwrap();
                self.bump();
                self.bump();
                self.bump();
                let method = self
                    .expect(Kind::Ident, "expected floating-point constructor")?
                    .span;
                if self.source.text_at(method) != "from_bits" {
                    return Err(self.diagnostic(
                        "E0101",
                        "parse",
                        "supported floating-point constructor is from_bits",
                        Some(method),
                    ));
                }
                self.expect(Kind::LParen, "from_bits requires `(`")?;
                let name = self
                    .expect(Kind::Ident, "from_bits requires a named integer value")?
                    .span;
                self.node()?;
                let operand = self.push_expr(ExprKind::Name(name), name)?;
                end = self
                    .expect(
                        Kind::RParen,
                        "from_bits requires one named integer value and `)`",
                    )?
                    .span
                    .end;
                ExprKind::Conversion {
                    op: ConversionOp::FloatFromBits(selected),
                    operand,
                    name_span: method,
                }
            }
            Some(PrimaryStart::Name)
                if self.arrays_enabled()
                    && self.source.text_at(token.span) == "b"
                    && self.tokens.get(self.cursor + 1).is_some_and(|next| {
                        next.kind == Kind::String && token.span.end == next.span.start
                    }) =>
            {
                self.bump();
                let quoted = self.bump().span;
                end = quoted.end;
                let contents = self.source.span(quoted.start + 1, quoted.end - 1);
                let length = super::byte_strings::length(self.source.text_at(contents)).map_err(
                    |failure| {
                        let too_long = failure.reason == super::byte_strings::Reason::Length;
                        self.diagnostic(
                            if too_long { "E0400" } else { "E0101" },
                            "parse",
                            failure.reason.message(),
                            Some(if too_long {
                                self.source.span(token.span.start, end)
                            } else {
                                self.source.span(
                                    contents.start + failure.range.start,
                                    contents.start + failure.range.end,
                                )
                            }),
                        )
                    },
                )?;
                ExprKind::ByteString { contents, length }
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
                let application = if self.type_arguments_ahead() {
                    Some(self.generic_call_arguments(path, path_span)?)
                } else {
                    None
                };
                if let Some(application) = application.filter(|_| self.double_colon()) {
                    self.bump();
                    self.bump();
                    let variant = self
                        .expect(Kind::Ident, "expected enum variant after type arguments")?
                        .span;
                    end = variant.end;
                    let args = if self.take(Kind::LParen).is_some() {
                        let mut args = Vec::new();
                        if self.peek().kind != Kind::RParen {
                            loop {
                                let argument = Argument::Value(
                                    self.expression(depth + 1, LiteralContext::Allowed)?,
                                );
                                self.candidate_reserve(
                                    &mut args,
                                    MAX_PARAMS,
                                    "generic variant arguments",
                                    variant,
                                )?;
                                args.push(argument);
                                if self.take(Kind::Comma).is_none() {
                                    break;
                                }
                            }
                        }
                        end = self
                            .expect(Kind::RParen, "variant payload requires `)`")?
                            .span
                            .end;
                        Some(args)
                    } else {
                        None
                    };
                    ExprKind::GenericVariant {
                        application,
                        variant,
                        args,
                    }
                } else if self.take(Kind::LParen).is_some() {
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
                    if self.double_dot() {
                        return Err(self.array_index_range_error(index));
                    }
                    end = self.array_close("array index requires `]`")?;
                    ExprKind::IndexRead {
                        base: token.span,
                        index,
                    }
                } else if let Some(op) = self.named_conversion_ahead(path) {
                    // The ordinary named read is a real child, independently
                    // charged as syntax and retained before its conversion.
                    self.node()?;
                    let operand = self.push_expr(ExprKind::Name(token.span), token.span)?;
                    self.bump(); // `.` established by the closed lookahead.
                    let name_span = self.bump().span;
                    self.bump(); // `(` established by the closed lookahead.
                    end = self.conversion_close()?;
                    ExprKind::Conversion {
                        op,
                        operand,
                        name_span,
                    }
                } else if self.mode.owned()
                    && matches!(path, ItemPath::Unqualified(_))
                    && !self.double_dot()
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
                        if self.peek().kind != Kind::Dot || self.double_dot() {
                            break;
                        }
                        if self.named_conversion_ahead(path).is_some() {
                            return Err(self.array_unsupported(self.peek().span));
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
                        if self.double_dot() {
                            return Err(self.array_index_range_error(index));
                        }
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
                    if self
                        .named_conversion_ahead(ItemPath::Unqualified(token.span))
                        .is_some()
                    {
                        return Err(self.array_unsupported(self.peek().span));
                    }
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
        if (self.arrays_enabled()
            && (self.array_punctuation("[")
                || (self.peek().kind == Kind::Dot && !self.double_dot())))
            || (matches!(kind, ExprKind::Conversion { .. }) && self.peek().kind == Kind::LParen)
        {
            return Err(self.array_unsupported(self.peek().span));
        }
        self.push_expr(kind, self.source.span(token.span.start, end))
    }
}

#[path = "parser/arrays.rs"]
mod arrays;
#[path = "parser/characters.rs"]
mod characters;
#[path = "parser/conversions.rs"]
mod conversions;
pub(super) use characters::character_value;
#[path = "parser/enums.rs"]
mod enums;
#[cfg(test)]
#[path = "parser/u8_syntax_tests.rs"]
mod u8_syntax_tests;
pub(super) use enums::SyntaxStorage;

#[cfg(test)]
#[path = "parser/builtin_tests.rs"]
mod builtin_tests;

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
mod generic_slice_tests;
#[cfg(test)]
mod generic_syntax_tests;
#[cfg(test)]
#[path = "parser/project_tests.rs"]
mod project_tests;

#[cfg(test)]
#[path = "parser/activation_tests.rs"]
mod activation_tests;

#[test]
fn bounded_enum_production_parser_policy_layout() {
    println!(
        "ENUM_PRODUCTION_PARSER_LAYOUT policy={} parser={} storage={} parse_result={}",
        std::mem::size_of::<EnumSyntaxPolicy>(),
        std::mem::size_of::<Parser<'_>>(),
        std::mem::size_of::<enums::SyntaxStorage>(),
        std::mem::size_of::<Result<(Program, usize), Vec<Diagnostic>>>()
    );
    assert_eq!(std::mem::size_of::<EnumSyntaxPolicy>(), 1);
}

#[cfg(test)]
mod copy_record_tests;
