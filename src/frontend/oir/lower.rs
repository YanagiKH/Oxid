use super::*;

const STAGE: &str = "oir-lower";
fn failure(kind: FailureKind, span: Span) -> OirFailure {
    OirFailure::new(kind, STAGE, Some(span))
}

/// Exact aggregate expansion, before allocating OIR storage/maps. One condition
/// creates a then entry, an optional else entry and only a reachable join.
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
                if let hir::StmtKind::If {
                    then_block,
                    else_block,
                    ..
                } = statement.kind
                {
                    let joins = !view.block_returns(then_block)
                        || else_block.is_none_or(|id| !view.block_returns(id));
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
    Ok(())
}

struct BlockBuilder {
    blocks: Vec<BasicBlock>,
    current: Option<BlockId>,
}
impl BlockBuilder {
    fn new(span: Span) -> Self {
        Self {
            blocks: vec![BasicBlock {
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
        self.blocks[id.0].statements.push(assign);
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
            };
            let root = match statement.kind {
                hir::StmtKind::Let { init, .. } | hir::StmtKind::Expr(init) => Some(init),
                hir::StmtKind::Return(value) => value,
                hir::StmtKind::If { condition, .. } => Some(condition),
            };
            if let Some(root) = root {
                while next_expr <= root.0 {
                    let expr = &function.expressions[next_expr];
                    let destination = expression_map[next_expr];
                    let value = match &expr.kind {
                        hir::ExprKind::Bool(value) => Some(Rvalue::Bool(*value)),
                        hir::ExprKind::I32(value) => Some(Rvalue::I32(*value)),
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
                hir::StmtKind::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    let then_entry = builder.reserve(function.blocks[then_block.0].span);
                    let else_entry =
                        else_block.map(|id| builder.reserve(function.blocks[id.0].span));
                    let joins = !view.block_returns(then_block)
                        || else_block.is_none_or(|id| !view.block_returns(id));
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
