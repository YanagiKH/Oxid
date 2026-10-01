//! Deterministic scalar reference execution. Only an immutable verified witness
//! can enter here. Fuel counts abstract-machine work, not wall-clock time.
use super::*;

const MAX_FUEL: usize = 1_000_000;
const MAX_FRAMES: usize = 1_024;
const MAX_LIVE_SLOTS: usize = 200_000;
#[derive(Clone, Copy)]
struct Limits {
    fuel: usize,
    frames: usize,
    slots: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            fuel: MAX_FUEL,
            frames: MAX_FRAMES,
            slots: MAX_LIVE_SLOTS,
        }
    }
}
#[derive(Clone, Copy)]
struct Resume {
    destination: LocalId,
    continuation: BlockId,
    origin: Span,
}
struct Frame {
    function: hir::DefId,
    block: BlockId,
    next: usize,
    slots: Vec<Option<Scalar>>,
    return_to: Option<Resume>,
}
fn internal(kind: FailureKind, span: Option<Span>) -> RunFailure {
    RunFailure::Internal(OirFailure::new(kind, "oir-run", span))
}
fn add(left: usize, right: usize, span: Span) -> Result<usize, RunFailure> {
    left.checked_add(right)
        .ok_or_else(|| internal(FailureKind::Accounting, Some(span)))
}
fn function(
    program: &VerifiedProgram,
    id: hir::DefId,
    origin: Option<Span>,
) -> Result<&Function, RunFailure> {
    program
        .program
        .functions
        .get(id.0)
        .filter(|f| f.id == id)
        .ok_or_else(|| internal(FailureKind::InvalidFunctionId, origin))
}
fn read(frame: &Frame, operand: Operand) -> Result<Scalar, RunFailure> {
    frame
        .slots
        .get(operand.local.0)
        .ok_or_else(|| internal(FailureKind::InvalidLocal, Some(operand.span)))?
        .ok_or_else(|| internal(FailureKind::Uninitialized, Some(operand.span)))
}
fn write(
    frame: &mut Frame,
    function: &Function,
    destination: LocalId,
    value: Scalar,
    origin: Span,
) -> Result<(), RunFailure> {
    let local = function
        .locals
        .get(destination.0)
        .ok_or_else(|| internal(FailureKind::InvalidLocal, Some(origin)))?;
    if local.ty != value.ty() {
        return Err(internal(FailureKind::TypeMismatch, Some(origin)));
    }
    let slot = frame
        .slots
        .get_mut(destination.0)
        .ok_or_else(|| internal(FailureKind::InvalidLocal, Some(origin)))?;
    if slot.is_some() {
        return Err(internal(FailureKind::AlreadyInitialized, Some(origin)));
    }
    *slot = Some(value);
    Ok(())
}
fn arguments(function: &Function, args: &[Scalar], origin: Span) -> Result<(), RunFailure> {
    if args.len() != function.param_count || args.len() > super::super::parser::MAX_PARAMS {
        return Err(internal(FailureKind::Arity, Some(origin)));
    }
    for (index, value) in args.iter().enumerate() {
        let local = function
            .locals
            .get(index)
            .ok_or_else(|| internal(FailureKind::InvalidLocal, Some(origin)))?;
        if local.kind != LocalKind::Parameter || local.ty != value.ty() {
            return Err(internal(FailureKind::TypeMismatch, Some(origin)));
        }
    }
    Ok(())
}
fn frame(
    function: &Function,
    args: &[Scalar],
    return_to: Option<Resume>,
) -> Result<Frame, RunFailure> {
    arguments(function, args, function.span)?;
    let mut frame = Frame {
        function: function.id,
        block: function.entry,
        next: 0,
        slots: vec![None; function.locals.len()],
        return_to,
    };
    for (index, &value) in args.iter().enumerate() {
        write(&mut frame, function, LocalId(index), value, function.span)?;
    }
    Ok(frame)
}
fn charge(fuel: &mut usize, cost: usize, origin: Span) -> Result<(), RunFailure> {
    *fuel = fuel.checked_sub(cost).ok_or(RunFailure::Fuel(origin))?;
    Ok(())
}
// Only fuel accounting changes before this preflight succeeds; no activation
// state changes or variable-sized allocation/read loop starts beforehand. Fuel, frame, slot failure precedence is deliberate.
fn preflight(
    fuel: &mut usize,
    cost: usize,
    frames: usize,
    slots: usize,
    locals: usize,
    limits: Limits,
    origin: Span,
) -> Result<usize, RunFailure> {
    charge(fuel, cost, origin)?;
    if add(frames, 1, origin)? > limits.frames {
        return Err(RunFailure::Frames(origin));
    }
    let next_slots = add(slots, locals, origin)?;
    if next_slots > limits.slots {
        return Err(RunFailure::Slots(origin));
    }
    Ok(next_slots)
}

pub(super) fn run(program: &VerifiedProgram, entry: hir::DefId) -> Result<Scalar, RunFailure> {
    invoke(program, entry, &[], Limits::default())
}
// Private invocation seam validates IDs, arity and scalar types independently of
// the source entry adapter. Tests may only lower the absolute resource ceilings.
fn invoke(
    program: &VerifiedProgram,
    entry: hir::DefId,
    args: &[Scalar],
    limits: Limits,
) -> Result<Scalar, RunFailure> {
    execute(
        program,
        entry,
        args,
        limits,
        #[cfg(test)]
        &mut |_| {},
    )
}
// The observation seam and event types are compiled only into unit tests. They
// compare source-model call/branch/return traces without a public tracing API.
#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
enum Event {
    Enter(hir::DefId),
    Branch(hir::DefId, bool),
    Return(hir::DefId, Scalar),
}
fn execute(
    program: &VerifiedProgram,
    entry: hir::DefId,
    args: &[Scalar],
    limits: Limits,
    #[cfg(test)] observe: &mut dyn FnMut(Event),
) -> Result<Scalar, RunFailure> {
    let limits = Limits {
        fuel: limits.fuel.min(MAX_FUEL),
        frames: limits.frames.min(MAX_FRAMES),
        slots: limits.slots.min(MAX_LIVE_SLOTS),
    };
    let entry = function(program, entry, None)?;
    arguments(entry, args, entry.span)?;
    let mut fuel = limits.fuel;
    let mut live_slots = preflight(
        &mut fuel,
        add(1, entry.locals.len(), entry.span)?,
        0,
        0,
        entry.locals.len(),
        limits,
        entry.span,
    )?;
    // Header capacity is bounded separately from live scalar slot storage.
    let mut frames = Vec::with_capacity(limits.frames);
    frames.push(frame(entry, args, None)?);
    #[cfg(test)]
    observe(Event::Enter(entry.id));
    loop {
        let active = frames
            .last_mut()
            .ok_or_else(|| internal(FailureKind::CallFrame, None))?;
        let current = function(program, active.function, None)?;
        let block = current
            .blocks
            .get(active.block.0)
            .ok_or_else(|| internal(FailureKind::InvalidBlock, Some(current.span)))?;
        if let Some(assign) = block.statements.get(active.next) {
            charge(&mut fuel, 1, assign.span)?;
            let value = match assign.value {
                Rvalue::Bool(value) => Scalar::Bool(value),
                Rvalue::I32(value) => Scalar::I32(value),
                Rvalue::Unit => Scalar::Unit,
                Rvalue::Copy(operand) => read(active, operand)?,
            };
            write(active, current, assign.destination, value, assign.span)?;
            active.next = add(active.next, 1, assign.span)?;
            continue;
        }
        if active.next != block.statements.len() {
            return Err(internal(FailureKind::InvalidBlock, Some(block.span)));
        }
        let end = block
            .terminator
            .as_ref()
            .ok_or_else(|| internal(FailureKind::MissingTerminator, Some(block.span)))?;
        match &end.kind {
            TerminatorKind::Call {
                target,
                args,
                destination,
                continuation,
            } => {
                let callee = function(program, *target, Some(end.span))?;
                let cost = add(add(1, args.len(), end.span)?, callee.locals.len(), end.span)?;
                // Read the length without traversing/copying the suspended stack.
                let count = frames.len();
                let next_slots = preflight(
                    &mut fuel,
                    cost,
                    count,
                    live_slots,
                    callee.locals.len(),
                    limits,
                    end.span,
                )?;
                let active = frames
                    .last()
                    .ok_or_else(|| internal(FailureKind::CallFrame, Some(end.span)))?;
                let values = args
                    .iter()
                    .map(|&arg| read(active, arg))
                    .collect::<Result<Vec<_>, _>>()?;
                let child = frame(
                    callee,
                    &values,
                    Some(Resume {
                        destination: *destination,
                        continuation: *continuation,
                        origin: end.span,
                    }),
                )?;
                frames.push(child);
                #[cfg(test)]
                observe(Event::Enter(callee.id));
                live_slots = next_slots;
            }
            TerminatorKind::Branch {
                condition,
                then_block,
                else_block,
            } => {
                charge(&mut fuel, 1, end.span)?;
                let Scalar::Bool(value) = read(active, *condition)? else {
                    return Err(internal(FailureKind::TypeMismatch, Some(condition.span)));
                };
                #[cfg(test)]
                observe(Event::Branch(current.id, value));
                active.block = if value { *then_block } else { *else_block };
                active.next = 0;
            }
            TerminatorKind::Goto { target } => {
                charge(&mut fuel, 1, end.span)?;
                active.block = *target;
                active.next = 0;
            }
            TerminatorKind::Return(operand) => {
                charge(&mut fuel, 1, end.span)?;
                let value = read(active, *operand)?;
                if value.ty() != current.result {
                    return Err(internal(FailureKind::TypeMismatch, Some(end.span)));
                }
                #[cfg(test)]
                observe(Event::Return(current.id, value));
                let completed = frames
                    .pop()
                    .ok_or_else(|| internal(FailureKind::CallFrame, Some(end.span)))?;
                live_slots = live_slots
                    .checked_sub(completed.slots.len())
                    .ok_or_else(|| internal(FailureKind::Accounting, Some(end.span)))?;
                match (frames.last_mut(), completed.return_to) {
                    (Some(caller), Some(resume)) => {
                        let parent = function(program, caller.function, Some(resume.origin))?;
                        write(caller, parent, resume.destination, value, resume.origin)?;
                        caller.block = resume.continuation;
                        caller.next = 0;
                    }
                    (None, None) if live_slots == 0 => return Ok(value),
                    _ => return Err(internal(FailureKind::CallFrame, Some(end.span))),
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "execute_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "source_oracle.rs"]
mod source_oracle;
