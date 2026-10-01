//! Validate arbitrary raw OIR, without trusting IDs, spans, or its producer.
use super::*;
const STAGE: &str = "oir-verify";
fn failure(kind: FailureKind, span: Span) -> OirFailure {
    OirFailure::new(kind, STAGE, Some(span))
}
fn span(sources: &SourceMap, span: Span) -> Result<(), OirFailure> {
    if sources.is_valid_span(span) {
        Ok(())
    } else {
        Err(failure(FailureKind::InvalidSpan, span))
    }
}
fn local(function: &Function, id: LocalId, origin: Span) -> Result<&LocalDecl, OirFailure> {
    function
        .locals
        .get(id.0)
        .ok_or_else(|| failure(FailureKind::InvalidLocal, origin))
}
fn operand(
    function: &Function,
    value: Operand,
    sources: &SourceMap,
) -> Result<hir::Ty, OirFailure> {
    span(sources, value.span)?;
    Ok(local(function, value.local, value.span)?.ty)
}
fn same_type(actual: hir::Ty, expected: hir::Ty, origin: Span) -> Result<(), OirFailure> {
    if actual == expected {
        Ok(())
    } else {
        Err(failure(FailureKind::TypeMismatch, origin))
    }
}
fn terminator(block: &BasicBlock) -> Result<&Terminator, OirFailure> {
    block
        .terminator
        .as_ref()
        .ok_or_else(|| failure(FailureKind::MissingTerminator, block.span))
}

/// No recursive traversal, unchecked indexing, panics, or debug-only gates.
/// First validate every signature; then every block, including unreachable ones;
/// finally verify the acyclic CFG, canonical definitions, and dominance.
pub(super) fn verify(program: Program, sources: &SourceMap) -> Result<VerifiedProgram, OirFailure> {
    let mut budget = Budget::default();
    if program.functions.len() > MAX_BLOCKS {
        return Err(OirFailure::new(
            FailureKind::ResourceLimit("functions"),
            STAGE,
            None,
        ));
    }
    for function in &program.functions {
        let origin = Some(function.span);
        Budget::add(
            &mut budget.locals,
            function.locals.len(),
            MAX_LOCALS,
            "locals",
            STAGE,
            origin,
        )?;
        Budget::add(
            &mut budget.blocks,
            function.blocks.len(),
            MAX_BLOCKS,
            "blocks",
            STAGE,
            origin,
        )?;
        for block in &function.blocks {
            Budget::add(
                &mut budget.assignments,
                block.statements.len(),
                MAX_ASSIGNMENTS,
                "assignments",
                STAGE,
                Some(block.span),
            )?;
            if let Some(Terminator {
                kind: TerminatorKind::Call { args, .. },
                span,
            }) = &block.terminator
            {
                if args.len() > super::super::parser::MAX_PARAMS {
                    return Err(failure(FailureKind::ResourceLimit("call arguments"), *span));
                }
            }
        }
    }
    // Finish the whole signature table before looking at any call, including
    // forward calls and recursion. Parameter types live in this single table.
    for (index, function) in program.functions.iter().enumerate() {
        span(sources, function.span)?;
        if function.id != hir::DefId(index) {
            return Err(failure(FailureKind::InvalidFunctionId, function.span));
        }
        if function.param_count > function.locals.len()
            || function.param_count > super::super::parser::MAX_PARAMS
        {
            return Err(failure(FailureKind::ParameterCount, function.span));
        }
        for (index, decl) in function.locals.iter().enumerate() {
            span(sources, decl.span)?;
            if (index < function.param_count) != (decl.kind == LocalKind::Parameter) {
                return Err(failure(FailureKind::ParameterKind, decl.span));
            }
        }
        if function.blocks.is_empty() {
            return Err(failure(FailureKind::NoBlocks, function.span));
        }
        if function.blocks.get(function.entry.0).is_none() {
            return Err(failure(FailureKind::InvalidBlock, function.span));
        }
    }
    for function in &program.functions {
        for block in &function.blocks {
            span(sources, block.span)?;
            for assign in &block.statements {
                span(sources, assign.span)?;
                let expected = local(function, assign.destination, assign.span)?.ty;
                let actual = match assign.value {
                    Rvalue::Bool(_) => hir::Ty::Bool,
                    Rvalue::I32(_) => hir::Ty::I32,
                    Rvalue::Unit => hir::Ty::Unit,
                    Rvalue::Copy(value) => operand(function, value, sources)?,
                    Rvalue::CompareScalar {
                        op,
                        left,
                        right,
                        operator_span,
                    } => {
                        span(sources, operator_span)?;
                        let left_ty = operand(function, left, sources)?;
                        let expected = match op {
                            hir::ComparisonOp::Equal | hir::ComparisonOp::NotEqual => {
                                if !matches!(left_ty, hir::Ty::I32 | hir::Ty::Bool) {
                                    return Err(failure(FailureKind::TypeMismatch, left.span));
                                }
                                left_ty
                            }
                            hir::ComparisonOp::Less
                            | hir::ComparisonOp::LessEqual
                            | hir::ComparisonOp::Greater
                            | hir::ComparisonOp::GreaterEqual => {
                                same_type(left_ty, hir::Ty::I32, left.span)?;
                                hir::Ty::I32
                            }
                        };
                        same_type(operand(function, right, sources)?, expected, right.span)?;
                        hir::Ty::Bool
                    }
                    Rvalue::CheckedI32 {
                        left,
                        right,
                        operator_span,
                        ..
                    } => {
                        span(sources, operator_span)?;
                        for value in [left, right] {
                            same_type(
                                operand(function, value, sources)?,
                                hir::Ty::I32,
                                value.span,
                            )?;
                        }
                        hir::Ty::I32
                    }
                };
                same_type(actual, expected, assign.span)?;
            }
            let end = terminator(block)?;
            span(sources, end.span)?;
            match &end.kind {
                TerminatorKind::Branch {
                    condition,
                    then_block,
                    else_block,
                } => {
                    same_type(
                        operand(function, *condition, sources)?,
                        hir::Ty::Bool,
                        condition.span,
                    )?;
                    for target in [then_block, else_block] {
                        if function.blocks.get(target.0).is_none() {
                            return Err(failure(FailureKind::InvalidBlock, end.span));
                        }
                    }
                }
                TerminatorKind::Goto { target } => {
                    if function.blocks.get(target.0).is_none() {
                        return Err(failure(FailureKind::InvalidBlock, end.span));
                    }
                }
                TerminatorKind::Return(value) => same_type(
                    operand(function, *value, sources)?,
                    function.result,
                    value.span,
                )?,
                TerminatorKind::Call {
                    target,
                    args,
                    destination,
                    continuation,
                } => {
                    let callee = program
                        .functions
                        .get(target.0)
                        .ok_or_else(|| failure(FailureKind::InvalidTarget, end.span))?;
                    let result = local(function, *destination, end.span)?.ty;
                    if function.blocks.get(continuation.0).is_none() {
                        return Err(failure(FailureKind::InvalidBlock, end.span));
                    }
                    if args.len() != callee.param_count {
                        return Err(failure(FailureKind::Arity, end.span));
                    }
                    for (index, arg) in args.iter().enumerate() {
                        // Parameter prefix was validated above; still use a fallible lookup.
                        let parameter = local(callee, LocalId(index), end.span)?;
                        same_type(operand(function, *arg, sources)?, parameter.ty, arg.span)?;
                    }
                    same_type(result, callee.result, end.span)?;
                }
            }
        }
    }
    for function in &program.functions {
        cfg(function)?;
    }
    Ok(VerifiedProgram { program })
}

// Each non-parameter slot has one canonical definition across the entire
// function, even when two writes would be in mutually exclusive arms. This is
// an immutable no-phi IR rule, not support for mutation or merged values.
#[derive(Clone, Copy)]
enum Definition {
    Parameter,
    Assignment { block: usize, statement: usize },
    CallResult { block: usize },
}
fn define(
    definitions: &mut [Option<Definition>],
    destination: LocalId,
    definition: Definition,
    origin: Span,
) -> Result<(), OirFailure> {
    let slot = definitions
        .get_mut(destination.0)
        .ok_or_else(|| failure(FailureKind::InvalidLocal, origin))?;
    if slot.is_some() {
        return Err(failure(FailureKind::AlreadyInitialized, origin));
    }
    *slot = Some(definition);
    Ok(())
}
fn definitions(function: &Function) -> Result<Vec<Option<Definition>>, OirFailure> {
    let mut table = vec![None; function.locals.len()];
    for parameter in 0..function.param_count {
        define(
            &mut table,
            LocalId(parameter),
            Definition::Parameter,
            function.span,
        )?;
    }
    for (block_id, block) in function.blocks.iter().enumerate() {
        for (statement, assign) in block.statements.iter().enumerate() {
            define(
                &mut table,
                assign.destination,
                Definition::Assignment {
                    block: block_id,
                    statement,
                },
                assign.span,
            )?;
        }
        let end = terminator(block)?;
        if let TerminatorKind::Call { destination, .. } = end.kind {
            define(
                &mut table,
                destination,
                Definition::CallResult { block: block_id },
                end.span,
            )?;
        }
    }
    Ok(table)
}

// Raw references and dimensions are always checked, even though structural
// validation and bounded constructors have already established their ranges.
fn at<T>(table: &[T], index: usize, origin: Span) -> Result<&T, OirFailure> {
    table
        .get(index)
        .ok_or_else(|| failure(FailureKind::InvalidBlock, origin))
}
fn at_mut<T>(table: &mut [T], index: usize, origin: Span) -> Result<&mut T, OirFailure> {
    table
        .get_mut(index)
        .ok_or_else(|| failure(FailureKind::InvalidBlock, origin))
}
fn successors(block: &BasicBlock) -> Result<[Option<BlockId>; 2], OirFailure> {
    Ok(match terminator(block)?.kind {
        TerminatorKind::Branch {
            then_block,
            else_block,
            ..
        } => [Some(then_block), Some(else_block)],
        TerminatorKind::Goto { target } => [Some(target), None],
        TerminatorKind::Call { continuation, .. } => [Some(continuation), None],
        TerminatorKind::Return(_) => [None, None],
    })
}

#[derive(Debug)]
pub(super) struct ScratchDimensions {
    pub(super) offsets: usize,
    pub(super) levels: usize,
    pub(super) ancestors: usize,
}
pub(super) fn checked_product(
    left: usize,
    right: usize,
    limit: usize,
    name: &'static str,
    origin: Span,
) -> Result<usize, OirFailure> {
    left.checked_mul(right)
        .filter(|&value| value <= limit)
        .ok_or_else(|| failure(FailureKind::ResourceLimit(name), origin))
}
pub(super) fn scratch_dimensions(
    blocks: usize,
    edges: usize,
    origin: Span,
) -> Result<ScratchDimensions, OirFailure> {
    let mut bounded_blocks = 0;
    Budget::add(
        &mut bounded_blocks,
        blocks,
        MAX_BLOCKS,
        "blocks",
        STAGE,
        Some(origin),
    )?;
    let max_edges = checked_product(blocks, 2, MAX_BLOCKS * 2, "CFG edges", origin)?;
    let mut bounded_edges = 0;
    Budget::add(
        &mut bounded_edges,
        edges,
        max_edges,
        "CFG edges",
        STAGE,
        Some(origin),
    )?;
    let mut offsets = blocks;
    Budget::add(
        &mut offsets,
        1,
        MAX_BLOCKS + 1,
        "CFG offsets",
        STAGE,
        Some(origin),
    )?;
    // floor(log2(B)) + 1 suffices for every possible depth <= B - 1.
    // Empty is useful for dimension tests; actual bodies cannot be empty.
    let levels = blocks.checked_ilog2().map_or(0, |log| log as usize + 1);
    let max_levels = MAX_BLOCKS.ilog2() as usize + 1;
    let ancestors = checked_product(
        blocks,
        levels,
        MAX_BLOCKS * max_levels,
        "dominator ancestors",
        origin,
    )?;
    Ok(ScratchDimensions {
        offsets,
        levels,
        ancestors,
    })
}

struct Predecessors {
    offsets: Vec<usize>,
    blocks: Vec<usize>,
}
impl Predecessors {
    fn of(&self, block: usize, origin: Span) -> Result<&[usize], OirFailure> {
        let after = block
            .checked_add(1)
            .ok_or_else(|| failure(FailureKind::InvalidBlock, origin))?;
        let start = *at(&self.offsets, block, origin)?;
        let end = *at(&self.offsets, after, origin)?;
        self.blocks
            .get(start..end)
            .ok_or_else(|| failure(FailureKind::InvalidBlock, origin))
    }
}
fn predecessors(function: &Function) -> Result<Predecessors, OirFailure> {
    let count = function.blocks.len();
    let origin = function.span;
    let shape = scratch_dimensions(count, 0, origin)?;
    let mut counts = vec![0; count];
    let mut edges = 0;
    for block in &function.blocks {
        for successor in successors(block)?.into_iter().flatten() {
            Budget::add(
                &mut edges,
                1,
                MAX_BLOCKS * 2,
                "CFG edges",
                STAGE,
                Some(block.span),
            )?;
            Budget::add(
                at_mut(&mut counts, successor.0, block.span)?,
                1,
                MAX_BLOCKS * 2,
                "CFG predecessors",
                STAGE,
                Some(block.span),
            )?;
        }
    }
    scratch_dimensions(count, edges, origin)?;
    let mut offsets = Vec::with_capacity(shape.offsets);
    offsets.push(0);
    let mut total = 0;
    for count in &counts {
        Budget::add(
            &mut total,
            *count,
            edges,
            "CFG offsets",
            STAGE,
            Some(origin),
        )?;
        offsets.push(total);
    }
    // Reuse counts as insertion cursors rather than allocating an edge object
    // or a predecessor Vec for each node. Duplicate branch targets deliberately
    // retain edge multiplicity in both predecessors and Kahn indegrees.
    for (index, cursor) in counts.iter_mut().enumerate() {
        *cursor = *at(&offsets, index, origin)?;
    }
    let mut blocks = vec![0; edges];
    for (from, block) in function.blocks.iter().enumerate() {
        for successor in successors(block)?.into_iter().flatten() {
            let cursor = at_mut(&mut counts, successor.0, block.span)?;
            *at_mut(&mut blocks, *cursor, block.span)? = from;
            Budget::add(cursor, 1, edges, "CFG offsets", STAGE, Some(block.span))?;
        }
    }
    Ok(Predecessors { offsets, blocks })
}

fn topological(function: &Function, predecessors: &Predecessors) -> Result<Vec<usize>, OirFailure> {
    let count = function.blocks.len();
    let origin = function.span;
    let mut seen = vec![false; count];
    let mut order = Vec::with_capacity(count);
    *at_mut(&mut seen, function.entry.0, origin)? = true;
    order.push(function.entry.0);
    let mut cursor = 0;
    while let Some(&current) = order.get(cursor) {
        let block = at(&function.blocks, current, origin)?;
        for next in successors(block)?.into_iter().flatten() {
            let reached = at_mut(&mut seen, next.0, block.span)?;
            if !*reached {
                *reached = true;
                order.push(next.0);
            }
        }
        cursor += 1; // <= the previously checked block count.
    }
    let reachable = order.len();
    order.clear();
    let mut indegrees = vec![0; count];
    for (block, &reached) in seen.iter().enumerate() {
        if reached {
            let mut degree = 0;
            for &pred in predecessors.of(block, origin)? {
                if *at(&seen, pred, origin)? {
                    Budget::add(
                        &mut degree,
                        1,
                        MAX_BLOCKS * 2,
                        "CFG predecessors",
                        STAGE,
                        Some(origin),
                    )?;
                }
            }
            *at_mut(&mut indegrees, block, origin)? = degree;
            if degree == 0 {
                order.push(block);
            }
        }
    }
    cursor = 0;
    while let Some(&current) = order.get(cursor) {
        let block = at(&function.blocks, current, origin)?;
        for next in successors(block)?.into_iter().flatten() {
            let degree = at_mut(&mut indegrees, next.0, block.span)?;
            *degree = degree
                .checked_sub(1)
                .ok_or_else(|| failure(FailureKind::InvalidBlock, block.span))?;
            if *degree == 0 {
                order.push(next.0);
            }
        }
        cursor += 1;
    }
    // Preserve the previous verifier's cycle diagnostic for a reachable cycle
    // that also strands blocks, such as a self-call continuation back to entry.
    if order.len() != reachable {
        return Err(failure(FailureKind::Cycle, origin));
    }
    if reachable != count {
        return Err(failure(FailureKind::Unreachable, origin));
    }
    Ok(order)
}

struct Ancestors {
    levels: usize,
    cells: Vec<usize>,
    depths: Vec<usize>,
}
impl Ancestors {
    fn index(&self, block: usize, level: usize, origin: Span) -> Result<usize, OirFailure> {
        if level >= self.levels {
            return Err(failure(FailureKind::InvalidBlock, origin));
        }
        block
            .checked_mul(self.levels)
            .and_then(|base| base.checked_add(level))
            .filter(|&index| index < self.cells.len())
            .ok_or_else(|| failure(FailureKind::InvalidBlock, origin))
    }
    fn get(&self, block: usize, level: usize, origin: Span) -> Result<usize, OirFailure> {
        Ok(*at(&self.cells, self.index(block, level, origin)?, origin)?)
    }
    fn set(
        &mut self,
        block: usize,
        level: usize,
        value: usize,
        origin: Span,
    ) -> Result<(), OirFailure> {
        let index = self.index(block, level, origin)?;
        *at_mut(&mut self.cells, index, origin)? = value;
        Ok(())
    }
    fn lca(&self, mut left: usize, mut right: usize, origin: Span) -> Result<usize, OirFailure> {
        if at(&self.depths, left, origin)? < at(&self.depths, right, origin)? {
            std::mem::swap(&mut left, &mut right);
        }
        let difference = at(&self.depths, left, origin)? - at(&self.depths, right, origin)?;
        for level in 0..self.levels {
            if difference & (1usize << level) != 0 {
                left = self.get(left, level, origin)?;
            }
        }
        if left == right {
            return Ok(left);
        }
        for level in (0..self.levels).rev() {
            let up_left = self.get(left, level, origin)?;
            let up_right = self.get(right, level, origin)?;
            if up_left != up_right {
                left = up_left;
                right = up_right;
            }
        }
        self.get(left, 0, origin)
    }
}
struct Dominance {
    enter: Vec<usize>,
    exit: Vec<usize>,
}
impl Dominance {
    fn contains(
        &self,
        definition: usize,
        use_block: usize,
        origin: Span,
    ) -> Result<bool, OirFailure> {
        Ok(
            at(&self.enter, definition, origin)? <= at(&self.enter, use_block, origin)?
                && at(&self.exit, use_block, origin)? <= at(&self.exit, definition, origin)?,
        )
    }
}
fn dominance(
    function: &Function,
    predecessors: &Predecessors,
    order: &[usize],
) -> Result<Dominance, OirFailure> {
    let count = function.blocks.len();
    let origin = function.span;
    let shape = scratch_dimensions(count, predecessors.blocks.len(), origin)?;
    let mut ancestors = Ancestors {
        levels: shape.levels,
        cells: vec![0; shape.ancestors],
        depths: vec![0; count],
    };
    const NONE: usize = usize::MAX;
    let mut first_child = vec![NONE; count];
    let mut next_sibling = vec![NONE; count];
    for &block in order {
        let parent = if block == function.entry.0 {
            block
        } else {
            let mut preds = predecessors.of(block, origin)?.iter().copied();
            let first = preds
                .next()
                .ok_or_else(|| failure(FailureKind::Unreachable, origin))?;
            let mut common = first;
            for pred in preds {
                common = ancestors.lca(common, pred, origin)?;
            }
            let mut depth = *at(&ancestors.depths, common, origin)?;
            Budget::add(&mut depth, 1, count, "dominator depth", STAGE, Some(origin))?;
            *at_mut(&mut ancestors.depths, block, origin)? = depth;
            *at_mut(&mut next_sibling, block, origin)? = *at(&first_child, common, origin)?;
            *at_mut(&mut first_child, common, origin)? = block;
            common
        };
        ancestors.set(block, 0, parent, origin)?;
        for level in 1..shape.levels {
            let half = ancestors.get(block, level - 1, origin)?;
            ancestors.set(
                block,
                level,
                ancestors.get(half, level - 1, origin)?,
                origin,
            )?;
        }
    }
    let mut result = Dominance {
        enter: vec![0; count],
        exit: vec![0; count],
    };
    let mut current = function.entry.0;
    let mut clock = 0;
    let clock_limit = checked_product(count, 2, MAX_BLOCKS * 2, "dominator clock", origin)?;
    // Parent/first-child/next-sibling walks need no recursion or DFS frame stack.
    loop {
        *at_mut(&mut result.enter, current, origin)? = clock;
        Budget::add(
            &mut clock,
            1,
            clock_limit,
            "dominator clock",
            STAGE,
            Some(origin),
        )?;
        let child = *at(&first_child, current, origin)?;
        if child != NONE {
            current = child;
            continue;
        }
        loop {
            *at_mut(&mut result.exit, current, origin)? = clock;
            Budget::add(
                &mut clock,
                1,
                clock_limit,
                "dominator clock",
                STAGE,
                Some(origin),
            )?;
            if current == function.entry.0 {
                return Ok(result);
            }
            let sibling = *at(&next_sibling, current, origin)?;
            if sibling != NONE {
                current = sibling;
                break;
            }
            current = ancestors.get(current, 0, origin)?;
        }
    }
}
fn read(
    definitions: &[Option<Definition>],
    dominance: &Dominance,
    value: Operand,
    use_block: usize,
    use_statement: usize,
) -> Result<(), OirFailure> {
    let definition = definitions
        .get(value.local.0)
        .ok_or_else(|| failure(FailureKind::InvalidLocal, value.span))?;
    let available = match definition {
        Some(Definition::Parameter) => true,
        Some(Definition::Assignment { block, statement }) if *block == use_block => {
            *statement < use_statement
        }
        Some(Definition::Assignment { block, .. }) => {
            dominance.contains(*block, use_block, value.span)?
        }
        // A call has exactly one normal successor. Strict block dominance in
        // this acyclic CFG therefore implies its continuation edge was crossed.
        // This must change if calls ever gain unwind or other successor edges.
        Some(Definition::CallResult { block }) => {
            *block != use_block && dominance.contains(*block, use_block, value.span)?
        }
        None => false,
    };
    if available {
        Ok(())
    } else {
        Err(failure(FailureKind::Uninitialized, value.span))
    }
}
fn cfg(function: &Function) -> Result<(), OirFailure> {
    let predecessors = predecessors(function)?;
    let order = topological(function, &predecessors)?;
    let definitions = definitions(function)?;
    let dominance = dominance(function, &predecessors, &order)?;
    for (block_id, block) in function.blocks.iter().enumerate() {
        for (index, assign) in block.statements.iter().enumerate() {
            match assign.value {
                Rvalue::Copy(value) => read(&definitions, &dominance, value, block_id, index)?,
                Rvalue::CheckedI32 { left, right, .. }
                | Rvalue::CompareScalar { left, right, .. } => {
                    read(&definitions, &dominance, left, block_id, index)?;
                    read(&definitions, &dominance, right, block_id, index)?;
                }
                Rvalue::Bool(_) | Rvalue::I32(_) | Rvalue::Unit => {}
            }
        }
        let position = block.statements.len();
        match &terminator(block)?.kind {
            TerminatorKind::Return(value)
            | TerminatorKind::Branch {
                condition: value, ..
            } => {
                read(&definitions, &dominance, *value, block_id, position)?;
            }
            TerminatorKind::Call { args, .. } => {
                for &value in args {
                    read(&definitions, &dominance, value, block_id, position)?;
                }
            }
            TerminatorKind::Goto { .. } => {}
        }
    }
    Ok(())
}
