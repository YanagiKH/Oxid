//! Complete declaration/name resolution. No ownership or loan analysis.
use super::hir::*;
use crate::frontend::{
    ast,
    diagnostic::Diagnostic,
    owned_diagnostic::{self, diagnostic, name, secondary},
    parser::MAX_DIAGNOSTICS,
    source::{SourceFile, Span},
};
use std::collections::HashMap;

#[derive(Debug)]
pub(super) struct ResolvedOwnedProgram<'src> {
    source_text: &'src str,
    records: Vec<Record>,
    signatures: Vec<Signature>,
    functions: Vec<Function>,
    entry: Option<DefId>,
}
impl ResolvedOwnedProgram<'_> {
    pub(super) fn text(&self, span: Span) -> &str {
        &self.source_text[span.start..span.end]
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
    &source.text()[span.start..span.end]
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
    source: &SourceFile,
    ty: ast::TypeSyntax,
    records: &HashMap<&str, (RecordId, Span)>,
) -> Result<ValueTy, Box<Diagnostic>> {
    match ty.kind {
        ast::TypeSyntaxKind::Unit => Ok(ValueTy::Scalar(Ty::Unit)),
        ast::TypeSyntaxKind::Name(span) => match text(source, span) {
            "bool" => Ok(ValueTy::Scalar(Ty::Bool)),
            "i32" => Ok(ValueTy::Scalar(Ty::I32)),
            spelling => records
                .get(spelling)
                .map(|entry| ValueTy::Owned(entry.0))
                .ok_or_else(|| {
                    error(
                        "E0202",
                        format_args!("unknown type `{}`", name(spelling)),
                        span,
                    )
                }),
        },
        ast::TypeSyntaxKind::Reference { .. } => Err(error(
            "E0202",
            format_args!("reference types are restricted to parameters"),
            ty.span,
        )),
    }
}
fn parameter_type(
    source: &SourceFile,
    ty: ast::TypeSyntax,
    records: &HashMap<&str, (RecordId, Span)>,
) -> Result<ParameterTy, Box<Diagnostic>> {
    if let ast::TypeSyntaxKind::Reference { mutable, referent } = ty.kind {
        let record = records
            .get(text(source, referent))
            .ok_or_else(|| {
                error(
                    "E0202",
                    format_args!(
                        "reference parameter requires a record type, found `{}`",
                        name(text(source, referent))
                    ),
                    referent,
                )
            })?
            .0;
        Ok(ParameterTy::Reference {
            record,
            kind: if mutable {
                BorrowKind::Exclusive
            } else {
                BorrowKind::Shared
            },
        })
    } else {
        value_type(source, ty, records).map(ParameterTy::Value)
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

pub(super) fn resolve<'src>(
    source: &'src SourceFile,
    ast: &ast::Program,
) -> Result<ResolvedOwnedProgram<'src>, Vec<Diagnostic>> {
    // Match the already qualified declaration envelope before name/body work.
    // These producer checks do not replace independent raw declaration checks.
    if crate::frontend::oir::owned_types::admit_declaration_counts(
        ast.records.iter().map(|record| record.fields.len()),
    )
    .is_err()
    {
        return Err(vec![*error(
            "E0400",
            format_args!("owned declaration resource limit exceeded"),
            source.span(0, 0),
        )]);
    }
    let mut names = HashMap::new();
    let mut record_names = HashMap::new();
    let mut diagnostics = Vec::new();
    for item in &ast.items {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
        let result = match *item {
            ast::ItemId::Function(index) => {
                let span = ast.functions[index].name;
                match names.entry(text(source, span)) {
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert((DefId(index), span));
                        Ok(())
                    }
                    std::collections::hash_map::Entry::Occupied(entry) => {
                        Err(duplicate(span, entry.get().1))
                    }
                }
            }
            ast::ItemId::Struct(index) => {
                let span = ast.records[index].name;
                if matches!(text(source, span), "bool" | "i32") {
                    Err(error(
                        "E0202",
                        format_args!("scalar type names cannot be redeclared"),
                        span,
                    ))
                } else {
                    match record_names.entry(text(source, span)) {
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            entry.insert((RecordId(index), span));
                            Ok(())
                        }
                        std::collections::hash_map::Entry::Occupied(entry) => {
                            Err(duplicate(span, entry.get().1))
                        }
                    }
                }
            }
        };
        if let Err(error) = result {
            diagnostics.push(*error);
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let mut records = Vec::new();
    for (index, record) in ast.records.iter().enumerate() {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
        let result = (|| {
            let mut fields = Vec::new();
            let mut names = HashMap::new();
            for field in &record.fields {
                if let Some(first) = names.insert(text(source, field.name), field.name) {
                    return Err(duplicate(field.name, first));
                }
                let ValueTy::Scalar(ty) = value_type(source, field.ty, &record_names)? else {
                    return Err(error(
                        "E0202",
                        format_args!("record fields must have scalar bool, i32 or () type"),
                        field.ty.span,
                    ));
                };
                fields.push(Field {
                    id: FieldId {
                        record: RecordId(index),
                        index: fields.len(),
                    },
                    ty,
                    name_span: field.name,
                    span: field.span,
                });
            }
            Ok(Record {
                id: RecordId(index),
                name_span: record.name,
                span: record.span,
                end: record.end,
                fields,
            })
        })();
        match result {
            Ok(record) => records.push(record),
            Err(error) => diagnostics.push(*error),
        }
    }
    let mut signatures = Vec::new();
    for function in &ast.functions {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
        let result: Result<Signature, Box<Diagnostic>> = (|| {
            let params = function
                .params
                .iter()
                .map(|p| parameter_type(source, p.ty, &record_names))
                .collect::<Result<Vec<_>, _>>()?;
            let result = value_type(source, function.result, &record_names)?;
            for block in &function.blocks {
                for statement in &block.body {
                    if let ast::StmtKind::Let {
                        annotation: Some(ty),
                        ..
                    } = statement.kind
                    {
                        value_type(source, ty, &record_names)?;
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
            Err(error) => diagnostics.push(*error),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let mut functions = Vec::new();
    for (index, function) in ast.functions.iter().enumerate() {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
        let mut resolver = Resolver {
            source,
            ast,
            names: &names,
            record_names: &record_names,
            records: &records,
            scope: HashMap::new(),
            bindings: Vec::new(),
            expressions: Vec::new(),
        };
        match resolver.function(DefId(index), function) {
            Ok(function) => functions.push(function),
            Err(error) => diagnostics.push(*error),
        }
    }
    if diagnostics.is_empty() {
        Ok(ResolvedOwnedProgram {
            source_text: source.text(),
            records,
            signatures,
            functions,
            entry: names.get("main").map(|e| e.0),
        })
    } else {
        Err(diagnostics)
    }
}
struct Resolver<'a> {
    source: &'a SourceFile,
    ast: &'a ast::Program,
    names: &'a HashMap<&'a str, (DefId, Span)>,
    record_names: &'a HashMap<&'a str, (RecordId, Span)>,
    records: &'a [Record],
    scope: HashMap<&'a str, (BindingId, Span)>,
    bindings: Vec<Binding>,
    expressions: Vec<Expr>,
}
impl<'a> Resolver<'a> {
    fn text(&self, span: Span) -> &'a str {
        &self.source.text()[span.start..span.end]
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
        if let Some((_, previous)) = self.names.get(name) {
            return Err(duplicate(span, *previous));
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
                        .map(|ty| value_type(self.source, ty, self.record_names))
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
                let record = self
                    .record_names
                    .get(self.text(*record))
                    .ok_or_else(|| {
                        error(
                            "E0202",
                            format_args!(
                                "unknown record type `{}`",
                                owned_diagnostic::name(self.text(*record))
                            ),
                            *record,
                        )
                    })?
                    .0;
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
                let name = self.text(*callee);
                let target = self
                    .names
                    .get(name)
                    .ok_or_else(|| {
                        diagnostic(
                            "E0200",
                            "resolve",
                            format_args!(
                                "unknown direct function `{}`",
                                owned_diagnostic::name(name)
                            ),
                            Some(*callee),
                        )
                    })?
                    .0;
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
