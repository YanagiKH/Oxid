//! C3a affected-HIR sizing. This is not source admission authority.
//!
//! Only a cfg(test) resolver-storage observation consumes this plan. It accounts
//! complete known carriers, cumulative scratch and Option-to-final coexistence.
//! Typechecking, match cursor frames and later allocation paths must be priced
//! before they become reachable; the current frame types are not placeholders
//! for those later types. There is deliberately no retained enum table or ledger.
//!
//! AST/text, immutable index payload/query scratch, unchanged record-layout
//! graph scratch, diagnostics, observation logs, raw/consumer plans and machine
//! stack/allocator metadata are separate. This is not a global HIR or RSS cap.
#![allow(dead_code)] // Production source/consumer activation remains closed.

use super::{hir::*, lower, resolve, resolver_storage, typeck};
use crate::frontend::{
    ast,
    declaration_index::{DeclarationIndex, WorkMeter},
    diagnostic::Diagnostic,
    owned_diagnostic,
    parser::{MAX_BLOCK_NESTING, MAX_NESTING},
    project::budget::{Allocator, ReserveFailure},
    source::Span,
};
use std::mem::size_of;

// Same numeric ceiling, separate from AST/index/raw admission. Future dynamic
// projection capacities spend the remainder of this allowance, not a second cap.
pub(super) const MAX_HIR_BYTES: usize = super::budget::MAX_RAW_BYTES;

/// Sorted borrowed-name row. An active binding's diagnostic origin is
/// already in Binding::span; retaining another active Span would be redundant.
#[derive(Clone, Copy, Debug)]
pub(super) struct ScopeName {
    pub(super) name: Span,
    pub(super) active: Option<BindingId>,
}
/// Measured scope shape for the private resolver-only observation.
type ScopeStorage = resolver_storage::PaidScope;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct HirCounts {
    pub(super) records: usize,
    pub(super) record_fields: usize,
    pub(super) max_record_fields: usize,
    pub(super) functions: usize,
    pub(super) parameters: usize,
    pub(super) bindings: usize,
    pub(super) expressions: usize,
    pub(super) blocks: usize,
    pub(super) statements: usize,
    pub(super) calls: usize,
    pub(super) call_arguments: usize,
    pub(super) borrow_arguments: usize,
    pub(super) record_literals: usize,
    pub(super) field_initializers: usize,
    pub(super) array_literals: usize,
    pub(super) array_entries: usize,
    pub(super) matches: usize,
    pub(super) match_arms: usize,
    pub(super) scope_marks: usize,
    pub(super) loop_slots: usize,
    pub(super) resolve_frames: usize,
    pub(super) type_frames: usize,
    pub(super) max_expression_depth: usize,
    pub(super) max_body_depth: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HirPlan {
    pub(super) counts: HirCounts,
    pub(super) resolved: usize,
    pub(super) typed: usize,
    pub(super) staging: usize,
    pub(super) resolver_scratch: usize,
    pub(super) typeck_scratch: usize,
    pub(super) fixed: usize,
    pub(super) lower_fixed: usize,
    pub(super) total: usize,
}
/// Conservative named-value model, never instantiated as a compiler frame.
/// Count each complete wrapper once: its embedded payload is not another field.
/// Distinct fields cover separate construction/transfer/caller storage without
/// assuming that Rust elides a Copy, a return slot or an Option construction.
struct PlanReturnEnvelope {
    // preflight's accumulator and calculate's by-value argument.
    counts: [HirCounts; 2],
    // calculate's Self construction, map's input payload, one caller-retained
    // plan, and with_dynamic's by-value self. These lifetime windows are summed
    // conservatively, not asserted to be an exact simultaneous machine peak.
    plans: [HirPlan; 4],
    calculate_return: Result<HirPlan, Box<Diagnostic>>,
    mapped_option: Option<HirPlan>,
    preflight_return: Result<Option<HirPlan>, Box<Diagnostic>>,
}
struct CapacityReturnEnvelope {
    // new's Self construction, one caller ticket, reserve's by-value self and
    // check_observed's nested by-value self. The Result embeds its own payload.
    capacities: [Capacity; 4],
    created: Result<Capacity, Box<Diagnostic>>,
}
struct CursorTemporaries {
    // Insertion's row and Some(row) can coexist with the independently charged
    // destination array. The active cursor is borrowed from that array.
    expression: ExprCursor,
    expression_slot: Option<ExprCursor>,
    expression_child: Option<ast::ExprId>,
    block: BlockCursor,
    block_slot: Option<BlockCursor>,
    block_child: Option<ast::BodyBlockId>,
}
struct ScalarReturnEnvelope {
    // count_function -> count_statement -> count_expression -> visit -> debit
    // is the longest unit-return chain; increment/charge paths are shorter.
    units: [Result<(), Box<Diagnostic>>; 5],
    // Covers add -> admit -> with_dynamic and saved/intermediate arithmetic
    // values in charge/coexist without assuming return-slot reuse. The same
    // saved-scalar bank covers post-child ExprId guard/append temporaries: those
    // arise after recursion returns, not as another pending recursive value.
    sizes: [Result<usize, Box<Diagnostic>>; 3],
    work_conversion: Result<u64, Box<Diagnostic>>,
}
struct VectorReturnEnvelope<T> {
    // reserve's owned input header and its complete fallible return carrier;
    // backing capacity is separately charged, not included in this fixed model.
    local: Vec<T>,
    returned: Result<Vec<T>, Box<Diagnostic>>,
}
// The primitive currently has only test callers. Measure every row type in the
// passive plan (plus its actual u64 test instantiation) instead of assuming all
// generic Result<Vec<T>, _> layouts are identical. New uses must extend this set.
pub(super) const VECTOR_RETURN_ENVELOPE_BYTES: usize = {
    let mut largest = 0;
    macro_rules! include {
        ($($ty:ty),* $(,)?) => { $(
            let bytes = size_of::<VectorReturnEnvelope<$ty>>();
            if bytes > largest { largest = bytes; }
        )* };
    }
    include!(
        Record,
        Field,
        Signature,
        Function,
        ParameterTy,
        Binding,
        Expr,
        BodyBlock,
        Stmt,
        Argument,
        FieldInit,
        ExprId,
        MatchArm,
        typeck::TypedBody,
        ValueTy,
        Option<Projection>,
        typeck::FlowSummary,
        Vec<Option<Projection>>,
        typeck::BorrowProjection,
        Option<ValueTy>,
        Option<ParameterTy>,
        Option<typeck::FlowSummary>,
        ScopeName,
        usize,
        LoopId,
        resolve::ResolveFrame,
        typeck::TypeFrame,
        (ParameterTy, Span),
        bool,
        u64,
    );
    largest
};

fn failure(message: &'static str, at: Span) -> Box<Diagnostic> {
    owned_diagnostic::diagnostic("E0400", "resolve", format_args!("{message}"), Some(at))
}
fn invalid(at: Span) -> Box<Diagnostic> {
    owned_diagnostic::diagnostic(
        "E0500",
        "resolve",
        format_args!("invalid source structure for HIR preflight"),
        Some(at),
    )
}
fn add(left: usize, right: usize, at: Span) -> Result<usize, Box<Diagnostic>> {
    left.checked_add(right)
        .ok_or_else(|| failure("affected HIR size overflow", at))
}
fn mul(left: usize, right: usize, at: Span) -> Result<usize, Box<Diagnostic>> {
    left.checked_mul(right)
        .ok_or_else(|| failure("affected HIR size overflow", at))
}
fn increment(value: &mut usize, amount: usize, at: Span) -> Result<(), Box<Diagnostic>> {
    *value = add(*value, amount, at)?;
    Ok(())
}
fn charge<T>(bytes: &mut usize, slots: usize, at: Span) -> Result<(), Box<Diagnostic>> {
    increment(bytes, mul(slots, size_of::<T>(), at)?, at)
}
fn visit(work: &WorkMeter, amount: usize, at: Span) -> Result<(), Box<Diagnostic>> {
    let amount = u64::try_from(amount).map_err(|_| failure("affected HIR work overflow", at))?;
    work.debit(amount, at, "affected HIR preflight")
}
fn admit(total: usize, extra: usize, limit: usize, at: Span) -> Result<usize, Box<Diagnostic>> {
    let total = add(total, extra, at)?;
    if total <= limit.min(MAX_HIR_BYTES) {
        Ok(total)
    } else {
        Err(failure(
            "affected HIR and projection payload limit exceeded",
            at,
        ))
    }
}
impl HirPlan {
    fn calculate(c: HirCounts, at: Span) -> Result<Self, Box<Diagnostic>> {
        let mut resolved = 0;
        charge::<Record>(&mut resolved, c.records, at)?;
        charge::<Field>(&mut resolved, c.record_fields, at)?;
        charge::<Signature>(&mut resolved, c.functions, at)?;
        charge::<Function>(&mut resolved, c.functions, at)?;
        charge::<ParameterTy>(&mut resolved, c.parameters, at)?;
        charge::<Binding>(&mut resolved, c.bindings, at)?;
        charge::<Expr>(&mut resolved, c.expressions, at)?;
        charge::<BodyBlock>(&mut resolved, c.blocks, at)?;
        charge::<Stmt>(&mut resolved, c.statements, at)?;
        charge::<Argument>(&mut resolved, c.call_arguments, at)?;
        charge::<FieldInit>(&mut resolved, c.field_initializers, at)?;
        charge::<ExprId>(&mut resolved, c.array_entries, at)?;
        // Match's Vec header is already inside Stmt, never charged twice.
        charge::<MatchArm>(&mut resolved, c.match_arms, at)?;

        let mut typed = 0;
        // All SIX headers, including sparse borrow projections, are embedded.
        charge::<typeck::TypedBody>(&mut typed, c.functions, at)?;
        charge::<ValueTy>(&mut typed, c.expressions, at)?;
        charge::<Option<Projection>>(&mut typed, c.expressions, at)?;
        charge::<ParameterTy>(&mut typed, c.bindings, at)?;
        charge::<typeck::FlowSummary>(&mut typed, c.blocks, at)?;
        charge::<Vec<Option<Projection>>>(&mut typed, c.blocks, at)?;
        charge::<Option<Projection>>(&mut typed, c.statements, at)?;
        charge::<typeck::BorrowProjection>(&mut typed, c.borrow_arguments, at)?;

        // Explicitly coexist with all final vectors; no assumed collect reuse.
        let mut staging = 0;
        charge::<Option<ValueTy>>(&mut staging, c.expressions, at)?;
        charge::<Option<ParameterTy>>(&mut staging, c.bindings, at)?;
        charge::<Option<typeck::FlowSummary>>(&mut staging, c.blocks, at)?;
        charge::<Vec<Option<ValueTy>>>(&mut staging, c.functions, at)?;
        charge::<Vec<Option<ParameterTy>>>(&mut staging, c.functions, at)?;
        charge::<Vec<Option<typeck::FlowSummary>>>(&mut staging, c.functions, at)?;

        let mut resolver_scratch = 0;
        // PaidScope is embedded in the complete Resolver policy below. Its
        // backing rows are separate; do not charge that embedded header twice.
        increment(
            &mut resolver_scratch,
            mul(c.functions, resolver_storage::function_carrier_bytes(), at)?,
            at,
        )?;
        charge::<ScopeName>(&mut resolver_scratch, c.bindings, at)?;
        charge::<usize>(&mut resolver_scratch, c.bindings, at)?;
        charge::<usize>(&mut resolver_scratch, c.scope_marks, at)?;
        charge::<LoopId>(&mut resolver_scratch, c.loop_slots, at)?;
        charge::<Vec<LoopId>>(&mut resolver_scratch, c.functions, at)?;
        charge::<resolve::ResolveFrame>(&mut resolver_scratch, c.resolve_frames, at)?;
        charge::<Vec<resolve::ResolveFrame>>(&mut resolver_scratch, c.functions, at)?;
        // Complete current resolver header, including inherited inline map.
        increment(
            &mut resolver_scratch,
            mul(c.functions, resolve::resolver_carrier_bytes(), at)?,
            at,
        )?;
        // Pending local vector headers coexist with complete prepaid HIR rows.
        // Sum across visits rather than assume only the largest nested call.
        charge::<Vec<Argument>>(&mut resolver_scratch, c.calls, at)?;
        increment(
            &mut resolver_scratch,
            mul(
                c.call_arguments,
                resolver_storage::argument_carrier_bytes(),
                at,
            )?,
            at,
        )?;
        increment(
            &mut resolver_scratch,
            mul(
                c.record_literals,
                resolver_storage::literal_lookup_carrier_bytes(),
                at,
            )?,
            at,
        )?;
        charge::<Vec<FieldInit>>(&mut resolver_scratch, c.record_literals, at)?;
        charge::<Vec<ExprId>>(&mut resolver_scratch, c.array_literals, at)?;
        charge::<Vec<Field>>(&mut resolver_scratch, c.records, at)?;
        charge::<Vec<ParameterTy>>(&mut resolver_scratch, c.functions, at)?;
        charge::<Vec<BodyBlock>>(&mut resolver_scratch, c.functions, at)?;
        // Paid branches additionally name inner params/blocks/frames before
        // transfer to the existing outer buffers, plus each block body and
        // each call's resolved-arguments builder. Keep the old charges above.
        increment(
            &mut resolver_scratch,
            mul(
                c.functions,
                resolver_storage::function_branch_header_bytes(),
                at,
            )?,
            at,
        )?;
        charge::<Vec<Stmt>>(&mut resolver_scratch, c.blocks, at)?;
        charge::<Vec<Argument>>(&mut resolver_scratch, c.calls, at)?;
        // The paid duplicate branches retain empty local legacy map headers,
        // never their backing tables. Count those changed complete headers.
        charge::<std::collections::HashMap<&str, Span>>(
            &mut resolver_scratch,
            add(c.records, c.record_literals, at)?,
            at,
        )?;

        let mut typeck_scratch = 0;
        charge::<typeck::TypeFrame>(&mut typeck_scratch, c.type_frames, at)?;
        charge::<Vec<typeck::TypeFrame>>(&mut typeck_scratch, c.functions, at)?;
        charge::<(ParameterTy, Span)>(&mut typeck_scratch, c.call_arguments, at)?;
        charge::<Vec<(ParameterTy, Span)>>(&mut typeck_scratch, c.calls, at)?;
        // The selected record is unresolved here. Every literal can require
        // the largest whole declaration's presence vector, even when missing
        // all fields. Cumulative sums cover nested pending outer vectors.
        charge::<bool>(
            &mut typeck_scratch,
            mul(c.record_literals, c.max_record_fields, at)?,
            at,
        )?;
        charge::<Vec<bool>>(&mut typeck_scratch, c.record_literals, at)?;
        charge::<[u64; 4]>(&mut typeck_scratch, c.matches, at)?;

        let mut fixed = size_of::<typeck::TypedOwnedProgram<'_>>();
        // TypedOwnedProgram already encloses ResolvedOwnedProgram, its index
        // owner, source view and all top-level Vec headers. Do not add them again.
        // These whole measured models include each embedded Result/Option
        // payload once, plus the separately named copies/temporaries above.
        // This is an explicit carrier envelope, not a machine-stack/RSS bound.
        charge::<PlanReturnEnvelope>(&mut fixed, 1, at)?;
        charge::<CapacityReturnEnvelope>(&mut fixed, 1, at)?;
        charge::<CursorTemporaries>(&mut fixed, 1, at)?;
        charge::<ScalarReturnEnvelope>(&mut fixed, 1, at)?;
        increment(&mut fixed, resolver_storage::fixed_carrier_bytes(), at)?;
        increment(&mut fixed, VECTOR_RETURN_ENVELOPE_BYTES, at)?;
        charge::<[Option<ExprCursor>; MAX_NESTING]>(&mut fixed, 1, at)?;
        charge::<[Option<BlockCursor>; MAX_BLOCK_NESTING]>(&mut fixed, 1, at)?;
        let mut lower_fixed = 0;
        for bytes in lower::fixed_carrier_bytes() {
            increment(&mut lower_fixed, bytes, at)?;
        }
        let mut total = 0;
        for bytes in [
            resolved,
            typed,
            staging,
            resolver_scratch,
            typeck_scratch,
            fixed,
            lower_fixed,
        ] {
            total = admit(total, bytes, MAX_HIR_BYTES, at)?;
        }
        Ok(Self {
            counts: c,
            resolved,
            typed,
            staging,
            resolver_scratch,
            typeck_scratch,
            fixed,
            lower_fixed,
            total,
        })
    }
    /// Model the one shared seed-plus-projection boundary without mutating a
    /// resolved program or granting authority to allocate any particular vector.
    pub(super) fn with_dynamic(self, bytes: usize, at: Span) -> Result<usize, Box<Diagnostic>> {
        admit(self.total, bytes, MAX_HIR_BYTES, at)
    }
}

#[derive(Clone, Copy)]
struct ExprCursor {
    id: ast::ExprId,
    next: usize,
}
#[derive(Clone, Copy)]
struct BlockCursor {
    id: ast::BodyBlockId,
    statement: usize,
    child: usize,
}

/// Entire immutable index is the selector, including unused/imported enums.
/// Zero-enum programs perform no new traversal, work debit or HIR admission.
pub(super) fn preflight_enum_hir(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
) -> Result<Option<HirPlan>, Box<Diagnostic>> {
    if index.enum_count() == 0 {
        return Ok(None);
    }
    let sources = index.sources();
    let at = sources.eof();
    let mut counts = HirCounts::default();
    for id in 0..index.record_count() {
        visit(work, 1, at)?;
        let (key, module) = index.record(RecordId(id))?;
        let record = &sources.ast(module)?.records[key.index];
        visit(work, record.fields.len(), record.span)?;
        increment(&mut counts.records, 1, record.span)?;
        increment(&mut counts.record_fields, record.fields.len(), record.span)?;
        counts.max_record_fields = counts.max_record_fields.max(record.fields.len());
    }
    for id in 0..index.function_count() {
        visit(work, 1, at)?;
        let (key, module) = index.function(DefId(id))?;
        let ast = sources.ast(module)?;
        count_function(ast, &ast.functions[key.index], work, &mut counts)?;
    }
    HirPlan::calculate(counts, at).map(Some)
}

pub(super) fn count_function(
    ast: &ast::Program,
    function: &ast::Function,
    work: &WorkMeter,
    c: &mut HirCounts,
) -> Result<(), Box<Diagnostic>> {
    let at = function.name;
    increment(&mut c.functions, 1, at)?;
    visit(work, function.params.len(), at)?;
    increment(&mut c.parameters, function.params.len(), at)?;
    increment(&mut c.bindings, function.params.len(), at)?;
    let mut frames = [None; MAX_BLOCK_NESTING];
    let mut depth = 1usize;
    let mut maximum = 1usize;
    let mut next_block = 1usize;
    if function.body.0 != 0 || function.blocks.is_empty() {
        return Err(invalid(at));
    }
    frames[0] = Some(BlockCursor {
        id: function.body,
        statement: 0,
        child: 0,
    });
    visit(work, 1, at)?;
    increment(&mut c.blocks, 1, at)?;
    while depth != 0 {
        let cursor = frames[depth - 1].as_mut().ok_or_else(|| invalid(at))?;
        let block = function
            .blocks
            .get(cursor.id.0)
            .ok_or_else(|| invalid(at))?;
        let Some(statement) = block.body.get(cursor.statement) else {
            frames[depth - 1] = None;
            depth -= 1;
            continue;
        };
        if cursor.child == 0 {
            count_statement(ast, statement, work, c)?;
        }
        let child = match &statement.kind {
            ast::StmtKind::While { body, .. } if cursor.child == 0 => Some(*body),
            ast::StmtKind::If {
                then_block,
                else_block,
                ..
            } => match cursor.child {
                0 => Some(*then_block),
                1 => *else_block,
                _ => None,
            },
            ast::StmtKind::Match { arms, .. } => arms.get(cursor.child).map(|arm| arm.body),
            _ => None,
        };
        if let Some(child) = child {
            cursor.child = add(cursor.child, 1, statement.span)?;
            visit(work, 1, statement.span)?;
            // Parser preorder is a tree certificate requiring no bitmap. Merely
            // checking forward block IDs permits shared bodies to amplify work.
            if child.0 != next_block || child.0 >= function.blocks.len() {
                return Err(invalid(statement.span));
            }
            if depth == MAX_BLOCK_NESTING {
                return Err(failure(
                    "affected HIR block depth limit exceeded",
                    statement.span,
                ));
            }
            next_block = add(next_block, 1, statement.span)?;
            increment(&mut c.blocks, 1, statement.span)?;
            frames[depth] = Some(BlockCursor {
                id: child,
                statement: 0,
                child: 0,
            });
            depth = add(depth, 1, statement.span)?;
            maximum = maximum.max(depth);
        } else {
            cursor.statement = add(cursor.statement, 1, statement.span)?;
            cursor.child = 0;
        }
    }
    if next_block != function.blocks.len() {
        return Err(invalid(at));
    }
    c.max_body_depth = c.max_body_depth.max(maximum);
    increment(&mut c.scope_marks, maximum, at)?;
    increment(&mut c.loop_slots, maximum, at)?;
    increment(&mut c.resolve_frames, add(mul(maximum, 4, at)?, 8, at)?, at)?;
    increment(&mut c.type_frames, add(mul(maximum, 3, at)?, 8, at)?, at)?;
    Ok(())
}
fn count_statement(
    ast: &ast::Program,
    statement: &ast::Stmt,
    work: &WorkMeter,
    c: &mut HirCounts,
) -> Result<(), Box<Diagnostic>> {
    let at = statement.span;
    visit(work, 1, at)?;
    increment(&mut c.statements, 1, at)?;
    match &statement.kind {
        ast::StmtKind::Let { init, .. } => {
            increment(&mut c.bindings, 1, at)?;
            count_expression(ast, *init, at, work, c)?;
        }
        ast::StmtKind::Assign { value, .. }
        | ast::StmtKind::FieldAssign { value, .. }
        | ast::StmtKind::Expr(value) => count_expression(ast, *value, at, work, c)?,
        ast::StmtKind::IndexAssign { target, value, .. } => {
            // Conservatively include the target's IndexRead wrapper even though
            // current resolution only retains its index child, never the load.
            count_expression(ast, *target, at, work, c)?;
            count_expression(ast, *value, at, work, c)?;
        }
        ast::StmtKind::Return(Some(value)) => count_expression(ast, *value, at, work, c)?,
        ast::StmtKind::If { condition, .. } | ast::StmtKind::While { condition, .. } => {
            count_expression(ast, *condition, at, work, c)?
        }
        ast::StmtKind::Match { arms, .. } => {
            visit(work, arms.len(), at)?;
            increment(&mut c.matches, 1, at)?;
            increment(&mut c.match_arms, arms.len(), at)?;
            for arm in arms {
                if arm.binding.is_some() {
                    increment(&mut c.bindings, 1, arm.span)?;
                }
            }
        }
        ast::StmtKind::Return(None) | ast::StmtKind::Break | ast::StmtKind::Continue => {}
    }
    Ok(())
}
fn count_expression(
    ast: &ast::Program,
    root: ast::ExprId,
    origin: Span,
    work: &WorkMeter,
    c: &mut HirCounts,
) -> Result<(), Box<Diagnostic>> {
    let at = ast
        .expressions
        .get(root.0)
        .ok_or_else(|| invalid(origin))?
        .span;
    let mut frames = [None; MAX_NESTING];
    frames[0] = Some(ExprCursor { id: root, next: 0 });
    let mut depth = 1usize;
    while depth != 0 {
        let cursor = frames[depth - 1].as_mut().ok_or_else(|| invalid(at))?;
        let expression = ast
            .expressions
            .get(cursor.id.0)
            .ok_or_else(|| invalid(at))?;
        let at = expression.span;
        if cursor.next == 0 {
            visit(work, 1, at)?;
            increment(&mut c.expressions, 1, at)?;
            c.max_expression_depth = c.max_expression_depth.max(depth);
            match &expression.kind {
                ast::ExprKind::Call { args, .. }
                | ast::ExprKind::QualifiedValue {
                    args: Some(args), ..
                } => {
                    visit(work, args.len(), at)?;
                    increment(&mut c.calls, 1, at)?;
                    increment(&mut c.call_arguments, args.len(), at)?;
                    for arg in args {
                        if matches!(arg, ast::Argument::Borrow { .. }) {
                            increment(&mut c.borrow_arguments, 1, at)?;
                        }
                    }
                }
                ast::ExprKind::StructLiteral { fields, .. } => {
                    visit(work, fields.len(), at)?;
                    increment(&mut c.record_literals, 1, at)?;
                    increment(&mut c.field_initializers, fields.len(), at)?;
                }
                ast::ExprKind::ArrayLiteral { elements } => {
                    visit(work, elements.len(), at)?;
                    increment(&mut c.array_literals, 1, at)?;
                    increment(&mut c.array_entries, elements.len(), at)?;
                }
                _ => {}
            }
        }
        let child = loop {
            let next = cursor.next;
            cursor.next = add(next, 1, at)?;
            break match &expression.kind {
                ast::ExprKind::Group(inner)
                | ast::ExprKind::Negate { operand: inner, .. }
                | ast::ExprKind::Not { operand: inner, .. }
                | ast::ExprKind::IndexRead { index: inner, .. } => (next == 0).then_some(*inner),
                ast::ExprKind::Arithmetic { left, right, .. }
                | ast::ExprKind::Comparison { left, right, .. }
                | ast::ExprKind::Logical { left, right, .. } => match next {
                    0 => Some(*left),
                    1 => Some(*right),
                    _ => None,
                },
                ast::ExprKind::Call { args, .. }
                | ast::ExprKind::QualifiedValue {
                    args: Some(args), ..
                } => match args.get(next) {
                    Some(ast::Argument::Value(value)) => Some(*value),
                    Some(ast::Argument::Borrow { .. }) => continue,
                    None => None,
                },
                ast::ExprKind::StructLiteral { fields, .. } => {
                    fields.get(next).map(|field| field.value)
                }
                ast::ExprKind::ArrayLiteral { elements } => elements.get(next).copied(),
                _ => None,
            };
        };
        if let Some(child) = child {
            if child.0 >= cursor.id.0 {
                return Err(invalid(at));
            }
            if depth == MAX_NESTING {
                return Err(failure("affected HIR expression depth limit exceeded", at));
            }
            frames[depth] = Some(ExprCursor { id: child, next: 0 });
            depth = add(depth, 1, at)?;
        } else {
            frames[depth - 1] = None;
            depth -= 1;
        }
    }
    Ok(())
}

/// Capacity arithmetic primitive, not a semantic/admission witness. The caller
/// must derive this exact request from a prepaid immutable plan before reserve.
#[derive(Clone, Copy, Debug)]
pub(super) struct Capacity {
    slots: usize,
    bytes: usize,
    width: usize,
}
impl Capacity {
    pub(super) fn new<T>(slots: usize, prepaid: usize, at: Span) -> Result<Self, Box<Diagnostic>> {
        let bytes = mul(slots, size_of::<T>(), at)?;
        admit(0, bytes, prepaid, at)?;
        Ok(Self {
            slots,
            bytes,
            width: size_of::<T>(),
        })
    }
    fn coexist<T>(
        old_capacity: usize,
        new_capacity: usize,
        at: Span,
    ) -> Result<usize, Box<Diagnostic>> {
        mul(add(old_capacity, new_capacity, at)?, size_of::<T>(), at)
    }
    pub(super) fn reserve<T>(
        self,
        allocator: &mut Allocator,
        mut values: Vec<T>,
        at: Span,
        label: &'static str,
    ) -> Result<Vec<T>, Box<Diagnostic>> {
        if self.width != size_of::<T>()
            || values.len() > self.slots
            || values.capacity() > self.slots
        {
            return Err(failure("affected HIR capacity exceeds prepaid request", at));
        }
        // try_reserve_exact takes additional relative to LEN, not capacity.
        let additional = self
            .slots
            .checked_sub(values.len())
            .ok_or_else(|| invalid(at))?;
        allocator
            .vector_exact(&mut values, additional, label)
            .map_err(|failure_kind| {
                failure(
                    match failure_kind {
                        ReserveFailure::Overflow => "affected HIR size overflow",
                        ReserveFailure::Allocation => "affected HIR allocation failed",
                    },
                    at,
                )
            })?;
        self.check_observed(values.capacity(), at)?;
        Ok(values)
    }
    pub(super) fn check_observed(self, capacity: usize, at: Span) -> Result<(), Box<Diagnostic>> {
        if capacity > self.slots {
            Err(failure("affected HIR capacity exceeds prepaid request", at))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "hir_budget_tests.rs"]
mod tests;
