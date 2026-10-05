//! Complete declaration/name resolution. No ownership or loan analysis.
use super::hir::*;
use crate::frontend::{
    ast,
    declaration_index::{
        self as index, Access, DeclarationIndex, Exposure, IndexLimits, PreparedTypeName,
        QuerySession, SourceOwner, TypeContext, WorkMeter,
    },
    diagnostic::Diagnostic,
    owned_diagnostic::{self, diagnostic, secondary},
    parser::MAX_DIAGNOSTICS,
    project::{
        budget::{Allocator, ReserveFailure},
        ItemPathRef, ModuleId,
    },
    source::{SourceFile, SourceMap, SourceView, Span},
};
use std::collections::HashMap;

/// Construction policy, retained even for array-free inputs. No executable
/// entrypoint accepts a caller-selected admission policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SourceAdmission {
    Executable,
    #[cfg(test)]
    ObserveArrayTypes,
    #[cfg(test)]
    ObserveArrayPipeline,
    #[cfg(test)]
    ArrayConsumer,
}
impl SourceAdmission {
    pub(super) fn executable(self) -> bool {
        self == Self::Executable
    }
    pub(super) fn allows_lowering(self) -> bool {
        match self {
            Self::Executable => true,
            #[cfg(test)]
            Self::ObserveArrayPipeline | Self::ArrayConsumer => true,
            #[cfg(test)]
            Self::ObserveArrayTypes => false,
        }
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // Keep the checked legacy carrier allocation-free.
enum IndexOwner<'src> {
    Owned(DeclarationIndex<'src>),
    Borrowed(&'src DeclarationIndex<'src>),
}

#[derive(Debug)]
enum MeterOwner<'src> {
    Owned(WorkMeter),
    Borrowed(&'src WorkMeter),
}
#[derive(Debug)]
pub(in crate::frontend::oir) struct ResolvedOwnedProgram<'src> {
    projection_bytes: std::cell::Cell<usize>,
    admission: SourceAdmission,
    index: IndexOwner<'src>,
    work: MeterOwner<'src>,
    sources: SourceView<'src>,
    records: Vec<Record>,
    signatures: Vec<Signature>,
    functions: Vec<Function>,
    entry: Option<DefId>,
}
impl<'src> ResolvedOwnedProgram<'src> {
    /// Retained typed path payload is cumulative across functions. Admit the
    /// complete exact reservation before allocating; raw paths are independently
    /// inventoried again by lowering and by the verifier.
    pub(super) fn admit_projection(
        &self,
        length: usize,
        span: Span,
    ) -> Result<(), Box<Diagnostic>> {
        if length == 0 || length > 64 {
            return Err(error(
                "E0400",
                format_args!("record access path depth limit exceeded"),
                span,
            ));
        }
        let bytes = length
            .checked_mul(std::mem::size_of::<FieldId>())
            .ok_or_else(|| {
                error(
                    "E0400",
                    format_args!("typed record projection payload limit exceeded"),
                    span,
                )
            })?;
        self.admit_projection_metadata(bytes, span)?;
        // Existing one-hop fields retain their permission-query work charge.
        if length > 1 {
            self.work()
                .debit(length as u64, span, "record projection path")?;
        }
        Ok(())
    }
    /// Sparse borrow-path lookup entries and path fields share a cumulative byte
    /// cap. Admit each complete requested reservation before any growth.
    pub(super) fn admit_projection_metadata(
        &self,
        bytes: usize,
        span: Span,
    ) -> Result<(), Box<Diagnostic>> {
        let total = self
            .projection_bytes
            .get()
            .checked_add(bytes)
            .filter(|bytes| *bytes <= super::budget::MAX_RAW_BYTES)
            .ok_or_else(|| {
                error(
                    "E0400",
                    format_args!("typed record projection payload limit exceeded"),
                    span,
                )
            })?;
        self.projection_bytes.set(total);
        Ok(())
    }
    pub(super) fn admission(&self) -> SourceAdmission {
        self.admission
    }
    pub(super) fn index(&self) -> &DeclarationIndex<'src> {
        match &self.index {
            IndexOwner::Owned(index) => index,
            IndexOwner::Borrowed(index) => index,
        }
    }
    pub(super) fn work(&self) -> &WorkMeter {
        match &self.work {
            MeterOwner::Owned(work) => work,
            MeterOwner::Borrowed(work) => work,
        }
    }
    pub(super) fn query(&self) -> QuerySession<'_, 'src> {
        self.index().query(self.work())
    }
    pub(super) fn requester(&self, function: DefId) -> Result<ModuleId, Box<Diagnostic>> {
        self.index().function(function).map(|(_, module)| module)
    }
    pub(super) fn prepare_name(
        &self,
        record: RecordId,
        at: Span,
    ) -> Result<PreparedTypeName<'src>, Box<Diagnostic>> {
        self.query().prepare_type_name(record, at)
    }
    pub(super) fn text(&self, span: Span) -> &str {
        self.sources.text(span)
    }
    pub(super) fn records(&self) -> &[Record] {
        &self.records
    }
    pub(super) fn signatures(&self) -> &[Signature] {
        &self.signatures
    }
    pub(super) fn functions(&self) -> &[Function] {
        &self.functions
    }
    pub(super) fn entry(&self) -> Option<DefId> {
        self.entry
    }
}
fn text(source: &SourceFile, span: Span) -> &str {
    source.text_at(span)
}
fn error(code: &'static str, message: std::fmt::Arguments<'_>, span: Span) -> Box<Diagnostic> {
    diagnostic(code, "resolve", message, Some(span))
}
fn duplicate(span: Span, original: Span) -> Box<Diagnostic> {
    secondary(
        error(
            "E0201",
            format_args!("duplicate binding; shadowing is unavailable in typed-preview"),
            span,
        ),
        original,
        format_args!("first declared here"),
    )
}
fn value_type(
    query: &mut QuerySession<'_, '_>,
    requester: ModuleId,
    ty: ast::TypeSyntax,
) -> Result<ValueTy, Box<Diagnostic>> {
    query.value_type(requester, ty, TypeContext::Value)
}
// Generic value-type resolution must never silently authorize enum containment.
fn record_field_type(ty: ValueTy, span: Span) -> Result<ValueTy, Box<Diagnostic>> {
    if matches!(ty, ValueTy::Owned(AggregateTy::Enum(_))) {
        Err(error(
            "E0300",
            format_args!("enum values cannot be record fields"),
            span,
        ))
    } else {
        Ok(ty)
    }
}
#[test]
fn bounded_enum_source_record_field_fence_is_independent_of_resolution() {
    let mut sources = SourceMap::new();
    let id = sources.add("enum-field-gate.ox".into(), "field".into());
    let span = sources.get(id).span(0, 5);
    for id in [0, usize::MAX] {
        let error = record_field_type(
            ValueTy::Owned(AggregateTy::Enum(
                crate::frontend::oir::owned_types::EnumId(id),
            )),
            span,
        )
        .unwrap_err();
        assert_eq!(error.code, "E0300");
        assert_eq!(error.primary, Some(span));
    }
}

fn parameter_type(
    query: &mut QuerySession<'_, '_>,
    requester: ModuleId,
    ty: ast::TypeSyntax,
) -> Result<ParameterTy, Box<Diagnostic>> {
    query.parameter_type(requester, ty)
}
fn decimal_i32(digits: &str, negative: bool, span: Span) -> Result<i32, Box<Diagnostic>> {
    let mut value = 0i32;
    for byte in digits.bytes() {
        let digit = i32::from(byte - b'0');
        value = value
            .checked_mul(10)
            .and_then(|value| {
                if negative {
                    value.checked_sub(digit)
                } else {
                    value.checked_add(digit)
                }
            })
            .ok_or_else(|| {
                error(
                    "E0203",
                    format_args!(
                        "decimal literal is outside the i32 range [-2147483648, 2147483647]"
                    ),
                    span,
                )
            })?;
    }
    Ok(value)
}

#[cfg(test)]
pub(super) fn resolve<'src>(
    source: &'src SourceFile,
    ast: &'src ast::Program,
) -> Result<ResolvedOwnedProgram<'src>, Vec<Diagnostic>> {
    resolve_with_view(source, ast, SourceView::Single(source))
}
pub(super) fn resolve_in_map<'src>(
    source: &'src SourceFile,
    ast: &'src ast::Program,
    sources: &'src SourceMap,
) -> Result<ResolvedOwnedProgram<'src>, Vec<Diagnostic>> {
    resolve_with_view(source, ast, SourceView::Map(sources))
}
fn resolve_with_view<'src>(
    source: &'src SourceFile,
    ast: &'src ast::Program,
    view: SourceView<'src>,
) -> Result<ResolvedOwnedProgram<'src>, Vec<Diagnostic>> {
    let sources = SourceOwner::original(source, ast, view).map_err(|e| vec![*e])?;
    resolve_sources(sources)
}
pub(in crate::frontend) fn resolve_sources(
    sources: SourceOwner<'_>,
) -> Result<ResolvedOwnedProgram<'_>, Vec<Diagnostic>> {
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let (index, (records, signatures, functions)) =
        resolve_source_parts(sources, &work, &mut allocator, IndexLimits::default())?;
    let entry = index.root_original_main();
    Ok(ResolvedOwnedProgram {
        projection_bytes: std::cell::Cell::new(0),
        admission: SourceAdmission::Executable,
        sources: sources.view(),
        index: IndexOwner::Owned(index),
        work: MeterOwner::Owned(work),
        records,
        signatures,
        functions,
        entry,
    })
}
#[cfg(test)]
pub(in crate::frontend::oir) fn resolve_observed<'s>(
    source: &'s SourceFile,
    ast: &'s ast::Program,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<ResolvedOwnedProgram<'s>, Vec<Diagnostic>> {
    let sources =
        SourceOwner::original(source, ast, SourceView::Single(source)).map_err(|e| vec![*e])?;
    let (index, (records, signatures, functions)) =
        resolve_source_parts(sources, work, allocator, IndexLimits::default())?;
    let entry = index.root_original_main();
    Ok(ResolvedOwnedProgram {
        projection_bytes: std::cell::Cell::new(0),
        admission: SourceAdmission::Executable,
        sources: sources.view(),
        index: IndexOwner::Owned(index),
        work: MeterOwner::Borrowed(work),
        records,
        signatures,
        functions,
        entry,
    })
}
fn resolve_source_parts<'s>(
    sources: SourceOwner<'s>,
    work: &WorkMeter,
    allocator: &mut Allocator,
    limits: IndexLimits,
) -> Result<(DeclarationIndex<'s>, ResolvedParts), Vec<Diagnostic>> {
    let facts = index::collect_originals(sources, limits, work, allocator).map_err(|e| vec![*e])?;
    let index = facts.finish(work, allocator)?;
    let parts = resolve_index(&index, work, allocator)?;
    Ok((index, parts))
}
pub(in crate::frontend) fn resolve_project<'s>(
    index: &'s DeclarationIndex<'s>,
    work: &'s WorkMeter,
) -> Result<ResolvedOwnedProgram<'s>, Vec<Diagnostic>> {
    let mut allocator = Allocator::default();
    let (records, signatures, functions) = resolve_index(index, work, &mut allocator)?;
    Ok(ResolvedOwnedProgram {
        projection_bytes: std::cell::Cell::new(0),
        admission: SourceAdmission::Executable,
        sources: index.sources().view(),
        index: IndexOwner::Borrowed(index),
        work: MeterOwner::Borrowed(work),
        records,
        signatures,
        functions,
        entry: index.root_original_main(),
    })
}
type ResolvedParts = (Vec<Record>, Vec<Signature>, Vec<Function>);
fn resolve_index(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<ResolvedParts, Vec<Diagnostic>> {
    let sources = index.sources();
    let mut diagnostics = Vec::new();
    let mut records = Vec::new();
    work.phase("record-fields");
    for id in 0..index.record_count() {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
        let (key, module) = index.record(RecordId(id)).map_err(|e| vec![*e])?;
        let record = &sources.ast(module).map_err(|e| vec![*e])?.records[key.index];
        work.record_start(RecordId(id), record.name);
        let result = (|| {
            let mut fields = Vec::new();
            let mut names = HashMap::new();
            for field in &record.fields {
                if let Some(first) = names.insert(sources.text(field.name)?, field.name) {
                    return Err(duplicate(field.name, first));
                }
                let ty = record_field_type(
                    value_type(&mut index.query(work), module, field.ty)?,
                    field.span,
                )?;
                fields.push(Field {
                    id: FieldId {
                        record: RecordId(id),
                        index: fields.len(),
                    },
                    ty,
                    name_span: field.name,
                    span: field.span,
                });
            }
            Ok(Record {
                id: RecordId(id),
                name_span: record.name,
                span: record.span,
                end: record.end,
                fields,
            })
        })();
        match result {
            Ok(record) => records.push(record),
            Err(error) => {
                work.record_error(&error);
                diagnostics.push(*error)
            }
        }
    }
    work.phase("signatures");
    let mut signatures = Vec::new();
    for id in 0..index.function_count() {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
        let (key, module) = index.function(DefId(id)).map_err(|e| vec![*e])?;
        let function = &sources.ast(module).map_err(|e| vec![*e])?.functions[key.index];
        work.signature_start(DefId(id), function.name);
        let result: Result<Signature, Box<Diagnostic>> = (|| {
            let params = function
                .params
                .iter()
                .map(|p| parameter_type(&mut index.query(work), module, p.ty))
                .collect::<Result<Vec<_>, _>>()?;
            let result = value_type(&mut index.query(work), module, function.result)?;
            for block in &function.blocks {
                for statement in &block.body {
                    if let ast::StmtKind::Let {
                        annotation: Some(ty),
                        ..
                    } = statement.kind
                    {
                        value_type(&mut index.query(work), module, ty)?;
                    }
                }
            }
            Ok(Signature {
                params,
                result,
                span: function.name,
            })
        })();
        match result {
            Ok(signature) => signatures.push(signature),
            Err(error) => {
                work.record_error(&error);
                diagnostics.push(*error)
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    if records.iter().any(|record| {
        record
            .fields
            .iter()
            .any(|field| matches!(field.ty, ValueTy::Owned(_)))
    }) {
        use crate::frontend::oir::owned_types::{admit_value_layouts, DeclarationError};
        if let Err(failure) = admit_value_layouts(
            records
                .iter()
                .map(|record| record.fields.iter().map(|field| field.ty)),
        ) {
            let (code, message, span) = match failure {
                DeclarationError::ContainmentCycle(record) => (
                    "E0202",
                    "record containment must be acyclic",
                    records[record.0].name_span,
                ),
                DeclarationError::ResourceLimit(resource) => ("E0400", resource, sources.eof()),
                DeclarationError::LayoutOverflow => {
                    ("E0400", "record layout overflow", sources.eof())
                }
                DeclarationError::Allocation => (
                    "E0400",
                    "record declaration allocation failed",
                    sources.eof(),
                ),
                _ => (
                    "E0500",
                    "invalid resolved record declaration",
                    sources.eof(),
                ),
            };
            return Err(vec![*error(code, format_args!("{message}"), span)]);
        }
    }
    work.phase("exposure");
    // Exposure consumes already selected nominal identities; it does not resolve
    // signatures a second time or consult caller enumeration.
    for (id, signature) in signatures.iter().enumerate() {
        let (key, module) = index.function(DefId(id)).map_err(|e| vec![*e])?;
        let function = &sources.ast(module).map_err(|e| vec![*e])?.functions[key.index];
        let params = signature
            .params
            .iter()
            .zip(&function.params)
            .map(|(ty, p)| {
                (
                    match *ty {
                        ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(r)))
                        | ParameterTy::Reference {
                            referent: BorrowedTy::Exact(AggregateTy::Record(r)),
                            ..
                        } => Some(r),
                        _ => None,
                    },
                    &p.ty,
                )
            });
        let result = (
            match signature.result {
                ValueTy::Owned(AggregateTy::Record(r)) => Some(r),
                _ => None,
            },
            &function.result,
        );
        for (record, ty) in params.chain(std::iter::once(result)) {
            let Some(record) = record else { continue };
            let at = match ty.kind {
                ast::TypeSyntaxKind::Reference { referent, .. } => sources
                    .path_span(ItemPathRef {
                        file: ty.span.file,
                        path: referent,
                    })
                    .map_err(|e| vec![*e])?,
                _ => ty.span,
            };
            match index
                .query(work)
                .signature_exposure(DefId(id), record, at)
                .map_err(|e| vec![*e])?
            {
                Exposure::Allowed => (),
                Exposure::Denied { record, restrictor } => {
                    let mut error = secondary(
                        error(
                            "E0207",
                            format_args!("function signature exposes a less visible record type"),
                            at,
                        ),
                        record,
                        format_args!("record type declared here"),
                    );
                    if let Some(restrictor) = restrictor {
                        error = secondary(
                            error,
                            restrictor,
                            format_args!("restrictive module declared here"),
                        );
                    }
                    work.record_error(&error);
                    diagnostics.push(*error);
                    break;
                }
            }
        }
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    work.phase("body-resolution");
    let mut functions = Vec::new();
    let mut array_entries = 0usize;
    for id in 0..index.function_count() {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
        let (key, requester) = index.function(DefId(id)).map_err(|e| vec![*e])?;
        let ast = sources.ast(requester).map_err(|e| vec![*e])?;
        let mut resolver = Resolver {
            allocator,
            array_entries: &mut array_entries,
            ast,
            index,
            work,
            requester,
            records: &records,
            scope: HashMap::new(),
            bindings: Vec::new(),
            expressions: Vec::new(),
        };
        match resolver.function(DefId(id), &ast.functions[key.index]) {
            Ok(function) => functions.push(function),
            Err(error) => {
                work.record_error(&error);
                diagnostics.push(*error)
            }
        }
    }
    if diagnostics.is_empty() {
        Ok((records, signatures, functions))
    } else {
        Err(diagnostics)
    }
}
struct Resolver<'i, 'a> {
    allocator: &'i mut Allocator,
    array_entries: &'i mut usize,
    ast: &'a ast::Program,
    index: &'i DeclarationIndex<'a>,
    work: &'i WorkMeter,
    requester: ModuleId,
    records: &'i [Record],
    scope: HashMap<&'a str, (BindingId, Span)>,
    bindings: Vec<Binding>,
    expressions: Vec<Expr>,
}
impl<'a> Resolver<'_, 'a> {
    fn text(&self, span: Span) -> &'a str {
        self.index
            .sources()
            .text(span)
            .expect("validated source span")
    }
    fn lookup(&self, span: Span) -> Result<BindingId, Box<Diagnostic>> {
        // Projected array access and borrowing retain the named-root path in their
        // base span; only its first identifier participates in local lookup.
        let start = self
            .ast
            .tokens
            .partition_point(|token| token.span.end <= span.start);
        let root = self
            .ast
            .tokens
            .get(start)
            .map(|token| token.span)
            .unwrap_or(span);
        self.scope
            .get(self.text(root))
            .map(|entry| entry.0)
            .ok_or_else(|| {
                error(
                    "E0200",
                    format_args!(
                        "unknown local `{}`",
                        owned_diagnostic::name(self.text(span))
                    ),
                    span,
                )
            })
    }
    fn bind(
        &mut self,
        span: Span,
        annotation: Option<ValueTy>,
        mutable: bool,
        scope: BodyBlockId,
        parameter_position: Option<usize>,
    ) -> Result<BindingId, Box<Diagnostic>> {
        let name = self.text(span);
        if let Some((_, previous)) = self.scope.get(name) {
            return Err(duplicate(span, *previous));
        }
        if let Some(previous) = self
            .index
            .query(self.work)
            .value_binding_for_local_conflict(self.requester, span)?
        {
            return Err(duplicate(span, previous));
        }
        let id = BindingId(self.bindings.len());
        self.bindings.push(Binding {
            span,
            annotation,
            mutable,
            scope,
            parameter_position,
        });
        self.scope.insert(name, (id, span));
        Ok(id)
    }
    fn function(
        &mut self,
        id: DefId,
        function: &ast::Function,
    ) -> Result<Function, Box<Diagnostic>> {
        for (position, param) in function.params.iter().enumerate() {
            self.bind(
                param.name,
                None,
                false,
                BodyBlockId(function.body.0),
                Some(position),
            )?;
        }
        // Keep block IDs stable while resolving statements depth first. Only
        // currently active names stay in the lookup table; each scope removes
        // its own names on exit, so neither cloning nor ancestor scans are needed.
        enum Frame {
            Enter(ast::BodyBlockId),
            Next(ast::BodyBlockId, usize),
            Leave,
            LeaveLoop,
        }
        let mut blocks: Vec<_> = function
            .blocks
            .iter()
            .map(|block| BodyBlock {
                body: Vec::with_capacity(block.body.len()),
                span: block.span,
                end: block.end,
            })
            .collect();
        let mut scopes: Vec<Vec<&'a str>> = Vec::new();
        let mut loops = Vec::new();
        let mut frames = vec![Frame::Enter(function.body)];
        while let Some(frame) = frames.pop() {
            let (block, index) = match frame {
                Frame::Enter(block) => {
                    scopes.push(Vec::new());
                    frames.push(Frame::Leave);
                    frames.push(Frame::Next(block, 0));
                    continue;
                }
                Frame::Leave => {
                    for name in scopes.pop().expect("entered scope") {
                        self.scope.remove(name);
                    }
                    continue;
                }
                Frame::LeaveLoop => {
                    loops.pop().expect("entered loop context");
                    continue;
                }
                Frame::Next(block, index) => (block, index),
            };
            let Some(statement) = function.blocks[block.0].body.get(index) else {
                continue;
            };
            frames.push(Frame::Next(block, index + 1));
            let kind = match &statement.kind {
                ast::StmtKind::Let {
                    mutable,
                    name,
                    annotation,
                    init,
                } => {
                    let init = self.expression(*init)?;
                    let annotation = annotation
                        .map(|ty| value_type(&mut self.index.query(self.work), self.requester, ty))
                        .transpose()?;
                    let local =
                        self.bind(*name, annotation, *mutable, BodyBlockId(block.0), None)?;
                    scopes
                        .last_mut()
                        .expect("active body scope")
                        .push(self.text(*name));
                    StmtKind::Let {
                        binding: local,
                        init,
                    }
                }
                ast::StmtKind::Assign {
                    name,
                    operator_span,
                    value,
                } => {
                    let text = self.text(*name);
                    let local = self
                        .scope
                        .get(text)
                        .ok_or_else(|| {
                            diagnostic(
                                "E0200",
                                "resolve",
                                format_args!("unknown local `{}`", owned_diagnostic::name(text)),
                                Some(*name),
                            )
                        })?
                        .0;
                    StmtKind::Assign {
                        binding: local,
                        target_span: *name,
                        operator_span: *operator_span,
                        value: self.expression(*value)?,
                    }
                }
                ast::StmtKind::FieldAssign {
                    base,
                    field,
                    target_span,
                    operator_span,
                    value,
                } => {
                    let binding = self.lookup(*base)?;
                    StmtKind::FieldAssign {
                        base: binding,
                        base_span: *base,
                        field_span: *field,
                        target_span: *target_span,
                        operator_span: *operator_span,
                        value: self.expression(*value)?,
                    }
                }
                ast::StmtKind::IndexAssign {
                    target,
                    operator_span,
                    value,
                } => {
                    let target = &self.ast.expressions[target.0];
                    self.work.debit(1, target.span, "array resolve store")?;
                    let ast::ExprKind::IndexRead { base, index } = target.kind else {
                        return Err(error(
                            "E0500",
                            format_args!("invalid indexed assignment target"),
                            target.span,
                        ));
                    };
                    let binding = self.lookup(base)?;
                    // The retained AST target wrapper is syntax only, never a HIR read.
                    let value = self.expression(*value)?;
                    let index = self.expression(index)?;
                    StmtKind::IndexAssign {
                        base: binding,
                        base_span: base,
                        target_span: target.span,
                        operator_span: *operator_span,
                        value,
                        index,
                    }
                }
                ast::StmtKind::Expr(expr) => StmtKind::Expr(self.expression(*expr)?),
                ast::StmtKind::Match { .. } => {
                    return Err(error(
                        "E0101",
                        format_args!("enum source syntax is unavailable"),
                        statement.span,
                    ));
                }
                ast::StmtKind::Return(expr) => {
                    StmtKind::Return(expr.map(|expr| self.expression(expr)).transpose()?)
                }
                ast::StmtKind::Break | ast::StmtKind::Continue => {
                    let is_break = matches!(statement.kind, ast::StmtKind::Break);
                    let keyword = if is_break { "break" } else { "continue" };
                    let target = loops.last().copied().ok_or_else(|| {
                        diagnostic(
                            "E0204",
                            "resolve",
                            format_args!(
                                "`{keyword}` requires an enclosing while in the same function"
                            ),
                            Some(statement.span),
                        )
                    })?;
                    if is_break {
                        StmtKind::Break { target }
                    } else {
                        StmtKind::Continue { target }
                    }
                }
                ast::StmtKind::While { condition, body } => {
                    let condition = self.expression(*condition)?;
                    let loop_id = LoopId(body.0);
                    loops.push(loop_id);
                    frames.push(Frame::LeaveLoop);
                    frames.push(Frame::Enter(*body));
                    StmtKind::While {
                        loop_id,
                        condition,
                        body: BodyBlockId(body.0),
                    }
                }
                ast::StmtKind::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    let condition = self.expression(*condition)?;
                    if let Some(otherwise) = else_block {
                        frames.push(Frame::Enter(*otherwise));
                    }
                    frames.push(Frame::Enter(*then_block));
                    StmtKind::If {
                        condition,
                        then_block: BodyBlockId(then_block.0),
                        else_block: else_block.map(|id| BodyBlockId(id.0)),
                    }
                }
            };
            blocks[block.0].body.push(Stmt {
                kind,
                span: statement.span,
            });
        }
        Ok(Function {
            id,
            bindings: std::mem::take(&mut self.bindings),
            expressions: std::mem::take(&mut self.expressions),
            body: BodyBlockId(function.body.0),
            blocks,
            end: function.end,
        })
    }
    fn expression(&mut self, id: ast::ExprId) -> Result<ExprId, Box<Diagnostic>> {
        let expr = &self.ast.expressions[id.0];
        let kind = match &expr.kind {
            ast::ExprKind::QualifiedValue { .. } => {
                return Err(error(
                    "E0101",
                    format_args!("enum source syntax is unavailable"),
                    expr.span,
                ));
            }
            ast::ExprKind::ArrayLiteral { elements } => {
                self.work.debit(1, expr.span, "array resolve literal")?;
                let requested = checked_array_entries(*self.array_entries, elements.len())
                    .map_err(|failure| array_reserve_error(failure, expr.span))?;
                let mut resolved = Vec::new();
                self.allocator
                    .vector_exact(&mut resolved, elements.len(), "array HIR elements")
                    .map_err(|failure| array_reserve_error(failure, expr.span))?;
                *self.array_entries = requested;
                for element in elements {
                    self.work.debit(1, expr.span, "array resolve edge")?;
                    resolved.push(self.expression(*element)?);
                }
                ExprKind::ArrayLiteral { elements: resolved }
            }
            ast::ExprKind::IndexRead { base, index } => {
                self.work.debit(1, expr.span, "array resolve read")?;
                let binding = self.lookup(*base)?;
                let index = self.expression(*index)?;
                ExprKind::IndexRead {
                    base: binding,
                    base_span: *base,
                    index,
                }
            }
            ast::ExprKind::ArrayLength { base } => {
                self.work.debit(1, expr.span, "array resolve length")?;
                ExprKind::ArrayLength {
                    base: self.lookup(*base)?,
                    base_span: *base,
                }
            }
            ast::ExprKind::FieldRead { base, field } => ExprKind::FieldRead {
                base: self.lookup(*base)?,
                base_span: *base,
                field_span: *field,
            },
            ast::ExprKind::StructLiteral { record, fields } => {
                let path = *record;
                let at = self.index.sources().path_span(ItemPathRef {
                    file: expr.span.file,
                    path,
                })?;
                let record = self.index.query(self.work).record_type(
                    self.requester,
                    ItemPathRef {
                        file: expr.span.file,
                        path,
                    },
                    TypeContext::Constructor,
                )?;
                if let Access::Denied(field) =
                    self.index
                        .query(self.work)
                        .construction_access(self.requester, record, at)?
                {
                    let mut primary = at;
                    for init in fields {
                        if self
                            .index
                            .query(self.work)
                            .initializer_matches_field(field, init.name)?
                        {
                            primary = init.name;
                            break;
                        }
                    }
                    return Err(self
                        .index
                        .query(self.work)
                        .private_field_diagnostic(field, primary, "resolve")?);
                }
                let declared = &self.records[record.0];
                let mut seen = HashMap::new();
                let mut resolved = Vec::new();
                for field in fields {
                    let spelling = self.text(field.name);
                    let target = declared
                        .fields
                        .iter()
                        .find(|f| self.text(f.name_span) == spelling)
                        .ok_or_else(|| {
                            error(
                                "E0200",
                                format_args!(
                                    "unknown literal field `{}`",
                                    owned_diagnostic::name(spelling)
                                ),
                                field.name,
                            )
                        })?;
                    if let Some(first) = seen.insert(spelling, field.name) {
                        return Err(duplicate(field.name, first));
                    }
                    resolved.push(FieldInit {
                        field: target.id,
                        value: self.expression(field.value)?,
                        span: field.span,
                    });
                }
                #[cfg(test)]
                self.work
                    .observe(crate::frontend::declaration_index::Observation::Target {
                        operation: "constructor-resolved",
                        origin: at,
                        kind: "record",
                        id: record.0,
                    });
                ExprKind::StructLiteral {
                    record,
                    fields: resolved,
                }
            }
            ast::ExprKind::Bool(value) => ExprKind::Bool(*value),
            ast::ExprKind::Number { digits, negative } => {
                ExprKind::I32(decimal_i32(self.text(*digits), *negative, expr.span)?)
            }
            ast::ExprKind::Unit => ExprKind::Unit,
            ast::ExprKind::Name(span) => {
                let name = self.text(*span);
                let local = self.scope.get(name).ok_or_else(|| {
                    diagnostic(
                        "E0200",
                        "resolve",
                        format_args!("unknown local `{}`", owned_diagnostic::name(name)),
                        Some(*span),
                    )
                })?;
                ExprKind::Binding(local.0)
            }
            ast::ExprKind::Call { callee, args } => {
                let target = self.index.query(self.work).callee(
                    self.requester,
                    ItemPathRef {
                        file: expr.span.file,
                        path: *callee,
                    },
                    false,
                )?;
                let args = args
                    .iter()
                    .map(|arg| match arg {
                        ast::Argument::Value(id) => self.expression(*id).map(Argument::Value),
                        ast::Argument::Borrow {
                            mutable,
                            place,
                            span,
                        } => {
                            let (name_span, star_span) = match place {
                                ast::BorrowPlace::OwnerName(name) => (*name, None),
                                ast::BorrowPlace::ForwardedParameter { name, star_span } => {
                                    (*name, Some(*star_span))
                                }
                            };
                            let binding = self.lookup(name_span)?;
                            Ok(Argument::Borrow {
                                kind: if *mutable {
                                    BorrowKind::Exclusive
                                } else {
                                    BorrowKind::Shared
                                },
                                place: if star_span.is_some() {
                                    BorrowPlace::Forwarded(binding)
                                } else {
                                    BorrowPlace::Owner(binding)
                                },
                                span: *span,
                                name_span,
                                star_span,
                            })
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                ExprKind::Call { target, args }
            }
            ast::ExprKind::Group(inner) => ExprKind::Group(self.expression(*inner)?),
            ast::ExprKind::Negate {
                operand,
                operator_span,
            } => ExprKind::Negate {
                operand: self.expression(*operand)?,
                operator_span: *operator_span,
            },
            ast::ExprKind::Not {
                operand,
                operator_span,
            } => ExprKind::Not {
                operand: self.expression(*operand)?,
                operator_span: *operator_span,
            },
            ast::ExprKind::Logical {
                op,
                left,
                right,
                operator_span,
            } => {
                let left = self.expression(*left)?;
                let right = self.expression(*right)?;
                ExprKind::Logical {
                    op: *op,
                    left,
                    right,
                    operator_span: *operator_span,
                }
            }
            ast::ExprKind::Comparison {
                op,
                left,
                right,
                operator_span,
            } => {
                let left = self.expression(*left)?;
                let right = self.expression(*right)?;
                ExprKind::Comparison {
                    op: *op,
                    left,
                    right,
                    operator_span: *operator_span,
                }
            }
            ast::ExprKind::Arithmetic {
                op,
                left,
                right,
                operator_span,
            } => {
                // Complete the left subtree before starting the right, including calls.
                let left = self.expression(*left)?;
                let right = self.expression(*right)?;
                ExprKind::Arithmetic {
                    op: *op,
                    left,
                    right,
                    operator_span: *operator_span,
                }
            }
        };
        let id = ExprId(self.expressions.len());
        self.expressions.push(Expr {
            kind,
            span: expr.span,
        });
        Ok(id)
    }
}

#[cfg(test)]
mod source_identity_tests {
    use super::*;
    #[test]
    fn projection_payload_admission_is_cumulative_and_checked_before_growth() {
        let mut map = SourceMap::new();
        let file = map.add("paths.ox".into(), "struct C{}".into());
        let source = map.get(file);
        let ast = crate::frontend::parser::parse_with_mode(
            source,
            crate::frontend::lexer::lex(source).unwrap(),
            crate::frontend::parser::SourceMode::OwnedCandidate,
        )
        .unwrap();
        let program = resolve(source, &ast).unwrap();
        let span = source.span(0, 0);
        let maximum = super::super::budget::MAX_RAW_BYTES;
        program
            .projection_bytes
            .set(maximum - std::mem::size_of::<FieldId>());
        program.admit_projection(1, span).unwrap();
        assert_eq!(program.admit_projection(1, span).unwrap_err().code, "E0400");
        assert_eq!(program.projection_bytes.get(), maximum);
        program.projection_bytes.set(0);
        assert_eq!(
            program.admit_projection(65, span).unwrap_err().code,
            "E0400"
        );
        assert_eq!(program.projection_bytes.get(), 0);
        let metadata = std::mem::size_of::<Vec<FieldId>>();
        program.admit_projection_metadata(metadata, span).unwrap();
        program.admit_projection(2, span).unwrap();
        assert_eq!(
            program.projection_bytes.get(),
            metadata + 2 * std::mem::size_of::<FieldId>()
        );
        assert_eq!(
            program
                .admit_projection_metadata(usize::MAX, span)
                .unwrap_err()
                .code,
            "E0400"
        );
        assert_eq!(
            program.projection_bytes.get(),
            metadata + 2 * std::mem::size_of::<FieldId>()
        );
    }
    #[test]
    fn resolved_owned_names_select_each_original_file() {
        let mut map = SourceMap::new();
        let first = map.add("first.ox".into(), "fn old() -> () { return; }".into());
        let second = map.add(
            "second.ox".into(),
            "struct C {} fn main() -> () { return; }".into(),
        );
        let file = map.get(second);
        let ast = crate::frontend::parser::parse(file, crate::frontend::lexer::lex(file).unwrap())
            .unwrap();
        let program = resolve_in_map(file, &ast, &map).unwrap();
        assert_eq!(program.text(map.get(first).span(3, 6)), "old");
        assert_eq!(program.text(map.get(second).span(7, 8)), "C");
        assert_eq!(program.index().function(DefId(0)).unwrap().0.file, second);
    }
}

// Cumulative requested slots include pending outer buffers and never reset at
// function boundaries. These are checks, not a trusted allocation inventory.
fn checked_array_entries(current: usize, length: usize) -> Result<usize, ReserveFailure> {
    if length > 1024 {
        return Err(ReserveFailure::Overflow);
    }
    let total = current
        .checked_add(length)
        .ok_or(ReserveFailure::Overflow)?;
    total
        .checked_mul(std::mem::size_of::<ExprId>())
        .ok_or(ReserveFailure::Overflow)?;
    Ok(total)
}
fn array_reserve_error(failure: ReserveFailure, span: Span) -> Box<Diagnostic> {
    error(
        "E0400",
        format_args!(
            "{}",
            match failure {
                ReserveFailure::Overflow => "array HIR count overflow",
                ReserveFailure::Allocation => "array HIR allocation failed",
            }
        ),
        span,
    )
}

/// Private observation construction only. Callers must copy inert facts before
/// typing consumes this owner; no executable entrypoint chooses this policy.
#[cfg(test)]
pub(super) fn resolve_array_types<'s>(
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<ResolvedOwnedProgram<'s>, Vec<Diagnostic>> {
    let (index, (records, signatures, functions)) =
        resolve_source_parts(sources, work, allocator, limits)?;
    let entry = index.root_original_main();
    Ok(ResolvedOwnedProgram {
        projection_bytes: std::cell::Cell::new(0),
        admission: SourceAdmission::ObserveArrayTypes,
        sources: sources.view(),
        index: IndexOwner::Owned(index),
        work: MeterOwner::Borrowed(work),
        records,
        signatures,
        functions,
        entry,
    })
}

/// Only the separate source-only test entry chooses this immutable policy.
/// A types-only owner cannot be converted or forwarded into this construction.
#[cfg(test)]
pub(super) fn resolve_array_pipeline<'s>(
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<ResolvedOwnedProgram<'s>, Vec<Diagnostic>> {
    let (index, (records, signatures, functions)) =
        resolve_source_parts(sources, work, allocator, limits)?;
    let entry = index.root_original_main();
    Ok(ResolvedOwnedProgram {
        projection_bytes: std::cell::Cell::new(0),
        admission: SourceAdmission::ObserveArrayPipeline,
        sources: sources.view(),
        index: IndexOwner::Owned(index),
        work: MeterOwner::Borrowed(work),
        records,
        signatures,
        functions,
        entry,
    })
}

/// A fresh source owner is required for consumer qualification. Existing
/// observation admissions remain immutable and cannot be promoted.
#[cfg(test)]
pub(super) fn resolve_array_consumer<'s>(
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &'s WorkMeter,
    allocator: &mut Allocator,
) -> Result<ResolvedOwnedProgram<'s>, Vec<Diagnostic>> {
    let (index, (records, signatures, functions)) =
        resolve_source_parts(sources, work, allocator, limits)?;
    let entry = index.root_original_main();
    Ok(ResolvedOwnedProgram {
        projection_bytes: std::cell::Cell::new(0),
        admission: SourceAdmission::ArrayConsumer,
        sources: sources.view(),
        index: IndexOwner::Owned(index),
        work: MeterOwner::Borrowed(work),
        records,
        signatures,
        functions,
        entry,
    })
}

#[cfg(test)]
mod array_reservation_tests {
    use super::*;
    #[test]
    fn unit3b1_checked_array_entry_arithmetic_is_cumulative() {
        assert_eq!(checked_array_entries(0, 0), Ok(0));
        assert_eq!(checked_array_entries(2, 3), Ok(5));
        assert_eq!(checked_array_entries(0, 1024), Ok(1024));
        for (control, current, length) in [
            ("length", 0, 1025),
            ("cumulative", usize::MAX, 1),
            ("multiply", usize::MAX / std::mem::size_of::<ExprId>(), 1),
        ] {
            let result = checked_array_entries(current, length);
            println!("CONTROL {{\"schema\":\"oxid-array-types-controls-v1\",\"kind\":\"overflow\",\"control\":\"{control}\",\"synthetic\":true,\"current\":{current},\"length\":{length},\"element_bytes\":{},\"result\":\"{:?}\",\"reservation_attempts\":0}}", std::mem::size_of::<ExprId>(), result);
            assert_eq!(result, Err(ReserveFailure::Overflow));
        }
        let mut allocator = Allocator {
            attempts: usize::MAX,
            ..Allocator::default()
        };
        let mut values = Vec::<ExprId>::new();
        let result = allocator.vector_exact(&mut values, 0, "array HIR elements");
        println!("CONTROL {{\"schema\":\"oxid-array-types-controls-v1\",\"kind\":\"overflow\",\"control\":\"attempt\",\"synthetic\":true,\"attempt_counter\":{},\"result\":\"{:?}\",\"trace_rows\":{}}}", allocator.attempts, result, allocator.trace.len());
        assert_eq!(result, Err(ReserveFailure::Overflow));
        assert!(allocator.trace.is_empty() && values.is_empty() && values.capacity() == 0);
    }
}
