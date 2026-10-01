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
    block_returns: Vec<bool>,
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
                assert_eq!(function.blocks.len(), body.block_returns.len());
                assert!(body.block_returns[function.body.0]);
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
    pub(super) fn block_returns(&self, id: BodyBlockId) -> bool {
        self.body.block_returns[id.0]
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
            returned: bool,
        },
        Join {
            block: BodyBlockId,
            index: usize,
            then_block: BodyBlockId,
            else_block: Option<BodyBlockId>,
        },
    }
    let mut block_returns = vec![None; function.blocks.len()];
    let mut frames = vec![Frame::Block {
        block: function.body,
        index: 0,
        returned: false,
    }];
    while let Some(frame) = frames.pop() {
        let (block, index, mut returned) = match frame {
            Frame::Block {
                block,
                index,
                returned,
            } => (block, index, returned),
            Frame::Join {
                block,
                index,
                then_block,
                else_block,
            } => {
                let then_returns = block_returns[then_block.0].expect("then arm was checked");
                let else_returns =
                    else_block.is_some_and(|id| block_returns[id.0].expect("else arm was checked"));
                (block, index, then_returns && else_returns)
            }
        };
        let Some(statement) = function.blocks[block.0].body.get(index) else {
            block_returns[block.0] = Some(returned);
            continue;
        };
        if returned {
            return Err(Diagnostic::new(
                "E0303",
                "type",
                "statement after terminal return is unavailable in typed-preview",
                Some(statement.span),
            ));
        }
        let root = match statement.kind {
            StmtKind::Let { init, .. } | StmtKind::Expr(init) => Some(init),
            StmtKind::Return(value) => value,
            StmtKind::If { condition, .. } => Some(condition),
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
                returned = true;
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
                frames.push(Frame::Join {
                    block,
                    index: index + 1,
                    then_block,
                    else_block,
                });
                if let Some(otherwise) = else_block {
                    frames.push(Frame::Block {
                        block: otherwise,
                        index: 0,
                        returned: false,
                    });
                }
                frames.push(Frame::Block {
                    block: then_block,
                    index: 0,
                    returned: false,
                });
                continue;
            }
        }
        frames.push(Frame::Block {
            block,
            index: index + 1,
            returned,
        });
    }
    if !block_returns[function.body.0].expect("function body was checked") {
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
    let block_returns = block_returns
        .into_iter()
        .map(|flow| flow.expect("all resolved blocks have checked flow"))
        .collect();
    Ok(TypedBody {
        expressions,
        locals,
        block_returns,
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
                    typed.bodies[0].block_returns.pop();
                }
                7 => {
                    typed.program.functions[0].body = BodyBlockId(99);
                }
                _ => {
                    typed.bodies[0].block_returns[0] = false;
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
