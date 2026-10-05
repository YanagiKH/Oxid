//! Deterministic count-and-emit templates; only the raw verifier certifies output.
use super::super::{budget as raw_budget, *};
use super::{
    budget, hir as source,
    typeck::{TypedOwnedFunction, TypedOwnedProgram},
};
use std::mem::size_of;
type Result<T> = std::result::Result<T, OwnedFailure>;
type Counts = raw_budget::FunctionCounts;
const EXPR_FRAMES: usize = 3 * (crate::frontend::parser::MAX_NESTING + 1) + 8;
const BODY_FRAMES: usize = 4 * (crate::frontend::parser::MAX_BLOCK_NESTING + 1) + 8;
const LOOP_FRAMES: usize = crate::frontend::parser::MAX_BLOCK_NESTING + 1;
fn invariant(span: Span) -> OwnedFailure {
    OwnedFailure::malformed(Malformed::CanonicalSite, span)
}
fn require(condition: bool, span: Span) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(invariant(span))
    }
}
#[derive(Clone, Copy)]
pub(super) enum BindingLocation {
    ScalarValue(LocalId),
    ScalarPlace(PlaceId),
    Owner(OwnerPlaceId),
    Reference(ReferenceParamId),
}
#[derive(Clone, Copy)]
pub(super) enum EvaluatedValue {
    Scalar(Operand),
    Owned(OwnerPlaceId),
}
#[derive(Clone, Copy)]
struct LoopTargets {
    id: source::LoopId,
    header: BlockId,
    exit: BlockId,
    prefix: usize,
}
struct Stack<T: Copy, const N: usize> {
    values: [Option<T>; N],
    len: usize,
}
impl<T: Copy, const N: usize> Stack<T, N> {
    fn new() -> Self {
        Self {
            values: [None; N],
            len: 0,
        }
    }
    fn push(&mut self, value: T, span: Span) -> Result<()> {
        if self.len == N {
            return Err(invariant(span));
        }
        self.values[self.len] = Some(value);
        self.len += 1;
        Ok(())
    }
    fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            self.values[self.len].take()
        }
    }
    fn last(&self) -> Option<T> {
        self.len.checked_sub(1).and_then(|i| self.values[i])
    }
}
#[derive(Clone, Copy)]
enum BodyFrame {
    Body {
        block: source::BodyBlockId,
        next: usize,
    },
    Enter {
        block: source::BodyBlockId,
        entry: BlockId,
        join: Option<BlockId>,
        prefix: usize,
    },
    Finish {
        end: Span,
        join: Option<BlockId>,
        prefix: usize,
    },
    Join {
        block: Option<BlockId>,
        span: Span,
    },
    LoopEnd {
        id: source::LoopId,
        exit: BlockId,
        span: Span,
        prefix: usize,
    },
}
#[derive(Clone, Copy)]
struct CallFrame {
    expression: source::ExprId,
    call: CallSiteId,
    next: usize,
    next_loan: usize,
    result: CallResult,
    parent: Option<(CallSiteId, usize)>,
}
#[derive(Clone, Copy)]
enum ExprFrame {
    Visit(source::ExprId),
    Emit(source::ExprId),
    StageField(source::ExprId),
    LogicalLeft(source::ExprId),
    LogicalRight {
        expression: source::ExprId,
        predecessor: BlockId,
        join: BlockId,
    },
    Literal {
        expression: source::ExprId,
        next: usize,
    },
    ArrayLiteral {
        expression: source::ExprId,
        next: usize,
    },
    Call(CallFrame),
    Prepare(CallFrame),
}
struct Output {
    raw: RawOwnedFunction,
    bindings: Vec<Option<BindingLocation>>,
    expressions: Vec<Option<EvaluatedValue>>,
    lexical: Vec<OwnerPlaceId>,
    expected: Counts,
}
struct Walk<'a, 'b> {
    view: &'a TypedOwnedFunction<'a>,
    output: Option<Output>,
    block_counts: Option<&'b mut [usize]>,
    counts: Counts,
    current: Option<BlockId>,
    active: usize,
    next_expression: usize,
    preparing: Option<(CallSiteId, usize)>,
    #[cfg(test)]
    pass: super::array_pipeline::LowerPass,
}
impl<'a, 'b> Walk<'a, 'b> {
    fn new(
        view: &'a TypedOwnedFunction<'a>,
        block_counts: Option<&'b mut [usize]>,
        expected: Option<Counts>,
        #[cfg(test)] pass: super::array_pipeline::LowerPass,
    ) -> Result<Self> {
        if !view.admission().allows_lowering() {
            return Err(invariant(view.signature().span));
        }
        let output = if let Some(c) = expected {
            let f = view.hir();
            let signature = view.signature();
            Some(Output {
                raw: RawOwnedFunction {
                    id: f.id,
                    span: signature.span,
                    result: signature.result,
                    parameters: budget::reserve(c.parameters)?,
                    locals: budget::reserve(c.locals)?,
                    places: budget::reserve(c.places)?,
                    owners: budget::reserve(c.owners)?,
                    references: budget::reserve(c.references)?,
                    calls: budget::reserve(c.calls)?,
                    loans: budget::reserve(c.loans)?,
                    entry: BlockId(0),
                    blocks: budget::reserve(c.blocks)?,
                },
                bindings: budget::filled(f.bindings.len(), None)?,
                expressions: budget::filled(f.expressions.len(), None)?,
                lexical: budget::reserve(c.owners)?,
                expected: c,
            })
        } else {
            None
        };
        let mut this = Self {
            view,
            output,
            block_counts,
            counts: Counts::default(),
            current: None,
            active: 0,
            next_expression: 0,
            preparing: None,
            #[cfg(test)]
            pass,
        };
        this.inventory()?;
        let entry = this.block(view.signature().span)?;
        this.enter(Some(entry), view.signature().span)?;
        Ok(this)
    }
    fn inventory(&mut self) -> Result<()> {
        #[cfg(test)]
        budget::guard_event(budget::GuardEvent::InventoryEntry);
        let f = self.view.hir();
        let signature = self.view.signature();
        self.counts.parameters = signature.params.len();
        budget::cap(
            self.counts.parameters,
            crate::frontend::parser::MAX_PARAMS,
            "parameters",
        )?;
        for (index, binding) in f.bindings.iter().enumerate().take(signature.params.len()) {
            require(
                binding.parameter_position == Some(index) && !binding.mutable,
                binding.span,
            )?;
            let location = match self.view.binding_ty(source::BindingId(index)) {
                ParameterTy::Value(ValueTy::Scalar(ty)) => BindingLocation::ScalarValue(
                    self.local(ty, LocalKind::Parameter, binding.span)?,
                ),
                ParameterTy::Value(ValueTy::Owned(record)) => {
                    let owner = self.owner(
                        record,
                        OwnerKind::Parameter { position: index },
                        binding.span,
                    )?;
                    self.register(owner, binding.span)?;
                    BindingLocation::Owner(owner)
                }
                ParameterTy::Reference { referent, kind } => {
                    let id = ReferenceParamId(self.counts.references);
                    self.counts.references = budget::add(self.counts.references, 1)?;
                    self.counts.ownership_active = true;
                    if let Some(out) = &mut self.output {
                        budget::append(
                            &mut out.raw.references,
                            ReferenceDecl {
                                referent: BorrowedSlot::check(referent)?,
                                kind,
                                position: index,
                                span: binding.span,
                            },
                            out.expected.references,
                            binding.span,
                        )?;
                    }
                    BindingLocation::Reference(id)
                }
            };
            if let Some(out) = &mut self.output {
                out.bindings[index] = Some(location);
                let parameter = match location {
                    BindingLocation::ScalarValue(id) => ParameterBinding::Scalar(id),
                    BindingLocation::Owner(id) => ParameterBinding::Owned(id),
                    BindingLocation::Reference(id) => ParameterBinding::Reference(id),
                    BindingLocation::ScalarPlace(_) => return Err(invariant(binding.span)),
                };
                budget::append(
                    &mut out.raw.parameters,
                    parameter,
                    out.expected.parameters,
                    binding.span,
                )?;
            }
        }
        for (index, binding) in f.bindings.iter().enumerate().skip(signature.params.len()) {
            require(binding.parameter_position.is_none(), binding.span)?;
            let ParameterTy::Value(ty) = self.view.binding_ty(source::BindingId(index)) else {
                return Err(invariant(binding.span));
            };
            if let ValueTy::Scalar(ty) = ty {
                let location = if binding.mutable {
                    let id = PlaceId(self.counts.places);
                    self.counts.places = budget::add(self.counts.places, 1)?;
                    if let Some(out) = &mut self.output {
                        budget::append(
                            &mut out.raw.places,
                            PlaceDecl {
                                ty,
                                span: binding.span,
                            },
                            out.expected.places,
                            binding.span,
                        )?;
                    }
                    BindingLocation::ScalarPlace(id)
                } else {
                    BindingLocation::ScalarValue(self.local(
                        ty,
                        LocalKind::Binding,
                        binding.span,
                    )?)
                };
                if let Some(out) = &mut self.output {
                    out.bindings[index] = Some(location);
                }
            }
        }
        for (index, expression) in f.expressions.iter().enumerate() {
            if let ValueTy::Scalar(ty) = self.view.expression_ty(source::ExprId(index)) {
                let local = self.local(ty, LocalKind::Temporary, expression.span)?;
                if let Some(out) = &mut self.output {
                    out.expressions[index] = Some(EvaluatedValue::Scalar(Operand {
                        local,
                        span: expression.span,
                    }));
                }
            }
        }
        self.check_counts()
    }
    fn check_counts(&self) -> Result<()> {
        let c = self.counts;
        budget::cap(
            budget::add(budget::add(c.locals, c.places)?, c.owners)?,
            MAX_LOCALS,
            "locals",
        )?;
        budget::cap(c.blocks, MAX_BLOCKS, "blocks")?;
        let statements = budget::add(c.statements, c.merges)?;
        budget::cap(statements, MAX_ASSIGNMENTS, "assignments")?;
        let expanded = budget::add(
            budget::add(
                statements,
                budget::add(c.descriptor_arguments, c.preparations)?,
            )?,
            budget::add(
                budget::add(c.constructed_fields, c.constructed_elements)?,
                budget::add(c.composite_fields, c.projection_fields)?,
            )?,
        )?;
        budget::cap(expanded, MAX_ASSIGNMENTS, "expanded ownership events")?;
        budget::cap(
            budget::function_bytes(c)?,
            budget::MAX_RAW_BYTES,
            "source raw payload",
        )?;
        Ok(())
    }
    fn local(&mut self, ty: hir::Ty, kind: LocalKind, span: Span) -> Result<LocalId> {
        let id = LocalId(self.counts.locals);
        self.counts.locals = budget::add(self.counts.locals, 1)?;
        if let Some(out) = &mut self.output {
            budget::append(
                &mut out.raw.locals,
                LocalDecl { ty, kind, span },
                out.expected.locals,
                span,
            )?;
        }
        self.check_counts()?;
        Ok(id)
    }
    fn owner(
        &mut self,
        aggregate: AggregateTy,
        kind: OwnerKind,
        span: Span,
    ) -> Result<OwnerPlaceId> {
        let id = OwnerPlaceId(self.counts.owners);
        self.counts.owners = budget::add(self.counts.owners, 1)?;
        self.counts.ownership_active = true;
        if let Some(out) = &mut self.output {
            budget::append(
                &mut out.raw.owners,
                OwnerDecl {
                    aggregate: AggregateSlot::try_from_aggregate(aggregate)?,
                    kind,
                    span,
                },
                out.expected.owners,
                span,
            )?;
        }
        self.check_counts()?;
        Ok(id)
    }
    fn register(&mut self, owner: OwnerPlaceId, span: Span) -> Result<()> {
        self.active = budget::add(self.active, 1)?;
        if let Some(out) = &mut self.output {
            budget::append(&mut out.lexical, owner, out.expected.owners, span)?;
        }
        Ok(())
    }
    fn restore(&mut self, prefix: usize, span: Span) -> Result<()> {
        require(prefix <= self.active, span)?;
        self.active = prefix;
        if let Some(out) = &mut self.output {
            out.lexical.truncate(prefix);
        }
        Ok(())
    }
    fn location(&self, id: source::BindingId, span: Span) -> Result<BindingLocation> {
        if let Some(out) = &self.output {
            return out
                .bindings
                .get(id.0)
                .copied()
                .flatten()
                .ok_or_else(|| invariant(span));
        }
        Ok(match self.view.binding_ty(id) {
            ParameterTy::Value(ValueTy::Scalar(_)) if self.view.hir().bindings[id.0].mutable => {
                BindingLocation::ScalarPlace(PlaceId(0))
            }
            ParameterTy::Value(ValueTy::Scalar(_)) => BindingLocation::ScalarValue(LocalId(0)),
            ParameterTy::Value(ValueTy::Owned(_)) => BindingLocation::Owner(OwnerPlaceId(0)),
            ParameterTy::Reference { .. } => BindingLocation::Reference(ReferenceParamId(0)),
        })
    }
    fn value(&self, id: source::ExprId) -> Result<EvaluatedValue> {
        let span = self.view.hir().expressions[id.0].span;
        if let Some(out) = &self.output {
            return out.expressions[id.0].ok_or_else(|| invariant(span));
        }
        Ok(match self.view.expression_ty(id) {
            ValueTy::Scalar(_) => EvaluatedValue::Scalar(Operand {
                local: LocalId(0),
                span,
            }),
            ValueTy::Owned(_) => EvaluatedValue::Owned(OwnerPlaceId(0)),
        })
    }
    fn operand(&self, id: source::ExprId) -> Result<Operand> {
        match self.value(id)? {
            EvaluatedValue::Scalar(v) => Ok(v),
            _ => Err(invariant(self.view.hir().expressions[id.0].span)),
        }
    }
    fn set_owned(&mut self, id: source::ExprId, owner: OwnerPlaceId) {
        if let Some(out) = &mut self.output {
            out.expressions[id.0] = Some(EvaluatedValue::Owned(owner));
        }
    }
    fn projection_path(
        &mut self,
        projection: &source::Projection,
        span: Span,
    ) -> Result<Vec<FieldId>> {
        self.counts.projection_fields =
            budget::add(self.counts.projection_fields, projection.path.len())?;
        self.check_counts()?;
        let mut path = if self.output.is_some() {
            budget::reserve(projection.path.len())?
        } else {
            Vec::new()
        };
        if self.output.is_some() {
            for field in &projection.path {
                budget::append(&mut path, *field, projection.path.len(), span)?;
            }
        }
        Ok(path)
    }
    fn base(&self, id: source::BindingId, span: Span) -> Result<AccessBase> {
        match self.location(id, span)? {
            BindingLocation::Owner(id) => Ok(AccessBase::Owner(id)),
            BindingLocation::Reference(id) => Ok(AccessBase::Parameter(id)),
            _ => Err(invariant(span)),
        }
    }
    fn complete(&mut self, id: source::ExprId) -> Result<()> {
        require(
            id.0 == self.next_expression,
            self.view.hir().expressions[id.0].span,
        )?;
        self.next_expression += 1;
        #[cfg(test)]
        super::array_pipeline::completed(
            self.pass,
            self.view.hir().id.0,
            id.0,
            self.view.hir().expressions[id.0].span,
        );
        Ok(())
    }
    fn block(&mut self, span: Span) -> Result<BlockId> {
        #[cfg(test)]
        budget::guard_event(budget::GuardEvent::BlockAllocation);
        let id = BlockId(self.counts.blocks);
        self.counts.blocks = budget::add(self.counts.blocks, 1)?;
        self.check_counts()?;
        if let Some(out) = &mut self.output {
            let count = *self
                .block_counts
                .as_ref()
                .and_then(|c| c.get(id.0))
                .ok_or_else(|| invariant(span))?;
            let statements = budget::reserve(count)?;
            budget::append(
                &mut out.raw.blocks,
                OwnedBlock {
                    merge: None,
                    span,
                    statements,
                    terminator: None,
                },
                out.expected.blocks,
                span,
            )?;
        }
        Ok(id)
    }
    fn enter(&mut self, block: Option<BlockId>, span: Span) -> Result<()> {
        require(self.current.is_none(), span)?;
        if let (Some(out), Some(id)) = (&self.output, block) {
            require(out.raw.blocks[id.0].terminator.is_none(), span)?;
        }
        self.current = block;
        Ok(())
    }
    fn record_statements(&mut self, n: usize, span: Span) -> Result<BlockId> {
        let block = self.current.ok_or_else(|| invariant(span))?;
        self.counts.statements = budget::add(self.counts.statements, n)?;
        self.counts.diagnostic_origins = budget::add(self.counts.diagnostic_origins, n)?;
        self.check_counts()?;
        if self.output.is_none() {
            if let Some(counts) = &mut self.block_counts {
                counts[block.0] = budget::add(counts[block.0], n)?;
            }
        }
        Ok(block)
    }
    fn statement(
        &mut self,
        kind: OwnedInstruction,
        span: Span,
        primary: Span,
        cause: Span,
    ) -> Result<()> {
        if !matches!(kind, OwnedInstruction::Scalar(_)) {
            self.counts.ownership_active = true;
        }
        if matches!(
            kind,
            OwnedInstruction::PrepareScalar { .. }
                | OwnedInstruction::PrepareOwned { .. }
                | OwnedInstruction::PrepareBorrow { .. }
        ) {
            self.counts.preparations = budget::add(self.counts.preparations, 1)?;
        }
        let block = self.record_statements(1, span)?;
        if let Some(out) = &mut self.output {
            #[cfg(test)]
            budget::guard_event(budget::GuardEvent::EmitStep);
            let count = self.block_counts.as_ref().ok_or_else(|| invariant(span))?[block.0];
            budget::append(
                &mut out.raw.blocks[block.0].statements,
                OwnedStatement {
                    kind,
                    span,
                    diagnostic_origins: Some(DiagnosticOrigins { primary, cause }),
                },
                count,
                span,
            )?;
        }
        Ok(())
    }
    fn close(&mut self, kind: OwnedTerminatorKind, span: Span, cause: Span) -> Result<()> {
        let block = self.current.take().ok_or_else(|| invariant(span))?;
        self.counts.edges = budget::add(
            self.counts.edges,
            match kind {
                OwnedTerminatorKind::Branch { .. } => 2,
                OwnedTerminatorKind::Goto(_) | OwnedTerminatorKind::Invoke { .. } => 1,
                _ => 0,
            },
        )?;
        self.counts.diagnostic_origins = budget::add(self.counts.diagnostic_origins, 1)?;
        if matches!(
            kind,
            OwnedTerminatorKind::Invoke { .. } | OwnedTerminatorKind::ReturnOwned(_)
        ) {
            self.counts.ownership_active = true;
        }
        if let Some(out) = &mut self.output {
            let b = &mut out.raw.blocks[block.0];
            require(b.terminator.is_none(), span)?;
            b.terminator = Some(OwnedTerminator {
                kind,
                span,
                diagnostic_origins: Some(DiagnosticOrigins {
                    primary: span,
                    cause,
                }),
            });
        }
        self.check_counts()
    }
    fn cleanup(&mut self, prefix: usize, span: Span) -> Result<()> {
        require(prefix <= self.active, span)?;
        let length = self.active - prefix;
        if length == 0 {
            return Ok(());
        }
        self.counts.ownership_active = true;
        if self.output.is_none() {
            self.record_statements(length, span)?;
            return Ok(());
        }
        for index in (prefix..self.active).rev() {
            let owner = self.output.as_ref().ok_or_else(|| invariant(span))?.lexical[index];
            self.statement(OwnedInstruction::StorageEnd(owner), span, span, span)?;
        }
        Ok(())
    }
    fn scalar_assign(
        &mut self,
        destination: LocalId,
        value: Rvalue,
        span: Span,
        cause: Span,
    ) -> Result<()> {
        self.statement(
            OwnedInstruction::Scalar(Statement::Assign(Assign {
                destination,
                value,
                span,
            })),
            span,
            span,
            cause,
        )
    }
    fn end_value(
        &mut self,
        owner: OwnerPlaceId,
        span: Span,
        primary: Span,
        cause: Span,
    ) -> Result<()> {
        self.statement(OwnedInstruction::StorageEnd(owner), span, primary, cause)
    }

    fn expression(&mut self, root: source::ExprId, cause: Span) -> Result<()> {
        let mut frames = Stack::<ExprFrame, EXPR_FRAMES>::new();
        frames.push(ExprFrame::Visit(root), cause)?;
        while let Some(frame) = frames.pop() {
            match frame {
                ExprFrame::Visit(id) => {
                    let expression = &self.view.hir().expressions[id.0];
                    match &expression.kind {
                        source::ExprKind::ArrayLiteral { .. } => frames.push(
                            ExprFrame::ArrayLiteral {
                                expression: id,
                                next: 0,
                            },
                            cause,
                        )?,
                        source::ExprKind::Logical { left, .. } => {
                            frames.push(ExprFrame::LogicalLeft(id), cause)?;
                            frames.push(ExprFrame::Visit(*left), cause)?;
                        }
                        source::ExprKind::Call { .. } => {
                            let call = self.open_call(id, cause)?;
                            frames.push(ExprFrame::Call(call), cause)?;
                        }
                        source::ExprKind::StructLiteral { .. } => frames.push(
                            ExprFrame::Literal {
                                expression: id,
                                next: 0,
                            },
                            cause,
                        )?,
                        other => {
                            frames.push(ExprFrame::Emit(id), cause)?;
                            match other {
                                source::ExprKind::Group(inner)
                                | source::ExprKind::Negate { operand: inner, .. }
                                | source::ExprKind::Not { operand: inner, .. }
                                | source::ExprKind::IndexRead { index: inner, .. } => {
                                    frames.push(ExprFrame::Visit(*inner), cause)?
                                }
                                source::ExprKind::Arithmetic { left, right, .. }
                                | source::ExprKind::Comparison { left, right, .. } => {
                                    frames.push(ExprFrame::Visit(*right), cause)?;
                                    frames.push(ExprFrame::Visit(*left), cause)?;
                                }
                                _ => {}
                            }
                        }
                    }
                }
                ExprFrame::Emit(id) => {
                    self.simple_expression(id, cause)?;
                    self.complete(id)?;
                }
                ExprFrame::ArrayLiteral {
                    expression: id,
                    next,
                } => {
                    let expression = &self.view.hir().expressions[id.0];
                    let source::ExprKind::ArrayLiteral { elements } = &expression.kind else {
                        return Err(invariant(expression.span));
                    };
                    if let Some(child) = elements.get(next) {
                        frames.push(
                            ExprFrame::ArrayLiteral {
                                expression: id,
                                next: next + 1,
                            },
                            cause,
                        )?;
                        frames.push(ExprFrame::Visit(*child), cause)?;
                    } else {
                        let ValueTy::Owned(AggregateTy::FixedArray(array)) =
                            self.view.expression_ty(id)
                        else {
                            return Err(invariant(expression.span));
                        };
                        require(array.length() == elements.len(), expression.span)?;
                        budget::cap(elements.len(), 1024, "fixed array length")?;
                        let owner = self.owner(
                            AggregateTy::FixedArray(array),
                            OwnerKind::Temporary,
                            expression.span,
                        )?;
                        self.statement(
                            OwnedInstruction::StorageLive(owner),
                            expression.span,
                            expression.span,
                            cause,
                        )?;
                        self.counts.constructed_elements =
                            budget::add(self.counts.constructed_elements, elements.len())?;
                        self.check_counts()?;
                        let mut values = if self.output.is_some() {
                            #[cfg(test)]
                            super::array_pipeline::literal_context(
                                self.view.hir().id.0,
                                id.0,
                                self.counts,
                            );
                            budget::reserve_array_operands(elements.len())?
                        } else {
                            Vec::new()
                        };
                        if self.output.is_some() {
                            for child in elements {
                                budget::append(
                                    &mut values,
                                    self.operand(*child)?,
                                    elements.len(),
                                    expression.span,
                                )?;
                            }
                        }
                        self.statement(
                            OwnedInstruction::ConstructArray {
                                destination: owner,
                                elements: values,
                            },
                            expression.span,
                            expression.span,
                            cause,
                        )?;
                        self.set_owned(id, owner);
                        self.complete(id)?;
                    }
                }
                ExprFrame::StageField(id) => {
                    let span = self.view.hir().expressions[id.0].span;
                    let ValueTy::Owned(aggregate) = self.view.expression_ty(id) else {
                        return Err(invariant(span));
                    };
                    let EvaluatedValue::Owned(source) = self.value(id)? else {
                        return Err(invariant(span));
                    };
                    let destination = self.owner(aggregate, OwnerKind::Temporary, span)?;
                    self.statement(
                        OwnedInstruction::StorageLive(destination),
                        span,
                        span,
                        cause,
                    )?;
                    self.statement(
                        OwnedInstruction::MoveInitialize {
                            destination,
                            source,
                        },
                        span,
                        span,
                        cause,
                    )?;
                    self.end_value(source, span, span, cause)?;
                    self.set_owned(id, destination);
                }
                ExprFrame::Literal {
                    expression: id,
                    next,
                } => {
                    let expression = &self.view.hir().expressions[id.0];
                    let source::ExprKind::StructLiteral { record, fields } = &expression.kind
                    else {
                        return Err(invariant(expression.span));
                    };
                    if let Some(field) = fields.get(next) {
                        frames.push(
                            ExprFrame::Literal {
                                expression: id,
                                next: next + 1,
                            },
                            cause,
                        )?;
                        if matches!(self.view.expression_ty(field.value), ValueTy::Owned(_)) {
                            frames.push(ExprFrame::StageField(field.value), cause)?;
                        }
                        frames.push(ExprFrame::Visit(field.value), cause)?;
                    } else {
                        let owner = self.owner(
                            AggregateTy::Record(*record),
                            OwnerKind::Temporary,
                            expression.span,
                        )?;
                        self.statement(
                            OwnedInstruction::StorageLive(owner),
                            expression.span,
                            expression.span,
                            cause,
                        )?;
                        let composite = fields.iter().any(|field| {
                            matches!(self.view.expression_ty(field.value), ValueTy::Owned(_))
                        });
                        if composite {
                            self.counts.composite_fields =
                                budget::add(self.counts.composite_fields, fields.len())?;
                        } else {
                            self.counts.constructed_fields =
                                budget::add(self.counts.constructed_fields, fields.len())?;
                        }
                        self.counts.max_constructor_fields = self
                            .counts
                            .max_constructor_fields
                            .max(fields.len().min(1024));
                        self.check_counts()?;
                        if composite {
                            let mut values = if self.output.is_some() {
                                budget::reserve(fields.len())?
                            } else {
                                Vec::new()
                            };
                            if self.output.is_some() {
                                for field in fields {
                                    let value = match self.value(field.value)? {
                                        EvaluatedValue::Scalar(value) => {
                                            FieldInitializer::Scalar(value)
                                        }
                                        EvaluatedValue::Owned(value) => {
                                            FieldInitializer::Owned(value)
                                        }
                                    };
                                    budget::append(
                                        &mut values,
                                        (field.field, value),
                                        fields.len(),
                                        expression.span,
                                    )?;
                                }
                            }
                            self.statement(
                                OwnedInstruction::ConstructComposite {
                                    destination: owner,
                                    fields: values,
                                },
                                expression.span,
                                expression.span,
                                cause,
                            )?;
                            for field in fields {
                                if let EvaluatedValue::Owned(owner) = self.value(field.value)? {
                                    self.end_value(
                                        owner,
                                        expression.span,
                                        self.view.hir().expressions[field.value.0].span,
                                        cause,
                                    )?;
                                }
                            }
                        } else {
                            let mut values = if self.output.is_some() {
                                budget::reserve(fields.len())?
                            } else {
                                Vec::new()
                            };
                            if self.output.is_some() {
                                for field in fields {
                                    budget::append(
                                        &mut values,
                                        (field.field, self.operand(field.value)?),
                                        fields.len(),
                                        expression.span,
                                    )?;
                                }
                            }
                            self.statement(
                                OwnedInstruction::Construct {
                                    destination: owner,
                                    fields: values,
                                },
                                expression.span,
                                expression.span,
                                cause,
                            )?;
                        }
                        self.set_owned(id, owner);
                        self.complete(id)?;
                    }
                }
                ExprFrame::LogicalLeft(id) => {
                    let expression = &self.view.hir().expressions[id.0];
                    let source::ExprKind::Logical {
                        op, left, right, ..
                    } = expression.kind
                    else {
                        return Err(invariant(expression.span));
                    };
                    let predecessor = self.current.ok_or_else(|| invariant(expression.span))?;
                    let rhs = self.block(self.view.hir().expressions[right.0].span)?;
                    let join = self.block(expression.span)?;
                    let (then_block, else_block) = if op == source::LogicalOp::And {
                        (rhs, join)
                    } else {
                        (join, rhs)
                    };
                    self.close(
                        OwnedTerminatorKind::Branch {
                            condition: self.operand(left)?,
                            then_block,
                            else_block,
                        },
                        expression.span,
                        cause,
                    )?;
                    self.enter(Some(rhs), expression.span)?;
                    frames.push(
                        ExprFrame::LogicalRight {
                            expression: id,
                            predecessor,
                            join,
                        },
                        cause,
                    )?;
                    frames.push(ExprFrame::Visit(right), cause)?;
                }
                ExprFrame::LogicalRight {
                    expression: id,
                    predecessor,
                    join,
                } => {
                    let expression = &self.view.hir().expressions[id.0];
                    let source::ExprKind::Logical {
                        left,
                        right,
                        operator_span,
                        ..
                    } = expression.kind
                    else {
                        return Err(invariant(expression.span));
                    };
                    let rhs_end = self.current.ok_or_else(|| invariant(expression.span))?;
                    self.close(OwnedTerminatorKind::Goto(join), expression.span, cause)?;
                    self.enter(Some(join), expression.span)?;
                    self.counts.merges = budget::add(self.counts.merges, 1)?;
                    self.check_counts()?;
                    let merge = BoolMerge {
                        operator_span,
                        destination: self.operand(id)?.local,
                        incoming: [
                            MergeInput {
                                predecessor,
                                value: self.operand(left)?,
                            },
                            MergeInput {
                                predecessor: rhs_end,
                                value: self.operand(right)?,
                            },
                        ],
                        span: expression.span,
                    };
                    if let Some(out) = &mut self.output {
                        require(out.raw.blocks[join.0].merge.is_none(), expression.span)?;
                        out.raw.blocks[join.0].merge = Some(merge);
                    }
                    self.complete(id)?;
                }
                ExprFrame::Call(mut call) => {
                    let expression = &self.view.hir().expressions[call.expression.0];
                    let source::ExprKind::Call { args, .. } = &expression.kind else {
                        return Err(invariant(expression.span));
                    };
                    if let Some(arg) = args.get(call.next) {
                        self.preparing = Some((call.call, call.next));
                        match arg {
                            source::Argument::Value(id) => {
                                frames.push(ExprFrame::Prepare(call), cause)?;
                                frames.push(ExprFrame::Visit(*id), cause)?;
                            }
                            source::Argument::Borrow { span, .. } => {
                                self.statement(
                                    OwnedInstruction::PrepareBorrow {
                                        call: call.call,
                                        argument: call.next,
                                        loan: LoanId(call.next_loan),
                                    },
                                    *span,
                                    *span,
                                    *span,
                                )?;
                                call.next += 1;
                                call.next_loan += 1;
                                frames.push(ExprFrame::Call(call), cause)?;
                            }
                        }
                    } else {
                        let continuation = self.block(expression.span)?;
                        self.close(
                            OwnedTerminatorKind::Invoke {
                                call: call.call,
                                continuation,
                            },
                            expression.span,
                            cause,
                        )?;
                        self.enter(Some(continuation), expression.span)?;
                        if let CallResult::Owned(owner) = call.result {
                            self.set_owned(call.expression, owner);
                        }
                        self.preparing = call.parent;
                        self.complete(call.expression)?;
                    }
                }
                ExprFrame::Prepare(mut call) => {
                    let expression = &self.view.hir().expressions[call.expression.0];
                    let source::ExprKind::Call { args, .. } = &expression.kind else {
                        return Err(invariant(expression.span));
                    };
                    let source::Argument::Value(id) = args[call.next] else {
                        return Err(invariant(expression.span));
                    };
                    let span = self.view.hir().expressions[id.0].span;
                    match self.value(id)? {
                        EvaluatedValue::Scalar(value) => self.statement(
                            OwnedInstruction::PrepareScalar {
                                call: call.call,
                                argument: call.next,
                                value,
                            },
                            span,
                            span,
                            cause,
                        )?,
                        EvaluatedValue::Owned(source) => {
                            self.statement(
                                OwnedInstruction::PrepareOwned {
                                    call: call.call,
                                    argument: call.next,
                                    source,
                                },
                                span,
                                span,
                                cause,
                            )?;
                            self.end_value(source, span, span, cause)?;
                        }
                    }
                    call.next += 1;
                    frames.push(ExprFrame::Call(call), cause)?;
                }
            }
        }
        Ok(())
    }
    fn simple_expression(&mut self, id: source::ExprId, cause: Span) -> Result<()> {
        let expression = &self.view.hir().expressions[id.0];
        let span = expression.span;
        if let ValueTy::Owned(record) = self.view.expression_ty(id) {
            match expression.kind {
                source::ExprKind::ArrayLiteral { .. }
                | source::ExprKind::IndexRead { .. }
                | source::ExprKind::ArrayLength { .. } => return Err(invariant(span)),
                source::ExprKind::Binding(binding) => {
                    let BindingLocation::Owner(source) = self.location(binding, span)? else {
                        return Err(invariant(span));
                    };
                    let destination = self.owner(record, OwnerKind::Temporary, span)?;
                    self.statement(
                        OwnedInstruction::StorageLive(destination),
                        span,
                        span,
                        cause,
                    )?;
                    self.statement(
                        OwnedInstruction::MoveInitialize {
                            destination,
                            source,
                        },
                        span,
                        span,
                        cause,
                    )?;
                    self.set_owned(id, destination);
                }
                source::ExprKind::Group(inner) => {
                    let EvaluatedValue::Owned(owner) = self.value(inner)? else {
                        return Err(invariant(span));
                    };
                    self.set_owned(id, owner);
                }
                _ => return Err(invariant(span)),
            }
            return Ok(());
        }
        let destination = self.operand(id)?.local;
        let value = match expression.kind {
            source::ExprKind::ArrayLiteral { .. } => return Err(invariant(span)),
            source::ExprKind::IndexRead { base, index, .. } => {
                let kind = if let Some(projection) = self.view.expression_projection(id) {
                    OwnedInstruction::ReadProjection {
                        destination,
                        base: self.base(base, span)?,
                        path: self.projection_path(projection, span)?,
                        index: Some(self.operand(index)?),
                    }
                } else {
                    OwnedInstruction::ReadIndex {
                        destination,
                        base: self.base(base, span)?,
                        index: self.operand(index)?,
                    }
                };
                self.statement(kind, span, span, cause)?;
                return Ok(());
            }
            source::ExprKind::ArrayLength { base, .. } => {
                let kind = if let Some(projection) = self.view.expression_projection(id) {
                    OwnedInstruction::ProjectionLength {
                        destination,
                        base: self.base(base, span)?,
                        path: self.projection_path(projection, span)?,
                    }
                } else {
                    OwnedInstruction::ArrayLength {
                        destination,
                        base: self.base(base, span)?,
                    }
                };
                self.statement(kind, span, span, cause)?;
                return Ok(());
            }
            source::ExprKind::Bool(value) => Rvalue::Bool(value),
            source::ExprKind::I32(value) => Rvalue::I32(value),
            source::ExprKind::Unit => Rvalue::Unit,
            source::ExprKind::Binding(binding) => match self.location(binding, span)? {
                BindingLocation::ScalarValue(local) => Rvalue::Copy(Operand { local, span }),
                BindingLocation::ScalarPlace(id) => Rvalue::Load(Place { id, span }),
                _ => return Err(invariant(span)),
            },
            source::ExprKind::Group(inner) => Rvalue::Copy(self.operand(inner)?),
            source::ExprKind::Negate {
                operand,
                operator_span,
            } => Rvalue::CheckedNegateI32 {
                operand: self.operand(operand)?,
                operator_span,
            },
            source::ExprKind::Not {
                operand,
                operator_span,
            } => Rvalue::NotBool {
                operand: self.operand(operand)?,
                operator_span,
            },
            source::ExprKind::Comparison {
                op,
                left,
                right,
                operator_span,
            } => Rvalue::CompareScalar {
                op,
                left: self.operand(left)?,
                right: self.operand(right)?,
                operator_span,
            },
            source::ExprKind::Arithmetic {
                op,
                left,
                right,
                operator_span,
            } => Rvalue::CheckedI32 {
                op,
                left: self.operand(left)?,
                right: self.operand(right)?,
                operator_span,
            },
            source::ExprKind::FieldRead { base, .. } => {
                let projection = self
                    .view
                    .expression_projection(id)
                    .ok_or_else(|| invariant(span))?;
                let kind = if projection.path.len() == 1 {
                    OwnedInstruction::ReadField {
                        destination,
                        base: self.base(base, span)?,
                        field: projection.field,
                    }
                } else {
                    OwnedInstruction::ReadProjection {
                        destination,
                        base: self.base(base, span)?,
                        path: self.projection_path(projection, span)?,
                        index: None,
                    }
                };
                self.statement(kind, span, span, cause)?;
                return Ok(());
            }
            _ => return Err(invariant(span)),
        };
        self.scalar_assign(destination, value, span, cause)
    }
    fn open_call(&mut self, id: source::ExprId, cause: Span) -> Result<CallFrame> {
        let expression = &self.view.hir().expressions[id.0];
        let span = expression.span;
        let source::ExprKind::Call { target, args } = &expression.kind else {
            return Err(invariant(span));
        };
        budget::cap(
            args.len(),
            crate::frontend::parser::MAX_PARAMS,
            "call arguments",
        )?;
        let call = CallSiteId(self.counts.calls);
        self.counts.calls = budget::add(self.counts.calls, 1)?;
        self.counts.descriptor_arguments =
            budget::add(self.counts.descriptor_arguments, args.len())?;
        self.counts.ownership_active = true;
        self.check_counts()?;
        let first_loan = self.counts.loans;
        let mut arguments = if self.output.is_some() {
            budget::reserve(args.len())?
        } else {
            Vec::new()
        };
        for (argument, value) in args.iter().enumerate() {
            let slot = match value {
                source::Argument::Value(id) => match self.view.expression_ty(*id) {
                    ValueTy::Scalar(_) => ArgumentSlot::Scalar,
                    ValueTy::Owned(record) => ArgumentSlot::Owned(self.owner(
                        record,
                        OwnerKind::StagedArgument { call, argument },
                        self.view.hir().expressions[id.0].span,
                    )?),
                },
                source::Argument::Borrow {
                    kind, place, span, ..
                } => {
                    let binding = match place {
                        source::BorrowPlace::Owner(id) | source::BorrowPlace::Forwarded(id) => *id,
                    };
                    // The argument retains its original authority identity, while the
                    // loan carries exactly the callee's contextual borrowed view.
                    let ParameterTy::Reference {
                        referent,
                        kind: expected_kind,
                    } = self.view.call_parameter_ty(*target, argument)
                    else {
                        return Err(invariant(*span));
                    };
                    require(*kind == expected_kind, *span)?;
                    let authority = self.base(binding, *span)?;
                    let projection = match self.view.borrow_projection(id, argument) {
                        Some(projection) => self.projection_path(projection, *span)?,
                        None => Vec::new(),
                    };
                    let loan = LoanId(self.counts.loans);
                    self.counts.loans = budget::add(self.counts.loans, 1)?;
                    if let Some(out) = &mut self.output {
                        budget::append(
                            &mut out.raw.loans,
                            LoanDecl {
                                call,
                                argument,
                                authority,
                                projection,
                                kind: *kind,
                                referent: BorrowedSlot::check(referent)?,
                                span: *span,
                            },
                            out.expected.loans,
                            *span,
                        )?;
                    }
                    ArgumentSlot::Borrow(loan)
                }
            };
            if self.output.is_some() {
                budget::append(&mut arguments, slot, args.len(), span)?;
            }
        }
        let result = match self.view.expression_ty(id) {
            ValueTy::Scalar(_) => CallResult::Scalar(self.operand(id)?.local),
            ValueTy::Owned(record) => {
                CallResult::Owned(self.owner(record, OwnerKind::CallResult { call }, span)?)
            }
        };
        if let Some(out) = &mut self.output {
            budget::append(
                &mut out.raw.calls,
                CallDecl {
                    target: *target,
                    arguments,
                    result,
                    parent: self.preparing,
                    span,
                },
                out.expected.calls,
                span,
            )?;
        }
        self.statement(OwnedInstruction::OpenCall(call), span, span, cause)?;
        Ok(CallFrame {
            expression: id,
            call,
            next: 0,
            next_loan: first_loan,
            result,
            parent: self.preparing,
        })
    }

    fn body(&mut self) -> Result<()> {
        let f = self.view.hir();
        let span = self.view.signature().span;
        let mut frames = Stack::<BodyFrame, BODY_FRAMES>::new();
        let mut loops = Stack::<LoopTargets, LOOP_FRAMES>::new();
        frames.push(
            BodyFrame::Body {
                block: f.body,
                next: 0,
            },
            span,
        )?;
        while let Some(frame) = frames.pop() {
            let (block, index, statement) = match frame {
                BodyFrame::Body { block, next } => {
                    let Some(statement) = f.blocks[block.0].body.get(next) else {
                        continue;
                    };
                    require(self.current.is_some(), statement.span)?;
                    frames.push(
                        BodyFrame::Body {
                            block,
                            next: next + 1,
                        },
                        statement.span,
                    )?;
                    (block, next, statement)
                }
                BodyFrame::Enter {
                    block,
                    entry,
                    join,
                    prefix,
                } => {
                    self.restore(prefix, f.blocks[block.0].span)?;
                    self.enter(Some(entry), f.blocks[block.0].span)?;
                    frames.push(
                        BodyFrame::Finish {
                            end: f.blocks[block.0].end,
                            join,
                            prefix,
                        },
                        span,
                    )?;
                    frames.push(BodyFrame::Body { block, next: 0 }, span)?;
                    continue;
                }
                BodyFrame::Finish { end, join, prefix } => {
                    if self.current.is_some() {
                        self.cleanup(prefix, end)?;
                        self.close(
                            OwnedTerminatorKind::Goto(join.ok_or_else(|| invariant(end))?),
                            end,
                            end,
                        )?;
                    }
                    self.restore(prefix, end)?;
                    continue;
                }
                BodyFrame::Join { block, span } => {
                    self.enter(block, span)?;
                    continue;
                }
                BodyFrame::LoopEnd {
                    id,
                    exit,
                    span,
                    prefix,
                } => {
                    require(loops.pop().is_some_and(|entry| entry.id == id), span)?;
                    self.restore(prefix, span)?;
                    self.enter(Some(exit), span)?;
                    continue;
                }
            };
            let s = statement.span;
            if let source::StmtKind::While {
                loop_id,
                condition,
                body,
            } = statement.kind
            {
                let prefix = self.active;
                let header = self.block(s)?;
                let entry = self.block(f.blocks[body.0].span)?;
                let exit = self.block(s)?;
                require(loop_id.0 == body.0, s)?;
                loops.push(
                    LoopTargets {
                        id: loop_id,
                        header,
                        exit,
                        prefix,
                    },
                    s,
                )?;
                self.close(OwnedTerminatorKind::Goto(header), s, s)?;
                self.enter(Some(header), s)?;
                self.expression(condition, s)?;
                self.close(
                    OwnedTerminatorKind::Branch {
                        condition: self.operand(condition)?,
                        then_block: entry,
                        else_block: exit,
                    },
                    s,
                    s,
                )?;
                frames.push(
                    BodyFrame::LoopEnd {
                        id: loop_id,
                        exit,
                        span: s,
                        prefix,
                    },
                    s,
                )?;
                frames.push(
                    BodyFrame::Enter {
                        block: body,
                        entry,
                        join: Some(header),
                        prefix,
                    },
                    s,
                )?;
                continue;
            }
            let statement_index = index;
            if let source::StmtKind::IndexAssign {
                base,
                target_span,
                value,
                index,
                ..
            } = statement.kind
            {
                self.expression(value, s)?;
                self.expression(index, s)?;
                let kind = if let Some(projection) =
                    self.view.statement_projection(block, statement_index)
                {
                    OwnedInstruction::WriteProjection {
                        base: self.base(base, target_span)?,
                        path: self.projection_path(projection, target_span)?,
                        index: Some(self.operand(index)?),
                        value: self.operand(value)?,
                    }
                } else {
                    OwnedInstruction::WriteIndex {
                        base: self.base(base, target_span)?,
                        index: self.operand(index)?,
                        value: self.operand(value)?,
                    }
                };
                self.statement(kind, s, target_span, s)?;
                continue;
            }
            let root = match statement.kind {
                source::StmtKind::Let { init, .. }
                | source::StmtKind::Assign { value: init, .. }
                | source::StmtKind::FieldAssign { value: init, .. }
                | source::StmtKind::Expr(init) => Some(init),
                source::StmtKind::Return(value) => value,
                source::StmtKind::If { condition, .. } => Some(condition),
                _ => None,
            };
            if let Some(root) = root {
                self.expression(root, s)?;
            }
            match statement.kind {
                source::StmtKind::IndexAssign { .. } => return Err(invariant(s)),
                source::StmtKind::Let { binding, init } => match self.value(init)? {
                    EvaluatedValue::Scalar(value) => match self.location(binding, s)? {
                        BindingLocation::ScalarValue(destination) => {
                            self.scalar_assign(destination, Rvalue::Copy(value), s, s)?
                        }
                        BindingLocation::ScalarPlace(id) => self.statement(
                            OwnedInstruction::Scalar(Statement::Initialize {
                                place: Place {
                                    id,
                                    span: f.bindings[binding.0].span,
                                },
                                value,
                                span: s,
                            }),
                            s,
                            s,
                            s,
                        )?,
                        _ => return Err(invariant(s)),
                    },
                    EvaluatedValue::Owned(source) => {
                        let ParameterTy::Value(ValueTy::Owned(record)) =
                            self.view.binding_ty(binding)
                        else {
                            return Err(invariant(s));
                        };
                        let declaration = &f.bindings[binding.0];
                        let destination = self.owner(
                            record,
                            OwnerKind::Local {
                                mutable: declaration.mutable,
                            },
                            declaration.span,
                        )?;
                        if let Some(out) = &mut self.output {
                            require(out.bindings[binding.0].is_none(), s)?;
                            out.bindings[binding.0] = Some(BindingLocation::Owner(destination));
                        }
                        self.statement(
                            OwnedInstruction::StorageLive(destination),
                            s,
                            declaration.span,
                            s,
                        )?;
                        self.statement(
                            OwnedInstruction::MoveInitialize {
                                destination,
                                source,
                            },
                            s,
                            declaration.span,
                            s,
                        )?;
                        self.end_value(source, s, f.expressions[init.0].span, s)?;
                        self.register(destination, s)?;
                    }
                },
                source::StmtKind::Assign {
                    binding,
                    target_span,
                    operator_span,
                    value,
                } => match self.value(value)? {
                    EvaluatedValue::Scalar(value) => {
                        let BindingLocation::ScalarPlace(id) =
                            self.location(binding, target_span)?
                        else {
                            return Err(invariant(s));
                        };
                        self.statement(
                            OwnedInstruction::Scalar(Statement::Store {
                                place: Place {
                                    id,
                                    span: target_span,
                                },
                                value,
                                operator_span,
                                span: s,
                            }),
                            s,
                            target_span,
                            s,
                        )?;
                    }
                    EvaluatedValue::Owned(source) => {
                        let BindingLocation::Owner(destination) =
                            self.location(binding, target_span)?
                        else {
                            return Err(invariant(s));
                        };
                        self.statement(
                            OwnedInstruction::Replace {
                                destination,
                                source,
                            },
                            s,
                            target_span,
                            s,
                        )?;
                        self.end_value(source, s, f.expressions[value.0].span, s)?;
                    }
                },
                source::StmtKind::FieldAssign {
                    base,
                    target_span,
                    value,
                    ..
                } => {
                    let projection = self
                        .view
                        .statement_projection(block, index)
                        .ok_or_else(|| invariant(s))?;
                    let kind = if projection.path.len() == 1 {
                        OwnedInstruction::WriteField {
                            base: self.base(base, target_span)?,
                            field: projection.field,
                            value: self.operand(value)?,
                        }
                    } else {
                        OwnedInstruction::WriteProjection {
                            base: self.base(base, target_span)?,
                            path: self.projection_path(projection, target_span)?,
                            index: None,
                            value: self.operand(value)?,
                        }
                    };
                    self.statement(kind, s, target_span, s)?;
                }
                source::StmtKind::Expr(id) => {
                    if let EvaluatedValue::Owned(owner) = self.value(id)? {
                        self.statement(
                            OwnedInstruction::Discard(owner),
                            s,
                            f.expressions[id.0].span,
                            s,
                        )?;
                        self.end_value(owner, s, f.expressions[id.0].span, s)?;
                    }
                }
                source::StmtKind::Return(value) => {
                    let value = match value {
                        Some(id) => self.value(id)?,
                        None => {
                            let local = self.local(hir::Ty::Unit, LocalKind::Temporary, s)?;
                            self.scalar_assign(local, Rvalue::Unit, s, s)?;
                            EvaluatedValue::Scalar(Operand { local, span: s })
                        }
                    };
                    self.cleanup(0, s)?;
                    self.close(
                        match value {
                            EvaluatedValue::Scalar(value) => {
                                OwnedTerminatorKind::ReturnScalar(value)
                            }
                            EvaluatedValue::Owned(owner) => OwnedTerminatorKind::ReturnOwned(owner),
                        },
                        s,
                        s,
                    )?;
                }
                source::StmtKind::Break { target } | source::StmtKind::Continue { target } => {
                    let targets = loops
                        .last()
                        .filter(|entry| entry.id == target)
                        .ok_or_else(|| invariant(s))?;
                    self.cleanup(targets.prefix, s)?;
                    self.close(
                        OwnedTerminatorKind::Goto(
                            if matches!(statement.kind, source::StmtKind::Break { .. }) {
                                targets.exit
                            } else {
                                targets.header
                            },
                        ),
                        s,
                        s,
                    )?;
                }
                source::StmtKind::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    let prefix = self.active;
                    let then_entry = self.block(f.blocks[then_block.0].span)?;
                    let else_entry = match else_block {
                        Some(id) => Some(self.block(f.blocks[id.0].span)?),
                        None => None,
                    };
                    let joins = self.view.block_flow(then_block).falls_through()
                        || else_block.is_none_or(|id| self.view.block_flow(id).falls_through());
                    let join = if joins { Some(self.block(s)?) } else { None };
                    self.close(
                        OwnedTerminatorKind::Branch {
                            condition: self.operand(condition)?,
                            then_block: then_entry,
                            else_block: else_entry.or(join).ok_or_else(|| invariant(s))?,
                        },
                        s,
                        s,
                    )?;
                    frames.push(
                        BodyFrame::Join {
                            block: join,
                            span: s,
                        },
                        s,
                    )?;
                    if let (Some(block), Some(entry)) = (else_block, else_entry) {
                        frames.push(
                            BodyFrame::Enter {
                                block,
                                entry,
                                join,
                                prefix,
                            },
                            s,
                        )?;
                    }
                    frames.push(
                        BodyFrame::Enter {
                            block: then_block,
                            entry: then_entry,
                            join,
                            prefix,
                        },
                        s,
                    )?;
                }
                source::StmtKind::While { .. } => return Err(invariant(s)),
            }
        }
        require(
            loops.len == 0
                && self.current.is_none()
                && self.preparing.is_none()
                && self.next_expression == f.expressions.len(),
            span,
        )?;
        if let Some(out) = &self.output {
            require(out.raw.blocks.iter().all(|b| b.terminator.is_some()), span)?;
            for (index, b) in out.raw.blocks.iter().enumerate() {
                require(
                    b.statements.len()
                        == self.block_counts.as_ref().ok_or_else(|| invariant(span))?[index],
                    span,
                )?;
            }
        }
        self.check_counts()
    }
}

pub(super) fn count_function(
    view: &TypedOwnedFunction<'_>,
    blocks: Option<&mut [usize]>,
) -> Result<Counts> {
    #[cfg(test)]
    let pass = if blocks.is_some() {
        super::array_pipeline::LowerPass::BlockCount
    } else {
        super::array_pipeline::LowerPass::EmissionCount
    };
    count_function_for(
        view,
        blocks,
        #[cfg(test)]
        pass,
    )
}
pub(super) fn count_preflight_function(view: &TypedOwnedFunction<'_>) -> Result<Counts> {
    count_function_for(
        view,
        None,
        #[cfg(test)]
        super::array_pipeline::LowerPass::PreflightCount,
    )
}
fn count_function_for(
    view: &TypedOwnedFunction<'_>,
    blocks: Option<&mut [usize]>,
    #[cfg(test)] pass: super::array_pipeline::LowerPass,
) -> Result<Counts> {
    let mut walk = Walk::new(
        view,
        blocks,
        None,
        #[cfg(test)]
        pass,
    )?;
    #[cfg(test)]
    budget::guard_event(budget::GuardEvent::CountEntry);
    let result = walk.body();
    #[cfg(test)]
    super::array_pipeline::counted(pass, view.hir().id.0, walk.counts, result.is_ok());
    result?;
    Ok(walk.counts)
}
pub(super) fn scratch_bytes(view: &TypedOwnedFunction<'_>, count: Counts) -> Result<usize> {
    let mut bytes = 0;
    for (n, width) in [
        (count.blocks, size_of::<usize>()),
        (
            view.hir().bindings.len(),
            size_of::<Option<BindingLocation>>(),
        ),
        (
            view.hir().expressions.len(),
            size_of::<Option<EvaluatedValue>>(),
        ),
        (count.owners, size_of::<OwnerPlaceId>()),
    ] {
        bytes = budget::add(bytes, budget::mul(n, width)?)?;
    }
    Ok(bytes)
}
pub(super) fn lower(typed: &TypedOwnedProgram<'_>) -> Result<RawOwnedProgram> {
    lower_with_limits(typed, budget::Limits::DEFAULT)
}
pub(super) fn lower_with_limits(
    typed: &TypedOwnedProgram<'_>,
    limits: budget::Limits,
) -> Result<RawOwnedProgram> {
    let expected = budget::preflight(typed, limits)?;
    let mut records = budget::reserve(typed.records().len())?;
    for record in typed.records() {
        let mut fields = budget::reserve(record.fields.len())?;
        for field in &record.fields {
            budget::append(
                &mut fields,
                RawFieldDecl {
                    id: field.id,
                    ty: ParameterTy::Value(field.ty),
                    span: field.name_span,
                },
                record.fields.len(),
                field.name_span,
            )?;
        }
        budget::append(
            &mut records,
            RawRecordDecl {
                id: record.id,
                span: record.name_span,
                fields,
            },
            typed.records().len(),
            record.name_span,
        )?;
    }
    let mut functions = budget::reserve(typed.functions().len())?;
    let mut bytes = budget::add(
        size_of::<RawOwnedProgram>(),
        budget::mul(records.len(), size_of::<RawRecordDecl>())?,
    )?;
    for r in &records {
        bytes = budget::add(
            bytes,
            budget::mul(r.fields.len(), size_of::<RawFieldDecl>())?,
        )?;
    }
    for view in typed.functions() {
        let count = count_function(&view, None)?;
        let mut blocks = budget::filled(count.blocks, 0usize)?;
        require(
            count_function(&view, Some(&mut blocks))? == count,
            view.signature().span,
        )?;
        #[cfg(test)]
        for (block, statements) in blocks.iter().copied().enumerate() {
            super::array_pipeline::counted_block(view.hir().id.0, block, statements);
        }
        let mut walk = Walk::new(
            &view,
            Some(&mut blocks),
            Some(count),
            #[cfg(test)]
            super::array_pipeline::LowerPass::Emit,
        )?;
        let result = walk.body();
        #[cfg(test)]
        if result.is_err() {
            super::array_pipeline::counted(
                super::array_pipeline::LowerPass::Emit,
                view.hir().id.0,
                walk.counts,
                false,
            );
        }
        result?;
        require(walk.counts == count, view.signature().span)?;
        let output = walk
            .output
            .take()
            .ok_or_else(|| invariant(view.signature().span))?;
        let f = output.raw;
        require(
            f.parameters.len() == count.parameters
                && f.locals.len() == count.locals
                && f.places.len() == count.places
                && f.owners.len() == count.owners
                && f.references.len() == count.references
                && f.calls.len() == count.calls
                && f.loans.len() == count.loans
                && f.blocks.len() == count.blocks,
            view.signature().span,
        )?;
        #[cfg(test)]
        super::array_pipeline::counted(
            super::array_pipeline::LowerPass::Emit,
            view.hir().id.0,
            walk.counts,
            true,
        );
        bytes = budget::add(bytes, budget::function_bytes(count)?)?;
        budget::append(
            &mut functions,
            f,
            typed.functions().len(),
            view.signature().span,
        )?;
    }
    if bytes != expected.raw_bytes {
        let mut error = OwnedFailure::resource("source count mismatch");
        error.kind = OwnedFailureKind::Malformed(Malformed::CanonicalSite);
        return Err(error);
    }
    Ok(RawOwnedProgram { records, functions })
}

/// Exercises the same emission constructor before any output work. Kept inside
/// the source test boundary; it cannot return raw storage or executable authority.
#[cfg(test)]
pub(super) fn check_array_type_emission_fence(view: &TypedOwnedFunction<'_>) -> Result<()> {
    Walk::new(
        view,
        None,
        Some(Counts::default()),
        super::array_pipeline::LowerPass::Emit,
    )
    .map(|_| ())
}

#[test]
fn unit3b2_lowering_layout_without_source_observation() {
    assert_eq!((EXPR_FRAMES, BODY_FRAMES, LOOP_FRAMES), (203, 268, 65));
    macro_rules! layout {
        ($name:literal, $ty:ty) => {
            println!(
                "LAYOUT {{\"name\":{},\"bytes\":{},\"align\":{}}}",
                concat!("\"", $name, "\""),
                size_of::<$ty>(),
                std::mem::align_of::<$ty>()
            );
        };
    }
    layout!("ExprFrame", ExprFrame);
    layout!("Option<ExprFrame>", Option<ExprFrame>);
    layout!("Stack<ExprFrame,203>", Stack<ExprFrame, EXPR_FRAMES>);
    layout!("BodyFrame", BodyFrame);
    layout!("Option<BodyFrame>", Option<BodyFrame>);
    layout!("Stack<BodyFrame,268>", Stack<BodyFrame, BODY_FRAMES>);
    layout!("LoopTargets", LoopTargets);
    layout!("Option<LoopTargets>", Option<LoopTargets>);
    layout!("Stack<LoopTargets,65>", Stack<LoopTargets, LOOP_FRAMES>);
    layout!("CallFrame", CallFrame);
    layout!("Walk", Walk<'_, '_>);
    layout!("Output", Output);
    layout!("Option<Output>", Option<Output>);
    layout!("FunctionCounts", Counts);
    layout!("BindingLocation", BindingLocation);
    layout!("Option<BindingLocation>", Option<BindingLocation>);
    layout!("EvaluatedValue", EvaluatedValue);
    layout!("Option<EvaluatedValue>", Option<EvaluatedValue>);
    layout!("Vec<Operand>", Vec<Operand>);
    layout!("Operand", Operand);
    layout!("SourceAdmission", super::resolve::SourceAdmission);
    layout!(
        "ResolvedOwnedProgram",
        super::resolve::ResolvedOwnedProgram<'_>
    );
    layout!("TypedOwnedProgram", TypedOwnedProgram<'_>);
    layout!("TypedOwnedFunction", TypedOwnedFunction<'_>);
    layout!("Allocator", crate::frontend::project::budget::Allocator);
    layout!(
        "ReserveEvent",
        crate::frontend::project::budget::ReserveEvent
    );
}
