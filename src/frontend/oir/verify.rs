//! Validate arbitrary raw OIR, without trusting IDs, spans, or its producer.
use super::*;
const STAGE: &str = "oir-verify";
#[cfg(test)]
#[path = "cyclic_tests.rs"]
mod cyclic_tests;
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
fn place(function: &Function, value: Place, sources: &SourceMap) -> Result<hir::Ty, OirFailure> {
    span(sources, value.span)?;
    function
        .places
        .get(value.id.0)
        .map(|decl| decl.ty)
        .ok_or_else(|| failure(FailureKind::InvalidPlace, value.span))
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
/// finally verify the reachable CFG, canonical definitions, and dominance.
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
            &mut budget.locals,
            function.places.len(),
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
            Budget::add(
                &mut budget.assignments,
                usize::from(block.merge.is_some()),
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
        for decl in &function.places {
            span(sources, decl.span)?;
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
            if let Some(merge) = &block.merge {
                span(sources, merge.span)?;
                span(sources, merge.operator_span)?;
                same_type(
                    local(function, merge.destination, merge.span)?.ty,
                    hir::Ty::Bool,
                    merge.span,
                )?;
                for input in &merge.incoming {
                    if function.blocks.get(input.predecessor.0).is_none() {
                        return Err(failure(FailureKind::InvalidBlock, merge.span));
                    }
                    same_type(
                        operand(function, input.value, sources)?,
                        hir::Ty::Bool,
                        input.value.span,
                    )?;
                }
            }
            for statement in &block.statements {
                span(sources, statement.span())?;
                let assign = match statement {
                    Statement::Assign(assign) => assign,
                    Statement::Initialize {
                        place: target,
                        value,
                        ..
                    }
                    | Statement::Store {
                        place: target,
                        value,
                        ..
                    } => {
                        let expected = place(function, *target, sources)?;
                        same_type(operand(function, *value, sources)?, expected, value.span)?;
                        if let Statement::Store { operator_span, .. } = statement {
                            span(sources, *operator_span)?;
                        }
                        continue;
                    }
                };
                let expected = local(function, assign.destination, assign.span)?.ty;
                let actual = match assign.value {
                    Rvalue::Load(value) => place(function, value, sources)?,
                    Rvalue::NotBool {
                        operand: value,
                        operator_span,
                    } => {
                        span(sources, operator_span)?;
                        same_type(
                            operand(function, value, sources)?,
                            hir::Ty::Bool,
                            value.span,
                        )?;
                        hir::Ty::Bool
                    }
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
// an immutable SSA rule. A bool merge defines one slot at block entry; its
// incoming edge operands are reads, never opposite-arm writes to that slot.
#[derive(Clone, Copy)]
enum Definition {
    Parameter,
    Merge { block: usize },
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
        if let Some(merge) = &block.merge {
            define(
                &mut table,
                merge.destination,
                Definition::Merge { block: block_id },
                merge.span,
            )?;
        }
        for (statement, instruction) in block.statements.iter().enumerate() {
            let Statement::Assign(assign) = instruction else {
                continue;
            };
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

// Places have a separate canonical initialization table. Stores do not define
// values or initialize places. Reuse the position/dominance proof, not SSA IDs.
fn initializations(function: &Function) -> Result<Vec<Option<Definition>>, OirFailure> {
    let mut table = vec![None; function.places.len()];
    for (block, body) in function.blocks.iter().enumerate() {
        for (statement, instruction) in body.statements.iter().enumerate() {
            if let Statement::Initialize { place, span, .. } = instruction {
                let entry = table
                    .get_mut(place.id.0)
                    .ok_or_else(|| failure(FailureKind::InvalidPlace, *span))?;
                if entry.is_some() {
                    return Err(failure(FailureKind::AlreadyInitialized, *span));
                }
                *entry = Some(Definition::Assignment { block, statement });
            }
        }
    }
    for (index, entry) in table.iter().enumerate() {
        if entry.is_none() {
            return Err(failure(
                FailureKind::Uninitialized,
                function.places[index].span,
            ));
        }
    }
    Ok(table)
}
fn read_place(
    table: &[Option<Definition>],
    dominance: &Dominance,
    place: Place,
    block: usize,
    statement: usize,
) -> Result<(), OirFailure> {
    // This operand indexes only the separate initialization table. It is never
    // a source value operand or an index into the SSA definition table.
    read(
        table,
        dominance,
        Operand {
            local: LocalId(place.id.0),
            span: place.span,
        },
        block,
        statement,
    )
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
    pub(super) vertices: usize,
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
    Ok(ScratchDimensions {
        offsets,
        vertices: bounded_blocks,
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
    // retain edge multiplicity for the exact bool-merge predecessor check.
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

const NONE: usize = usize::MAX;

struct DepthFirst {
    order: Vec<usize>,
    parent: Vec<usize>,
    number: Vec<usize>,
}
fn depth_first(function: &Function) -> Result<DepthFirst, OirFailure> {
    let origin = function.span;
    let count = scratch_dimensions(function.blocks.len(), 0, origin)?.vertices;
    let mut result = DepthFirst {
        order: Vec::with_capacity(count),
        parent: vec![NONE; count],
        number: vec![NONE; count],
    };
    // Each vertex owns a two-slot successor cursor. Walking DFS parents when
    // its slots are exhausted replaces recursive calls, including at MAX_BLOCKS.
    let mut next_edge = vec![0u8; count];
    let root = function.entry.0;
    *at_mut(&mut result.number, root, origin)? = 0;
    *at_mut(&mut result.parent, root, origin)? = root;
    result.order.push(root);
    let mut current = root;
    loop {
        let cursor = at_mut(&mut next_edge, current, origin)?;
        if *cursor == 2 {
            if current == root {
                break;
            }
            current = *at(&result.parent, current, origin)?;
            continue;
        }
        let edge = usize::from(*cursor);
        *cursor += 1; // The two successor slots bound this increment.
        let body = at(&function.blocks, current, origin)?;
        if let Some(next) = *at(&successors(body)?, edge, body.span)? {
            let number = at_mut(&mut result.number, next.0, body.span)?;
            if *number == NONE {
                *number = result.order.len();
                *at_mut(&mut result.parent, next.0, body.span)? = current;
                result.order.push(next.0);
                current = next.0;
            }
        }
    }
    if result.order.len() != count {
        return Err(failure(FailureKind::Unreachable, origin));
    }
    Ok(result)
}

// Simple Lengauer-Tarjan link/eval forest. A link always points to a strict
// DFS ancestor. Compression preserves the label with the least semidominator
// on the linked path, excluding its unlinked root. The explicit reusable path
// replaces recursive COMPRESS without changing its bottom-up update order.
struct EvalForest {
    ancestor: Vec<usize>,
    label: Vec<usize>,
    path: Vec<usize>,
}
impl EvalForest {
    fn eval(&mut self, vertex: usize, semi: &[usize], origin: Span) -> Result<usize, OirFailure> {
        self.path.clear();
        let mut current = vertex;
        loop {
            let parent = *at(&self.ancestor, current, origin)?;
            if parent == NONE || *at(&self.ancestor, parent, origin)? == NONE {
                break;
            }
            self.path.push(current);
            current = parent;
        }
        while let Some(current) = self.path.pop() {
            let parent = *at(&self.ancestor, current, origin)?;
            let parent_label = *at(&self.label, parent, origin)?;
            let label = *at(&self.label, current, origin)?;
            if at(semi, parent_label, origin)? < at(semi, label, origin)? {
                *at_mut(&mut self.label, current, origin)? = parent_label;
            }
            *at_mut(&mut self.ancestor, current, origin)? = *at(&self.ancestor, parent, origin)?;
        }
        Ok(*at(&self.label, vertex, origin)?)
    }
}

// Lengauer and Tarjan, "A Fast Algorithm for Finding Dominators in a
// Flowgraph" (1979), https://doi.org/10.1145/357062.357071, simple link/eval
// variant. There are B-1 links, at most
// E+B-1 evals, and one insertion/removal per non-entry bucket member. Path
// compression gives O((B+E) log B) total work, not a fixed-point sweep bound
// that assumes reducible source CFGs. All arrays and the reusable eval path
// are O(B); predecessors are O(B+E). No matrix or extra acceptance cap.
// Peak CFG scratch here is 11B+E+1 usize cells including the predecessor and
// DFS tables; canonical value/place tables and the caller's raw IR are separate.
fn immediate_dominators(
    function: &Function,
    predecessors: &Predecessors,
    dfs: &DepthFirst,
) -> Result<Vec<usize>, OirFailure> {
    let origin = function.span;
    let count =
        scratch_dimensions(function.blocks.len(), predecessors.blocks.len(), origin)?.vertices;
    let mut semi = dfs.number.clone();
    let mut forest = EvalForest {
        ancestor: vec![NONE; count],
        label: (0..count).collect(),
        path: Vec::with_capacity(count),
    };
    let mut bucket = vec![NONE; count];
    let mut next_member = vec![NONE; count];
    let mut dominator = vec![NONE; count];
    for &block in dfs.order.iter().skip(1).rev() {
        let mut best = *at(&semi, block, origin)?;
        for &pred in predecessors.of(block, origin)? {
            let label = forest.eval(pred, &semi, origin)?;
            best = best.min(*at(&semi, label, origin)?);
        }
        *at_mut(&mut semi, block, origin)? = best;
        let semidominator = *at(&dfs.order, best, origin)?;
        *at_mut(&mut next_member, block, origin)? = *at(&bucket, semidominator, origin)?;
        *at_mut(&mut bucket, semidominator, origin)? = block;
        let parent = *at(&dfs.parent, block, origin)?;
        *at_mut(&mut forest.ancestor, block, origin)? = parent;
        let mut member = *at(&bucket, parent, origin)?;
        *at_mut(&mut bucket, parent, origin)? = NONE;
        while member != NONE {
            let label = forest.eval(member, &semi, origin)?;
            *at_mut(&mut dominator, member, origin)? =
                if at(&semi, label, origin)? < at(&semi, member, origin)? {
                    label
                } else {
                    parent
                };
            member = *at(&next_member, member, origin)?;
        }
    }
    // Provisional dominators precede their blocks in DFS order. Correct them
    // once in forward order; this is not an iterative convergence loop.
    for &block in dfs.order.iter().skip(1) {
        let parent = *at(&dominator, block, origin)?;
        let semidominator = *at(&dfs.order, *at(&semi, block, origin)?, origin)?;
        if parent != semidominator {
            *at_mut(&mut dominator, block, origin)? = *at(&dominator, parent, origin)?;
        }
    }
    *at_mut(&mut dominator, function.entry.0, origin)? = function.entry.0;
    Ok(dominator)
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
    dfs: &DepthFirst,
) -> Result<Dominance, OirFailure> {
    let count = function.blocks.len();
    let origin = function.span;
    let dominator = immediate_dominators(function, predecessors, dfs)?;
    let mut first_child = vec![NONE; count];
    let mut next_sibling = vec![NONE; count];
    for &block in dfs.order.iter().skip(1) {
        let parent = *at(&dominator, block, origin)?;
        *at_mut(&mut next_sibling, block, origin)? = *at(&first_child, parent, origin)?;
        *at_mut(&mut first_child, parent, origin)? = block;
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
            current = *at(&dominator, current, origin)?;
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
        Some(Definition::Merge { block }) => dominance.contains(*block, use_block, value.span)?,
        Some(Definition::Assignment { block, statement }) if *block == use_block => {
            *statement < use_statement
        }
        Some(Definition::Assignment { block, .. }) => {
            dominance.contains(*block, use_block, value.span)?
        }
        // A call has exactly one normal successor. On every path to a different
        // dominated block, leaving the call block crosses that continuation
        // edge. This remains true with backedges and irreducible cycles. Reads
        // inside the call block stay unavailable, including on its first visit.
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
    let dfs = depth_first(function)?;
    let definitions = definitions(function)?;
    let initializations = initializations(function)?;
    let dominance = dominance(function, &predecessors, &dfs)?;
    for (block_id, block) in function.blocks.iter().enumerate() {
        if let Some(merge) = &block.merge {
            let preds = predecessors.of(block_id, merge.span)?;
            let [left, right] = merge.incoming;
            if block_id == function.entry.0
                || preds.len() != 2
                || preds[0] == preds[1]
                || left.predecessor == right.predecessor
                || !preds.contains(&left.predecessor.0)
                || !preds.contains(&right.predecessor.0)
            {
                return Err(failure(FailureKind::InvalidMerge, merge.span));
            }
            for input in &merge.incoming {
                let pred = input.predecessor.0;
                let block = at(&function.blocks, pred, merge.span)?;
                // A Call defines its result on this one normal successor edge.
                // It remains unavailable in that block's statements/arguments.
                let is_edge_result = matches!(definitions.get(input.value.local.0), Some(Some(Definition::CallResult {block})) if *block == pred)
                    && matches!(terminator(block)?.kind, TerminatorKind::Call {continuation,..} if continuation.0 == block_id);
                if !is_edge_result {
                    read(
                        &definitions,
                        &dominance,
                        input.value,
                        pred,
                        block.statements.len(),
                    )?;
                }
            }
        }
        for (index, instruction) in block.statements.iter().enumerate() {
            let assign = match instruction {
                Statement::Assign(assign) => assign,
                Statement::Initialize { value, .. } => {
                    read(&definitions, &dominance, *value, block_id, index)?;
                    continue;
                }
                Statement::Store { place, value, .. } => {
                    read_place(&initializations, &dominance, *place, block_id, index)?;
                    read(&definitions, &dominance, *value, block_id, index)?;
                    continue;
                }
            };
            match assign.value {
                Rvalue::Load(place) => {
                    read_place(&initializations, &dominance, place, block_id, index)?
                }
                Rvalue::Copy(value) | Rvalue::NotBool { operand: value, .. } => {
                    read(&definitions, &dominance, value, block_id, index)?
                }
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
