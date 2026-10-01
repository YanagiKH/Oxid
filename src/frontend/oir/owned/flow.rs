use super::super::verify::CfgView;
use super::*;
use budget::{filled, reserve, Meter};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum State {
    Dead,
    Uninitialized,
    Available,
    Moved,
}
impl State {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::Dead,
            1 => Self::Uninitialized,
            2 => Self::Available,
            _ => Self::Moved,
        }
    }
}
fn transition(
    state: State,
    legal: &[State],
    next: State,
    kind: Violation,
    s: Span,
    d: Span,
) -> Result<State, OwnedFailure> {
    if legal.contains(&state) {
        Ok(next)
    } else {
        Err(OwnedFailure::violation(kind, s, Some(d)))
    }
}
fn step(
    f: &RawOwnedFunction,
    id: usize,
    state: State,
    i: &OwnedInstruction,
    s: Span,
) -> Result<State, OwnedFailure> {
    use State::*;
    let declaration = &f.owners[id];
    let owner = OwnerPlaceId(id);
    let mut state = state;
    let (mut event, mut legal, mut next, mut failure) =
        (false, &[][..], state, Violation::Unavailable);
    match i {
        OwnedInstruction::StorageLive(o) if *o == owner => {
            event = true;
            legal = &[Dead];
            next = Uninitialized;
            failure = Violation::Lifetime;
        }
        OwnedInstruction::StorageEnd(o) if *o == owner => {
            event = true;
            legal = &[Uninitialized, Available, Moved];
            next = Dead;
            failure = Violation::Lifetime;
        }
        OwnedInstruction::Construct { destination, .. }
        | OwnedInstruction::MoveInitialize { destination, .. }
            if *destination == owner =>
        {
            event = true;
            legal = &[Uninitialized];
            next = Available;
            failure = Violation::Initialization;
        }
        OwnedInstruction::Replace { destination, .. } if *destination == owner => {
            event = true;
            legal = &[Available, Moved];
            next = Available;
            failure = Violation::Initialization;
        }
        OwnedInstruction::MoveInitialize { source, .. }
        | OwnedInstruction::Replace { source, .. }
        | OwnedInstruction::Discard(source)
        | OwnedInstruction::PrepareOwned { source, .. }
            if *source == owner =>
        {
            event = true;
            legal = &[Available];
            next = Moved;
        }
        OwnedInstruction::ReadField {
            base: AccessBase::Owner(o),
            ..
        }
        | OwnedInstruction::WriteField {
            base: AccessBase::Owner(o),
            ..
        } if *o == owner => {
            event = true;
            legal = &[Available];
            next = Available;
        }
        OwnedInstruction::PrepareBorrow { loan, .. }
            if f.loans[loan.0].authority == AccessBase::Owner(owner) =>
        {
            event = true;
            legal = &[Available];
            next = Available;
        }
        OwnedInstruction::OpenCall(c) if matches!(declaration.kind,OwnerKind::StagedArgument{call,..} if call==*c) =>
        {
            event = true;
            legal = &[Dead];
            next = Uninitialized;
            failure = Violation::Lifetime;
        }
        OwnedInstruction::PrepareOwned { call, argument, .. }
            if declaration.kind
                == (OwnerKind::StagedArgument {
                    call: *call,
                    argument: *argument,
                }) =>
        {
            event = true;
            legal = &[Uninitialized];
            next = Available;
            failure = Violation::Initialization;
        }
        _ => {}
    }
    if event {
        state = transition(state, legal, next, failure, s, declaration.span)?;
    }
    Ok(state)
}
fn end_step(
    f: &RawOwnedFunction,
    id: usize,
    state: State,
    e: &OwnedTerminator,
) -> Result<State, OwnedFailure> {
    use State::*;
    let d = &f.owners[id];
    match e.kind {
        OwnedTerminatorKind::Invoke { call, .. } => match d.kind {
            OwnerKind::StagedArgument { call: c, .. } if c == call => transition(
                state,
                &[Available],
                Dead,
                Violation::Unavailable,
                e.span,
                d.span,
            ),
            OwnerKind::CallResult { call: c } if c == call => transition(
                state,
                &[Dead],
                Available,
                Violation::Lifetime,
                e.span,
                d.span,
            ),
            _ => Ok(state),
        },
        OwnedTerminatorKind::ReturnOwned(o) if o.0 == id => transition(
            state,
            &[Available],
            Moved,
            Violation::Unavailable,
            e.span,
            d.span,
        ),
        _ => Ok(state),
    }
}
fn record_key(s: Span) -> (usize, usize, usize) {
    (s.file.0, s.start, s.end)
}
struct FailureSite {
    error: OwnedFailure,
    block: usize,
    position: usize,
    state: State,
}
/// Saturate all valid state transitions even after finding an invalid one. This
/// both bounds convergence at 4B and keeps diagnostics independent of BFS order.
fn availability(f: &RawOwnedFunction, id: usize, meter: &mut Meter) -> Result<(), OwnedFailure> {
    let mut seen = filled(f.blocks.len(), 0u8)?;
    let mut queue = reserve(budget::mul(4, f.blocks.len())?)?;
    let initial = if matches!(f.owners[id].kind, OwnerKind::Parameter { .. }) {
        State::Available
    } else {
        State::Dead
    };
    seen[f.entry.0] = 1u8 << (initial as u8);
    queue.push(f.entry.0 * 4 + initial as usize);
    let mut cursor = 0;
    let mut failed: Option<FailureSite> = None;
    while cursor < queue.len() {
        let pair = queue[cursor];
        cursor += 1;
        let block = pair / 4;
        let mut state = State::from(pair % 4);
        meter.visit()?;
        let b = &f.blocks[block];
        let mut broken = false;
        for (position, i) in b.statements.iter().enumerate() {
            meter.visit()?;
            match step(f, id, state, &i.kind, i.span) {
                Ok(next) => state = next,
                Err(error) => {
                    select_failure(
                        &mut failed,
                        FailureSite {
                            error,
                            block,
                            position,
                            state,
                        },
                    );
                    broken = true;
                    break;
                }
            }
        }
        if broken {
            continue;
        }
        let e = b
            .terminator
            .as_ref()
            .ok_or_else(|| OwnedFailure::malformed(Malformed::MissingTerminator, b.span))?;
        meter.visit()?;
        match end_step(f, id, state, e) {
            Ok(next) => state = next,
            Err(error) => {
                select_failure(
                    &mut failed,
                    FailureSite {
                        error,
                        block,
                        position: b.statements.len(),
                        state,
                    },
                );
                continue;
            }
        }
        for next in f.successors(block)?.into_iter().flatten() {
            meter.visit()?;
            let bit = 1u8 << (state as u8);
            if seen[next.0] & bit == 0 {
                seen[next.0] |= bit;
                queue.push(next.0 * 4 + state as usize);
            }
        }
    }
    drop(queue);
    if let Some(mut failure) = failed {
        if failure.state == State::Moved {
            failure.error.related = Origin::from(move_origin(
                f,
                id,
                &seen,
                failure.block,
                failure.position,
                meter,
            )?);
        }
        return Err(failure.error);
    }
    Ok(())
}
fn select_failure(best: &mut Option<FailureSite>, candidate: FailureSite) {
    let key = |f: &FailureSite| {
        (
            record_key(f.error.primary.get().unwrap_or(Span {
                file: super::super::super::source::SourceFileId(0),
                start: 0,
                end: 0,
            })),
            f.block,
            f.position,
            f.state as u8,
        )
    };
    if best.as_ref().is_none_or(|b| key(&candidate) < key(b)) {
        *best = Some(candidate);
    }
}
/// Reconstruct only realizable M-state paths. Each predecessor's at-most-four
/// incoming states is replayed for each of its at-most-two outgoing edges.
/// The latest move in that block stops the reverse path, so older generations
/// and unrelated branches cannot become a secondary diagnostic.
fn move_origin(
    f: &RawOwnedFunction,
    id: usize,
    seen: &[u8],
    block: usize,
    position: usize,
    meter: &mut Meter,
) -> Result<Option<Span>, OwnedFailure> {
    let n = f.blocks.len();
    let mut offsets = filled(n + 1, 0usize)?;
    let mut edges = 0;
    for b in 0..n {
        meter.visit()?;
        for to in f.successors(b)?.into_iter().flatten() {
            offsets[to.0 + 1] += 1;
            edges += 1;
        }
    }
    for i in 1..=n {
        offsets[i] += offsets[i - 1];
    }
    let mut insertion = reserve(n)?;
    insertion.extend_from_slice(&offsets[..n]);
    let mut predecessors = filled(edges, 0usize)?;
    for b in 0..n {
        for to in f.successors(b)?.into_iter().flatten() {
            predecessors[insertion[to.0]] = b;
            insertion[to.0] += 1;
        }
    }
    drop(insertion);
    let mut visited = filled(n, false)?;
    let mut queue = reserve(n)?;
    let mut best = None;
    let mut inspect = |b: usize, limit: usize, normal_end: bool| -> Result<bool, OwnedFailure> {
        let mut needs_pred = false;
        for incoming in 0..4 {
            if seen[b] & (1 << incoming) == 0 {
                continue;
            }
            let mut state = State::from(incoming);
            let mut last = None;
            let mut valid = true;
            meter.visit()?;
            for i in f.blocks[b].statements.iter().take(limit) {
                meter.visit()?;
                match step(f, id, state, &i.kind, i.span) {
                    Ok(next) => {
                        if next == State::Moved && state != State::Moved {
                            last = Some(i.span);
                        }
                        if next != State::Moved {
                            last = None;
                        }
                        state = next;
                    }
                    Err(_) => {
                        valid = false;
                        break;
                    }
                }
            }
            if normal_end && valid {
                let e = f.blocks[b]
                    .terminator
                    .as_ref()
                    .ok_or_else(|| OwnedFailure::malformed(Malformed::MissingTerminator, f.span))?;
                meter.visit()?;
                match end_step(f, id, state, e) {
                    Ok(next) => {
                        if next == State::Moved && state != State::Moved {
                            last = Some(e.span);
                        }
                        if next != State::Moved {
                            last = None;
                        }
                        state = next;
                    }
                    Err(_) => valid = false,
                }
            }
            if valid && state == State::Moved {
                if let Some(origin) = last {
                    if best.is_none_or(|s| record_key(origin) < record_key(s)) {
                        best = Some(origin);
                    }
                } else if incoming == State::Moved as usize {
                    needs_pred = true;
                }
            }
        }
        Ok(needs_pred)
    };
    if inspect(block, position, false)? {
        visited[block] = true;
        queue.push(block);
    }
    let mut cursor = 0;
    while cursor < queue.len() {
        let b = queue[cursor];
        cursor += 1;
        for &pred in &predecessors[offsets[b]..offsets[b + 1]] {
            if inspect(pred, f.blocks[pred].statements.len(), true)? && !visited[pred] {
                visited[pred] = true;
                queue.push(pred);
            }
        }
    }
    Ok(best)
}
#[derive(Clone, Copy)]
enum Access {
    Read,
    Write,
    Consume,
    End,
    Borrow(BorrowKind),
}
fn overlap(f: &RawOwnedFunction, a: AccessBase, b: AccessBase) -> bool {
    match (a, b) {
        (AccessBase::Owner(a), AccessBase::Owner(b)) => a == b,
        (AccessBase::Parameter(a), AccessBase::Parameter(b)) => {
            a == b
                || (f.references[a.0].record == f.references[b.0].record
                    && f.references[a.0].kind == BorrowKind::Shared
                    && f.references[b.0].kind == BorrowKind::Shared)
        }
        _ => false,
    }
}
fn accesses(
    f: &RawOwnedFunction,
    i: &OwnedInstruction,
    mut visit: impl FnMut(AccessBase, Access) -> Result<(), OwnedFailure>,
) -> Result<(), OwnedFailure> {
    match i {
        OwnedInstruction::ReadField { base, .. } => visit(*base, Access::Read)?,
        OwnedInstruction::WriteField { base, .. } => visit(*base, Access::Write)?,
        OwnedInstruction::MoveInitialize {
            source,
            destination,
        }
        | OwnedInstruction::Replace {
            source,
            destination,
        } => {
            visit(AccessBase::Owner(*source), Access::Consume)?;
            visit(AccessBase::Owner(*destination), Access::Write)?;
        }
        OwnedInstruction::Construct { destination, .. } => {
            visit(AccessBase::Owner(*destination), Access::Write)?
        }
        OwnedInstruction::Discard(o) | OwnedInstruction::PrepareOwned { source: o, .. } => {
            visit(AccessBase::Owner(*o), Access::Consume)?
        }
        OwnedInstruction::StorageEnd(o) => visit(AccessBase::Owner(*o), Access::End)?,
        OwnedInstruction::PrepareBorrow { loan, .. } => {
            let l = &f.loans[loan.0];
            visit(l.authority, Access::Borrow(l.kind))?;
        }
        _ => {}
    }
    Ok(())
}
fn conflict(kind: BorrowKind, access: Access) -> bool {
    match access {
        Access::Read | Access::Borrow(BorrowKind::Shared) => kind == BorrowKind::Exclusive,
        _ => true,
    }
}
fn permissions(f: &RawOwnedFunction) -> Result<(), OwnedFailure> {
    for b in &f.blocks {
        for i in &b.statements {
            match i.kind {
                OwnedInstruction::Replace { destination, .. }
                    if f.owners[destination.0].kind != (OwnerKind::Local { mutable: true }) =>
                {
                    return Err(OwnedFailure::violation(
                        Violation::Permission,
                        i.span,
                        Some(f.owners[destination.0].span),
                    ))
                }
                OwnedInstruction::WriteField { base, .. } => {
                    if !shape::base(f, base, i.span)?.1 {
                        return Err(OwnedFailure::violation(Violation::Permission, i.span, None));
                    }
                }
                OwnedInstruction::PrepareBorrow { loan, .. } => {
                    let l = &f.loans[loan.0];
                    if l.kind == BorrowKind::Exclusive && !shape::base(f, l.authority, i.span)?.1 {
                        return Err(OwnedFailure::violation(
                            Violation::Permission,
                            i.span,
                            Some(l.span),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}
fn exact(
    f: &RawOwnedFunction,
    initial: usize,
    meter: &mut Meter,
    mut statement: impl FnMut(usize, &OwnedStatement) -> Result<usize, OwnedFailure>,
    mut terminator: impl FnMut(usize, &OwnedTerminator) -> Result<usize, OwnedFailure>,
    kind: Violation,
) -> Result<(), OwnedFailure> {
    let mut state = filled(f.blocks.len(), usize::MAX)?;
    let mut queue = reserve(f.blocks.len())?;
    state[f.entry.0] = initial;
    queue.push(f.entry.0);
    let mut cursor = 0;
    while cursor < queue.len() {
        let b = queue[cursor];
        cursor += 1;
        meter.visit()?;
        let mut s = state[b];
        for i in &f.blocks[b].statements {
            meter.visit()?;
            s = statement(s, i)?;
        }
        let end = f.blocks[b]
            .terminator
            .as_ref()
            .ok_or_else(|| OwnedFailure::malformed(Malformed::MissingTerminator, f.span))?;
        meter.visit()?;
        s = terminator(s, end)?;
        for to in f.successors(b)?.into_iter().flatten() {
            meter.visit()?;
            let old = &mut state[to.0];
            if *old == usize::MAX {
                *old = s;
                queue.push(to.0);
            } else if *old != s {
                return Err(OwnedFailure::violation(kind, f.blocks[to.0].span, None));
            }
        }
    }
    Ok(())
}
fn loans(f: &RawOwnedFunction, id: usize, meter: &mut Meter) -> Result<(), OwnedFailure> {
    let l = &f.loans[id];
    exact(
        f,
        0,
        meter,
        |mut state, i| {
            if matches!(i.kind,OwnedInstruction::PrepareBorrow{loan,..} if loan.0==id) {
                if state != 0 {
                    return Err(OwnedFailure::violation(
                        Violation::LoanRegion,
                        i.span,
                        Some(l.span),
                    ));
                }
                state = 1;
            } else if state == 1 {
                accesses(f, &i.kind, |base, access| {
                    if overlap(f, l.authority, base) && conflict(l.kind, access) {
                        let mut e = OwnedFailure::violation(Violation::LoanConflict, i.span, None);
                        e.related = Origin(l.span);
                        return Err(e);
                    }
                    Ok(())
                })?;
            }
            Ok(state)
        },
        |state, e| match e.kind {
            OwnedTerminatorKind::Invoke { call, .. } if call == l.call => {
                if state != 1 {
                    return Err(OwnedFailure::violation(
                        Violation::LoanRegion,
                        e.span,
                        Some(l.span),
                    ));
                }
                Ok(0)
            }
            OwnedTerminatorKind::ReturnOwned(o) => {
                if state == 1 {
                    let mut error = OwnedFailure::violation(
                        if overlap(f, l.authority, AccessBase::Owner(o)) {
                            Violation::LoanConflict
                        } else {
                            Violation::ActiveAtExit
                        },
                        e.span,
                        None,
                    );
                    error.related = Origin(l.span);
                    Err(error)
                } else {
                    Ok(state)
                }
            }
            OwnedTerminatorKind::ReturnScalar(_) => {
                if state == 1 {
                    Err(OwnedFailure::violation(
                        Violation::ActiveAtExit,
                        e.span,
                        Some(l.span),
                    ))
                } else {
                    Ok(state)
                }
            }
            _ => Ok(state),
        },
        Violation::LoanRegion,
    )
}
fn descendant(shape: &shape::Shape, parent: usize, child: usize) -> bool {
    let p = &shape.calls[parent];
    let c = &shape.calls[child];
    p.enter < c.enter && c.exit < p.exit
}
fn call_regions(
    f: &RawOwnedFunction,
    shape: &shape::Shape,
    id: usize,
    meter: &mut Meter,
) -> Result<(), OwnedFailure> {
    let c = &f.calls[id];
    exact(
        f,
        0,
        meter,
        |mut state, i| {
            match i.kind {
                OwnedInstruction::OpenCall(other) => {
                    // Check immediate parent even when this parent's region is Off.
                    if let Some((parent, argument)) = f.calls[other.0].parent {
                        if parent.0 == id && state != argument + 1 {
                            return Err(OwnedFailure::violation(
                                Violation::CallRegion,
                                i.span,
                                None,
                            ));
                        }
                    }
                    if other.0 == id {
                        if state != 0 {
                            return Err(OwnedFailure::violation(
                                Violation::CallRegion,
                                i.span,
                                None,
                            ));
                        }
                        state = 1;
                    } else if state != 0 && !descendant(shape, id, other.0) {
                        return Err(OwnedFailure::violation(Violation::CallRegion, i.span, None));
                    }
                }
                OwnedInstruction::PrepareScalar { call, argument, .. }
                | OwnedInstruction::PrepareOwned { call, argument, .. }
                | OwnedInstruction::PrepareBorrow { call, argument, .. } => {
                    if call.0 == id {
                        if state != argument + 1 {
                            return Err(OwnedFailure::violation(
                                Violation::CallRegion,
                                i.span,
                                None,
                            ));
                        }
                        state += 1;
                    } else if state != 0 && descendant(shape, call.0, id) {
                        return Err(OwnedFailure::violation(Violation::CallRegion, i.span, None));
                    }
                }
                _ => {}
            }
            Ok(state)
        },
        |state, e| match e.kind {
            OwnedTerminatorKind::Invoke { call, .. } if call.0 == id => {
                if state == c.arguments.len() + 1 {
                    Ok(0)
                } else {
                    Err(OwnedFailure::violation(Violation::CallRegion, e.span, None))
                }
            }
            OwnedTerminatorKind::Invoke { call, .. }
                if state != 0 && descendant(shape, call.0, id) =>
            {
                Err(OwnedFailure::violation(Violation::CallRegion, e.span, None))
            }
            OwnedTerminatorKind::ReturnScalar(_) | OwnedTerminatorKind::ReturnOwned(_)
                if state != 0 =>
            {
                Err(OwnedFailure::violation(
                    Violation::ActiveAtExit,
                    e.span,
                    None,
                ))
            }
            _ => Ok(state),
        },
        Violation::CallRegion,
    )
}
pub(super) fn check(
    f: &RawOwnedFunction,
    shape: &shape::Shape,
    meter: &mut Meter,
) -> Result<(), OwnedFailure> {
    permissions(f)?;
    for id in 0..f.owners.len() {
        availability(f, id, meter)?;
    }
    for id in 0..f.loans.len() {
        loans(f, id, meter)?;
    }
    for id in 0..f.calls.len() {
        call_regions(f, shape, id, meter)?;
    }
    Ok(())
}
