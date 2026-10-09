//! Source origins only. This does not certify lowering semantics or raw OIR safety.
//! Count and validation walks are separate and checked in release. Conversion
//! occurrence authentication uses one explicitly accounted temporary bitset.
use super::super::*;
use crate::frontend::{ast, declaration_index::DeclarationIndex, source::SourceFileId};
#[path = "conversion_seen.rs"]
mod conversion_seen;
pub(in crate::frontend::oir) use conversion_seen::{
    conversion_seen_bytes, conversion_seen_carrier_bytes, ConversionOwners, ConversionSeen,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::frontend::oir) struct Counts {
    pub declarations: usize,
    pub spans: usize,
}
impl Counts {
    fn total(self) -> Result<usize, Box<Diagnostic>> {
        self.declarations.checked_add(self.spans).ok_or_else(bad)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::frontend::oir) struct BindUsage {
    pub count: Counts,
    pub validation: Counts,
    pub dimensions: usize,
}
#[cfg(test)]
thread_local! {
    static LAST_USAGE: std::cell::Cell<Option<BindUsage>> = const { std::cell::Cell::new(None) };
    static LAST_SCALAR_SCRATCH: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
pub(in crate::frontend::oir) fn last_usage() -> Option<BindUsage> {
    LAST_USAGE.get()
}

const ASSOCIATION_BAD_MESSAGE: &str = "invalid source OIR association";
const ASSOCIATION_RESOURCE_MESSAGE: &str = "conversion source association scratch limit exceeded";

pub(in crate::frontend::oir) fn bad() -> Box<Diagnostic> {
    Diagnostic::new("E0500", "oir-project-bind", ASSOCIATION_BAD_MESSAGE, None)
}
/// New association errors have finite borrowed messages and no source origins,
/// secondary labels or notes. This explicit model includes their construction
/// arguments and owned text/Diagnostic payload while helper/tracker owners are
/// still live; it excludes std allocator internals, renderers and process RSS.
#[allow(dead_code)]
struct AssociationErrorCarriers {
    code: &'static str,
    stage: &'static str,
    message: &'static str,
    origin: Option<Span>,
    into_string: String,
    diagnostic_value: Diagnostic,
    box_input: Diagnostic,
    result_box: Box<Diagnostic>,
    secondary_header: Vec<(Span, String)>,
    notes_header: Vec<String>,
    mapped_result: Result<(), Box<Diagnostic>>,
}
pub(in crate::frontend::oir) const fn association_error_bytes() -> usize {
    // The longer of the two finite message payloads is the resource diagnostic.
    std::mem::size_of::<AssociationErrorCarriers>()
        + std::mem::size_of::<Diagnostic>()
        + ASSOCIATION_RESOURCE_MESSAGE.len()
}

fn increment(value: &mut usize) -> Result<(), Box<Diagnostic>> {
    *value = value.checked_add(1).ok_or_else(bad)?;
    Ok(())
}

pub(in crate::frontend::oir) struct Visitor<'s> {
    sources: Option<&'s SourceMap>,
    file: SourceFileId,
    counts: Counts,
    dimensions: usize,
    has_conversions: bool,
}
impl<'s> Visitor<'s> {
    pub fn count() -> Self {
        Self {
            sources: None,
            file: SourceFileId(0),
            counts: Counts::default(),
            dimensions: 0,
            has_conversions: false,
        }
    }
    pub fn validate(sources: &'s SourceMap) -> Self {
        Self {
            sources: Some(sources),
            ..Self::count()
        }
    }
    pub fn has_conversions(&self) -> bool {
        self.has_conversions
    }
    pub fn file(&mut self, file: SourceFileId) {
        self.file = file;
    }
    pub fn dimension(&mut self, actual: usize, expected: usize) -> Result<(), Box<Diagnostic>> {
        increment(&mut self.dimensions)?;
        if actual != expected {
            return Err(bad());
        }
        Ok(())
    }
    pub fn declaration(&mut self) -> Result<(), Box<Diagnostic>> {
        increment(&mut self.counts.declarations)
    }
    pub fn span(&mut self, span: Span) -> Result<(), Box<Diagnostic>> {
        increment(&mut self.counts.spans)?;
        if self
            .sources
            .is_some_and(|map| span.file != self.file || !map.is_valid_span(span))
        {
            return Err(bad());
        }
        Ok(())
    }
    pub fn merge(&mut self, merge: &BoolMerge) -> Result<(), Box<Diagnostic>> {
        self.span(merge.span)?;
        self.span(merge.operator_span)?;
        for input in &merge.incoming {
            self.span(input.value.span)?;
        }
        Ok(())
    }
    pub fn statement(&mut self, statement: &Statement) -> Result<(), Box<Diagnostic>> {
        match statement {
            Statement::Assign(Assign { span, value, .. }) => {
                self.span(*span)?;
                match value {
                    Rvalue::Load(place) => self.span(place.span)?,
                    Rvalue::CheckedI32ToU8 {
                        operand, name_span, ..
                    }
                    | Rvalue::U8ToI32 {
                        operand, name_span, ..
                    } => {
                        self.has_conversions = true;
                        self.span(operand.span)?;
                        self.span(*name_span)?;
                    }
                    Rvalue::CheckedNegateI32 {
                        operand,
                        operator_span,
                    }
                    | Rvalue::NotBool {
                        operand,
                        operator_span,
                    } => {
                        self.span(operand.span)?;
                        self.span(*operator_span)?;
                    }
                    Rvalue::Copy(operand) => self.span(operand.span)?,
                    Rvalue::CompareScalar {
                        left,
                        right,
                        operator_span,
                        ..
                    }
                    | Rvalue::CheckedI32 {
                        left,
                        right,
                        operator_span,
                        ..
                    } => {
                        self.span(left.span)?;
                        self.span(right.span)?;
                        self.span(*operator_span)?;
                    }
                    Rvalue::Bool(_) | Rvalue::I32(_) | Rvalue::Unit => (),
                }
            }
            Statement::Initialize { place, value, span } => {
                self.span(*span)?;
                self.span(place.span)?;
                self.span(value.span)?;
            }
            Statement::Store {
                place,
                value,
                operator_span,
                span,
            } => {
                self.span(*span)?;
                self.span(*operator_span)?;
                self.span(place.span)?;
                self.span(value.span)?;
            }
        }
        Ok(())
    }
    pub fn finish(self, count: Self) -> Result<BindUsage, Box<Diagnostic>> {
        // This equality is deliberately not a debug assertion.
        if self.counts != count.counts {
            return Err(bad());
        }
        self.counts.total()?;
        count.counts.total()?;
        let usage = BindUsage {
            count: count.counts,
            validation: self.counts,
            dimensions: self.dimensions,
        };
        #[cfg(test)]
        LAST_USAGE.set(Some(usage));
        Ok(usage)
    }
}

// Every conversion adds a bounded direct-source proof, with no search, source
// reparsing, retained lookup table, or dependency on AST allocation order. Each
// structural inspection and each compared identifier byte is charged to the
// existing association dimensions before inspection; the ordinary span visitor
// separately accounts the assignment, receiver and intrinsic-name origins.
// This proves conversion/receiver origins and adjacent snapshot correspondence,
// not complete source-to-OIR local-declaration or lowering equivalence. Exact
// local types, initialization and arbitrary-CFG dominance remain verifier duties.
#[allow(clippy::too_many_arguments)]
pub(in crate::frontend::oir) fn authenticate_conversion(
    ast: &ast::Program,
    owner: &ast::Function,
    assign: &Assign,
    previous: Option<&Statement>,
    locals: &[LocalDecl],
    places: &[PlaceDecl],
    sources: &SourceMap,
    visitor: &mut Visitor<'_>,
    seen: &mut ConversionSeen,
    owner_slot: usize,
) -> Result<(), Box<Diagnostic>> {
    // A real discriminator inspection also happens for predecessor assignments.
    increment(&mut visitor.dimensions)?;
    let (operation, operand, name_span, source_expr) = match assign.value {
        Rvalue::CheckedI32ToU8 {
            operand,
            name_span,
            source_expr,
        } => (
            ast::ConversionOp::ToU8Checked,
            operand,
            name_span,
            source_expr,
        ),
        Rvalue::U8ToI32 {
            operand,
            name_span,
            source_expr,
        } => (ast::ConversionOp::ToI32, operand, name_span, source_expr),
        _ => return Ok(()),
    };
    macro_rules! inspect {
        ($expression:expr) => {{
            increment(&mut visitor.dimensions)?;
            $expression
        }};
    }
    let file = inspect!(sources.files().get(owner.name.file.0)).ok_or_else(bad)?;
    if !inspect!(ast.belongs_to(file)) {
        return Err(bad());
    }
    let expression = inspect!(ast.expressions.get(source_expr.0)).ok_or_else(bad)?;
    let ast::ExprKind::Conversion {
        op,
        operand: receiver,
        name_span: expected_name,
    } = inspect!(&expression.kind)
    else {
        return Err(bad());
    };
    if !inspect!(*op == operation && expression.span == assign.span && *expected_name == name_span)
    {
        return Err(bad());
    }
    let receiver = inspect!(ast.expressions.get(receiver.0)).ok_or_else(bad)?;
    let ast::ExprKind::Name(receiver_name) = inspect!(&receiver.kind) else {
        return Err(bad());
    };
    if !inspect!(*receiver_name == receiver.span && receiver.span == operand.span) {
        return Err(bad());
    }
    let body = inspect!(owner.blocks.get(owner.body.0)).ok_or_else(bad)?;
    if !inspect!(
        assign.span.file == owner.name.file
            && body.span.file == assign.span.file
            && body.span.start <= assign.span.start
            && assign.span.end <= owner.end.end
            && operand.span.start == assign.span.start
            && operand.span.end <= name_span.start
            && name_span.end < assign.span.end
    ) {
        return Err(bad());
    }
    let name = inspect!(sources.try_text(name_span)).ok_or_else(bad)?;
    let expected = match operation {
        ast::ConversionOp::ToU8Checked => "to_u8_checked",
        ast::ConversionOp::ToI32 => "to_i32",
    };
    if !inspect!(name.len() == expected.len()) {
        return Err(bad());
    }
    for (actual, expected) in name.bytes().zip(expected.bytes()) {
        if !inspect!(actual == expected) {
            return Err(bad());
        }
    }
    let previous = inspect!(previous.and_then(Statement::as_assignment)).ok_or_else(bad)?;
    if !inspect!(
        previous.destination == operand.local
            && previous.span == operand.span
            && previous.destination != assign.destination
    ) {
        return Err(bad());
    }
    for (local, span) in [
        (previous.destination, operand.span),
        (assign.destination, assign.span),
    ] {
        let declaration = inspect!(locals.get(local.0)).ok_or_else(bad)?;
        if !inspect!(declaration.kind == LocalKind::Temporary && declaration.span == span) {
            return Err(bad());
        }
    }
    let binding_span = match inspect!(&previous.value) {
        Rvalue::Copy(value) => {
            let declaration = inspect!(locals.get(value.local.0)).ok_or_else(bad)?;
            if !inspect!(
                value.span == operand.span
                    && matches!(declaration.kind, LocalKind::Parameter | LocalKind::Binding)
            ) {
                return Err(bad());
            }
            declaration.span
        }
        Rvalue::Load(place) => {
            let declaration = inspect!(places.get(place.id.0)).ok_or_else(bad)?;
            if !inspect!(place.span == operand.span) {
                return Err(bad());
            }
            declaration.span
        }
        _ => return Err(bad()),
    };
    let receiver_text = inspect!(sources.try_text(operand.span)).ok_or_else(bad)?;
    let binding_text = inspect!(sources.try_text(binding_span)).ok_or_else(bad)?;
    if !inspect!(
        binding_span.file == owner.name.file
            && owner.name.start <= binding_span.start
            && binding_span.end <= owner.end.end
            && binding_text.len() == receiver_text.len()
    ) {
        return Err(bad());
    }
    for (binding, receiver) in binding_text.bytes().zip(receiver_text.bytes()) {
        if !inspect!(binding == receiver) {
            return Err(bad());
        }
    }
    seen.mark(owner_slot, source_expr, visitor)?;
    Ok(())
}

/// Explicit named transport envelope for source resource accounting. This is
/// an inventory of simultaneously possible argument/result and lookup roles,
/// never a claim about the compiler's stack layout or process peak memory.
#[allow(dead_code)]
struct ConversionAuthenticationCarriers {
    ast: &'static ast::Program,
    owner: &'static ast::Function,
    assign: &'static Assign,
    previous: Option<&'static Statement>,
    locals: &'static [LocalDecl],
    places: &'static [PlaceDecl],
    sources: &'static SourceMap,
    visitor: &'static Visitor<'static>,
    seen: &'static ConversionSeen,
    owner_slot: usize,
    operation: ast::ConversionOp,
    operand: Operand,
    name_span: Span,
    source_expr: ast::ExprId,
    conversion_tuple: (ast::ConversionOp, Operand, Span, ast::ExprId),
    source_return: Option<&'static crate::frontend::source::SourceFile>,
    source: &'static crate::frontend::source::SourceFile,
    expression_return: Option<&'static ast::Expr>,
    expression: &'static ast::Expr,
    receiver_return: Option<&'static ast::Expr>,
    receiver: &'static ast::Expr,
    receiver_name: Span,
    body_return: Option<&'static ast::BodyBlock>,
    body: &'static ast::BodyBlock,
    text_returns: [Option<&'static str>; 3],
    texts: [&'static str; 4],
    snapshot_return: Option<&'static Assign>,
    snapshot: &'static Assign,
    local_return: Option<&'static LocalDecl>,
    local: &'static LocalDecl,
    place_return: Option<&'static PlaceDecl>,
    place: &'static PlaceDecl,
    binding_span: Span,
    value: &'static Rvalue,
    local_iterator: std::array::IntoIter<(LocalId, Span), 2>,
    byte_iterator: std::iter::Zip<std::str::Bytes<'static>, std::str::Bytes<'static>>,
    next_byte_pair: Option<(u8, u8)>,
    predicate: bool,
    increment_return: Result<(), Box<Diagnostic>>,
    return_value: Result<(), Box<Diagnostic>>,
}
pub(in crate::frontend::oir) const fn conversion_carrier_bytes() -> usize {
    std::mem::size_of::<ConversionAuthenticationCarriers>()
}

pub(in crate::frontend::oir) enum Declarations<'a, 's> {
    Original(&'a ast::Program),
    Project(&'a DeclarationIndex<'s>),
}
impl Declarations<'_, '_> {
    fn conversion_owners(&self) -> ConversionOwners<'_> {
        match self {
            Self::Original(ast) => ConversionOwners::Original(ast),
            Self::Project(index) => ConversionOwners::Indexed(index.sources()),
        }
    }
    fn count(&self) -> usize {
        match self {
            Self::Original(ast) => ast.functions.len(),
            Self::Project(index) => index.function_count(),
        }
    }
    fn function(
        &self,
        id: hir::DefId,
    ) -> Result<(&ast::Program, &ast::Function, usize), Box<Diagnostic>> {
        let (program, ordinal, owner_slot) = match self {
            Self::Original(ast) => (*ast, id.0, 0),
            Self::Project(index) => {
                let (key, module) = index.function(id).map_err(|_| bad())?;
                (
                    index.sources().ast(module).map_err(|_| bad())?,
                    key.index,
                    module.0,
                )
            }
        };
        Ok((
            program,
            program.functions.get(ordinal).ok_or_else(bad)?,
            owner_slot,
        ))
    }
    #[cfg(test)]
    fn name(&self, id: hir::DefId) -> Result<Span, Box<Diagnostic>> {
        match self {
            Self::Original(ast) => ast.functions.get(id.0).map(|f| f.name).ok_or_else(bad),
            Self::Project(index) => {
                let (key, module) = index.function(id).map_err(|_| bad())?;
                index
                    .sources()
                    .ast(module)
                    .map_err(|_| bad())?
                    .functions
                    .get(key.index)
                    .map(|f| f.name)
                    .ok_or_else(bad)
            }
        }
    }
}
fn scalar_function(
    function: &Function,
    visitor: &mut Visitor<'_>,
    authentication: Option<(&ast::Program, &ast::Function, &SourceMap, usize)>,
    seen: &mut ConversionSeen,
) -> Result<(), Box<Diagnostic>> {
    visitor.declaration()?;
    visitor.span(function.span)?;
    for local in &function.locals {
        visitor.span(local.span)?;
    }
    for place in &function.places {
        visitor.span(place.span)?;
    }
    for block in &function.blocks {
        visitor.span(block.span)?;
        if let Some(merge) = &block.merge {
            visitor.merge(merge)?;
        }
        for (ordinal, statement) in block.statements.iter().enumerate() {
            visitor.statement(statement)?;
            if let (Some((ast, owner, sources, owner_slot)), Some(assign)) =
                (authentication, statement.as_assignment())
            {
                authenticate_conversion(
                    ast,
                    owner,
                    assign,
                    ordinal
                        .checked_sub(1)
                        .and_then(|index| block.statements.get(index)),
                    &function.locals,
                    &function.places,
                    sources,
                    visitor,
                    seen,
                    owner_slot,
                )?;
            }
        }
        if let Some(terminator) = &block.terminator {
            visitor.span(terminator.span)?;
            match &terminator.kind {
                TerminatorKind::Branch { condition, .. } => visitor.span(condition.span)?,
                TerminatorKind::Call { args, .. } => {
                    for arg in args {
                        visitor.span(arg.span)?;
                    }
                }
                TerminatorKind::Return(operand) => visitor.span(operand.span)?,
                TerminatorKind::Goto { .. } => (),
            }
        }
    }
    Ok(())
}
pub(in crate::frontend::oir) fn scalar(
    raw: &Program,
    sources: &SourceMap,
    declarations: Declarations<'_, '_>,
) -> Result<BindUsage, Box<Diagnostic>> {
    let mut count = Visitor::count();
    for function in &raw.functions {
        scalar_function(function, &mut count, None, &mut ConversionSeen::empty())?;
    }
    let mut visitor = Visitor::validate(sources);
    visitor.dimension(raw.functions.len(), declarations.count())?;
    let mut seen = if count.has_conversions() {
        // A genuine index may include synthetic builtin functions. They are an
        // owned-route domain, and index.function only addresses source rows.
        // Reject that mismatch before any tracker allocation or later lookup.
        if let Declarations::Project(index) = &declarations {
            visitor.dimension(index.function_count(), index.source_function_count())?;
        }
        ConversionSeen::new(declarations.conversion_owners(), &mut visitor)?
    } else {
        ConversionSeen::empty()
    };
    let modeled_scratch = scalar_association_carrier_bytes()
        .checked_add(seen.modeled_bytes()?.max(association_error_bytes()))
        .ok_or_else(bad)?;
    #[cfg(test)]
    LAST_SCALAR_SCRATCH.set(Some(modeled_scratch));
    #[cfg(not(test))]
    let _ = modeled_scratch;
    for (ordinal, function) in raw.functions.iter().enumerate() {
        let (ast, declaration, owner_slot) = declarations.function(hir::DefId(ordinal))?;
        let expected = declaration.name;
        if function.id != hir::DefId(ordinal) || function.span != expected {
            return Err(bad());
        }
        visitor.file(expected.file);
        scalar_function(
            function,
            &mut visitor,
            Some((ast, declaration, sources, owner_slot)),
            &mut seen,
        )?;
    }
    visitor.finish(count)
}

/// Scalar-only caller/consumer control inventory. This is separate from the
/// shared tracker bank: owned callers price their own calling convention. The
/// additive model makes no credit for optimized copies or stack-slot reuse.
#[allow(dead_code, clippy::type_complexity)]
struct ScalarAssociationCarriers {
    raw_argument: &'static Program,
    source_argument: &'static SourceMap,
    declaration_argument: Declarations<'static, 'static>,
    count_visitor: Visitor<'static>,
    validation_visitor: Visitor<'static>,
    tracker_local: ConversionSeen,
    function_iteration: std::iter::Enumerate<std::slice::Iter<'static, Function>>,
    function_count_iteration: std::slice::Iter<'static, Function>,
    block_iteration: std::slice::Iter<'static, BasicBlock>,
    statement_iteration: std::iter::Enumerate<std::slice::Iter<'static, Statement>>,
    function: &'static Function,
    statement: &'static Statement,
    block: &'static BasicBlock,
    function_ordinal: usize,
    statement_ordinal: usize,
    function_call_arguments: [(
        &'static Function,
        &'static Visitor<'static>,
        &'static ConversionSeen,
    ); 2],
    function_call_results: [Result<(), Box<Diagnostic>>; 2],
    count_empty_tracker: ConversionSeen,
    previous_checked_index: Option<usize>,
    previous_lookup_index: usize,
    previous_lookup_capture: &'static BasicBlock,
    previous_lookup_receiver: &'static [Statement],
    assignment: Option<&'static Assign>,
    previous: Option<&'static Statement>,
    function_lookup:
        Result<(&'static ast::Program, &'static ast::Function, usize), Box<Diagnostic>>,
    function_tuple: (&'static ast::Program, &'static ast::Function, usize),
    builtin_count_guard: (usize, usize),
    source_count_result: Result<(), Box<Diagnostic>>,
    expected_origin: Span,
    function_auth_argument: Option<(
        &'static ast::Program,
        &'static ast::Function,
        &'static SourceMap,
        usize,
    )>,
    function_auth_receiver: Option<(
        &'static ast::Program,
        &'static ast::Function,
        &'static SourceMap,
        usize,
    )>,
    function_auth_match: (
        Option<(
            &'static ast::Program,
            &'static ast::Function,
            &'static SourceMap,
            usize,
        )>,
        Option<&'static Assign>,
    ),
    usage: BindUsage,
    usage_result: Result<BindUsage, Box<Diagnostic>>,
    scalar_result: Result<BindUsage, Box<Diagnostic>>,
    constructor_raw_argument: Program,
    constructor_source_argument: &'static SourceMap,
    constructor_declarations: Declarations<'static, 'static>,
    associated_literal: AssociatedScalar<'static>,
    associated_result: Result<AssociatedScalar<'static>, Box<Diagnostic>>,
    consumer_argument: AssociatedScalar<'static>,
    unpack_receiver: AssociatedScalar<'static>,
    consumed_tuple: (Program, &'static SourceMap),
    unpacked_program: Program,
    unpacked_sources: &'static SourceMap,
    witness_result: Result<VerifiedProgram, OirFailure>,
    checked_bytes: Result<usize, Box<Diagnostic>>,
    modeled_scratch: usize,
    checked_scratch: Option<usize>,
    model_error_envelope: usize,
}
pub(in crate::frontend::oir) const fn scalar_association_carrier_bytes() -> usize {
    std::mem::size_of::<ScalarAssociationCarriers>()
}

/// Owns exactly the raw program that passed the independent source association.
/// Its private constructor binds that immutable program to this source owner;
/// no detached flag or reusable source token can authorize a different program.
#[derive(Debug)]
pub(in crate::frontend::oir) struct AssociatedScalar<'s> {
    raw: Program,
    sources: &'s SourceMap,
}
impl<'s> AssociatedScalar<'s> {
    pub(in crate::frontend::oir) fn into_parts(self) -> (Program, &'s SourceMap) {
        (self.raw, self.sources)
    }
}
pub(in crate::frontend::oir) fn authenticate_scalar<'s>(
    raw: Program,
    sources: &'s SourceMap,
    declarations: Declarations<'_, '_>,
) -> Result<AssociatedScalar<'s>, Box<Diagnostic>> {
    scalar(&raw, sources, declarations)?;
    Ok(AssociatedScalar { raw, sources })
}

#[cfg(test)]
#[path = "../u8_association_tests.rs"]
mod u8_tests;

#[cfg(test)]
mod root_projection_tests {
    use super::*;
    use crate::frontend::{
        declaration_index::{collect_originals, IndexLimits, SourceOwner, WorkMeter},
        project::{budget::Allocator, ModuleId, ProjectLimits, ProjectSources, SyntaxFlavor},
    };

    pub(super) fn project(text: &str) -> ProjectSources {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "oxid-root-projection-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let file = directory.join("main.ox");
        std::fs::write(&file, text).unwrap();
        ProjectSources::load_typed(file.to_str().unwrap(), ProjectLimits::default()).unwrap()
    }

    fn equivalent(text: &str, names: &[&str], flavor: SyntaxFlavor) {
        let project = project(text);
        assert_eq!(project.syntax_flavor(), flavor);
        assert_eq!(project.modules().len(), 1);
        let owner = SourceOwner::project(&project);
        let root = owner.ast(ModuleId(0)).unwrap();
        assert!(root.modules.is_empty());
        assert!(root.imports.is_empty());
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let frozen = collect_originals(owner, IndexLimits::default(), &work, &mut allocator)
            .unwrap()
            .finish(&work, &mut allocator)
            .unwrap();

        // This is only a count/order/name projection of a genuine root AST.
        // It neither constructs an original source owner nor runs verification.
        let original = Declarations::Original(root);
        let indexed = Declarations::Project(&frozen);
        assert_eq!(original.count(), names.len());
        assert_eq!(indexed.count(), original.count());
        for (ordinal, name) in names.iter().enumerate() {
            let id = hir::DefId(ordinal);
            let span = original.name(id).unwrap();
            assert_eq!(span, root.functions[ordinal].name);
            assert_eq!(indexed.name(id).unwrap(), span);
            assert_eq!(project.try_text(span), Some(*name));
        }
        for ordinal in [names.len(), usize::MAX] {
            let original = original.name(hir::DefId(ordinal)).unwrap_err();
            let indexed = indexed.name(hir::DefId(ordinal)).unwrap_err();
            assert_eq!(
                (original.code, original.stage),
                ("E0500", "oir-project-bind")
            );
            assert_eq!(
                (indexed.code, indexed.stage),
                (original.code, original.stage)
            );
            assert_eq!(indexed.message, original.message);
            assert_eq!(indexed.primary, original.primary);
        }
    }

    #[test]
    fn checked_hir_import_root_declarations_public_main() {
        equivalent(
            "pub fn main()->i32{return 7;}",
            &["main"],
            SyntaxFlavor::ProjectSyntax,
        );
    }

    #[test]
    fn checked_hir_import_root_declarations_private_helper_before_main() {
        equivalent(
            "fn helper()->i32{return 7;} pub fn main()->i32{return helper();}",
            &["helper", "main"],
            SyntaxFlavor::ProjectSyntax,
        );
    }

    #[test]
    fn checked_hir_import_root_declarations_library_without_main() {
        equivalent(
            "pub fn library()->i32{return helper();} fn helper()->i32{return 7;}",
            &["library", "helper"],
            SyntaxFlavor::ProjectSyntax,
        );
    }

    #[test]
    fn checked_hir_import_root_declarations_empty() {
        equivalent("", &[], SyntaxFlavor::OriginalSingleFile);
    }
}
