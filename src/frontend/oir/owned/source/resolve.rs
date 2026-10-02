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
    project::{budget::Allocator, ItemPathRef, ModuleId},
    source::{SourceFile, SourceMap, SourceView, Span},
};
use std::collections::HashMap;

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
    index: IndexOwner<'src>,
    work: MeterOwner<'src>,
    sources: SourceView<'src>,
    records: Vec<Record>,
    signatures: Vec<Signature>,
    functions: Vec<Function>,
    entry: Option<DefId>,
}
impl<'src> ResolvedOwnedProgram<'src> {
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
fn parameter_type(
    query: &mut QuerySession<'_, '_>,
    requester: ModuleId,
    ty: ast::TypeSyntax,
) -> Result<ParameterTy, Box<Diagnostic>> {
    if let ast::TypeSyntaxKind::Reference { mutable, referent } = ty.kind {
        let record = query.record_type(
            requester,
            ItemPathRef {
                file: ty.span.file,
                path: referent,
            },
            TypeContext::Reference,
        )?;
        Ok(ParameterTy::Reference {
            record,
            kind: if mutable {
                BorrowKind::Exclusive
            } else {
                BorrowKind::Shared
            },
        })
    } else {
        value_type(query, requester, ty).map(ParameterTy::Value)
    }
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
        resolve_source_parts(sources, &work, &mut allocator)?;
    let entry = index.root_original_main();
    Ok(ResolvedOwnedProgram {
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
    let (index, (records, signatures, functions)) = resolve_source_parts(sources, work, allocator)?;
    let entry = index.root_original_main();
    Ok(ResolvedOwnedProgram {
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
) -> Result<(DeclarationIndex<'s>, ResolvedParts), Vec<Diagnostic>> {
    let facts = index::collect_originals(sources, IndexLimits::default(), work, allocator)
        .map_err(|e| vec![*e])?;
    let index = facts.finish(work, allocator)?;
    let parts = resolve_index(&index, work)?;
    Ok((index, parts))
}
pub(in crate::frontend) fn resolve_project<'s>(
    index: &'s DeclarationIndex<'s>,
    work: &'s WorkMeter,
) -> Result<ResolvedOwnedProgram<'s>, Vec<Diagnostic>> {
    let (records, signatures, functions) = resolve_index(index, work)?;
    Ok(ResolvedOwnedProgram {
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
                let ValueTy::Scalar(ty) = value_type(&mut index.query(work), module, field.ty)?
                else {
                    return Err(error(
                        "E0202",
                        format_args!("record fields must have scalar bool, i32 or () type"),
                        field.ty.span,
                    ));
                };
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
                        ParameterTy::Value(ValueTy::Owned(r))
                        | ParameterTy::Reference { record: r, .. } => Some(r),
                        _ => None,
                    },
                    &p.ty,
                )
            });
        let result = (
            match signature.result {
                ValueTy::Owned(r) => Some(r),
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
    for id in 0..index.function_count() {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
        let (key, requester) = index.function(DefId(id)).map_err(|e| vec![*e])?;
        let ast = sources.ast(requester).map_err(|e| vec![*e])?;
        let mut resolver = Resolver {
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
        self.scope
            .get(self.text(span))
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
                ast::StmtKind::Expr(expr) => StmtKind::Expr(self.expression(*expr)?),
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
