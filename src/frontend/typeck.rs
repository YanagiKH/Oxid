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
}
impl TypedProgram {
    pub fn function_count(&self) -> usize {
        debug_assert_eq!(self.program.functions.len(), self.bodies.len());
        for (function, body) in self.program.functions.iter().zip(&self.bodies) {
            debug_assert_eq!(function.expressions.len(), body.expressions.len());
            debug_assert_eq!(function.locals.len(), body.locals.len());
        }
        self.program.functions.len()
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
    for (slot, ty) in locals.iter_mut().zip(&signature.params) {
        *slot = Some(*ty);
    }
    let mut expressions = Vec::with_capacity(function.expressions.len());
    let mut next_expr = 0;
    let mut returned = false;
    for statement in &function.body {
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
        }
    }
    if !returned {
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
    Ok(TypedBody {
        expressions,
        locals,
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
    fn resolved_ids_and_complete_type_tables_survive_the_pipeline() {
        let text = "fn main() -> () { let answer = identity(true); identity(answer); return (); } fn identity(value: bool) -> bool { let result = (value); return result; }";
        let typed = checked(text);
        assert_eq!(typed.function_count(), 2);
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
