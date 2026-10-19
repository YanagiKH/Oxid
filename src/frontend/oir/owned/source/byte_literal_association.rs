//! Independent original-source proof for nonempty byte-constant bundles.
//! Empty byte literals use ordinary zero-length arrays and carry no scalar claim.
use super::*;
use crate::frontend::{ast, byte_literal, declaration_index::WorkMeter, project::ModuleId};
use std::mem::size_of;

const UNOWNED: usize = usize::MAX;

#[derive(Clone, Copy)]
enum WalkNode {
    Expression(ast::ExprId),
    Block(ast::BodyBlockId),
}

/// Function membership and replay are independent state. Neither table is a
/// transferable permit: both die before the exact owned raw wrapper is returned.
pub(super) struct LiteralProof<'w> {
    offsets: Vec<usize>,
    owners: Vec<usize>,
    replay: Vec<u8>,
    stack: Vec<WalkNode>,
    blocks: Vec<u8>,
    work: &'w WorkMeter,
    proven: usize,
}

#[derive(Clone, Copy)]
struct Plan {
    modules: usize,
    expressions: usize,
    stack: usize,
    blocks: usize,
}

fn add(a: usize, b: usize) -> Result<usize, Box<Diagnostic>> {
    a.checked_add(b).ok_or_else(bad)
}
fn bytes(n: usize, width: usize) -> Result<usize, Box<Diagnostic>> {
    n.checked_mul(width).ok_or_else(bad)
}
fn debit(work: &WorkMeter, amount: usize, at: Span) -> Result<(), Box<Diagnostic>> {
    work.debit(
        u64::try_from(amount).map_err(|_| bad())?,
        at,
        "byte literal association",
    )
}
fn step(work: &WorkMeter, visitor: &mut Visitor<'_>, at: Span) -> Result<(), Box<Diagnostic>> {
    debit(work, 1, at)?;
    visitor.dimension(1, 1)
}

impl Plan {
    fn count(
        index: &DeclarationIndex<'_>,
        work: &WorkMeter,
        visitor: &mut Visitor<'_>,
    ) -> Result<Self, Box<Diagnostic>> {
        let mut plan = Self {
            modules: index.sources().count(),
            expressions: 0,
            stack: 0,
            blocks: 0,
        };
        for module in 0..plan.modules {
            step(work, visitor, index.sources().eof())?;
            let ast = index
                .sources()
                .try_ast_borrowed(ModuleId(module))
                .ok_or_else(bad)?;
            plan.expressions = add(plan.expressions, ast.expressions.len())?;
            for owner in &ast.functions {
                step(work, visitor, owner.name)?;
                plan.blocks = plan.blocks.max(owner.blocks.len());
                plan.stack = plan
                    .stack
                    .max(add(ast.expressions.len(), owner.blocks.len())?);
            }
        }
        Ok(plan)
    }
    fn payload(self) -> Result<usize, Box<Diagnostic>> {
        let mut total = bytes(add(self.modules, 1)?, size_of::<usize>())?;
        total = add(total, bytes(self.expressions, size_of::<usize>())?)?;
        total = add(total, bytes(self.expressions, size_of::<u8>())?)?;
        total = add(total, bytes(self.stack, size_of::<WalkNode>())?)?;
        add(total, bytes(self.blocks, size_of::<u8>())?)
    }
}

impl<'w> LiteralProof<'w> {
    pub(super) fn new(
        typed: &'w super::super::typeck::TypedOwnedProgram<'_>,
        sources: &SourceMap,
        visitor: &mut Visitor<'_>,
        conversions: &ConversionSeen,
        seed: usize,
    ) -> Result<Self, Box<Diagnostic>> {
        // Imported/experimental/candidate producers do not gain a scalar
        // source authority merely by supplying the same public-looking fields.
        if !typed.admission().executable() {
            return Err(bad());
        }
        let index = typed.index();
        index.require_current_source_pipeline()?;
        let work = typed.work();
        let plan = Plan::count(index, work, visitor)?;
        let fixed = add(carrier_bytes(), conversions.modeled_bytes()?)?;
        let requested = plan.payload()?;
        let admit = |payload| {
            super::super::budget::admit_conversion_scratch(seed, add(fixed, payload)?)
                .map(|_| ())
                .map_err(|error| super::super::diagnostic::lower(&error, sources))
        };
        admit(requested)?;
        // Every request goes through the established fallible source allocator.
        // Account any actual excess capacity before initialization or traversal.
        let mut excess = 0usize;
        macro_rules! reserve {
            ($count:expr, $ty:ty) => {{
                let count = $count;
                let values = super::super::budget::reserve::<$ty>(count)
                    .map_err(|error| super::super::diagnostic::lower(&error, sources))?;
                let extra = values.capacity().checked_sub(count).ok_or_else(bad)?;
                excess = add(excess, bytes(extra, size_of::<$ty>())?)?;
                admit(add(requested, excess)?)?;
                values
            }};
        }
        let offsets = reserve!(add(plan.modules, 1)?, usize);
        let mut owners = reserve!(plan.expressions, usize);
        let mut replay = reserve!(plan.expressions, u8);
        let stack = reserve!(plan.stack, WalkNode);
        let mut blocks = reserve!(plan.blocks, u8);
        debit(
            work,
            add(add(plan.expressions, plan.expressions)?, plan.blocks)?,
            index.sources().eof(),
        )?;
        owners.resize(plan.expressions, UNOWNED);
        replay.resize(plan.expressions, 0);
        blocks.resize(plan.blocks, 0);
        let mut proof = Self {
            offsets,
            owners,
            replay,
            stack,
            blocks,
            work,
            proven: 0,
        };
        let mut offset = 0usize;
        for module in 0..plan.modules {
            step(work, visitor, index.sources().eof())?;
            proof.offsets.push(offset);
            let ast = index
                .sources()
                .try_ast_borrowed(ModuleId(module))
                .ok_or_else(bad)?;
            for (ordinal, owner) in ast.functions.iter().enumerate() {
                step(work, visitor, owner.name)?;
                let source = sources.files().get(owner.name.file.0).ok_or_else(bad)?;
                if !ast.belongs_to(source) {
                    return Err(bad());
                }
                proof.walk(ast, owner, ordinal, offset, visitor)?;
            }
            offset = add(offset, ast.expressions.len())?;
        }
        proof.offsets.push(offset);
        visitor.dimension(offset, plan.expressions)?;
        Ok(proof)
    }

    fn push(&mut self, node: WalkNode) -> Result<(), Box<Diagnostic>> {
        // This is a pre-reserved worklist, never an implicit growable stack.
        if self.stack.len() == self.stack.capacity() {
            return Err(bad());
        }
        self.stack.push(node);
        Ok(())
    }
    fn expression(
        &mut self,
        id: ast::ExprId,
        ast: &ast::Program,
        owner: usize,
        offset: usize,
        visitor: &mut Visitor<'_>,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        step(self.work, visitor, at)?;
        if ast.expressions.get(id.0).is_none() {
            return Err(bad());
        }
        let member = self.owners.get_mut(add(offset, id.0)?).ok_or_else(bad)?;
        // Duplicate edges, cross-function reuse and cycles are all rejected.
        if *member != UNOWNED {
            return Err(bad());
        }
        *member = owner;
        self.push(WalkNode::Expression(id))
    }
    fn block(
        &mut self,
        id: ast::BodyBlockId,
        owner: &ast::Function,
        visitor: &mut Visitor<'_>,
    ) -> Result<(), Box<Diagnostic>> {
        step(self.work, visitor, owner.name)?;
        if owner.blocks.get(id.0).is_none() {
            return Err(bad());
        }
        let state = self.blocks.get_mut(id.0).ok_or_else(bad)?;
        if *state != 0 {
            return Err(bad());
        }
        *state = 1;
        self.push(WalkNode::Block(id))
    }
    fn arguments(
        &mut self,
        arguments: &[ast::Argument],
        ast: &ast::Program,
        owner: usize,
        offset: usize,
        visitor: &mut Visitor<'_>,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        for argument in arguments {
            step(self.work, visitor, at)?;
            match argument {
                ast::Argument::Value(id) => {
                    self.expression(*id, ast, owner, offset, visitor, at)?
                }
                ast::Argument::Borrow {
                    place: ast::BorrowPlace::Range { start, end, .. },
                    ..
                } => {
                    self.expression(*start, ast, owner, offset, visitor, at)?;
                    self.expression(*end, ast, owner, offset, visitor, at)?;
                }
                ast::Argument::Borrow { .. } => (),
            }
        }
        Ok(())
    }
    fn walk(
        &mut self,
        ast: &ast::Program,
        owner: &ast::Function,
        ordinal: usize,
        offset: usize,
        visitor: &mut Visitor<'_>,
    ) -> Result<(), Box<Diagnostic>> {
        debit(self.work, owner.blocks.len(), owner.name)?;
        self.blocks
            .get_mut(..owner.blocks.len())
            .ok_or_else(bad)?
            .fill(0);
        if !self.stack.is_empty() {
            return Err(bad());
        }
        self.block(owner.body, owner, visitor)?;
        while let Some(node) = self.stack.pop() {
            step(self.work, visitor, owner.name)?;
            match node {
                WalkNode::Block(id) => {
                    let block = owner.blocks.get(id.0).ok_or_else(bad)?;
                    for statement in &block.body {
                        step(self.work, visitor, statement.span)?;
                        match &statement.kind {
                            ast::StmtKind::Let { init: value, .. }
                            | ast::StmtKind::Assign { value, .. }
                            | ast::StmtKind::FieldAssign { value, .. }
                            | ast::StmtKind::Expr(value) => {
                                self.expression(
                                    *value,
                                    ast,
                                    ordinal,
                                    offset,
                                    visitor,
                                    statement.span,
                                )?;
                            }
                            ast::StmtKind::IndexAssign { target, value, .. } => {
                                self.expression(
                                    *target,
                                    ast,
                                    ordinal,
                                    offset,
                                    visitor,
                                    statement.span,
                                )?;
                                self.expression(
                                    *value,
                                    ast,
                                    ordinal,
                                    offset,
                                    visitor,
                                    statement.span,
                                )?;
                            }
                            ast::StmtKind::Return(value) => {
                                if let Some(value) = value {
                                    self.expression(
                                        *value,
                                        ast,
                                        ordinal,
                                        offset,
                                        visitor,
                                        statement.span,
                                    )?;
                                }
                            }
                            ast::StmtKind::While { condition, body } => {
                                self.expression(
                                    *condition,
                                    ast,
                                    ordinal,
                                    offset,
                                    visitor,
                                    statement.span,
                                )?;
                                self.block(*body, owner, visitor)?;
                            }
                            ast::StmtKind::If {
                                condition,
                                then_block,
                                else_block,
                            } => {
                                self.expression(
                                    *condition,
                                    ast,
                                    ordinal,
                                    offset,
                                    visitor,
                                    statement.span,
                                )?;
                                self.block(*then_block, owner, visitor)?;
                                if let Some(body) = else_block {
                                    self.block(*body, owner, visitor)?;
                                }
                            }
                            ast::StmtKind::Match { arms, .. } => {
                                for arm in arms {
                                    step(self.work, visitor, arm.span)?;
                                    self.block(arm.body, owner, visitor)?;
                                }
                            }
                            ast::StmtKind::Break | ast::StmtKind::Continue => (),
                        }
                    }
                }
                WalkNode::Expression(id) => {
                    let expression = ast.expressions.get(id.0).ok_or_else(bad)?;
                    let at = expression.span;
                    match &expression.kind {
                        ast::ExprKind::Conversion { operand, .. }
                        | ast::ExprKind::Negate { operand, .. }
                        | ast::ExprKind::Not { operand, .. }
                        | ast::ExprKind::Group(operand)
                        | ast::ExprKind::IndexRead { index: operand, .. } => {
                            self.expression(*operand, ast, ordinal, offset, visitor, at)?;
                        }
                        ast::ExprKind::Logical { left, right, .. }
                        | ast::ExprKind::Comparison { left, right, .. }
                        | ast::ExprKind::Arithmetic { left, right, .. } => {
                            self.expression(*left, ast, ordinal, offset, visitor, at)?;
                            self.expression(*right, ast, ordinal, offset, visitor, at)?;
                        }
                        ast::ExprKind::Call { args, .. } => {
                            self.arguments(args, ast, ordinal, offset, visitor, at)?
                        }
                        ast::ExprKind::QualifiedValue { args, .. } => {
                            if let Some(args) = args {
                                self.arguments(args, ast, ordinal, offset, visitor, at)?;
                            }
                        }
                        ast::ExprKind::StructLiteral { fields, .. } => {
                            for field in fields {
                                step(self.work, visitor, field.span)?;
                                self.expression(
                                    field.value,
                                    ast,
                                    ordinal,
                                    offset,
                                    visitor,
                                    field.span,
                                )?;
                            }
                        }
                        ast::ExprKind::ArrayLiteral { elements } => {
                            for element in elements {
                                self.expression(*element, ast, ordinal, offset, visitor, at)?;
                            }
                        }
                        ast::ExprKind::ByteLiteral { .. }
                        | ast::ExprKind::Bool(_)
                        | ast::ExprKind::Number { .. }
                        | ast::ExprKind::Unit
                        | ast::ExprKind::Name(_)
                        | ast::ExprKind::FieldRead { .. }
                        | ast::ExprKind::ArrayLength { .. } => (),
                    }
                }
            }
        }
        Ok(())
    }

    /// Return the exclusive end of the proven bundle. The caller still visits
    /// every raw statement, and requires every ByteConst to be in this interval.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn bundle(
        &mut self,
        function: &RawOwnedFunction,
        block: &OwnedBlock,
        start: usize,
        ast: &ast::Program,
        owner: &ast::Function,
        sources: &SourceMap,
        module: usize,
        visitor: &mut Visitor<'_>,
    ) -> Result<usize, Box<Diagnostic>> {
        step(self.work, visitor, owner.name)?;
        let first = block.statements.get(start).ok_or_else(bad)?;
        let OwnedInstruction::Scalar(Statement::Assign(Assign {
            value:
                Rvalue::ByteConst {
                    source_expr,
                    ordinal: 0,
                    ..
                },
            ..
        })) = &first.kind
        else {
            return Err(bad());
        };
        let source_expr = *source_expr;
        let offset = *self.offsets.get(module).ok_or_else(bad)?;
        let end = *self.offsets.get(add(module, 1)?).ok_or_else(bad)?;
        let slot = add(offset, source_expr.0)?;
        if slot >= end {
            return Err(bad());
        }
        let original = ast.expressions.get(source_expr.0).ok_or_else(bad)?;
        let ast::ExprKind::ByteLiteral { decoded_len } = &original.kind else {
            return Err(bad());
        };
        let length = *decoded_len;
        if !(1..=1024).contains(&length) {
            return Err(bad());
        }
        let literal = original.span;
        let source = sources.files().get(literal.file.0).ok_or_else(bad)?;
        step(self.work, visitor, literal)?;
        let body = owner.blocks.get(owner.body.0).ok_or_else(bad)?;
        if literal.file != owner.name.file
            || body.span.file != literal.file
            || body.span.start > literal.start
            || literal.end > owner.end.end
            || !ast.belongs_to(source)
        {
            return Err(bad());
        }
        let owner_ordinal = *self.owners.get(slot).ok_or_else(bad)?;
        let structural_owner = ast.functions.get(owner_ordinal).ok_or_else(bad)?;
        if !std::ptr::eq(structural_owner, owner) {
            return Err(bad());
        }
        if *self.replay.get(slot).ok_or_else(bad)? != 0 {
            return Err(bad());
        }

        let live_position = add(start, length)?;
        let construct_position = add(live_position, 1)?;
        let finish = add(construct_position, 1)?;
        let live = block.statements.get(live_position).ok_or_else(bad)?;
        let construct = block.statements.get(construct_position).ok_or_else(bad)?;
        let OwnedInstruction::ConstructArray {
            destination,
            elements,
        } = &construct.kind
        else {
            return Err(bad());
        };
        let OwnedInstruction::StorageLive(live_destination) = &live.kind else {
            return Err(bad());
        };
        let declaration = function.owners.get(destination.0).ok_or_else(bad)?;
        let AggregateTy::FixedArray(array) = declaration.aggregate() else {
            return Err(bad());
        };
        step(self.work, visitor, literal)?;
        if live_destination != destination
            || declaration.kind != OwnerKind::Temporary
            || declaration.span != literal
            || array.element() != hir::Ty::U8
            || array.length() != length
            || elements.len() != length
            || live.span != literal
            || construct.span != literal
        {
            return Err(bad());
        }
        let Some(origins) = construct.diagnostic_origins else {
            return Err(bad());
        };
        if origins.primary != literal || live.diagnostic_origins != Some(origins) {
            return Err(bad());
        }

        // Independently debit the scanner envelope, suffix and atom inspections.
        debit(
            self.work,
            byte_literal::scan_work(literal).ok_or_else(bad)?,
            literal,
        )?;
        let mut scanner = byte_literal::Scanner::new(source, literal).map_err(|_| bad())?;
        for ordinal in 0..length {
            step(self.work, visitor, literal)?;
            let atom = scanner.next_atom().map_err(|_| bad())?.ok_or_else(bad)?;
            let instruction = block.statements.get(add(start, ordinal)?).ok_or_else(bad)?;
            let OwnedInstruction::Scalar(Statement::Assign(assign)) = &instruction.kind else {
                return Err(bad());
            };
            let Rvalue::ByteConst {
                value,
                source_expr: claimed,
                ordinal: claimed_ordinal,
            } = &assign.value
            else {
                return Err(bad());
            };
            let local = function.locals.get(assign.destination.0).ok_or_else(bad)?;
            let operand = elements.get(ordinal).ok_or_else(bad)?;
            let expected_origins = Some(DiagnosticOrigins {
                primary: atom.origin,
                cause: origins.cause,
            });
            step(self.work, visitor, atom.origin)?;
            if *value != atom.value
                || *claimed != source_expr
                || *claimed_ordinal != ordinal
                || assign.span != atom.origin
                || instruction.span != atom.origin
                || instruction.diagnostic_origins != expected_origins
                || local.ty != hir::Ty::U8
                || local.kind != LocalKind::Temporary
                || local.span != atom.origin
                || operand.local != assign.destination
                || operand.span != atom.origin
            {
                return Err(bad());
            }
            // Distinct nonempty atom spans plus exact declaration-span equality
            // imply distinct destination locals without a quadratic pair walk.
        }
        step(self.work, visitor, literal)?;
        if scanner.next_atom().map_err(|_| bad())?.is_some() {
            return Err(bad());
        }
        *self.replay.get_mut(slot).ok_or_else(bad)? = 1;
        self.proven = add(self.proven, length)?;
        Ok(finish)
    }

    pub(super) fn finish(&self, raw_constants: usize) -> Result<(), Box<Diagnostic>> {
        if self.proven == raw_constants {
            Ok(())
        } else {
            Err(bad())
        }
    }
}

/// Always-live additions to the existing count/validation caller. They are
/// admitted even on the zero-constant route, before either raw pass begins.
#[allow(dead_code, clippy::type_complexity)]
struct LiteralControlCarriers {
    count: usize,
    validation_count: usize,
    seeds: [usize; 2],
    seed_result: Result<usize, Box<Diagnostic>>,
    admission_result: Result<usize, OwnedFailure>,
    typed: Option<&'static super::super::typeck::TypedOwnedProgram<'static>>,
    proof: Option<LiteralProof<'static>>,
    constructed_proof: Result<LiteralProof<'static>, Box<Diagnostic>>,
    count_empty_proof: Option<LiteralProof<'static>>,
    coverage_cursor: usize,
    proof_argument: &'static Option<LiteralProof<'static>>,
    count_argument: &'static usize,
    work_roles: [Option<(&'static WorkMeter, Span)>; 4],
    work_result: Result<(), Box<Diagnostic>>,
    discriminator: bool,
    count_increment: Option<usize>,
    checked_count: Result<usize, Box<Diagnostic>>,
    source_match: FunctionSource<'static>,
    bundle_result: Result<usize, Box<Diagnostic>>,
    proof_borrow: Option<&'static LiteralProof<'static>>,
    eof_result: Span,
}
pub(super) const fn control_bytes() -> usize {
    size_of::<LiteralControlCarriers>()
        + crate::frontend::oir::source::association::association_error_bytes()
}

type ReservationResults = (
    Result<Vec<usize>, OwnedFailure>,
    Result<Vec<u8>, OwnedFailure>,
    Result<Vec<WalkNode>, OwnedFailure>,
);

/// Explicit transport inventory, separate from requested payload capacities.
/// It includes count/validation/control, reservation/error and AST-walk roles;
/// no credit is taken for compiler stack-slot reuse or elided copies.
#[allow(dead_code)]
struct LiteralCarriers {
    proofs: [LiteralProof<'static>; 2],
    proof_result: Result<LiteralProof<'static>, Box<Diagnostic>>,
    plans: [Plan; 3],
    plan_result: Result<Plan, Box<Diagnostic>>,
    typed: &'static super::super::typeck::TypedOwnedProgram<'static>,
    index: &'static DeclarationIndex<'static>,
    sources: &'static SourceMap,
    visitor: &'static Visitor<'static>,
    conversions: &'static ConversionSeen,
    seed: usize,
    fixed: usize,
    requested: usize,
    excess: usize,
    admission_closure: (usize, usize, &'static SourceMap),
    reservation_headers: (Vec<usize>, Vec<u8>, Vec<WalkNode>),
    reservation_results: ReservationResults,
    arithmetic_results: [Result<usize, Box<Diagnostic>>; 8],
    unit_results: [Result<(), Box<Diagnostic>>; 8],
    raw_counts: [usize; 3],
    coverage_cursors: [usize; 2],
    walk_arguments: (&'static ast::Program, &'static ast::Function, usize, usize),
    expression_arguments: (ast::ExprId, &'static ast::Program, usize, usize, Span),
    block_arguments: (ast::BodyBlockId, &'static ast::Function),
    argument_arguments: (
        &'static [ast::Argument],
        &'static ast::Program,
        usize,
        usize,
        Span,
    ),
    walk_node: Option<WalkNode>,
    module_iterator: std::ops::Range<usize>,
    function_iterator: std::iter::Enumerate<std::slice::Iter<'static, ast::Function>>,
    statement_iterator: std::slice::Iter<'static, ast::Stmt>,
    arm_iterator: std::slice::Iter<'static, ast::MatchArmSyntax>,
    field_iterator: std::slice::Iter<'static, ast::FieldInit>,
    element_iterator: std::slice::Iter<'static, ast::ExprId>,
    argument_iterator: std::slice::Iter<'static, ast::Argument>,
    function: &'static RawOwnedFunction,
    block: &'static OwnedBlock,
    source: &'static crate::frontend::source::SourceFile,
    expression: &'static ast::Expr,
    owner: &'static ast::Function,
    body: &'static ast::BodyBlock,
    assignments: [&'static Assign; 2],
    instructions: [&'static OwnedStatement; 4],
    local: &'static LocalDecl,
    declaration: &'static OwnerDecl,
    elements: &'static [Operand],
    operand: &'static Operand,
    origins: [Option<DiagnosticOrigins>; 3],
    array: FixedArrayTy,
    spans: [Span; 4],
    scalar_indices: [usize; 16],
    atom: Option<byte_literal::Atom>,
    atom_result: Result<Option<byte_literal::Atom>, byte_literal::Error>,
    predicates: [bool; 8],
}
fn carrier_bytes() -> usize {
    size_of::<LiteralCarriers>()
        + byte_literal::scanner_scratch_bytes()
        + crate::frontend::oir::source::association::association_error_bytes()
}

#[cfg(test)]
mod resource_formula_tests {
    use super::*;

    #[test]
    fn byte_literal_structural_payload_formula_prices_separate_state() {
        let plan = Plan {
            modules: 2,
            expressions: 7,
            stack: 5,
            blocks: 3,
        };
        assert_eq!(
            plan.payload().unwrap(),
            3 * size_of::<usize>() + 7 * size_of::<usize>() + 7 + 5 * size_of::<WalkNode>() + 3,
        );
        assert!(Plan {
            modules: usize::MAX,
            ..plan
        }
        .payload()
        .is_err());
        assert!(Plan {
            expressions: usize::MAX,
            ..plan
        }
        .payload()
        .is_err());
        assert!(Plan {
            stack: usize::MAX,
            ..plan
        }
        .payload()
        .is_err());
    }
}
