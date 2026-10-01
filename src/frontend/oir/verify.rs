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
/// finally walk the unique intraprocedural continuation chain.
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
                    Rvalue::Unit => hir::Ty::Unit,
                    Rvalue::Copy(value) => operand(function, value, sources)?,
                };
                same_type(actual, expected, assign.span)?;
            }
            let end = terminator(block)?;
            span(sources, end.span)?;
            match &end.kind {
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
        chain(function)?;
    }
    Ok(VerifiedProgram { program })
}
fn read(initialized: &[bool], value: Operand) -> Result<(), OirFailure> {
    match initialized.get(value.local.0) {
        Some(true) => Ok(()),
        Some(false) => Err(failure(FailureKind::Uninitialized, value.span)),
        None => Err(failure(FailureKind::InvalidLocal, value.span)),
    }
}
fn initialize(
    initialized: &mut [bool],
    destination: LocalId,
    span: Span,
) -> Result<(), OirFailure> {
    let state = initialized
        .get_mut(destination.0)
        .ok_or_else(|| failure(FailureKind::InvalidLocal, span))?;
    if *state {
        return Err(failure(FailureKind::AlreadyInitialized, span));
    }
    *state = true;
    Ok(())
}
fn chain(function: &Function) -> Result<(), OirFailure> {
    // One bitmap per function, never one per block or call. Call graph edges are
    // not CFG edges; recursive callees do not cause recursive compiler traversal.
    let mut initialized: Vec<bool> = (0..function.locals.len())
        .map(|index| index < function.param_count)
        .collect();
    let mut visited = vec![false; function.blocks.len()];
    let mut current = function.entry;
    loop {
        let seen = visited
            .get_mut(current.0)
            .ok_or_else(|| failure(FailureKind::InvalidBlock, function.span))?;
        if *seen {
            return Err(failure(FailureKind::Cycle, function.span));
        }
        *seen = true;
        let block = function
            .blocks
            .get(current.0)
            .ok_or_else(|| failure(FailureKind::InvalidBlock, function.span))?;
        for assign in &block.statements {
            if let Rvalue::Copy(value) = assign.value {
                read(&initialized, value)?;
            }
            initialize(&mut initialized, assign.destination, assign.span)?;
        }
        let end = terminator(block)?;
        match &end.kind {
            TerminatorKind::Return(value) => {
                read(&initialized, *value)?;
                break;
            }
            TerminatorKind::Call {
                args,
                destination,
                continuation,
                ..
            } => {
                for arg in args {
                    read(&initialized, *arg)?;
                }
                // The destination is available only on the normal continuation,
                // after every argument read, never to earlier uses or its own args.
                initialize(&mut initialized, *destination, end.span)?;
                current = *continuation;
            }
        }
    }
    if visited.iter().any(|seen| !seen) {
        return Err(failure(FailureKind::Unreachable, function.span));
    }
    Ok(())
}
