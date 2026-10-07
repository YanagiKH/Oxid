//! Contained scalar candidate comparison. The private paid leaf compares and
//! drops the owner; no default compiler caller or typed authority is connected.
//!
//! Values come from source/OPA correspondence and supplied resolution column 3.
//! Canonical HIR is used only by Session for shape and by the equality oracle.
//! No candidate owner/reference, checker result or executable authority escapes.
use super::{
    allocation::{Failure, Key, Receipt, Session},
    ast,
    ast_compare::{ComparedSyntax, Row},
    hir, MAX_ROWS,
};
use crate::frontend::{
    declaration_index::{IndexLimits, WorkMeter},
    lexer::{Kind, Token},
    project::{budget::Allocator, ModuleId},
    source::Span,
};
use std::mem::{size_of, size_of_val};

// Pure complete STF1 comparison is compiled; Verify consumers remain closed.
mod typed_compare;
mod verify_terminal;
pub(super) use verify_terminal::{Facts as VerifyFacts, Rejected as VerifyRejected};

// A source edit after carrier measurement and independent boundary review is
// required to admit success. There is no caller-controlled enablement flag.
pub(super) const VERIFY_ADMITTED: bool = true;

enum CanonicalInput<'h, 'm> {
    Observe(&'h hir::Program),
    Verify {
        canonical: hir::Program,
        context: verify_terminal::Context<'m>,
    },
}
impl CanonicalInput<'_, '_> {
    fn program(&self) -> &hir::Program {
        match self {
            Self::Observe(program) => program,
            Self::Verify { canonical, .. } => canonical,
        }
    }
}

enum Completion {
    Observed(ComparisonFacts),
    Verified(VerifyFacts),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ComparisonFacts {
    pub(super) allocation: Receipt,
    pub(super) equal: bool,
    pub(super) builder_named_bytes: usize,
    pub(super) builder_work: u64,
    /// Builder plus Session only. The caller adds already charged outer work.
    pub(super) charged_work: u64,
}

#[derive(Clone, Copy, Default)]
struct FunctionWindow {
    function: usize,
    row_begin: usize,
    row_end: usize,
    expression_begin: usize,
    expression_end: usize,
    locals: usize,
}
impl FunctionWindow {
    fn contains(self, reference: u8) -> bool {
        (self.row_begin..self.row_end).contains(&usize::from(reference))
    }
    fn expression(self, id: ast::ExprId) -> Result<hir::ExprId, Failure> {
        require((self.expression_begin..self.expression_end).contains(&id.0))?;
        Ok(hir::ExprId(
            id.0.checked_sub(self.expression_begin)
                .ok_or(Failure::Shape)?,
        ))
    }
    fn child(self, id: ast::ExprId, parent: usize) -> Result<hir::ExprId, Failure> {
        require(id.0 < parent)?;
        self.expression(id)
    }
}

// Actual scalar loop bank, reused between passes. There are no new ID maps,
// visited sets, staging vectors, name maps, or source-depth continuation stacks.
#[derive(Default)]
struct BuilderCursors {
    function: usize,
    global_expression: usize,
    row: usize,
    local: usize,
    block: usize,
    statement: usize,
    statement_row: u8,
    expression: usize,
    argument: usize,
    parameter: usize,
    parameter_row: u8,
}

struct Mapping<'a, 'b, 's, 'w> {
    syntax: &'a ComparedSyntax<'b, 's, 'w>,
    program: &'s ast::Program,
}
impl Mapping<'_, '_, '_, '_> {
    fn row(&self, reference: u8) -> Result<Row, Failure> {
        self.syntax.row(reference).map_err(Into::into)
    }
    fn resolution(&self, reference: u8) -> Result<i32, Failure> {
        require(reference != 0 && reference <= self.syntax.bound.wire.rows)?;
        self.syntax
            .bound
            .wire
            .word(3, usize::from(reference - 1))
            .map_err(Into::into)
    }
    fn positive(&self, reference: u8) -> Result<usize, Failure> {
        let value = usize::try_from(self.resolution(reference)?).map_err(|_| Failure::Shape)?;
        require((1..=MAX_ROWS).contains(&value))?;
        Ok(value)
    }
    fn target(&self, reference: u8) -> Result<u8, Failure> {
        let target = u8::try_from(self.positive(reference)?).map_err(|_| Failure::Shape)?;
        require(target <= self.syntax.bound.wire.rows)?;
        Ok(target)
    }
    fn ty(&self, reference: u8) -> Result<hir::Ty, Failure> {
        let row = self.row(reference)?;
        require(matches!(row.kind, 3 | 4))?;
        let ty = match self.resolution(reference)? {
            1 => hir::Ty::Bool,
            2 => hir::Ty::I32,
            3 => hir::Ty::Unit,
            _ => return Err(Failure::Shape),
        };
        require(row.kind != 4 || ty == hir::Ty::Unit)?;
        Ok(ty)
    }
    fn declaration(&self, window: FunctionWindow, reference: u8) -> Result<hir::LocalId, Failure> {
        require(window.contains(reference) && matches!(self.row(reference)?.kind, 2 | 6 | 7))?;
        let ordinal = self.positive(reference)?;
        require(ordinal <= window.locals)?;
        Ok(hir::LocalId(ordinal - 1))
    }
    fn local_target(&self, window: FunctionWindow, reference: u8) -> Result<hir::LocalId, Failure> {
        self.declaration(window, self.target(reference)?)
    }
    fn block(&self, window: FunctionWindow, reference: u8) -> Result<hir::BodyBlockId, Failure> {
        require(window.contains(reference) && self.row(reference)?.kind == 5)?;
        let (function, block) = self.syntax.block_location(reference)?;
        require(
            function == window.function && block.0 < self.program.functions[function].blocks.len(),
        )?;
        Ok(hir::BodyBlockId(block.0))
    }
    fn loop_target(&self, window: FunctionWindow, reference: u8) -> Result<hir::LoopId, Failure> {
        Ok(hir::LoopId(self.block(window, self.target(reference)?)?.0))
    }
    fn callee(&self, reference: u8) -> Result<hir::DefId, Failure> {
        let target = self.target(reference)?;
        require(self.row(target)?.kind == 1)?;
        let id = self.positive(target)? - 1;
        require(id < self.program.functions.len() && self.syntax.function_row(id)? == target)?;
        Ok(hir::DefId(id))
    }
    fn name_span(&self, reference: u8) -> Result<Span, Failure> {
        let index = usize::from(reference.checked_sub(1).ok_or(Failure::Shape)?);
        let token = self.program.tokens.get(index).ok_or(Failure::Shape)?;
        require(token.kind == Kind::Ident)?;
        Ok(token.span)
    }

    fn window(&self, function: usize, expression_begin: usize) -> Result<FunctionWindow, Failure> {
        require(
            function < self.program.functions.len()
                && expression_begin <= self.program.expressions.len(),
        )?;
        let row_begin = usize::from(self.syntax.function_row(function)?);
        // Use usize: the exclusive upper bound after row 128 is 129.
        let row_end = if function + 1 < self.program.functions.len() {
            usize::from(self.syntax.function_row(function + 1)?)
        } else {
            usize::from(self.syntax.bound.wire.rows)
                .checked_add(1)
                .ok_or(Failure::Overflow)?
        };
        require(row_begin < row_end)?;
        let mut expression_end = expression_begin;
        while expression_end < self.program.expressions.len() {
            let row = usize::from(self.syntax.expr_row(ast::ExprId(expression_end))?);
            require(row >= row_begin)?;
            if row >= row_end {
                break;
            }
            expression_end += 1;
        }
        let mut locals = 0usize;
        for reference in row_begin..row_end {
            let reference = small(reference)?;
            if matches!(self.row(reference)?.kind, 2 | 6 | 7) {
                locals = locals.checked_add(1).ok_or(Failure::Overflow)?;
                require(self.positive(reference)? == locals)?;
            }
        }
        Ok(FunctionWindow {
            function,
            row_begin,
            row_end,
            expression_begin,
            expression_end,
            locals,
        })
    }

    // Check every active resolution cell, including unreachable source rows.
    // Plausible but semantically wrong targets remain untrusted until equality.
    fn validate(&self) -> Result<(), Failure> {
        let mut cursors = BuilderCursors::default();
        while cursors.function < self.program.functions.len() {
            let window = self.window(cursors.function, cursors.global_expression)?;
            cursors.row = window.row_begin;
            while cursors.row < window.row_end {
                let reference = small(cursors.row)?;
                let row = self.row(reference)?;
                match row.kind {
                    1 => require(
                        cursors.row == window.row_begin
                            && self.positive(reference)? == cursors.function + 1,
                    )?,
                    2 | 6 | 7 => {
                        self.declaration(window, reference)?;
                    }
                    3 | 4 => {
                        self.ty(reference)?;
                    }
                    8 | 19 => {
                        self.local_target(window, reference)?;
                    }
                    11 | 12 => {
                        self.loop_target(window, reference)?;
                    }
                    15 => {
                        self.resolution(reference)?;
                    }
                    20 => {
                        self.callee(reference)?;
                    }
                    5 | 9 | 10 | 13 | 14 | 16..=18 | 21..=36 => {
                        require(self.resolution(reference)? == 0)?
                    }
                    _ => return Err(Failure::Shape),
                }
                cursors.row += 1;
            }
            cursors.global_expression = window.expression_end;
            cursors.function += 1;
        }
        require(cursors.global_expression == self.program.expressions.len())?;
        require(!self.program.functions.is_empty() || self.syntax.bound.wire.rows == 0)
    }
}
fn require(condition: bool) -> Result<(), Failure> {
    if condition {
        Ok(())
    } else {
        Err(Failure::Shape)
    }
}
fn small(value: usize) -> Result<u8, Failure> {
    u8::try_from(value)
        .ok()
        .filter(|&v| v != 0 && usize::from(v) <= MAX_ROWS)
        .ok_or(Failure::Shape)
}

/// The caller must bind `canonical` to the genuine resolver result for this
/// exact immutable source owner. `outside_fixed_bytes` supplies all outer
/// source/comparator/owner/result roles, including the canonical Program header
/// and the full Allocator owner. It excludes this module's measured bank and
/// Session's helper bank. Source/AST/input backing remains a separate baseline.
///
/// `remaining` is the SAME original work allowance after outer work is charged;
/// it is never replenished here. Returned charged_work excludes that outer work.
/// This entry returns fixed facts only, including on a complete disagreement.
/// Its parent must retain an uninhabited success type even when `equal` is true.
pub(super) fn compare_candidate(
    syntax: &ComparedSyntax<'_, '_, '_>,
    canonical: &hir::Program,
    allocator: &mut Allocator,
    outside_fixed_bytes: usize,
    remaining: IndexLimits,
) -> Result<ComparisonFacts, Failure> {
    match construct(
        syntax,
        CanonicalInput::Observe(canonical),
        allocator,
        outside_fixed_bytes,
        remaining,
    ) {
        Ok(Completion::Observed(facts)) => Ok(facts),
        Err(VerifyRejected::Candidate(failure)) => Err(failure),
        // Neither can arise from Observe; no compiler owner crosses the match.
        Ok(Completion::Verified(_)) | Err(_) => Err(Failure::Shape),
    }
}

#[allow(clippy::too_many_arguments, clippy::result_large_err)]
pub(super) fn verify_candidate(
    syntax: &ComparedSyntax<'_, '_, '_>,
    canonical: hir::Program,
    allocator: &mut Allocator,
    outside_fixed_bytes: usize,
    remaining: IndexLimits,
    work: &WorkMeter,
    origin: Span,
) -> Result<VerifyFacts, VerifyRejected> {
    if !VERIFY_ADMITTED {
        return Err(VerifyRejected::Disabled);
    }
    match construct(
        syntax,
        CanonicalInput::Verify {
            canonical,
            context: verify_terminal::Context { work, origin },
        },
        allocator,
        outside_fixed_bytes,
        remaining,
    )? {
        Completion::Verified(facts) => Ok(facts),
        Completion::Observed(_) => Err(Failure::Shape.into()),
    }
}

/// The one construction body serves observation and the closed Verify path.
/// Observe drops the candidate inside its existing allocation-observer scope;
/// Verify owns canonical HIR so it can release it before genuine typechecking.
#[allow(clippy::result_large_err)]
fn construct(
    syntax: &ComparedSyntax<'_, '_, '_>,
    input: CanonicalInput<'_, '_>,
    allocator: &mut Allocator,
    outside_fixed_bytes: usize,
    remaining: IndexLimits,
) -> Result<Completion, VerifyRejected> {
    let canonical = input.program();
    let program = syntax
        .bound
        .owner
        .ast(ModuleId(0))
        .map_err(|_| Failure::Shape)?;
    let mapping = Mapping { syntax, program };
    let builder_named_bytes = builder_named_bytes()?;
    let fixed = outside_fixed_bytes
        .checked_add(builder_named_bytes)
        .ok_or(Failure::Overflow)?;
    let builder_work = work_bound(usize::from(syntax.bound.wire.rows))?;
    let helper_limits = IndexLimits {
        work: remaining
            .work
            .min(IndexLimits::default().work)
            .checked_sub(builder_work)
            .ok_or(Failure::Admission)?,
        ..remaining
    };
    mapping.validate()?;
    let session = Session::admit(canonical, allocator, fixed, helper_limits)?;
    let prepaid = if let CanonicalInput::Verify { context, .. } = &input {
        // Read only admitted metadata. This cannot stand in for the mandatory
        // final complete() fill/order/capacity checks below.
        let admitted = session.admitted_receipt();
        let plan = verify_terminal::WorkPlan::calculate(
            admitted.requested,
            usize::from(syntax.bound.wire.rows),
        )?;
        let charged = builder_work
            .checked_add(admitted.helper_work)
            .and_then(|n| n.checked_add(plan.total))
            .ok_or(Failure::Overflow)?;
        // Preserve exactly the remaining original allowance; no fresh meter,
        // later candidate debit, refund or replenishment is possible here.
        require(
            remaining.work
                <= context
                    .work
                    .limit()
                    .checked_sub(context.work.used())
                    .ok_or(Failure::Overflow)?,
        )?;
        if charged > remaining.work {
            return Err(Failure::Admission.into());
        }
        context
            .work
            .debit(
                charged,
                context.origin,
                "checked HIR candidate and Verify passes",
            )
            .map_err(|_| Failure::Admission)?;
        Some(plan)
    } else {
        None
    };
    let mut signatures =
        session.reserve::<hir::Signature>(Key::Signatures, program.functions.len())?;
    let mut functions =
        session.reserve::<hir::Function>(Key::Functions, program.functions.len())?;
    let mut cursors = BuilderCursors::default();
    while cursors.function < program.functions.len() {
        let function = &program.functions[cursors.function];
        let row = mapping.row(syntax.function_row(cursors.function)?)?;
        let mut params =
            session.reserve::<hir::Ty>(Key::Parameters(cursors.function), function.params.len())?;
        cursors.parameter = 0;
        cursors.parameter_row = row.b;
        while cursors.parameter < function.params.len() {
            let parameter = mapping.row(cursors.parameter_row)?;
            require(parameter.kind == 2)?;
            params.push(cursors.parameter, mapping.ty(parameter.a)?)?;
            cursors.parameter_row = parameter.next;
            cursors.parameter += 1;
        }
        require(cursors.parameter_row == 0)?;
        let signature = hir::Signature {
            params: params.finish()?,
            result: mapping.ty(row.c)?,
            span: function.name,
        };
        signatures.push(cursors.function, signature)?;
        cursors.function += 1;
    }
    let signatures = signatures.finish()?;
    cursors.function = 0;
    while cursors.function < program.functions.len() {
        let window = mapping.window(cursors.function, cursors.global_expression)?;
        let function = &program.functions[cursors.function];
        let mut locals =
            session.reserve::<hir::Local>(Key::Locals(cursors.function), window.locals)?;
        cursors.local = 0;
        cursors.parameter = 0;
        cursors.row = window.row_begin;
        while cursors.row < window.row_end {
            let reference = small(cursors.row)?;
            let row = mapping.row(reference)?;
            let local = match row.kind {
                2 => {
                    let param = function
                        .params
                        .get(cursors.parameter)
                        .ok_or(Failure::Shape)?;
                    cursors.parameter += 1;
                    Some(hir::Local {
                        mutable: false,
                        span: param.name,
                        annotation: Some(mapping.ty(row.a)?),
                    })
                }
                6 | 7 => Some(hir::Local {
                    mutable: row.kind == 7,
                    span: mapping.name_span(row.a)?,
                    annotation: if row.b == 0 {
                        None
                    } else {
                        Some(mapping.ty(row.b)?)
                    },
                }),
                _ => None,
            };
            if let Some(local) = local {
                require(mapping.declaration(window, reference)?.0 == cursors.local)?;
                locals.push(cursors.local, local)?;
                cursors.local += 1;
            }
            cursors.row += 1;
        }
        require(cursors.local == window.locals && cursors.parameter == function.params.len())?;
        let locals = locals.finish()?;
        let mut expressions = session.reserve::<hir::Expr>(
            Key::Expressions(cursors.function),
            window.expression_end - window.expression_begin,
        )?;
        let mut blocks = session
            .reserve::<hir::BodyBlock>(Key::Blocks(cursors.function), function.blocks.len())?;
        cursors.row = window.row_begin;
        cursors.block = 0;
        while cursors.row < window.row_end {
            let reference = small(cursors.row)?;
            let row = mapping.row(reference)?;
            if row.kind == 5 {
                require(mapping.block(window, reference)?.0 == cursors.block)?;
                let block = function.blocks.get(cursors.block).ok_or(Failure::Shape)?;
                let mut statements = session.reserve::<hir::Stmt>(
                    Key::Statements {
                        function: cursors.function,
                        block: cursors.block,
                    },
                    block.body.len(),
                )?;
                cursors.statement = 0;
                cursors.statement_row = row.b;
                while cursors.statement < block.body.len() {
                    require(window.contains(cursors.statement_row))?;
                    let statement = &block.body[cursors.statement];
                    let row = mapping.row(cursors.statement_row)?;
                    let kind = statement_kind(
                        &mapping,
                        window,
                        cursors.statement_row,
                        row,
                        &statement.kind,
                    )?;
                    statements.push(
                        cursors.statement,
                        hir::Stmt {
                            kind,
                            span: statement.span,
                        },
                    )?;
                    cursors.statement_row = row.next;
                    cursors.statement += 1;
                }
                require(cursors.statement_row == 0)?;
                blocks.push(
                    cursors.block,
                    hir::BodyBlock {
                        body: statements.finish()?,
                        span: block.span,
                        end: block.end,
                    },
                )?;
                cursors.block += 1;
            }
            cursors.row += 1;
        }
        require(cursors.block == function.blocks.len())?;
        let blocks = blocks.finish()?;
        cursors.expression = window.expression_begin;
        while cursors.expression < window.expression_end {
            let expression = &program.expressions[cursors.expression];
            let reference = syntax.expr_row(ast::ExprId(cursors.expression))?;
            require(window.contains(reference))?;
            let kind = expression_kind(
                &mapping,
                window,
                cursors.expression,
                reference,
                &expression.kind,
                &session,
                &mut cursors.argument,
            )?;
            expressions.push(
                cursors.expression - window.expression_begin,
                hir::Expr {
                    kind,
                    span: expression.span,
                },
            )?;
            cursors.expression += 1;
        }
        let expressions = expressions.finish()?;
        let reference = syntax.function_row(cursors.function)?;
        let body = mapping.block(window, mapping.row(reference)?.d)?;
        require(body.0 == function.body.0)?;
        let function = hir::Function {
            id: hir::DefId(mapping.positive(reference)? - 1),
            locals,
            expressions,
            body,
            blocks,
            end: function.end,
        };
        functions.push(cursors.function, function)?;
        cursors.global_expression = window.expression_end;
        cursors.function += 1;
    }
    require(cursors.global_expression == program.expressions.len())?;
    let candidate = hir::Program {
        signatures,
        functions: functions.finish()?,
    };
    let allocation = session.complete()?;
    let equal = same_program(&candidate, canonical);
    // Session owns a canonical borrow and allocator borrow. Release both before
    // the owning input is moved, and before canonical HIR can be dropped.
    #[allow(clippy::drop_non_drop)] // Explicitly end the admitted borrow phase.
    drop(session);
    let charged_work = builder_work
        .checked_add(allocation.helper_work)
        .ok_or(Failure::Overflow)?;
    let facts = ComparisonFacts {
        allocation,
        equal,
        builder_named_bytes,
        builder_work,
        charged_work,
    };
    match input {
        CanonicalInput::Observe(_) => {
            drop(candidate);
            Ok(Completion::Observed(facts))
        }
        CanonicalInput::Verify { canonical, .. } => {
            drop(canonical);
            if !equal {
                drop(candidate);
                return Err(VerifyRejected::HirMismatch(facts));
            }
            verify_terminal::run(syntax, candidate, facts, prepaid.ok_or(Failure::Shape)?)
                .map(Completion::Verified)
        }
    }
}

fn statement_kind(
    mapping: &Mapping<'_, '_, '_, '_>,
    window: FunctionWindow,
    reference: u8,
    row: Row,
    kind: &ast::StmtKind,
) -> Result<hir::StmtKind, Failure> {
    Ok(match kind {
        ast::StmtKind::Let { init, .. } => hir::StmtKind::Let {
            local: mapping.declaration(window, reference)?,
            init: window.expression(*init)?,
        },
        ast::StmtKind::Assign {
            name,
            operator_span,
            value,
        } => hir::StmtKind::Assign {
            local: mapping.local_target(window, reference)?,
            target_span: *name,
            operator_span: *operator_span,
            value: window.expression(*value)?,
        },
        ast::StmtKind::Expr(value) => hir::StmtKind::Expr(window.expression(*value)?),
        ast::StmtKind::Return(value) => hir::StmtKind::Return(match value {
            Some(id) => Some(window.expression(*id)?),
            None => None,
        }),
        ast::StmtKind::Break => hir::StmtKind::Break {
            target: mapping.loop_target(window, reference)?,
        },
        ast::StmtKind::Continue => hir::StmtKind::Continue {
            target: mapping.loop_target(window, reference)?,
        },
        ast::StmtKind::While { condition, body } => {
            let mapped = mapping.block(window, row.b)?;
            require(mapped.0 == body.0)?;
            hir::StmtKind::While {
                loop_id: hir::LoopId(mapped.0),
                condition: window.expression(*condition)?,
                body: mapped,
            }
        }
        ast::StmtKind::If {
            condition,
            then_block,
            else_block,
        } => {
            let then = mapping.block(window, row.b)?;
            require(then.0 == then_block.0)?;
            let otherwise = match else_block {
                Some(block) => {
                    let mapped = mapping.block(window, row.c)?;
                    require(mapped.0 == block.0)?;
                    Some(mapped)
                }
                None => {
                    require(row.c == 0)?;
                    None
                }
            };
            hir::StmtKind::If {
                condition: window.expression(*condition)?,
                then_block: then,
                else_block: otherwise,
            }
        }
        ast::StmtKind::FieldAssign { .. }
        | ast::StmtKind::IndexAssign { .. }
        | ast::StmtKind::Match { .. } => return Err(Failure::Shape),
    })
}

#[allow(clippy::too_many_arguments)]
fn expression_kind(
    mapping: &Mapping<'_, '_, '_, '_>,
    window: FunctionWindow,
    index: usize,
    reference: u8,
    kind: &ast::ExprKind,
    session: &Session<'_, '_>,
    argument: &mut usize,
) -> Result<hir::ExprKind, Failure> {
    Ok(match kind {
        ast::ExprKind::Number { .. } => hir::ExprKind::I32(mapping.resolution(reference)?),
        ast::ExprKind::Bool(value) => hir::ExprKind::Bool(*value),
        ast::ExprKind::Unit => hir::ExprKind::Unit,
        ast::ExprKind::Name(_) => hir::ExprKind::Local(mapping.local_target(window, reference)?),
        ast::ExprKind::Call {
            callee: ast::ItemPath::Unqualified(_),
            args,
        } => {
            let target = mapping.callee(reference)?;
            let mut values = session.reserve::<hir::ExprId>(
                Key::Arguments {
                    function: window.function,
                    expression: window.expression(ast::ExprId(index))?.0,
                },
                args.len(),
            )?;
            *argument = 0;
            while *argument < args.len() {
                let ast::Argument::Value(value) = &args[*argument] else {
                    return Err(Failure::Shape);
                };
                values.push(*argument, window.child(*value, index)?)?;
                *argument += 1;
            }
            hir::ExprKind::Call {
                target,
                args: values.finish()?,
            }
        }
        ast::ExprKind::Group(inner) => hir::ExprKind::Group(window.child(*inner, index)?),
        ast::ExprKind::Negate {
            operand,
            operator_span,
        } => hir::ExprKind::Negate {
            operand: window.child(*operand, index)?,
            operator_span: *operator_span,
        },
        ast::ExprKind::Not {
            operand,
            operator_span,
        } => hir::ExprKind::Not {
            operand: window.child(*operand, index)?,
            operator_span: *operator_span,
        },
        ast::ExprKind::Arithmetic {
            op,
            left,
            right,
            operator_span,
        } => hir::ExprKind::Arithmetic {
            op: *op,
            left: window.child(*left, index)?,
            right: window.child(*right, index)?,
            operator_span: *operator_span,
        },
        ast::ExprKind::Comparison {
            op,
            left,
            right,
            operator_span,
        } => hir::ExprKind::Comparison {
            op: *op,
            left: window.child(*left, index)?,
            right: window.child(*right, index)?,
            operator_span: *operator_span,
        },
        ast::ExprKind::Logical {
            op,
            left,
            right,
            operator_span,
        } => hir::ExprKind::Logical {
            op: *op,
            left: window.child(*left, index)?,
            right: window.child(*right, index)?,
            operator_span: *operator_span,
        },
        ast::ExprKind::Call {
            callee: ast::ItemPath::Absolute(_),
            ..
        }
        | ast::ExprKind::QualifiedValue { .. }
        | ast::ExprKind::StructLiteral { .. }
        | ast::ExprKind::FieldRead { .. }
        | ast::ExprKind::ArrayLiteral { .. }
        | ast::ExprKind::IndexRead { .. }
        | ast::ExprKind::ArrayLength { .. } => return Err(Failure::Shape),
    })
}

// Deliberately explicit aggregate fields and exhaustive candidate-enum matches:
// a HIR field/variant addition must force this oracle to be reviewed. Edges are
// IDs, so equality has fixed helper depth and never recurses over source trees.
fn same_program(a: &hir::Program, b: &hir::Program) -> bool {
    let hir::Program {
        signatures: a_signatures,
        functions: a_functions,
    } = a;
    let hir::Program {
        signatures: b_signatures,
        functions: b_functions,
    } = b;
    if a_signatures.len() != b_signatures.len() || a_functions.len() != b_functions.len() {
        return false;
    }
    let mut index = 0;
    while index < a_signatures.len() {
        if !same_signature(&a_signatures[index], &b_signatures[index]) {
            return false;
        }
        index += 1;
    }
    index = 0;
    while index < a_functions.len() {
        if !same_function(&a_functions[index], &b_functions[index]) {
            return false;
        }
        index += 1;
    }
    true
}
fn same_signature(a: &hir::Signature, b: &hir::Signature) -> bool {
    let hir::Signature {
        params: ap,
        result: ar,
        span: aspan,
    } = a;
    let hir::Signature {
        params: bp,
        result: br,
        span: bspan,
    } = b;
    ap == bp && ar == br && aspan == bspan
}
fn same_function(a: &hir::Function, b: &hir::Function) -> bool {
    let hir::Function {
        id: ai,
        locals: al,
        expressions: ae,
        body: ab,
        blocks: abs,
        end: aend,
    } = a;
    let hir::Function {
        id: bi,
        locals: bl,
        expressions: be,
        body: bb,
        blocks: bbs,
        end: bend,
    } = b;
    if ai != bi
        || ab != bb
        || aend != bend
        || al.len() != bl.len()
        || ae.len() != be.len()
        || abs.len() != bbs.len()
    {
        return false;
    }
    let mut index = 0;
    while index < al.len() {
        if !same_local(&al[index], &bl[index]) {
            return false;
        }
        index += 1;
    }
    index = 0;
    while index < ae.len() {
        if !same_expr(&ae[index], &be[index]) {
            return false;
        }
        index += 1;
    }
    index = 0;
    while index < abs.len() {
        if !same_block(&abs[index], &bbs[index]) {
            return false;
        }
        index += 1;
    }
    true
}
fn same_local(a: &hir::Local, b: &hir::Local) -> bool {
    let hir::Local {
        mutable: am,
        span: aspan,
        annotation: at,
    } = a;
    let hir::Local {
        mutable: bm,
        span: bspan,
        annotation: bt,
    } = b;
    am == bm && aspan == bspan && at == bt
}
fn same_block(a: &hir::BodyBlock, b: &hir::BodyBlock) -> bool {
    let hir::BodyBlock {
        body: ab,
        span: aspan,
        end: ae,
    } = a;
    let hir::BodyBlock {
        body: bb,
        span: bspan,
        end: be,
    } = b;
    if aspan != bspan || ae != be || ab.len() != bb.len() {
        return false;
    }
    let mut index = 0;
    while index < ab.len() {
        if !same_stmt(&ab[index], &bb[index]) {
            return false;
        }
        index += 1;
    }
    true
}
fn same_stmt(a: &hir::Stmt, b: &hir::Stmt) -> bool {
    let hir::Stmt {
        kind: ak,
        span: aspan,
    } = a;
    let hir::Stmt {
        kind: bk,
        span: bspan,
    } = b;
    if aspan != bspan {
        return false;
    }
    match ak {
        hir::StmtKind::Let {
            local: al,
            init: ai,
        } => match bk {
            hir::StmtKind::Let {
                local: bl,
                init: bi,
            } => al == bl && ai == bi,
            _ => false,
        },
        hir::StmtKind::Assign {
            local: al,
            target_span: at,
            operator_span: ao,
            value: av,
        } => match bk {
            hir::StmtKind::Assign {
                local: bl,
                target_span: bt,
                operator_span: bo,
                value: bv,
            } => al == bl && at == bt && ao == bo && av == bv,
            _ => false,
        },
        hir::StmtKind::Expr(av) => match bk {
            hir::StmtKind::Expr(bv) => av == bv,
            _ => false,
        },
        hir::StmtKind::Return(av) => match bk {
            hir::StmtKind::Return(bv) => av == bv,
            _ => false,
        },
        hir::StmtKind::Break { target: at } => match bk {
            hir::StmtKind::Break { target: bt } => at == bt,
            _ => false,
        },
        hir::StmtKind::Continue { target: at } => match bk {
            hir::StmtKind::Continue { target: bt } => at == bt,
            _ => false,
        },
        hir::StmtKind::While {
            loop_id: ai,
            condition: ac,
            body: ab,
        } => match bk {
            hir::StmtKind::While {
                loop_id: bi,
                condition: bc,
                body: bb,
            } => ai == bi && ac == bc && ab == bb,
            _ => false,
        },
        hir::StmtKind::If {
            condition: ac,
            then_block: at,
            else_block: ae,
        } => match bk {
            hir::StmtKind::If {
                condition: bc,
                then_block: bt,
                else_block: be,
            } => ac == bc && at == bt && ae == be,
            _ => false,
        },
    }
}
fn same_expr(a: &hir::Expr, b: &hir::Expr) -> bool {
    let hir::Expr {
        kind: ak,
        span: aspan,
    } = a;
    let hir::Expr {
        kind: bk,
        span: bspan,
    } = b;
    if aspan != bspan {
        return false;
    }
    match ak {
        hir::ExprKind::Negate {
            operand: av,
            operator_span: ao,
        } => match bk {
            hir::ExprKind::Negate {
                operand: bv,
                operator_span: bo,
            } => av == bv && ao == bo,
            _ => false,
        },
        hir::ExprKind::Not {
            operand: av,
            operator_span: ao,
        } => match bk {
            hir::ExprKind::Not {
                operand: bv,
                operator_span: bo,
            } => av == bv && ao == bo,
            _ => false,
        },
        hir::ExprKind::Logical {
            op: ao,
            left: al,
            right: ar,
            operator_span: asp,
        } => match bk {
            hir::ExprKind::Logical {
                op: bo,
                left: bl,
                right: br,
                operator_span: bsp,
            } => ao == bo && al == bl && ar == br && asp == bsp,
            _ => false,
        },
        hir::ExprKind::Comparison {
            op: ao,
            left: al,
            right: ar,
            operator_span: asp,
        } => match bk {
            hir::ExprKind::Comparison {
                op: bo,
                left: bl,
                right: br,
                operator_span: bsp,
            } => ao == bo && al == bl && ar == br && asp == bsp,
            _ => false,
        },
        hir::ExprKind::Arithmetic {
            op: ao,
            left: al,
            right: ar,
            operator_span: asp,
        } => match bk {
            hir::ExprKind::Arithmetic {
                op: bo,
                left: bl,
                right: br,
                operator_span: bsp,
            } => ao == bo && al == bl && ar == br && asp == bsp,
            _ => false,
        },
        hir::ExprKind::Bool(av) => match bk {
            hir::ExprKind::Bool(bv) => av == bv,
            _ => false,
        },
        hir::ExprKind::I32(av) => match bk {
            hir::ExprKind::I32(bv) => av == bv,
            _ => false,
        },
        hir::ExprKind::Unit => matches!(bk, hir::ExprKind::Unit),
        hir::ExprKind::Local(av) => match bk {
            hir::ExprKind::Local(bv) => av == bv,
            _ => false,
        },
        hir::ExprKind::Group(av) => match bk {
            hir::ExprKind::Group(bv) => av == bv,
            _ => false,
        },
        hir::ExprKind::Call {
            target: at,
            args: aa,
        } => match bk {
            hir::ExprKind::Call {
                target: bt,
                args: ba,
            } => at == bt && aa == ba,
            _ => false,
        },
    }
}

fn copies<T>(count: usize) -> Result<usize, Failure> {
    size_of::<T>().checked_mul(count).ok_or(Failure::Overflow)
}
fn sum<const N: usize>(values: [usize; N]) -> Result<usize, Failure> {
    values.into_iter().try_fold(0usize, |total, value| {
        total.checked_add(value).ok_or(Failure::Overflow)
    })
}

/// Measured named carrier envelope, not compiler-frame/stack or RSS accounting.
/// The caller supplies source owner/binding/comparator banks, canonical owner,
/// full Allocator owner, outer resolver/rejection/results and owned baseline B.
/// Session supplies all eight reserve/push/finish chains: its single held caller
/// ExactVec<T> and final caller Vec<T> roles match this construction exactly.
/// There is at most one caller wrapper of each T, including held Function+Expr+
/// BodyBlock wrappers while a Stmt vector is filled. No such wrapper is added a
/// second time here; nested in-slot Vec headers are already charged in Hn/Hc.
/// Rows constructed locally and their helper transport are separate named roles.
pub(super) fn builder_named_bytes() -> Result<usize, Failure> {
    let roles = [
        // Complete common dispatch/input/result carriers, including the owned
        // canonical alternative even for the preserved Observe entry. These
        // are transport roles additional to the enclosing Program owners.
        size_of::<(
            &ComparedSyntax<'_, '_, '_>,
            CanonicalInput<'_, '_>,
            &mut Allocator,
            usize,
            IndexLimits,
        )>(),
        copies::<CanonicalInput<'_, '_>>(2)?,
        size_of::<&CanonicalInput<'_, '_>>(),
        size_of::<&hir::Program>(),
        copies::<Completion>(2)?,
        copies::<VerifyRejected>(2)?,
        copies::<Result<Completion, VerifyRejected>>(3)?,
        size_of::<Option<verify_terminal::WorkPlan>>(),
        // Entry arguments, mapping construction/caller and source AST borrow.
        size_of::<(
            &ComparedSyntax<'_, '_, '_>,
            &hir::Program,
            &mut Allocator,
            usize,
            IndexLimits,
        )>(),
        copies::<Mapping<'_, '_, '_, '_>>(2)?,
        size_of::<&ast::Program>(),
        size_of::<Result<&ast::Program, Box<crate::frontend::diagnostic::Diagnostic>>>(),
        // Candidate owner once; canonical owner belongs to outside_fixed_bytes.
        size_of::<hir::Program>(),
        size_of::<ComparisonFacts>(),
        copies::<Result<ComparisonFacts, Failure>>(2)?,
        size_of::<Receipt>(),
        copies::<IndexLimits>(2)?,
        // compare and validate have separate cursor banks. window has its own
        // local/input/return/caller roles, with checked range iterator carriers.
        copies::<BuilderCursors>(2)?,
        copies::<FunctionWindow>(3)?,
        size_of::<Result<FunctionWindow, Failure>>(),
        size_of::<(&Mapping<'_, '_, '_, '_>, usize, usize)>(),
        copies::<usize>(8)?,
        copies::<std::ops::Range<usize>>(3)?,
        // Source borrows and simultaneous row construction/read transports.
        size_of::<(
            &ast::Function,
            &ast::Param,
            &ast::BodyBlock,
            &ast::Stmt,
            &ast::Expr,
            &ast::Argument,
            &Token,
        )>(),
        copies::<Row>(4)?,
        copies::<Result<Row, Failure>>(3)?,
        size_of::<Result<Row, super::Boundary>>(),
        // Mapping helper inputs along callee -> target -> positive -> resolution
        // and block/local target chains. These are scalar/reference roles only.
        copies::<(&Mapping<'_, '_, '_, '_>, u8)>(5)?,
        copies::<(&Mapping<'_, '_, '_, '_>, FunctionWindow, u8)>(3)?,
        size_of::<(FunctionWindow, ast::ExprId, usize)>(),
        // Distinct nested window receiver inputs and compared-ID transports;
        // these are additional to the window-construction/caller copies above.
        size_of::<(FunctionWindow, ast::ExprId)>(),
        size_of::<(FunctionWindow, u8)>(),
        size_of::<(&ComparedSyntax<'_, '_, '_>, ast::ExprId)>(),
        size_of::<(&ComparedSyntax<'_, '_, '_>, usize)>(),
        copies::<Result<u8, super::Boundary>>(2)?,
        size_of::<(
            &Mapping<'_, '_, '_, '_>,
            FunctionWindow,
            u8,
            Row,
            &ast::StmtKind,
        )>(),
        size_of::<(
            &Mapping<'_, '_, '_, '_>,
            FunctionWindow,
            usize,
            u8,
            &ast::ExprKind,
            &Session<'_, '_>,
            &mut usize,
        )>(),
        // Fixed decoding values/returns/caller roles, including the exact
        // Row::read / Wire::word byte, word and arithmetic carrier banks.
        size_of::<(super::Wire<'_>, u8, usize, u32, u32, u32, u8, u32, [u32; 4])>(),
        size_of::<(
            super::Wire<'_>,
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
        copies::<Result<i32, super::Boundary>>(2)?,
        copies::<Result<i32, Failure>>(2)?,
        copies::<Result<usize, Failure>>(5)?,
        copies::<Result<u8, Failure>>(3)?,
        size_of::<Result<(usize, ast::BodyBlockId), super::Boundary>>(),
        size_of::<(usize, ast::BodyBlockId)>(),
        copies::<Result<hir::LocalId, Failure>>(3)?,
        copies::<Result<hir::BodyBlockId, Failure>>(3)?,
        copies::<Result<hir::LoopId, Failure>>(2)?,
        copies::<Result<hir::DefId, Failure>>(2)?,
        copies::<Result<hir::ExprId, Failure>>(3)?,
        copies::<Result<hir::Ty, Failure>>(2)?,
        copies::<Result<Span, Failure>>(2)?,
        copies::<Result<(), Failure>>(12)?,
        copies::<usize>(8)?,
        copies::<u8>(6)?,
        copies::<i32>(2)?,
        size_of::<Span>(),
        // Actual row locals/transfers. push's own by-value T is in Session.
        // ExprKind/StmtKind construction and helper Result+caller are separate
        // from the enclosing Expr/Stmt being transferred to the exact wrapper.
        size_of::<hir::Signature>(),
        size_of::<hir::Function>(),
        size_of::<Option<hir::Local>>(),
        size_of::<hir::Local>(),
        size_of::<hir::BodyBlock>(),
        size_of::<hir::Stmt>(),
        copies::<hir::StmtKind>(2)?,
        size_of::<Result<hir::StmtKind, Failure>>(),
        size_of::<hir::Expr>(),
        copies::<hir::ExprKind>(2)?,
        size_of::<Result<hir::ExprKind, Failure>>(),
        size_of::<(
            hir::Ty,
            Option<hir::Ty>,
            hir::ExprId,
            Option<hir::ExprId>,
            hir::LocalId,
            hir::DefId,
            hir::BodyBlockId,
            Option<hir::BodyBlockId>,
            hir::LoopId,
        )>(),
        // Seven concrete equality helper input/destructuring/index/result banks.
        // Distinct enum branch field bindings are conservatively summed below.
        size_of::<(
            &hir::Program,
            &hir::Program,
            &Vec<hir::Signature>,
            &Vec<hir::Signature>,
            &Vec<hir::Function>,
            &Vec<hir::Function>,
            usize,
            bool,
        )>(),
        size_of::<(
            &hir::Signature,
            &hir::Signature,
            &Vec<hir::Ty>,
            &Vec<hir::Ty>,
            &hir::Ty,
            &hir::Ty,
            &Span,
            &Span,
            bool,
        )>(),
        size_of::<(
            &hir::Function,
            &hir::Function,
            &hir::DefId,
            &hir::DefId,
            &Vec<hir::Local>,
            &Vec<hir::Local>,
            &Vec<hir::Expr>,
            &Vec<hir::Expr>,
            &hir::BodyBlockId,
            &hir::BodyBlockId,
            &Vec<hir::BodyBlock>,
            &Vec<hir::BodyBlock>,
            &Span,
            &Span,
            usize,
            bool,
        )>(),
        size_of::<(
            &hir::Local,
            &hir::Local,
            &bool,
            &bool,
            &Span,
            &Span,
            &Option<hir::Ty>,
            &Option<hir::Ty>,
            bool,
        )>(),
        size_of::<(
            &hir::BodyBlock,
            &hir::BodyBlock,
            &Vec<hir::Stmt>,
            &Vec<hir::Stmt>,
            &Span,
            &Span,
            &Span,
            &Span,
            usize,
            bool,
        )>(),
        size_of::<(
            &hir::Stmt,
            &hir::Stmt,
            &hir::StmtKind,
            &hir::StmtKind,
            &Span,
            &Span,
            bool,
        )>(),
        size_of::<(
            &hir::Expr,
            &hir::Expr,
            &hir::ExprKind,
            &hir::ExprKind,
            &Span,
            &Span,
            bool,
        )>(),
        copies::<(&hir::LocalId, &hir::ExprId, &Span, &Span)>(2)?,
        copies::<(
            &Option<hir::ExprId>,
            &hir::LoopId,
            &hir::ExprId,
            &hir::BodyBlockId,
            &Option<hir::BodyBlockId>,
        )>(2)?,
        copies::<(
            &hir::ExprId,
            &Span,
            &hir::LogicalOp,
            &hir::ComparisonOp,
            &hir::ArithmeticOp,
            &hir::ExprId,
            &hir::ExprId,
            &Span,
        )>(2)?,
        copies::<(&bool, &i32, &hir::LocalId, &hir::DefId, &Vec<hir::ExprId>)>(2)?,
        // Primitive/slice equality inputs and length/index/scalar result roles.
        size_of::<(
            &[hir::Ty],
            &[hir::Ty],
            &[hir::ExprId],
            &[hir::ExprId],
            usize,
            usize,
        )>(),
        copies::<(&Span, &Span, usize, bool)>(3)?,
        copies::<bool>(7)?,
        // Work/size calculation arithmetic, arrays, sum input/iterator and full
        // checked results. The main named-cost array is charged just below.
        copies::<[u64; 9]>(2)?,
        size_of::<std::array::IntoIter<u64, 9>>(),
        copies::<usize>(6)?,
        copies::<u64>(6)?,
        copies::<Result<u64, Failure>>(3)?,
        copies::<Result<usize, Failure>>(4)?,
    ];
    let bank = size_of_val(&roles)
        .checked_mul(2)
        .and_then(|n| n.checked_add(size_of_val(&roles.into_iter())))
        .ok_or(Failure::Overflow)?;
    sum(roles)?.checked_add(bank).ok_or(Failure::Overflow)
}

/// Only the closed Verify source plan adds this complete downstream bank.
pub(super) fn verify_named_bytes() -> Result<usize, Failure> {
    let roles = [
        verify_terminal::named_bytes()?,
        size_of::<(
            &ComparedSyntax<'_, '_, '_>,
            hir::Program,
            &mut Allocator,
            usize,
            IndexLimits,
            &WorkMeter,
            Span,
        )>(),
        copies::<Result<VerifyFacts, VerifyRejected>>(2)?,
        size_of::<verify_terminal::Context<'_>>(),
        size_of::<verify_terminal::WorkPlan>(),
        size_of::<&verify_terminal::Context<'_>>(),
        copies::<u64>(3)?,
    ];
    let bank = size_of_val(&roles)
        .checked_mul(2)
        .and_then(|n| n.checked_add(size_of_val(&roles.into_iter())))
        .ok_or(Failure::Overflow)?;
    sum(roles)?.checked_add(bank).ok_or(Failure::Overflow)
}

/// A deterministic bookkeeping bound, not elapsed time or CPU instructions.
/// All loops below are linear in the already compared <=128-row syntax. The
/// nine terms cover: two function/expression-window passes; two declaration
/// scans; complete resolution-role validation; local fill; block scans; signature
/// parameters; statements; expression+argument construction; and equality's
/// signature/function/local/block/statement/expression/parameter/argument visits.
/// Every category's objects are row-bounded; equality duplicates function/let
/// roles and arguments, so four row passes cover its complete scalar HIR walk.
/// Each term includes one extra visit for its terminal test. 256 units per visit
/// conservatively cover nested checked row/word/ID reads, branches and fixed
/// field checks; 512 units cover entry, checked cost-bank summation and receipts.
/// Session separately charges all shape rereads, reserves, pushes and finishes.
fn work_bound(rows: usize) -> Result<u64, Failure> {
    require(rows <= MAX_ROWS)?;
    let passes = [2u64, 2, 1, 1, 1, 1, 1, 2, 4];
    let passes = passes
        .into_iter()
        .try_fold(0u64, |sum, n| sum.checked_add(n).ok_or(Failure::Overflow))?;
    u64::try_from(rows)
        .map_err(|_| Failure::Overflow)?
        .checked_add(1)
        .and_then(|n| n.checked_mul(passes))
        .and_then(|n| n.checked_mul(256))
        .and_then(|n| n.checked_add(512))
        .ok_or(Failure::Overflow)
}

#[cfg(test)]
mod tests;
