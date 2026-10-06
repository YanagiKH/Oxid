//! Immutable source ownership and bounded, declaration-only loading.
//!
//! Public typed dispatch and historical adapters share one source-discovery owner.
#![allow(dead_code)] // Historical qualification adapters retain their private API.

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
    source::{SourceFileId, SourceMap, Span, MAX_SOURCE_BYTES},
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
        match builder.load_all() {
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
// Adapt the existing boxed diagnostic APIs without allocating another error wrapper.
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
        let text = self
            .read_source(Path::new(self.entry), self.entry, None)
            .map_err(one)?;
        let mut display = String::new();
        self.allocator
            .string(&mut display, self.entry.len(), "entry display")
            .map_err(|error| one(reserve_error(error, None)))?;
        display.push_str(self.entry);
        let program = self.parse_file(display, text, None)?;
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
            let child = self.load_child(frame.module, frame.next)?;
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
        let tokens = lexer::lex_with_limit(source, remaining_tokens).map_err(one)?;
        self.project.usage.non_eof_tokens =
            add(self.project.usage.non_eof_tokens, tokens.len() - 1, origin).map_err(one)?;
        let remaining_nodes = self
            .limits
            .nodes
            .min(parser::MAX_NODES)
            .checked_sub(self.project.usage.syntax_nodes)
            .ok_or_else(|| one(overflow(origin)))?;
        let (program, nodes) = match self.enums {
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
        let program = self.parse_file(display, text, at)?;
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
