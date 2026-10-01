//! Admission and LLVM lowering consume only the verifier's immutable witness.
//! Admission is intentionally stricter than checking/reference execution.
use super::*;
use std::fmt::Write;

const MAX_FUNCTIONS: usize = 256;
const MAX_PARAMS: usize = 64;
const MAX_FUNCTION_LOCALS: usize = 256;
const MAX_NATIVE_LOCALS: usize = 8_192;
const MAX_NATIVE_BLOCKS: usize = 4_096;
const MAX_DEPTH: usize = 32;
const MAX_COST: usize = 100_000;

fn reject(message: impl Into<String>, span: Option<Span>) -> Box<Diagnostic> {
    Diagnostic::new("E0700", "native-admission", message, span)
}
fn limit(value: usize, maximum: usize, name: &str, span: Span) -> Result<(), Box<Diagnostic>> {
    if value > maximum {
        Err(reject(
            format!("native preview {name} limit exceeded ({maximum})"),
            Some(span),
        ))
    } else {
        Ok(())
    }
}
#[derive(Clone, Copy, Default, Debug)]
struct Bound {
    cost: usize,
    depth: usize,
    slots: usize,
}

impl VerifiedProgram {
    pub(in crate::frontend) fn native_module(
        &self,
        entry: Option<hir::DefId>,
    ) -> Result<String, Box<Diagnostic>> {
        let id = entry.ok_or_else(|| {
            reject(
                "native compile requires a declared zero-argument main",
                None,
            )
        })?;
        let root = self
            .program
            .functions
            .get(id.0)
            .filter(|f| f.id == id)
            .ok_or_else(|| {
                Diagnostic::new(
                    "E0500",
                    "native-admission",
                    "internal compiler error: invalid native entry identity",
                    None,
                )
            })?;
        if root.param_count != 0 {
            return Err(reject(
                "native main must have no parameters",
                Some(root.span),
            ));
        }
        self.admit()?;
        Ok(emit(&self.program, id))
    }

    fn admit(&self) -> Result<Vec<Bound>, Box<Diagnostic>> {
        let functions = &self.program.functions;
        if functions.is_empty() {
            return Ok(Vec::new());
        }
        let origin = functions[0].span;
        limit(functions.len(), MAX_FUNCTIONS, "function count", origin)?;
        let mut callers = vec![Vec::new(); functions.len()];
        let mut remaining = vec![0usize; functions.len()];
        let (mut locals, mut blocks) = (0, 0);
        for f in functions {
            limit(f.param_count, MAX_PARAMS, "parameter count", f.span)?;
            limit(
                f.locals.len(),
                MAX_FUNCTION_LOCALS,
                "locals per function",
                f.span,
            )?;
            locals += f.locals.len();
            blocks += f.blocks.len();
            for b in &f.blocks {
                // Closed native allowlist: future reference operations stay unsupported.
                for a in &b.statements {
                    #[allow(unreachable_patterns)]
                    match a.value {
                        Rvalue::Bool(_) | Rvalue::I32(_) | Rvalue::Unit | Rvalue::Copy(_) => {}
                        _ => return Err(reject("native preview does not support this OIR operation (including checked arithmetic)", Some(a.span))),
                    }
                }
                if let TerminatorKind::Call { target, .. } =
                    &b.terminator.as_ref().expect("verified terminator").kind
                {
                    remaining[f.id.0] += 1;
                    callers[target.0].push(f.id.0);
                }
            }
        }
        limit(locals, MAX_NATIVE_LOCALS, "aggregate locals", origin)?;
        limit(blocks, MAX_NATIVE_BLOCKS, "aggregate blocks", origin)?;
        let mut ready: Vec<_> = remaining
            .iter()
            .enumerate()
            .filter_map(|(i, &n)| (n == 0).then_some(i))
            .collect();
        let mut bounds = vec![Bound::default(); functions.len()];
        let mut visited = 0;
        while let Some(i) = ready.pop() {
            visited += 1;
            let f = &functions[i];
            let mut bound = Bound {
                cost: f.locals.len(),
                depth: 1,
                slots: f.locals.len(),
            };
            // Summing every block (including both branch arms) overestimates any
            // path through the verified acyclic CFG. Repeated call sites count
            // separately; sharing a callee does not hide exponential execution.
            for b in &f.blocks {
                bound.cost = bound.cost.saturating_add(b.statements.len() + 1);
                if let TerminatorKind::Call { target, args, .. } =
                    &b.terminator.as_ref().expect("verified terminator").kind
                {
                    let child = bounds[target.0];
                    bound.cost = bound
                        .cost
                        .saturating_add(args.len())
                        .saturating_add(child.cost);
                    bound.depth = bound.depth.max(1 + child.depth);
                    bound.slots = bound.slots.max(f.locals.len() + child.slots);
                }
            }
            // Root allocation's extra fuel unit is included even for non-main
            // functions. No executable path admitted here can hit runner limits.
            limit(
                bound.cost.saturating_add(1),
                MAX_COST,
                "reference fuel upper bound",
                f.span,
            )?;
            limit(bound.depth, MAX_DEPTH, "call depth", f.span)?;
            limit(bound.slots, MAX_NATIVE_LOCALS, "live local slots", f.span)?;
            bounds[i] = bound;
            for &caller in &callers[i] {
                remaining[caller] -= 1;
                if remaining[caller] == 0 {
                    ready.push(caller);
                }
            }
        }
        if visited != functions.len() {
            let f = &functions[remaining
                .iter()
                .position(|&n| n != 0)
                .expect("cycle member")];
            return Err(reject("native preview does not support recursive call graphs, including unused functions and unchosen branches", Some(f.span)));
        }
        Ok(bounds)
    }
}
fn ty(ty: hir::Ty) -> &'static str {
    match ty {
        hir::Ty::Bool => "i1",
        hir::Ty::Unit => "i8",
        hir::Ty::I32 => "i32",
    }
}
fn emit(program: &Program, entry: hir::DefId) -> String {
    let mut out = String::from("; Oxid experimental scalar native ABI 1\nsource_filename = \"oxid-native\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n\ndeclare i32 @__oxid_print_bool(i32)\ndeclare i32 @__oxid_print_i32(i32)\ndeclare i32 @__oxid_print_unit()\n");
    for f in &program.functions {
        // Source names and paths never enter LLVM symbol syntax. Deterministic
        // numeric DefIds avoid collisions with main, libc and runtime symbols.
        write!(
            out,
            "\ndefine internal {} @__oxid_fn_{}(",
            ty(f.result),
            f.id.0
        )
        .unwrap();
        for (i, p) in f.locals.iter().take(f.param_count).enumerate() {
            if i != 0 {
                out.push_str(", ");
            }
            write!(out, "{} %v{i}", ty(p.ty)).unwrap();
        }
        out.push_str(") noinline {\nentry:\n");
        writeln!(out, "  br label %b{}", f.entry.0).unwrap();
        for (i, b) in f.blocks.iter().enumerate() {
            writeln!(out, "b{i}:").unwrap();
            for a in &b.statements {
                let t = ty(f.locals[a.destination.0].ty);
                #[allow(unreachable_patterns)]
                let rhs = match a.value {
                    Rvalue::Bool(v) => {
                        if v {
                            "true".into()
                        } else {
                            "false".into()
                        }
                    }
                    Rvalue::I32(v) => v.to_string(),
                    Rvalue::Unit => "0".into(),
                    Rvalue::Copy(v) => format!("%v{}", v.local.0),
                    _ => unreachable!("native admission allowlist"),
                };
                writeln!(out, "  %v{} = or {t} {rhs}, 0", a.destination.0).unwrap();
            }
            match &b.terminator.as_ref().expect("verified terminator").kind {
                TerminatorKind::Return(v) => {
                    writeln!(out, "  ret {} %v{}", ty(f.result), v.local.0).unwrap();
                }
                TerminatorKind::Goto { target } => {
                    writeln!(out, "  br label %b{}", target.0).unwrap();
                }
                TerminatorKind::Branch {
                    condition,
                    then_block,
                    else_block,
                } => {
                    writeln!(
                        out,
                        "  br i1 %v{}, label %b{}, label %b{}",
                        condition.local.0, then_block.0, else_block.0
                    )
                    .unwrap();
                }
                TerminatorKind::Call {
                    target,
                    args,
                    destination,
                    continuation,
                } => {
                    write!(
                        out,
                        "  %v{} = call {} @__oxid_fn_{}(",
                        destination.0,
                        ty(f.locals[destination.0].ty),
                        target.0
                    )
                    .unwrap();
                    for (j, arg) in args.iter().enumerate() {
                        if j != 0 {
                            out.push_str(", ");
                        }
                        write!(out, "{} %v{}", ty(f.locals[arg.local.0].ty), arg.local.0).unwrap();
                    }
                    writeln!(out, ")\n  br label %b{}", continuation.0).unwrap();
                }
            }
        }
        out.push_str("}\n");
    }
    let result = program.functions[entry.0].result;
    writeln!(
        out,
        "\ndefine i32 @main() {{\nentry:\n  %value = call {} @__oxid_fn_{}()",
        ty(result),
        entry.0
    )
    .unwrap();
    match result {
        hir::Ty::Bool => out.push_str(
            "  %wide = zext i1 %value to i32\n  %status = call i32 @__oxid_print_bool(i32 %wide)\n",
        ),
        hir::Ty::I32 => out.push_str("  %status = call i32 @__oxid_print_i32(i32 %value)\n"),
        hir::Ty::Unit => out.push_str("  %status = call i32 @__oxid_print_unit()\n"),
    }
    out.push_str("  ret i32 %status\n}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{lexer, parser};
    fn verified(text: &str) -> VerifiedProgram {
        let mut sources = SourceMap::new();
        let id = sources.add("native-unit.ox".into(), text.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
        lower_and_verify(&typed, &sources).unwrap()
    }
    #[test]
    fn conservative_cost_counts_both_arms_and_repeated_calls() {
        let p = verified("fn identity(x: bool) -> bool { return x; } fn main() -> bool { if true { identity(false); } else { identity(true); } return false; }");
        let b = p.admit().unwrap();
        // identity: parameter + return temporary + copy + Return.
        assert_eq!(b[0].cost, 4);
        let main = &p.program.functions[1];
        let own = main.locals.len()
            + main
                .blocks
                .iter()
                .map(|b| b.statements.len() + 1)
                .sum::<usize>();
        assert_eq!(b[1].cost, own + 2 * (1 + b[0].cost));
        assert_eq!(b[1].depth, 2);
        assert_eq!(
            b[1].slots,
            main.locals.len() + p.program.functions[0].locals.len()
        );
        assert_eq!(p.run(Some(hir::DefId(1))), Ok(Scalar::Bool(false)));
    }
    #[test]
    fn admission_limits_are_inclusive() {
        let p = verified("fn main() -> () { return; }");
        let span = p.program.functions[0].span;
        for (cap, label) in [
            (MAX_COST, "fuel"),
            (MAX_DEPTH, "depth"),
            (MAX_PARAMS, "parameters"),
            (MAX_FUNCTION_LOCALS, "locals"),
            (MAX_NATIVE_LOCALS, "aggregate locals"),
            (MAX_NATIVE_BLOCKS, "blocks"),
            (MAX_FUNCTIONS, "functions"),
        ] {
            assert!(limit(cap, cap, label, span).is_ok());
            assert!(limit(cap + 1, cap, label, span).is_err());
        }
    }
    #[test]
    fn lowering_uses_private_scalar_abi_and_never_source_identifiers() {
        let p = verified("fn printf(value: i32) -> i32 { return value; } fn main() -> i32 { return printf(-2147483648); }");
        let ir = p.native_module(Some(hir::DefId(1))).unwrap();
        assert!(!ir.contains("printf"));
        assert!(ir.contains("define internal i32 @__oxid_fn_0(i32 %v0) noinline"));
        assert!(ir.contains("-2147483648"));
        assert!(!ir.contains("nsw"));
        assert!(!ir.contains("poison"));
        assert!(!ir.contains("undef"));
    }
}
