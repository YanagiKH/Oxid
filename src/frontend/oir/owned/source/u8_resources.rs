//! RFC0030 named carrier successor. The predecessor banks stay byte-identical.
//! These are conservative overlapping role inventories, not native stack or RSS.
use super::super::*;
use super::{association::AssociatedOwned, hir};
use crate::frontend::{ast, diagnostic::Diagnostic, parser::MAX_NESTING};
use std::mem::size_of;

#[allow(dead_code)]
struct ResolverConversionFrame {
    operation: &'static ast::ConversionOp,
    child: &'static ast::ExprId,
    name: &'static Span,
    source_expr: ast::ExprId,
    child_return: Result<hir::ExprId, Box<Diagnostic>>,
    resolved_child: hir::ExprId,
    construction: hir::ExprKind,
}
#[allow(dead_code)]
struct TypeConversionFrame {
    operation: &'static ast::ConversionOp,
    operand: &'static hir::ExprId,
    pair_return: (hir::Ty, hir::Ty),
    input: hir::Ty,
    output: hir::Ty,
    child_return: Result<ValueTy, Box<Diagnostic>>,
    actual: ValueTy,
    expected: ValueTy,
    returned: ValueTy,
}
#[allow(dead_code)]
struct LowerConversionCarriers {
    source_expr: ast::ExprId,
    operation: ast::ConversionOp,
    operand: hir::ExprId,
    name: Span,
    operand_return: Result<Operand, OwnedFailure>,
    constructed: Rvalue,
}
#[allow(dead_code)]
struct AssociationOwnershipCarriers {
    constructed: AssociatedOwned<'static>,
    returned: Result<AssociatedOwned<'static>, Box<Diagnostic>>,
    normalized: Result<AssociatedOwned<'static>, Vec<Diagnostic>>,
    caller: AssociatedOwned<'static>,
    consumer_argument: AssociatedOwned<'static>,
    consumed_tuple: (RawOwnedProgram, &'static SourceMap),
    consumer_source_flag: bool,
    validation_source_flag: bool,
    shape_source_flag: bool,
    source_context: super::association::FunctionSource<'static>,
    source_context_pattern: (
        &'static ast::Program,
        &'static ast::Function,
        &'static SourceMap,
        usize,
    ),
    iterator: std::iter::Enumerate<std::slice::Iter<'static, OwnedStatement>>,
    next: Option<(usize, &'static OwnedStatement)>,
    position: usize,
    previous_position: Option<usize>,
    previous_instruction: Option<&'static OwnedStatement>,
    previous: Option<&'static Statement>,
    assign: &'static Assign,
    helper_return: Result<(), Box<Diagnostic>>,
    typed_source: &'static super::typeck::TypedOwnedProgram<'static>,
    source_seed: Option<usize>,
    optional_typed: Option<&'static super::typeck::TypedOwnedProgram<'static>>,
    seed_return: Result<usize, Box<Diagnostic>>,
    conversion_plan: super::hir_budget::HirPlan,
    conversion_plan_return: Result<super::hir_budget::HirPlan, Box<Diagnostic>>,
    projected_seed_return: Result<usize, Box<Diagnostic>>,
    plan_index: &'static crate::frontend::declaration_index::DeclarationIndex<'static>,
    plan_work: &'static crate::frontend::declaration_index::WorkMeter,
    dynamic_projection_bytes: usize,
    conversion_origin: Span,
    // Actual-capacity inventory roles are distinct from the AST plan walk.
    capacity_inventory: super::hir_budget::CapacityExcess<'static>,
    capacity_inventory_construction: super::hir_budget::CapacityExcess<'static>,
    capacity_inventory_return: super::hir_budget::CapacityExcess<'static>,
    capacity_inventory_consumed: super::hir_budget::CapacityExcess<'static>,
    inventory_work: &'static crate::frontend::declaration_index::WorkMeter,
    inventory_origin: Span,
    inventory_zero: usize,
    typed_capacity_receiver: &'static super::typeck::TypedOwnedProgram<'static>,
    capacity_vector_return: Result<(), Box<Diagnostic>>,
    capacity_finish_return: usize,
    projection_patterns: [&'static hir::Projection; 2],
    vector_patterns: [&'static Vec<usize>; 4],
    capacity_inventory_receiver: &'static mut super::hir_budget::CapacityExcess<'static>,
    capacity_total_return: Result<usize, Box<Diagnostic>>,
    capacity_excess: usize,
    projection_cell: &'static std::cell::Cell<usize>,
    projection_addition: Option<usize>,
    dynamic: usize,
    vector_header: &'static Vec<usize>,
    vector_capacity: usize,
    vector_length: usize,
    vector_subtraction: Option<usize>,
    vector_excess: usize,
    vector_width: usize,
    arithmetic_results: [Result<usize, Box<Diagnostic>>; 2],
    capacity_debit: Result<(), Box<Diagnostic>>,
    resolved_receiver: &'static super::resolve::ResolvedOwnedProgram<'static>,
    resolved_return: Result<(), Box<Diagnostic>>,
    records: std::slice::Iter<'static, hir::Record>,
    next_record: Option<&'static hir::Record>,
    signatures: std::slice::Iter<'static, hir::Signature>,
    next_signature: Option<&'static hir::Signature>,
    functions: std::slice::Iter<'static, hir::Function>,
    next_function: Option<&'static hir::Function>,
    expressions: std::slice::Iter<'static, hir::Expr>,
    next_expression: Option<&'static hir::Expr>,
    blocks: std::slice::Iter<'static, hir::BodyBlock>,
    next_block: Option<&'static hir::BodyBlock>,
    statements: std::slice::Iter<'static, hir::Stmt>,
    next_statement: Option<&'static hir::Stmt>,
    typed_bodies: std::slice::Iter<'static, super::typeck::TypedBody>,
    next_body: Option<&'static super::typeck::TypedBody>,
    projections: std::slice::Iter<'static, Option<hir::Projection>>,
    next_projection: Option<&'static Option<hir::Projection>>,
    statement_rows: std::slice::Iter<'static, Vec<Option<hir::Projection>>>,
    next_statement_row: Option<&'static Vec<Option<hir::Projection>>>,
    statement_projections: std::slice::Iter<'static, Option<hir::Projection>>,
    next_statement_projection: Option<&'static Option<hir::Projection>>,
    borrow_projections: std::slice::Iter<'static, super::typeck::BorrowProjection>,
    next_borrow_projection: Option<&'static super::typeck::BorrowProjection>,
    owner_count: usize,
    total_expressions: usize,
    extent: usize,
    owner_loop: std::ops::Range<usize>,
    requested: usize,
    additional: usize,
    subtraction: Option<usize>,
    admitted: Result<usize, OwnedFailure>,
}

pub(super) const fn components() -> [usize; 5] {
    [
        size_of::<ResolverConversionFrame>(),
        size_of::<TypeConversionFrame>(),
        size_of::<LowerConversionCarriers>(),
        size_of::<AssociationOwnershipCarriers>(),
        crate::frontend::oir::source::association::conversion_carrier_bytes(),
    ]
}
pub(super) const fn fixed_bytes() -> usize {
    let c = components();
    // Recursive resolver/type frames retain their full possible depth. Lowering
    // and source authentication finish each operation before visiting the next.
    MAX_NESTING * (c[0] + c[1]) + c[2] + c[3] + c[4]
}
#[test]
fn rfc0030_owned_named_carrier_successor_measurement() {
    println!(
        "RFC0030 owned named carrier components={:?} depth={} additional_fixed_bytes={}",
        components(),
        MAX_NESTING,
        fixed_bytes()
    );
    assert!(fixed_bytes() > 0);
}
