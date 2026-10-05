//! Exhaustive, allocation-free tuple projection of actual immutable compiler rows.
use super::*;

pub(super) struct Json<'a>(pub &'a str);
fn escape(out: &mut impl Write, text: &str) -> fmt::Result {
    for ch in text.chars() {
        match ch {
            '"' => out.write_str("\\\"")?,
            '\\' => out.write_str("\\\\")?,
            '\u{8}' => out.write_str("\\b")?,
            '\t' => out.write_str("\\t")?,
            '\n' => out.write_str("\\n")?,
            '\u{c}' => out.write_str("\\f")?,
            '\r' => out.write_str("\\r")?,
            '\u{0}'..='\u{1f}' => write!(out, "\\u{:04x}", ch as u32)?,
            _ => out.write_char(ch)?,
        }
    }
    Ok(())
}
impl fmt::Display for Json<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"")?;
        escape(f, self.0)?;
        f.write_str("\"")
    }
}
pub(super) struct Optional<T>(pub Option<T>);
impl<T: fmt::Display> fmt::Display for Optional<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(value) => value.fmt(f),
            None => f.write_str("null"),
        }
    }
}
pub(super) struct SpanRow(pub Span);
impl fmt::Display for SpanRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{},{},{}]", self.0.file.0, self.0.start, self.0.end)
    }
}
struct ScalarType(hir::Ty);
impl fmt::Display for ScalarType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Json(match self.0 {
            hir::Ty::Bool => "bool",
            hir::Ty::I32 => "i32",
            hir::Ty::Unit => "unit",
        })
        .fmt(f)
    }
}
struct Aggregate(AggregateTy);
impl fmt::Display for Aggregate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            AggregateTy::Record(id) => write!(f, "[\"record\",{}]", id.0),
            AggregateTy::Enum(_) => unreachable!("enum source gate"),
            AggregateTy::FixedArray(array) => write!(
                f,
                "[\"fixed-array\",{},{}]",
                ScalarType(array.element()),
                array.length()
            ),
        }
    }
}
struct Borrowed(BorrowedTy);
impl fmt::Display for Borrowed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            BorrowedTy::Exact(aggregate) => Aggregate(aggregate).fmt(f),
            BorrowedTy::ScalarSlice(element) => {
                write!(f, "[\"scalar-slice\",{}]", ScalarType(element))
            }
        }
    }
}
struct ValueType(ValueTy);
impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            ValueTy::Scalar(ty) => write!(f, "[\"scalar\",{}]", ScalarType(ty)),
            ValueTy::Owned(ty) => write!(f, "[\"owned\",{}]", Aggregate(ty)),
        }
    }
}
struct FieldValueType(ValueTy);
impl fmt::Display for FieldValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            ValueTy::Scalar(ty) => ScalarType(ty).fmt(f),
            ValueTy::Owned(ty) => Aggregate(ty).fmt(f),
        }
    }
}
struct Borrow(BorrowKind);
impl fmt::Display for Borrow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Json(match self.0 {
            BorrowKind::Shared => "shared",
            BorrowKind::Exclusive => "exclusive",
        })
        .fmt(f)
    }
}
struct ParameterType(ParameterTy);
impl fmt::Display for ParameterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            ParameterTy::Value(ty) => write!(f, "[\"value\",{}]", ValueType(ty)),
            ParameterTy::Reference { referent, kind } => {
                write!(f, "[\"reference\",{},{}]", Borrowed(referent), Borrow(kind))
            }
        }
    }
}
pub(super) struct CountsRow(pub raw_budget::FunctionCounts);
impl fmt::Display for CountsRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = self.0;
        write!(
            f,
            "[{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}]",
            c.locals,
            c.places,
            c.owners,
            c.references,
            c.parameters,
            c.calls,
            c.loans,
            c.blocks,
            c.edges,
            c.statements,
            c.merges,
            c.descriptor_arguments,
            c.preparations,
            c.constructed_fields,
            c.constructed_elements,
            c.diagnostic_origins,
            c.max_constructor_fields,
            c.ownership_active
        )
    }
}
pub(super) struct UsageRow(pub OwnershipUsage);
impl fmt::Display for UsageRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let u = self.0;
        write!(
            f,
            "[{},{},{},{},{},{},{}]",
            u.owners,
            u.expanded_events,
            u.work,
            u.scratch_bytes,
            u.metadata_bytes,
            u.owner_cells,
            u.owner_layout_bytes
        )
    }
}
fn add(a: &mut usize, b: usize) -> ObservationResult<()> {
    *a = a.checked_add(b).ok_or("projection count overflow")?;
    Ok(())
}
pub(super) fn sources(out: &mut Output, sources: &SourceMap) -> ObservationResult<()> {
    for (id, source) in sources.files().iter().enumerate() {
        out.row(
            "source",
            format_args!(
                "{},{},{},{},{},{}",
                id,
                source.identity(),
                Json(if id == 0 { "root" } else { "child" }),
                Json(source.path()),
                source.text().len(),
                Json(source.text())
            ),
        )?;
    }
    Ok(())
}
pub(super) fn modules(
    out: &mut Output,
    modules: &[crate::frontend::project::ModuleHeader],
) -> ObservationResult<()> {
    for (id, module) in modules.iter().enumerate() {
        out.row(
            "module",
            format_args!(
                "{},{},{},{},{},{},{},{}",
                id,
                module.file.0,
                Optional(module.parent.map(|id| id.0)),
                Optional(module.declaration.map(SpanRow)),
                Optional(module.public.map(SpanRow)),
                module.depth,
                Json(&module.relative_path),
                Json("project-header")
            ),
        )?;
    }
    Ok(())
}
pub(super) fn original_module(out: &mut Output, source: &SourceFile) -> ObservationResult<()> {
    out.row(
        "module",
        format_args!(
            "0,0,null,null,null,0,{},{}",
            Json(source.path()),
            Json("single-source-adapter")
        ),
    )
}
pub(super) fn ast_inventory(out: &mut Output, owner: SourceOwner<'_>) -> ObservationResult<()> {
    if out.control.resource_only() {
        return Ok(());
    }
    let mut expressions = 0;
    let mut elements = 0;
    let mut targets = 0;
    for module in 0..owner.count() {
        let ast = owner
            .ast(ModuleId(module))
            .map_err(|_| "original AST unavailable")?;
        add(&mut expressions, ast.expressions.len())?;
        for expression in &ast.expressions {
            if let ast::ExprKind::ArrayLiteral { elements: children } = &expression.kind {
                add(&mut elements, children.len())?;
            }
        }
        for function in &ast.functions {
            for body in &function.blocks {
                for statement in &body.body {
                    if matches!(statement.kind, ast::StmtKind::IndexAssign { .. }) {
                        add(&mut targets, 1)?;
                    }
                }
            }
        }
    }
    out.ast_elements = Some(elements);
    out.row(
        "ast-inventory",
        format_args!("{},{},{},{}", owner.count(), expressions, elements, targets),
    )
}
pub(super) fn trace(out: &mut Output, allocator: &Allocator) -> ObservationResult<()> {
    if allocator.observer_trace_overflow {
        return Err("frontend trace incomplete");
    }
    if allocator.observer_trace_limit != Some(out.limits.trace_rows) {
        return Err("frontend trace bound lost");
    }
    if !out.control.resource_only() {
        for (ordinal, row) in allocator.trace.iter().enumerate() {
            out.row(
                "frontend-reservation",
                format_args!(
                    "{},{},{},{},{}",
                    ordinal + 1,
                    Json(row.kind),
                    row.length,
                    row.element_bytes,
                    row.success
                ),
            )?;
        }
    }
    let trace_rows = out.limits.trace_rows;
    out.row(
        "frontend-trace",
        format_args!(
            "{},{},{},{}",
            allocator.attempts,
            allocator.trace.len(),
            trace_rows,
            size_of::<crate::frontend::project::budget::ReserveEvent>()
        ),
    )
}

struct Field(FieldId);
impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{},{}]", self.0.record.0, self.0.index)
    }
}
fn arithmetic(op: source_hir::ArithmeticOp) -> &'static str {
    match op {
        source_hir::ArithmeticOp::Add => "add",
        source_hir::ArithmeticOp::Subtract => "sub",
        source_hir::ArithmeticOp::Multiply => "mul",
        source_hir::ArithmeticOp::Divide => "div",
        source_hir::ArithmeticOp::Remainder => "rem",
    }
}
fn comparison(op: source_hir::ComparisonOp) -> &'static str {
    match op {
        source_hir::ComparisonOp::Equal => "eq",
        source_hir::ComparisonOp::NotEqual => "ne",
        source_hir::ComparisonOp::Less => "lt",
        source_hir::ComparisonOp::LessEqual => "le",
        source_hir::ComparisonOp::Greater => "gt",
        source_hir::ComparisonOp::GreaterEqual => "ge",
    }
}
struct Expression<'a>(&'a source_hir::ExprKind);
impl fmt::Display for Expression<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use source_hir::ExprKind as E;
        match self.0 {
            E::Bool(value) => write!(f, "[\"bool\",{value}]"),
            E::I32(value) => write!(f, "[\"i32\",{value}]"),
            E::Unit => f.write_str("[\"unit\"]"),
            E::Binding(id) => write!(f, "[\"binding\",{}]", id.0),
            E::Group(id) => write!(f, "[\"group\",{}]", id.0),
            E::Negate {
                operand,
                operator_span,
            } => write!(f, "[\"negate\",{},{}]", operand.0, SpanRow(*operator_span)),
            E::Not {
                operand,
                operator_span,
            } => write!(f, "[\"not\",{},{}]", operand.0, SpanRow(*operator_span)),
            E::Logical {
                op,
                left,
                right,
                operator_span,
            } => write!(
                f,
                "[\"logical\",{},{},{},{}]",
                Json(match op {
                    source_hir::LogicalOp::And => "and",
                    source_hir::LogicalOp::Or => "or",
                }),
                left.0,
                right.0,
                SpanRow(*operator_span)
            ),
            E::Comparison {
                op,
                left,
                right,
                operator_span,
            } => write!(
                f,
                "[\"comparison\",{},{},{},{}]",
                Json(comparison(*op)),
                left.0,
                right.0,
                SpanRow(*operator_span)
            ),
            E::Arithmetic {
                op,
                left,
                right,
                operator_span,
            } => write!(
                f,
                "[\"arithmetic\",{},{},{},{}]",
                Json(arithmetic(*op)),
                left.0,
                right.0,
                SpanRow(*operator_span)
            ),
            E::Call { target, args } => write!(f, "[\"call\",{},{}]", target.0, args.len()),
            E::StructLiteral { record, fields } => {
                write!(f, "[\"record-literal\",{},{}]", record.0, fields.len())
            }
            E::ArrayLiteral { elements } => write!(f, "[\"array-literal\",{}]", elements.len()),
            E::IndexRead {
                base,
                base_span,
                index,
            } => write!(
                f,
                "[\"index-read\",{},{},{}]",
                base.0,
                SpanRow(*base_span),
                index.0
            ),
            E::ArrayLength { base, base_span } => {
                write!(f, "[\"array-length\",{},{}]", base.0, SpanRow(*base_span))
            }
            E::FieldRead {
                base,
                base_span,
                field_span,
            } => write!(
                f,
                "[\"field-read\",{},{},{}]",
                base.0,
                SpanRow(*base_span),
                SpanRow(*field_span)
            ),
        }
    }
}
struct HirArgument<'a>(&'a source_hir::Argument);
impl fmt::Display for HirArgument<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            source_hir::Argument::Value(id) => write!(f, "[\"value\",{}]", id.0),
            source_hir::Argument::Borrow {
                kind,
                place,
                span,
                name_span,
                star_span,
            } => {
                let (category, id) = match place {
                    source_hir::BorrowPlace::Owner(id) => ("owner", id.0),
                    source_hir::BorrowPlace::Forwarded(id) => ("forwarded", id.0),
                };
                write!(
                    f,
                    "[\"borrow\",{},{},{},{},{},{}]",
                    Borrow(*kind),
                    Json(category),
                    id,
                    SpanRow(*span),
                    SpanRow(*name_span),
                    Optional(star_span.map(SpanRow))
                )
            }
        }
    }
}
struct HirStatement<'a>(&'a source_hir::StmtKind);
impl fmt::Display for HirStatement<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use source_hir::StmtKind as S;
        match self.0 {
            S::Let { binding, init } => write!(f, "[\"let\",{},{}]", binding.0, init.0),
            S::Assign {
                binding,
                target_span,
                operator_span,
                value,
            } => write!(
                f,
                "[\"assign\",{},{},{},{}]",
                binding.0,
                SpanRow(*target_span),
                SpanRow(*operator_span),
                value.0
            ),
            S::FieldAssign {
                base,
                base_span,
                field_span,
                target_span,
                operator_span,
                value,
            } => write!(
                f,
                "[\"field-assign\",{},{},{},{},{},{}]",
                base.0,
                SpanRow(*base_span),
                SpanRow(*field_span),
                SpanRow(*target_span),
                SpanRow(*operator_span),
                value.0
            ),
            S::IndexAssign {
                base,
                base_span,
                target_span,
                operator_span,
                value,
                index,
            } => write!(
                f,
                "[\"index-assign\",{},{},{},{},{},{}]",
                base.0,
                SpanRow(*base_span),
                SpanRow(*target_span),
                SpanRow(*operator_span),
                value.0,
                index.0
            ),
            S::Expr(id) => write!(f, "[\"expr\",{}]", id.0),
            S::Return(id) => write!(f, "[\"return\",{}]", Optional(id.map(|id| id.0))),
            S::Break { target } => write!(f, "[\"break\",{}]", target.0),
            S::Continue { target } => write!(f, "[\"continue\",{}]", target.0),
            S::While {
                loop_id,
                condition,
                body,
            } => write!(f, "[\"while\",{},{},{}]", loop_id.0, condition.0, body.0),
            S::If {
                condition,
                then_block,
                else_block,
            } => write!(
                f,
                "[\"if\",{},{},{}]",
                condition.0,
                then_block.0,
                Optional(else_block.map(|id| id.0))
            ),
        }
    }
}
pub(super) fn resolved(
    out: &mut Output,
    resolved: &resolve::ResolvedOwnedProgram<'_>,
) -> ObservationResult<()> {
    if out.control.resource_only() {
        return Ok(());
    }
    let mut elements = 0;
    for record in resolved.records() {
        let (_, module) = resolved
            .index()
            .record(record.id)
            .map_err(|_| "record index metadata missing")?;
        out.row(
            "hir-record",
            format_args!(
                "{},{},{},{},{},{}",
                record.id.0,
                module.0,
                SpanRow(record.name_span),
                SpanRow(record.span),
                SpanRow(record.end),
                record.fields.len()
            ),
        )?;
        for field in &record.fields {
            out.row(
                "hir-field",
                format_args!(
                    "{},{},{},{},{}",
                    Field(field.id),
                    FieldValueType(field.ty),
                    SpanRow(field.name_span),
                    SpanRow(field.span),
                    record.id.0
                ),
            )?;
        }
    }
    for (id, signature) in resolved.signatures().iter().enumerate() {
        out.row(
            "hir-signature",
            format_args!(
                "{},{},{},{}",
                id,
                SpanRow(signature.span),
                ValueType(signature.result),
                signature.params.len()
            ),
        )?;
        for (position, ty) in signature.params.iter().enumerate() {
            out.row(
                "hir-parameter",
                format_args!("{id},{position},{}", ParameterType(*ty)),
            )?;
        }
    }
    for function in resolved.functions() {
        let id = function.id.0;
        let (_, module) = resolved
            .index()
            .function(function.id)
            .map_err(|_| "function index metadata missing")?;
        out.row(
            "hir-function",
            format_args!(
                "{id},{},{},{},{},{},{}",
                module.0,
                function.body.0,
                SpanRow(function.end),
                function.bindings.len(),
                function.expressions.len(),
                function.blocks.len()
            ),
        )?;
        for (binding, b) in function.bindings.iter().enumerate() {
            out.row(
                "hir-binding",
                format_args!(
                    "{id},{binding},{},{},{},{},{}",
                    b.mutable,
                    SpanRow(b.span),
                    Optional(b.annotation.map(ValueType)),
                    b.scope.0,
                    Optional(b.parameter_position)
                ),
            )?;
        }
        for (body, b) in function.blocks.iter().enumerate() {
            out.row(
                "hir-body",
                format_args!(
                    "{id},{body},{},{},{}",
                    SpanRow(b.span),
                    SpanRow(b.end),
                    b.body.len()
                ),
            )?;
            for (position, s) in b.body.iter().enumerate() {
                out.row(
                    "hir-statement",
                    format_args!(
                        "{id},{body},{position},{},{}",
                        SpanRow(s.span),
                        HirStatement(&s.kind)
                    ),
                )?;
            }
        }
        for (expression, e) in function.expressions.iter().enumerate() {
            out.row(
                "hir-expression",
                format_args!(
                    "{id},{expression},{},{}",
                    SpanRow(e.span),
                    Expression(&e.kind)
                ),
            )?;
            match &e.kind {
                source_hir::ExprKind::ArrayLiteral { elements: children } => {
                    add(&mut elements, children.len())?;
                    for (position, child) in children.iter().enumerate() {
                        out.row(
                            "hir-array-element",
                            format_args!("{id},{expression},{position},{}", child.0),
                        )?;
                    }
                }
                source_hir::ExprKind::StructLiteral { fields, .. } => {
                    for (position, field) in fields.iter().enumerate() {
                        out.row(
                            "hir-field-init",
                            format_args!(
                                "{id},{expression},{position},{},{},{}",
                                Field(field.field),
                                field.value.0,
                                SpanRow(field.span)
                            ),
                        )?;
                    }
                }
                source_hir::ExprKind::Call { args, .. } => {
                    for (position, arg) in args.iter().enumerate() {
                        out.row(
                            "hir-argument",
                            format_args!("{id},{expression},{position},{}", HirArgument(arg)),
                        )?;
                    }
                }
                _ => {}
            }
        }
    }
    out.hir_elements = Some(elements);
    out.row(
        "hir-summary",
        format_args!(
            "{},{},{},{}",
            resolved.records().len(),
            resolved.functions().len(),
            Optional(resolved.entry().map(|id| id.0)),
            elements
        ),
    )
}
struct Projection<'a>(&'a source_hir::Projection);
impl fmt::Display for Projection<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.base {
            source_hir::AccessBase::Owner(binding) => {
                write!(f, "[\"owner\",{},{}]", binding.0, Field(self.0.field))
            }
            source_hir::AccessBase::Reference { binding, kind } => write!(
                f,
                "[\"reference\",{},{},{}]",
                binding.0,
                Borrow(kind),
                Field(self.0.field)
            ),
        }
    }
}
pub(super) fn typed(
    out: &mut Output,
    typed: &typeck::TypedOwnedProgram<'_>,
) -> ObservationResult<()> {
    if out.control.resource_only() {
        return Ok(());
    }
    for view in typed.functions() {
        let function = view.hir();
        let id = function.id.0;
        for expression in 0..function.expressions.len() {
            out.row(
                "typed-expression",
                format_args!(
                    "{id},{expression},{},{}",
                    ValueType(view.expression_ty(source_hir::ExprId(expression))),
                    Optional(
                        view.expression_projection(source_hir::ExprId(expression))
                            .map(Projection)
                    )
                ),
            )?;
        }
        for binding in 0..function.bindings.len() {
            out.row(
                "typed-binding",
                format_args!(
                    "{id},{binding},{}",
                    ParameterType(view.binding_ty(source_hir::BindingId(binding)))
                ),
            )?;
        }
        for (body, block) in function.blocks.iter().enumerate() {
            let flow = view.block_flow(source_hir::BodyBlockId(body));
            out.row(
                "typed-body",
                format_args!(
                    "{id},{body},{},{}",
                    flow.falls_through(),
                    flow.returns_only()
                ),
            )?;
            for position in 0..block.body.len() {
                out.row(
                    "typed-statement",
                    format_args!(
                        "{id},{body},{position},{}",
                        Optional(
                            view.statement_projection(source_hir::BodyBlockId(body), position)
                                .map(Projection)
                        )
                    ),
                )?;
            }
        }
    }
    Ok(())
}

struct OperandRow(Operand);
impl fmt::Display for OperandRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{},{}]", self.0.local.0, SpanRow(self.0.span))
    }
}
struct PlaceRow(Place);
impl fmt::Display for PlaceRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{},{}]", self.0.id.0, SpanRow(self.0.span))
    }
}
struct Base(AccessBase);
impl fmt::Display for Base {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            AccessBase::Owner(id) => write!(f, "[\"owner\",{}]", id.0),
            AccessBase::Parameter(id) => write!(f, "[\"reference\",{}]", id.0),
        }
    }
}
struct Origins(DiagnosticOrigins);
impl fmt::Display for Origins {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{},{}]", SpanRow(self.0.primary), SpanRow(self.0.cause))
    }
}
struct OwnerClass(OwnerKind);
impl fmt::Display for OwnerClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            OwnerKind::Parameter { position } => write!(f, "[\"parameter\",{position}]"),
            OwnerKind::Local { mutable } => write!(f, "[\"local\",{mutable}]"),
            OwnerKind::Temporary => f.write_str("[\"temporary\"]"),
            OwnerKind::StagedArgument { call, argument } => {
                write!(f, "[\"staged-argument\",{},{argument}]", call.0)
            }
            OwnerKind::CallResult { call } => write!(f, "[\"call-result\",{}]", call.0),
        }
    }
}
struct ParameterRow(ParameterBinding);
impl fmt::Display for ParameterRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            ParameterBinding::Scalar(id) => write!(f, "[\"scalar\",{}]", id.0),
            ParameterBinding::Owned(id) => write!(f, "[\"owned\",{}]", id.0),
            ParameterBinding::Reference(id) => write!(f, "[\"reference\",{}]", id.0),
        }
    }
}
struct Slot(ArgumentSlot);
impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            ArgumentSlot::Scalar => f.write_str("[\"scalar\"]"),
            ArgumentSlot::Owned(id) => write!(f, "[\"owned\",{}]", id.0),
            ArgumentSlot::Borrow(id) => write!(f, "[\"borrow\",{}]", id.0),
        }
    }
}
struct ResultRow(CallResult);
impl fmt::Display for ResultRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            CallResult::Scalar(id) => write!(f, "[\"scalar\",{}]", id.0),
            CallResult::Owned(id) => write!(f, "[\"owned\",{}]", id.0),
        }
    }
}
struct Parent((CallSiteId, usize));
impl fmt::Display for Parent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{},{}]", self.0 .0 .0, self.0 .1)
    }
}
struct ValueRow<'a>(&'a Rvalue);
impl fmt::Display for ValueRow<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Rvalue::Load(place) => write!(f, "[\"load\",{}]", PlaceRow(*place)),
            Rvalue::Bool(value) => write!(f, "[\"bool\",{value}]"),
            Rvalue::I32(value) => write!(f, "[\"i32\",{value}]"),
            Rvalue::Unit => f.write_str("[\"unit\"]"),
            Rvalue::Copy(value) => write!(f, "[\"copy\",{}]", OperandRow(*value)),
            Rvalue::CheckedNegateI32 {
                operand,
                operator_span,
            } => write!(
                f,
                "[\"checked-negate-i32\",{},{}]",
                OperandRow(*operand),
                SpanRow(*operator_span)
            ),
            Rvalue::NotBool {
                operand,
                operator_span,
            } => write!(
                f,
                "[\"not-bool\",{},{}]",
                OperandRow(*operand),
                SpanRow(*operator_span)
            ),
            Rvalue::CompareScalar {
                op,
                left,
                right,
                operator_span,
            } => write!(
                f,
                "[\"compare\",{},{},{},{}]",
                Json(comparison(*op)),
                OperandRow(*left),
                OperandRow(*right),
                SpanRow(*operator_span)
            ),
            Rvalue::CheckedI32 {
                op,
                left,
                right,
                operator_span,
            } => write!(
                f,
                "[\"checked-i32\",{},{},{},{}]",
                Json(arithmetic(*op)),
                OperandRow(*left),
                OperandRow(*right),
                SpanRow(*operator_span)
            ),
        }
    }
}
struct ScalarStatement<'a>(&'a Statement);
impl fmt::Display for ScalarStatement<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Statement::Assign(assign) => write!(
                f,
                "[\"assign\",{},{},{}]",
                assign.destination.0,
                ValueRow(&assign.value),
                SpanRow(assign.span)
            ),
            Statement::Initialize { place, value, span } => write!(
                f,
                "[\"initialize\",{},{},{}]",
                PlaceRow(*place),
                OperandRow(*value),
                SpanRow(*span)
            ),
            Statement::Store {
                place,
                value,
                operator_span,
                span,
            } => write!(
                f,
                "[\"store\",{},{},{},{}]",
                PlaceRow(*place),
                OperandRow(*value),
                SpanRow(*operator_span),
                SpanRow(*span)
            ),
        }
    }
}
struct PathFields<'a>(&'a [FieldId]);
impl fmt::Display for PathFields<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[")?;
        for (index, field) in self.0.iter().enumerate() {
            if index != 0 {
                f.write_str(",")?;
            }
            Field(*field).fmt(f)?;
        }
        f.write_str("]")
    }
}
struct Instruction<'a>(&'a OwnedInstruction);
impl fmt::Display for Instruction<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use OwnedInstruction as I;
        match self.0 {
            I::ConstructEnum { .. } | I::ConsumeVariant { .. } => unreachable!("enum source gate"),
            I::ConstructComposite {
                destination,
                fields,
            } => write!(
                f,
                "[\"construct-composite\",{},{}]",
                destination.0,
                fields.len()
            ),
            I::ReadProjection {
                destination,
                base,
                path,
                index,
            } => write!(
                f,
                "[\"read-projection\",{},{},{},{}]",
                destination.0,
                Base(*base),
                PathFields(path),
                Optional(index.map(OperandRow))
            ),
            I::WriteProjection {
                base,
                path,
                index,
                value,
            } => write!(
                f,
                "[\"write-projection\",{},{},{},{}]",
                Base(*base),
                PathFields(path),
                Optional(index.map(OperandRow)),
                OperandRow(*value)
            ),
            I::ProjectionLength {
                destination,
                base,
                path,
            } => write!(
                f,
                "[\"projection-length\",{},{},{}]",
                destination.0,
                Base(*base),
                PathFields(path)
            ),
            I::Scalar(value) => write!(f, "[\"scalar\",{}]", ScalarStatement(value)),
            I::StorageLive(id) => write!(f, "[\"storage-live\",{}]", id.0),
            I::StorageEnd(id) => write!(f, "[\"storage-end\",{}]", id.0),
            I::Discard(id) => write!(f, "[\"discard\",{}]", id.0),
            I::Construct {
                destination,
                fields,
            } => write!(f, "[\"construct\",{},{}]", destination.0, fields.len()),
            I::ConstructArray {
                destination,
                elements,
            } => write!(
                f,
                "[\"construct-array\",{},{}]",
                destination.0,
                elements.len()
            ),
            I::MoveInitialize {
                destination,
                source,
            } => write!(f, "[\"move-initialize\",{},{}]", destination.0, source.0),
            I::Replace {
                destination,
                source,
            } => write!(f, "[\"replace\",{},{}]", destination.0, source.0),
            I::ReadField {
                destination,
                base,
                field,
            } => write!(
                f,
                "[\"read-field\",{},{},{}]",
                destination.0,
                Base(*base),
                Field(*field)
            ),
            I::WriteField { base, field, value } => write!(
                f,
                "[\"write-field\",{},{},{}]",
                Base(*base),
                Field(*field),
                OperandRow(*value)
            ),
            I::ReadIndex {
                destination,
                base,
                index,
            } => write!(
                f,
                "[\"read-index\",{},{},{}]",
                destination.0,
                Base(*base),
                OperandRow(*index)
            ),
            I::WriteIndex { base, index, value } => write!(
                f,
                "[\"write-index\",{},{},{}]",
                Base(*base),
                OperandRow(*index),
                OperandRow(*value)
            ),
            I::ArrayLength { destination, base } => {
                write!(f, "[\"array-length\",{},{}]", destination.0, Base(*base))
            }
            I::OpenCall(id) => write!(f, "[\"open-call\",{}]", id.0),
            I::PrepareScalar {
                call,
                argument,
                value,
            } => write!(
                f,
                "[\"prepare-scalar\",{},{},{}]",
                call.0,
                argument,
                OperandRow(*value)
            ),
            I::PrepareOwned {
                call,
                argument,
                source,
            } => write!(
                f,
                "[\"prepare-owned\",{},{},{}]",
                call.0, argument, source.0
            ),
            I::PrepareBorrow {
                call,
                argument,
                loan,
            } => write!(f, "[\"prepare-borrow\",{},{},{}]", call.0, argument, loan.0),
        }
    }
}
struct TerminatorRow<'a>(&'a OwnedTerminatorKind);
impl fmt::Display for TerminatorRow<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            OwnedTerminatorKind::MatchDispatch { .. } => unreachable!("enum source gate"),
            OwnedTerminatorKind::Branch {
                condition,
                then_block,
                else_block,
            } => write!(
                f,
                "[\"branch\",{},{},{}]",
                OperandRow(*condition),
                then_block.0,
                else_block.0
            ),
            OwnedTerminatorKind::Goto(id) => write!(f, "[\"goto\",{}]", id.0),
            OwnedTerminatorKind::Invoke { call, continuation } => {
                write!(f, "[\"invoke\",{},{}]", call.0, continuation.0)
            }
            OwnedTerminatorKind::ReturnScalar(value) => {
                write!(f, "[\"return-scalar\",{}]", OperandRow(*value))
            }
            OwnedTerminatorKind::ReturnOwned(id) => write!(f, "[\"return-owned\",{}]", id.0),
        }
    }
}
pub(super) fn raw(out: &mut Output, raw: &RawOwnedProgram) -> ObservationResult<()> {
    let mut elements = 0;
    for record in &raw.records {
        out.row(
            "raw-record",
            format_args!(
                "{},{},{}",
                record.id.0,
                SpanRow(record.span),
                record.fields.len()
            ),
        )?;
        for field in &record.fields {
            out.row(
                "raw-field",
                format_args!(
                    "{},{},{}",
                    Field(field.id),
                    ParameterType(field.ty),
                    SpanRow(field.span)
                ),
            )?;
        }
    }
    for function in &raw.functions {
        let id = function.id.0;
        out.row(
            "raw-function",
            format_args!(
                "{id},{},{},{},{},{},{},{},{},{},{},{}",
                SpanRow(function.span),
                ValueType(function.result),
                function.parameters.len(),
                function.locals.len(),
                function.places.len(),
                function.owners.len(),
                function.references.len(),
                function.calls.len(),
                function.loans.len(),
                function.entry.0,
                function.blocks.len()
            ),
        )?;
        for (position, p) in function.parameters.iter().enumerate() {
            out.row(
                "raw-parameter",
                format_args!("{id},{position},{}", ParameterRow(*p)),
            )?;
        }
        for (local, l) in function.locals.iter().enumerate() {
            out.row(
                "raw-local",
                format_args!(
                    "{id},{local},{},{},{}",
                    ScalarType(l.ty),
                    Json(match l.kind {
                        LocalKind::Parameter => "parameter",
                        LocalKind::Binding => "binding",
                        LocalKind::Temporary => "temporary",
                    }),
                    SpanRow(l.span)
                ),
            )?;
        }
        for (place, p) in function.places.iter().enumerate() {
            out.row(
                "raw-place",
                format_args!("{id},{place},{},{}", ScalarType(p.ty), SpanRow(p.span)),
            )?;
        }
        for (owner, o) in function.owners.iter().enumerate() {
            out.row(
                "raw-owner",
                format_args!(
                    "{id},{owner},{},{},{}",
                    Aggregate(o.aggregate()),
                    OwnerClass(o.kind),
                    SpanRow(o.span)
                ),
            )?;
        }
        for (reference, r) in function.references.iter().enumerate() {
            out.row(
                "raw-reference",
                format_args!(
                    "{id},{reference},{},{},{},{}",
                    Borrowed(r.referent()),
                    Borrow(r.kind),
                    r.position,
                    SpanRow(r.span)
                ),
            )?;
        }
        for (call, c) in function.calls.iter().enumerate() {
            out.row(
                "raw-call",
                format_args!(
                    "{id},{call},{},{},{},{},{}",
                    c.target.0,
                    ResultRow(c.result),
                    Optional(c.parent.map(Parent)),
                    SpanRow(c.span),
                    c.arguments.len()
                ),
            )?;
            for (position, slot) in c.arguments.iter().enumerate() {
                out.row(
                    "raw-call-argument",
                    format_args!("{id},{call},{position},{}", Slot(*slot)),
                )?;
            }
        }
        for (loan, l) in function.loans.iter().enumerate() {
            out.row(
                "raw-loan",
                format_args!(
                    "{id},{loan},{},{},{},{},{},{}",
                    l.call.0,
                    l.argument,
                    Base(l.authority),
                    Borrow(l.kind),
                    Borrowed(l.referent()),
                    SpanRow(l.span)
                ),
            )?;
        }
        for (block, b) in function.blocks.iter().enumerate() {
            out.row(
                "raw-block",
                format_args!(
                    "{id},{block},{},{},{},{}",
                    SpanRow(b.span),
                    b.statements.len(),
                    b.merge.is_some(),
                    b.terminator.is_some()
                ),
            )?;
            if let Some(merge) = &b.merge {
                out.row(
                    "raw-merge",
                    format_args!(
                        "{id},{block},{},{},{}",
                        merge.destination.0,
                        SpanRow(merge.span),
                        SpanRow(merge.operator_span)
                    ),
                )?;
                for (position, input) in merge.incoming.iter().enumerate() {
                    out.row(
                        "raw-merge-input",
                        format_args!(
                            "{id},{block},{position},{},{}",
                            input.predecessor.0,
                            OperandRow(input.value)
                        ),
                    )?;
                }
            }
            for (position, s) in b.statements.iter().enumerate() {
                out.row(
                    "raw-statement",
                    format_args!(
                        "{id},{block},{position},{},{},{}",
                        SpanRow(s.span),
                        Optional(s.diagnostic_origins.map(Origins)),
                        Instruction(&s.kind)
                    ),
                )?;
                match &s.kind {
                    OwnedInstruction::Construct { fields, .. } => {
                        for (element, (field, value)) in fields.iter().enumerate() {
                            out.row(
                                "raw-field-operand",
                                format_args!(
                                    "{id},{block},{position},{element},{},{}",
                                    Field(*field),
                                    OperandRow(*value)
                                ),
                            )?;
                        }
                    }
                    OwnedInstruction::ConstructArray {
                        elements: values, ..
                    } => {
                        add(&mut elements, values.len())?;
                        for (element, value) in values.iter().enumerate() {
                            out.row(
                                "raw-array-operand",
                                format_args!(
                                    "{id},{block},{position},{element},{}",
                                    OperandRow(*value)
                                ),
                            )?;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(t) = &b.terminator {
                out.row(
                    "raw-terminator",
                    format_args!(
                        "{id},{block},{},{},{}",
                        SpanRow(t.span),
                        Optional(t.diagnostic_origins.map(Origins)),
                        TerminatorRow(&t.kind)
                    ),
                )?;
            }
        }
    }
    out.raw_elements = Some(elements);
    out.row(
        "raw-summary",
        format_args!("{},{},{}", raw.records.len(), raw.functions.len(), elements),
    )
}

struct DiagnosticSpan<'a>(Span, &'a SourceMap);
impl fmt::Display for DiagnosticSpan<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let span = self.0;
        let source = self.1.get(span.file);
        let (line, column) = source.location(span.start);
        let (end_line, end_column) = source.location(span.end);
        write!(f,"{{\"file_id\":{},\"path\":{},\"start\":{},\"end\":{},\"line\":{},\"column\":{},\"end_line\":{},\"end_column\":{}}}",span.file.0,Json(source.path()),span.start,span.end,line,column,end_line,end_column)
    }
}
struct DiagnosticJson<'a>(&'a Diagnostic, &'a SourceMap);
impl fmt::Display for DiagnosticJson<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = self.0;
        write!(f,"{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":{},\"stage\":{},\"message\":{},\"primary\":{},\"secondary\":[",Json(d.code),Json(d.stage),Json(&d.message),Optional(d.primary.map(|span|DiagnosticSpan(span,self.1))))?;
        for (index, (span, message)) in d.secondary.iter().enumerate() {
            if index != 0 {
                f.write_char(',')?;
            }
            write!(
                f,
                "{{\"span\":{},\"message\":{}}}",
                DiagnosticSpan(*span, self.1),
                Json(message)
            )?;
        }
        f.write_str("],\"notes\":[")?;
        for (index, note) in d.notes.iter().enumerate() {
            if index != 0 {
                f.write_char(',')?;
            }
            Json(note).fmt(f)?;
        }
        f.write_str("]}")
    }
}
struct Escaping<'a, 'b>(&'a mut fmt::Formatter<'b>);
impl Write for Escaping<'_, '_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        escape(self.0, text)
    }
}
struct DiagnosticHuman<'a>(&'a Diagnostic, &'a SourceMap);
impl fmt::Display for DiagnosticHuman<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('"')?;
        self.0.write_human(self.1, &mut Escaping(f))?;
        f.write_char('"')
    }
}
fn json_character_width(ch: char) -> usize {
    match ch {
        '"' | '\\' | '\u{8}' | '\t' | '\n' | '\u{c}' | '\r' => 2,
        '\u{0}'..='\u{1f}' => 6,
        _ => ch.len_utf8(),
    }
}
/// Conservative byte-visit accounting, including this preflight itself, both
/// count/render passes, and the actual sink's independent scan/copy. Human
/// escape_default expansion and its subsequent JSON escaping are distinct.
fn diagnostic_text_work(text: &str) -> ObservationResult<usize> {
    let mut json = 2usize;
    let mut human = 0usize;
    let mut human_json = 0usize;
    for ch in text.chars() {
        add(&mut json, json_character_width(ch))?;
        if ch.is_control() {
            for escaped in ch.escape_default() {
                add(&mut human, escaped.len_utf8())?;
                add(&mut human_json, json_character_width(escaped))?;
            }
        } else {
            add(&mut human, ch.len_utf8())?;
            add(&mut human_json, json_character_width(ch))?;
        }
    }
    let mut work = text
        .len()
        .checked_mul(6)
        .ok_or("diagnostic input work overflow")?;
    for length in [json, human, human_json] {
        add(
            &mut work,
            length
                .checked_mul(4)
                .ok_or("diagnostic escaping work overflow")?,
        )?;
    }
    Ok(work)
}
fn diagnostic_work(d: &Diagnostic, sources: &SourceMap) -> ObservationResult<usize> {
    // Covers fixed JSON/human punctuation, field names and formatter headers.
    // This is a work envelope, not a measurement of CPU instructions or time.
    let mut work = 2048usize;
    for text in [d.code, d.stage, d.message.as_str()] {
        add(&mut work, diagnostic_text_work(text)?)?;
    }
    for (_, text) in &d.secondary {
        add(&mut work, diagnostic_text_work(text)?)?;
    }
    for note in &d.notes {
        add(&mut work, diagnostic_text_work(note)?)?;
        add(&mut work, 256)?;
    }
    for span in d
        .primary
        .iter()
        .copied()
        .chain(d.secondary.iter().map(|(span, _)| *span))
    {
        if !sources.is_valid_span(span) {
            return Err("invalid diagnostic span");
        }
        let source = sources.get(span.file);
        // Human start plus JSON start/end, on both formatting passes. Scanning
        // from byte0 is conservative relative to location's actual line start.
        let prefix = span
            .start
            .checked_mul(2)
            .and_then(|n| n.checked_add(span.end))
            .and_then(|n| n.checked_mul(2))
            .ok_or("diagnostic location work overflow")?;
        add(&mut work, prefix)?;
        add(&mut work, diagnostic_text_work(source.path())?)?;
        let comparisons = usize::BITS - source.line_count().max(1).leading_zeros();
        add(
            &mut work,
            (comparisons as usize)
                .checked_mul(6)
                .ok_or("diagnostic location search overflow")?,
        )?;
        // Fixed location punctuation, all at-most20-digit usize values, span
        // validation and the two count/render passes are bounded separately.
        add(&mut work, 1024)?;
    }
    Ok(work)
}
fn diagnostic_payload(d: &Diagnostic) -> ObservationResult<usize> {
    let mut bytes = d.message.capacity();
    add(
        &mut bytes,
        d.secondary
            .capacity()
            .checked_mul(size_of::<(Span, String)>())
            .ok_or("diagnostic secondary overflow")?,
    )?;
    for (_, text) in &d.secondary {
        add(&mut bytes, text.capacity())?;
    }
    add(
        &mut bytes,
        d.notes
            .capacity()
            .checked_mul(size_of::<String>())
            .ok_or("diagnostic notes overflow")?,
    )?;
    for text in &d.notes {
        add(&mut bytes, text.capacity())?;
    }
    Ok(bytes)
}
pub(super) fn diagnostics(
    out: &mut Output,
    sources: &SourceMap,
    errors: &[Diagnostic],
    header_capacity: usize,
) -> ObservationResult<()> {
    if header_capacity < errors.len() {
        return Err("diagnostic header capacity mismatch");
    }
    let mut payload = header_capacity
        .checked_mul(size_of::<Diagnostic>())
        .ok_or("diagnostic header overflow")?;
    for d in errors {
        add(&mut payload, diagnostic_payload(d)?)?;
    }
    out.aux(payload)?;
    for d in errors {
        add(&mut out.diagnostic_work, diagnostic_work(d, sources)?)?;
        if out.diagnostic_work > out.limits.diagnostic_work {
            return Err("diagnostic work limit exceeded");
        }
        out.row(
            "diagnostic",
            format_args!(
                "{},{}",
                DiagnosticJson(d, sources),
                DiagnosticHuman(d, sources)
            ),
        )?;
    }
    out.auxiliary = out
        .auxiliary
        .checked_sub(payload)
        .ok_or("diagnostic lifetime underflow")?;
    Ok(())
}

#[cfg(test)]
mod formatter_controls {
    use super::*;
    #[test]
    fn unit3b2_observer_diagnostic_stream_matches_existing_renderers() {
        let mut sources = SourceMap::new();
        let id = sources.add("a\"\\\n.ox".into(), "é\r\n🦀".into());
        let span = sources.get(id).span(4, 8);
        let mut diagnostic =
            Diagnostic::new("E0311", "ownership", "control\u{7f}\u{85}\\\"", Some(span))
                .secondary(sources.get(id).span(0, 2), "original\nlabel");
        diagnostic.notes.push("note\twith\rcontrols".into());
        assert_eq!(
            format!("{}", DiagnosticJson(&diagnostic, &sources)),
            diagnostic.render_json(&sources)
        );
        assert_eq!(
            format!("{}", DiagnosticHuman(&diagnostic, &sources)),
            crate::frontend::diagnostic::json_string(&diagnostic.render_human(&sources))
        );
        let work = diagnostic_work(&diagnostic, &sources).unwrap();
        assert!(work > diagnostic.message.len() * 4);
    }
}
