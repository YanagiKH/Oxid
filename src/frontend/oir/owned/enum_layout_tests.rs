//! Current-source measurements: admission must account for enclosing carriers,
//! not just the newly introduced payload types.
use super::*;
#[test]
fn bounded_enum_enclosing_layout_measurements() {
    use crate::frontend::ast;
    use std::mem::{align_of, size_of};
    macro_rules! report {
        ($($t:ty),+ $(,)?) => { $(
            println!("enum-layout {} bytes={} align={}", stringify!($t), size_of::<$t>(), align_of::<$t>());
        )+ };
    }
    report!(
        ast::Program,
        ast::Expr,
        ast::ExprKind,
        ast::Stmt,
        ast::StmtKind,
        ast::Function,
        ast::TypeSyntax,
        RawOwnedProgram,
        verified::VerifiedOwnedProgram,
        RawEnumDecl,
        RawVariantDecl,
        Declarations,
        EnumDeclarations,
        RawOwnedFunction,
        OwnedInstruction,
        OwnedStatement,
        OwnedTerminatorKind,
        OwnedTerminator,
        OwnedBlock,
        OwnerDecl,
        AggregateTy,
        AggregateSlot,
        ValueTy,
        ParameterTy,
        plan::FrameUsage,
        plan::FunctionPlan,
        plan::CallPlan,
        budget::FunctionCounts,
    );
}
