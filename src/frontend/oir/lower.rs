use super::*;

const STAGE: &str = "oir-lower";
#[derive(Clone, Copy)]
enum BindingLocation {
    Value(LocalId),
    Place(PlaceId),
}
#[derive(Clone, Copy)]
struct LoopTargets {
    header: BlockId,
    exit: BlockId,
}
fn failure(kind: FailureKind, span: Span) -> OirFailure {
    OirFailure::new(kind, STAGE, Some(span))
}

/// Exact aggregate expansion, before allocating OIR storage/maps. One condition
/// creates a then entry, an optional else entry and only a reachable join.
fn preflight(typed: &typeck::TypedProgram) -> Result<Budget, OirFailure> {
    let mut budget = Budget::default();
    for view in typed.functions() {
        let function = view.hir();
        let span = Some(view.signature().span);
        for amount in [function.locals.len(), function.expressions.len()] {
            Budget::add(
                &mut budget.locals,
                amount,
                MAX_LOCALS,
                "locals",
                STAGE,
                span,
            )?;
        }
        Budget::add(&mut budget.blocks, 1, MAX_BLOCKS, "blocks", STAGE, span)?;
        for expr in &function.expressions {
            if matches!(expr.kind, hir::ExprKind::Logical { .. }) {
                Budget::add(
                    &mut budget.blocks,
                    2,
                    MAX_BLOCKS,
                    "blocks",
                    STAGE,
                    Some(expr.span),
                )?;
            }
            match &expr.kind {
                hir::ExprKind::Call { args, .. } => {
                    if args.len() > super::super::parser::MAX_PARAMS {
                        return Err(failure(
                            FailureKind::ResourceLimit("call arguments"),
                            expr.span,
                        ));
                    }
                    Budget::add(
                        &mut budget.blocks,
                        1,
                        MAX_BLOCKS,
                        "blocks",
                        STAGE,
                        Some(expr.span),
                    )?;
                }
                _ => Budget::add(
                    &mut budget.assignments,
                    1,
                    MAX_ASSIGNMENTS,
                    "assignments",
                    STAGE,
                    Some(expr.span),
                )?,
            }
        }
        for block in &function.blocks {
            for statement in &block.body {
                if matches!(statement.kind, hir::StmtKind::Return(None)) {
                    Budget::add(
                        &mut budget.locals,
                        1,
                        MAX_LOCALS,
                        "locals",
                        STAGE,
                        Some(statement.span),
                    )?;
                }
                if matches!(
                    statement.kind,
                    hir::StmtKind::Let { .. }
                        | hir::StmtKind::Assign { .. }
                        | hir::StmtKind::Return(None)
                ) {
                    Budget::add(
                        &mut budget.assignments,
                        1,
                        MAX_ASSIGNMENTS,
                        "assignments",
                        STAGE,
                        Some(statement.span),
                    )?;
                }
                if matches!(statement.kind, hir::StmtKind::While { .. }) {
                    Budget::add(
                        &mut budget.blocks,
                        3,
                        MAX_BLOCKS,
                        "blocks",
                        STAGE,
                        Some(statement.span),
                    )?;
                }
                if let hir::StmtKind::If {
                    then_block,
                    else_block,
                    ..
                } = statement.kind
                {
                    let joins = view.block_flow(then_block).falls_through()
                        || else_block.is_none_or(|id| view.block_flow(id).falls_through());
                    let count = 1 + usize::from(else_block.is_some()) + usize::from(joins);
                    Budget::add(
                        &mut budget.blocks,
                        count,
                        MAX_BLOCKS,
                        "blocks",
                        STAGE,
                        Some(statement.span),
                    )?;
                }
            }
        }
    }
    Ok(budget)
}

struct BlockBuilder {
    blocks: Vec<BasicBlock>,
    current: Option<BlockId>,
}
impl BlockBuilder {
    fn new(span: Span) -> Self {
        Self {
            blocks: vec![BasicBlock {
                merge: None,
                span,
                statements: vec![],
                terminator: None,
            }],
            current: Some(BlockId(0)),
        }
    }
    fn reserve(&mut self, span: Span) -> BlockId {
        let id = BlockId(self.blocks.len());
        self.blocks.push(BasicBlock {
            merge: None,
            span,
            statements: vec![],
            terminator: None,
        });
        id
    }
    fn enter(&mut self, id: Option<BlockId>, span: Span) -> Result<(), OirFailure> {
        if self.current.is_some() || id.is_some_and(|id| self.blocks[id.0].terminator.is_some()) {
            return Err(failure(FailureKind::BuilderClosed, span));
        }
        self.current = id;
        Ok(())
    }
    fn assign(&mut self, assign: Assign) -> Result<(), OirFailure> {
        let id = self
            .current
            .ok_or_else(|| failure(FailureKind::BuilderClosed, assign.span))?;
        self.blocks[id.0].statements.push(Statement::Assign(assign));
        Ok(())
    }
    fn statement(&mut self, statement: Statement) -> Result<(), OirFailure> {
        let id = self
            .current
            .ok_or_else(|| failure(FailureKind::BuilderClosed, statement.span()))?;
        self.blocks[id.0].statements.push(statement);
        Ok(())
    }
    fn close(&mut self, terminator: Terminator) -> Result<(), OirFailure> {
        let id = self
            .current
            .take()
            .ok_or_else(|| failure(FailureKind::BuilderClosed, terminator.span))?;
        self.blocks[id.0].terminator = Some(terminator);
        Ok(())
    }
    fn call(
        &mut self,
        target: hir::DefId,
        args: Vec<Operand>,
        destination: LocalId,
        span: Span,
    ) -> Result<(), OirFailure> {
        let continuation = self.reserve(span);
        self.close(Terminator {
            span,
            kind: TerminatorKind::Call {
                target,
                args,
                destination,
                continuation,
            },
        })?;
        self.enter(Some(continuation), span)
    }
    fn finish(self, span: Span) -> Result<Vec<BasicBlock>, OirFailure> {
        if self.current.is_some() || self.blocks.iter().any(|b| b.terminator.is_none()) {
            return Err(failure(FailureKind::IncompleteBody, span));
        }
        Ok(self.blocks)
    }
}

/// Explicit source-body stages preserve lexical expression order while moving
/// the open OIR path between arms. IDs own no recursive statement trees.
enum Frame {
    Body {
        id: hir::BodyBlockId,
        next: usize,
    },
    EnterArm {
        id: hir::BodyBlockId,
        entry: BlockId,
        join: Option<BlockId>,
    },
    FinishArm {
        end: Span,
        join: Option<BlockId>,
    },
    Join {
        id: Option<BlockId>,
        span: Span,
    },
    FinishLoop {
        loop_id: hir::LoopId,
        exit: BlockId,
        span: Span,
    },
}

// Expression structure, not the flat HIR tape, chooses execution paths. The
// HIR tape remains postorder and is checked exactly once as nodes are completed.
enum ExprFrame {
    Visit(hir::ExprId),
    Emit(hir::ExprId),
    LogicalLeft(hir::ExprId),
    LogicalRight {
        id: hir::ExprId,
        predecessor: BlockId,
        join: BlockId,
    },
}
fn complete_expression(id: hir::ExprId, next: &mut usize, span: Span) -> Result<(), OirFailure> {
    if id.0 != *next {
        return Err(failure(FailureKind::IncompleteBody, span));
    }
    *next += 1; // Bounded by the preflighted HIR expression table.
    Ok(())
}
fn lower_expression(
    function: &hir::Function,
    local_map: &[BindingLocation],
    expression_map: &[LocalId],
    root: hir::ExprId,
    builder: &mut BlockBuilder,
    next: &mut usize,
) -> Result<(), OirFailure> {
    let operand = |id: hir::ExprId| Operand {
        local: expression_map[id.0],
        span: function.expressions[id.0].span,
    };
    let mut frames = vec![ExprFrame::Visit(root)];
    while let Some(frame) = frames.pop() {
        match frame {
            ExprFrame::Visit(id) => {
                let expr = &function.expressions[id.0];
                if let hir::ExprKind::Logical { left, .. } = expr.kind {
                    frames.push(ExprFrame::LogicalLeft(id));
                    frames.push(ExprFrame::Visit(left));
                    continue;
                }
                frames.push(ExprFrame::Emit(id));
                match &expr.kind {
                    hir::ExprKind::Negate { operand: inner, .. }
                    | hir::ExprKind::Not { operand: inner, .. }
                    | hir::ExprKind::Group(inner) => frames.push(ExprFrame::Visit(*inner)),
                    hir::ExprKind::Arithmetic { left, right, .. }
                    | hir::ExprKind::Comparison { left, right, .. } => {
                        frames.push(ExprFrame::Visit(*right));
                        frames.push(ExprFrame::Visit(*left));
                    }
                    hir::ExprKind::Call { args, .. } => {
                        for &arg in args.iter().rev() {
                            frames.push(ExprFrame::Visit(arg));
                        }
                    }
                    _ => {}
                }
            }
            ExprFrame::LogicalLeft(id) => {
                let expr = &function.expressions[id.0];
                let hir::ExprKind::Logical {
                    op, left, right, ..
                } = expr.kind
                else {
                    return Err(failure(FailureKind::IncompleteBody, expr.span));
                };
                let predecessor = builder
                    .current
                    .ok_or_else(|| failure(FailureKind::BuilderClosed, expr.span))?;
                let rhs = builder.reserve(function.expressions[right.0].span);
                let join = builder.reserve(expr.span);
                let (then_block, else_block) = match op {
                    hir::LogicalOp::And => (rhs, join),
                    hir::LogicalOp::Or => (join, rhs),
                };
                builder.close(Terminator {
                    kind: TerminatorKind::Branch {
                        condition: operand(left),
                        then_block,
                        else_block,
                    },
                    span: expr.span,
                })?;
                builder.enter(Some(rhs), expr.span)?;
                frames.push(ExprFrame::LogicalRight {
                    id,
                    predecessor,
                    join,
                });
                frames.push(ExprFrame::Visit(right));
            }
            ExprFrame::LogicalRight {
                id,
                predecessor,
                join,
            } => {
                let expr = &function.expressions[id.0];
                let hir::ExprKind::Logical {
                    left,
                    right,
                    operator_span,
                    ..
                } = expr.kind
                else {
                    return Err(failure(FailureKind::IncompleteBody, expr.span));
                };
                let rhs_end = builder
                    .current
                    .ok_or_else(|| failure(FailureKind::BuilderClosed, expr.span))?;
                builder.close(Terminator {
                    kind: TerminatorKind::Goto { target: join },
                    span: expr.span,
                })?;
                builder.enter(Some(join), expr.span)?;
                builder.blocks[join.0].merge = Some(BoolMerge {
                    operator_span,
                    destination: expression_map[id.0],
                    incoming: [
                        MergeInput {
                            predecessor,
                            value: operand(left),
                        },
                        MergeInput {
                            predecessor: rhs_end,
                            value: operand(right),
                        },
                    ],
                    span: expr.span,
                });
                complete_expression(id, next, expr.span)?;
            }
            ExprFrame::Emit(id) => {
                let expr = &function.expressions[id.0];
                let destination = expression_map[id.0];
                let value = match &expr.kind {
                    hir::ExprKind::Bool(value) => Some(Rvalue::Bool(*value)),
                    hir::ExprKind::I32(value) => Some(Rvalue::I32(*value)),
                    hir::ExprKind::Unit => Some(Rvalue::Unit),
                    hir::ExprKind::Local(id) => Some(match local_map[id.0] {
                        BindingLocation::Value(local) => Rvalue::Copy(Operand {
                            local,
                            span: expr.span,
                        }),
                        BindingLocation::Place(id) => Rvalue::Load(Place {
                            id,
                            span: expr.span,
                        }),
                    }),
                    hir::ExprKind::Group(inner) => Some(Rvalue::Copy(operand(*inner))),
                    hir::ExprKind::Negate {
                        operand: inner,
                        operator_span,
                    } => Some(Rvalue::CheckedNegateI32 {
                        operand: operand(*inner),
                        operator_span: *operator_span,
                    }),
                    hir::ExprKind::Not {
                        operand: inner,
                        operator_span,
                    } => Some(Rvalue::NotBool {
                        operand: operand(*inner),
                        operator_span: *operator_span,
                    }),
                    hir::ExprKind::Comparison {
                        op,
                        left,
                        right,
                        operator_span,
                    } => Some(Rvalue::CompareScalar {
                        op: *op,
                        left: operand(*left),
                        right: operand(*right),
                        operator_span: *operator_span,
                    }),
                    hir::ExprKind::Arithmetic {
                        op,
                        left,
                        right,
                        operator_span,
                    } => Some(Rvalue::CheckedI32 {
                        op: *op,
                        left: operand(*left),
                        right: operand(*right),
                        operator_span: *operator_span,
                    }),
                    hir::ExprKind::Call { target, args } => {
                        builder.call(
                            *target,
                            args.iter().map(|id| operand(*id)).collect(),
                            destination,
                            expr.span,
                        )?;
                        None
                    }
                    hir::ExprKind::Logical { .. } => {
                        return Err(failure(FailureKind::IncompleteBody, expr.span))
                    }
                };
                if let Some(value) = value {
                    builder.assign(Assign {
                        destination,
                        value,
                        span: expr.span,
                    })?;
                }
                complete_expression(id, next, expr.span)?;
            }
        }
    }
    Ok(())
}

pub(super) fn lower(typed: &typeck::TypedProgram) -> Result<Program, OirFailure> {
    preflight(typed)?;
    let mut functions = Vec::with_capacity(typed.functions().len());
    for view in typed.functions() {
        let function = view.hir();
        let signature = view.signature();
        let mut locals = Vec::new();
        let mut places = Vec::new();
        let mut local_map = Vec::with_capacity(function.locals.len());
        for (index, local) in function.locals.iter().enumerate() {
            if local.mutable {
                local_map.push(BindingLocation::Place(PlaceId(places.len())));
                places.push(PlaceDecl {
                    ty: view.local_ty(hir::LocalId(index)),
                    span: local.span,
                });
                continue;
            }
            local_map.push(BindingLocation::Value(LocalId(locals.len())));
            locals.push(LocalDecl {
                ty: view.local_ty(hir::LocalId(index)),
                span: local.span,
                kind: if index < signature.params.len() {
                    LocalKind::Parameter
                } else {
                    LocalKind::Binding
                },
            });
        }
        let mut expression_map = Vec::with_capacity(function.expressions.len());
        for (index, expr) in function.expressions.iter().enumerate() {
            expression_map.push(LocalId(locals.len()));
            locals.push(LocalDecl {
                ty: view.expression_ty(hir::ExprId(index)),
                kind: LocalKind::Temporary,
                span: expr.span,
            });
        }
        let operand = |id: hir::ExprId| Operand {
            local: expression_map[id.0],
            span: function.expressions[id.0].span,
        };
        let mut builder = BlockBuilder::new(signature.span);
        let mut loop_targets = vec![None; function.blocks.len()];
        let mut active_loops = Vec::new();
        let mut next_expr = 0;
        let mut frames = vec![Frame::Body {
            id: function.body,
            next: 0,
        }];
        while let Some(frame) = frames.pop() {
            let statement = match frame {
                Frame::Body { id, next } => {
                    let Some(statement) = function.blocks[id.0].body.get(next) else {
                        continue;
                    };
                    frames.push(Frame::Body { id, next: next + 1 });
                    statement
                }
                Frame::EnterArm { id, entry, join } => {
                    let arm = &function.blocks[id.0];
                    builder.enter(Some(entry), arm.span)?;
                    frames.push(Frame::FinishArm { end: arm.end, join });
                    frames.push(Frame::Body { id, next: 0 });
                    continue;
                }
                Frame::FinishArm { end, join } => {
                    if builder.current.is_some() {
                        let target =
                            join.ok_or_else(|| failure(FailureKind::IncompleteBody, end))?;
                        builder.close(Terminator {
                            span: end,
                            kind: TerminatorKind::Goto { target },
                        })?;
                    }
                    continue;
                }
                Frame::Join { id, span } => {
                    builder.enter(id, span)?;
                    continue;
                }
                Frame::FinishLoop {
                    loop_id,
                    exit,
                    span,
                } => {
                    assert_eq!(active_loops.pop(), Some(loop_id));
                    assert!(loop_targets[loop_id.0].take().is_some());
                    builder.enter(Some(exit), span)?;
                    continue;
                }
            };
            if let hir::StmtKind::While {
                loop_id,
                condition,
                body,
            } = statement.kind
            {
                let header = builder.reserve(statement.span);
                let body_entry = builder.reserve(function.blocks[body.0].span);
                let exit = builder.reserve(statement.span);
                assert_eq!(
                    loop_id.0, body.0,
                    "resolved loop ID is its unique body block"
                );
                assert!(loop_targets[loop_id.0]
                    .replace(LoopTargets { header, exit })
                    .is_none());
                active_loops.push(loop_id);
                builder.close(Terminator {
                    span: statement.span,
                    kind: TerminatorKind::Goto { target: header },
                })?;
                builder.enter(Some(header), statement.span)?;
                lower_expression(
                    function,
                    &local_map,
                    &expression_map,
                    condition,
                    &mut builder,
                    &mut next_expr,
                )?;
                builder.close(Terminator {
                    span: statement.span,
                    kind: TerminatorKind::Branch {
                        condition: operand(condition),
                        then_block: body_entry,
                        else_block: exit,
                    },
                })?;
                frames.push(Frame::FinishLoop {
                    loop_id,
                    exit,
                    span: statement.span,
                });
                frames.push(Frame::EnterArm {
                    id: body,
                    entry: body_entry,
                    join: Some(header),
                });
                continue;
            }
            let root = match statement.kind {
                hir::StmtKind::Let { init, .. }
                | hir::StmtKind::Assign { value: init, .. }
                | hir::StmtKind::Expr(init) => Some(init),
                hir::StmtKind::Return(value) => value,
                hir::StmtKind::Break { .. } | hir::StmtKind::Continue { .. } => None,
                hir::StmtKind::If { condition, .. } => Some(condition),
                hir::StmtKind::While { .. } => unreachable!("while lowered above"),
            };
            if let Some(root) = root {
                lower_expression(
                    function,
                    &local_map,
                    &expression_map,
                    root,
                    &mut builder,
                    &mut next_expr,
                )?;
            }
            match statement.kind {
                hir::StmtKind::Let { local, init } => match local_map[local.0] {
                    BindingLocation::Value(destination) => builder.assign(Assign {
                        destination,
                        value: Rvalue::Copy(operand(init)),
                        span: statement.span,
                    })?,
                    BindingLocation::Place(id) => builder.statement(Statement::Initialize {
                        place: Place {
                            id,
                            span: function.locals[local.0].span,
                        },
                        value: operand(init),
                        span: statement.span,
                    })?,
                },
                hir::StmtKind::Assign {
                    local,
                    target_span,
                    operator_span,
                    value,
                } => {
                    let BindingLocation::Place(id) = local_map[local.0] else {
                        return Err(failure(FailureKind::InvalidPlace, target_span));
                    };
                    builder.statement(Statement::Store {
                        place: Place {
                            id,
                            span: target_span,
                        },
                        value: operand(value),
                        operator_span,
                        span: statement.span,
                    })?;
                }
                hir::StmtKind::While { .. } => unreachable!("while lowered above"),
                hir::StmtKind::Break { target } | hir::StmtKind::Continue { target } => {
                    assert_eq!(
                        active_loops.last(),
                        Some(&target),
                        "resolved transfer targets the nearest active loop"
                    );
                    let targets = loop_targets[target.0].expect("resolved loop target is active");
                    let target = if matches!(statement.kind, hir::StmtKind::Break { .. }) {
                        targets.exit
                    } else {
                        targets.header
                    };
                    builder.close(Terminator {
                        kind: TerminatorKind::Goto { target },
                        span: statement.span,
                    })?;
                }
                hir::StmtKind::Expr(_) => {}
                hir::StmtKind::Return(value) => {
                    let value = match value {
                        Some(id) => operand(id),
                        None => {
                            let local = LocalId(locals.len());
                            locals.push(LocalDecl {
                                ty: hir::Ty::Unit,
                                kind: LocalKind::Temporary,
                                span: statement.span,
                            });
                            builder.assign(Assign {
                                destination: local,
                                value: Rvalue::Unit,
                                span: statement.span,
                            })?;
                            Operand {
                                local,
                                span: statement.span,
                            }
                        }
                    };
                    builder.close(Terminator {
                        kind: TerminatorKind::Return(value),
                        span: statement.span,
                    })?;
                }
                hir::StmtKind::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    let then_entry = builder.reserve(function.blocks[then_block.0].span);
                    let else_entry =
                        else_block.map(|id| builder.reserve(function.blocks[id.0].span));
                    let joins = view.block_flow(then_block).falls_through()
                        || else_block.is_none_or(|id| view.block_flow(id).falls_through());
                    let join = joins.then(|| builder.reserve(statement.span));
                    let false_entry = else_entry
                        .or(join)
                        .ok_or_else(|| failure(FailureKind::IncompleteBody, statement.span))?;
                    builder.close(Terminator {
                        kind: TerminatorKind::Branch {
                            condition: operand(condition),
                            then_block: then_entry,
                            else_block: false_entry,
                        },
                        span: statement.span,
                    })?;
                    frames.push(Frame::Join {
                        id: join,
                        span: statement.span,
                    });
                    if let (Some(id), Some(entry)) = (else_block, else_entry) {
                        frames.push(Frame::EnterArm { id, entry, join });
                    }
                    frames.push(Frame::EnterArm {
                        id: then_block,
                        entry: then_entry,
                        join,
                    });
                }
            }
        }
        assert!(active_loops.is_empty());
        assert!(loop_targets.iter().all(Option::is_none));
        if next_expr != function.expressions.len() {
            return Err(failure(FailureKind::IncompleteBody, signature.span));
        }
        functions.push(Function {
            places,
            id: function.id,
            span: signature.span,
            result: signature.result,
            param_count: signature.params.len(),
            locals,
            entry: BlockId(0),
            blocks: builder.finish(signature.span)?,
        });
    }
    Ok(Program { functions })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builder_rejects_double_close_and_append_after_close() {
        let span = Span {
            file: super::super::super::source::SourceFileId(0),
            start: 0,
            end: 0,
        };
        let mut builder = BlockBuilder::new(span);
        let terminator = Terminator {
            span,
            kind: TerminatorKind::Return(Operand {
                local: LocalId(0),
                span,
            }),
        };
        builder.close(terminator.clone()).unwrap();
        assert_eq!(
            builder.close(terminator).unwrap_err().kind,
            FailureKind::BuilderClosed
        );
        assert_eq!(
            builder
                .assign(Assign {
                    destination: LocalId(0),
                    value: Rvalue::Unit,
                    span
                })
                .unwrap_err()
                .kind,
            FailureKind::BuilderClosed
        );
        assert_eq!(
            BlockBuilder::new(span).finish(span).unwrap_err().kind,
            FailureKind::IncompleteBody
        );
    }
}

#[cfg(test)]
mod branch_builder_tests {
    use super::*;
    use crate::frontend::{hir, lexer, parser, source::SourceMap, typeck};

    #[test]
    fn structured_lowering_closes_all_and_only_real_paths_before_verification() {
        for (text, count) in [
            ("fn f(c: bool) -> () { if c { return; } else { return; } }", 3),
            ("fn f(c: bool) -> () { if c { if c { return; } else { return; } } else { return; } }", 5),
            ("fn f(c: bool) -> () { if c { if c {} else {} } else {} return; }", 7),
            ("fn f(c: bool) -> () { if c {} return; }", 3),
        ] {
            let mut sources = SourceMap::new();
            let id = sources.add("input.ox".into(), text.into());
            let source = sources.get(id);
            let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
            let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
            let raw = lower(&typed).unwrap();
            assert_eq!(raw.functions[0].blocks.len(), count, "{text}");
            assert!(raw.functions[0].blocks.iter().all(|b| b.terminator.is_some()));
        }
    }

    #[test]
    fn explicit_loop_transfers_close_paths_without_synthetic_edges() {
        for (text, count) in [
            ("fn f() -> () { while true { break; } return; }", 4),
            ("fn f() -> () { while true { continue; } return; }", 4),
            (
                "fn f(c: bool) -> () { while c { if c { break; } else { continue; } } return; }",
                6,
            ),
            (
                "fn f(c: bool) -> () { while c { if c { return; } else { break; } } return; }",
                6,
            ),
            (
                "fn f(c: bool) -> () { while c { if c { continue; } else { return; } } return; }",
                6,
            ),
            (
                "fn f(c: bool) -> () { while c { if c { break; } } return; }",
                6,
            ),
        ] {
            let mut sources = SourceMap::new();
            let id = sources.add("input.ox".into(), text.into());
            let source = sources.get(id);
            let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
            let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
            let raw = lower(&typed).unwrap();
            assert_eq!(raw.functions[0].blocks.len(), count, "{text}");
            assert!(raw.functions[0]
                .blocks
                .iter()
                .all(|b| b.terminator.is_some()));
        }
    }

    #[test]
    fn reserved_blocks_cannot_be_entered_over_an_open_or_closed_path() {
        let span = Span {
            file: crate::frontend::source::SourceFileId(0),
            start: 0,
            end: 0,
        };
        let mut builder = BlockBuilder::new(span);
        let next = builder.reserve(span);
        assert_eq!(
            builder.enter(Some(next), span).unwrap_err().kind,
            FailureKind::BuilderClosed
        );
        builder
            .close(Terminator {
                span,
                kind: TerminatorKind::Goto { target: next },
            })
            .unwrap();
        assert_eq!(
            builder.enter(Some(BlockId(0)), span).unwrap_err().kind,
            FailureKind::BuilderClosed
        );
        builder.enter(Some(next), span).unwrap();
        assert_eq!(
            builder.finish(span).unwrap_err().kind,
            FailureKind::IncompleteBody
        );
    }
}

#[cfg(test)]
mod loop_transfer_tests {
    use super::*;
    use crate::frontend::{lexer, parser};

    fn fixture(text: &str) -> (SourceMap, typeck::TypedProgram) {
        let mut sources = SourceMap::new();
        let id = sources.add("loop-targets.ox".into(), text.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
        (sources, typed)
    }

    fn goto_at(function: &Function, text: &str, statement: &str) -> BlockId {
        let start = text.find(statement).unwrap();
        let matching: Vec<_> = function
            .blocks
            .iter()
            .filter_map(|block| {
                let terminator = block.terminator.as_ref().unwrap();
                (terminator.span.start == start && terminator.span.end == start + statement.len())
                    .then_some(terminator)
            })
            .collect();
        assert_eq!(
            matching.len(),
            1,
            "exactly one full-span transfer: {statement}"
        );
        let TerminatorKind::Goto { target } = matching[0].kind else {
            panic!("transfer must lower directly to one Goto");
        };
        target
    }

    #[test]
    fn nested_transfers_target_the_nearest_loop_and_restore_later_siblings() {
        let text = "fn f(c: bool) -> () { while c { while c { if c { continue /* inner 雪 */; } else { break /* inner */; } } continue /* outer */; } while c { break /* sibling */; } return; }";
        let (sources, typed) = fixture(text);
        let raw = lower(&typed).unwrap();
        let function = &raw.functions[0];
        assert_eq!(function.blocks.len(), 12);
        assert_eq!(
            goto_at(function, text, "continue /* inner 雪 */;"),
            BlockId(4)
        );
        assert_eq!(goto_at(function, text, "break /* inner */;"), BlockId(6));
        assert_eq!(goto_at(function, text, "continue /* outer */;"), BlockId(1));
        assert_eq!(goto_at(function, text, "break /* sibling */;"), BlockId(11));
        // Every loop-body/arm path ends in its explicit transfer, so no closing
        // brace acquires a redundant charged edge.
        assert!(function.blocks.iter().all(|block| {
            let span = block.terminator.as_ref().unwrap().span;
            &text[span.start..span.end] != "}"
        }));
        verify::verify(raw, &sources).unwrap();
    }

    #[test]
    fn continue_reenters_the_full_condition_before_calls_and_lazy_merges() {
        let text = "fn f(c: bool) -> () { while pred(c) && !pred(false) { continue /* all condition */; } return; } fn pred(c: bool) -> bool { return c; }";
        let (sources, typed) = fixture(text);
        let raw = lower(&typed).unwrap();
        let function = &raw.functions[0];
        let header = goto_at(function, text, "continue /* all condition */;");
        assert_eq!(header, BlockId(1));
        assert!(matches!(
            function.blocks[header.0].terminator.as_ref().unwrap().kind,
            TerminatorKind::Call {
                target: hir::DefId(1),
                ..
            }
        ));
        assert!(function.blocks[header.0].merge.is_none());
        let merged_condition = function
            .blocks
            .iter()
            .position(|block| block.merge.is_some())
            .unwrap();
        assert_ne!(header.0, merged_condition);
        assert!(matches!(
            function.blocks[merged_condition]
                .terminator
                .as_ref()
                .unwrap()
                .kind,
            TerminatorKind::Branch {
                then_block: BlockId(2),
                else_block: BlockId(3),
                ..
            }
        ));
        verify::verify(raw, &sources).unwrap();
    }

    #[test]
    fn preflight_matches_exact_emission_for_all_terminal_and_fallthrough_combinations() {
        for left in ["", "return;", "break;", "continue;"] {
            for right in [
                None,
                Some(""),
                Some("return;"),
                Some("break;"),
                Some("continue;"),
            ] {
                let otherwise = right.map_or(String::new(), |body| format!("else {{ {body} }}"));
                let text = format!(
                    "fn f(c: bool) -> () {{ while c {{ if c {{ {left} }} {otherwise} }} return; }}"
                );
                let (sources, typed) = fixture(&text);
                let budget = preflight(&typed).unwrap();
                let raw = lower(&typed).unwrap();
                let expected = if right.is_none() || left.is_empty() || right == Some("") {
                    4 + 1 + usize::from(right.is_some()) + 1
                } else {
                    4 + 2
                };
                assert_eq!(budget.blocks, expected, "{text}");
                assert_eq!(raw.functions[0].blocks.len(), expected, "{text}");
                assert_eq!(
                    budget.locals,
                    raw.functions
                        .iter()
                        .map(Function::slot_count)
                        .sum::<usize>()
                );
                assert_eq!(
                    budget.assignments,
                    raw.functions
                        .iter()
                        .flat_map(|function| &function.blocks)
                        .map(|block| block.statements.len() + usize::from(block.merge.is_some()))
                        .sum::<usize>()
                );
                verify::verify(raw, &sources).unwrap();
            }
        }
        // Calls, laziness, mutable slots, implicit unit returns and nested loops
        // use the same exact aggregate accounting in combination with transfers.
        let text = "fn f(c: bool) -> () { let mut n = 0; while pred(c) && c { if c { n = n + 1; continue; } while c { break; } return; } return; } fn pred(c: bool) -> bool { return c; }";
        let (sources, typed) = fixture(text);
        let budget = preflight(&typed).unwrap();
        let raw = lower(&typed).unwrap();
        assert_eq!(
            budget.blocks,
            raw.functions
                .iter()
                .map(|function| function.blocks.len())
                .sum::<usize>()
        );
        assert_eq!(
            budget.locals,
            raw.functions
                .iter()
                .map(Function::slot_count)
                .sum::<usize>()
        );
        assert_eq!(
            budget.assignments,
            raw.functions
                .iter()
                .flat_map(|function| &function.blocks)
                .map(|block| block.statements.len() + usize::from(block.merge.is_some()))
                .sum::<usize>()
        );
        verify::verify(raw, &sources).unwrap();
    }
}
