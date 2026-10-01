use super::*;

const STAGE: &str = "oir-lower";
fn failure(kind: FailureKind, span: Span) -> OirFailure {
    OirFailure::new(kind, STAGE, Some(span))
}

/// Preflight the exact expansion before allocating any OIR storage or maps.
/// Each local/assignment corresponds to a distinct counted parser parameter,
/// expression, let statement or bare return. Blocks correspond to functions/calls.
fn preflight(typed: &typeck::TypedProgram) -> Result<(), OirFailure> {
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
        for statement in &function.body {
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
                hir::StmtKind::Let { .. } | hir::StmtKind::Return(None)
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
        }
    }
    Ok(())
}

struct BlockBuilder {
    blocks: Vec<BasicBlock>,
    current: usize,
}
impl BlockBuilder {
    fn new(span: Span) -> Self {
        Self {
            blocks: vec![BasicBlock {
                span,
                statements: vec![],
                terminator: None,
            }],
            current: 0,
        }
    }
    fn assign(&mut self, assign: Assign) -> Result<(), OirFailure> {
        let block = &mut self.blocks[self.current];
        if block.terminator.is_some() {
            return Err(failure(FailureKind::BuilderClosed, assign.span));
        }
        block.statements.push(assign);
        Ok(())
    }
    fn close(&mut self, terminator: Terminator) -> Result<(), OirFailure> {
        let block = &mut self.blocks[self.current];
        if block.terminator.is_some() {
            return Err(failure(FailureKind::BuilderClosed, terminator.span));
        }
        block.terminator = Some(terminator);
        Ok(())
    }
    fn call(
        &mut self,
        target: hir::DefId,
        args: Vec<Operand>,
        destination: LocalId,
        span: Span,
    ) -> Result<(), OirFailure> {
        let continuation = BlockId(self.blocks.len());
        self.close(Terminator {
            span,
            kind: TerminatorKind::Call {
                target,
                args,
                destination,
                continuation,
            },
        })?;
        self.blocks.push(BasicBlock {
            span,
            statements: vec![],
            terminator: None,
        });
        self.current = continuation.0;
        Ok(())
    }
    fn finish(self, span: Span) -> Result<Vec<BasicBlock>, OirFailure> {
        if self.blocks.iter().any(|b| b.terminator.is_none()) {
            return Err(failure(FailureKind::IncompleteBody, span));
        }
        Ok(self.blocks)
    }
}

pub(super) fn lower(typed: &typeck::TypedProgram) -> Result<Program, OirFailure> {
    preflight(typed)?;
    let mut functions = Vec::with_capacity(typed.functions().len());
    for view in typed.functions() {
        let function = view.hir();
        let signature = view.signature();
        let mut locals = Vec::new();
        let mut local_map = Vec::with_capacity(function.locals.len());
        for (index, local) in function.locals.iter().enumerate() {
            local_map.push(LocalId(locals.len()));
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
        let mut next_expr = 0;
        for statement in &function.body {
            let root = match statement.kind {
                hir::StmtKind::Let { init, .. } | hir::StmtKind::Expr(init) => Some(init),
                hir::StmtKind::Return(value) => value,
            };
            if let Some(root) = root {
                while next_expr <= root.0 {
                    let expr = &function.expressions[next_expr];
                    let destination = expression_map[next_expr];
                    let value = match &expr.kind {
                        hir::ExprKind::Bool(value) => Some(Rvalue::Bool(*value)),
                        hir::ExprKind::Unit => Some(Rvalue::Unit),
                        hir::ExprKind::Local(id) => Some(Rvalue::Copy(Operand {
                            local: local_map[id.0],
                            span: expr.span,
                        })),
                        hir::ExprKind::Group(inner) => Some(Rvalue::Copy(operand(*inner))),
                        hir::ExprKind::Call { target, args } => {
                            builder.call(
                                *target,
                                args.iter().map(|id| operand(*id)).collect(),
                                destination,
                                expr.span,
                            )?;
                            None
                        }
                    };
                    if let Some(value) = value {
                        builder.assign(Assign {
                            destination,
                            value,
                            span: expr.span,
                        })?;
                    }
                    next_expr += 1;
                }
            }
            match statement.kind {
                hir::StmtKind::Let { local, init } => builder.assign(Assign {
                    destination: local_map[local.0],
                    value: Rvalue::Copy(operand(init)),
                    span: statement.span,
                })?,
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
            }
        }
        if next_expr != function.expressions.len() {
            return Err(failure(FailureKind::IncompleteBody, signature.span));
        }
        functions.push(Function {
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
