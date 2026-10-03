//! Complete source typing, deliberately without an ownership/loan checker.
use super::{hir::*, resolve::ResolvedOwnedProgram};
use crate::frontend::{
    declaration_index::{Access, PreparedTypeName},
    diagnostic::Diagnostic,
    owned_diagnostic,
    parser::MAX_DIAGNOSTICS,
    source::Span,
};

#[derive(Debug)]
pub(in crate::frontend::oir) struct TypedOwnedProgram<'src> {
    program: ResolvedOwnedProgram<'src>,
    bodies: Vec<TypedBody>,
}
#[derive(Debug)]
struct TypedBody {
    expressions: Vec<ValueTy>,
    bindings: Vec<ParameterTy>,
    block_flows: Vec<FlowSummary>,
    projections: Vec<Option<Projection>>,
    statement_projections: Vec<Vec<Option<Projection>>>,
}
/// Possible exits from a statement list. Loop transfers always refer to its
/// nearest enclosing while; that while consumes them before its own summary
/// reaches the containing list. These outcomes describe conservative source
/// paths, not constant-condition or termination analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FlowSummary {
    fallthrough: bool,
    returns: bool,
    breaks: bool,
    continues: bool,
}
impl FlowSummary {
    const FALLTHROUGH: Self = Self::new(true, false, false, false);
    const RETURN: Self = Self::new(false, true, false, false);
    const BREAK: Self = Self::new(false, false, true, false);
    const CONTINUE: Self = Self::new(false, false, false, true);

    const fn new(fallthrough: bool, returns: bool, breaks: bool, continues: bool) -> Self {
        Self {
            fallthrough,
            returns,
            breaks,
            continues,
        }
    }
    pub(super) fn falls_through(self) -> bool {
        self.fallthrough
    }
    pub(super) fn returns_only(self) -> bool {
        self == Self::RETURN
    }
    fn union(self, other: Self) -> Self {
        Self::new(
            self.fallthrough || other.fallthrough,
            self.returns || other.returns,
            self.breaks || other.breaks,
            self.continues || other.continues,
        )
    }
    /// Only fallthrough paths enter a following statement; previous terminal
    /// outcomes must survive even when that statement has different exits.
    fn then(self, next: Self) -> Self {
        if !self.fallthrough {
            return self;
        }
        Self::new(false, self.returns, self.breaks, self.continues).union(next)
    }
    fn after_while(self) -> Self {
        // The condition's false edge is always possible. Body fallthrough and
        // continue repeat it; break exits; only returns escape the whole loop.
        Self::new(true, self.returns, false, false)
    }
}

pub(super) struct TypedOwnedFunction<'a> {
    function: &'a Function,
    signature: &'a Signature,
    body: &'a TypedBody,
}
impl TypedOwnedProgram<'_> {
    pub(super) fn index(&self) -> &crate::frontend::declaration_index::DeclarationIndex<'_> {
        self.program.index()
    }
    pub(super) fn records(&self) -> &[Record] {
        self.program.records()
    }
    pub(super) fn signatures(&self) -> &[Signature] {
        self.program.signatures()
    }
    pub(in crate::frontend::oir) fn entry(&self) -> Option<DefId> {
        self.program.entry()
    }
    pub(super) fn text(&self, span: Span) -> &str {
        self.program.text(span)
    }
    pub(super) fn functions(&self) -> impl ExactSizeIterator<Item = TypedOwnedFunction<'_>> {
        assert_eq!(self.program.functions().len(), self.bodies.len());
        assert_eq!(self.program.signatures().len(), self.bodies.len());
        self.program
            .functions()
            .iter()
            .enumerate()
            .map(|(index, function)| {
                let body = &self.bodies[index];
                let signature = &self.program.signatures()[index];
                assert_eq!(function.id, DefId(index));
                assert_eq!(function.expressions.len(), body.expressions.len());
                assert_eq!(function.bindings.len(), body.bindings.len());
                assert_eq!(function.blocks.len(), body.block_flows.len());
                assert!(body.block_flows[function.body.0].returns_only());
                assert_eq!(&body.bindings[..signature.params.len()], &signature.params);
                TypedOwnedFunction {
                    function,
                    signature,
                    body,
                }
            })
    }
}
impl<'a> TypedOwnedFunction<'a> {
    pub(super) fn hir(&self) -> &'a Function {
        self.function
    }
    pub(super) fn signature(&self) -> &'a Signature {
        self.signature
    }
    pub(super) fn expression_ty(&self, id: ExprId) -> ValueTy {
        self.body.expressions[id.0]
    }
    pub(super) fn binding_ty(&self, id: BindingId) -> ParameterTy {
        self.body.bindings[id.0]
    }
    pub(super) fn block_flow(&self, id: BodyBlockId) -> FlowSummary {
        self.body.block_flows[id.0]
    }
    pub(super) fn expression_projection(&self, id: ExprId) -> Option<Projection> {
        self.body.projections[id.0]
    }
    pub(super) fn statement_projection(
        &self,
        block: BodyBlockId,
        index: usize,
    ) -> Option<Projection> {
        self.body.statement_projections[block.0][index]
    }
}
fn diagnostic(
    code: &'static str,
    stage: &'static str,
    message: impl std::fmt::Display,
    span: Option<Span>,
) -> Box<Diagnostic> {
    owned_diagnostic::diagnostic(code, stage, format_args!("{message}"), span)
}
fn error(code: &'static str, message: impl std::fmt::Display, span: Span) -> Box<Diagnostic> {
    diagnostic(code, "type", message, Some(span))
}
trait Label {
    fn owned_secondary(self, span: Span, message: &'static str) -> Self;
}
impl Label for Box<Diagnostic> {
    fn owned_secondary(self, span: Span, message: &'static str) -> Self {
        owned_diagnostic::secondary(self, span, format_args!("{message}"))
    }
}
#[allow(clippy::large_enum_variant)] // Both bounded formatter frames are preaccounted; no heap allocation.
enum TypeName<'a> {
    Scalar(Ty),
    Record(PreparedTypeName<'a>),
}
impl std::fmt::Display for TypeName<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scalar(ty) => write!(f, "{ty}"),
            Self::Record(name) => write!(f, "{name}"),
        }
    }
}
fn type_name<'a>(
    program: &ResolvedOwnedProgram<'a>,
    ty: ValueTy,
    span: Span,
) -> Result<TypeName<'a>, Box<Diagnostic>> {
    match ty {
        ValueTy::Scalar(ty) => Ok(TypeName::Scalar(ty)),
        ValueTy::Owned(AggregateTy::Record(record)) => {
            program.prepare_name(record, span).map(TypeName::Record)
        }
        ValueTy::Owned(AggregateTy::FixedArray(_)) => {
            Err(error("E0500", "unsupported aggregate type", span))
        }
    }
}
fn mismatch(
    program: &ResolvedOwnedProgram<'_>,
    expected: ValueTy,
    actual: ValueTy,
    span: Span,
) -> Box<Diagnostic> {
    // Complete both bounded preparations before invoking infallible Display.
    let expected = match type_name(program, expected, span) {
        Ok(name) => name,
        Err(error) => return error,
    };
    let actual = match type_name(program, actual, span) {
        Ok(name) => name,
        Err(error) => return error,
    };
    error(
        "E0300",
        format_args!("type mismatch: expected {expected}, found {actual}"),
        span,
    )
}
fn immutable(function: &Function, binding: BindingId, span: Span) -> Box<Diagnostic> {
    error("E0304", "operation requires a mutable owned binding", span).owned_secondary(
        function.bindings[binding.0].span,
        "immutable binding declared here",
    )
}
pub(in crate::frontend::oir) fn check(
    program: ResolvedOwnedProgram<'_>,
) -> Result<TypedOwnedProgram<'_>, Vec<Diagnostic>> {
    program.work().phase("type");
    let mut bodies = Vec::new();
    let mut diagnostics = Vec::new();
    for function in program.functions() {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
        match check_body(&program, function) {
            Ok(body) => bodies.push(body),
            Err(error) => {
                program.work().record_error(&error);
                diagnostics.push(*error)
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(TypedOwnedProgram { program, bodies })
    } else {
        Err(diagnostics)
    }
}
fn projection(
    program: &ResolvedOwnedProgram<'_>,
    function: &Function,
    binding: BindingId,
    base_span: Span,
    field_span: Span,
    bindings: &[Option<ParameterTy>],
) -> Result<Projection, Box<Diagnostic>> {
    let (record, base) = match bindings[binding.0].expect("resolved field base initialized") {
        ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(record))) => {
            (record, AccessBase::Owner(binding))
        }
        ParameterTy::Reference {
            aggregate: AggregateTy::Record(record),
            kind,
        } => (record, AccessBase::Reference { binding, kind }),
        _ => {
            return Err(
                error("E0305", "field access requires a record binding", base_span)
                    .owned_secondary(function.bindings[binding.0].span, "binding declared here"),
            )
        }
    };
    let spelling = program.text(field_span);
    let field = program.records()[record.0]
        .fields
        .iter()
        .find(|field| program.text(field.name_span) == spelling)
        .ok_or_else(|| {
            error(
                "E0305",
                format_args!("unknown field `{}`", owned_diagnostic::name(spelling)),
                field_span,
            )
        })?
        .id;
    let requester = program.requester(function.id)?;
    if let Access::Denied(field) = program.query().field_access(requester, field, field_span)? {
        return Err(program
            .query()
            .private_field_diagnostic(field, field_span, "type")?);
    }
    Ok(Projection { base, field })
}
fn borrow_type(
    function: &Function,
    kind: BorrowKind,
    place: BorrowPlace,
    span: Span,
    bindings: &[Option<ParameterTy>],
) -> Result<ParameterTy, Box<Diagnostic>> {
    let record = match place {
        BorrowPlace::Owner(binding) => {
            match bindings[binding.0].expect("borrow binding initialized") {
                ParameterTy::Value(ValueTy::Owned(record)) => {
                    if kind == BorrowKind::Exclusive && !function.bindings[binding.0].mutable {
                        return Err(immutable(function, binding, span));
                    }
                    record
                }
                ParameterTy::Value(ValueTy::Scalar(_)) => {
                    return Err(error(
                        "E0300",
                        "borrowing requires a whole record owner",
                        span,
                    ))
                }
                ParameterTy::Reference { .. } => {
                    return Err(error(
                        "E0312",
                        "reference parameters require explicit &*p or &mut *p forwarding",
                        span,
                    ))
                }
            }
        }
        BorrowPlace::Forwarded(binding) => {
            match bindings[binding.0].expect("borrow binding initialized") {
                ParameterTy::Reference {
                    aggregate: record, ..
                } => record,
                ParameterTy::Value(_) => {
                    return Err(error(
                        "E0312",
                        "explicit reborrow requires a reference parameter",
                        span,
                    ))
                }
            }
        }
    };
    // Requested permission is retained even if the parent grants only shared.
    // The authoritative raw verifier diagnoses that ownership permission error.
    Ok(ParameterTy::Reference {
        aggregate: record,
        kind,
    })
}
fn expression_type(
    program: &ResolvedOwnedProgram<'_>,
    function: &Function,
    id: ExprId,
    bindings: &[Option<ParameterTy>],
    expressions: &mut [Option<ValueTy>],
    projections: &mut [Option<Projection>],
) -> Result<ValueTy, Box<Diagnostic>> {
    if let Some(ty) = expressions[id.0] {
        return Ok(ty);
    }
    let expr = &function.expressions[id.0];
    let scalar = ValueTy::Scalar;
    let mut child = |id| expression_type(program, function, id, bindings, expressions, projections);
    let ty = match &expr.kind {
        ExprKind::Bool(_) => scalar(Ty::Bool),
        ExprKind::I32(_) => scalar(Ty::I32),
        ExprKind::Unit => scalar(Ty::Unit),
        ExprKind::Binding(binding) => match bindings[binding.0]
            .expect("resolved binding initialized")
        {
            ParameterTy::Value(ty) => ty,
            ParameterTy::Reference { .. } => return Err(error(
                "E0312",
                "reference parameter cannot be used as a value; use a field or explicit reborrow",
                expr.span,
            )),
        },
        ExprKind::Group(inner) => child(*inner)?,
        ExprKind::Not { operand, .. } => {
            let actual = child(*operand)?;
            if actual != scalar(Ty::Bool) {
                return Err(mismatch(
                    program,
                    scalar(Ty::Bool),
                    actual,
                    function.expressions[operand.0].span,
                ));
            }
            scalar(Ty::Bool)
        }
        ExprKind::Arithmetic { left, right, .. } | ExprKind::Logical { left, right, .. } => {
            let expected = if matches!(expr.kind, ExprKind::Logical { .. }) {
                scalar(Ty::Bool)
            } else {
                scalar(Ty::I32)
            };
            for operand in [left, right] {
                let actual = child(*operand)?;
                if actual != expected {
                    return Err(mismatch(
                        program,
                        expected,
                        actual,
                        function.expressions[operand.0].span,
                    ));
                }
            }
            expected
        }
        ExprKind::Comparison {
            op, left, right, ..
        } => {
            let actual = child(*left)?;
            let expected = match op {
                ComparisonOp::Equal | ComparisonOp::NotEqual => {
                    if !matches!(actual, ValueTy::Scalar(Ty::Bool | Ty::I32)) {
                        return Err(error(
                            "E0300",
                            "equality requires i32 or bool operands",
                            function.expressions[left.0].span,
                        ));
                    }
                    actual
                }
                _ => {
                    if actual != scalar(Ty::I32) {
                        return Err(mismatch(
                            program,
                            scalar(Ty::I32),
                            actual,
                            function.expressions[left.0].span,
                        ));
                    }
                    scalar(Ty::I32)
                }
            };
            let actual = child(*right)?;
            if actual != expected {
                return Err(mismatch(
                    program,
                    expected,
                    actual,
                    function.expressions[right.0].span,
                ));
            }
            scalar(Ty::Bool)
        }
        ExprKind::Call { target, args } => {
            let called = &program.signatures()[target.0];
            // Visit all well-formed argument values in source order, including
            // statically skipped logical branches and ignored helper results.
            let mut actuals = Vec::with_capacity(args.len());
            for arg in args {
                actuals.push(match arg {
                    Argument::Value(value) => (
                        ParameterTy::Value(child(*value)?),
                        function.expressions[value.0].span,
                    ),
                    Argument::Borrow {
                        kind, place, span, ..
                    } => (
                        borrow_type(function, *kind, *place, *span, bindings)?,
                        *span,
                    ),
                });
            }
            if args.len() != called.params.len() {
                return Err(error(
                    "E0301",
                    format_args!(
                        "wrong argument count: expected {}, found {}",
                        called.params.len(),
                        args.len()
                    ),
                    expr.span,
                )
                .owned_secondary(called.span, "function declared here"));
            }
            #[allow(clippy::unused_enumerate_index)]
            // Position is used only by passive test observation.
            for (_position, ((actual, span), expected)) in
                actuals.into_iter().zip(&called.params).enumerate()
            {
                if actual != *expected {
                    return Err(error(
                        "E0300",
                        "call argument type or borrow mode does not match parameter",
                        span,
                    )
                    .owned_secondary(called.span, "function declared here"));
                }
                #[cfg(test)]
                if let Argument::Borrow { place, .. } = args[_position] {
                    let binding = match place {
                        BorrowPlace::Owner(binding) | BorrowPlace::Forwarded(binding) => binding,
                    };
                    program.work().observe(
                        crate::frontend::declaration_index::Observation::BorrowArgument {
                            function: function.id,
                            origin: span,
                            binding: binding.0,
                            ty: actual,
                        },
                    );
                }
            }
            called.result
        }
        ExprKind::StructLiteral { record, fields } => {
            let declared = &program.records()[record.0];
            let mut present = vec![false; declared.fields.len()];
            for field in fields {
                present[field.field.index] = true;
                let actual = child(field.value)?;
                let expected = scalar(declared.fields[field.field.index].ty);
                if actual != expected {
                    return Err(mismatch(
                        program,
                        expected,
                        actual,
                        function.expressions[field.value.0].span,
                    ));
                }
            }
            if fields.len() != declared.fields.len() {
                return Err(owned_diagnostic::missing_fields(
                    "E0300",
                    "type",
                    declared
                        .fields
                        .iter()
                        .filter(|decl| !present[decl.id.index])
                        .map(|field| program.text(field.name_span)),
                    Some(expr.span),
                ));
            }
            ValueTy::Owned(AggregateTy::Record(*record))
        }
        ExprKind::FieldRead {
            base,
            base_span,
            field_span,
        } => {
            let projection =
                projection(program, function, *base, *base_span, *field_span, bindings)?;
            projections[id.0] = Some(projection);
            let ty = scalar(
                program.records()[projection.field.record.0].fields[projection.field.index].ty,
            );
            #[cfg(test)]
            program.work().observe(
                crate::frontend::declaration_index::Observation::Projection {
                    operation: "read",
                    function: function.id,
                    origin: *field_span,
                    field: projection.field,
                    ty,
                },
            );
            ty
        }
    };
    expressions[id.0] = Some(ty);
    #[cfg(test)]
    program.work().observe(
        crate::frontend::declaration_index::Observation::Expression {
            function: function.id,
            origin: expr.span,
            ty,
            field: projections[id.0].map(|p| p.field),
        },
    );
    Ok(ty)
}
fn check_body(
    program: &ResolvedOwnedProgram<'_>,
    function: &Function,
) -> Result<TypedBody, Box<Diagnostic>> {
    let signature = &program.signatures()[function.id.0];
    let mut bindings = vec![None; function.bindings.len()];
    for (index, ty) in signature.params.iter().enumerate() {
        bindings[index] = Some(*ty);
        #[cfg(test)]
        program
            .work()
            .observe(crate::frontend::declaration_index::Observation::Binding {
                function: function.id,
                origin: function.bindings[index].span,
                ty: *ty,
            });
    }
    let mut expressions = vec![None; function.expressions.len()];
    let mut projections = vec![None; function.expressions.len()];
    let mut statement_projections: Vec<Vec<Option<Projection>>> = function
        .blocks
        .iter()
        .map(|block| vec![None; block.body.len()])
        .collect();
    // A continuation frame records each statement-list result without Rust
    // recursion. Both children complete before their parent's flow is resumed.
    enum Frame {
        Block {
            block: BodyBlockId,
            index: usize,
            flow: FlowSummary,
            active_loop: Option<LoopId>,
        },
        IfJoin {
            block: BodyBlockId,
            index: usize,
            before: FlowSummary,
            active_loop: Option<LoopId>,
            then_block: BodyBlockId,
            else_block: Option<BodyBlockId>,
        },
        WhileJoin {
            block: BodyBlockId,
            index: usize,
            before: FlowSummary,
            active_loop: Option<LoopId>,
            body: BodyBlockId,
        },
    }
    let mut block_flows: Vec<Option<FlowSummary>> = vec![None; function.blocks.len()];
    let mut frames = vec![Frame::Block {
        block: function.body,
        index: 0,
        flow: FlowSummary::FALLTHROUGH,
        active_loop: None,
    }];
    while let Some(frame) = frames.pop() {
        let (block, index, mut flow, active_loop) = match frame {
            Frame::Block {
                block,
                index,
                flow,
                active_loop,
            } => (block, index, flow, active_loop),
            Frame::IfJoin {
                block,
                index,
                before,
                active_loop,
                then_block,
                else_block,
            } => {
                let then_flow = block_flows[then_block.0].expect("then arm was checked");
                let else_flow = else_block.map_or(FlowSummary::FALLTHROUGH, |id| {
                    block_flows[id.0].expect("else arm was checked")
                });
                (
                    block,
                    index,
                    before.then(then_flow.union(else_flow)),
                    active_loop,
                )
            }
            Frame::WhileJoin {
                block,
                index,
                before,
                active_loop,
                body,
            } => {
                let body_flow = block_flows[body.0].expect("while body was checked");
                (
                    block,
                    index,
                    before.then(body_flow.after_while()),
                    active_loop,
                )
            }
        };
        let Some(statement) = function.blocks[block.0].body.get(index) else {
            block_flows[block.0] = Some(flow);
            continue;
        };
        if !flow.falls_through() {
            return Err(diagnostic(
                "E0303",
                "type",
                if flow.returns_only() {
                    "statement after terminal return is unavailable in typed-preview"
                } else {
                    "statement after terminal control transfer is unavailable in typed-preview"
                },
                Some(statement.span),
            ));
        }
        let root = match statement.kind {
            StmtKind::Let { init, .. }
            | StmtKind::Assign { value: init, .. }
            | StmtKind::FieldAssign { value: init, .. }
            | StmtKind::Expr(init) => Some(init),
            StmtKind::Return(value) => value,
            StmtKind::Break { .. } | StmtKind::Continue { .. } => None,
            StmtKind::If { condition, .. } | StmtKind::While { condition, .. } => Some(condition),
        };
        if let Some(root) = root {
            expression_type(
                program,
                function,
                root,
                &bindings,
                &mut expressions,
                &mut projections,
            )?;
        }
        match statement.kind {
            StmtKind::Let { binding, init } => {
                let actual = expressions[init.0].expect("typed initializer");
                if let Some(expected) = function.bindings[binding.0].annotation {
                    if expected != actual {
                        return Err(mismatch(
                            program,
                            expected,
                            actual,
                            function.expressions[init.0].span,
                        )
                        .owned_secondary(
                            function.bindings[binding.0].span,
                            "binding declared here",
                        ));
                    }
                }
                bindings[binding.0] = Some(ParameterTy::Value(actual));
                #[cfg(test)]
                program
                    .work()
                    .observe(crate::frontend::declaration_index::Observation::Binding {
                        function: function.id,
                        origin: function.bindings[binding.0].span,
                        ty: ParameterTy::Value(actual),
                    });
            }
            StmtKind::Assign {
                binding,
                target_span,
                value,
                ..
            } => {
                let declaration = &function.bindings[binding.0];
                if !declaration.mutable {
                    return Err(diagnostic(
                        "E0304",
                        "type",
                        "assignment requires a mutable binding",
                        Some(target_span),
                    )
                    .owned_secondary(declaration.span, "immutable binding declared here"));
                }
                let expected =
                    bindings[binding.0].expect("resolved assignment target has an initializer");
                let ParameterTy::Value(expected) = expected else {
                    return Err(error(
                        "E0312",
                        "reference parameters cannot be assigned",
                        target_span,
                    ));
                };
                let actual = expressions[value.0].expect("typed assignment");
                if actual != expected {
                    return Err(mismatch(
                        program,
                        expected,
                        actual,
                        function.expressions[value.0].span,
                    )
                    .owned_secondary(declaration.span, "binding declared here"));
                }
            }
            StmtKind::FieldAssign {
                base,
                base_span,
                field_span,
                target_span,
                value,
                ..
            } => {
                let projection =
                    projection(program, function, base, base_span, field_span, &bindings)?;
                if matches!(projection.base, AccessBase::Owner(_))
                    && !function.bindings[base.0].mutable
                {
                    return Err(immutable(function, base, target_span));
                }
                let expected = ValueTy::Scalar(
                    program.records()[projection.field.record.0].fields[projection.field.index].ty,
                );
                let actual = expressions[value.0].expect("typed field assignment");
                if expected != actual {
                    return Err(mismatch(
                        program,
                        expected,
                        actual,
                        function.expressions[value.0].span,
                    ));
                }
                statement_projections[block.0][index] = Some(projection);
                #[cfg(test)]
                program.work().observe(
                    crate::frontend::declaration_index::Observation::Projection {
                        operation: "write",
                        function: function.id,
                        origin: field_span,
                        field: projection.field,
                        ty: expected,
                    },
                );
            }
            StmtKind::Expr(_) => {}
            StmtKind::Return(value) => {
                let actual = value.map_or(ValueTy::Scalar(Ty::Unit), |id| {
                    expressions[id.0].expect("typed return")
                });
                if signature.result != actual {
                    return Err(mismatch(
                        program,
                        signature.result,
                        actual,
                        value.map_or(statement.span, |id| function.expressions[id.0].span),
                    ));
                }
                flow = flow.then(FlowSummary::RETURN);
            }
            StmtKind::Break { target } | StmtKind::Continue { target } => {
                assert_eq!(
                    active_loop,
                    Some(target),
                    "resolved transfer targets the nearest active loop"
                );
                assert!(target.0 < function.blocks.len());
                let transfer = if matches!(statement.kind, StmtKind::Break { .. }) {
                    FlowSummary::BREAK
                } else {
                    FlowSummary::CONTINUE
                };
                flow = flow.then(transfer);
            }
            StmtKind::While {
                loop_id,
                condition,
                body,
            } => {
                let actual = expressions[condition.0].expect("typed condition");
                if actual != ValueTy::Scalar(Ty::Bool) {
                    return Err(mismatch(
                        program,
                        ValueTy::Scalar(Ty::Bool),
                        actual,
                        function.expressions[condition.0].span,
                    ));
                }
                assert_eq!(
                    loop_id.0, body.0,
                    "resolved loop ID is its unique body block"
                );
                frames.push(Frame::WhileJoin {
                    block,
                    index: index + 1,
                    before: flow,
                    active_loop,
                    body,
                });
                frames.push(Frame::Block {
                    block: body,
                    index: 0,
                    flow: FlowSummary::FALLTHROUGH,
                    active_loop: Some(loop_id),
                });
                continue;
            }
            StmtKind::If {
                condition,
                then_block,
                else_block,
            } => {
                let actual = expressions[condition.0].expect("typed condition");
                if actual != ValueTy::Scalar(Ty::Bool) {
                    return Err(mismatch(
                        program,
                        ValueTy::Scalar(Ty::Bool),
                        actual,
                        function.expressions[condition.0].span,
                    ));
                }
                frames.push(Frame::IfJoin {
                    block,
                    index: index + 1,
                    before: flow,
                    active_loop,
                    then_block,
                    else_block,
                });
                if let Some(otherwise) = else_block {
                    frames.push(Frame::Block {
                        block: otherwise,
                        index: 0,
                        flow: FlowSummary::FALLTHROUGH,
                        active_loop,
                    });
                }
                frames.push(Frame::Block {
                    block: then_block,
                    index: 0,
                    flow: FlowSummary::FALLTHROUGH,
                    active_loop,
                });
                continue;
            }
        }
        frames.push(Frame::Block {
            block,
            index: index + 1,
            flow,
            active_loop,
        });
    }
    if !block_flows[function.body.0]
        .expect("function body was checked")
        .returns_only()
    {
        return Err(diagnostic(
            "E0302",
            "type",
            "function requires an explicit terminal return",
            Some(function.end),
        ));
    }
    let bindings = bindings
        .into_iter()
        .map(|ty| ty.expect("all resolved bindings have typed initializers"))
        .collect();
    let block_flows = block_flows
        .into_iter()
        .map(|flow| flow.expect("all resolved blocks have checked flow"))
        .collect();
    Ok(TypedBody {
        expressions: expressions
            .into_iter()
            .map(|ty| ty.expect("every expression was typed"))
            .collect(),
        projections,
        statement_projections,
        bindings,
        block_flows,
    })
}
