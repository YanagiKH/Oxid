//! Current-source carrier successor evidence, never a stack/RSS claim.
use super::*;
use crate::frontend::{ast, lexer};
use std::mem::{align_of, size_of};
#[test]
fn bounded_u8_shared_carriers_match_predecessor_envelopes() {
    macro_rules! row {
        ($name:literal,$ty:ty,$size:expr,$align:expr) => {{
            println!(
                "U8-CARRIERS-1 {} size={} align={}",
                $name,
                size_of::<$ty>(),
                align_of::<$ty>()
            );
            assert_eq!((size_of::<$ty>(), align_of::<$ty>()), ($size, $align));
        }};
    }
    row!("Ty", hir::Ty, 1, 1);
    row!("Option<Ty>", Option<hir::Ty>, 1, 1);
    row!("Scalar", Scalar, 8, 4);
    row!("Option<Scalar>", Option<Scalar>, 8, 4);
    row!("AST ExprKind", ast::ExprKind, 64, 8);
    row!("HIR ExprKind", hir::ExprKind, 48, 8);
    row!("Rvalue", Rvalue, 96, 8);
    row!("Assign", Assign, 128, 8);
    row!("Span", Span, 24, 8);
    row!("Token", lexer::Token, 32, 8);
    println!("U8-CARRIERS-1 ASTExpr={} HIRExpr={} OIRStatement={} RawFunction={} RunFailure={} OptionRunFailure={} VerifiedProgram={} Operand={}",
        size_of::<ast::Expr>(),size_of::<hir::Expr>(),size_of::<Statement>(),size_of::<Function>(),
        size_of::<RunFailure>(),size_of::<Option<RunFailure>>(),size_of::<VerifiedProgram>(),size_of::<Operand>());
}
