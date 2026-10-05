//! A successful typed program has no holes: construction is private to this pass.
use super::{diagnostic::Diagnostic, hir::*, parser::MAX_DIAGNOSTICS};
#[derive(Debug)]
pub struct TypedProgram {
    program: Program,
    bodies: Vec<TypedBody>,
}
#[derive(Debug)]
struct TypedBody {
    expressions: Vec<Ty>,
    locals: Vec<Ty>,
    block_flows: Vec<FlowSummary>,
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
/// Immutable frontend-only view; successful construction remains in this pass.
pub(super) struct TypedFunction<'a> {
    function: &'a Function,
    signature: &'a Signature,
    body: &'a TypedBody,
}
impl TypedProgram {
    pub(super) fn functions(&self) -> impl ExactSizeIterator<Item = TypedFunction<'_>> {
        // These are producer invariants, checked in release too. Raw OIR verification
        // below the lowering boundary never relies on these assertions.
        assert_eq!(self.program.functions.len(), self.program.signatures.len());
        assert_eq!(self.program.functions.len(), self.bodies.len());
        self.program
            .functions
            .iter()
            .enumerate()
            .map(|(index, function)| {
                let body = &self.bodies[index];
                let signature = &self.program.signatures[index];
                assert_eq!(function.id, DefId(index));
                assert_eq!(function.expressions.len(), body.expressions.len());
                assert_eq!(function.locals.len(), body.locals.len());
                assert_eq!(function.blocks.len(), body.block_flows.len());
                assert!(body.block_flows[function.body.0].returns_only());
                assert!(signature.params.len() <= body.locals.len());
                assert_eq!(&body.locals[..signature.params.len()], &signature.params);
                TypedFunction {
                    function,
                    signature,
                    body,
                }
            })
    }
}
impl<'a> TypedFunction<'a> {
    pub(super) fn hir(&self) -> &'a Function {
        self.function
    }
    pub(super) fn signature(&self) -> &'a Signature {
        self.signature
    }
    pub(super) fn expression_ty(&self, id: ExprId) -> Ty {
        self.body.expressions[id.0]
    }
    pub(super) fn local_ty(&self, id: LocalId) -> Ty {
        self.body.locals[id.0]
    }
    pub(super) fn block_flow(&self, id: BodyBlockId) -> FlowSummary {
        self.body.block_flows[id.0]
    }
}

fn mismatch(expected: Ty, actual: Ty, span: super::source::Span) -> Box<Diagnostic> {
    Diagnostic::new(
        "E0300",
        "type",
        format!("type mismatch: expected {expected}, found {actual}"),
        Some(span),
    )
}
pub fn check(program: Program) -> Result<TypedProgram, Vec<Diagnostic>> {
    let mut bodies = Vec::new();
    let mut diagnostics = Vec::new();
    for function in &program.functions {
        match check_body(&program, function) {
            Ok(body) => bodies.push(body),
            Err(error) => diagnostics.push(*error),
        }
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
    }
    if diagnostics.is_empty() {
        Ok(TypedProgram { program, bodies })
    } else {
        Err(diagnostics)
    }
}
fn check_body(program: &Program, function: &Function) -> Result<TypedBody, Box<Diagnostic>> {
    let signature = &program.signatures[function.id.0];
    let mut locals = vec![None; function.locals.len()];
    for (index, ty) in signature.params.iter().enumerate() {
        locals[index] = Some(*ty);
    }
    let mut expressions = Vec::with_capacity(function.expressions.len());
    let mut next_expr = 0;
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
            return Err(Diagnostic::new(
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
            | StmtKind::Expr(init) => Some(init),
            StmtKind::Return(value) => value,
            StmtKind::Break { .. } | StmtKind::Continue { .. } => None,
            StmtKind::If { condition, .. } | StmtKind::While { condition, .. } => Some(condition),
        };
        // Resolver emits children before parents and statement roots in source order.
        if let Some(root) = root {
            while next_expr <= root.0 {
                let expr = &function.expressions[next_expr];
                let ty = match &expr.kind {
                    ExprKind::Bool(value) => {
                        let _literal_value = *value;
                        Ty::Bool
                    }
                    ExprKind::Unit => Ty::Unit,
                    ExprKind::I32(_) => Ty::I32,
                    ExprKind::Local(id) => {
                        locals[id.0].expect("resolved locals are initialized before use")
                    }
                    ExprKind::Group(inner) => expressions[inner.0],
                    ExprKind::Negate { operand, .. } => {
                        let actual = expressions[operand.0];
                        if actual != Ty::I32 {
                            return Err(mismatch(
                                Ty::I32,
                                actual,
                                function.expressions[operand.0].span,
                            ));
                        }
                        Ty::I32
                    }
                    ExprKind::Not { operand, .. } => {
                        let actual = expressions[operand.0];
                        if actual != Ty::Bool {
                            return Err(mismatch(
                                Ty::Bool,
                                actual,
                                function.expressions[operand.0].span,
                            ));
                        }
                        Ty::Bool
                    }
                    ExprKind::Logical { left, right, .. } => {
                        for operand in [left, right] {
                            let actual = expressions[operand.0];
                            if actual != Ty::Bool {
                                return Err(mismatch(
                                    Ty::Bool,
                                    actual,
                                    function.expressions[operand.0].span,
                                ));
                            }
                        }
                        Ty::Bool
                    }
                    ExprKind::Comparison {
                        op, left, right, ..
                    } => {
                        let left_ty = expressions[left.0];
                        let expected = match op {
                            ComparisonOp::Equal | ComparisonOp::NotEqual => {
                                if !matches!(left_ty, Ty::I32 | Ty::Bool) {
                                    return Err(Diagnostic::new(
                                        "E0300", "type",
                                        format!("equality requires i32 or bool operands, found {left_ty}"),
                                        Some(function.expressions[left.0].span),
                                    ));
                                }
                                left_ty
                            }
                            ComparisonOp::Less
                            | ComparisonOp::LessEqual
                            | ComparisonOp::Greater
                            | ComparisonOp::GreaterEqual => {
                                if left_ty != Ty::I32 {
                                    return Err(mismatch(
                                        Ty::I32,
                                        left_ty,
                                        function.expressions[left.0].span,
                                    ));
                                }
                                Ty::I32
                            }
                        };
                        let right_ty = expressions[right.0];
                        if right_ty != expected {
                            return Err(mismatch(
                                expected,
                                right_ty,
                                function.expressions[right.0].span,
                            ));
                        }
                        Ty::Bool
                    }
                    ExprKind::Arithmetic { left, right, .. } => {
                        for operand in [left, right] {
                            let actual = expressions[operand.0];
                            if actual != Ty::I32 {
                                return Err(mismatch(
                                    Ty::I32,
                                    actual,
                                    function.expressions[operand.0].span,
                                ));
                            }
                        }
                        Ty::I32
                    }
                    ExprKind::Call { target, args } => {
                        let called = &program.signatures[target.0];
                        if args.len() != called.params.len() {
                            return Err(Diagnostic::new(
                                "E0301",
                                "type",
                                format!(
                                    "wrong argument count: expected {}, found {}",
                                    called.params.len(),
                                    args.len()
                                ),
                                Some(expr.span),
                            )
                            .secondary(called.span, "function declared here"));
                        }
                        for (arg, expected) in args.iter().zip(&called.params) {
                            let actual = expressions[arg.0];
                            if actual != *expected {
                                return Err(mismatch(
                                    *expected,
                                    actual,
                                    function.expressions[arg.0].span,
                                )
                                .secondary(called.span, "function declared here"));
                            }
                        }
                        called.result
                    }
                };
                expressions.push(ty);
                next_expr += 1;
            }
        }
        match statement.kind {
            StmtKind::Let { local, init } => {
                let actual = expressions[init.0];
                if let Some(expected) = function.locals[local.0].annotation {
                    if expected != actual {
                        return Err(
                            mismatch(expected, actual, function.expressions[init.0].span)
                                .secondary(function.locals[local.0].span, "binding declared here"),
                        );
                    }
                }
                locals[local.0] = Some(actual);
            }
            StmtKind::Assign {
                local,
                target_span,
                value,
                ..
            } => {
                let declaration = &function.locals[local.0];
                if !declaration.mutable {
                    return Err(Diagnostic::new(
                        "E0304",
                        "type",
                        "assignment requires a mutable local",
                        Some(target_span),
                    )
                    .secondary(declaration.span, "immutable binding declared here"));
                }
                let expected =
                    locals[local.0].expect("resolved assignment target has an initializer");
                let actual = expressions[value.0];
                if actual != expected {
                    return Err(
                        mismatch(expected, actual, function.expressions[value.0].span)
                            .secondary(declaration.span, "binding declared here"),
                    );
                }
            }
            StmtKind::Expr(_) => {}
            StmtKind::Return(value) => {
                let actual = value.map_or(Ty::Unit, |id| expressions[id.0]);
                if signature.result != actual {
                    return Err(mismatch(
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
                let actual = expressions[condition.0];
                if actual != Ty::Bool {
                    return Err(mismatch(
                        Ty::Bool,
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
                let actual = expressions[condition.0];
                if actual != Ty::Bool {
                    return Err(mismatch(
                        Ty::Bool,
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
        return Err(Diagnostic::new(
            "E0302",
            "type",
            "function requires an explicit terminal return",
            Some(function.end),
        ));
    }
    let locals = locals
        .into_iter()
        .map(|ty| ty.expect("all resolved locals have typed initializers"))
        .collect();
    assert_eq!(next_expr, function.expressions.len());
    let block_flows = block_flows
        .into_iter()
        .map(|flow| flow.expect("all resolved blocks have checked flow"))
        .collect();
    Ok(TypedBody {
        expressions,
        locals,
        block_flows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{hir, lexer, parser, source::SourceMap};
    fn checked(text: &str) -> TypedProgram {
        let mut sources = SourceMap::new();
        let id = sources.add("input.ox".into(), text.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        check(hir::resolve(source, &ast).unwrap()).unwrap()
    }
    #[test]
    fn immutable_views_expose_exact_function_signature_and_type_alignment() {
        let typed = checked("fn f(x: bool, u: ()) -> bool { let y = (x); u; return y; }");
        let views: Vec<_> = typed.functions().collect();
        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert_eq!(view.signature().params, [Ty::Bool, Ty::Unit]);
        assert_eq!(view.signature().result, Ty::Bool);
        assert_eq!(view.hir().id, DefId(0));
        assert_eq!(
            (0..3)
                .map(|id| view.local_ty(LocalId(id)))
                .collect::<Vec<_>>(),
            [Ty::Bool, Ty::Unit, Ty::Bool]
        );
        assert_eq!(
            (0..4)
                .map(|id| view.expression_ty(ExprId(id)))
                .collect::<Vec<_>>(),
            [Ty::Bool, Ty::Bool, Ty::Unit, Ty::Bool]
        );
    }
    #[test]
    fn view_boundary_never_silently_truncates_internal_tables() {
        for mutation in 0..9 {
            let mut typed = checked("fn f(x: bool) -> bool { return x; }");
            match mutation {
                0 => {
                    typed.bodies.pop();
                }
                1 => {
                    typed.program.signatures.pop();
                }
                2 => {
                    typed.bodies[0].expressions.pop();
                }
                3 => {
                    typed.bodies[0].locals.pop();
                }
                4 => {
                    typed.program.functions[0].id = DefId(9);
                }
                5 => {
                    typed.bodies[0].locals[0] = Ty::Unit;
                }
                6 => {
                    typed.bodies[0].block_flows.pop();
                }
                7 => {
                    typed.program.functions[0].body = BodyBlockId(99);
                }
                _ => {
                    typed.bodies[0].block_flows[0] = FlowSummary::FALLTHROUGH;
                }
            }
            assert!(std::panic::catch_unwind(|| typed.functions().for_each(|_| {})).is_err());
        }
    }
    #[test]
    fn resolved_ids_and_complete_type_tables_survive_the_pipeline() {
        let text = "fn main() -> () { let answer = identity(true); identity(answer); return (); } fn identity(value: bool) -> bool { let result = (value); return result; }";
        let typed = checked(text);
        assert_eq!(typed.functions().len(), 2);
        assert_eq!(
            typed.bodies[0].expressions,
            [Ty::Bool, Ty::Bool, Ty::Bool, Ty::Bool, Ty::Unit]
        );
        assert_eq!(typed.bodies[0].locals, [Ty::Bool]);
        assert_eq!(typed.bodies[1].expressions, [Ty::Bool, Ty::Bool, Ty::Bool]);
        assert_eq!(typed.bodies[1].locals, [Ty::Bool, Ty::Bool]);
        assert_eq!(typed.program.functions[0].id, DefId(0));
        assert_eq!(typed.program.functions[1].id, DefId(1));
        assert!(matches!(
            typed.program.functions[0].expressions[1].kind,
            ExprKind::Call {
                target: DefId(1),
                ..
            }
        ));
        assert!(matches!(
            typed.program.functions[1].expressions[0].kind,
            ExprKind::Local(LocalId(0))
        ));
        assert!(matches!(
            typed.program.functions[1].expressions[2].kind,
            ExprKind::Local(LocalId(1))
        ));
        let call = &typed.program.functions[0].expressions[1];
        assert_eq!(&text[call.span.start..call.span.end], "identity(true)");
        assert_eq!(format!("{typed:?}"), format!("{:?}", checked(text)));
    }
}

#[cfg(test)]
#[path = "branch_tests.rs"]
mod branch_tests;

#[cfg(test)]
mod loop_flow_tests {
    use super::*;
    use crate::frontend::{lexer, parser, source::SourceMap};

    fn resolved(text: &str) -> Program {
        let mut sources = SourceMap::new();
        let id = sources.add("loop-flow.ox".into(), text.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        super::super::hir::resolve(source, &ast).unwrap()
    }

    #[test]
    fn outcome_union_and_sequence_cover_the_complete_truth_table() {
        fn outcomes(bits: u8) -> FlowSummary {
            FlowSummary::new(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0)
        }
        for left in 0u8..16 {
            let before = outcomes(left);
            assert_eq!(before.falls_through(), left & 1 != 0);
            assert_eq!(before.returns_only(), left == 2);
            assert_eq!(before.after_while(), outcomes(1 | (left & 2)));
            for right in 0u8..16 {
                let next = outcomes(right);
                assert_eq!(before.union(next), outcomes(left | right));
                // Enumerate individual paths: terminal paths keep their exit;
                // each fallthrough path can take every exit of the next list.
                let mut expected = 0;
                for exit in [1, 2, 4, 8] {
                    if left & exit != 0 {
                        expected |= if exit == 1 { right } else { exit };
                    }
                }
                assert_eq!(
                    before.then(next),
                    outcomes(expected),
                    "{left:04b}; {right:04b}"
                );
            }
        }
    }

    #[test]
    fn partial_branches_keep_prior_exits_and_nested_loops_consume_only_their_exits() {
        let returning = FlowSummary::RETURN;
        let breaking = FlowSummary::BREAK;
        let continuing = FlowSummary::CONTINUE;
        let falling = FlowSummary::FALLTHROUGH;
        for (body, expected) in [
            (
                "if a { return; } if b { break; } continue;",
                returning.union(breaking).union(continuing),
            ),
            (
                "if a { return; } while b { break; } continue;",
                returning.union(continuing),
            ),
            (
                "if a { break; } while b { return; } continue;",
                returning.union(breaking).union(continuing),
            ),
            ("if a { break; }", falling.union(breaking)),
            ("if a { continue; }", falling.union(continuing)),
            (
                "while a { if b { return; } else { break; } } continue;",
                returning.union(continuing),
            ),
            ("while a { continue; } break;", breaking),
            ("if a { return; } if b {} return;", returning),
        ] {
            let text = format!("fn f(a: bool, b: bool) -> () {{ while a {{ {body} }} return; }}");
            let typed = check(resolved(&text)).unwrap();
            let view = typed.functions().next().unwrap();
            assert_eq!(view.block_flow(BodyBlockId(1)), expected, "{body}");
            assert_eq!(view.block_flow(view.hir().body), returning, "{body}");
        }
    }

    #[test]
    fn all_terminal_arm_pairs_reject_following_statements_and_keep_return_only_text() {
        for left in ["return;", "break;", "continue;"] {
            for right in ["return;", "break;", "continue;"] {
                let text = format!("fn f(c: bool) -> () {{ while c {{ if c {{ {left} }} else {{ {right} }} true; }} return; }}");
                let errors = check(resolved(&text)).unwrap_err();
                assert_eq!(errors.len(), 1);
                let error = &errors[0];
                assert_eq!(error.code, "E0303");
                let span = error.primary.unwrap();
                assert_eq!(&text[span.start..span.end], "true;");
                let expected = if left == "return;" && right == "return;" {
                    "statement after terminal return is unavailable in typed-preview"
                } else {
                    "statement after terminal control transfer is unavailable in typed-preview"
                };
                assert_eq!(error.message, expected);
            }
        }
        for body in [
            "break; true;",
            "continue; true;",
            "if c { return; } break; true;",
            "if c { break; } continue; true;",
            "if c { continue; } return; true;",
        ] {
            let text = format!("fn f(c: bool) -> () {{ while c {{ {body} }} return; }}");
            let error = check(resolved(&text)).unwrap_err().remove(0);
            assert_eq!(error.code, "E0303");
            assert_eq!(
                error.message,
                "statement after terminal control transfer is unavailable in typed-preview"
            );
            let span = error.primary.unwrap();
            assert_eq!(&text[span.start..span.end], "true;");
        }
    }

    #[test]
    fn while_keeps_false_edge_and_checks_literal_false_branches() {
        for body in [
            "return;",
            "break;",
            "continue;",
            "if true { break; } else { continue; }",
        ] {
            let text = format!("fn f() -> () {{ while true {{ {body} }} }}");
            assert_eq!(check(resolved(&text)).unwrap_err()[0].code, "E0302");
        }
        for text in [
            "fn f() -> () { while false { if false { break; } else { let x: bool = 1; } } return; }",
            "fn f() -> () { while false { if true { continue; } else { return 1; } } return; }",
        ] {
            assert_eq!(check(resolved(text)).unwrap_err()[0].code, "E0300");
        }
    }

    #[test]
    fn typechecker_rejects_broken_resolved_loop_target_invariants() {
        for replacement in [LoopId(1), LoopId(0), LoopId(99)] {
            let mut program =
                resolved("fn f() -> () { while true { while true { break; } } return; }");
            let StmtKind::Break { target } = &mut program.functions[0].blocks[2].body[0].kind
            else {
                panic!("fixture has the innermost break");
            };
            *target = replacement;
            assert!(std::panic::catch_unwind(|| check(program)).is_err());
        }
    }
}
