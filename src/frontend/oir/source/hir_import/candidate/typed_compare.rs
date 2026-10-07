//! Disconnected comparison of every active STF1 semantic cell.
//!
//! Only the genuine scalar checker may construct the borrowed TypedProgram.
//! The enclosing leaf must first establish complete candidate/canonical HIR
//! equality and pass that candidate through the checker. This helper borrows
//! its immutable views and the already validated source/OPA correspondence;
//! it cannot construct, repair, retain or return any compiler owner.
//!
//! No dynamic storage is introduced. The source/AST/observation baseline and
//! the checker's inherited TypedProgram, HIR and typed-table allocations are
//! excluded from this helper's *new* named carrier ledger. Their coexistence
//! belongs to the enclosing phase ledger, not to this fixed comparison bank.
use super::{ast, hir, require, small, ComparedSyntax, Failure, FunctionWindow, Mapping, Row};
use crate::frontend::{
    project::ModuleId,
    typeck::{FlowSummary, TypedFunction, TypedProgram},
};
use std::mem::{size_of, size_of_val};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ComparisonFacts {
    pub(super) equal: bool,
    pub(super) cells: usize,
    pub(super) functions: usize,
    pub(super) locals: usize,
    pub(super) expressions: usize,
    pub(super) blocks: usize,
    pub(super) zeroes: usize,
    pub(super) charged_work: u64,
    pub(super) named_bytes: usize,
}

/// Actual scalar scan state, reused for successive functions and row passes.
#[derive(Default)]
struct Cursors {
    function: usize,
    global_expression: usize,
    row: usize,
    local: usize,
    parameter: usize,
    block: usize,
    expression: usize,
    expression_rows: usize,
}

/// Names the actual opaque immutable-function iterator and its held transport.
struct FunctionCursor<I> {
    iterator: I,
}

/// The expected scalar value and supplied value of one comparison. This is
/// reused per cell; it is not an observational cache or a reconstructed table.
struct CellComparison {
    expected: i32,
    supplied: i32,
}

fn type_code(ty: hir::Ty) -> i32 {
    match ty {
        hir::Ty::Bool => 1,
        hir::Ty::I32 => 2,
        hir::Ty::Unit => 3,
    }
}

// STF1's bit representation belongs here, never in the genuine typechecker.
fn outcome_code((fallthrough, returns, breaks, continues): (bool, bool, bool, bool)) -> i32 {
    i32::from(fallthrough)
        | (i32::from(returns) << 1)
        | (i32::from(breaks) << 2)
        | (i32::from(continues) << 3)
}
fn flow_code(flow: FlowSummary) -> i32 {
    outcome_code(flow.outcomes())
}

fn compare_cell(
    syntax: &ComparedSyntax<'_, '_, '_>,
    facts: &mut ComparisonFacts,
    reference: u8,
    expected: i32,
) -> Result<(), Failure> {
    require(reference != 0 && reference <= syntax.bound.wire.rows)?;
    let cell = CellComparison {
        expected,
        supplied: syntax.bound.wire.word(4, usize::from(reference - 1))?,
    };
    facts.equal &= cell.expected == cell.supplied;
    facts.cells = facts.cells.checked_add(1).ok_or(Failure::Overflow)?;
    Ok(())
}

/// Compare every active cell, including all unused-role zeroes. Mismatches are
/// accumulated, never repaired and never used to skip the remaining cells.
/// Empty inputs are valid only when both source and typed function sets are
/// empty. No zip truncation, row omission or owner escapes this entry.
///
/// The caller must prepay `work_bound(rows)` and `named_bytes()` in its shared
/// phase plan. `remaining_work` is the allowance available for this operation,
/// not a fresh meter; this helper returns exactly its conservative debit.
pub(super) fn compare(
    syntax: &ComparedSyntax<'_, '_, '_>,
    typed: &TypedProgram,
    remaining_work: u64,
) -> Result<ComparisonFacts, Failure> {
    let charged_work = work_bound(usize::from(syntax.bound.wire.rows))?;
    if charged_work > remaining_work {
        return Err(Failure::Admission);
    }
    let named_bytes = named_bytes()?;
    let program = syntax
        .bound
        .owner
        .ast(ModuleId(0))
        .map_err(|_| Failure::Shape)?;
    let mapping = Mapping { syntax, program };
    let mut facts = ComparisonFacts {
        equal: true,
        cells: 0,
        functions: 0,
        locals: 0,
        expressions: 0,
        blocks: 0,
        zeroes: 0,
        charged_work,
        named_bytes,
    };
    let mut cursors = Cursors::default();
    let mut functions = FunctionCursor {
        iterator: typed.functions(),
    };
    require(functions.iterator.len() == program.functions.len())?;
    require(!program.functions.is_empty() || syntax.bound.wire.rows == 0)?;
    while cursors.function < program.functions.len() {
        let view = functions.iterator.next().ok_or(Failure::Shape)?;
        let window = mapping.window(cursors.function, cursors.global_expression)?;
        let function = view.hir();
        let source = &program.functions[cursors.function];
        require(function.id.0 == cursors.function)?;
        require(function.locals.len() == window.locals)?;
        require(function.expressions.len() == window.expression_end - window.expression_begin)?;
        require(function.blocks.len() == source.blocks.len())?;
        require(view.signature().params.len() == source.params.len())?;
        require(function.body.0 == source.body.0)?;
        cursors.row = window.row_begin;
        cursors.local = 0;
        cursors.parameter = 0;
        cursors.block = 0;
        cursors.expression_rows = 0;
        while cursors.row < window.row_end {
            let reference = small(cursors.row)?;
            let row = mapping.row(reference)?;
            match row.kind {
                1 => {
                    require(cursors.row == window.row_begin)?;
                    compare_cell(
                        syntax,
                        &mut facts,
                        reference,
                        type_code(view.signature().result),
                    )?;
                    facts.functions = facts.functions.checked_add(1).ok_or(Failure::Overflow)?;
                }
                2 | 6 | 7 => {
                    let local = mapping.declaration(window, reference)?;
                    require(local.0 == cursors.local && local.0 < function.locals.len())?;
                    if row.kind == 2 {
                        require(
                            cursors.local == cursors.parameter
                                && cursors.parameter < source.params.len(),
                        )?;
                        cursors.parameter += 1;
                    }
                    compare_cell(
                        syntax,
                        &mut facts,
                        reference,
                        type_code(view.local_ty(local)),
                    )?;
                    cursors.local += 1;
                    facts.locals = facts.locals.checked_add(1).ok_or(Failure::Overflow)?;
                }
                5 => {
                    let block = mapping.block(window, reference)?;
                    require(block.0 == cursors.block && block.0 < function.blocks.len())?;
                    compare_cell(
                        syntax,
                        &mut facts,
                        reference,
                        flow_code(view.block_flow(block)),
                    )?;
                    cursors.block += 1;
                    facts.blocks = facts.blocks.checked_add(1).ok_or(Failure::Overflow)?;
                }
                15..=36 => {
                    // Source expression IDs are postorder rather than OPA
                    // allocation order. The exact expression pass below uses
                    // the already validated bijective row map instead.
                    cursors.expression_rows += 1;
                }
                3 | 4 | 8..=14 => {
                    compare_cell(syntax, &mut facts, reference, 0)?;
                    facts.zeroes = facts.zeroes.checked_add(1).ok_or(Failure::Overflow)?;
                }
                _ => return Err(Failure::Shape),
            }
            cursors.row += 1;
        }
        require(cursors.local == function.locals.len())?;
        require(cursors.parameter == source.params.len())?;
        require(cursors.block == function.blocks.len())?;
        require(cursors.expression_rows == function.expressions.len())?;
        cursors.expression = window.expression_begin;
        while cursors.expression < window.expression_end {
            let source_id = ast::ExprId(cursors.expression);
            let reference = syntax.expr_row(source_id)?;
            require(window.contains(reference))?;
            require(matches!(mapping.row(reference)?.kind, 15..=36))?;
            let expression = window.expression(source_id)?;
            require(expression.0 < function.expressions.len())?;
            compare_cell(
                syntax,
                &mut facts,
                reference,
                type_code(view.expression_ty(expression)),
            )?;
            facts.expressions = facts.expressions.checked_add(1).ok_or(Failure::Overflow)?;
            cursors.expression += 1;
        }
        cursors.global_expression = window.expression_end;
        cursors.function += 1;
    }
    require(functions.iterator.next().is_none())?;
    require(facts.functions == program.functions.len())?;
    require(cursors.global_expression == program.expressions.len())?;
    require(facts.expressions == program.expressions.len())?;
    require(facts.cells == usize::from(syntax.bound.wire.rows))?;
    let classified = facts
        .functions
        .checked_add(facts.locals)
        .and_then(|n| n.checked_add(facts.expressions))
        .and_then(|n| n.checked_add(facts.blocks))
        .and_then(|n| n.checked_add(facts.zeroes))
        .ok_or(Failure::Overflow)?;
    require(classified == facts.cells)?;
    Ok(facts)
}

/// Eight complete row-bounded passes cover function/typed invariant visits,
/// window expression scans plus their one-per-function lookahead, declaration
/// scans, active rows, the postorder expression pass and parameter invariant
/// equality. Every category is <= rows, and each loop's terminal visit is
/// included. 256 units per visit includes nested row/word/ID reads and fixed
/// comparisons; 1024 covers entry, layout summation and fixed result transport.
/// This is conservative logical work, not a CPU-instruction or runtime-fuel cap.
pub(super) fn work_bound(rows: usize) -> Result<u64, Failure> {
    require(rows <= super::MAX_ROWS)?;
    u64::try_from(rows)
        .map_err(|_| Failure::Overflow)?
        .checked_add(1)
        .and_then(|n| n.checked_mul(8))
        .and_then(|n| n.checked_mul(256))
        .and_then(|n| n.checked_add(1024))
        .ok_or(Failure::Overflow)
}

fn copies<T>(count: usize) -> Result<usize, Failure> {
    size_of::<T>().checked_mul(count).ok_or(Failure::Overflow)
}

// Infer the real opaque iterator type without constructing a TypedProgram or
// invoking the factory. Its captures, return and held iterator are all priced.
fn iterator_named_bytes<I, F>(_: F) -> Result<usize, Failure>
where
    I: ExactSizeIterator<Item = TypedFunction<'static>>,
    F: FnOnce(&'static TypedProgram) -> I,
{
    size_of::<F>()
        .checked_add(size_of::<I>())
        .and_then(|n| n.checked_add(size_of::<FunctionCursor<I>>()))
        .and_then(|n| n.checked_add(size_of::<&mut I>()))
        .ok_or(Failure::Overflow)
}

/// Complete named fixed-value/transport envelope for the new comparison only.
/// Roles across distinct phases are conservatively added, not optimized away.
/// This is not a machine stack maximum, an allocator bound, or a price for the
/// inherited checker/typed owners. The parent must also price its call/result
/// carriers and the source/OPA comparison bank it keeps alive here.
pub(super) fn named_bytes() -> Result<usize, Failure> {
    let roles = [
        size_of::<(&ComparedSyntax<'_, '_, '_>, &TypedProgram, u64)>(),
        copies::<Mapping<'_, '_, '_, '_>>(2)?,
        size_of::<&ast::Program>(),
        size_of::<Result<&ast::Program, Box<crate::frontend::diagnostic::Diagnostic>>>(),
        size_of::<Cursors>(),
        copies::<ComparisonFacts>(2)?,
        copies::<Result<ComparisonFacts, Failure>>(2)?,
        iterator_named_bytes(|typed: &'static TypedProgram| typed.functions())?,
        copies::<TypedFunction<'_>>(2)?,
        size_of::<Option<TypedFunction<'_>>>(),
        size_of::<Result<TypedFunction<'_>, Failure>>(),
        size_of::<(
            &TypedFunction<'_>,
            &hir::Function,
            &hir::Signature,
            &ast::Function,
        )>(),
        size_of::<(&TypedFunction<'_>, hir::LocalId)>(),
        size_of::<(&TypedFunction<'_>, hir::ExprId)>(),
        size_of::<(&TypedFunction<'_>, hir::BodyBlockId)>(),
        copies::<FunctionWindow>(3)?,
        size_of::<Result<FunctionWindow, Failure>>(),
        size_of::<(&Mapping<'_, '_, '_, '_>, usize, usize)>(),
        copies::<std::ops::Range<usize>>(3)?,
        copies::<(&Mapping<'_, '_, '_, '_>, u8)>(3)?,
        copies::<(&Mapping<'_, '_, '_, '_>, FunctionWindow, u8)>(2)?,
        size_of::<(FunctionWindow, ast::ExprId)>(),
        size_of::<(FunctionWindow, u8)>(),
        size_of::<(&ComparedSyntax<'_, '_, '_>, ast::ExprId)>(),
        size_of::<(&ComparedSyntax<'_, '_, '_>, usize)>(),
        size_of::<(&ComparedSyntax<'_, '_, '_>, u8)>(),
        copies::<Row>(3)?,
        copies::<Result<Row, Failure>>(2)?,
        size_of::<Result<Row, super::super::Boundary>>(),
        copies::<Result<u8, super::super::Boundary>>(2)?,
        size_of::<Result<(usize, ast::BodyBlockId), super::super::Boundary>>(),
        size_of::<(usize, ast::BodyBlockId)>(),
        size_of::<(&ComparedSyntax<'_, '_, '_>, &mut ComparisonFacts, u8, i32)>(),
        copies::<CellComparison>(2)?,
        size_of::<(
            super::super::Wire<'_>,
            u8,
            usize,
            u32,
            u32,
            u32,
            u8,
            u32,
            [u32; 4],
        )>(),
        size_of::<(
            super::super::Wire<'_>,
            usize,
            usize,
            usize,
            [u8; 4],
            usize,
            &mut u8,
            usize,
        )>(),
        size_of::<std::iter::Enumerate<std::slice::IterMut<'_, u8>>>(),
        size_of::<std::slice::Iter<'_, u32>>(),
        copies::<Result<i32, super::super::Boundary>>(2)?,
        copies::<Result<i32, Failure>>(2)?,
        copies::<Result<usize, Failure>>(3)?,
        copies::<Result<u8, Failure>>(2)?,
        size_of::<Result<hir::LocalId, Failure>>(),
        size_of::<Result<hir::BodyBlockId, Failure>>(),
        size_of::<Result<hir::ExprId, Failure>>(),
        size_of::<(ast::ExprId, hir::ExprId, hir::LocalId, hir::BodyBlockId)>(),
        copies::<hir::Ty>(2)?,
        copies::<FlowSummary>(2)?,
        copies::<(bool, bool, bool, bool)>(2)?,
        copies::<i32>(6)?,
        copies::<usize>(16)?,
        copies::<u8>(6)?,
        copies::<bool>(6)?,
        copies::<u64>(4)?,
        copies::<Result<(), Failure>>(8)?,
        copies::<Result<usize, Failure>>(3)?,
        copies::<Result<u64, Failure>>(2)?,
    ];
    let bank = size_of_val(&roles)
        .checked_mul(2)
        .and_then(|n| n.checked_add(size_of_val(&roles.into_iter())))
        .ok_or(Failure::Overflow)?;
    roles.into_iter().try_fold(bank, |total, value| {
        total.checked_add(value).ok_or(Failure::Overflow)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_hir_import_typed_projection_covers_all_types_and_flow_bits() {
        assert_eq!(
            [
                type_code(hir::Ty::Bool),
                type_code(hir::Ty::I32),
                type_code(hir::Ty::Unit),
            ],
            [1, 2, 3],
        );
        for mask in 0..16 {
            assert_eq!(
                outcome_code((mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0)),
                mask,
            );
        }
    }

    #[test]
    fn checked_hir_import_typed_comparison_layout_and_work_are_fixed() {
        assert!(named_bytes().unwrap() >= size_of::<Cursors>() + 2 * size_of::<ComparisonFacts>());
        assert!(work_bound(0).unwrap() > 0);
        for rows in 1..=super::super::MAX_ROWS {
            assert_eq!(
                work_bound(rows).unwrap() - work_bound(rows - 1).unwrap(),
                8 * 256,
            );
        }
        assert_eq!(work_bound(super::super::MAX_ROWS + 1), Err(Failure::Shape));
        assert_eq!(work_bound(usize::MAX), Err(Failure::Shape));
        eprintln!(
            "typed comparison facts/result/cursors/cell/named: {}/{}/{}/{}/{}",
            size_of::<ComparisonFacts>(),
            size_of::<Result<ComparisonFacts, Failure>>(),
            size_of::<Cursors>(),
            size_of::<CellComparison>(),
            named_bytes().unwrap(),
        );
    }
}
