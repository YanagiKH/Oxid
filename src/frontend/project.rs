//! Immutable source ownership and bounded, declaration-only loading.
//!
//! Public typed dispatch and historical adapters share one source-discovery owner.
#![allow(dead_code)] // Historical qualification adapters retain their private API.

#[cfg(test)]
#[path = "project/builtin_tests.rs"]
mod builtin_tests;

#[cfg(test)]
#[path = "project/array_syntax_tests.rs"]
mod array_syntax_tests;
pub(super) mod budget;
#[cfg(test)]
#[path = "project/enum_carrier_tests.rs"]
mod enum_carrier_tests;
#[cfg(test)]
#[path = "project/enum_index_tests.rs"]
mod enum_index_tests;
mod filesystem;
#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "project/unit2_tests.rs"]
mod unit2_tests;

use super::{
    ast,
    diagnostic::Diagnostic,
    lexer, owned_diagnostic, parser,
    source::{SourceFile, SourceFileId, SourceMap, Span, MAX_SOURCE_BYTES},
};
use budget::{Allocator, ReserveFailure};
use std::mem::size_of;
use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ModuleId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FunctionAstKey {
    pub file: SourceFileId,
    pub index: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RecordAstKey {
    pub file: SourceFileId,
    pub index: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EnumAstKey {
    pub file: SourceFileId,
    pub index: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ExprKey {
    pub file: SourceFileId,
    pub expression: ast::ExprId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ItemPathRef {
    pub file: SourceFileId,
    pub path: ast::ItemPath,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct QualifiedPathRef {
    pub file: SourceFileId,
    pub path: ast::PathId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BlockKey {
    pub function: FunctionAstKey,
    pub block: ast::BodyBlockId,
}

/// Aggregate successful AST syntax, independent of parser recovery state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SyntaxFlavor {
    OriginalSingleFile,
    ProjectSyntax,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ProjectLimits {
    pub source_bytes: usize,
    pub tokens: usize,
    pub nodes: usize,
    pub modules: usize,
    pub depth: usize,
    pub component_bytes: usize,
    pub relative_bytes: usize,
    pub path_bytes: usize,
    pub probes: usize,
    pub directory_entries: usize,
    pub directory_name_units: usize,
}
impl Default for ProjectLimits {
    fn default() -> Self {
        Self {
            source_bytes: MAX_SOURCE_BYTES,
            tokens: lexer::MAX_TOKENS,
            nodes: parser::MAX_NODES,
            modules: 256,
            depth: 32,
            component_bytes: 255,
            relative_bytes: 4096,
            path_bytes: 4 * 1024 * 1024,
            probes: 8448,
            directory_entries: 100_000,
            directory_name_units: 16 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SourceUsage {
    pub source_bytes: usize,
    pub non_eof_tokens: usize,
    pub syntax_nodes: usize,
    pub line_starts: usize,
    pub modules: usize,
    pub retained_path_bytes: usize,
    pub probes: usize,
    pub directory_entries: usize,
    pub directory_name_units: usize,
}

#[derive(Debug)]
pub(super) struct ModuleHeader {
    pub file: SourceFileId,
    pub parent: Option<ModuleId>,
    pub declaration: Option<Span>,
    pub public: Option<Span>,
    pub depth: usize,
    pub relative_path: String,
    pub canonical_path: Option<PathBuf>,
}

#[derive(Debug)]
pub(super) struct ProjectSources {
    sources: SourceMap,
    programs: Vec<ast::Program>,
    modules: Vec<ModuleHeader>,
    canonical_root: Option<PathBuf>,
    usage: SourceUsage,
    syntax_flavor: SyntaxFlavor,
}

#[derive(Debug)]
pub(super) struct LoadFailure {
    pub sources: SourceMap,
    pub diagnostics: Vec<Diagnostic>,
    pub usage: SourceUsage,
    #[cfg(test)]
    pub allocator: Allocator,
}

/// Dependency-light transient loader seam. Standalone canonical observers do not
/// need the external process implementation or its cryptographic dependencies.
/// Implementations cannot select parser tokens without the comparison below.
pub(super) trait LexicalProvider {
    fn begin_module(
        &mut self,
        source: &SourceFile,
        usage: SourceUsage,
    ) -> Result<(), Box<Diagnostic>>;
    fn observe(
        &mut self,
        source: &SourceFile,
        limit: usize,
        inventory: &Inventory,
        allocator: &mut Allocator,
    ) -> Result<LexicalObservation, Box<Diagnostic>>;
    fn comparison_started(&mut self);
    fn comparison_finished(&mut self, matched: bool);
    fn selected_for_parser(
        &mut self,
        source: &SourceFile,
        tokens: &[lexer::Token],
        capacity: usize,
    ) -> Result<(), Box<Diagnostic>>;
}

/// Immutable source association travels with the actual returned allocation.
/// It is revalidated when consuming it at the parser boundary, independently
/// of any provider receipt or later parser-minted provenance.
pub(super) struct LexicalObservation {
    identity: u64,
    file: SourceFileId,
    source_len: usize,
    value: Result<Vec<lexer::Token>, Box<Diagnostic>>,
}
enum ComparisonFailure {
    Mismatch(&'static str),
    Storage(lexer::Failure),
}
type ComparisonResult = Result<Result<Vec<lexer::Token>, Box<Diagnostic>>, ComparisonFailure>;

pub(super) fn lexical_comparison_scratch_bytes() -> usize {
    3 * size_of::<ComparisonFailure>() + 3 * size_of::<ComparisonResult>()
}

impl LexicalObservation {
    pub(super) fn new(
        source: &SourceFile,
        value: Result<Vec<lexer::Token>, Box<Diagnostic>>,
    ) -> Self {
        Self {
            identity: source.identity(),
            file: source.span(0, 0).file,
            source_len: source.text().len(),
            value,
        }
    }
    fn compare_and_consume(
        self,
        source: &SourceFile,
        canonical: Result<Vec<lexer::Token>, lexer::Failure>,
        limit: usize,
    ) -> ComparisonResult {
        if self.identity != source.identity()
            || self.file != source.span(0, 0).file
            || self.source_len != source.text().len()
        {
            return Err(ComparisonFailure::Mismatch(
                "lexical observation belongs to another retained source",
            ));
        }
        // Canonical execution already occurred. Association retains precedence
        // over its resource outcome; only valid associations expose that error.
        let canonical = match canonical {
            Err(failure) if failure.is_storage() => {
                return Err(ComparisonFailure::Storage(failure))
            }
            outcome => outcome.map_err(lexer::Failure::diagnostic),
        };
        match (self.value, canonical) {
            (Ok(tokens), Ok(canonical)) => {
                // Existing post-allocation envelope check. Requested growth is
                // prepaid separately; this cannot prevent allocator over-return.
                let capacity = (source.text().len().min(limit) + 1)
                    .max(4)
                    .next_power_of_two();
                if canonical.capacity() > capacity {
                    return Err(ComparisonFailure::Mismatch(
                        "canonical token capacity exceeded admitted bound",
                    ));
                }
                if tokens.len() != canonical.len()
                    || !tokens
                        .iter()
                        .zip(&canonical)
                        .all(|(a, b)| a.kind == b.kind && a.span == b.span)
                {
                    return Err(ComparisonFailure::Mismatch(
                        "lexical provider disagrees with canonical tokens",
                    ));
                }
                drop(canonical);
                Ok(Ok(tokens))
            }
            (Err(observed), Err(canonical)) => {
                if observed.code != canonical.code
                    || observed.stage != canonical.stage
                    || observed.message != canonical.message
                    || observed.primary != canonical.primary
                    || observed.secondary != canonical.secondary
                    || observed.notes != canonical.notes
                {
                    return Err(ComparisonFailure::Mismatch(
                        "lexical provider disagrees with canonical diagnostic",
                    ));
                }
                drop(canonical);
                Ok(Err(observed))
            }
            _ => Err(ComparisonFailure::Mismatch(
                "lexical provider disagrees with canonical outcome",
            )),
        }
    }
}
fn lexical_failure(source: &SourceFile, message: &'static str) -> Box<Diagnostic> {
    owned_diagnostic::diagnostic(
        "E0703",
        "lexical-provider",
        format_args!("{message}"),
        Some(source.span(0, 0)),
    )
}

// Test-only reserve traces enlarge LoadFailure; do not allocate while handling failure.
#[cfg_attr(test, allow(clippy::result_large_err))]
impl ProjectSources {
    pub fn load_original(entry: &str, limits: ProjectLimits) -> Result<Self, LoadFailure> {
        Self::load(
            entry,
            limits,
            parser::SourceMode::OwnedCandidate,
            &mut Allocator::default(),
        )
    }
    pub fn load_modules(entry: &str, limits: ProjectLimits) -> Result<Self, LoadFailure> {
        Self::load(
            entry,
            limits,
            parser::SourceMode::ModuleCandidate,
            &mut Allocator::default(),
        )
    }
    /// Parse and load the bounded typed grammar once for public typed dispatch.
    pub fn load_typed(entry: &str, limits: ProjectLimits) -> Result<Self, LoadFailure> {
        Self::load_with_syntax(
            entry,
            limits,
            parser::SourceMode::ProjectCandidate,
            &mut Allocator::default(),
            parser::ArraySyntaxPolicy::Enabled,
            ProjectEnumSyntax::Enabled,
        )
    }
    pub(super) fn load_typed_with_provider(
        entry: &str,
        limits: ProjectLimits,
        provider: &mut dyn LexicalProvider,
    ) -> Result<Self, LoadFailure> {
        Self::load_with_syntax_provider(
            entry,
            limits,
            parser::SourceMode::ProjectCandidate,
            &mut Allocator::default(),
            parser::ArraySyntaxPolicy::Enabled,
            ProjectEnumSyntax::Enabled,
            Some(provider),
        )
    }
    /// Preserve the typed grammar before standard imports for gate controls.
    #[cfg(test)]
    pub(super) fn load_typed_closed_std(
        entry: &str,
        limits: ProjectLimits,
    ) -> Result<Self, LoadFailure> {
        Self::load_with_syntax(
            entry,
            limits,
            parser::SourceMode::ProjectCandidate,
            &mut Allocator::default(),
            parser::ArraySyntaxPolicy::Enabled,
            ProjectEnumSyntax::StdClosed,
        )
    }
    /// Historical qualification adapter for the same typed loader.
    pub fn load_project_candidate(entry: &str, limits: ProjectLimits) -> Result<Self, LoadFailure> {
        Self::load_typed(entry, limits)
    }
    fn load(
        entry: &str,
        limits: ProjectLimits,
        mode: parser::SourceMode,
        allocator: &mut Allocator,
    ) -> Result<Self, LoadFailure> {
        Self::load_with_arrays(
            entry,
            limits,
            mode,
            allocator,
            parser::ArraySyntaxPolicy::Closed,
        )
    }
    #[cfg(test)]
    pub(super) fn load_array_candidate(
        entry: &str,
        limits: ProjectLimits,
        allocator: &mut Allocator,
    ) -> Result<Self, LoadFailure> {
        Self::load_with_arrays(
            entry,
            limits,
            parser::SourceMode::ProjectCandidate,
            allocator,
            parser::ArraySyntaxPolicy::Candidate,
        )
    }
    #[cfg(test)]
    pub(super) fn load_enum_index_candidate(
        entry: &str,
        limits: ProjectLimits,
        allocator: &mut Allocator,
    ) -> Result<Self, LoadFailure> {
        Self::load_with_syntax(
            entry,
            limits,
            parser::SourceMode::ProjectCandidate,
            allocator,
            parser::ArraySyntaxPolicy::Enabled,
            ProjectEnumSyntax::Candidate,
        )
    }
    #[cfg(test)]
    pub(super) fn load_builtin_candidate(
        entry: &str,
        limits: ProjectLimits,
        allocator: &mut Allocator,
    ) -> Result<Self, LoadFailure> {
        Self::load_with_syntax(
            entry,
            limits,
            parser::SourceMode::ProjectCandidate,
            allocator,
            parser::ArraySyntaxPolicy::Enabled,
            ProjectEnumSyntax::BuiltinCandidate,
        )
    }
    #[cfg(test)]
    pub(super) fn load_output_candidate(
        entry: &str,
        limits: ProjectLimits,
        allocator: &mut Allocator,
    ) -> Result<Self, LoadFailure> {
        Self::load_with_syntax(
            entry,
            limits,
            parser::SourceMode::ProjectCandidate,
            allocator,
            parser::ArraySyntaxPolicy::Enabled,
            ProjectEnumSyntax::OutputCandidate,
        )
    }
    fn load_with_arrays(
        entry: &str,
        limits: ProjectLimits,
        mode: parser::SourceMode,
        allocator: &mut Allocator,
        arrays: parser::ArraySyntaxPolicy,
    ) -> Result<Self, LoadFailure> {
        Self::load_with_syntax(
            entry,
            limits,
            mode,
            allocator,
            arrays,
            ProjectEnumSyntax::Closed,
        )
    }
    fn load_with_syntax(
        entry: &str,
        limits: ProjectLimits,
        mode: parser::SourceMode,
        allocator: &mut Allocator,
        arrays: parser::ArraySyntaxPolicy,
        enums: ProjectEnumSyntax,
    ) -> Result<Self, LoadFailure> {
        Self::load_with_syntax_provider(entry, limits, mode, allocator, arrays, enums, None)
    }
    #[allow(clippy::too_many_arguments)]
    fn load_with_syntax_provider(
        entry: &str,
        limits: ProjectLimits,
        mode: parser::SourceMode,
        allocator: &mut Allocator,
        arrays: parser::ArraySyntaxPolicy,
        enums: ProjectEnumSyntax,
        mut provider: Option<&mut dyn LexicalProvider>,
    ) -> Result<Self, LoadFailure> {
        let mut builder = SourceSetBuilder {
            entry,
            limits,
            allocator,
            mode,
            arrays,
            enums,
            project: Self {
                sources: SourceMap::new(),
                programs: Vec::new(),
                modules: Vec::new(),
                canonical_root: None,
                usage: SourceUsage::default(),
                syntax_flavor: SyntaxFlavor::OriginalSingleFile,
            },
        };
        match builder.load_all_with_provider(&mut provider) {
            Ok(()) => Ok(builder.project),
            Err(diagnostics) => Err(LoadFailure {
                sources: builder.project.sources,
                diagnostics,
                usage: builder.project.usage,
                #[cfg(test)]
                allocator: std::mem::take(builder.allocator),
            }),
        }
    }
    pub fn sources(&self) -> &SourceMap {
        &self.sources
    }
    pub fn usage(&self) -> SourceUsage {
        self.usage
    }
    pub fn syntax_flavor(&self) -> SyntaxFlavor {
        self.syntax_flavor
    }
    pub fn uses_owned_syntax(&self) -> bool {
        self.programs
            .iter()
            .enumerate()
            .any(|(file, program)| program.uses_owned_syntax(self.sources.get(SourceFileId(file))))
    }
    pub fn modules(&self) -> &[ModuleHeader] {
        &self.modules
    }
    pub fn try_text(&self, span: Span) -> Option<&str> {
        self.sources.try_text(span)
    }
    pub fn text(&self, span: Span) -> &str {
        self.sources.text(span)
    }
    pub fn try_file_ast(&self, file: SourceFileId) -> Option<&ast::Program> {
        self.modules
            .get(file.0)
            .filter(|header| header.file == file)?;
        self.programs.get(file.0)
    }
    pub fn try_function(&self, key: FunctionAstKey) -> Option<&ast::Function> {
        self.try_file_ast(key.file)?
            .functions
            .get(key.index)
            .filter(|function| function.name.file == key.file)
    }
    pub fn try_record(&self, key: RecordAstKey) -> Option<&ast::StructDecl> {
        self.try_file_ast(key.file)?
            .records
            .get(key.index)
            .filter(|record| record.name.file == key.file)
    }
    pub fn try_enum(&self, key: EnumAstKey) -> Option<&ast::EnumDecl> {
        self.try_file_ast(key.file)?
            .enums
            .get(key.index)
            .filter(|declaration| declaration.name.file == key.file)
    }
    pub fn try_expression(&self, key: ExprKey) -> Option<&ast::Expr> {
        self.try_file_ast(key.file)?
            .expressions
            .get(key.expression.0)
            .filter(|expression| expression.span.file == key.file)
    }
    pub fn try_block(&self, key: BlockKey) -> Option<&ast::BodyBlock> {
        self.try_function(key.function)?
            .blocks
            .get(key.block.0)
            .filter(|block| block.span.file == key.function.file)
    }
    pub fn function_handles(&self) -> impl Iterator<Item = FunctionAstKey> + '_ {
        self.programs
            .iter()
            .enumerate()
            .flat_map(|(file, program)| {
                (0..program.functions.len()).map(move |index| FunctionAstKey {
                    file: SourceFileId(file),
                    index,
                })
            })
    }
    pub fn record_handles(&self) -> impl Iterator<Item = RecordAstKey> + '_ {
        self.programs
            .iter()
            .enumerate()
            .flat_map(|(file, program)| {
                (0..program.records.len()).map(move |index| RecordAstKey {
                    file: SourceFileId(file),
                    index,
                })
            })
    }
    pub fn enum_handles(&self) -> impl Iterator<Item = EnumAstKey> + '_ {
        self.programs
            .iter()
            .enumerate()
            .flat_map(|(file, program)| {
                (0..program.enums.len()).map(move |index| EnumAstKey {
                    file: SourceFileId(file),
                    index,
                })
            })
    }
    /// Only successful original syntax may feed the old one-file consumers.
    pub fn original_file(&self) -> Option<(&super::source::SourceFile, &ast::Program)> {
        (self.syntax_flavor == SyntaxFlavor::OriginalSingleFile && self.programs.len() == 1)
            .then(|| (self.sources.get(SourceFileId(0)), &self.programs[0]))
    }
    /// Exact requested retained lengths. Vec capacity/allocator/OS storage excluded.
    pub fn inventory(&self) -> Option<Inventory> {
        let mut result = Inventory {
            owner_bytes: size_of::<Self>(),
            source_bytes: self.usage.source_bytes,
            source_headers: self
                .sources
                .files()
                .len()
                .checked_mul(size_of::<super::source::SourceFile>())?,
            line_starts: self.usage.line_starts.checked_mul(size_of::<usize>())?,
            ast_headers: self.programs.len().checked_mul(size_of::<ast::Program>())?,
            module_headers: self.modules.len().checked_mul(size_of::<ModuleHeader>())?,
            path_bytes: self.usage.retained_path_bytes,
            fixed_loader_scratch: size_of::<[u8; 8192]>().checked_add(size_of::<[Frame; 33]>())?,
            ..Inventory::default()
        };
        for program in &self.programs {
            result.tokens = result.tokens.checked_add(
                program
                    .tokens
                    .len()
                    .checked_mul(size_of::<lexer::Token>())?,
            )?;
            result.ast_payload = result
                .ast_payload
                .checked_add(
                    program
                        .functions
                        .len()
                        .checked_mul(size_of::<ast::Function>())?,
                )?
                .checked_add(
                    program
                        .records
                        .len()
                        .checked_mul(size_of::<ast::StructDecl>())?,
                )?
                .checked_add(
                    program
                        .enums
                        .len()
                        .checked_mul(size_of::<ast::EnumDecl>())?,
                )?
                .checked_add(
                    program
                        .expressions
                        .len()
                        .checked_mul(size_of::<ast::Expr>())?,
                )?
                .checked_add(program.items.len().checked_mul(size_of::<ast::ItemId>())?)?
                .checked_add(
                    program
                        .modules
                        .len()
                        .checked_mul(size_of::<ast::ModuleDecl>())?,
                )?
                .checked_add(
                    program
                        .imports
                        .len()
                        .checked_mul(size_of::<ast::ImportDecl>())?,
                )?
                .checked_add(
                    program
                        .paths
                        .len()
                        .checked_mul(size_of::<ast::AbsolutePath>())?,
                )?
                .checked_add(program.path_segments.len().checked_mul(size_of::<Span>())?)?;
            for function in &program.functions {
                result.ast_payload = result
                    .ast_payload
                    .checked_add(function.params.len().checked_mul(size_of::<ast::Param>())?)?
                    .checked_add(
                        function
                            .blocks
                            .len()
                            .checked_mul(size_of::<ast::BodyBlock>())?,
                    )?;
                for block in &function.blocks {
                    result.ast_payload = result
                        .ast_payload
                        .checked_add(block.body.len().checked_mul(size_of::<ast::Stmt>())?)?;
                    for statement in &block.body {
                        if let ast::StmtKind::Match { arms, .. } = &statement.kind {
                            result.ast_payload = result.ast_payload.checked_add(
                                arms.len().checked_mul(size_of::<ast::MatchArmSyntax>())?,
                            )?;
                        }
                    }
                }
            }
            for enumeration in &program.enums {
                result.ast_payload = result.ast_payload.checked_add(
                    enumeration
                        .variants
                        .len()
                        .checked_mul(size_of::<ast::EnumVariantSyntax>())?,
                )?;
            }
            for record in &program.records {
                result.ast_payload = result.ast_payload.checked_add(
                    record
                        .fields
                        .len()
                        .checked_mul(size_of::<ast::StructField>())?,
                )?;
            }
            for expression in &program.expressions {
                result.ast_payload = result.ast_payload.checked_add(match &expression.kind {
                    ast::ExprKind::Call { args, .. } => {
                        args.len().checked_mul(size_of::<ast::Argument>())?
                    }
                    ast::ExprKind::QualifiedValue { args, .. } => {
                        args.as_ref().map_or(Some(0), |args| {
                            args.len().checked_mul(size_of::<ast::Argument>())
                        })?
                    }
                    ast::ExprKind::StructLiteral { fields, .. } => {
                        fields.len().checked_mul(size_of::<ast::FieldInit>())?
                    }
                    ast::ExprKind::ArrayLiteral { elements } => {
                        elements.len().checked_mul(size_of::<ast::ExprId>())?
                    }
                    _ => 0,
                })?;
            }
        }
        Some(result)
    }
    fn root_eof(&self) -> Span {
        let file = self.sources.get(SourceFileId(0));
        file.span(file.text().len(), file.text().len())
    }
}

#[derive(Debug, Default)]
pub(super) struct Inventory {
    pub owner_bytes: usize,
    pub source_bytes: usize,
    pub source_headers: usize,
    pub line_starts: usize,
    pub tokens: usize,
    pub ast_headers: usize,
    pub ast_payload: usize,
    pub module_headers: usize,
    pub path_bytes: usize,
    /// Fixed read buffer plus fixed DFS stack; excludes existing parser scratch.
    pub fixed_loader_scratch: usize,
}

type Errors = Vec<Diagnostic>;
// Best-effort legacy adaptation: this allocates a singleton diagnostic Vec.
// Inline lexer failure signaling does not make this path process-wide OOM safe.
#[allow(clippy::boxed_local)]
fn one(error: Box<Diagnostic>) -> Errors {
    vec![*error]
}
fn resource(message: &'static str, origin: Option<Span>) -> Box<Diagnostic> {
    owned_diagnostic::diagnostic("E0400", "source-project", format_args!("{message}"), origin)
}
fn policy(message: &'static str, origin: Span) -> Box<Diagnostic> {
    owned_diagnostic::diagnostic("E0005", "source", format_args!("{message}"), Some(origin))
}
fn allocation(origin: Option<Span>) -> Box<Diagnostic> {
    resource("project source allocation failed", origin)
}
fn reserve_error(error: ReserveFailure, origin: Option<Span>) -> Box<Diagnostic> {
    match error {
        ReserveFailure::Overflow => overflow(origin),
        ReserveFailure::Allocation => allocation(origin),
    }
}
fn overflow(origin: Option<Span>) -> Box<Diagnostic> {
    resource("project source count overflow", origin)
}
fn add(left: usize, right: usize, origin: Option<Span>) -> Result<usize, Box<Diagnostic>> {
    left.checked_add(right).ok_or_else(|| overflow(origin))
}
fn io_error(error: io::Error, display: &str, origin: Option<Span>) -> Box<Diagnostic> {
    if origin.is_none() {
        Diagnostic::new(
            "E0002",
            "source",
            format!("cannot read file {display}: {error}"),
            None,
        )
    } else {
        owned_diagnostic::diagnostic(
            "E0002",
            "source",
            format_args!("cannot read module: {error}"),
            origin,
        )
    }
}

#[derive(Clone, Copy)]
enum ProjectEnumSyntax {
    #[cfg(test)]
    OutputCandidate,
    #[cfg(test)]
    BuiltinCandidate,
    #[cfg(test)]
    StdClosed,
    Closed,
    #[allow(dead_code)]
    Enabled,
    #[cfg(test)]
    Candidate,
}

struct SourceSetBuilder<'a> {
    entry: &'a str,
    limits: ProjectLimits,
    allocator: &'a mut Allocator,
    mode: parser::SourceMode,
    arrays: parser::ArraySyntaxPolicy,
    enums: ProjectEnumSyntax,
    project: ProjectSources,
}
#[derive(Clone, Copy)]
struct Frame {
    module: ModuleId,
    next: usize,
}

impl SourceSetBuilder<'_> {
    fn load_all(&mut self) -> Result<(), Errors> {
        self.load_all_with_provider(&mut None)
    }
    fn load_all_with_provider(
        &mut self,
        provider: &mut Option<&mut dyn LexicalProvider>,
    ) -> Result<(), Errors> {
        let text = self
            .read_source(Path::new(self.entry), self.entry, None)
            .map_err(one)?;
        let mut display = String::new();
        self.allocator
            .string(&mut display, self.entry.len(), "entry display")
            .map_err(|error| one(reserve_error(error, None)))?;
        display.push_str(self.entry);
        let program = self.parse_file_with_provider(display, text, None, provider)?;
        let root = self.project.root_eof();
        if self.limits.modules == 0 {
            return Err(one(resource("module count limit exceeded", Some(root))));
        }
        self.reserve_file(Some(root)).map_err(one)?;
        self.project.programs.push(program);
        self.project.modules.push(ModuleHeader {
            file: SourceFileId(0),
            parent: None,
            declaration: None,
            public: None,
            depth: 0,
            relative_path: String::new(),
            canonical_path: None,
        });
        self.project.usage.modules = 1;
        // No additional path policy, including host capability checks, for one file.
        if self.project.programs[0].modules.is_empty() {
            return Ok(());
        }
        self.project.syntax_flavor = SyntaxFlavor::ProjectSyntax;
        let origin = self.project.programs[0].modules[0].name;
        // Root canonicalization is deferred until the first declared child has
        // passed the earlier M/D/component/relative-path dimensional checks.
        let first_name_bytes = self.project.text(origin).len();
        let first = ChildDimensions::new(
            self.project.usage.modules,
            0,
            0,
            first_name_bytes,
            Some(origin),
        )
        .map_err(one)?;
        first
            .admit(self.limits, first_name_bytes, Some(origin))
            .map_err(one)?;
        filesystem::qualified_host(origin).map_err(one)?;
        let parent = Path::new(self.entry)
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        // OS-returned canonicalization storage is outside controlled reserve accounting.
        let root_path = parent
            .canonicalize()
            .map_err(|e| one(io_error(e, self.entry, Some(origin))))?;
        let entry_path = Path::new(self.entry)
            .canonicalize()
            .map_err(|e| one(io_error(e, self.entry, Some(origin))))?;
        let retained = add(
            self.project.usage.retained_path_bytes,
            filesystem::path_units(&root_path),
            Some(origin),
        )
        .and_then(|n| add(n, filesystem::path_units(&entry_path), Some(origin)))
        .map_err(one)?;
        if retained > self.limits.path_bytes {
            return Err(one(resource(
                "retained module path budget exceeded",
                Some(origin),
            )));
        }
        self.project.usage.retained_path_bytes = retained;
        self.project.canonical_root = Some(root_path);
        self.project.modules[0].canonical_path = Some(entry_path);
        // Fixed depth-bounded traversal storage: no recursive AST/vector remapping.
        let mut stack = [Frame {
            module: ModuleId(0),
            next: 0,
        }; 33];
        let mut height = 1;
        while height != 0 {
            let frame = stack[height - 1];
            if frame.next == self.project.programs[frame.module.0].modules.len() {
                height -= 1;
                continue;
            }
            stack[height - 1].next += 1;
            let child = self.load_child_with_provider(frame.module, frame.next, provider)?;
            if height == stack.len() {
                return Err(one(Diagnostic::new(
                    "E0500",
                    "source-project",
                    "module traversal depth invariant failed",
                    Some(self.project.root_eof()),
                )));
            }
            stack[height] = Frame {
                module: child,
                next: 0,
            };
            height += 1;
        }
        Ok(())
    }

    fn reserve_file(&mut self, origin: Option<Span>) -> Result<(), Box<Diagnostic>> {
        // Count and byte products for both tables are admitted before either grows.
        let count = add(self.project.programs.len(), 1, origin)?;
        count
            .checked_mul(size_of::<ast::Program>())
            .and_then(|_| count.checked_mul(size_of::<ModuleHeader>()))
            .ok_or_else(|| overflow(origin))?;
        self.allocator
            .vector(&mut self.project.programs, 1, "file ASTs")
            .map_err(|error| reserve_error(error, origin))?;
        self.allocator
            .vector(&mut self.project.modules, 1, "module headers")
            .map_err(|error| reserve_error(error, origin))
    }

    fn read_source(
        &mut self,
        path: &Path,
        display: &str,
        origin: Option<Span>,
    ) -> Result<String, Box<Diagnostic>> {
        let mut file = File::open(path).map_err(|e| io_error(e, display, origin))?;
        let mut bytes = Vec::new();
        let mut scratch = [0u8; 8192];
        let remaining = self
            .limits
            .source_bytes
            .min(MAX_SOURCE_BYTES)
            .checked_sub(self.project.usage.source_bytes)
            .ok_or_else(|| overflow(origin))?;
        loop {
            let available = remaining
                .checked_sub(bytes.len())
                .ok_or_else(|| overflow(origin))?;
            let requested = available
                .checked_add(1)
                .ok_or_else(|| overflow(origin))?
                .min(scratch.len());
            let count = match file.read(&mut scratch[..requested]) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                result => result.map_err(|e| io_error(e, display, origin))?,
            };
            if count == 0 {
                break;
            }
            if count > available {
                return Err(if origin.is_none() {
                    Diagnostic::new(
                        "E0400",
                        "source",
                        format!("source exceeds {MAX_SOURCE_BYTES} bytes: {display}"),
                        None,
                    )
                } else {
                    owned_diagnostic::diagnostic(
                        "E0400",
                        "source",
                        format_args!("aggregate source byte limit exceeded"),
                        origin,
                    )
                });
            }
            self.allocator
                .vector(&mut bytes, count, "source bytes")
                .map_err(|error| reserve_error(error, origin))?;
            bytes.extend_from_slice(&scratch[..count]);
        }
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
        String::from_utf8(bytes).map_err(|_| {
            if origin.is_none() {
                Diagnostic::new(
                    "E0003",
                    "source",
                    format!("source is not valid UTF-8: {display}"),
                    None,
                )
            } else {
                owned_diagnostic::diagnostic(
                    "E0003",
                    "source",
                    format_args!("module source is not valid UTF-8"),
                    origin,
                )
            }
        })
    }

    fn parse_file(
        &mut self,
        display: String,
        text: String,
        origin: Option<Span>,
    ) -> Result<ast::Program, Errors> {
        self.parse_file_with_provider(display, text, origin, &mut None)
    }
    fn parse_file_with_provider(
        &mut self,
        display: String,
        text: String,
        origin: Option<Span>,
        provider: &mut Option<&mut dyn LexicalProvider>,
    ) -> Result<ast::Program, Errors> {
        let path_bytes = add(
            self.project.usage.retained_path_bytes,
            display.len(),
            origin,
        )
        .map_err(one)?;
        let file = self
            .project
            .sources
            .try_add(display, text, self.allocator)
            .map_err(|error| one(reserve_error(error, origin)))?;
        self.project.usage.retained_path_bytes = path_bytes;
        let source = self.project.sources.get(file);
        self.project.usage.line_starts =
            add(self.project.usage.line_starts, source.line_count(), origin).map_err(one)?;
        let remaining_tokens = self
            .limits
            .tokens
            .min(lexer::MAX_TOKENS)
            .checked_sub(self.project.usage.non_eof_tokens)
            .ok_or_else(|| one(overflow(origin)))?;
        let tokens = if let Some(provider) = provider.as_deref_mut() {
            provider
                .begin_module(source, self.project.usage)
                .map_err(one)?;
            let inventory = self
                .project
                .inventory()
                .ok_or_else(|| one(overflow(origin)))?;
            let observed = provider
                .observe(source, remaining_tokens, &inventory, self.allocator)
                .map_err(one)?;
            provider.comparison_started();
            let canonical = lexer::lex_with_allocator(source, remaining_tokens, self.allocator);
            let result = observed.compare_and_consume(source, canonical, remaining_tokens);
            provider.comparison_finished(result.is_ok());
            result
                .map_err(|failure| {
                    one(match failure {
                        ComparisonFailure::Mismatch(message) => lexical_failure(source, message),
                        ComparisonFailure::Storage(failure) => failure.diagnostic(),
                    })
                })?
                .map_err(one)?
        } else {
            lexer::lex_with_allocator(source, remaining_tokens, self.allocator)
                .map_err(|failure| one(failure.diagnostic()))?
        };
        self.project.usage.non_eof_tokens =
            add(self.project.usage.non_eof_tokens, tokens.len() - 1, origin).map_err(one)?;
        let remaining_nodes = self
            .limits
            .nodes
            .min(parser::MAX_NODES)
            .checked_sub(self.project.usage.syntax_nodes)
            .ok_or_else(|| one(overflow(origin)))?;
        if let Some(provider) = provider.as_deref_mut() {
            provider
                .selected_for_parser(source, &tokens, tokens.capacity())
                .map_err(one)?;
        }
        let (program, nodes) = match self.enums {
            #[cfg(test)]
            ProjectEnumSyntax::StdClosed => parser::parse_typed_closed_std_counted(
                source,
                tokens,
                self.mode,
                remaining_nodes,
                self.allocator,
                &mut Default::default(),
            )?,
            #[cfg(test)]
            ProjectEnumSyntax::OutputCandidate => parser::parse_output_candidate_counted(
                source,
                tokens,
                self.mode,
                remaining_nodes,
                self.allocator,
                &mut Default::default(),
            )?,
            #[cfg(test)]
            ProjectEnumSyntax::BuiltinCandidate => parser::parse_builtin_candidate_counted(
                source,
                tokens,
                self.mode,
                remaining_nodes,
                self.allocator,
                &mut Default::default(),
            )?,
            ProjectEnumSyntax::Closed => {
                if self.arrays == parser::ArraySyntaxPolicy::Closed {
                    parser::parse_counted(
                        source,
                        tokens,
                        self.mode,
                        remaining_nodes,
                        self.allocator,
                    )?
                } else {
                    parser::parse_counted_with_arrays(
                        source,
                        tokens,
                        self.mode,
                        remaining_nodes,
                        self.allocator,
                        self.arrays,
                    )?
                }
            }
            ProjectEnumSyntax::Enabled => parser::parse_typed_counted(
                source,
                tokens,
                self.mode,
                remaining_nodes,
                self.allocator,
                &mut Default::default(),
            )?,
            #[cfg(test)]
            ProjectEnumSyntax::Candidate => parser::parse_enum_candidate_counted(
                source,
                tokens,
                self.mode,
                remaining_nodes,
                self.allocator,
                &mut Default::default(),
            )?,
        };
        self.project.usage.syntax_nodes =
            add(self.project.usage.syntax_nodes, nodes, origin).map_err(one)?;
        if program.uses_project_syntax() {
            self.project.syntax_flavor = SyntaxFlavor::ProjectSyntax;
        }
        Ok(program)
    }

    fn load_child(&mut self, parent: ModuleId, local: usize) -> Result<ModuleId, Errors> {
        self.load_child_with_provider(parent, local, &mut None)
    }
    fn load_child_with_provider(
        &mut self,
        parent: ModuleId,
        local: usize,
        provider: &mut Option<&mut dyn LexicalProvider>,
    ) -> Result<ModuleId, Errors> {
        let declaration = self.project.programs[parent.0].modules[local];
        let at = Some(declaration.name);
        let name = self.project.text(declaration.name);
        let parent_header = &self.project.modules[parent.0];
        let prefix = parent_header
            .relative_path
            .strip_suffix(".ox")
            .unwrap_or("");
        let root_path = self
            .project
            .canonical_root
            .as_ref()
            .expect("module root established");
        let display_parent = Path::new(self.entry).parent().unwrap_or(Path::new(""));
        let display_parent = display_parent.to_str().expect("entry spelling is UTF-8");
        // Checked arithmetic for every dimensional plan precedes every cap/semantic test.
        let plan = ChildPlan::new(
            self.project.usage,
            self.limits,
            parent_header.depth,
            prefix.len(),
            name.len(),
            add(
                display_parent.len(),
                usize::from(!display_parent.is_empty() && !display_parent.ends_with('/')),
                at,
            )
            .map_err(one)?,
            add(
                filesystem::path_units(root_path),
                usize::from(!root_path.as_os_str().as_encoded_bytes().ends_with(b"/")),
                at,
            )
            .map_err(one)?,
            at,
        )
        .map_err(one)?;
        for earlier in &self.project.programs[parent.0].modules[..local] {
            let old = self.project.text(earlier.name);
            if old == name {
                return Err(one(owned_diagnostic::secondary(
                    owned_diagnostic::diagnostic(
                        "E0201",
                        "resolve",
                        format_args!("duplicate module declaration"),
                        at,
                    ),
                    earlier.name,
                    format_args!("first declared here"),
                )));
            }
            if old.eq_ignore_ascii_case(name) {
                return Err(one(owned_diagnostic::secondary(
                    policy(
                        "module names collide under ASCII case folding",
                        declaration.name,
                    ),
                    earlier.name,
                    format_args!("first declared here"),
                )));
            }
        }
        let mut relative = String::new();
        self.allocator
            .string(&mut relative, plan.relative_bytes, "logical module path")
            .map_err(|error| one(reserve_error(error, at)))?;
        relative.push_str(prefix);
        if !prefix.is_empty() {
            relative.push('/');
        }
        relative.push_str(name);
        relative.push_str(".ox");
        let mut display = String::new();
        self.allocator
            .string(&mut display, plan.display_bytes, "module display")
            .map_err(|error| one(reserve_error(error, at)))?;
        display.push_str(display_parent);
        if !display_parent.is_empty() && !display_parent.ends_with('/') {
            display.push('/');
        }
        display.push_str(&relative);
        // ChildPlan uses exact joining lengths, including an existing root slash.
        if relative.len() != plan.relative_bytes || display.len() != plan.display_bytes {
            return Err(one(Diagnostic::new(
                "E0500",
                "source-project",
                "module path count invariant failed",
                at,
            )));
        }
        let canonical = filesystem::verify(
            root_path,
            &relative,
            self.limits,
            &mut self.project.usage,
            self.allocator,
            declaration.name,
        )
        .map_err(one)?;
        if filesystem::path_units(&canonical) != plan.canonical_bytes {
            return Err(one(policy(
                "module canonical pathname changed during loading",
                declaration.name,
            )));
        }
        for header in &self.project.modules {
            if header.canonical_path.as_ref() == Some(&canonical) {
                let earlier = header
                    .declaration
                    .unwrap_or_else(|| self.project.root_eof());
                return Err(one(owned_diagnostic::secondary(
                    policy(
                        "module canonical pathname was already loaded",
                        declaration.name,
                    ),
                    earlier,
                    format_args!("first loaded here"),
                )));
            }
        }
        self.reserve_file(at).map_err(one)?;
        let text = self.read_source(&canonical, &display, at).map_err(one)?;
        // display is charged when admitted into SourceMap; these two owned paths here.
        self.project.usage.retained_path_bytes =
            add(self.project.usage.retained_path_bytes, relative.len(), at)
                .and_then(|n| add(n, filesystem::path_units(&canonical), at))
                .map_err(one)?;
        let program = self.parse_file_with_provider(display, text, at, provider)?;
        let id = ModuleId(self.project.programs.len());
        self.project.programs.push(program);
        self.project.modules.push(ModuleHeader {
            file: SourceFileId(id.0),
            parent: Some(parent),
            declaration: at,
            public: declaration.public,
            depth: plan.depth,
            relative_path: relative,
            canonical_path: Some(canonical),
        });
        self.project.usage.modules = plan.modules;
        Ok(id)
    }
}

/// Pure count-only preflight, also used for unreachable overflow/reduced-cap seams.
#[derive(Debug)]
struct ChildPlan {
    modules: usize,
    depth: usize,
    relative_bytes: usize,
    display_bytes: usize,
    canonical_bytes: usize,
}
impl ChildPlan {
    #[allow(clippy::too_many_arguments)]
    fn new(
        usage: SourceUsage,
        limits: ProjectLimits,
        parent_depth: usize,
        prefix_bytes: usize,
        component_bytes: usize,
        display_prefix_bytes: usize,
        canonical_prefix_bytes: usize,
        origin: Option<Span>,
    ) -> Result<Self, Box<Diagnostic>> {
        let dimensions = ChildDimensions::new(
            usage.modules,
            parent_depth,
            prefix_bytes,
            component_bytes,
            origin,
        )?;
        let ChildDimensions {
            modules,
            depth,
            relative_bytes,
        } = dimensions;
        let display_bytes = add(display_prefix_bytes, relative_bytes, origin)?;
        let canonical_bytes = add(canonical_prefix_bytes, relative_bytes, origin)?;
        let path_bytes = add(usage.retained_path_bytes, relative_bytes, origin)
            .and_then(|n| add(n, display_bytes, origin))
            .and_then(|n| add(n, canonical_bytes, origin))?;
        let probes = add(usage.probes, depth, origin)?;
        dimensions.admit(limits, component_bytes, origin)?;
        if path_bytes > limits.path_bytes.min(4 * 1024 * 1024) {
            return Err(resource("retained module path budget exceeded", origin));
        }
        if probes > limits.probes.min(8448) {
            return Err(resource("module directory probe budget exceeded", origin));
        }
        Ok(Self {
            modules,
            depth,
            relative_bytes,
            display_bytes,
            canonical_bytes,
        })
    }
}

#[derive(Clone, Copy)]
struct ChildDimensions {
    modules: usize,
    depth: usize,
    relative_bytes: usize,
}
impl ChildDimensions {
    fn new(
        modules: usize,
        parent_depth: usize,
        prefix_bytes: usize,
        component_bytes: usize,
        origin: Option<Span>,
    ) -> Result<Self, Box<Diagnostic>> {
        let modules = add(modules, 1, origin)?;
        let depth = add(parent_depth, 1, origin)?;
        let relative_bytes = add(prefix_bytes, usize::from(prefix_bytes != 0), origin)
            .and_then(|n| add(n, component_bytes, origin))
            .and_then(|n| add(n, 3, origin))?;
        Ok(Self {
            modules,
            depth,
            relative_bytes,
        })
    }
    fn admit(
        self,
        limits: ProjectLimits,
        component_bytes: usize,
        origin: Option<Span>,
    ) -> Result<(), Box<Diagnostic>> {
        if self.modules > limits.modules.min(256) {
            return Err(resource("module count limit exceeded", origin));
        }
        if self.depth > limits.depth.min(32) {
            return Err(resource("module depth limit exceeded", origin));
        }
        if component_bytes > limits.component_bytes.min(255) {
            return Err(resource("module component limit exceeded", origin));
        }
        if self.relative_bytes > limits.relative_bytes.min(4096) {
            return Err(resource("module relative path limit exceeded", origin));
        }
        Ok(())
    }
}

#[test]
fn bounded_enum_production_project_policy_layout() {
    println!(
        "ENUM_PRODUCTION_PROJECT_LAYOUT policy={} builder={} sources={}",
        std::mem::size_of::<ProjectEnumSyntax>(),
        std::mem::size_of::<SourceSetBuilder<'_>>(),
        std::mem::size_of::<ProjectSources>()
    );
    assert_eq!(std::mem::size_of::<ProjectEnumSyntax>(), 1);
}

#[cfg(test)]
impl ProjectSources {
    /// In-memory root/sibling fixtures for index tests, not a filesystem loader.
    /// Real parsed declarations determine source preorder and checked origins.
    /// Production host qualification and filesystem discovery remain untouched.
    pub(super) fn from_u8_index_test_files(files: &[(&str, &str)]) -> Self {
        assert_eq!(files[0].0, "main.ox");
        let mut project = Self {
            sources: SourceMap::new(),
            programs: Vec::new(),
            modules: Vec::new(),
            canonical_root: None,
            usage: SourceUsage::default(),
            syntax_flavor: SyntaxFlavor::ProjectSyntax,
        };
        let mut pending = vec![("main.ox".to_owned(), None)];
        let mut next = 0;
        while next < pending.len() {
            let (name, declaration) = &pending[next];
            let matching: Vec<_> = files.iter().filter(|(path, _)| path == name).collect();
            assert_eq!(matching.len(), 1, "fixture path must be unique: {name}");
            let text = matching[0].1;
            let file = project.sources.add(name.clone(), text.into());
            let source = project.sources.get(file);
            let tokens = lexer::lex(source).unwrap();
            let (program, nodes) = parser::parse_typed_counted(
                source,
                tokens,
                parser::SourceMode::ProjectCandidate,
                parser::MAX_NODES,
                &mut Allocator::default(),
                &mut Default::default(),
            )
            .unwrap();
            assert!(program.belongs_to(source));
            project.usage.source_bytes += text.len();
            project.usage.non_eof_tokens += program.tokens.len() - 1;
            project.usage.syntax_nodes += nodes;
            project.usage.line_starts += source.line_count();
            project.usage.modules += 1;
            project.usage.retained_path_bytes += name.len();
            let (parent, declaration, public, depth, relative_path) = match declaration {
                None => (None, None, None, 0, String::new()),
                Some(decl) => {
                    let decl: &ast::ModuleDecl = decl;
                    assert!(
                        program.modules.is_empty(),
                        "fixture supports root siblings only"
                    );
                    project.usage.retained_path_bytes += name.len();
                    (
                        Some(ModuleId(0)),
                        Some(decl.name),
                        decl.public,
                        1,
                        name.clone(),
                    )
                }
            };
            project.modules.push(ModuleHeader {
                file,
                parent,
                declaration,
                public,
                depth,
                relative_path,
                canonical_path: None,
            });
            if next == 0 {
                for decl in &program.modules {
                    let child = format!("{}.ox", source.text_at(decl.name));
                    assert!(
                        !pending.iter().any(|(path, _)| path == &child),
                        "fixture module declared twice"
                    );
                    pending.push((child, Some(*decl)));
                }
            }
            project.programs.push(program);
            next += 1;
        }
        assert_eq!(project.programs.len(), files.len(), "unused fixture source");
        project
    }

    pub(super) fn corrupt_header_file_for_u8_test(&mut self, module: usize, file: SourceFileId) {
        self.modules[module].file = file;
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod lexical_loader_layout_tests {
    use super::*;
    use std::mem::{align_of, size_of};
    // Exact retained baseline carriers. The optional mutable provider belongs
    // only to call frames; changing these layouts is not part of this route.
    struct BaselineProjectSources {
        sources: SourceMap,
        programs: Vec<ast::Program>,
        modules: Vec<ModuleHeader>,
        canonical_root: Option<PathBuf>,
        usage: SourceUsage,
        syntax_flavor: SyntaxFlavor,
    }
    struct BaselineLoadFailure {
        sources: SourceMap,
        diagnostics: Vec<Diagnostic>,
        usage: SourceUsage,
        allocator: Allocator,
    }
    struct BaselineBuilder<'a> {
        entry: &'a str,
        limits: ProjectLimits,
        allocator: &'a mut Allocator,
        mode: parser::SourceMode,
        arrays: parser::ArraySyntaxPolicy,
        enums: ProjectEnumSyntax,
        project: ProjectSources,
    }
    #[test]
    fn lexical_provider_keeps_all_retained_loader_layouts_unchanged() {
        assert_eq!(
            size_of::<ProjectSources>(),
            size_of::<BaselineProjectSources>()
        );
        assert_eq!(
            align_of::<ProjectSources>(),
            align_of::<BaselineProjectSources>()
        );
        assert_eq!(size_of::<LoadFailure>(), size_of::<BaselineLoadFailure>());
        assert_eq!(align_of::<LoadFailure>(), align_of::<BaselineLoadFailure>());
        assert_eq!(
            size_of::<SourceSetBuilder<'_>>(),
            size_of::<BaselineBuilder<'_>>()
        );
        assert_eq!(
            align_of::<SourceSetBuilder<'_>>(),
            align_of::<BaselineBuilder<'_>>()
        );
    }
}
// Append to frontend/project.rs, or keep this module in a test-only included file.
#[cfg(test)]
mod lexer_reservation_caller_tests {
    use super::*;
    use lexer::{Kind, Token};

    const INPUT: &str = "fn main()->(){return;}";
    const COMMENTS: &str = "/**//**//**//**/";
    const MALFORMED: &str = ";/*";
    const ZERO_CREDIT: &str = " ";

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Callback {
        Begin,
        Observe,
        ComparisonStarted,
        ComparisonFinished(bool),
        Selected,
    }
    #[derive(Clone, Copy, Debug)]
    enum Association {
        Valid,
        Identity,
        File,
        Length,
    }
    #[derive(Clone, Copy, Debug)]
    enum Mutation {
        None,
        TokenKind,
        TokenStart,
        TokenEnd,
        TokenFile,
        TokenCount,
        DiagnosticCode,
        DiagnosticStage,
        DiagnosticMessage,
        DiagnosticPrimary,
        DiagnosticSecondary,
        DiagnosticNotes,
        Outcome,
    }

    // Callback receipts are bounded and allocation-free. The fake provider's
    // observed token/diagnostic value is constructed before canonical lexing.
    // This proves preservation of completed observation callbacks, not execution
    // of the external production provider process.
    struct Provider {
        association: Association,
        mutation: Mutation,
        refuse_begin: bool,
        refuse_observe: bool,
        refuse_selected: bool,
        callbacks: [Option<Callback>; 5],
        callback_count: usize,
        completed_observations: usize,
        allocator_address: usize,
        attempts_after_observe: usize,
        observed_token_address: usize,
        observed_message_address: usize,
        selected_token_address: usize,
    }
    impl Provider {
        fn new(association: Association, mutation: Mutation) -> Self {
            Self {
                association,
                mutation,
                refuse_begin: false,
                refuse_observe: false,
                refuse_selected: false,
                callbacks: [None; 5],
                callback_count: 0,
                completed_observations: 0,
                allocator_address: 0,
                attempts_after_observe: 0,
                observed_token_address: 0,
                observed_message_address: 0,
                selected_token_address: 0,
            }
        }
        fn record(&mut self, callback: Callback) {
            let slot = self
                .callbacks
                .get_mut(self.callback_count)
                .expect("unexpected callback");
            *slot = Some(callback);
            self.callback_count += 1;
        }
        fn assert_callbacks(&self, expected: &[Callback]) {
            assert_eq!(self.callback_count, expected.len());
            for (index, &callback) in expected.iter().enumerate() {
                assert_eq!(self.callbacks[index], Some(callback));
            }
            assert!(self.callbacks[expected.len()..].iter().all(Option::is_none));
        }
        fn assert_completed(&self, allocator: &Allocator, matched: bool) {
            self.assert_callbacks(&[
                Callback::Begin,
                Callback::Observe,
                Callback::ComparisonStarted,
                Callback::ComparisonFinished(matched),
            ]);
            assert_eq!(self.completed_observations, 1);
            assert_eq!(self.attempts_after_observe, 2);
            assert_eq!(
                self.allocator_address,
                allocator as *const Allocator as usize
            );
            assert_eq!(self.selected_token_address, 0);
        }
    }

    fn observed_value(source: &SourceFile) -> Result<Vec<Token>, Box<Diagnostic>> {
        let spans: &[(Kind, usize, usize)] = match source.text() {
            INPUT => &[
                (Kind::Fn, 0, 2),
                (Kind::Trivia, 2, 3),
                (Kind::Ident, 3, 7),
                (Kind::LParen, 7, 8),
                (Kind::RParen, 8, 9),
                (Kind::Arrow, 9, 11),
                (Kind::LParen, 11, 12),
                (Kind::RParen, 12, 13),
                (Kind::LBrace, 13, 14),
                (Kind::Return, 14, 20),
                (Kind::Semi, 20, 21),
                (Kind::RBrace, 21, 22),
                (Kind::Eof, 22, 22),
            ],
            COMMENTS => &[
                (Kind::Trivia, 0, 4),
                (Kind::Trivia, 4, 8),
                (Kind::Trivia, 8, 12),
                (Kind::Trivia, 12, 16),
                (Kind::Eof, 16, 16),
            ],
            "" => &[(Kind::Eof, 0, 0)],
            ZERO_CREDIT => {
                return Err(Diagnostic::new(
                    "E0400",
                    "lex",
                    "token resource limit exceeded",
                    Some(source.span(0, 1)),
                ))
            }
            MALFORMED => {
                return Err(Diagnostic::new(
                    "E0100",
                    "lex",
                    "unterminated block comment",
                    Some(source.span(1, 3)),
                ))
            }
            _ => panic!("fixture lacks independently specified observation"),
        };
        Ok(spans
            .iter()
            .map(|&(kind, start, end)| Token {
                kind,
                span: source.span(start, end),
            })
            .collect())
    }

    impl LexicalProvider for Provider {
        fn begin_module(
            &mut self,
            source: &SourceFile,
            _usage: SourceUsage,
        ) -> Result<(), Box<Diagnostic>> {
            self.record(Callback::Begin);
            if self.refuse_begin {
                return Err(lexical_failure(source, "test begin refusal"));
            }
            Ok(())
        }
        fn observe(
            &mut self,
            source: &SourceFile,
            _limit: usize,
            _inventory: &Inventory,
            allocator: &mut Allocator,
        ) -> Result<LexicalObservation, Box<Diagnostic>> {
            self.record(Callback::Observe);
            self.allocator_address = allocator as *const Allocator as usize;
            if self.refuse_observe {
                return Err(lexical_failure(source, "test observe refusal"));
            }
            let mut value = observed_value(source);
            if matches!(self.mutation, Mutation::Outcome) {
                value = match value {
                    Ok(_) => Err(Diagnostic::new(
                        "E0100",
                        "lex",
                        "forged content error",
                        Some(source.span(0, 0)),
                    )),
                    Err(_) => Ok(vec![Token {
                        kind: Kind::Eof,
                        span: source.span(source.text().len(), source.text().len()),
                    }]),
                };
            }
            match &mut value {
                Ok(tokens) => {
                    match self.mutation {
                        Mutation::TokenKind => tokens[0].kind = Kind::Invalid,
                        Mutation::TokenStart => tokens[0].span.start += 1,
                        Mutation::TokenEnd => tokens[0].span.end -= 1,
                        Mutation::TokenFile => tokens[0].span.file = SourceFileId(1),
                        Mutation::TokenCount => {
                            tokens.pop();
                        }
                        _ => {}
                    }
                    self.observed_token_address = tokens.as_ptr() as usize;
                }
                Err(error) => {
                    match self.mutation {
                        Mutation::DiagnosticCode => error.code = "E0400",
                        Mutation::DiagnosticStage => error.stage = "parse",
                        Mutation::DiagnosticMessage => error.message.push('!'),
                        Mutation::DiagnosticPrimary => error.primary = Some(source.span(0, 3)),
                        Mutation::DiagnosticSecondary => {
                            error.secondary.push((source.span(0, 1), "extra".into()))
                        }
                        Mutation::DiagnosticNotes => error.notes.push("extra".into()),
                        _ => {}
                    }
                    self.observed_message_address = error.message.as_ptr() as usize;
                }
            }
            let mut observation = LexicalObservation::new(source, value);
            match self.association {
                Association::Valid => {}
                Association::Identity => observation.identity = source.identity().wrapping_add(1),
                Association::File => observation.file = SourceFileId(1),
                Association::Length => observation.source_len += 1,
            }
            self.completed_observations += 1;
            self.attempts_after_observe = allocator.attempts;
            Ok(observation)
        }
        fn comparison_started(&mut self) {
            self.record(Callback::ComparisonStarted);
        }
        fn comparison_finished(&mut self, matched: bool) {
            self.record(Callback::ComparisonFinished(matched));
        }
        fn selected_for_parser(
            &mut self,
            source: &SourceFile,
            tokens: &[Token],
            capacity: usize,
        ) -> Result<(), Box<Diagnostic>> {
            self.record(Callback::Selected);
            self.selected_token_address = tokens.as_ptr() as usize;
            assert_eq!(self.selected_token_address, self.observed_token_address);
            assert!(capacity >= tokens.len());
            if self.refuse_selected {
                return Err(lexical_failure(source, "test selected refusal"));
            }
            Ok(())
        }
    }

    fn builder(allocator: &mut Allocator) -> SourceSetBuilder<'_> {
        SourceSetBuilder {
            entry: "retained-provider.ox",
            limits: ProjectLimits::default(),
            allocator,
            mode: parser::SourceMode::OwnedCandidate,
            arrays: parser::ArraySyntaxPolicy::Closed,
            enums: ProjectEnumSyntax::Closed,
            project: ProjectSources {
                sources: SourceMap::new(),
                programs: Vec::new(),
                modules: Vec::new(),
                canonical_root: None,
                usage: SourceUsage::default(),
                syntax_flavor: SyntaxFlavor::OriginalSingleFile,
            },
        }
    }
    fn parse_with_provider(
        builder: &mut SourceSetBuilder<'_>,
        text: &str,
        provider: &mut Provider,
    ) -> Result<ast::Program, Errors> {
        builder.parse_file_with_provider(
            "retained-provider.ox".into(),
            text.into(),
            None,
            &mut Some(provider as &mut dyn LexicalProvider),
        )
    }
    fn assert_error(
        errors: &[Diagnostic],
        code: &str,
        stage: &str,
        message: &str,
        primary: Option<Span>,
    ) {
        assert_eq!(errors.len(), 1, "{errors:?}");
        let error = &errors[0];
        assert_eq!(error.code, code);
        assert_eq!(error.stage, stage);
        assert_eq!(error.message, message);
        assert_eq!(error.primary, primary);
        assert!(error.secondary.is_empty());
        assert!(error.notes.is_empty());
    }
    fn assert_trace(allocator: &Allocator, targets: &[usize], failed: bool) {
        // Direct parse_file has only source-owner line/file requests before lex.
        assert_eq!(allocator.attempts, 2 + targets.len());
        assert_eq!(allocator.trace.len(), 2 + targets.len());
        assert!(!allocator.observer_trace_overflow);
        for (index, event) in allocator.trace.iter().enumerate() {
            let expected = match index {
                0 => ("line starts", 1, size_of::<usize>()),
                1 => ("source files", 1, size_of::<SourceFile>()),
                _ => ("lexer token tape", targets[index - 2], size_of::<Token>()),
            };
            assert_eq!((event.kind, event.length, event.element_bytes), expected);
            assert_eq!(event.success, !failed || index + 1 != allocator.attempts);
        }
    }
    fn assert_retained(builder: &SourceSetBuilder<'_>, text: &str, non_eof_tokens: usize) {
        assert_eq!(builder.project.sources.files().len(), 1);
        let source = builder.project.sources.get(SourceFileId(0));
        assert_eq!(source.text(), text);
        assert_eq!(source.path(), "retained-provider.ox");
        assert_eq!(builder.project.usage.non_eof_tokens, non_eof_tokens);
        assert_eq!(builder.project.usage.line_starts, 1);
        assert!(builder.project.programs.is_empty());
        assert!(builder.project.modules.is_empty());
    }

    #[test]
    fn lexer_caller_provider_valid_storage_failures_preserve_completed_observe() {
        // Each predicted request, including a growth-at-EOF and empty EOF.
        type ReservationSite = (usize, usize, usize);
        let cases: &[(&str, &[ReservationSite])] = &[
            (INPUT, &[(4, 0, 2), (8, 8, 9), (16, 13, 14)]),
            (COMMENTS, &[(4, 0, 4), (8, 16, 16)]),
            ("", &[(4, 0, 0)]),
            (MALFORMED, &[(4, 0, 1)]),
        ];
        for &(text, sites) in cases {
            for (index, &(_, start, end)) in sites.iter().enumerate() {
                let mut allocator = Allocator {
                    fail_at: Some(3 + index),
                    ..Allocator::default()
                };
                allocator.observer_trace_bound(16).unwrap();
                let mut provider = Provider::new(Association::Valid, Mutation::None);
                let mut builder = builder(&mut allocator);
                let errors = parse_with_provider(&mut builder, text, &mut provider).unwrap_err();
                let source = builder.project.sources.get(SourceFileId(0));
                assert_error(
                    &errors,
                    "E0400",
                    "lex",
                    "token storage allocation failed",
                    Some(source.span(start, end)),
                );
                let targets: Vec<_> = sites[..=index]
                    .iter()
                    .map(|&(target, _, _)| target)
                    .collect();
                assert_trace(builder.allocator, &targets, true);
                provider.assert_completed(builder.allocator, false);
                assert_retained(&builder, text, 0);
                assert_eq!(builder.project.usage.syntax_nodes, 0);
            }
        }
    }

    #[test]
    fn lexer_caller_invalid_association_wins_after_every_canonical_storage_failure() {
        for association in [
            Association::Identity,
            Association::File,
            Association::Length,
        ] {
            for (index, targets) in [&[4][..], &[4, 8][..], &[4, 8, 16][..]].iter().enumerate() {
                let mut allocator = Allocator {
                    fail_at: Some(3 + index),
                    ..Allocator::default()
                };
                allocator.observer_trace_bound(16).unwrap();
                let mut provider = Provider::new(association, Mutation::None);
                let mut builder = builder(&mut allocator);
                let errors = parse_with_provider(&mut builder, INPUT, &mut provider).unwrap_err();
                let source = builder.project.sources.get(SourceFileId(0));
                assert_error(
                    &errors,
                    "E0703",
                    "lexical-provider",
                    "lexical observation belongs to another retained source",
                    Some(source.span(0, 0)),
                );
                // The failed request still happened: association validation must
                // neither move before canonical execution nor lose precedence.
                assert_trace(builder.allocator, targets, true);
                provider.assert_completed(builder.allocator, false);
                assert_retained(&builder, INPUT, 0);
                assert_eq!(builder.project.usage.syntax_nodes, 0);
            }
        }
    }

    #[test]
    fn lexer_caller_provider_equal_content_diagnostic_closes_true_without_selection() {
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(16).unwrap();
        let mut provider = Provider::new(Association::Valid, Mutation::None);
        let mut builder = builder(&mut allocator);
        let errors = parse_with_provider(&mut builder, MALFORMED, &mut provider).unwrap_err();
        let source = builder.project.sources.get(SourceFileId(0));
        assert_error(
            &errors,
            "E0100",
            "lex",
            "unterminated block comment",
            Some(source.span(1, 3)),
        );
        assert_eq!(
            errors[0].message.as_ptr() as usize,
            provider.observed_message_address
        );
        assert_trace(builder.allocator, &[4], false);
        provider.assert_completed(builder.allocator, true);
        assert_retained(&builder, MALFORMED, 0);
        assert_eq!(builder.project.usage.syntax_nodes, 0);
    }

    #[test]
    fn lexer_caller_provider_token_and_outcome_mismatches_close_false() {
        for mutation in [
            Mutation::TokenKind,
            Mutation::TokenStart,
            Mutation::TokenEnd,
            Mutation::TokenFile,
            Mutation::TokenCount,
            Mutation::Outcome,
        ] {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(16).unwrap();
            let mut provider = Provider::new(Association::Valid, mutation);
            let mut builder = builder(&mut allocator);
            let errors = parse_with_provider(&mut builder, INPUT, &mut provider).unwrap_err();
            let message = if matches!(mutation, Mutation::Outcome) {
                "lexical provider disagrees with canonical outcome"
            } else {
                "lexical provider disagrees with canonical tokens"
            };
            let source = builder.project.sources.get(SourceFileId(0));
            assert_error(
                &errors,
                "E0703",
                "lexical-provider",
                message,
                Some(source.span(0, 0)),
            );
            assert_trace(builder.allocator, &[4, 8, 16], false);
            provider.assert_completed(builder.allocator, false);
            assert_retained(&builder, INPUT, 0);
            assert_eq!(builder.project.usage.syntax_nodes, 0);
        }
    }

    #[test]
    fn lexer_caller_provider_each_diagnostic_field_and_outcome_mismatch_close_false() {
        for mutation in [
            Mutation::DiagnosticCode,
            Mutation::DiagnosticStage,
            Mutation::DiagnosticMessage,
            Mutation::DiagnosticPrimary,
            Mutation::DiagnosticSecondary,
            Mutation::DiagnosticNotes,
            Mutation::Outcome,
        ] {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(16).unwrap();
            let mut provider = Provider::new(Association::Valid, mutation);
            let mut builder = builder(&mut allocator);
            let errors = parse_with_provider(&mut builder, MALFORMED, &mut provider).unwrap_err();
            let message = if matches!(mutation, Mutation::Outcome) {
                "lexical provider disagrees with canonical outcome"
            } else {
                "lexical provider disagrees with canonical diagnostic"
            };
            let source = builder.project.sources.get(SourceFileId(0));
            assert_error(
                &errors,
                "E0703",
                "lexical-provider",
                message,
                Some(source.span(0, 0)),
            );
            assert_trace(builder.allocator, &[4], false);
            provider.assert_completed(builder.allocator, false);
            assert_retained(&builder, MALFORMED, 0);
            assert_eq!(builder.project.usage.syntax_nodes, 0);
        }
    }

    #[test]
    fn lexer_caller_provider_begin_and_observe_refusal_precede_canonical_request() {
        for refuse_begin in [true, false] {
            let mut allocator = Allocator {
                fail_at: Some(3),
                ..Allocator::default()
            };
            allocator.observer_trace_bound(16).unwrap();
            let mut provider = Provider::new(Association::Valid, Mutation::None);
            provider.refuse_begin = refuse_begin;
            provider.refuse_observe = !refuse_begin;
            let mut builder = builder(&mut allocator);
            let errors = parse_with_provider(&mut builder, INPUT, &mut provider).unwrap_err();
            let source = builder.project.sources.get(SourceFileId(0));
            assert_error(
                &errors,
                "E0703",
                "lexical-provider",
                if refuse_begin {
                    "test begin refusal"
                } else {
                    "test observe refusal"
                },
                Some(source.span(0, 0)),
            );
            assert_trace(builder.allocator, &[], false);
            provider.assert_callbacks(if refuse_begin {
                &[Callback::Begin]
            } else {
                &[Callback::Begin, Callback::Observe]
            });
            assert_eq!(provider.completed_observations, 0);
            assert_retained(&builder, INPUT, 0);
        }
    }

    #[test]
    fn lexer_caller_provider_selected_tape_ownership_and_selection_refusal_are_preserved() {
        for refuse_selected in [false, true] {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(16).unwrap();
            let mut provider = Provider::new(Association::Valid, Mutation::None);
            provider.refuse_selected = refuse_selected;
            let mut builder = builder(&mut allocator);
            let result = parse_with_provider(&mut builder, INPUT, &mut provider);
            if refuse_selected {
                let errors = result.unwrap_err();
                let source = builder.project.sources.get(SourceFileId(0));
                assert_error(
                    &errors,
                    "E0703",
                    "lexical-provider",
                    "test selected refusal",
                    Some(source.span(0, 0)),
                );
                assert_eq!(builder.project.usage.syntax_nodes, 0);
            } else {
                let program = result.unwrap();
                assert_eq!(
                    program.tokens.as_ptr() as usize,
                    provider.observed_token_address
                );
                assert_eq!(program.tokens.len(), 13);
                assert!(program.belongs_to(builder.project.sources.get(SourceFileId(0))));
                assert!(builder.project.usage.syntax_nodes > 0);
            }
            provider.assert_callbacks(&[
                Callback::Begin,
                Callback::Observe,
                Callback::ComparisonStarted,
                Callback::ComparisonFinished(true),
                Callback::Selected,
            ]);
            assert_eq!(provider.completed_observations, 1);
            assert_eq!(provider.attempts_after_observe, 2);
            assert_eq!(
                provider.observed_token_address,
                provider.selected_token_address
            );
            assert_trace(builder.allocator, &[4, 8, 16], false);
            assert_retained(&builder, INPUT, 12);
        }
    }

    #[test]
    fn lexer_caller_valid_association_storage_failure_is_not_an_outcome_mismatch() {
        for ordinal in [3, 4, 5] {
            let mut allocator = Allocator {
                fail_at: Some(ordinal),
                ..Allocator::default()
            };
            allocator.observer_trace_bound(16).unwrap();
            let mut provider = Provider::new(Association::Valid, Mutation::Outcome);
            let mut builder = builder(&mut allocator);
            let errors = parse_with_provider(&mut builder, INPUT, &mut provider).unwrap_err();
            let source = builder.project.sources.get(SourceFileId(0));
            let (start, end) = [(0, 2), (8, 9), (13, 14)][ordinal - 3];
            assert_error(
                &errors,
                "E0400",
                "lex",
                "token storage allocation failed",
                Some(source.span(start, end)),
            );
            assert_trace(builder.allocator, &[4, 8, 16][..ordinal - 2], true);
            provider.assert_completed(builder.allocator, false);
            assert_retained(&builder, INPUT, 0);
        }
    }
    #[test]
    fn lexer_caller_ordinary_route_uses_same_allocator_for_all_token_and_eof_sites() {
        type ReservationSite = (usize, usize, usize);
        let cases: &[(&str, &[ReservationSite])] = &[
            (INPUT, &[(4, 0, 2), (8, 8, 9), (16, 13, 14)]),
            (COMMENTS, &[(4, 0, 4), (8, 16, 16)]),
            ("", &[(4, 0, 0)]),
        ];
        for &(text, sites) in cases {
            for (index, &(_, start, end)) in sites.iter().enumerate() {
                let mut allocator = Allocator {
                    fail_at: Some(3 + index),
                    ..Allocator::default()
                };
                allocator.observer_trace_bound(16).unwrap();
                let mut builder = builder(&mut allocator);
                let errors = builder
                    .parse_file("retained-provider.ox".into(), text.into(), None)
                    .unwrap_err();
                let source = builder.project.sources.get(SourceFileId(0));
                assert_error(
                    &errors,
                    "E0400",
                    "lex",
                    "token storage allocation failed",
                    Some(source.span(start, end)),
                );
                let targets: Vec<_> = sites[..=index]
                    .iter()
                    .map(|&(target, _, _)| target)
                    .collect();
                assert_trace(builder.allocator, &targets, true);
                assert_retained(&builder, text, 0);
                assert_eq!(builder.project.usage.syntax_nodes, 0);
            }
        }
    }

    #[test]
    fn lexer_caller_invalid_association_keeps_precedence_over_success_and_content_error() {
        for association in [
            Association::Identity,
            Association::File,
            Association::Length,
        ] {
            for (text, targets) in [(INPUT, &[4, 8, 16][..]), (MALFORMED, &[4][..])] {
                let mut allocator = Allocator::default();
                allocator.observer_trace_bound(16).unwrap();
                let mut provider = Provider::new(association, Mutation::None);
                let mut builder = builder(&mut allocator);
                let errors = parse_with_provider(&mut builder, text, &mut provider).unwrap_err();
                let source = builder.project.sources.get(SourceFileId(0));
                assert_error(
                    &errors,
                    "E0703",
                    "lexical-provider",
                    "lexical observation belongs to another retained source",
                    Some(source.span(0, 0)),
                );
                assert_trace(builder.allocator, targets, false);
                provider.assert_completed(builder.allocator, false);
                assert_retained(&builder, text, 0);
                assert_eq!(builder.project.usage.syntax_nodes, 0);
            }
        }
    }

    #[test]
    fn lexer_caller_equal_content_resource_error_is_matched_not_storage_failure() {
        let mut allocator = Allocator {
            fail_at: Some(3),
            ..Allocator::default()
        };
        allocator.observer_trace_bound(16).unwrap();
        let mut provider = Provider::new(Association::Valid, Mutation::None);
        let mut builder = builder(&mut allocator);
        builder.limits.tokens = 0;
        let errors = parse_with_provider(&mut builder, ZERO_CREDIT, &mut provider).unwrap_err();
        let source = builder.project.sources.get(SourceFileId(0));
        assert_error(
            &errors,
            "E0400",
            "lex",
            "token resource limit exceeded",
            Some(source.span(0, 1)),
        );
        assert_eq!(
            errors[0].message.as_ptr() as usize,
            provider.observed_message_address
        );
        // Content admission refuses before any canonical reserve; fail_at 3 is not consumed.
        assert_trace(builder.allocator, &[], false);
        provider.assert_completed(builder.allocator, true);
        assert_retained(&builder, ZERO_CREDIT, 0);
        assert_eq!(builder.project.usage.syntax_nodes, 0);
    }
    // Append INSIDE project::lexer_reservation_caller_tests before its closing brace.
    // No successful trace is used to configure a selected request.
    mod actual_null_callers {
        use super::*;
        use crate::frontend::project::budget::{real_null_observer as null, ReserveEvent};
        use null::growth::{self, GrowthReport, GrowthTarget};
        use std::alloc::Layout;
        use std::mem::{align_of, align_of_val, offset_of, size_of_val};

        fn fresh_target(attempt: usize) -> null::Target {
            null::Target {
                attempt,
                kind: "lexer token tape",
                slots: 4,
                element_bytes: size_of::<Token>(),
                layout: Layout::array::<Token>(4).unwrap(),
            }
        }
        fn growth_target(attempt: usize, old: usize, new: usize) -> GrowthTarget {
            GrowthTarget {
                attempt,
                kind: "lexer token tape",
                old_len: old,
                old_capacity: old,
                additional: new - old,
                new_slots: new,
                element_bytes: size_of::<Token>(),
                element_align: align_of::<Token>(),
                old_layout: Layout::array::<Token>(old).unwrap(),
                new_layout: Layout::array::<Token>(new).unwrap(),
                operation: null::Operation::Realloc,
            }
        }
        fn assert_fresh(report: null::Report, target: null::Target) {
            assert_eq!(report.target, target);
            assert!(report.selected && report.matched && report.fired);
            assert_eq!(report.rejection, None);
            assert_eq!(
                report.actual,
                Some(null::GlobalEvent {
                    operation: null::Operation::Alloc,
                    layout: target.layout,
                    new_size: None,
                })
            );
        }
        fn assert_growth(report: GrowthReport, target: GrowthTarget) {
            assert_eq!(report.target, target);
            assert!(report.selected && report.matched && report.fired);
            assert_eq!(report.rejection, None);
            assert_eq!(
                report.actual,
                Some(growth::GrowthEvent {
                    operation: null::Operation::Realloc,
                    layout: target.old_layout,
                    new_size: Some(target.new_layout.size()),
                    old_address_matches: true,
                })
            );
            assert_eq!(report.reserve_failed, Some(true));
            assert!(report.owner_unchanged && report.address_unchanged);
            assert!(report.length_unchanged && report.capacity_unchanged);
            assert_eq!(report.drop_count, 1);
            assert_eq!(
                report.drop_event,
                Some(growth::GrowthDropEvent {
                    layout: target.old_layout,
                    old_address_matches: true,
                    after_reserve_return: true,
                })
            );
            assert!(report.trace_preserved);
        }
        fn diagnostic_ok(
            errors: &[Diagnostic],
            file: &SourceFile,
            start: usize,
            end: usize,
            invalid: bool,
        ) -> bool {
            let Some(error) = errors.first() else {
                return false;
            };
            errors.len() == 1
                && error.code == if invalid { "E0703" } else { "E0400" }
                && error.stage == if invalid { "lexical-provider" } else { "lex" }
                && error.message
                    == if invalid {
                        "lexical observation belongs to another retained source"
                    } else {
                        "token storage allocation failed"
                    }
                && error.primary
                    == Some(if invalid {
                        file.span(0, 0)
                    } else {
                        file.span(start, end)
                    })
                && error.secondary.is_empty()
                && error.notes.is_empty()
        }
        fn trace_ok(
            allocator: &Allocator,
            expected: &[(&str, usize, usize)],
            ordinal: usize,
        ) -> bool {
            allocator.attempts == ordinal
                && allocator.trace.len() == ordinal
                && !allocator.observer_trace_overflow
                && allocator.trace.iter().zip(expected).enumerate().all(
                    |(index, (event, expected))| {
                        (event.kind, event.length, event.element_bytes) == *expected
                            && event.success == (index + 1 != ordinal)
                    },
                )
        }
        fn diagnostic_heap(errors: &[Diagnostic], capacity: usize) -> usize {
            errors
                .iter()
                .fold(capacity * size_of::<Diagnostic>(), |bytes, error| {
                    bytes
                        + error.message.capacity()
                        + error.secondary.capacity() * size_of::<(Span, String)>()
                        + error
                            .secondary
                            .iter()
                            .map(|(_, text)| text.capacity())
                            .sum::<usize>()
                        + error.notes.capacity() * size_of::<String>()
                        + error.notes.iter().map(String::capacity).sum::<usize>()
                })
        }
        fn observation_heap(observed: &LexicalObservation) -> usize {
            match &observed.value {
                Ok(tokens) => tokens.capacity() * size_of::<Token>(),
                Err(error) => diagnostic_heap(std::slice::from_ref(error.as_ref()), 1),
            }
        }
        struct MeasuredProvider {
            inner: Provider,
            observed_heap: usize,
        }
        impl LexicalProvider for MeasuredProvider {
            fn begin_module(
                &mut self,
                source: &SourceFile,
                usage: SourceUsage,
            ) -> Result<(), Box<Diagnostic>> {
                self.inner.begin_module(source, usage)
            }
            fn observe(
                &mut self,
                source: &SourceFile,
                limit: usize,
                inventory: &Inventory,
                allocator: &mut Allocator,
            ) -> Result<LexicalObservation, Box<Diagnostic>> {
                let observed = self.inner.observe(source, limit, inventory, allocator)?;
                self.observed_heap = observation_heap(&observed);
                Ok(observed)
            }
            fn comparison_started(&mut self) {
                self.inner.comparison_started();
            }
            fn comparison_finished(&mut self, matched: bool) {
                self.inner.comparison_finished(matched);
            }
            fn selected_for_parser(
                &mut self,
                source: &SourceFile,
                tokens: &[Token],
                capacity: usize,
            ) -> Result<(), Box<Diagnostic>> {
                self.inner.selected_for_parser(source, tokens, capacity)
            }
        }
        #[derive(Clone, Copy, Debug)]
        struct Facts {
            failed: bool,
            diagnostic: bool,
            callbacks: bool,
            retained: bool,
            trace: bool,
            source_heap: usize,
            observed_heap: usize,
            diagnostic_heap: usize,
            prior_token_heap: usize,
        }
        impl Facts {
            fn assert(self) {
                assert!(
                    self.failed && self.diagnostic && self.callbacks && self.retained && self.trace,
                    "{self:?}"
                );
            }
        }
        // Owned strings are prepared before selection and move into real source registration.
        // Provider callback state is destroyed before the primitive Facts result escapes.
        fn direct_action(
            display: String,
            text: String,
            association: Option<Association>,
            ordinal: usize,
            start: usize,
            end: usize,
        ) -> impl for<'a> FnOnce(&'a mut Allocator) -> Facts {
            move |allocator| {
                let mut provider = association.map(|a| MeasuredProvider {
                    inner: Provider::new(a, Mutation::None),
                    observed_heap: 0,
                });
                let mut build = builder(allocator);
                let mut supplied = provider.as_mut().map(|p| p as &mut dyn LexicalProvider);
                let result = build.parse_file_with_provider(display, text, None, &mut supplied);
                let file = build.project.sources.get(SourceFileId(0));
                let invalid = matches!(
                    association,
                    Some(Association::Identity | Association::File | Association::Length)
                );
                let diagnostic = result
                    .as_ref()
                    .err()
                    .is_some_and(|errors| diagnostic_ok(errors, file, start, end, invalid));
                let callbacks = provider.as_ref().is_none_or(|measured| {
                    let p = &measured.inner;
                    p.callback_count == 4
                        && p.callbacks
                            == [
                                Some(Callback::Begin),
                                Some(Callback::Observe),
                                Some(Callback::ComparisonStarted),
                                Some(Callback::ComparisonFinished(false)),
                                None,
                            ]
                        && p.completed_observations == 1
                        && p.attempts_after_observe == 2
                        && p.allocator_address == build.allocator as *const Allocator as usize
                        && p.selected_token_address == 0
                });
                let expected = [
                    ("line starts", 1, size_of::<usize>()),
                    ("source files", 1, size_of::<SourceFile>()),
                    ("lexer token tape", 4, size_of::<Token>()),
                    ("lexer token tape", 8, size_of::<Token>()),
                    ("lexer token tape", 16, size_of::<Token>()),
                ];
                let facts = Facts {
                    failed: result.is_err(),
                    diagnostic,
                    callbacks,
                    retained: build.project.sources.files().len() == 1
                        && file.path() == "retained-provider.ox"
                        && build.project.usage.non_eof_tokens == 0
                        && build.project.usage.syntax_nodes == 0
                        && build.project.usage.line_starts == 1
                        && build.project.programs.is_empty()
                        && build.project.modules.is_empty(),
                    trace: trace_ok(build.allocator, &expected, ordinal),
                    source_heap: build.project.sources.heap_capacity_bytes().unwrap(),
                    observed_heap: provider.as_ref().map_or(0, |p| p.observed_heap),
                    diagnostic_heap: result
                        .as_ref()
                        .err()
                        .map_or(0, |errors| diagnostic_heap(errors, errors.capacity())),
                    prior_token_heap: 0,
                };
                drop(result);
                // End the fixture owner and its borrows before dropping the builder,
                // preserving the reviewed caller lifetime even without a destructor.
                #[allow(clippy::drop_non_drop)]
                drop(provider);
                drop(build);
                facts
            }
        }

        #[test]
        fn lexer_caller_actual_null_direct_ordinary_and_provider_all_sites() {
            // 3 ordinary + 3 valid + 9 invalid association sites for INPUT, plus
            // EOF growth and empty/malformed first sites for ordinary/valid routes.
            type ReservationSite = (usize, usize, usize, usize);
            let cases: &[(&str, &[ReservationSite])] = &[
                (INPUT, &[(3, 4, 0, 2), (4, 8, 8, 9), (5, 16, 13, 14)]),
                (COMMENTS, &[(3, 4, 0, 4), (4, 8, 16, 16)]),
                ("", &[(3, 4, 0, 0)]),
                (MALFORMED, &[(3, 4, 0, 1)]),
            ];
            for &(text, sites) in cases {
                for association in [
                    None,
                    Some(Association::Valid),
                    Some(Association::Identity),
                    Some(Association::File),
                    Some(Association::Length),
                ] {
                    if text != INPUT && !matches!(association, None | Some(Association::Valid)) {
                        continue;
                    }
                    for &(ordinal, slots, start, end) in sites {
                        let mut allocator = Allocator::default();
                        allocator.observer_trace_bound(5).unwrap();
                        let trace_capacity = allocator.trace.capacity();
                        let action = direct_action(
                            "retained-provider.ox".into(),
                            text.into(),
                            association,
                            ordinal,
                            start,
                            end,
                        );
                        let facts = if slots == 4 {
                            let target = fresh_target(ordinal);
                            let (facts, report) =
                                null::with_selected(&mut allocator, target, action).unwrap();
                            assert_fresh(report, target);
                            facts
                        } else {
                            let target = growth_target(ordinal, slots / 2, slots);
                            let (facts, report) =
                                growth::with_selected_growth(&mut allocator, target, action)
                                    .unwrap();
                            assert_growth(report, target);
                            facts
                        };
                        println!("caller-null-heap direct ordinal={ordinal} source={} observed={} diagnostic={} old-selected={}", facts.source_heap, facts.observed_heap, facts.diagnostic_heap, if slots == 4 { 0 } else { slots / 2 * size_of::<Token>() });
                        facts.assert();
                        assert_eq!(allocator.trace.capacity(), trace_capacity);
                        assert_eq!(allocator.attempts, ordinal);
                        assert_eq!(allocator.trace.len(), ordinal);
                    }
                }
            }
        }

        const MODULE_ROOT: &str = "mod a;";
        struct ModuleProvider {
            association: Association,
            current: SourceFileId,
            callbacks: [Option<(SourceFileId, Callback)>; 10],
            used: usize,
            overflow: bool,
            observations: usize,
            observed_at: [usize; 2],
            observed_heaps: [usize; 2],
        }
        impl ModuleProvider {
            fn new(association: Association) -> Self {
                Self {
                    association,
                    current: SourceFileId(0),
                    callbacks: [None; 10],
                    used: 0,
                    overflow: false,
                    observations: 0,
                    observed_at: [0; 2],
                    observed_heaps: [0; 2],
                }
            }
            fn record(&mut self, callback: Callback) {
                if let Some(slot) = self.callbacks.get_mut(self.used) {
                    *slot = Some((self.current, callback));
                    self.used += 1;
                } else {
                    self.overflow = true;
                }
            }
        }
        impl LexicalProvider for ModuleProvider {
            fn begin_module(
                &mut self,
                source: &SourceFile,
                _: SourceUsage,
            ) -> Result<(), Box<Diagnostic>> {
                self.current = source.span(0, 0).file;
                self.record(Callback::Begin);
                Ok(())
            }
            fn observe(
                &mut self,
                source: &SourceFile,
                _: usize,
                _: &Inventory,
                allocator: &mut Allocator,
            ) -> Result<LexicalObservation, Box<Diagnostic>> {
                self.record(Callback::Observe);
                let value = if source.text() == MODULE_ROOT {
                    Ok([
                        (Kind::Mod, 0, 3),
                        (Kind::Trivia, 3, 4),
                        (Kind::Ident, 4, 5),
                        (Kind::Semi, 5, 6),
                        (Kind::Eof, 6, 6),
                    ]
                    .iter()
                    .map(|&(kind, start, end)| Token {
                        kind,
                        span: source.span(start, end),
                    })
                    .collect())
                } else {
                    observed_value(source)
                };
                let mut observed = LexicalObservation::new(source, value);
                if self.current == SourceFileId(1) {
                    match self.association {
                        Association::Valid => {}
                        Association::Identity => {
                            observed.identity = source.identity().wrapping_add(1)
                        }
                        Association::File => observed.file = SourceFileId(0),
                        Association::Length => observed.source_len += 1,
                    }
                }
                if let Some(slot) = self.observed_at.get_mut(self.observations) {
                    *slot = allocator.attempts;
                } else {
                    self.overflow = true;
                }
                if let Some(slot) = self.observed_heaps.get_mut(self.observations) {
                    *slot = observation_heap(&observed);
                }
                self.observations += 1;
                Ok(observed)
            }
            fn comparison_started(&mut self) {
                self.record(Callback::ComparisonStarted);
            }
            fn comparison_finished(&mut self, matched: bool) {
                self.record(Callback::ComparisonFinished(matched));
            }
            fn selected_for_parser(
                &mut self,
                _: &SourceFile,
                _: &[Token],
                _: usize,
            ) -> Result<(), Box<Diagnostic>> {
                self.record(Callback::Selected);
                Ok(())
            }
        }
        fn module_schedule(
            entry: &str,
            canonical_root_bytes: usize,
        ) -> [(&'static str, usize, usize); 20] {
            let display = Path::new(entry).parent().unwrap().to_str().unwrap();
            let child_display =
                display.len() + usize::from(!display.is_empty() && !display.ends_with('/')) + 4;
            [
                ("source bytes", 6, 1),
                ("entry display", entry.len(), 1),
                ("line starts", 1, size_of::<usize>()),
                ("source files", 1, size_of::<SourceFile>()),
                ("lexer token tape", 4, size_of::<Token>()),
                ("lexer token tape", 8, size_of::<Token>()),
                ("module declarations", 1, size_of::<ast::ModuleDecl>()),
                ("module items", 1, size_of::<ast::ItemId>()),
                ("file ASTs", 1, size_of::<ast::Program>()),
                ("module headers", 1, size_of::<ModuleHeader>()),
                ("logical module path", 4, 1),
                ("module display", child_display, 1),
                ("module probe path", canonical_root_bytes + 1 + 4, 1),
                ("file ASTs", 2, size_of::<ast::Program>()),
                ("module headers", 2, size_of::<ModuleHeader>()),
                ("source bytes", COMMENTS.len(), 1),
                ("line starts", 1, size_of::<usize>()),
                ("source files", 2, size_of::<SourceFile>()),
                ("lexer token tape", 4, size_of::<Token>()),
                ("lexer token tape", 8, size_of::<Token>()),
            ]
        }
        fn module_action<'e>(
            entry: &'e str,
            expected: &'e [(&'static str, usize, usize); 20],
            association: Option<Association>,
            ordinal: usize,
        ) -> impl for<'a> FnOnce(&'a mut Allocator) -> Facts + 'e {
            move |allocator| {
                let mut provider = association.map(ModuleProvider::new);
                let mut build = builder(allocator);
                build.entry = entry;
                build.mode = parser::SourceMode::ModuleCandidate;
                let result = build.load_all_with_provider(
                    &mut provider.as_mut().map(|p| p as &mut dyn LexicalProvider),
                );
                let child = ordinal >= 19;
                let file = build.project.sources.get(SourceFileId(usize::from(child)));
                let (start, end) = match ordinal {
                    5 => (0, 3),
                    6 => (6, 6),
                    19 => (0, 4),
                    20 => (16, 16),
                    _ => unreachable!(),
                };
                let invalid = child
                    && matches!(
                        association,
                        Some(Association::Identity | Association::File | Association::Length)
                    );
                let diagnostic = result
                    .as_ref()
                    .err()
                    .is_some_and(|errors| diagnostic_ok(errors, file, start, end, invalid));
                let expected_callbacks = [
                    Some((SourceFileId(0), Callback::Begin)),
                    Some((SourceFileId(0), Callback::Observe)),
                    Some((SourceFileId(0), Callback::ComparisonStarted)),
                    Some((SourceFileId(0), Callback::ComparisonFinished(true))),
                    Some((SourceFileId(0), Callback::Selected)),
                    Some((SourceFileId(1), Callback::Begin)),
                    Some((SourceFileId(1), Callback::Observe)),
                    Some((SourceFileId(1), Callback::ComparisonStarted)),
                    Some((SourceFileId(1), Callback::ComparisonFinished(false))),
                    None,
                ];
                let callbacks = provider.as_ref().is_none_or(|p| {
                    !p.overflow
                        && if child {
                            p.used == 9
                                && p.callbacks == expected_callbacks
                                && p.observations == 2
                                && p.observed_at == [4, 18]
                        } else {
                            p.used == 4
                                && p.callbacks[..3] == expected_callbacks[..3]
                                && p.callbacks[3]
                                    == Some((SourceFileId(0), Callback::ComparisonFinished(false)))
                                && p.callbacks[4..].iter().all(Option::is_none)
                                && p.observations == 1
                                && p.observed_at == [4, 0]
                        }
                });
                let facts = Facts {
                    failed: result.is_err(),
                    diagnostic,
                    callbacks,
                    retained: build.project.sources.files().len() == if child { 2 } else { 1 }
                        && file.text() == if child { COMMENTS } else { MODULE_ROOT }
                        && build.project.programs.len() == usize::from(child)
                        && build.project.modules.len() == usize::from(child)
                        && build.project.usage.non_eof_tokens == if child { 4 } else { 0 }
                        && build.project.usage.syntax_nodes == usize::from(child)
                        && build.project.usage.modules == usize::from(child)
                        && build.project.usage.source_bytes == if child { 22 } else { 6 }
                        && build.project.usage.line_starts == if child { 2 } else { 1 },
                    trace: trace_ok(build.allocator, expected, ordinal),
                    source_heap: build.project.sources.heap_capacity_bytes().unwrap(),
                    observed_heap: provider
                        .as_ref()
                        .map_or(0, |p| p.observed_heaps[usize::from(child)]),
                    diagnostic_heap: result
                        .as_ref()
                        .err()
                        .map_or(0, |errors| diagnostic_heap(errors, errors.capacity())),
                    prior_token_heap: build
                        .project
                        .programs
                        .iter()
                        .map(|program| program.tokens.capacity() * size_of::<Token>())
                        .sum(),
                };
                drop(result);
                // End the fixture owner and its borrows before dropping the builder,
                // preserving the reviewed caller lifetime even without a destructor.
                #[allow(clippy::drop_non_drop)]
                drop(provider);
                drop(build);
                facts
            }
        }
        struct ModuleFixture(PathBuf);
        impl ModuleFixture {
            fn new() -> Self {
                use std::sync::atomic::{AtomicUsize, Ordering};
                static NEXT: AtomicUsize = AtomicUsize::new(0);
                let directory = std::env::temp_dir().join(format!(
                    "oxid-lexer-real-null-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                std::fs::create_dir(&directory).unwrap();
                std::fs::write(directory.join("app.ox"), MODULE_ROOT).unwrap();
                std::fs::write(directory.join("a.ox"), COMMENTS).unwrap();
                Self(directory)
            }
        }
        impl Drop for ModuleFixture {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).unwrap();
            }
        }

        #[cfg(target_os = "linux")]
        #[test]
        fn lexer_caller_actual_null_real_module_traversal_precedes_outer_allocator_take() {
            let fixture = ModuleFixture::new();
            let entry = fixture.0.join("app.ox").to_str().unwrap().to_owned();
            let expected = module_schedule(
                &entry,
                filesystem::path_units(&fixture.0.canonicalize().unwrap()),
            );
            for association in [
                None,
                Some(Association::Valid),
                Some(Association::Identity),
                Some(Association::File),
                Some(Association::Length),
            ] {
                for ordinal in [5, 6, 19, 20] {
                    let mut allocator = Allocator::default();
                    allocator.observer_trace_bound(20).unwrap();
                    let capacity = allocator.trace.capacity();
                    let action = module_action(&entry, &expected, association, ordinal);
                    let facts = if matches!(ordinal, 5 | 19) {
                        let target = fresh_target(ordinal);
                        let (facts, report) =
                            null::with_selected(&mut allocator, target, action).unwrap();
                        assert_fresh(report, target);
                        facts
                    } else {
                        let target = growth_target(ordinal, 4, 8);
                        let (facts, report) =
                            growth::with_selected_growth(&mut allocator, target, action).unwrap();
                        assert_growth(report, target);
                        facts
                    };
                    println!("caller-null-heap module ordinal={ordinal} source={} current-observed={} prior-tokens={} diagnostic={} old-selected={}", facts.source_heap, facts.observed_heap, facts.prior_token_heap, facts.diagnostic_heap, if matches!(ordinal, 5 | 19) { 0 } else { 4 * size_of::<Token>() });
                    facts.assert();
                    assert_eq!(allocator.trace.capacity(), capacity);
                    assert_eq!(allocator.attempts, ordinal);
                }
            }
        }

        // Named conservative driver roles supplement the observer's generic F/R
        // transport banks. References do not duplicate their referent backing.
        #[allow(dead_code)]
        struct DirectCarriers<'a> {
            allocator: Allocator,
            builder: SourceSetBuilder<'a>,
            provider: Option<MeasuredProvider>,
            provider_borrow: Option<&'a mut dyn LexicalProvider>,
            parse_return: Result<ast::Program, Errors>,
            parse_caller: Result<ast::Program, Errors>,
            diagnostic_borrow: &'a [Diagnostic],
            file: &'a SourceFile,
            expected: [(&'static str, usize, usize); 5],
            facts: Facts,
            diagnostic: bool,
            callbacks: bool,
            invalid: bool,
            trace_capacity: usize,
            trace_requested: usize,
            trace_retained: usize,
        }
        #[allow(dead_code)]
        struct ModuleCarriers<'a> {
            allocator: Allocator,
            builder: SourceSetBuilder<'a>,
            provider: Option<ModuleProvider>,
            provider_borrow: Option<&'a mut dyn LexicalProvider>,
            load_return: Result<(), Errors>,
            load_caller: Result<(), Errors>,
            callbacks: [Option<(SourceFileId, Callback)>; 10],
            expected: [(&'static str, usize, usize); 20],
            fixture: ModuleFixture,
            entry: String,
            canonical_root: PathBuf,
            child: bool,
            diagnostic: bool,
            callback_match: bool,
            invalid: bool,
            facts: Facts,
            file: &'a SourceFile,
            start: usize,
            end: usize,
            trace_capacity: usize,
        }
        #[test]
        fn lexer_caller_null_layout_measurement_only() {
            // This test never invokes either closure or installs either selector.
            let display = String::from("retained-provider.ox");
            let text = String::from(INPUT);
            let source_requested = display.len() + text.len();
            let source_retained = display.capacity() + text.capacity();
            let direct = direct_action(display, text, Some(Association::Valid), 4, 8, 9);
            let expected = module_schedule("/tmp/measurement/app.ox", "/tmp/measurement".len());
            let module = module_action(
                "/tmp/measurement/app.ox",
                &expected,
                Some(Association::Valid),
                20,
            );
            let mut direct_allocator = Allocator::default();
            direct_allocator.observer_trace_bound(5).unwrap();
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(20).unwrap();
            println!("caller-null-bank direct carriers={} align={} closure={} facts={} fresh-selection={} growth-selection={} trace-requested={} trace-retained={} source-capture-requested={} source-capture-retained={}",
            size_of::<DirectCarriers<'_>>(), align_of::<DirectCarriers<'_>>(), size_of_val(&direct), size_of::<Facts>(),
            null::selection_carriers_bytes(&direct), growth::selection_carriers_bytes(&direct),
            5 * size_of::<ReserveEvent>(), direct_allocator.trace.capacity() * size_of::<ReserveEvent>(),
            source_requested, source_retained);
            println!("caller-null-bank module carriers={} align={} closure={} facts={} fresh-selection={} growth-selection={} trace-requested={} trace-retained={}",
            size_of::<ModuleCarriers<'_>>(), align_of::<ModuleCarriers<'_>>(), size_of_val(&module), size_of::<Facts>(),
            null::selection_carriers_bytes(&module), growth::selection_carriers_bytes(&module),
            20 * size_of::<ReserveEvent>(), allocator.trace.capacity() * size_of::<ReserveEvent>());
            println!("caller-null-components provider={} module-provider={} builder={} source-file={} source-map={} reserve-row={} token={} token-align={} diagnostic={} lexer-fixed={} observer-fixed={}",
            size_of::<MeasuredProvider>(), size_of::<ModuleProvider>(), size_of::<SourceSetBuilder<'_>>(), size_of::<SourceFile>(), size_of::<SourceMap>(),
            size_of::<ReserveEvent>(), size_of::<Token>(), align_of::<Token>(), size_of::<Diagnostic>(), lexer::reservation_scratch_bytes() + lexer::reservation_observer_bytes(), growth::fixed_carriers_bytes::<Token>());
            macro_rules! fields {
            ($ty:ty; $($field:ident),+ $(,)?) => {
                $(println!("caller-null-offset {}.{}={}", stringify!($ty), stringify!($field), offset_of!($ty, $field));)+
            };
        }
            fields!(DirectCarriers<'_>; allocator, builder, provider, provider_borrow, parse_return, parse_caller,
            diagnostic_borrow, file, expected, facts, diagnostic, callbacks, invalid, trace_capacity, trace_requested, trace_retained);
            fields!(ModuleCarriers<'_>; allocator, builder, provider, provider_borrow, load_return, load_caller,
            callbacks, expected, fixture, entry, canonical_root, child, diagnostic, callback_match, invalid, facts, file, start, end, trace_capacity);
            fields!(Facts; failed, diagnostic, callbacks, retained, trace, source_heap, observed_heap, diagnostic_heap, prior_token_heap);
            println!(
                "caller-null-align direct-closure={} module-closure={} facts={}",
                align_of_val(&direct),
                align_of_val(&module),
                align_of::<Facts>()
            );
            for (name, driver, fresh, growth) in [
                (
                    "direct",
                    size_of::<DirectCarriers<'_>>(),
                    null::selection_carriers_bytes(&direct),
                    growth::selection_carriers_bytes(&direct),
                ),
                (
                    "module",
                    size_of::<ModuleCarriers<'_>>(),
                    null::selection_carriers_bytes(&module),
                    growth::selection_carriers_bytes(&module),
                ),
            ] {
                let exclusive = driver
                    .checked_add(fresh.max(growth))
                    .and_then(|v| v.checked_add(growth::fixed_carriers_bytes::<Token>()))
                    .and_then(|v| v.checked_add(lexer::reservation_scratch_bytes()))
                    .and_then(|v| v.checked_add(lexer::reservation_observer_bytes()))
                    .unwrap();
                let conservative = exclusive.checked_add(fresh.min(growth)).unwrap();
                println!(
                "caller-null-sum {name} exclusive-controller={} conservative-both-transports={}",
                exclusive, conservative
            );
            }
            assert_eq!(direct_allocator.attempts, 0);
            assert!(direct_allocator.trace.is_empty());
            assert_eq!(allocator.attempts, 0);
            assert!(allocator.trace.is_empty());
        }
    }
    // Insert inside lexer_reservation_caller_tests. Only this module is new.
    // Actual Provider test-capture is distinct from external-process qualification.
    mod concrete_provider_receipts {
        use super::*;
        use crate::frontend::lexical_provider::{
            Provider as ActualProvider, ReservationFixture, ReservationLayout, ReservationReceipt,
        };
        use crate::frontend::project::budget::{real_null_observer as null, ReserveEvent};
        use null::growth::{self, GrowthTarget};
        use std::alloc::Layout;
        use std::mem::{align_of, align_of_val, offset_of, size_of_val};

        const PATH: &str = "retained-provider.ox";
        const EMPTY_HASH: [u8; 32] = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        // Independently specified by source registration -> LXI1 request -> LXS1
        // decode -> canonical schedule. Never populated from an observed trace.
        fn expected() -> [(&'static str, usize, usize); 7] {
            [
                ("line starts", 1, size_of::<usize>()),
                ("source files", 1, size_of::<SourceFile>()),
                ("lexical provider input", 34, 1),
                ("lexical provider tokens", 13, size_of::<Token>()),
                ("lexer token tape", 4, size_of::<Token>()),
                ("lexer token tape", 8, size_of::<Token>()),
                ("lexer token tape", 16, size_of::<Token>()),
            ]
        }
        fn fresh_target() -> null::Target {
            null::Target {
                attempt: 5,
                kind: "lexer token tape",
                slots: 4,
                element_bytes: size_of::<Token>(),
                layout: Layout::array::<Token>(4).unwrap(),
            }
        }
        fn growth_target(ordinal: usize) -> GrowthTarget {
            let (old, new) = match ordinal {
                6 => (4, 8),
                7 => (8, 16),
                _ => panic!("closed target"),
            };
            GrowthTarget {
                attempt: ordinal,
                kind: "lexer token tape",
                old_len: old,
                old_capacity: old,
                additional: new - old,
                new_slots: new,
                element_bytes: size_of::<Token>(),
                element_align: align_of::<Token>(),
                old_layout: Layout::array::<Token>(old).unwrap(),
                new_layout: Layout::array::<Token>(new).unwrap(),
                operation: null::Operation::Realloc,
            }
        }

        struct MeasuredProvider {
            inner: ActualProvider,
            fixture: ReservationFixture,
            association: Association,
            before: Option<ReservationReceipt>,
            callbacks: [Option<Callback>; 5],
            count: usize,
            attempts_after_observe: usize,
            observation_heap: usize,
        }
        impl MeasuredProvider {
            fn new(fixture: ReservationFixture, association: Association) -> Self {
                Self {
                    inner: ActualProvider::reservation_fixture(fixture),
                    fixture,
                    association,
                    before: None,
                    callbacks: [None; 5],
                    count: 0,
                    attempts_after_observe: 0,
                    observation_heap: 0,
                }
            }
            fn record(&mut self, event: Callback) {
                self.callbacks[self.count] = Some(event);
                self.count += 1;
            }
        }
        fn diagnostic_heap(errors: &[Diagnostic], capacity: usize) -> usize {
            errors
                .iter()
                .fold(capacity * size_of::<Diagnostic>(), |n, e| {
                    n + e.message.capacity()
                        + e.secondary.capacity() * size_of::<(Span, String)>()
                        + e.secondary
                            .iter()
                            .map(|(_, text)| text.capacity())
                            .sum::<usize>()
                        + e.notes.capacity() * size_of::<String>()
                        + e.notes.iter().map(String::capacity).sum::<usize>()
                })
        }
        impl LexicalProvider for MeasuredProvider {
            fn begin_module(
                &mut self,
                source: &SourceFile,
                usage: SourceUsage,
            ) -> Result<(), Box<Diagnostic>> {
                self.record(Callback::Begin);
                self.inner.begin_module(source, usage)
            }
            fn observe(
                &mut self,
                source: &SourceFile,
                limit: usize,
                inventory: &Inventory,
                allocator: &mut Allocator,
            ) -> Result<LexicalObservation, Box<Diagnostic>> {
                self.record(Callback::Observe);
                let mut observed = self.inner.observe(source, limit, inventory, allocator)?;
                self.before = self.inner.reservation_receipt();
                self.attempts_after_observe = allocator.attempts;
                self.observation_heap = match &observed.value {
                    Ok(tokens) => tokens.capacity() * size_of::<Token>(),
                    Err(error) => diagnostic_heap(std::slice::from_ref(error.as_ref()), 1),
                };
                // Only the carrier association is forged. Actual Provider state,
                // source, wire framing, decoded tokens and receipt are untouched.
                match self.association {
                    Association::Valid => {}
                    Association::Identity => observed.identity = source.identity().wrapping_add(1),
                    Association::File => observed.file = SourceFileId(1),
                    Association::Length => observed.source_len += 1,
                }
                Ok(observed)
            }
            fn comparison_started(&mut self) {
                self.record(Callback::ComparisonStarted);
                self.inner.comparison_started();
            }
            fn comparison_finished(&mut self, matched: bool) {
                self.record(Callback::ComparisonFinished(matched));
                self.inner.comparison_finished(matched);
            }
            fn selected_for_parser(
                &mut self,
                source: &SourceFile,
                tokens: &[Token],
                capacity: usize,
            ) -> Result<(), Box<Diagnostic>> {
                self.record(Callback::Selected);
                self.inner.selected_for_parser(source, tokens, capacity)
            }
        }
        fn capture_ok(
            receipt: ReservationReceipt,
            fixture: ReservationFixture,
            source: &SourceFile,
        ) -> bool {
            receipt.file == SourceFileId(0)
                && receipt.identity == source.identity()
                && receipt.source_len == fixture.source().len()
                && receipt.source_sha256 == fixture.source_sha256()
                && receipt.executable_sha256 == [0; 32]
                && receipt.input_sha256 == fixture.input_sha256()
                && receipt.stdout_sha256 == fixture.stdout_sha256()
                && receipt.stderr_sha256 == EMPTY_HASH
                && receipt.input_bytes == fixture.input_bytes()
                && receipt.input_written == fixture.input_bytes()
                && receipt.stdout_bytes == fixture.stdout_bytes()
                && receipt.stderr_bytes == 0
                && receipt.status.is_none()
                && receipt.signal.is_none()
                && receipt.stop == "test-observation"
                && !receipt.spawned
                && !receipt.leader_reaped
                && !receipt.stdin_closed
                && !receipt.stdout_eof
                && !receipt.stderr_eof
                && !receipt.comparison_attempted
                && !receipt.comparison_matched
                && !receipt.selected_for_parser
        }
        #[derive(Clone, Copy, Debug)]
        struct Facts {
            failed: bool,
            diagnostic: bool,
            before: Option<ReservationReceipt>,
            after: Option<ReservationReceipt>,
            capture: bool,
            receipt_preserved: bool,
            callbacks: bool,
            retained: bool,
            trace: bool,
            source_heap: usize,
            observation_heap: usize,
            diagnostic_heap: usize,
            receipt_heap: usize,
        }
        impl Facts {
            fn assert(self) {
                assert!(
                    self.failed
                        && self.diagnostic
                        && self.capture
                        && self.receipt_preserved
                        && self.callbacks
                        && self.retained
                        && self.trace,
                    "{self:?}"
                );
                let before = self.before.unwrap();
                let after = self.after.unwrap();
                assert!(!before.comparison_attempted && after.comparison_attempted);
                assert!(!after.selected_for_parser);
            }
        }
        // This exact factory is used by selected execution AND pure F/R sizing.
        // Provider receipts and source Strings are prepaid before selection. They
        // move into the real caller and are ordinarily dropped before Facts escapes.
        fn actual_action(
            display: String,
            text: String,
            mut provider: MeasuredProvider,
            ordinal: usize,
            start: usize,
            end: usize,
            content: bool,
        ) -> impl for<'a> FnOnce(&'a mut Allocator) -> Facts {
            move |allocator| {
                let mut build = builder(allocator);
                let result = build.parse_file_with_provider(
                    display,
                    text,
                    None,
                    &mut Some(&mut provider as &mut dyn LexicalProvider),
                );
                let source = build.project.sources.get(SourceFileId(0));
                let invalid = !matches!(provider.association, Association::Valid);
                let diagnostic = result.as_ref().err().is_some_and(|errors| {
                    let Some(error) = errors.first() else {
                        return false;
                    };
                    errors.len() == 1
                        && error.code
                            == if invalid {
                                "E0703"
                            } else if content {
                                "E0100"
                            } else {
                                "E0400"
                            }
                        && error.stage == if invalid { "lexical-provider" } else { "lex" }
                        && error.message
                            == if invalid {
                                "lexical observation belongs to another retained source"
                            } else if content {
                                "unterminated block comment"
                            } else {
                                "token storage allocation failed"
                            }
                        && error.primary
                            == Some(if invalid {
                                source.span(0, 0)
                            } else {
                                source.span(start, end)
                            })
                        && error.secondary.is_empty()
                        && error.notes.is_empty()
                });
                let before = provider.before;
                let after = provider.inner.reservation_receipt();
                let capture = before.is_some_and(|r| capture_ok(r, provider.fixture, source));
                let receipt_preserved = before.zip(after).is_some_and(|(mut before, after)| {
                    before.comparison_attempted = true;
                    before.comparison_matched = content && !invalid;
                    before == after
                }) && provider.inner.reservation_receipt_count() == 1;
                let callbacks = provider.count == 4
                    && provider.callbacks
                        == [
                            Some(Callback::Begin),
                            Some(Callback::Observe),
                            Some(Callback::ComparisonStarted),
                            Some(Callback::ComparisonFinished(content && !invalid)),
                            None,
                        ]
                    && provider.attempts_after_observe == if content { 3 } else { 4 };
                let schedule = expected();
                let diagnostic_schedule = [
                    ("line starts", 1, size_of::<usize>()),
                    ("source files", 1, size_of::<SourceFile>()),
                    ("lexical provider input", 15, 1),
                    ("lexer token tape", 4, size_of::<Token>()),
                ];
                let prefix = if content {
                    diagnostic_schedule.as_slice()
                } else {
                    &schedule[..ordinal]
                };
                let trace = build.allocator.attempts == ordinal
                    && build.allocator.trace.len() == ordinal
                    && !build.allocator.observer_trace_overflow
                    && build.allocator.trace.iter().zip(prefix).enumerate().all(
                        |(i, (row, expected))| {
                            (row.kind, row.length, row.element_bytes) == *expected
                                && row.success == (content || i + 1 != ordinal)
                        },
                    );
                let facts = Facts {
                    failed: result.is_err(),
                    diagnostic,
                    before,
                    after,
                    capture,
                    receipt_preserved,
                    callbacks,
                    retained: build.project.sources.files().len() == 1
                        && source.path() == PATH
                        && source.text() == provider.fixture.source()
                        && build.project.usage.non_eof_tokens == 0
                        && build.project.usage.syntax_nodes == 0
                        && build.project.usage.line_starts == 1
                        && build.project.programs.is_empty()
                        && build.project.modules.is_empty(),
                    trace,
                    source_heap: build.project.sources.heap_capacity_bytes().unwrap(),
                    observation_heap: provider.observation_heap,
                    diagnostic_heap: result
                        .as_ref()
                        .err()
                        .map_or(0, |errors| diagnostic_heap(errors, errors.capacity())),
                    receipt_heap: provider.inner.reservation_layout().receipts_retained,
                };
                drop(result);
                drop(provider);
                drop(build);
                facts
            }
        }
        fn token_action(
            association: Association,
            ordinal: usize,
        ) -> impl for<'a> FnOnce(&'a mut Allocator) -> Facts {
            let (start, end) = match ordinal {
                5 => (0, 2),
                6 => (8, 9),
                7 => (13, 14),
                _ => panic!("closed site"),
            };
            actual_action(
                PATH.into(),
                INPUT.into(),
                MeasuredProvider::new(ReservationFixture::Tokens, association),
                ordinal,
                start,
                end,
                false,
            )
        }
        fn fresh_ok(report: null::Report) {
            let target = fresh_target();
            assert_eq!(report.target, target);
            assert!(report.selected && report.matched && report.fired);
            assert_eq!(report.rejection, None);
            assert_eq!(
                report.actual,
                Some(null::GlobalEvent {
                    operation: null::Operation::Alloc,
                    layout: target.layout,
                    new_size: None
                })
            );
        }
        fn growth_ok(report: growth::GrowthReport, target: GrowthTarget) {
            assert_eq!(report.target, target);
            assert!(report.selected && report.matched && report.fired);
            assert_eq!(report.rejection, None);
            assert_eq!(
                report.actual,
                Some(growth::GrowthEvent {
                    operation: null::Operation::Realloc,
                    layout: target.old_layout,
                    new_size: Some(target.new_layout.size()),
                    old_address_matches: true
                })
            );
            assert_eq!(report.reserve_failed, Some(true));
            assert!(
                report.owner_unchanged
                    && report.address_unchanged
                    && report.length_unchanged
                    && report.capacity_unchanged
                    && report.trace_preserved
            );
            assert_eq!(report.drop_count, 1);
            assert_eq!(
                report.drop_event,
                Some(growth::GrowthDropEvent {
                    layout: target.old_layout,
                    old_address_matches: true,
                    after_reserve_return: true
                })
            );
        }
        #[test]
        fn lexer_concrete_provider_capacity_all_sites_and_association_precedence() {
            for association in [
                Association::Valid,
                Association::Identity,
                Association::File,
                Association::Length,
            ] {
                for ordinal in 5..=7 {
                    let mut allocator = Allocator {
                        fail_at: Some(ordinal),
                        ..Allocator::default()
                    };
                    allocator.observer_trace_bound(7).unwrap();
                    token_action(association, ordinal)(&mut allocator).assert();
                    assert_eq!(allocator.attempts, ordinal);
                }
            }
        }
        #[test]
        fn lexer_concrete_provider_equal_content_diagnostic_closes_true() {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(4).unwrap();
            let action = actual_action(
                PATH.into(),
                ReservationFixture::UnterminatedComment.source().into(),
                MeasuredProvider::new(ReservationFixture::UnterminatedComment, Association::Valid),
                4,
                1,
                3,
                true,
            );
            let facts = action(&mut allocator);
            facts.assert();
            assert!(facts.after.unwrap().comparison_matched);
            assert_eq!(allocator.attempts, 4);
        }
        #[test]
        fn lexer_concrete_provider_actual_null_all_sites_and_association_precedence() {
            for association in [
                Association::Valid,
                Association::Identity,
                Association::File,
                Association::Length,
            ] {
                for ordinal in 5..=7 {
                    let mut allocator = Allocator::default();
                    allocator.observer_trace_bound(7).unwrap();
                    let action = token_action(association, ordinal);
                    let bank = control_bank(&action).unwrap();
                    assert_eq!(admit_control(&action, bank), Some(bank));
                    // Fresh selection has no trace prepayment check, so the closed
                    // driver checks it explicitly before either family is installed.
                    assert_eq!(allocator.observer_trace_limit, Some(7));
                    assert!(allocator.trace.capacity() >= 7 && allocator.trace.is_empty());
                    assert!(!allocator.observer_trace_overflow && allocator.attempts == 0);
                    let facts = if ordinal == 5 {
                        let (facts, report) =
                            null::with_selected(&mut allocator, fresh_target(), action).unwrap();
                        fresh_ok(report);
                        facts
                    } else {
                        let target = growth_target(ordinal);
                        let (facts, report) =
                            growth::with_selected_growth(&mut allocator, target, action).unwrap();
                        growth_ok(report, target);
                        facts
                    };
                    facts.assert();
                    assert!(!facts.after.unwrap().comparison_matched);
                    println!("concrete-provider-null ordinal={ordinal} association={association:?} before={:?} after={:?} source-retained={} observation-retained={} diagnostic-retained={} receipt-retained={} trace-retained={}",
                    facts.before, facts.after, facts.source_heap, facts.observation_heap, facts.diagnostic_heap,
                    facts.receipt_heap, allocator.trace.capacity() * size_of::<ReserveEvent>());
                }
            }
        }

        struct OuterFile(std::path::PathBuf);
        impl OuterFile {
            fn new() -> Self {
                static NEXT: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(0);
                let directory = std::env::temp_dir().join(format!(
                    "oxid-concrete-provider-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ));
                std::fs::create_dir(&directory).unwrap();
                let path = directory.join("main.ox");
                std::fs::write(&path, INPUT).unwrap();
                Self(path)
            }
        }
        impl Drop for OuterFile {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
                let _ = std::fs::remove_dir(self.0.parent().unwrap());
            }
        }
        #[test]
        fn lexer_concrete_provider_outer_loader_capacity_keeps_receipt_and_transferred_trace() {
            // Distinct capacity/public-error adaptation layer. No real-null selector
            // is installed: this outer route legitimately takes the allocator.
            let file = OuterFile::new();
            let entry = file.0.to_str().unwrap();
            for association in [
                Association::Valid,
                Association::Identity,
                Association::File,
                Association::Length,
            ] {
                for ordinal in 7..=9 {
                    let mut allocator = Allocator {
                        fail_at: Some(ordinal),
                        ..Allocator::default()
                    };
                    allocator.observer_trace_bound(9).unwrap();
                    let mut provider =
                        MeasuredProvider::new(ReservationFixture::Tokens, association);
                    let failure = ProjectSources::load_with_syntax_provider(
                        entry,
                        ProjectLimits::default(),
                        parser::SourceMode::OwnedCandidate,
                        &mut allocator,
                        parser::ArraySyntaxPolicy::Closed,
                        ProjectEnumSyntax::Closed,
                        Some(&mut provider),
                    )
                    .unwrap_err();
                    let source = failure.sources.get(SourceFileId(0));
                    let invalid = !matches!(association, Association::Valid);
                    let (start, end) = [(0, 2), (8, 9), (13, 14)][ordinal - 7];
                    assert_error(
                        &failure.diagnostics,
                        if invalid { "E0703" } else { "E0400" },
                        if invalid { "lexical-provider" } else { "lex" },
                        if invalid {
                            "lexical observation belongs to another retained source"
                        } else {
                            "token storage allocation failed"
                        },
                        Some(if invalid {
                            source.span(0, 0)
                        } else {
                            source.span(start, end)
                        }),
                    );
                    let before = provider.before.unwrap();
                    assert!(capture_ok(before, ReservationFixture::Tokens, source));
                    let mut expected_receipt = before;
                    expected_receipt.comparison_attempted = true;
                    assert_eq!(provider.inner.reservation_receipt(), Some(expected_receipt));
                    assert_eq!(provider.inner.reservation_receipt_count(), 1);
                    assert_eq!(provider.attempts_after_observe, 6);
                    assert_eq!(provider.count, 4);
                    assert_eq!(
                        provider.callbacks,
                        [
                            Some(Callback::Begin),
                            Some(Callback::Observe),
                            Some(Callback::ComparisonStarted),
                            Some(Callback::ComparisonFinished(false)),
                            None
                        ]
                    );
                    assert_eq!(failure.sources.files().len(), 1);
                    assert_eq!(source.text(), INPUT);
                    assert_eq!(source.path(), entry);
                    assert_eq!(failure.usage.source_bytes, 22);
                    assert_eq!(failure.usage.retained_path_bytes, entry.len());
                    assert_eq!(failure.usage.line_starts, 1);
                    assert_eq!(failure.usage.non_eof_tokens, 0);
                    assert_eq!(failure.usage.syntax_nodes, 0);
                    assert_eq!(failure.usage.modules, 0);
                    let expected = [
                        ("source bytes", 22, 1),
                        ("entry display", entry.len(), 1),
                        ("line starts", 1, size_of::<usize>()),
                        ("source files", 1, size_of::<SourceFile>()),
                        ("lexical provider input", 34, 1),
                        ("lexical provider tokens", 13, size_of::<Token>()),
                        ("lexer token tape", 4, size_of::<Token>()),
                        ("lexer token tape", 8, size_of::<Token>()),
                        ("lexer token tape", 16, size_of::<Token>()),
                    ];
                    assert_eq!(failure.allocator.attempts, ordinal);
                    assert_eq!(failure.allocator.trace.len(), ordinal);
                    assert_eq!(failure.allocator.observer_trace_limit, Some(9));
                    assert!(!failure.allocator.observer_trace_overflow);
                    for (index, row) in failure.allocator.trace.iter().enumerate() {
                        assert_eq!((row.kind, row.length, row.element_bytes), expected[index]);
                        assert_eq!(row.success, index + 1 != ordinal);
                    }
                    assert_eq!(allocator.attempts, 0);
                    assert!(allocator.trace.is_empty());
                }
            }
        }

        // Conservative named driver carriers. F and R transports are measured by
        // the SAME closure factory above and the existing generic selector banks.
        // Owned backings are separate; references/headers do not duplicate payload.
        #[allow(dead_code)]
        struct Carriers<'a> {
            allocator: Allocator,
            builder: SourceSetBuilder<'a>,
            provider: MeasuredProvider,
            provider_borrow: Option<&'a mut dyn LexicalProvider>,
            observation_return: Result<LexicalObservation, Box<Diagnostic>>,
            observation_caller: Result<LexicalObservation, Box<Diagnostic>>,
            parse_return: Result<ast::Program, Errors>,
            parse_caller: Result<ast::Program, Errors>,
            source: &'a SourceFile,
            inventory: Inventory,
            usage: SourceUsage,
            snapshot_return: Option<ReservationReceipt>,
            snapshot_caller: Option<ReservationReceipt>,
            before: Option<ReservationReceipt>,
            after: Option<ReservationReceipt>,
            normalized: ReservationReceipt,
            snapshot_zip: Option<(ReservationReceipt, ReservationReceipt)>,
            snapshot_mapping: ReservationReceipt,
            fixture: ReservationFixture,
            association: Association,
            layout_return: ReservationLayout,
            layout_caller: ReservationLayout,
            expected: [(&'static str, usize, usize); 7],
            diagnostic_expected: [(&'static str, usize, usize); 4],
            prefix: &'a [(&'static str, usize, usize)],
            facts: Facts,
            predicate_bools: [bool; 16],
            scalar_locals: [usize; 16],
            checked_sum: Option<usize>,
            admission_return: Option<usize>,
            admission_caller: Option<usize>,
            sizing_borrow: &'a (),
            requested_wire_header: Vec<u8>,
            captured_wire_header: Vec<u8>,
        }
        fn control_bank<F>(action: &F) -> Option<usize>
        where
            F: for<'a> FnOnce(&'a mut Allocator) -> Facts,
        {
            [
                size_of::<Carriers<'_>>(),
                null::selection_carriers_bytes(action)
                    .max(growth::selection_carriers_bytes(action)),
                growth::fixed_carriers_bytes::<Token>(),
                ActualProvider::reservation_scratch_bytes(),
                lexer::reservation_scratch_bytes(),
                lexer::reservation_observer_bytes(),
            ]
            .into_iter()
            .try_fold(0usize, usize::checked_add)
        }
        fn admit_control<F>(action: &F, limit: usize) -> Option<usize>
        where
            F: for<'a> FnOnce(&'a mut Allocator) -> Facts,
        {
            control_bank(action).filter(|&required| required <= limit)
        }
        #[test]
        fn lexer_concrete_provider_layout_measurement_only() {
            // No observe/decoder/lexer/parser/selector invocation. Actual factory
            // setup is permitted and retains prepaid receipts/source Strings only.
            let provider = MeasuredProvider::new(ReservationFixture::Tokens, Association::Valid);
            let layout = provider.inner.reservation_layout();
            let display = String::from(PATH);
            let text = String::from(INPUT);
            let source_captured = display.capacity() + text.capacity();
            let action = actual_action(display, text, provider, 6, 8, 9, false);
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(7).unwrap();
            let bank = control_bank(&action).unwrap();
            println!("concrete-provider-driver carriers={}/{} closure={}/{} facts={}/{} measured-provider={}/{} fresh-selector={} growth-selector={} exclusive-named-bank={} conservative-both={} provider={layout:?}",
            size_of::<Carriers<'_>>(), align_of::<Carriers<'_>>(), size_of_val(&action), align_of_val(&action),
            size_of::<Facts>(), align_of::<Facts>(), size_of::<MeasuredProvider>(), align_of::<MeasuredProvider>(),
            null::selection_carriers_bytes(&action), growth::selection_carriers_bytes(&action), bank,
            bank.checked_add(null::selection_carriers_bytes(&action).min(growth::selection_carriers_bytes(&action))).unwrap());
            println!("concrete-provider-payload trace-requested={} trace-retained={} source-captured={} receipts-requested={} receipts-retained={} input-requested=34 stdout-requested=249 decoded-requested={} canonical-requested-first={} canonical-requested-growth={} observer-added-dynamic=0",
            7 * size_of::<ReserveEvent>(), allocator.trace.capacity() * size_of::<ReserveEvent>(), source_captured,
            layout.receipts_requested, layout.receipts_retained, 13 * size_of::<Token>(),
            4 * size_of::<Token>(), (4 + 8) * size_of::<Token>());
            macro_rules! fields {
            ($ty:ty; $($field:ident),+ $(,)?) => {
                $(println!("concrete-provider-driver-field {}.{}={}", stringify!($ty), stringify!($field), offset_of!($ty, $field));)+
            };
        }
            fields!(MeasuredProvider; inner, fixture, association, before, callbacks, count, attempts_after_observe, observation_heap);
            fields!(Facts; failed, diagnostic, before, after, capture, receipt_preserved, callbacks, retained, trace, source_heap, observation_heap, diagnostic_heap, receipt_heap);
            fields!(Carriers<'_>; allocator, builder, provider, provider_borrow, observation_return, observation_caller,
            parse_return, parse_caller, source, inventory, usage, snapshot_return, snapshot_caller, before, after,
            normalized, snapshot_zip, snapshot_mapping, fixture, association, layout_return, layout_caller,
            expected, diagnostic_expected, prefix, facts, predicate_bools, scalar_locals, checked_sum,
            admission_return, admission_caller, sizing_borrow, requested_wire_header, captured_wire_header);
            assert_eq!(allocator.attempts, 0);
            assert!(allocator.trace.is_empty());
        }
        #[test]
        fn lexer_concrete_provider_control_bank_exact_and_one_short_without_action() {
            let action = token_action(Association::Valid, 6);
            let bank = control_bank(&action).unwrap();
            assert!(bank > 0);
            assert_eq!(admit_control(&action, bank), Some(bank));
            assert_eq!(admit_control(&action, bank - 1), None);
            // Merely borrowing the action for sizing/admission cannot invoke it.
            // No selector, allocator request, wire decode or parser is called here.
            drop(action);
        }
    }
    // Fixture bytes, token spans, credit arithmetic and every request ordinal were
    // frozen independently before these tests: three-module-credit evidence.
    #[cfg(target_os = "linux")]
    mod three_module_credit {
        use super::*;

        const ROOT: &str = "mod a;mod b;";
        const ROOT_TOKENS: &[(Kind, usize, usize)] = &[
            (Kind::Mod, 0, 3),
            (Kind::Trivia, 3, 4),
            (Kind::Ident, 4, 5),
            (Kind::Semi, 5, 6),
            (Kind::Mod, 6, 9),
            (Kind::Trivia, 9, 10),
            (Kind::Ident, 10, 11),
            (Kind::Semi, 11, 12),
            (Kind::Eof, 12, 12),
        ];
        const SPACE_TOKENS: &[(Kind, usize, usize)] = &[(Kind::Trivia, 0, 1), (Kind::Eof, 1, 1)];
        const EMPTY_TOKENS: &[(Kind, usize, usize)] = &[(Kind::Eof, 0, 0)];

        struct Fixture(PathBuf);
        impl Fixture {
            fn new(last: &str) -> Self {
                use std::sync::atomic::{AtomicUsize, Ordering};
                static NEXT: AtomicUsize = AtomicUsize::new(0);
                let directory = std::env::temp_dir().join(format!(
                    "oxid-lexer-three-module-credit-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                std::fs::create_dir(&directory).unwrap();
                std::fs::write(directory.join("app.ox"), ROOT).unwrap();
                std::fs::write(directory.join("a.ox"), ZERO_CREDIT).unwrap();
                std::fs::write(directory.join("b.ox"), last).unwrap();
                Self(directory)
            }
            fn display(&self, name: &str) -> String {
                self.0.join(name).to_str().unwrap().to_owned()
            }
            fn canonical(&self, name: &str) -> PathBuf {
                self.0.join(name).canonicalize().unwrap()
            }
            fn usage_before(&self, file: usize, last: &str) -> SourceUsage {
                let entry = self.display("app.ox").len();
                let root = filesystem::path_units(&self.0.canonicalize().unwrap());
                let canonical_entry = filesystem::path_units(&self.canonical("app.ox"));
                let child = 4
                    + filesystem::path_units(&self.canonical("a.ox"))
                    + self.display("a.ox").len();
                SourceUsage {
                    source_bytes: [12, 13, 13 + last.len()][file],
                    non_eof_tokens: [0, 8, 9][file],
                    syntax_nodes: [0, 2, 2][file],
                    line_starts: file + 1,
                    modules: file,
                    retained_path_bytes: if file == 0 {
                        entry
                    } else {
                        entry + root + canonical_entry + file * child
                    },
                    probes: file,
                    directory_entries: 3 * file,
                    directory_name_units: 14 * file,
                }
            }
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).unwrap();
            }
        }

        // Reuse the existing literal Provider's bounded callback, selected-tape
        // and equal-diagnostic ownership checks. Only the two preceding module
        // observations need new literals; the zero-credit final observations use
        // its existing empty/space fixtures without invoking canonical lexing.
        struct CreditProvider {
            modules: [Provider; 3],
            current: usize,
            limits: [Option<usize>; 3],
            usages: [Option<SourceUsage>; 3],
            owners: [Option<(u64, usize)>; 3],
        }
        impl CreditProvider {
            fn new() -> Self {
                Self {
                    modules: std::array::from_fn(|_| {
                        Provider::new(Association::Valid, Mutation::None)
                    }),
                    current: 0,
                    limits: [None; 3],
                    usages: [None; 3],
                    owners: [None; 3],
                }
            }
        }
        impl LexicalProvider for CreditProvider {
            fn begin_module(
                &mut self,
                source: &SourceFile,
                usage: SourceUsage,
            ) -> Result<(), Box<Diagnostic>> {
                self.current = source.span(0, 0).file.0;
                assert!(self.usages[self.current].is_none());
                self.usages[self.current] = Some(usage);
                self.owners[self.current] =
                    Some((source.identity(), source.text().as_ptr() as usize));
                self.modules[self.current].begin_module(source, usage)
            }
            fn observe(
                &mut self,
                source: &SourceFile,
                limit: usize,
                inventory: &Inventory,
                allocator: &mut Allocator,
            ) -> Result<LexicalObservation, Box<Diagnostic>> {
                self.limits[self.current] = Some(limit);
                let provider = &mut self.modules[self.current];
                if self.current == 2 {
                    return provider.observe(source, limit, inventory, allocator);
                }
                provider.record(Callback::Observe);
                provider.allocator_address = allocator as *const Allocator as usize;
                let spans = if self.current == 0 {
                    assert_eq!(source.text(), ROOT);
                    ROOT_TOKENS
                } else {
                    assert_eq!(source.text(), ZERO_CREDIT);
                    SPACE_TOKENS
                };
                let tokens: Vec<_> = spans
                    .iter()
                    .map(|&(kind, start, end)| Token {
                        kind,
                        span: source.span(start, end),
                    })
                    .collect();
                provider.observed_token_address = tokens.as_ptr() as usize;
                provider.completed_observations += 1;
                provider.attempts_after_observe = allocator.attempts;
                Ok(LexicalObservation::new(source, Ok(tokens)))
            }
            fn comparison_started(&mut self) {
                self.modules[self.current].comparison_started();
            }
            fn comparison_finished(&mut self, matched: bool) {
                self.modules[self.current].comparison_finished(matched);
            }
            fn selected_for_parser(
                &mut self,
                source: &SourceFile,
                tokens: &[Token],
                capacity: usize,
            ) -> Result<(), Box<Diagnostic>> {
                self.modules[self.current].selected_for_parser(source, tokens, capacity)
            }
        }

        fn schedule(fixture: &Fixture, last: &str) -> [(&'static str, usize, usize); 30] {
            let entry = fixture.display("app.ox").len();
            let display = fixture.display("a.ox").len();
            let probe = filesystem::path_units(&fixture.0.canonicalize().unwrap()) + 5;
            let mut expected = [
                ("source bytes", 12, 1),
                ("entry display", entry, 1),
                ("line starts", 1, size_of::<usize>()),
                ("source files", 1, size_of::<SourceFile>()),
                ("lexer token tape", 4, size_of::<Token>()),
                ("lexer token tape", 8, size_of::<Token>()),
                ("lexer token tape", 16, size_of::<Token>()),
                ("module declarations", 1, size_of::<ast::ModuleDecl>()),
                ("module items", 1, size_of::<ast::ItemId>()),
                ("module declarations", 2, size_of::<ast::ModuleDecl>()),
                ("module items", 2, size_of::<ast::ItemId>()),
                ("file ASTs", 1, size_of::<ast::Program>()),
                ("module headers", 1, size_of::<ModuleHeader>()),
                ("logical module path", 4, 1),
                ("module display", display, 1),
                ("module probe path", probe, 1),
                ("file ASTs", 2, size_of::<ast::Program>()),
                ("module headers", 2, size_of::<ModuleHeader>()),
                ("source bytes", 1, 1),
                ("line starts", 1, size_of::<usize>()),
                ("source files", 2, size_of::<SourceFile>()),
                ("lexer token tape", 4, size_of::<Token>()),
                ("logical module path", 4, 1),
                ("module display", display, 1),
                ("module probe path", probe, 1),
                ("file ASTs", 3, size_of::<ast::Program>()),
                ("module headers", 3, size_of::<ModuleHeader>()),
                ("line starts", 1, size_of::<usize>()),
                ("source files", 3, size_of::<SourceFile>()),
                ("lexer token tape", 4, size_of::<Token>()),
            ];
            if !last.is_empty() {
                expected[27..].copy_from_slice(&[
                    ("source bytes", 1, 1),
                    ("line starts", 1, size_of::<usize>()),
                    ("source files", 3, size_of::<SourceFile>()),
                ]);
            }
            expected
        }

        fn check(last: &str, reject_eof_reserve: bool, provider_route: bool) {
            assert!(last.is_empty() || last == ZERO_CREDIT);
            assert!(!reject_eof_reserve || last.is_empty());
            let success = last.is_empty() && !reject_eof_reserve;
            let fixture = Fixture::new(last);
            let entry = fixture.display("app.ox");
            let expected = schedule(&fixture, last);
            let mut allocator = Allocator {
                fail_at: Some(if reject_eof_reserve { 30 } else { 31 }),
                ..Allocator::default()
            };
            allocator.observer_trace_bound(30).unwrap();
            let trace_capacity = allocator.trace.capacity();
            let mut provider = provider_route.then(CreditProvider::new);
            let mut build = builder(&mut allocator);
            build.entry = &entry;
            build.mode = parser::SourceMode::ModuleCandidate;
            build.limits.tokens = 9;
            let result = build.load_all_with_provider(
                &mut provider.as_mut().map(|p| p as &mut dyn LexicalProvider),
            );
            if success {
                assert!(result.is_ok(), "{result:?}");
            } else {
                let errors = result.as_ref().unwrap_err();
                assert_error(
                    errors,
                    "E0400",
                    "lex",
                    if reject_eof_reserve {
                        "token storage allocation failed"
                    } else {
                        "token resource limit exceeded"
                    },
                    Some(Span {
                        file: SourceFileId(2),
                        start: 0,
                        end: usize::from(!last.is_empty()),
                    }),
                );
                if !last.is_empty() {
                    if let Some(provider) = &provider {
                        assert_eq!(
                            errors[0].message.as_ptr() as usize,
                            provider.modules[2].observed_message_address
                        );
                    }
                }
            }
            // Both schedules end at ordinal 30, but only empty b requests token
            // storage there. The one-space b refuses before its first reserve.
            assert_eq!(build.allocator.attempts, 30);
            assert_eq!(build.allocator.trace.len(), 30);
            assert_eq!(build.allocator.trace.capacity(), trace_capacity);
            assert!(!build.allocator.observer_trace_overflow);
            for (index, (event, expected)) in build.allocator.trace.iter().zip(expected).enumerate()
            {
                assert_eq!((event.kind, event.length, event.element_bytes), expected);
                assert_eq!(event.success, !(reject_eof_reserve && index == 29));
            }
            let mut usage = fixture.usage_before(2, last);
            usage.modules = if success { 3 } else { 2 };
            assert_eq!(build.project.usage, usage);
            assert_eq!(build.project.sources.files().len(), 3);
            assert_eq!(build.project.programs.len(), usage.modules);
            assert_eq!(build.project.modules.len(), usage.modules);
            assert_eq!(build.project.syntax_flavor, SyntaxFlavor::ProjectSyntax);
            assert_eq!(
                build.project.canonical_root,
                Some(fixture.0.canonicalize().unwrap())
            );
            let files = ["app.ox", "a.ox", "b.ox"];
            let texts = [ROOT, ZERO_CREDIT, last];
            for (index, source) in build.project.sources.files().iter().enumerate() {
                assert_eq!(source.path(), fixture.display(files[index]));
                assert_eq!(source.text(), texts[index]);
                assert_eq!(source.line_count(), 1);
                assert_eq!(source.span(0, 0).file, SourceFileId(index));
                for earlier in &build.project.sources.files()[..index] {
                    assert_ne!(source.identity(), earlier.identity());
                }
                if let Some(provider) = &provider {
                    assert_eq!(
                        provider.owners[index],
                        Some((source.identity(), source.text().as_ptr() as usize))
                    );
                    assert_eq!(
                        provider.usages[index],
                        Some(fixture.usage_before(index, last))
                    );
                }
            }
            let expected_tokens = [ROOT_TOKENS, SPACE_TOKENS, EMPTY_TOKENS];
            for (index, program) in build.project.programs.iter().enumerate() {
                let source = build.project.sources.get(SourceFileId(index));
                assert!(program.belongs_to(source));
                for (other, file) in build.project.sources.files().iter().enumerate() {
                    assert_eq!(program.belongs_to(file), other == index);
                }
                assert_eq!(program.tokens.len(), expected_tokens[index].len());
                for (token, &(kind, start, end)) in
                    program.tokens.iter().zip(expected_tokens[index])
                {
                    assert_eq!(token.kind, kind);
                    assert_eq!(token.span, source.span(start, end));
                }
                assert_eq!(program.modules.len(), if index == 0 { 2 } else { 0 });
                let header = &build.project.modules[index];
                assert_eq!(header.file, SourceFileId(index));
                assert_eq!(header.parent, (index != 0).then_some(ModuleId(0)));
                assert_eq!(header.depth, usize::from(index != 0));
                assert_eq!(header.public, None);
                assert_eq!(header.canonical_path, Some(fixture.canonical(files[index])));
                assert_eq!(
                    header.relative_path,
                    if index == 0 { "" } else { files[index] }
                );
                assert_eq!(
                    header.declaration,
                    match index {
                        0 => None,
                        1 => Some(Span {
                            file: SourceFileId(0),
                            start: 4,
                            end: 5
                        }),
                        2 => Some(Span {
                            file: SourceFileId(0),
                            start: 10,
                            end: 11
                        }),
                        _ => unreachable!(),
                    }
                );
                if let Some(provider) = &provider {
                    assert_eq!(
                        program.tokens.as_ptr() as usize,
                        provider.modules[index].observed_token_address
                    );
                }
            }
            assert_eq!(
                build
                    .project
                    .programs
                    .iter()
                    .map(|p| p.tokens.len())
                    .sum::<usize>(),
                if success { 12 } else { 11 }
            );
            assert_eq!(
                build
                    .project
                    .programs
                    .iter()
                    .flat_map(|p| &p.tokens)
                    .filter(|t| t.kind == Kind::Eof)
                    .count(),
                if success { 3 } else { 2 }
            );
            if let Some(provider) = &provider {
                assert_eq!(provider.limits, [Some(9), Some(1), Some(0)]);
                for (index, module) in provider.modules.iter().enumerate() {
                    assert_eq!(module.completed_observations, 1);
                    assert_eq!(
                        module.attempts_after_observe,
                        [4, 21, if last.is_empty() { 29 } else { 30 }][index]
                    );
                    assert_eq!(
                        module.allocator_address,
                        build.allocator as *const Allocator as usize
                    );
                    let selected = index < 2 || success;
                    let callbacks = [
                        Callback::Begin,
                        Callback::Observe,
                        Callback::ComparisonStarted,
                        Callback::ComparisonFinished(index < 2 || !reject_eof_reserve),
                        Callback::Selected,
                    ];
                    module.assert_callbacks(&callbacks[..if selected { 5 } else { 4 }]);
                    assert_eq!(
                        module.selected_token_address,
                        if selected {
                            module.observed_token_address
                        } else {
                            0
                        }
                    );
                }
            }
        }

        #[test]
        fn lexer_caller_three_modules_zero_credit_empty_eof_succeeds() {
            for provider_route in [false, true] {
                check("", false, provider_route);
            }
        }

        #[test]
        fn lexer_caller_three_modules_zero_credit_nonempty_refuses_before_reserve() {
            for provider_route in [false, true] {
                check(ZERO_CREDIT, false, provider_route);
            }
        }

        #[test]
        fn lexer_caller_three_modules_zero_credit_empty_still_reserves_eof() {
            for provider_route in [false, true] {
                check("", true, provider_route);
            }
        }
    }
}
