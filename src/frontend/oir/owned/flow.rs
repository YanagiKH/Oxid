use super::super::verify::CfgView;
use super::*;
use budget::{filled, reserve, Meter};
/// Immutable diagnostic facts from the exact verifier-denied raw access.
/// The private context constructor cannot be replaced by producer-supplied tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DenialContext {
    facts: DenialFacts,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DenialFacts {
    pub operation: DeniedOperation,
    pub role: DeniedRole,
    pub subject: DeniedSubject,
    pub state: ObservedState,
    pub counterpart: Option<OwnerSubject>,
    pub requested_borrow: Option<BorrowKind>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OwnerSubject {
    pub id: OwnerPlaceId,
    aggregate: AggregateSlot,
    pub class: OwnerKind,
    pub declaration: Span,
}
impl OwnerSubject {
    pub(super) fn aggregate(self) -> AggregateTy {
        self.aggregate.aggregate()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeniedSubject {
    Owner(OwnerSubject),
    Reference {
        id: ReferenceParamId,
        referent: BorrowedSlot,
        granted: BorrowKind,
        declaration: Span,
    },
}
impl DeniedSubject {
    pub(super) fn referent(self) -> BorrowedTy {
        match self {
            Self::Owner(owner) => BorrowedTy::Exact(owner.aggregate()),
            Self::Reference { referent, .. } => referent.referent(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ObservedState {
    Dead,
    Uninitialized,
    Available,
    Moved,
    NotObserved,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeniedOperation {
    StorageLive,
    StorageEnd,
    Construct,
    ConstructArray,
    ConstructEnum,
    ConsumeVariant,
    MatchDispatch,
    MoveInitialize,
    Replace,
    Discard,
    ReadField,
    WriteField,
    ReadIndex,
    WriteIndex,
    ArrayLength,
    OpenCall,
    PrepareScalar,
    PrepareOwned,
    PrepareBorrow,
    Invoke,
    ReturnOwned,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeniedRole {
    Storage,
    InitializationDestination,
    SourceConsume,
    ReplacementDestination,
    FieldBase,
    ArrayBase,
    BorrowAuthority,
    StagedInput,
    CallResult,
    ReturnValue,
    MatchSource,
}
impl DenialContext {
    pub(super) fn facts(&self) -> DenialFacts {
        self.facts
    }
    fn owner(f: &RawOwnedFunction, id: OwnerPlaceId) -> Option<OwnerSubject> {
        let owner = f.owners.get(id.0)?;
        Some(OwnerSubject {
            id,
            aggregate: owner.aggregate,
            class: owner.kind,
            declaration: owner.span,
        })
    }
    fn subject(f: &RawOwnedFunction, base: AccessBase) -> Option<DeniedSubject> {
        Some(match base {
            AccessBase::Owner(id) => DeniedSubject::Owner(Self::owner(f, id)?),
            AccessBase::Parameter(id) => {
                let r = f.references.get(id.0)?;
                DeniedSubject::Reference {
                    id,
                    referent: r.referent,
                    granted: r.kind,
                    declaration: r.span,
                }
            }
        })
    }
    /// Only listed operation/operand-role pairs can create context. Identity is
    /// checked against that operand, including both sides of a move/replace.
    fn instruction(
        f: &RawOwnedFunction,
        i: &OwnedInstruction,
        base: AccessBase,
        role: DeniedRole,
        state: ObservedState,
    ) -> Option<Self> {
        use DeniedRole::*;
        let mut counterpart = None;
        let mut requested_borrow = None;
        let operation = match (i, role) {
            (OwnedInstruction::StorageLive(o), Storage) if base == AccessBase::Owner(*o) => {
                DeniedOperation::StorageLive
            }
            (OwnedInstruction::StorageEnd(o), Storage) if base == AccessBase::Owner(*o) => {
                DeniedOperation::StorageEnd
            }
            (OwnedInstruction::Construct { destination, .. } | OwnedInstruction::ConstructComposite { destination, .. }, InitializationDestination)
                if base == AccessBase::Owner(*destination) =>
            {
                DeniedOperation::Construct
            }
            (OwnedInstruction::ConstructComposite { destination, fields }, SourceConsume)
                if fields.iter().any(|(_, value)| matches!(value, FieldInitializer::Owned(source) if base == AccessBase::Owner(*source))) => {
                counterpart = Some(Self::owner(f, *destination)?);
                DeniedOperation::Construct
            }
            (OwnedInstruction::ConstructEnum { destination, .. }, InitializationDestination)
                if base == AccessBase::Owner(*destination) => DeniedOperation::ConstructEnum,
            (OwnedInstruction::ConsumeVariant { match_id, .. }, SourceConsume)
                if base == AccessBase::Owner(f.matches.get(match_id.0)?.source) => DeniedOperation::ConsumeVariant,
            (OwnedInstruction::ConstructArray { destination, .. }, InitializationDestination)
                if base == AccessBase::Owner(*destination) =>
            {
                DeniedOperation::ConstructArray
            }
            (
                OwnedInstruction::MoveInitialize {
                    source,
                    destination,
                },
                SourceConsume,
            ) if base == AccessBase::Owner(*source) => {
                counterpart = Some(Self::owner(f, *destination)?);
                DeniedOperation::MoveInitialize
            }
            (
                OwnedInstruction::MoveInitialize {
                    source,
                    destination,
                },
                InitializationDestination,
            ) if base == AccessBase::Owner(*destination) => {
                counterpart = Some(Self::owner(f, *source)?);
                DeniedOperation::MoveInitialize
            }
            (
                OwnedInstruction::Replace {
                    source,
                    destination,
                },
                SourceConsume,
            ) if base == AccessBase::Owner(*source) => {
                counterpart = Some(Self::owner(f, *destination)?);
                DeniedOperation::Replace
            }
            (
                OwnedInstruction::Replace {
                    source,
                    destination,
                },
                ReplacementDestination,
            ) if base == AccessBase::Owner(*destination) => {
                counterpart = Some(Self::owner(f, *source)?);
                DeniedOperation::Replace
            }
            (OwnedInstruction::Discard(o), SourceConsume) if base == AccessBase::Owner(*o) => {
                DeniedOperation::Discard
            }
            (OwnedInstruction::ReadField { base: actual, .. } | OwnedInstruction::ReadProjection { base: actual, .. } | OwnedInstruction::ProjectionLength { base: actual, .. }, FieldBase) if base == *actual => {
                DeniedOperation::ReadField
            }
            (OwnedInstruction::WriteField { base: actual, .. } | OwnedInstruction::WriteProjection { base: actual, .. }, FieldBase) if base == *actual => {
                DeniedOperation::WriteField
            }
            (OwnedInstruction::ReadIndex { base: actual, .. }, ArrayBase) if base == *actual => {
                DeniedOperation::ReadIndex
            }
            (OwnedInstruction::WriteIndex { base: actual, .. }, ArrayBase) if base == *actual => {
                DeniedOperation::WriteIndex
            }
            (OwnedInstruction::ArrayLength { base: actual, .. }, ArrayBase) if base == *actual => {
                DeniedOperation::ArrayLength
            }
            (OwnedInstruction::PrepareBorrow { loan, .. }, BorrowAuthority) => {
                let loan = f.loans.get(loan.0)?;
                if base != loan.authority {
                    return None;
                }
                requested_borrow = Some(loan.kind);
                DeniedOperation::PrepareBorrow
            }
            (OwnedInstruction::PrepareOwned { source, .. }, SourceConsume)
                if base == AccessBase::Owner(*source) =>
            {
                DeniedOperation::PrepareOwned
            }
            (OwnedInstruction::PrepareOwned { call, argument, .. }, StagedInput) => {
                let Some(ArgumentSlot::Owned(owner)) =
                    f.calls.get(call.0)?.arguments.get(*argument)
                else {
                    return None;
                };
                if base != AccessBase::Owner(*owner) {
                    return None;
                }
                DeniedOperation::PrepareOwned
            }
            (OwnedInstruction::OpenCall(call), StagedInput) => {
                let AccessBase::Owner(owner) = base else {
                    return None;
                };
                if !matches!(f.owners.get(owner.0)?.kind, OwnerKind::StagedArgument { call: actual, .. } if actual == *call)
                {
                    return None;
                }
                DeniedOperation::OpenCall
            }
            _ => return None,
        };
        Some(Self {
            facts: DenialFacts {
                operation,
                role,
                subject: Self::subject(f, base)?,
                state,
                counterpart,
                requested_borrow,
            },
        })
    }
    fn terminator(
        f: &RawOwnedFunction,
        e: &OwnedTerminatorKind,
        id: OwnerPlaceId,
        role: DeniedRole,
        state: ObservedState,
    ) -> Option<Self> {
        let owner = Self::owner(f, id)?;
        let operation = match (e, role) {
            (OwnedTerminatorKind::MatchDispatch { match_id, .. }, DeniedRole::MatchSource)
                if f.matches.get(match_id.0)?.source == id =>
            {
                DeniedOperation::MatchDispatch
            }
            (OwnedTerminatorKind::Invoke { call, .. }, DeniedRole::StagedInput) if matches!(owner.class, OwnerKind::StagedArgument { call: actual, .. } if actual == *call) => {
                DeniedOperation::Invoke
            }
            (OwnedTerminatorKind::Invoke { call, .. }, DeniedRole::CallResult)
                if owner.class == (OwnerKind::CallResult { call: *call }) =>
            {
                DeniedOperation::Invoke
            }
            (OwnedTerminatorKind::ReturnOwned(actual), DeniedRole::ReturnValue)
                if *actual == id =>
            {
                DeniedOperation::ReturnOwned
            }
            _ => return None,
        };
        Some(Self {
            facts: DenialFacts {
                operation,
                role,
                subject: DeniedSubject::Owner(owner),
                state,
                counterpart: None,
                requested_borrow: None,
            },
        })
    }
}
impl From<State> for ObservedState {
    fn from(state: State) -> Self {
        match state {
            State::Dead => Self::Dead,
            State::Uninitialized => Self::Uninitialized,
            State::Available => Self::Available,
            State::Moved => Self::Moved,
        }
    }
}
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
    instruction: &OwnedStatement,
) -> Result<State, OwnedFailure> {
    let i = &instruction.kind;
    let s = instruction.primary_span();
    use State::*;
    let declaration = &f.owners[id];
    let owner = OwnerPlaceId(id);
    let mut state = state;
    let mut role = DeniedRole::Storage;
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
        | OwnedInstruction::ConstructArray { destination, .. }
        | OwnedInstruction::ConstructEnum { destination, .. }
        | OwnedInstruction::ReadStdin { destination, .. }
        | OwnedInstruction::ConstructComposite { destination, .. }
        | OwnedInstruction::MoveInitialize { destination, .. }
            if *destination == owner =>
        {
            event = true;
            legal = &[Uninitialized];
            next = Available;
            failure = Violation::Initialization;
            role = DeniedRole::InitializationDestination;
        }
        OwnedInstruction::ConsumeVariant { match_id, .. }
            if f.matches[match_id.0].source == owner =>
        {
            event = true;
            legal = &[Available];
            next = Moved;
            role = DeniedRole::SourceConsume;
        }
        OwnedInstruction::ConstructComposite { fields, .. }
            if fields.iter().any(
                |(_, value)| matches!(value, FieldInitializer::Owned(source) if *source == owner),
            ) =>
        {
            event = true;
            legal = &[Available];
            next = Moved;
            role = DeniedRole::SourceConsume;
        }
        OwnedInstruction::Replace { destination, .. } if *destination == owner => {
            event = true;
            legal = &[Available, Moved];
            next = Available;
            failure = Violation::Initialization;
            role = DeniedRole::ReplacementDestination;
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
            role = DeniedRole::SourceConsume;
        }
        OwnedInstruction::ReadProjection {
            base: AccessBase::Owner(o),
            ..
        }
        | OwnedInstruction::WriteProjection {
            base: AccessBase::Owner(o),
            ..
        }
        | OwnedInstruction::ProjectionLength {
            base: AccessBase::Owner(o),
            ..
        }
        | OwnedInstruction::ReadField {
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
            role = DeniedRole::FieldBase;
        }
        OwnedInstruction::ReadIndex {
            base: AccessBase::Owner(o),
            ..
        }
        | OwnedInstruction::WriteIndex {
            base: AccessBase::Owner(o),
            ..
        }
        | OwnedInstruction::ArrayLength {
            base: AccessBase::Owner(o),
            ..
        } if *o == owner => {
            event = true;
            legal = &[Available];
            next = Available;
            role = DeniedRole::ArrayBase;
        }
        OwnedInstruction::PrepareBorrow { loan, .. }
            if f.loans[loan.0].authority == AccessBase::Owner(owner) =>
        {
            event = true;
            legal = &[Available];
            next = Available;
            role = DeniedRole::BorrowAuthority;
        }
        OwnedInstruction::OpenCall(c) if matches!(declaration.kind,OwnerKind::StagedArgument{call,..} if call==*c) =>
        {
            event = true;
            legal = &[Dead];
            next = Uninitialized;
            failure = Violation::Lifetime;
            role = DeniedRole::StagedInput;
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
            role = DeniedRole::StagedInput;
        }
        _ => {}
    }
    if event {
        state =
            transition(state, legal, next, failure, s, declaration.span).map_err(|mut error| {
                error.context =
                    DenialContext::instruction(f, i, AccessBase::Owner(owner), role, state.into());
                error
            })?;
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
    let (legal, next, kind, role): (&[State], State, Violation, DeniedRole) = match e.kind {
        OwnedTerminatorKind::MatchDispatch { match_id, .. }
            if f.matches[match_id.0].source.0 == id =>
        {
            (
                &[Available],
                Available,
                Violation::Unavailable,
                DeniedRole::MatchSource,
            )
        }
        OwnedTerminatorKind::Invoke { call, .. } => match d.kind {
            OwnerKind::StagedArgument { call: c, .. } if c == call => (
                &[Available],
                Dead,
                Violation::Unavailable,
                DeniedRole::StagedInput,
            ),
            OwnerKind::CallResult { call: c } if c == call => (
                &[Dead],
                Available,
                Violation::Lifetime,
                DeniedRole::CallResult,
            ),
            _ => return Ok(state),
        },
        OwnedTerminatorKind::ReturnOwned(o) if o.0 == id => (
            &[Available],
            Moved,
            Violation::Unavailable,
            DeniedRole::ReturnValue,
        ),
        _ => return Ok(state),
    };
    transition(state, legal, next, kind, e.primary_span(), d.span).map_err(|mut error| {
        error.context = DenialContext::terminator(f, &e.kind, OwnerPlaceId(id), role, state.into());
        error
    })
}
fn record_key(s: Span) -> (usize, usize, usize) {
    (s.file.0, s.start, s.end)
}
struct FailureSite {
    error: OwnedFailure,
    operation_span: Span,
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
            match step(f, id, state, i) {
                Ok(next) => state = next,
                Err(error) => {
                    select_failure(
                        &mut failed,
                        FailureSite {
                            error,
                            operation_span: i.span,
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
                        operation_span: e.span,
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
            record_key(f.operation_span),
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
                match step(f, id, state, i) {
                    Ok(next) => {
                        if next == State::Moved && state != State::Moved {
                            last = Some(i.cause_span());
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
                            last = Some(e.cause_span());
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
fn views_may_alias(a: BorrowedTy, b: BorrowedTy) -> bool {
    a.accepts(b) || b.accepts(a)
}
fn overlap(f: &RawOwnedFunction, a: AccessBase, b: AccessBase) -> bool {
    match (a, b) {
        (AccessBase::Owner(a), AccessBase::Owner(b)) => a == b,
        (AccessBase::Parameter(a), AccessBase::Parameter(b)) => {
            a == b
                || (views_may_alias(f.references[a.0].referent(), f.references[b.0].referent())
                    && f.references[a.0].kind == BorrowKind::Shared
                    && f.references[b.0].kind == BorrowKind::Shared)
        }
        _ => false,
    }
}
fn accesses(
    f: &RawOwnedFunction,
    i: &OwnedInstruction,
    mut visit: impl FnMut(AccessBase, Access, DeniedRole) -> Result<(), OwnedFailure>,
) -> Result<(), OwnedFailure> {
    match i {
        OwnedInstruction::ReadStdin {
            buffer,
            destination,
        } => {
            visit(
                AccessBase::Parameter(*buffer),
                Access::Write,
                DeniedRole::ArrayBase,
            )?;
            visit(
                AccessBase::Owner(*destination),
                Access::Write,
                DeniedRole::InitializationDestination,
            )?;
        }
        OwnedInstruction::ReadField { base, .. }
        | OwnedInstruction::ReadProjection { base, .. }
        | OwnedInstruction::ProjectionLength { base, .. } => {
            visit(*base, Access::Read, DeniedRole::FieldBase)?
        }
        OwnedInstruction::WriteField { base, .. }
        | OwnedInstruction::WriteProjection { base, .. } => {
            visit(*base, Access::Write, DeniedRole::FieldBase)?
        }
        OwnedInstruction::ReadIndex { base, .. } | OwnedInstruction::ArrayLength { base, .. } => {
            visit(*base, Access::Read, DeniedRole::ArrayBase)?
        }
        OwnedInstruction::WriteIndex { base, .. } => {
            visit(*base, Access::Write, DeniedRole::ArrayBase)?
        }
        OwnedInstruction::MoveInitialize {
            source,
            destination,
        }
        | OwnedInstruction::Replace {
            source,
            destination,
        } => {
            visit(
                AccessBase::Owner(*source),
                Access::Consume,
                DeniedRole::SourceConsume,
            )?;
            visit(
                AccessBase::Owner(*destination),
                Access::Write,
                if matches!(i, OwnedInstruction::Replace { .. }) {
                    DeniedRole::ReplacementDestination
                } else {
                    DeniedRole::InitializationDestination
                },
            )?;
        }
        OwnedInstruction::ConstructComposite {
            destination,
            fields,
        } => {
            for (_, value) in fields {
                if let FieldInitializer::Owned(source) = value {
                    visit(
                        AccessBase::Owner(*source),
                        Access::Consume,
                        DeniedRole::SourceConsume,
                    )?;
                }
            }
            visit(
                AccessBase::Owner(*destination),
                Access::Write,
                DeniedRole::InitializationDestination,
            )?;
        }
        OwnedInstruction::Construct { destination, .. }
        | OwnedInstruction::ConstructArray { destination, .. }
        | OwnedInstruction::ConstructEnum { destination, .. } => visit(
            AccessBase::Owner(*destination),
            Access::Write,
            DeniedRole::InitializationDestination,
        )?,
        OwnedInstruction::ConsumeVariant { match_id, .. } => visit(
            AccessBase::Owner(f.matches[match_id.0].source),
            Access::Consume,
            DeniedRole::SourceConsume,
        )?,
        OwnedInstruction::Discard(o) | OwnedInstruction::PrepareOwned { source: o, .. } => visit(
            AccessBase::Owner(*o),
            Access::Consume,
            DeniedRole::SourceConsume,
        )?,
        OwnedInstruction::StorageEnd(o) => {
            visit(AccessBase::Owner(*o), Access::End, DeniedRole::Storage)?
        }
        OwnedInstruction::PrepareBorrow { loan, .. } => {
            let l = &f.loans[loan.0];
            visit(
                l.authority,
                Access::Borrow(l.kind),
                DeniedRole::BorrowAuthority,
            )?;
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
            let denied = match i.kind {
                OwnedInstruction::Replace { destination, .. }
                    if f.owners[destination.0].kind != (OwnerKind::Local { mutable: true }) =>
                {
                    Some((
                        AccessBase::Owner(destination),
                        DeniedRole::ReplacementDestination,
                        Some(f.owners[destination.0].span),
                    ))
                }
                OwnedInstruction::WriteField { base, .. }
                | OwnedInstruction::WriteProjection { base, .. }
                    if !shape::base(f, base, i.span)?.1 =>
                {
                    Some((base, DeniedRole::FieldBase, None))
                }
                OwnedInstruction::WriteIndex { base, .. } if !shape::base(f, base, i.span)?.1 => {
                    Some((base, DeniedRole::ArrayBase, None))
                }
                OwnedInstruction::PrepareBorrow { loan, .. } => {
                    let l = &f.loans[loan.0];
                    (l.kind == BorrowKind::Exclusive && !shape::base(f, l.authority, i.span)?.1)
                        .then_some((l.authority, DeniedRole::BorrowAuthority, Some(l.span)))
                }
                _ => None,
            };
            if let Some((base, role, declaration)) = denied {
                let mut error =
                    OwnedFailure::violation(Violation::Permission, i.primary_span(), declaration);
                error.context =
                    DenialContext::instruction(f, &i.kind, base, role, ObservedState::NotObserved);
                return Err(error);
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
fn loans(
    f: &RawOwnedFunction,
    shape: &shape::Shape,
    id: usize,
    meter: &mut Meter,
) -> Result<(), OwnedFailure> {
    let l = &f.loans[id];
    let cause = shape.acquisition_cause(f, LoanId(id))?;
    exact(
        f,
        0,
        meter,
        |mut state, i| {
            if matches!(i.kind,OwnedInstruction::PrepareBorrow{loan,..} if loan.0==id) {
                if state != 0 {
                    return Err(OwnedFailure::violation(
                        Violation::LoanRegion,
                        i.primary_span(),
                        Some(l.span),
                    ));
                }
                state = 1;
            } else if state == 1 {
                accesses(f, &i.kind, |base, access, role| {
                    if overlap(f, l.authority, base) && conflict(l.kind, access) {
                        let mut e = OwnedFailure::violation(
                            Violation::LoanConflict,
                            i.primary_span(),
                            None,
                        );
                        e.related = Origin(cause);
                        e.context = DenialContext::instruction(
                            f,
                            &i.kind,
                            base,
                            role,
                            ObservedState::NotObserved,
                        );
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
                        e.primary_span(),
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
                        e.primary_span(),
                        None,
                    );
                    error.related = Origin(cause);
                    if error.kind == OwnedFailureKind::Ownership(Violation::LoanConflict) {
                        error.context = DenialContext::terminator(
                            f,
                            &e.kind,
                            o,
                            DeniedRole::ReturnValue,
                            ObservedState::NotObserved,
                        );
                    }
                    Err(error)
                } else {
                    Ok(state)
                }
            }
            OwnedTerminatorKind::ReturnScalar(_) => {
                if state == 1 {
                    Err(OwnedFailure::violation(
                        Violation::ActiveAtExit,
                        e.primary_span(),
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
                                i.primary_span(),
                                None,
                            ));
                        }
                    }
                    if other.0 == id {
                        if state != 0 {
                            return Err(OwnedFailure::violation(
                                Violation::CallRegion,
                                i.primary_span(),
                                None,
                            ));
                        }
                        state = 1;
                    } else if state != 0 && !descendant(shape, id, other.0) {
                        return Err(OwnedFailure::violation(
                            Violation::CallRegion,
                            i.primary_span(),
                            None,
                        ));
                    }
                }
                OwnedInstruction::PrepareScalar { call, argument, .. }
                | OwnedInstruction::PrepareOwned { call, argument, .. }
                | OwnedInstruction::PrepareBorrow { call, argument, .. } => {
                    if call.0 == id {
                        if state != argument + 1 {
                            return Err(OwnedFailure::violation(
                                Violation::CallRegion,
                                i.primary_span(),
                                None,
                            ));
                        }
                        state += 1;
                    } else if state != 0 && descendant(shape, call.0, id) {
                        return Err(OwnedFailure::violation(
                            Violation::CallRegion,
                            i.primary_span(),
                            None,
                        ));
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
                    Err(OwnedFailure::violation(
                        Violation::CallRegion,
                        e.primary_span(),
                        None,
                    ))
                }
            }
            OwnedTerminatorKind::Invoke { call, .. }
                if state != 0 && descendant(shape, call.0, id) =>
            {
                Err(OwnedFailure::violation(
                    Violation::CallRegion,
                    e.primary_span(),
                    None,
                ))
            }
            OwnedTerminatorKind::ReturnScalar(_) | OwnedTerminatorKind::ReturnOwned(_)
                if state != 0 =>
            {
                Err(OwnedFailure::violation(
                    Violation::ActiveAtExit,
                    e.primary_span(),
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
        loans(f, shape, id, meter)?;
    }
    for id in 0..f.calls.len() {
        call_regions(f, shape, id, meter)?;
    }
    Ok(())
}
