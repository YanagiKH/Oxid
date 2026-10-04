//! Independent finite models, followed by deliberately mechanical raw adapters.
//!
//! The models below know no OIR IDs, production CFG traversal, transfer helpers,
//! overlap predicates, or verifier state. Only the adapters and assertions use
//! the production representation. Record cases reach `verify_owned`; array cases
//! reach the unprivileged probe of the same authoritative validation pipeline.
use super::*;

mod model {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Action {
        Idle,
        Read,
        Consume,
        Restore,
    }

    #[derive(Clone, Debug)]
    pub struct Graph {
        pub entry: usize,
        pub successors: Vec<Vec<usize>>,
        pub actions: Vec<Action>,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Storage {
        Dead,
        Uninitialized,
        Available,
        Moved,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Fault {
        Unavailable,
        Lifetime,
        Initialization,
    }

    #[derive(Clone, Copy, Debug)]
    pub enum Event {
        Idle,
        Read,
        Consume,
        Restore,
        Live,
        Initialize,
        End,
    }

    /// Declarative test semantics, intentionally independent of raw instructions.
    pub fn transition(state: Storage, event: Event) -> Result<Storage, Fault> {
        use Event::*;
        match event {
            Idle => Ok(state),
            Read if state == Storage::Available => Ok(state),
            Consume if state == Storage::Available => Ok(Storage::Moved),
            Read | Consume => Err(Fault::Unavailable),
            Restore if matches!(state, Storage::Available | Storage::Moved) => {
                Ok(Storage::Available)
            }
            Restore => Err(Fault::Initialization),
            Live if state == Storage::Dead => Ok(Storage::Uninitialized),
            Live => Err(Fault::Lifetime),
            Initialize if state == Storage::Uninitialized => Ok(Storage::Available),
            Initialize => Err(Fault::Initialization),
            End if state != Storage::Dead => Ok(Storage::Dead),
            End => Err(Fault::Lifetime),
        }
    }

    pub fn reachable(graph: &Graph) -> bool {
        let mut reached = vec![false; graph.actions.len()];
        let mut pending = vec![graph.entry];
        while let Some(block) = pending.pop() {
            if reached[block] {
                continue;
            }
            reached[block] = true;
            pending.extend(graph.successors[block].iter().copied());
        }
        reached.into_iter().all(|value| value)
    }

    /// Exhaust all graph/state pairs; no path-depth or iteration cutoff.
    pub fn available(graph: &Graph) -> Result<(), Fault> {
        let mut explored = vec![[false; 4]; graph.actions.len()];
        let mut pending = vec![(graph.entry, Storage::Available)];
        while let Some((block, incoming)) = pending.pop() {
            if explored[block][incoming as usize] {
                continue;
            }
            explored[block][incoming as usize] = true;
            let event = match graph.actions[block] {
                Action::Idle => Event::Idle,
                Action::Read => Event::Read,
                Action::Consume => Event::Consume,
                Action::Restore => Event::Restore,
            };
            let outgoing = transition(incoming, event)?;
            for &next in &graph.successors[block] {
                pending.push((next, outgoing));
            }
        }
        Ok(())
    }

    pub fn successor_choices(n: usize) -> Vec<Vec<usize>> {
        let mut result = vec![vec![]];
        for first in 0..n {
            result.push(vec![first]);
        }
        for first in 0..n {
            for second in 0..n {
                result.push(vec![first, second]);
            }
        }
        result
    }

    pub fn graph(n: usize, mut encoding: usize, entry: usize) -> Graph {
        let choices = successor_choices(n);
        let successors = (0..n)
            .map(|_| {
                let chosen = choices[encoding % choices.len()].clone();
                encoding /= choices.len();
                chosen
            })
            .collect();
        Graph {
            entry,
            successors,
            actions: vec![Action::Idle; n],
        }
    }

    pub fn label(graph: &mut Graph, mut encoding: usize) {
        for action in &mut graph.actions {
            *action = match encoding % 4 {
                0 => Action::Idle,
                1 => Action::Read,
                2 => Action::Consume,
                _ => Action::Restore,
            };
            encoding /= 4;
        }
    }

    /// Restricted-growth strings enumerate each labeled set partition once.
    pub fn partitions(n: usize) -> Vec<Vec<usize>> {
        fn extend(n: usize, prefix: &mut Vec<usize>, groups: usize, out: &mut Vec<Vec<usize>>) {
            if prefix.len() == n {
                out.push(prefix.clone());
                return;
            }
            for root in 0..=groups {
                prefix.push(root);
                extend(n, prefix, groups.max(root + 1), out);
                prefix.pop();
            }
        }
        if n == 0 {
            return vec![vec![]];
        }
        let mut result = Vec::new();
        extend(n, &mut vec![0], 1, &mut result);
        result
    }

    #[derive(Default, Clone, Copy)]
    struct Capability {
        readers: usize,
        writer: bool,
    }

    /// Acquire in position order, retaining every capability until invocation.
    pub fn aliases(partition: &[usize], exclusive: &[bool]) -> bool {
        let mut roots = vec![Capability::default(); partition.iter().max().map_or(0, |n| n + 1)];
        for (&root, &request_write) in partition.iter().zip(exclusive) {
            let cap = &mut roots[root];
            if cap.writer || (request_write && cap.readers != 0) {
                return false;
            }
            if request_write {
                cap.writer = true;
            } else {
                cap.readers += 1;
            }
        }
        // The normal invocation return discharges the entire preparation region.
        for cap in &mut roots {
            *cap = Capability::default();
        }
        roots.iter().all(|cap| cap.readers == 0 && !cap.writer)
    }

    #[derive(Clone, Copy, Debug)]
    pub enum Probe {
        Read,
        Write,
        Consume,
        Restore,
        End,
    }

    pub fn instrumented_aliases(
        partition: &[usize],
        exclusive: &[bool],
        boundary: usize,
        probe_root: usize,
        probe: Probe,
    ) -> bool {
        let count = partition.iter().max().map_or(0, |n| n + 1);
        let mut caps = vec![Capability::default(); count];
        let mut storage = vec![Storage::Available; count];
        for position in 0..=partition.len() {
            if position == boundary {
                let cap = caps[probe_root];
                let conflict = match probe {
                    Probe::Read => cap.writer,
                    _ => cap.writer || cap.readers != 0,
                };
                if conflict {
                    return false;
                }
                let event = match probe {
                    Probe::Read | Probe::Write => Event::Read,
                    Probe::Consume => Event::Consume,
                    Probe::Restore => Event::Restore,
                    Probe::End => Event::End,
                };
                let Ok(next) = transition(storage[probe_root], event) else {
                    return false;
                };
                storage[probe_root] = next;
            }
            if position == partition.len() {
                break;
            }
            let root = partition[position];
            if storage[root] != Storage::Available {
                return false;
            }
            let cap = &mut caps[root];
            if cap.writer || (exclusive[position] && cap.readers != 0) {
                return false;
            }
            if exclusive[position] {
                cap.writer = true;
            } else {
                cap.readers += 1;
            }
        }
        true
    }

    /// The call and its one loan share a region in this bounded model. Rather
    /// than production's one-assigned-state traversal, enumerate both possible
    /// incoming states and reject any vertex reached in both states.
    pub fn region(graph: &Graph, acquire: usize, invoke: usize) -> bool {
        let mut observed = vec![[false; 2]; graph.actions.len()];
        let mut pending = vec![(graph.entry, false)];
        while let Some((block, open)) = pending.pop() {
            if observed[block][usize::from(open)] {
                continue;
            }
            observed[block][usize::from(open)] = true;
            if observed[block][usize::from(!open)] {
                return false;
            }
            let outgoing = if block == acquire {
                if open {
                    return false;
                }
                true
            } else if block == invoke {
                if !open {
                    return false;
                }
                false
            } else {
                open
            };
            if graph.successors[block].is_empty() && outgoing {
                return false;
            }
            for &next in &graph.successors[block] {
                pending.push((next, outgoing));
            }
        }
        true
    }

    pub fn lifecycle(graph: &Graph, initial: Storage, events: &[Vec<Event>]) -> [bool; 3] {
        let mut faults = [false; 3];
        let mut explored = vec![[false; 4]; graph.actions.len()];
        let mut pending = vec![(graph.entry, initial)];
        while let Some((block, incoming)) = pending.pop() {
            if explored[block][incoming as usize] {
                continue;
            }
            explored[block][incoming as usize] = true;
            let mut current = incoming;
            let mut legal = true;
            for &event in &events[block] {
                match transition(current, event) {
                    Ok(next) => current = next,
                    Err(fault) => {
                        faults[match fault {
                            Fault::Unavailable => 0,
                            Fault::Lifetime => 1,
                            Fault::Initialization => 2,
                        }] = true;
                        legal = false;
                        break;
                    }
                }
            }
            if legal {
                for &next in &graph.successors[block] {
                    pending.push((next, current));
                }
            }
        }
        faults
    }
}

fn context() -> (SourceMap, Span) {
    let mut sources = SourceMap::new();
    let file = sources.add("independent-owned-oracle.ox".into(), "x".repeat(16_384));
    (
        sources,
        Span {
            file,
            start: 0,
            end: 1,
        },
    )
}

fn at(base: Span, offset: usize) -> Span {
    Span {
        start: offset,
        end: offset + 1,
        ..base
    }
}
fn field() -> FieldId {
    FieldId {
        record: RecordId(0),
        index: 0,
    }
}
fn operand(local: usize, span: Span) -> Operand {
    Operand {
        local: LocalId(local),
        span,
    }
}
fn instruction(kind: OwnedInstruction, span: Span) -> OwnedStatement {
    OwnedStatement {
        diagnostic_origins: None,
        kind,
        span,
    }
}
fn scalar(local: usize, value: Rvalue, span: Span) -> OwnedStatement {
    instruction(
        OwnedInstruction::Scalar(Statement::Assign(Assign {
            destination: LocalId(local),
            value,
            span,
        })),
        span,
    )
}
fn record(span: Span) -> RawRecordDecl {
    RawRecordDecl {
        id: RecordId(0),
        span,
        fields: vec![RawFieldDecl {
            id: field(),
            ty: ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32)),
            span,
        }],
    }
}
fn local(ty: hir::Ty, span: Span) -> LocalDecl {
    LocalDecl {
        ty,
        kind: LocalKind::Temporary,
        span,
    }
}
fn owner(kind: OwnerKind, span: Span) -> OwnerDecl {
    OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
        kind,
        span,
    }
}
fn construct(destination: usize, span: Span) -> OwnedStatement {
    instruction(
        OwnedInstruction::Construct {
            destination: OwnerPlaceId(destination),
            fields: vec![(field(), operand(1, span))],
        },
        span,
    )
}
fn block(statements: Vec<OwnedStatement>, kind: OwnedTerminatorKind, span: Span) -> OwnedBlock {
    OwnedBlock {
        merge: None,
        span,
        statements,
        terminator: Some(OwnedTerminator {
            diagnostic_origins: None,
            kind,
            span,
        }),
    }
}
fn empty_function(id: usize, span: Span) -> RawOwnedFunction {
    RawOwnedFunction {
        id: hir::DefId(id),
        span,
        result: ValueTy::Scalar(hir::Ty::Unit),
        parameters: vec![],
        locals: vec![],
        places: vec![],
        owners: vec![],
        references: vec![],
        calls: vec![],
        loans: vec![],
        entry: BlockId(0),
        blocks: vec![],
    }
}

/// Add exactly one acyclic prologue, relocating it without relabeling the model.
fn availability_raw(graph: &model::Graph, prologue: usize, span: Span) -> RawOwnedProgram {
    let physical = |label: usize| BlockId(label + usize::from(label >= prologue));
    let mut f = empty_function(0, span);
    f.entry = BlockId(prologue);
    f.locals = vec![
        local(hir::Ty::Unit, span),
        local(hir::Ty::I32, span),
        local(hir::Ty::Bool, span),
    ];
    f.owners
        .push(owner(OwnerKind::Local { mutable: true }, span));
    let setup = block(
        vec![
            scalar(0, Rvalue::Unit, span),
            scalar(1, Rvalue::I32(7), span),
            scalar(2, Rvalue::Bool(true), span),
            instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), span),
            construct(0, span),
        ],
        OwnedTerminatorKind::Goto(physical(graph.entry)),
        span,
    );
    for (label, action) in graph.actions.iter().enumerate() {
        let origin = at(span, 100 + label * 10);
        let mut statements = vec![];
        match action {
            model::Action::Idle => {}
            model::Action::Read => {
                let destination = LocalId(f.locals.len());
                f.locals.push(local(hir::Ty::I32, origin));
                statements.push(instruction(
                    OwnedInstruction::ReadField {
                        destination,
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        field: field(),
                    },
                    origin,
                ));
            }
            model::Action::Consume => statements.push(instruction(
                OwnedInstruction::Discard(OwnerPlaceId(0)),
                origin,
            )),
            model::Action::Restore => {
                let temporary = f.owners.len();
                f.owners.push(owner(OwnerKind::Temporary, origin));
                statements.extend([
                    instruction(
                        OwnedInstruction::StorageLive(OwnerPlaceId(temporary)),
                        origin,
                    ),
                    construct(temporary, at(origin, origin.start + 1)),
                    instruction(
                        OwnedInstruction::Replace {
                            destination: OwnerPlaceId(0),
                            source: OwnerPlaceId(temporary),
                        },
                        at(origin, origin.start + 2),
                    ),
                    instruction(
                        OwnedInstruction::StorageEnd(OwnerPlaceId(temporary)),
                        at(origin, origin.start + 3),
                    ),
                ]);
            }
        }
        let kind = match graph.successors[label].as_slice() {
            [] => OwnedTerminatorKind::ReturnScalar(operand(0, origin)),
            [to] => OwnedTerminatorKind::Goto(physical(*to)),
            [first, second] => OwnedTerminatorKind::Branch {
                condition: operand(2, origin),
                then_block: physical(*first),
                else_block: physical(*second),
            },
            _ => unreachable!("model has at most two successor slots"),
        };
        f.blocks.push(block(statements, kind, origin));
    }
    f.blocks.insert(prologue, setup);
    RawOwnedProgram {
        records: vec![record(span)],
        functions: vec![f],
    }
}

#[test]
fn exhaustive_four_action_reachable_models_and_all_prologue_positions() {
    let (sources, span) = context();
    let mut graph_entries = [0usize; 3];
    let mut models = [0usize; 3];
    let mut raw_adapters = [0usize; 3];
    let mut accepted_models = 0usize;
    let mut rejected_models = 0usize;
    for n in 1usize..=3 {
        let degree_choices = 1 + n + n * n;
        for edge_encoding in 0..degree_choices.pow(n as u32) {
            for entry in 0..n {
                let mut graph = model::graph(n, edge_encoding, entry);
                if !model::reachable(&graph) {
                    continue;
                }
                graph_entries[n - 1] += 1;
                for action_encoding in 0..4usize.pow(n as u32) {
                    model::label(&mut graph, action_encoding);
                    let expected = model::available(&graph);
                    models[n - 1] += 1;
                    if expected.is_ok() {
                        accepted_models += 1;
                    } else {
                        rejected_models += 1;
                    }
                    for prologue in 0..=n {
                        let actual =
                            verify_owned(availability_raw(&graph, prologue, span), &sources);
                        raw_adapters[n - 1] += 1;
                        let matched = match (&expected, &actual) {
                            (Ok(()), Ok(_)) => true,
                            (Err(model::Fault::Unavailable), Err(failure)) => {
                                failure.kind == OwnedFailureKind::Ownership(Violation::Unavailable)
                            }
                            _ => false,
                        };
                        assert!(matched, "independent model mismatch: n={n}, edges={edge_encoding}, entry={entry}, actions={action_encoding}, prologue={prologue}, graph={graph:?}, expected={expected:?}, actual={actual:?}");
                    }
                }
            }
        }
    }
    assert_eq!(graph_entries, [3, 56, 2_886]);
    assert_eq!(models, [12, 896, 184_704]);
    assert_eq!(raw_adapters, [24, 2_688, 738_816]);
    assert_eq!(models.iter().sum::<usize>(), 185_612);
    assert_eq!(raw_adapters.iter().sum::<usize>(), 741_528);
    println!("independent availability: 185612 model subjects, 741528 verified raw adapters, {accepted_models} accepted models, {rejected_models} unavailable models; raw adapters additionally include one prologue and one temporary owner per Restore-labeled block");
}

#[test]
fn exhaustive_unreachable_graph_classification_and_raw_structural_rejection() {
    let (sources, span) = context();
    let mut unreachable_models = 0usize;
    let mut unreachable_raw_candidates = 0usize;
    let mut executed_idle_adapters = 0usize;
    let mut candidate_models = 0usize;
    let mut candidate_raw = 0usize;
    for n in 1usize..=3 {
        let choices = 1 + n + n * n;
        for edges in 0..choices.pow(n as u32) {
            for entry in 0..n {
                candidate_models += 4usize.pow(n as u32);
                candidate_raw += (n + 1) * 4usize.pow(n as u32);
                let graph = model::graph(n, edges, entry);
                if model::reachable(&graph) {
                    continue;
                }
                unreachable_models += 4usize.pow(n as u32);
                unreachable_raw_candidates += (n + 1) * 4usize.pow(n as u32);
                // Every unreachable graph/entry/wrapper is materialized with Idle
                // labels. Other labels are counted, not claimed as executed.
                for prologue in 0..=n {
                    let actual = verify_owned(availability_raw(&graph, prologue, span), &sources);
                    assert!(matches!(actual, Err(OwnedFailure { kind: OwnedFailureKind::Malformed(Malformed::Scalar(FailureKind::Unreachable)), .. })), "unreachable graph incorrectly classified: graph={graph:?}, prologue={prologue}, actual={actual:?}");
                    executed_idle_adapters += 1;
                }
            }
        }
    }
    assert_eq!(candidate_models, 423_404);
    assert_eq!(candidate_raw, 1_692_024);
    assert_eq!(unreachable_models, 237_792);
    assert_eq!(unreachable_raw_candidates, 950_496);
    assert_eq!(executed_idle_adapters, 14_946);
    println!("unreachable structural family: 237792 labeled model candidates / 950496 raw candidates counted; 14946 all-Idle raw graph-entry-wrapper representatives executed and rejected");
}

fn alias_raw(partition: &[usize], exclusive: &[bool], span: Span) -> RawOwnedProgram {
    let mut caller = empty_function(0, span);
    caller.locals = vec![
        local(hir::Ty::Unit, span),
        local(hir::Ty::I32, span),
        local(hir::Ty::Unit, span),
    ];
    let mut statements = vec![
        scalar(0, Rvalue::Unit, span),
        scalar(1, Rvalue::I32(7), span),
    ];
    let root_count = partition.iter().max().map_or(0, |last| last + 1);
    for root in 0..root_count {
        caller.owners.push(owner(
            OwnerKind::Local { mutable: true },
            at(span, 200 + root),
        ));
        statements.push(instruction(
            OwnedInstruction::StorageLive(OwnerPlaceId(root)),
            at(span, 200 + root),
        ));
        statements.push(construct(root, at(span, 220 + root)));
    }
    statements.push(instruction(
        OwnedInstruction::OpenCall(CallSiteId(0)),
        at(span, 300),
    ));
    let mut target = empty_function(1, at(span, 1_000));
    target.locals.push(local(hir::Ty::Unit, at(span, 1_000)));
    let mut target_statements = vec![scalar(0, Rvalue::Unit, at(span, 1_001))];
    for (position, (&root, &is_exclusive)) in partition.iter().zip(exclusive).enumerate() {
        let kind = if is_exclusive {
            BorrowKind::Exclusive
        } else {
            BorrowKind::Shared
        };
        let acquisition = at(span, 400 + position);
        caller.loans.push(LoanDecl {
            call: CallSiteId(0),
            argument: position,
            authority: AccessBase::Owner(OwnerPlaceId(root)),
            kind,
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            span: acquisition,
        });
        statements.push(instruction(
            OwnedInstruction::PrepareBorrow {
                call: CallSiteId(0),
                argument: position,
                loan: LoanId(position),
            },
            acquisition,
        ));
        target
            .parameters
            .push(ParameterBinding::Reference(ReferenceParamId(position)));
        target.references.push(ReferenceDecl {
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            kind,
            position,
            span: at(span, 1_100 + position),
        });
        target
            .locals
            .push(local(hir::Ty::I32, at(span, 1_200 + position)));
        target_statements.push(instruction(
            OwnedInstruction::ReadField {
                destination: LocalId(position + 1),
                base: AccessBase::Parameter(ReferenceParamId(position)),
                field: field(),
            },
            at(span, 1_200 + position),
        ));
    }
    caller.calls.push(CallDecl {
        target: hir::DefId(1),
        arguments: (0..partition.len())
            .map(|i| ArgumentSlot::Borrow(LoanId(i)))
            .collect(),
        result: CallResult::Scalar(LocalId(2)),
        parent: None,
        span: at(span, 300),
    });
    caller.blocks.push(block(
        statements,
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        at(span, 500),
    ));
    caller.blocks.push(block(
        vec![],
        OwnedTerminatorKind::ReturnScalar(operand(2, at(span, 501))),
        at(span, 501),
    ));
    target.blocks.push(block(
        target_statements,
        OwnedTerminatorKind::ReturnScalar(operand(0, at(span, 1_300))),
        at(span, 1_300),
    ));
    RawOwnedProgram {
        records: vec![record(span)],
        functions: vec![caller, target],
    }
}

#[test]
fn exhaustive_argument_alias_partitions_and_modes() {
    let (sources, span) = context();
    let mut count = 0usize;
    let mut accepted = 0usize;
    let mut conflicting = 0usize;
    for k in 0usize..=4 {
        let partitions = model::partitions(k);
        assert_eq!(partitions.len(), [1, 1, 2, 5, 15][k]);
        for partition in partitions {
            for modes in 0..(1usize << k) {
                let exclusive: Vec<bool> = (0..k).map(|i| modes & (1 << i) != 0).collect();
                let expected = model::aliases(&partition, &exclusive);
                let actual = verify_owned(alias_raw(&partition, &exclusive, span), &sources);
                let matched = if expected {
                    actual.is_ok()
                } else {
                    matches!(
                        actual,
                        Err(OwnedFailure {
                            kind: OwnedFailureKind::Ownership(Violation::LoanConflict),
                            ..
                        })
                    )
                };
                assert!(matched, "alias model mismatch: partition={partition:?}, exclusive={exclusive:?}, expected={expected}, actual={actual:?}");
                count += 1;
                if expected {
                    accepted += 1;
                } else {
                    conflicting += 1;
                }
            }
        }
    }
    assert_eq!((count, accepted, conflicting), (291, 75, 216));
    println!("independent alias partitions: 291 raw comparisons, 75 accepted including aliased-shared callee reads, 216 conflicting");
}

fn initial_events(state: model::Storage) -> Vec<model::Event> {
    use model::{Event::*, Storage};
    match state {
        Storage::Dead => vec![],
        Storage::Uninitialized => vec![Live],
        Storage::Available => vec![Live, Initialize],
        Storage::Moved => vec![Live, Initialize, Consume],
    }
}

fn append_event(
    f: &mut RawOwnedFunction,
    statements: &mut Vec<OwnedStatement>,
    event: model::Event,
    span: Span,
) {
    use model::Event;
    let subject = OwnerPlaceId(0);
    match event {
        Event::Idle => {}
        Event::Read => {
            let destination = LocalId(f.locals.len());
            f.locals.push(local(hir::Ty::I32, span));
            statements.push(instruction(
                OwnedInstruction::ReadField {
                    destination,
                    base: AccessBase::Owner(subject),
                    field: field(),
                },
                span,
            ));
        }
        Event::Consume => statements.push(instruction(OwnedInstruction::Discard(subject), span)),
        Event::Restore => {
            let temporary = OwnerPlaceId(f.owners.len());
            f.owners.push(owner(OwnerKind::Temporary, span));
            statements.extend([
                instruction(OwnedInstruction::StorageLive(temporary), span),
                construct(temporary.0, span),
                instruction(
                    OwnedInstruction::Replace {
                        destination: subject,
                        source: temporary,
                    },
                    span,
                ),
                instruction(OwnedInstruction::StorageEnd(temporary), span),
            ]);
        }
        Event::Live => statements.push(instruction(OwnedInstruction::StorageLive(subject), span)),
        Event::Initialize => statements.push(construct(0, span)),
        Event::End => statements.push(instruction(OwnedInstruction::StorageEnd(subject), span)),
    }
}

fn lifecycle_raw(initial: model::Storage, word: &[model::Event], span: Span) -> RawOwnedProgram {
    let mut f = empty_function(0, span);
    f.locals = vec![local(hir::Ty::Unit, span), local(hir::Ty::I32, span)];
    f.owners
        .push(owner(OwnerKind::Local { mutable: true }, span));
    let mut statements = vec![
        scalar(0, Rvalue::Unit, span),
        scalar(1, Rvalue::I32(7), span),
    ];
    for (position, event) in initial_events(initial)
        .into_iter()
        .chain(word.iter().copied())
        .enumerate()
    {
        append_event(&mut f, &mut statements, event, at(span, 100 + position));
    }
    f.blocks.push(block(
        statements,
        OwnedTerminatorKind::ReturnScalar(operand(0, at(span, 500))),
        at(span, 500),
    ));
    RawOwnedProgram {
        records: vec![record(span)],
        functions: vec![f],
    }
}

#[test]
fn bounded_lifecycle_words_include_all_four_initial_storage_states() {
    use model::{Event, Storage};
    let (sources, span) = context();
    let choices = [
        Event::Idle,
        Event::Read,
        Event::Consume,
        Event::Restore,
        Event::Live,
        Event::Initialize,
        Event::End,
    ];
    let mut total = 0usize;
    let mut malformed = 0usize;
    let mut accepted = 0usize;
    let mut ownership_failures = 0usize;
    for length in 0u32..=4 {
        for encoding in 0..7usize.pow(length) {
            let mut digits = encoding;
            let word: Vec<_> = (0..length)
                .map(|_| {
                    let event = choices[digits % 7];
                    digits /= 7;
                    event
                })
                .collect();
            for initial in [
                Storage::Dead,
                Storage::Uninitialized,
                Storage::Available,
                Storage::Moved,
            ] {
                let setup = initial_events(initial);
                let live_sites = setup
                    .iter()
                    .chain(&word)
                    .filter(|event| matches!(event, Event::Live))
                    .count();
                let initialize_sites = setup
                    .iter()
                    .chain(&word)
                    .filter(|event| matches!(event, Event::Initialize))
                    .count();
                // A live lifetime may legally end without initialization. A
                // declaration has exactly one Live site and at most one initial
                // construction site, distinct from mutable whole replacement.
                let structurally_valid = live_sites == 1 && initialize_sites <= 1;
                let expected = word
                    .iter()
                    .try_fold(initial, |state, &event| model::transition(state, event));
                let actual = verify_owned(lifecycle_raw(initial, &word, span), &sources);
                let matched = if !structurally_valid {
                    malformed += 1;
                    matches!(
                        actual,
                        Err(OwnedFailure {
                            kind: OwnedFailureKind::Malformed(Malformed::CanonicalSite),
                            ..
                        })
                    )
                } else {
                    match expected {
                        Ok(_) => {
                            accepted += 1;
                            actual.is_ok()
                        }
                        Err(fault) => {
                            ownership_failures += 1;
                            let violation = match fault {
                                model::Fault::Unavailable => Violation::Unavailable,
                                model::Fault::Lifetime => Violation::Lifetime,
                                model::Fault::Initialization => Violation::Initialization,
                            };
                            matches!(&actual, Err(failure) if failure.kind == OwnedFailureKind::Ownership(violation))
                        }
                    }
                };
                assert!(matched, "lifecycle mismatch: initial={initial:?}, word={word:?}, structurally_valid={structurally_valid}, expected={expected:?}, actual={actual:?}");
                total += 1;
            }
        }
    }
    assert_eq!(total, 11_204);
    assert_eq!(total, malformed + accepted + ownership_failures);
    assert!(accepted > 0 && ownership_failures > 0 && malformed > 0);
    println!("bounded lifecycle words: {total} raw cases, {accepted} accepted, {ownership_failures} ownership violations, {malformed} canonical-site rejections; seven-event words of length 0..4 in each of four initial states, not the full lifecycle CFG family");
}

fn instrumented_alias_raw(
    partition: &[usize],
    exclusive: &[bool],
    boundary: usize,
    root: usize,
    probe: model::Probe,
    span: Span,
) -> RawOwnedProgram {
    let mut raw = alias_raw(partition, exclusive, span);
    let caller = &mut raw.functions[0];
    let insertion = caller.blocks[0]
        .statements
        .iter()
        .position(|statement| matches!(statement.kind, OwnedInstruction::OpenCall(_)))
        .expect("adapter has one call open")
        + 1
        + boundary;
    let origin = at(span, 700);
    let base = AccessBase::Owner(OwnerPlaceId(root));
    let mut events = vec![];
    match probe {
        model::Probe::Read => {
            let destination = LocalId(caller.locals.len());
            caller.locals.push(local(hir::Ty::I32, origin));
            events.push(instruction(
                OwnedInstruction::ReadField {
                    destination,
                    base,
                    field: field(),
                },
                origin,
            ));
        }
        model::Probe::Write => events.push(instruction(
            OwnedInstruction::WriteField {
                base,
                field: field(),
                value: operand(1, origin),
            },
            origin,
        )),
        model::Probe::Consume => events.push(instruction(
            OwnedInstruction::Discard(OwnerPlaceId(root)),
            origin,
        )),
        model::Probe::End => events.push(instruction(
            OwnedInstruction::StorageEnd(OwnerPlaceId(root)),
            origin,
        )),
        model::Probe::Restore => {
            let temporary = OwnerPlaceId(caller.owners.len());
            caller.owners.push(owner(OwnerKind::Temporary, origin));
            events.extend([
                instruction(OwnedInstruction::StorageLive(temporary), origin),
                construct(temporary.0, origin),
                instruction(
                    OwnedInstruction::Replace {
                        destination: OwnerPlaceId(root),
                        source: temporary,
                    },
                    origin,
                ),
                instruction(OwnedInstruction::StorageEnd(temporary), origin),
            ]);
        }
    }
    caller.blocks[0]
        .statements
        .splice(insertion..insertion, events);
    raw
}

#[test]
fn bounded_argument_alias_access_at_every_preparation_boundary() {
    use model::Probe;
    let (sources, span) = context();
    let probes = [
        Probe::Read,
        Probe::Write,
        Probe::Consume,
        Probe::Restore,
        Probe::End,
    ];
    let mut counts = [0usize; 5];
    let mut accepted = 0usize;
    for (k, count) in counts.iter_mut().enumerate() {
        for partition in model::partitions(k) {
            let roots = partition.iter().max().map_or(0, |n| n + 1);
            for modes in 0..(1usize << k) {
                let exclusive: Vec<_> = (0..k)
                    .map(|position| modes & (1 << position) != 0)
                    .collect();
                for boundary in 0..=k {
                    for root in 0..roots {
                        for probe in probes {
                            let expected = model::instrumented_aliases(
                                &partition, &exclusive, boundary, root, probe,
                            );
                            let actual = verify_owned(
                                instrumented_alias_raw(
                                    &partition, &exclusive, boundary, root, probe, span,
                                ),
                                &sources,
                            );
                            // More than one semantic invariant can fail (for
                            // example, a borrowed owner is consumed and then
                            // borrowed again). Compare the ownership category,
                            // leaving diagnostic precedence to dedicated tests.
                            let matched = if expected {
                                actual.is_ok()
                            } else {
                                matches!(
                                    actual,
                                    Err(OwnedFailure {
                                        kind: OwnedFailureKind::Ownership(_),
                                        ..
                                    })
                                )
                            };
                            assert!(matched, "instrumented aliases mismatch: partition={partition:?}, modes={exclusive:?}, boundary={boundary}, root={root}, probe={probe:?}, expected={expected}, actual={actual:?}");
                            *count += 1;
                            accepted += usize::from(expected);
                        }
                    }
                }
            }
        }
    }
    assert_eq!(counts, [0, 20, 180, 1_600, 14_800]);
    assert_eq!(counts.iter().sum::<usize>(), 16_600);
    println!("instrumented canonical alias family: 16600 raw comparisons, {accepted} accepted; every boundary, root and one of read/write/consume/restore/end, without redundant argument permutations");
}

#[derive(Clone, Copy, Debug)]
enum RegionAuthority {
    MutableOwner,
    SharedParameter,
    ExclusiveParameter,
}

#[derive(Clone, Copy, Debug)]
struct RegionCase {
    acquire: usize,
    invoke: usize,
    exclusive: bool,
    authority: RegionAuthority,
    prologue: usize,
}

fn region_raw(graph: &model::Graph, case: RegionCase, span: Span) -> RawOwnedProgram {
    let mut raw = alias_raw(&[0], &[case.exclusive], span);
    let caller = &mut raw.functions[0];
    let physical = |label: usize| BlockId(label + usize::from(label >= case.prologue));
    caller.entry = BlockId(case.prologue);
    caller.locals.push(local(hir::Ty::Bool, span));
    let mut setup = vec![
        scalar(0, Rvalue::Unit, span),
        scalar(1, Rvalue::I32(7), span),
        scalar(3, Rvalue::Bool(true), span),
    ];
    match case.authority {
        RegionAuthority::MutableOwner => {
            setup.push(instruction(
                OwnedInstruction::StorageLive(OwnerPlaceId(0)),
                span,
            ));
            setup.push(construct(0, span));
        }
        RegionAuthority::SharedParameter | RegionAuthority::ExclusiveParameter => {
            caller.owners.clear();
            caller
                .parameters
                .push(ParameterBinding::Reference(ReferenceParamId(0)));
            caller.references.push(ReferenceDecl {
                referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                    .unwrap(),
                kind: if matches!(case.authority, RegionAuthority::SharedParameter) {
                    BorrowKind::Shared
                } else {
                    BorrowKind::Exclusive
                },
                position: 0,
                span,
            });
            caller.loans[0].authority = AccessBase::Parameter(ReferenceParamId(0));
        }
    }
    caller.blocks.clear();
    for label in 0..graph.actions.len() {
        let origin = at(span, 100 + label * 10);
        let statements = if label == case.acquire {
            caller.loans[0].span = origin;
            vec![
                instruction(OwnedInstruction::OpenCall(CallSiteId(0)), origin),
                instruction(
                    OwnedInstruction::PrepareBorrow {
                        call: CallSiteId(0),
                        argument: 0,
                        loan: LoanId(0),
                    },
                    origin,
                ),
            ]
        } else {
            vec![]
        };
        let kind = if label == case.invoke {
            assert_eq!(graph.successors[label].len(), 1);
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: physical(graph.successors[label][0]),
            }
        } else {
            match graph.successors[label].as_slice() {
                [] => OwnedTerminatorKind::ReturnScalar(operand(0, origin)),
                [next] => OwnedTerminatorKind::Goto(physical(*next)),
                [first, second] => OwnedTerminatorKind::Branch {
                    condition: operand(3, origin),
                    then_block: physical(*first),
                    else_block: physical(*second),
                },
                _ => unreachable!("bounded model has at most two successor slots"),
            }
        };
        caller.blocks.push(block(statements, kind, origin));
    }
    caller.blocks.insert(
        case.prologue,
        block(
            setup,
            OwnedTerminatorKind::Goto(physical(graph.entry)),
            span,
        ),
    );
    raw
}

#[test]
fn bounded_exact_region_graphs_across_all_entries_modes_and_authorities() {
    let (sources, span) = context();
    let mut graph_candidates = 0usize;
    let mut reachable_graphs = 0usize;
    let mut comparisons = 0usize;
    let mut accepted = 0usize;
    let mut permission_failures = 0usize;
    let mut region_failures = 0usize;
    for edges in 0..13usize.pow(3) {
        for entry in 0..3 {
            let graph = model::graph(3, edges, entry);
            for acquire in 0..3 {
                for invoke in 0..3 {
                    if acquire == invoke || graph.successors[invoke].len() != 1 {
                        continue;
                    }
                    graph_candidates += 1;
                    if !model::reachable(&graph) {
                        continue;
                    }
                    reachable_graphs += 1;
                    let valid_region = model::region(&graph, acquire, invoke);
                    for exclusive in [false, true] {
                        for authority in [
                            RegionAuthority::MutableOwner,
                            RegionAuthority::SharedParameter,
                            RegionAuthority::ExclusiveParameter,
                        ] {
                            let allowed = !(exclusive
                                && matches!(authority, RegionAuthority::SharedParameter));
                            for prologue in 0..=3 {
                                let case = RegionCase {
                                    acquire,
                                    invoke,
                                    exclusive,
                                    authority,
                                    prologue,
                                };
                                let actual = verify_owned(region_raw(&graph, case, span), &sources);
                                let matched = if !allowed {
                                    permission_failures += 1;
                                    matches!(
                                        actual,
                                        Err(OwnedFailure {
                                            kind: OwnedFailureKind::Ownership(
                                                Violation::Permission
                                            ),
                                            ..
                                        })
                                    )
                                } else if valid_region {
                                    accepted += 1;
                                    actual.is_ok()
                                } else {
                                    region_failures += 1;
                                    matches!(
                                        actual,
                                        Err(OwnedFailure {
                                            kind: OwnedFailureKind::Ownership(
                                                Violation::LoanRegion
                                                    | Violation::CallRegion
                                                    | Violation::ActiveAtExit
                                            ),
                                            ..
                                        })
                                    )
                                };
                                assert!(matched, "bounded region mismatch: graph={graph:?}, case={case:?}, allowed={allowed}, valid_region={valid_region}, actual={actual:?}");
                                comparisons += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(graph_candidates, 9_126);
    assert_eq!(reachable_graphs, 3_360);
    assert_eq!(comparisons, 80_640);
    assert_eq!(
        comparisons,
        accepted + permission_failures + region_failures
    );
    assert!(accepted > 0 && region_failures > 0);
    println!("bounded exact-region family: 9126 canonical 3-block graph-entry/action candidates, 3360 reachable subjects, 80640 raw adapters across modes/authority classes/prologue positions; {accepted} accepted, {permission_failures} permission failures, {region_failures} region failures; not the full proposal-D family");
}

/// FNV-1a of the complete Debug raw fixture, for deterministic fixture identity;
/// this is a reproducibility checksum, not a cryptographic integrity claim.
fn raw_digest(raw: &RawOwnedProgram) -> u64 {
    format!("{raw:?}")
        .bytes()
        .fold(0xcbf29ce484222325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        })
}

fn lifecycle_graph_raw(
    graph: &model::Graph,
    initial: model::Storage,
    events: &[Vec<model::Event>],
    prologue: usize,
    span: Span,
) -> RawOwnedProgram {
    let mut raw = availability_raw(graph, prologue, span);
    let f = &mut raw.functions[0];
    let mut setup = vec![
        scalar(0, Rvalue::Unit, span),
        scalar(1, Rvalue::I32(7), span),
        scalar(2, Rvalue::Bool(true), span),
    ];
    for (position, event) in initial_events(initial).into_iter().enumerate() {
        append_event(f, &mut setup, event, at(span, 10 + position));
    }
    f.blocks[prologue].statements = setup;
    for (label, sequence) in events.iter().enumerate() {
        let mut statements = vec![];
        for (position, &event) in sequence.iter().enumerate() {
            append_event(
                f,
                &mut statements,
                event,
                at(span, 100 + label * 20 + position),
            );
        }
        f.blocks[label + usize::from(label >= prologue)].statements = statements;
    }
    raw
}

fn lifecycle_shape(initial: model::Storage, events: &[Vec<model::Event>]) -> bool {
    let sequence: Vec<_> = initial_events(initial)
        .into_iter()
        .chain(events.iter().flatten().copied())
        .collect();
    let lives = sequence
        .iter()
        .filter(|event| matches!(event, model::Event::Live))
        .count();
    let initializes = sequence
        .iter()
        .filter(|event| matches!(event, model::Event::Initialize))
        .count();
    lives == 1 && initializes <= 1
}

fn lifecycle_matches<T>(
    actual: &Result<T, OwnedFailure>,
    canonical: bool,
    reachable: bool,
    faults: [bool; 3],
) -> bool {
    if !canonical {
        return matches!(
            actual,
            Err(OwnedFailure {
                kind: OwnedFailureKind::Malformed(Malformed::CanonicalSite),
                ..
            })
        );
    }
    if !reachable {
        return matches!(
            actual,
            Err(OwnedFailure {
                kind: OwnedFailureKind::Malformed(Malformed::Scalar(FailureKind::Unreachable)),
                ..
            })
        );
    }
    if !faults.into_iter().any(|fault| fault) {
        return actual.is_ok();
    }
    match actual {
        Err(OwnedFailure {
            kind: OwnedFailureKind::Ownership(Violation::Unavailable),
            ..
        }) => faults[0],
        Err(OwnedFailure {
            kind: OwnedFailureKind::Ownership(Violation::Lifetime),
            ..
        }) => faults[1],
        Err(OwnedFailure {
            kind: OwnedFailureKind::Ownership(Violation::Initialization),
            ..
        }) => faults[2],
        _ => false,
    }
}

#[test]
fn held_out_lifecycle_joins_generations_backedges_and_early_returns() {
    use model::{Event::*, Storage};
    let (sources, span) = context();
    let cases = [
        (
            "join_restore_A_or_M",
            Storage::Available,
            vec![vec![1, 2], vec![3], vec![3], vec![]],
            vec![vec![Idle], vec![Consume], vec![Idle], vec![Restore, Read]],
        ),
        (
            "join_read_A_or_M",
            Storage::Available,
            vec![vec![1, 2], vec![3], vec![3], vec![]],
            vec![vec![Idle], vec![Consume], vec![Idle], vec![Read]],
        ),
        (
            "generation_end_before_backedge",
            Storage::Dead,
            vec![vec![1], vec![2, 3], vec![0], vec![]],
            vec![
                vec![Live, Initialize],
                vec![Read],
                vec![Read, End],
                vec![End],
            ],
        ),
        (
            "generation_missing_end_before_backedge",
            Storage::Dead,
            vec![vec![1], vec![2, 3], vec![0], vec![]],
            vec![vec![Live, Initialize], vec![Read], vec![Read], vec![End]],
        ),
        (
            "generation_move_restore_end",
            Storage::Dead,
            vec![vec![1], vec![2, 3], vec![0], vec![]],
            vec![
                vec![Live, Initialize],
                vec![Read],
                vec![Consume, Restore, End],
                vec![End],
            ],
        ),
        (
            "generation_early_return_cleanup",
            Storage::Dead,
            vec![vec![1], vec![2, 3], vec![0], vec![]],
            vec![vec![Live, Initialize], vec![Read], vec![End], vec![Read]],
        ),
        (
            "dead_or_available_not_revived",
            Storage::Dead,
            vec![vec![1, 2], vec![3], vec![3], vec![]],
            vec![
                vec![Idle],
                vec![Live, Initialize],
                vec![Idle],
                vec![Restore],
            ],
        ),
        (
            "use_after_end_before_branch",
            Storage::Available,
            vec![vec![1], vec![2, 3], vec![], vec![]],
            vec![vec![End], vec![Read], vec![Idle], vec![Idle]],
        ),
    ];
    let mut comparisons = 0usize;
    for (seed, initial, successors, events) in cases {
        let graph = model::Graph {
            entry: 0,
            actions: vec![model::Action::Idle; successors.len()],
            successors,
        };
        let faults = model::lifecycle(&graph, initial, &events);
        assert!(lifecycle_shape(initial, &events));
        assert!(model::reachable(&graph));
        for prologue in 0..=graph.actions.len() {
            let raw = lifecycle_graph_raw(&graph, initial, &events, prologue, span);
            let digest = raw_digest(&raw);
            let actual = verify_owned(raw, &sources);
            assert!(lifecycle_matches(&actual, true, true, faults), "held-out lifecycle mismatch seed={seed}, prologue={prologue}, raw_fnv1a={digest:016x}, faults={faults:?}, actual={actual:?}");
            println!("held-out lifecycle seed={seed}/wrapper={prologue} raw_fnv1a={digest:016x} faults={faults:?}");
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 40);
}

/// Independent 9E language: lexical names, activations, and capability handles.
/// There are no production IDs, call-region helpers, or overlap predicates here.
mod nested_model {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Mode {
        Shared,
        Exclusive,
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Name {
        Root,
        OtherRoot,
        Parent,
        Sibling,
        Child,
        Sink,
    }
    #[derive(Clone, Debug)]
    pub enum Expr {
        Read(Name),
        Write(Name),
        Call {
            target: usize,
            authorities: Vec<Name>,
            scalar_evaluation: Option<Vec<Expr>>,
        },
    }
    #[derive(Clone, Debug)]
    pub struct Function {
        pub parameters: Vec<(Name, Mode)>,
        pub scalar_parameter: bool,
        pub owners: Vec<(Name, usize)>,
        pub body: Vec<Expr>,
    }
    #[derive(Clone, Debug)]
    pub struct Program {
        pub functions: Vec<Function>,
    }
    #[derive(Clone, Copy, Debug)]
    pub struct Tuple {
        pub parent: Mode,
        pub first: Mode,
        pub second: Mode,
        pub request_child: bool,
        pub timing: usize,
        pub probe: usize,
        pub probe_authority: usize,
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Variant {
        OwnerBase,
        SharedAliasing,
        SharedDistinct,
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Outcome {
        Accept,
        Provenance,
        Permission,
        Conflict,
    }
    fn sink(mode: Mode) -> usize {
        if mode == Mode::Shared {
            3
        } else {
            4
        }
    }
    fn leaf_call(mode: Mode, authority: Name) -> Expr {
        Expr::Call {
            target: sink(mode),
            authorities: vec![authority],
            scalar_evaluation: None,
        }
    }
    pub fn program(tuple: Tuple, variant: Variant) -> Program {
        use Name::*;
        let alias_variant = variant != Variant::OwnerBase;
        let second_authority = if tuple.request_child { Child } else { Parent };
        let probe_authority = match tuple.probe_authority {
            0 => {
                if alias_variant {
                    Sibling
                } else {
                    Root
                }
            }
            1 => Parent,
            _ => Child,
        };
        let probe = match tuple.probe {
            0 => Expr::Read(probe_authority),
            1 => Expr::Write(probe_authority),
            2 => leaf_call(Mode::Shared, probe_authority),
            _ => leaf_call(Mode::Exclusive, probe_authority),
        };
        let activities = vec![leaf_call(tuple.second, second_authority), probe];
        let first_call = Expr::Call {
            target: 2,
            authorities: vec![Parent],
            scalar_evaluation: Some(if tuple.timing == 1 {
                activities.clone()
            } else {
                vec![]
            }),
        };
        let mut parent_body = vec![];
        // Both incoming alias handles remain shared. E is an actual attempted
        // upgrade, never an invented legal exclusive alias at function entry.
        if alias_variant && tuple.parent == Mode::Exclusive {
            parent_body.push(leaf_call(Mode::Exclusive, Parent));
        }
        if tuple.timing == 0 {
            parent_body.extend(activities.clone());
        }
        parent_body.push(first_call);
        if tuple.timing == 3 {
            parent_body.extend(activities.clone());
        }
        let mut child_body = vec![Expr::Read(Child)];
        if tuple.timing == 2 {
            child_body.extend(activities);
        }
        let parent_parameters = if alias_variant {
            vec![(Parent, Mode::Shared), (Sibling, Mode::Shared)]
        } else {
            vec![(Parent, tuple.parent)]
        };
        let root_authorities = if alias_variant {
            vec![
                Root,
                if variant == Variant::SharedAliasing {
                    Root
                } else {
                    OtherRoot
                },
            ]
        } else {
            vec![Root]
        };
        Program {
            functions: vec![
                Function {
                    parameters: vec![],
                    scalar_parameter: false,
                    owners: if alias_variant {
                        vec![(Root, 0), (OtherRoot, 1)]
                    } else {
                        vec![(Root, 0)]
                    },
                    body: vec![Expr::Call {
                        target: 1,
                        authorities: root_authorities,
                        scalar_evaluation: None,
                    }],
                },
                Function {
                    parameters: parent_parameters,
                    scalar_parameter: false,
                    owners: vec![],
                    body: parent_body,
                },
                Function {
                    parameters: vec![(Child, tuple.first)],
                    scalar_parameter: true,
                    owners: vec![],
                    body: child_body,
                },
                Function {
                    parameters: vec![(Sink, Mode::Shared)],
                    scalar_parameter: false,
                    owners: vec![],
                    body: vec![Expr::Read(Sink)],
                },
                Function {
                    parameters: vec![(Sink, Mode::Exclusive)],
                    scalar_parameter: false,
                    owners: vec![],
                    body: vec![Expr::Read(Sink)],
                },
            ],
        }
    }
    fn named_mode(function: &Function, name: Name) -> Option<Mode> {
        function
            .parameters
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, mode)| *mode)
            .or_else(|| {
                function
                    .owners
                    .iter()
                    .find(|(candidate, _)| *candidate == name)
                    .map(|_| Mode::Exclusive)
            })
    }
    fn provenance(function: &Function, expressions: &[Expr]) -> bool {
        expressions.iter().all(|expression| match expression {
            Expr::Read(name) | Expr::Write(name) => named_mode(function, *name).is_some(),
            Expr::Call {
                authorities,
                scalar_evaluation,
                ..
            } => {
                authorities
                    .iter()
                    .all(|name| named_mode(function, *name).is_some())
                    && scalar_evaluation
                        .as_ref()
                        .is_none_or(|body| provenance(function, body))
            }
        })
    }
    fn permission(program: &Program, function: &Function, expressions: &[Expr]) -> bool {
        expressions.iter().all(|expression| match expression {
            Expr::Read(_) => true,
            Expr::Write(name) => named_mode(function, *name) == Some(Mode::Exclusive),
            Expr::Call {
                target,
                authorities,
                scalar_evaluation,
            } => {
                authorities
                    .iter()
                    .zip(&program.functions[*target].parameters)
                    .all(|(name, (_, request))| {
                        *request == Mode::Shared
                            || named_mode(function, *name) == Some(Mode::Exclusive)
                    })
                    && scalar_evaluation
                        .as_ref()
                        .is_none_or(|body| permission(program, function, body))
            }
        })
    }
    #[derive(Debug)]
    struct Capability {
        parent: Option<usize>,
        root: usize,
        mode: Mode,
        live: bool,
    }
    #[derive(Debug)]
    struct Activation {
        function: usize,
        bindings: Vec<(Name, usize)>,
    }
    #[derive(Debug)]
    struct Preparation {
        activation: usize,
        target: usize,
        handles: Vec<usize>,
    }
    struct Machine<'a> {
        program: &'a Program,
        capabilities: Vec<Capability>,
        activations: Vec<Activation>,
        preparations: Vec<Preparation>,
    }
    impl Machine<'_> {
        fn handle(&self, name: Name) -> usize {
            self.activations
                .last()
                .unwrap()
                .bindings
                .iter()
                .find(|(candidate, _)| *candidate == name)
                .unwrap()
                .1
        }
        fn ancestor(&self, candidate: usize, mut handle: usize) -> bool {
            loop {
                if candidate == handle {
                    return true;
                }
                let Some(parent) = self.capabilities[handle].parent else {
                    return false;
                };
                handle = parent;
            }
        }
        fn access(&self, handle: usize, exclusive: bool) -> Result<(), Outcome> {
            let authority = &self.capabilities[handle];
            assert!(authority.live);
            if exclusive && authority.mode != Mode::Exclusive {
                return Err(Outcome::Permission);
            }
            for (other, capability) in self.capabilities.iter().enumerate() {
                if !capability.live
                    || capability.root != authority.root
                    || self.ancestor(other, handle)
                {
                    continue;
                }
                if exclusive || capability.mode == Mode::Exclusive {
                    return Err(Outcome::Conflict);
                }
            }
            Ok(())
        }
        fn borrow(&mut self, name: Name, mode: Mode) -> Result<usize, Outcome> {
            let parent = self.handle(name);
            self.access(parent, mode == Mode::Exclusive)?;
            let root = self.capabilities[parent].root;
            let handle = self.capabilities.len();
            self.capabilities.push(Capability {
                parent: Some(parent),
                root,
                mode,
                live: true,
            });
            Ok(handle)
        }
        fn execute(&mut self, expressions: &[Expr]) -> Result<(), Outcome> {
            for expression in expressions {
                match expression {
                    Expr::Read(name) => self.access(self.handle(*name), false)?,
                    Expr::Write(name) => self.access(self.handle(*name), true)?,
                    Expr::Call {
                        target,
                        authorities,
                        scalar_evaluation,
                    } => {
                        let activation = self.activations.len() - 1;
                        self.preparations.push(Preparation {
                            activation,
                            target: *target,
                            handles: vec![],
                        });
                        for (position, &name) in authorities.iter().enumerate() {
                            let requested = self.program.functions[*target].parameters[position].1;
                            let handle = self.borrow(name, requested)?;
                            self.preparations.last_mut().unwrap().handles.push(handle);
                        }
                        if let Some(body) = scalar_evaluation {
                            self.execute(body)?;
                        }
                        let preparing = self.preparations.pop().unwrap();
                        assert_eq!(
                            (preparing.activation, preparing.target),
                            (activation, *target)
                        );
                        let function = &self.program.functions[*target];
                        let bindings = function
                            .parameters
                            .iter()
                            .zip(&preparing.handles)
                            .map(|((name, _), &handle)| (*name, handle))
                            .collect();
                        self.activations.push(Activation {
                            function: *target,
                            bindings,
                        });
                        // Permission is a local declaration contract independent
                        // of statement timing, before dynamic capability checks.
                        if !permission(self.program, function, &function.body) {
                            return Err(Outcome::Permission);
                        }
                        self.execute(&function.body.clone())?;
                        assert_eq!(self.activations.pop().unwrap().function, *target);
                        for handle in preparing.handles {
                            self.capabilities[handle].live = false;
                        }
                    }
                }
            }
            Ok(())
        }
    }
    pub fn evaluate(program: &Program) -> Outcome {
        // Structural name resolution covers every body before any execution or
        // permission check, including an otherwise earlier failing upgrade.
        if program
            .functions
            .iter()
            .any(|function| !provenance(function, &function.body))
        {
            return Outcome::Provenance;
        }
        let root = &program.functions[0];
        let capabilities: Vec<_> = root
            .owners
            .iter()
            .map(|(_, root)| Capability {
                parent: None,
                root: *root,
                mode: Mode::Exclusive,
                live: true,
            })
            .collect();
        let bindings = root
            .owners
            .iter()
            .enumerate()
            .map(|(handle, (name, _))| (*name, handle))
            .collect();
        let mut machine = Machine {
            program,
            capabilities,
            activations: vec![Activation {
                function: 0,
                bindings,
            }],
            preparations: vec![],
        };
        match machine.execute(&root.body) {
            Ok(()) => {
                assert!(machine.preparations.is_empty());
                assert_eq!(machine.activations.len(), 1);
                Outcome::Accept
            }
            Err(outcome) => outcome,
        }
    }
}

struct NestedRawBuilder<'a> {
    model: &'a nested_model::Program,
    function: usize,
    raw: RawOwnedFunction,
    current: usize,
    origin: Span,
    offset: usize,
    unit: usize,
    integer: usize,
}
impl NestedRawBuilder<'_> {
    fn next_span(&mut self) -> Span {
        let span = at(self.origin, self.offset);
        self.offset += 1;
        span
    }
    fn authority(&self, name: nested_model::Name) -> AccessBase {
        let function = &self.model.functions[self.function];
        if let Some(index) = function
            .parameters
            .iter()
            .position(|(candidate, _)| *candidate == name)
        {
            return AccessBase::Parameter(ReferenceParamId(index));
        }
        if let Some(index) = function
            .owners
            .iter()
            .position(|(candidate, _)| *candidate == name)
        {
            return AccessBase::Owner(OwnerPlaceId(index));
        }
        // This is intentionally unresolvable IR provenance, not a fabricated
        // parameter in the active function or a cross-activation reference.
        if matches!(
            name,
            nested_model::Name::Root | nested_model::Name::OtherRoot
        ) {
            AccessBase::Owner(OwnerPlaceId(usize::MAX))
        } else {
            AccessBase::Parameter(ReferenceParamId(usize::MAX))
        }
    }
    fn emit(&mut self, expression: &nested_model::Expr, parent: Option<(CallSiteId, usize)>) {
        use nested_model::Expr;
        match expression {
            Expr::Read(name) => {
                let span = self.next_span();
                let destination = LocalId(self.raw.locals.len());
                self.raw.locals.push(local(hir::Ty::I32, span));
                let base = self.authority(*name);
                self.raw.blocks[self.current].statements.push(instruction(
                    OwnedInstruction::ReadField {
                        destination,
                        base,
                        field: field(),
                    },
                    span,
                ));
            }
            Expr::Write(name) => {
                let span = self.next_span();
                let base = self.authority(*name);
                self.raw.blocks[self.current].statements.push(instruction(
                    OwnedInstruction::WriteField {
                        base,
                        field: field(),
                        value: operand(self.integer, span),
                    },
                    span,
                ));
            }
            Expr::Call {
                target,
                authorities,
                scalar_evaluation,
            } => {
                let call = CallSiteId(self.raw.calls.len());
                let open_span = self.next_span();
                let result = LocalId(self.raw.locals.len());
                self.raw.locals.push(local(hir::Ty::Unit, open_span));
                let mut arguments = vec![];
                let mut preparations = vec![];
                for (argument, &name) in authorities.iter().enumerate() {
                    let span = self.next_span();
                    let loan = LoanId(self.raw.loans.len());
                    let authority = self.authority(name);
                    let kind = match self.model.functions[*target].parameters[argument].1 {
                        nested_model::Mode::Shared => BorrowKind::Shared,
                        nested_model::Mode::Exclusive => BorrowKind::Exclusive,
                    };
                    self.raw.loans.push(LoanDecl {
                        call,
                        argument,
                        authority,
                        kind,
                        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(
                            RecordId(0),
                        )))
                        .unwrap(),
                        span,
                    });
                    arguments.push(ArgumentSlot::Borrow(loan));
                    preparations.push(instruction(
                        OwnedInstruction::PrepareBorrow {
                            call,
                            argument,
                            loan,
                        },
                        span,
                    ));
                }
                assert_eq!(
                    scalar_evaluation.is_some(),
                    self.model.functions[*target].scalar_parameter
                );
                if scalar_evaluation.is_some() {
                    arguments.push(ArgumentSlot::Scalar);
                }
                self.raw.calls.push(CallDecl {
                    target: hir::DefId(*target),
                    arguments,
                    result: CallResult::Scalar(result),
                    parent,
                    span: open_span,
                });
                self.raw.blocks[self.current]
                    .statements
                    .push(instruction(OwnedInstruction::OpenCall(call), open_span));
                self.raw.blocks[self.current]
                    .statements
                    .extend(preparations);
                if let Some(body) = scalar_evaluation {
                    for child in body {
                        self.emit(child, Some((call, authorities.len())));
                    }
                    let span = self.next_span();
                    self.raw.blocks[self.current].statements.push(instruction(
                        OwnedInstruction::PrepareScalar {
                            call,
                            argument: authorities.len(),
                            value: operand(self.unit, span),
                        },
                        span,
                    ));
                }
                let span = self.next_span();
                let continuation = self.raw.blocks.len();
                self.raw.blocks[self.current].terminator = Some(OwnedTerminator {
                    diagnostic_origins: None,
                    kind: OwnedTerminatorKind::Invoke {
                        call,
                        continuation: BlockId(continuation),
                    },
                    span,
                });
                self.raw.blocks.push(block(
                    vec![],
                    OwnedTerminatorKind::ReturnScalar(operand(self.unit, span)),
                    span,
                ));
                self.current = continuation;
            }
        }
    }
}

fn nested_raw(model: &nested_model::Program, span: Span) -> RawOwnedProgram {
    let mut functions = vec![];
    for (id, function) in model.functions.iter().enumerate() {
        let origin = at(span, 2_000 * id);
        let mut raw = empty_function(id, origin);
        for (position, (_, mode)) in function.parameters.iter().enumerate() {
            raw.parameters
                .push(ParameterBinding::Reference(ReferenceParamId(position)));
            raw.references.push(ReferenceDecl {
                referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                    .unwrap(),
                kind: if *mode == nested_model::Mode::Shared {
                    BorrowKind::Shared
                } else {
                    BorrowKind::Exclusive
                },
                position,
                span: origin,
            });
        }
        if function.scalar_parameter {
            raw.parameters.push(ParameterBinding::Scalar(LocalId(0)));
            raw.locals.push(LocalDecl {
                ty: hir::Ty::Unit,
                kind: LocalKind::Parameter,
                span: origin,
            });
        }
        let unit = raw.locals.len();
        raw.locals.push(local(hir::Ty::Unit, origin));
        let integer = raw.locals.len();
        raw.locals.push(local(hir::Ty::I32, origin));
        let mut setup = vec![
            scalar(unit, Rvalue::Unit, origin),
            scalar(integer, Rvalue::I32(7), origin),
        ];
        for (owner_id, _) in function.owners.iter().enumerate() {
            raw.owners
                .push(owner(OwnerKind::Local { mutable: true }, origin));
            setup.push(instruction(
                OwnedInstruction::StorageLive(OwnerPlaceId(owner_id)),
                origin,
            ));
            setup.push(instruction(
                OwnedInstruction::Construct {
                    destination: OwnerPlaceId(owner_id),
                    fields: vec![(field(), operand(integer, origin))],
                },
                origin,
            ));
        }
        raw.blocks.push(block(
            setup,
            OwnedTerminatorKind::ReturnScalar(operand(unit, origin)),
            origin,
        ));
        let mut builder = NestedRawBuilder {
            model,
            function: id,
            raw,
            current: 0,
            origin,
            offset: 2_000 * id + 100,
            unit,
            integer,
        };
        for expression in &function.body {
            builder.emit(expression, None);
        }
        functions.push(builder.raw);
    }
    RawOwnedProgram {
        records: vec![record(span)],
        functions,
    }
}

#[test]
fn independent_nested_reborrow_capability_tree_768_plus_1536_alias_tuples() {
    use nested_model::{Mode, Outcome, Tuple, Variant};
    use std::io::Write as _;
    let (sources, span) = context();
    let mut counts = [[0usize; 4]; 3];
    let mut digests = [0u64; 3];
    // Opt-in qualification artifact: the actual complete raw fixture corpus,
    // not a source hash or only the abstract model. Normal tests write no files.
    let mut corpus = std::env::var_os("OXID_OWNED_NESTED_CORPUS").map(|path| {
        std::io::BufWriter::new(std::fs::File::create(path).expect("create requested raw corpus"))
    });
    for (variant_index, variant) in [
        Variant::OwnerBase,
        Variant::SharedAliasing,
        Variant::SharedDistinct,
    ]
    .into_iter()
    .enumerate()
    {
        let mut seed = 0usize;
        for parent in [Mode::Shared, Mode::Exclusive] {
            for first in [Mode::Shared, Mode::Exclusive] {
                for second in [Mode::Shared, Mode::Exclusive] {
                    for request_child in [false, true] {
                        for timing in 0..4 {
                            for probe in 0..4 {
                                for probe_authority in 0..3 {
                                    let tuple = Tuple {
                                        parent,
                                        first,
                                        second,
                                        request_child,
                                        timing,
                                        probe,
                                        probe_authority,
                                    };
                                    let model = nested_model::program(tuple, variant);
                                    let expected = nested_model::evaluate(&model);
                                    let raw = nested_raw(&model, span);
                                    let digest = raw_digest(&raw);
                                    if let Some(output) = &mut corpus {
                                        writeln!(output,"seed={variant:?}/{seed} tuple={tuple:?} expected={expected:?} raw_fnv1a={digest:016x}\n{raw:#?}\n").expect("write complete raw fixture");
                                    }
                                    digests[variant_index] =
                                        digests[variant_index].rotate_left(7) ^ digest;
                                    let actual = verify_owned(raw, &sources);
                                    let matched = match expected {
                                        Outcome::Accept => actual.is_ok(),
                                        Outcome::Provenance => matches!(
                                            actual,
                                            Err(OwnedFailure {
                                                kind: OwnedFailureKind::Malformed(Malformed::Id),
                                                ..
                                            })
                                        ),
                                        Outcome::Permission => matches!(
                                            actual,
                                            Err(OwnedFailure {
                                                kind: OwnedFailureKind::Ownership(
                                                    Violation::Permission
                                                ),
                                                ..
                                            })
                                        ),
                                        Outcome::Conflict => matches!(
                                            actual,
                                            Err(OwnedFailure {
                                                kind: OwnedFailureKind::Ownership(
                                                    Violation::LoanConflict
                                                ),
                                                ..
                                            })
                                        ),
                                    };
                                    assert!(matched,"nested capability mismatch seed={variant:?}/{seed}, tuple={tuple:?}, raw_fnv1a={digest:016x}, expected={expected:?}, actual={actual:?}, model={model:?}");
                                    counts[variant_index][match expected {
                                        Outcome::Accept => 0,
                                        Outcome::Provenance => 1,
                                        Outcome::Permission => 2,
                                        Outcome::Conflict => 3,
                                    }] += 1;
                                    seed += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(seed, 768);
        assert_eq!(counts[variant_index].iter().sum::<usize>(), 768);
        println!("nested capability {variant:?}: 768 tuple seeds / 768 raw comparisons; [accepted,invalid-provenance-shape,permission,loan-conflict]={:?}; aggregate raw FNV-1a={:016x}",counts[variant_index],digests[variant_index]);
    }
    assert_eq!(counts.iter().flatten().sum::<usize>(), 2_304);
    assert_eq!(counts[0][1], 640);
    assert_eq!(counts[1][1], 544);
    assert_eq!(counts[2][1], 544);
    assert_eq!(counts[1], counts[2]);
    assert_eq!(
        counts,
        [[52, 640, 62, 14], [14, 544, 210, 0], [14, 544, 210, 0]]
    );
    if let Some(output) = &mut corpus {
        output.flush().expect("finish requested raw corpus");
    }
    println!("9E total: exactly768 base dimension tuples +1536 shared alias/distinct instantiations; all2304 have actual raw verifier comparisons, with out-of-activation tuples deliberately represented as invalid IDs rather than valid executable scenarios");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArrayProbe {
    Read,
    Write,
    Length,
}

const ARRAY_PROBES: [ArrayProbe; 3] = [ArrayProbe::Read, ArrayProbe::Write, ArrayProbe::Length];

/// Representation-only adapter. Expectations stay in the independent models;
/// no production event summary, flow helper, or seal participates in this path.
/// A fresh scalar zero at the actual entry dominates every index use. Existing
/// writes keep their values; replacing model reads with writes is used only in
/// fixtures whose read results are unused and whose subject is a mutable owner.
fn array_raw(mut raw: RawOwnedProgram, probe: ArrayProbe, length: usize) -> RawOwnedProgram {
    let aggregate = AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, length).unwrap());
    let slot = AggregateSlot::try_from_aggregate(aggregate).unwrap();
    raw.records.clear();
    for f in &mut raw.functions {
        if matches!(f.result, ValueTy::Owned(_)) {
            f.result = ValueTy::Owned(aggregate);
        }
        for owner in &mut f.owners {
            owner.aggregate = slot;
        }
        for reference in &mut f.references {
            reference.referent = BorrowedSlot::check(BorrowedTy::Exact(slot.aggregate())).unwrap();
        }
        for loan in &mut f.loans {
            loan.referent = BorrowedSlot::check(BorrowedTy::Exact(slot.aggregate())).unwrap();
        }
        let index = f.locals.len();
        f.locals.push(local(hir::Ty::I32, f.span));
        f.blocks[f.entry.0]
            .statements
            .insert(0, scalar(index, Rvalue::I32(0), f.span));
        for b in &mut f.blocks {
            for statement in &mut b.statements {
                let s = statement.span;
                let replacement = match &statement.kind {
                    OwnedInstruction::Construct {
                        destination,
                        fields,
                    } => {
                        assert_eq!(fields.len(), 1, "model has one scalar record field");
                        Some(OwnedInstruction::ConstructArray {
                            destination: *destination,
                            elements: vec![fields[0].1; length],
                        })
                    }
                    OwnedInstruction::ReadField {
                        destination, base, ..
                    } => Some(match probe {
                        ArrayProbe::Read => OwnedInstruction::ReadIndex {
                            destination: *destination,
                            base: *base,
                            index: operand(index, s),
                        },
                        ArrayProbe::Write => OwnedInstruction::WriteIndex {
                            base: *base,
                            index: operand(index, s),
                            value: operand(index, s),
                        },
                        ArrayProbe::Length => OwnedInstruction::ArrayLength {
                            destination: *destination,
                            base: *base,
                        },
                    }),
                    OwnedInstruction::WriteField { base, value, .. } => {
                        Some(OwnedInstruction::WriteIndex {
                            base: *base,
                            index: operand(index, s),
                            value: *value,
                        })
                    }
                    _ => None,
                };
                if let Some(kind) = replacement {
                    statement.kind = kind;
                }
            }
        }
    }
    raw
}

fn probe_array(raw: &RawOwnedProgram, sources: &SourceMap) -> Result<OwnershipUsage, OwnedFailure> {
    verified::probe_array_validation(raw, sources, budget::Limits::DEFAULT)
}

#[test]
fn array_model_lifecycle_words_cover_all_initial_states_and_accesses() {
    use model::{Event, Storage};
    let (sources, span) = context();
    // Omit Idle padding and words without an array access. The existing record
    // family retains its larger domain; this is a separately counted slice.
    let choices = [
        Event::Read,
        Event::Consume,
        Event::Restore,
        Event::Live,
        Event::Initialize,
        Event::End,
    ];
    let mut subjects = 0usize;
    let mut comparisons = 0usize;
    let mut outcomes = [0usize; 3];
    for length in 1u32..=3 {
        for encoding in 0..6usize.pow(length) {
            let mut digits = encoding;
            let word: Vec<_> = (0..length)
                .map(|_| {
                    let event = choices[digits % choices.len()];
                    digits /= choices.len();
                    event
                })
                .collect();
            if !word.iter().any(|event| matches!(event, Event::Read)) {
                continue;
            }
            for initial in [
                Storage::Dead,
                Storage::Uninitialized,
                Storage::Available,
                Storage::Moved,
            ] {
                let canonical = lifecycle_shape(initial, std::slice::from_ref(&word));
                let expected = word
                    .iter()
                    .try_fold(initial, |state, &event| model::transition(state, event));
                let mut faults = [false; 3];
                if let Err(fault) = expected {
                    faults[match fault {
                        model::Fault::Unavailable => 0,
                        model::Fault::Lifetime => 1,
                        model::Fault::Initialization => 2,
                    }] = true;
                }
                subjects += 1;
                for probe in ARRAY_PROBES {
                    let raw = array_raw(lifecycle_raw(initial, &word, span), probe, 3);
                    let actual = probe_array(&raw, &sources);
                    assert!(lifecycle_matches(&actual, canonical, true, faults), "array lifecycle mismatch: initial={initial:?}, word={word:?}, probe={probe:?}, canonical={canonical}, expected={expected:?}, actual={actual:?}");
                    comparisons += 1;
                    outcomes[if !canonical {
                        2
                    } else if expected.is_ok() {
                        0
                    } else {
                        1
                    }] += 1;
                }
            }
        }
    }
    assert_eq!((subjects, comparisons), (412, 1_236));
    assert_eq!(outcomes.iter().sum::<usize>(), comparisons);
    assert!(outcomes.iter().all(|&count| count != 0));
    println!("array lifecycle: 412 model subjects, 1236 raw probe comparisons; [accepted,ownership,canonical]={outcomes:?}; six-event words of length 1..3 containing Read, without Idle padding");
}

#[test]
fn array_model_reachable_two_block_graphs_use_every_entry_and_prologue() {
    let (sources, span) = context();
    let mut subjects = [0usize; 2];
    let mut comparisons = [0usize; 2];
    let mut outcomes = [0usize; 2];
    for n in 1usize..=2 {
        let choices = 1 + n + n * n;
        for edges in 0..choices.pow(n as u32) {
            for entry in 0..n {
                let mut graph = model::graph(n, edges, entry);
                if !model::reachable(&graph) {
                    continue;
                }
                for labels in 0..4usize.pow(n as u32) {
                    model::label(&mut graph, labels);
                    if !graph.actions.contains(&model::Action::Read) {
                        continue;
                    }
                    let expected = model::available(&graph);
                    subjects[n - 1] += 1;
                    for prologue in 0..=n {
                        for probe in ARRAY_PROBES {
                            let raw = array_raw(availability_raw(&graph, prologue, span), probe, 3);
                            let actual = probe_array(&raw, &sources);
                            let matched = match expected {
                                Ok(()) => actual.is_ok(),
                                Err(model::Fault::Unavailable) => {
                                    matches!(&actual, Err(failure) if failure.kind == OwnedFailureKind::Ownership(Violation::Unavailable))
                                }
                                _ => false,
                            };
                            assert!(matched, "array availability mismatch: graph={graph:?}, prologue={prologue}, probe={probe:?}, expected={expected:?}, actual={actual:?}");
                            comparisons[n - 1] += 1;
                            outcomes[usize::from(expected.is_err())] += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(subjects, [3, 392]);
    assert_eq!(comparisons, [18, 3_528]);
    assert_eq!(comparisons.iter().sum::<usize>(), 3_546);
    assert!(outcomes.iter().all(|&count| count != 0));
    println!("array availability: 395 reachable labeled 1..2-block model subjects, 3546 raw probe comparisons across read/write/length and all prologue positions; [accepted,unavailable]={outcomes:?}");
}

#[test]
fn array_model_unreachable_two_block_graphs_reject_all_access_variants() {
    let (sources, span) = context();
    let mut subjects = 0usize;
    let mut comparisons = 0usize;
    for edges in 0..7usize.pow(2) {
        for entry in 0..2 {
            let mut graph = model::graph(2, edges, entry);
            if model::reachable(&graph) {
                continue;
            }
            graph.actions.fill(model::Action::Read);
            subjects += 1;
            for prologue in 0..=2 {
                for probe in ARRAY_PROBES {
                    let raw = array_raw(availability_raw(&graph, prologue, span), probe, 3);
                    let actual = probe_array(&raw, &sources);
                    assert!(matches!(actual, Err(OwnedFailure { kind: OwnedFailureKind::Malformed(Malformed::Scalar(FailureKind::Unreachable)), .. })), "array unreachable mismatch: graph={graph:?}, prologue={prologue}, probe={probe:?}, actual={actual:?}");
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!((subjects, comparisons), (42, 378));
    println!("array unreachable: 42 all-Read two-block graph-entry subjects, 378 raw probe comparisons; no unvisited action label candidates counted");
}

#[test]
fn array_model_joins_backedges_generations_and_empty_length_need_availability() {
    use model::{Event::*, Storage};
    let (sources, span) = context();
    let cases = [
        (
            "join_restore_A_or_M",
            Storage::Available,
            vec![vec![1, 2], vec![3], vec![3], vec![]],
            vec![vec![Idle], vec![Consume], vec![Idle], vec![Restore, Read]],
        ),
        (
            "join_access_A_or_M",
            Storage::Available,
            vec![vec![1, 2], vec![3], vec![3], vec![]],
            vec![vec![Idle], vec![Consume], vec![Idle], vec![Read]],
        ),
        (
            "generation_end_before_backedge",
            Storage::Dead,
            vec![vec![1], vec![2, 3], vec![0], vec![]],
            vec![
                vec![Live, Initialize],
                vec![Read],
                vec![Read, End],
                vec![End],
            ],
        ),
        (
            "generation_missing_end_before_backedge",
            Storage::Dead,
            vec![vec![1], vec![2, 3], vec![0], vec![]],
            vec![vec![Live, Initialize], vec![Read], vec![Read], vec![End]],
        ),
        (
            "generation_move_restore_end",
            Storage::Dead,
            vec![vec![1], vec![2, 3], vec![0], vec![]],
            vec![
                vec![Live, Initialize],
                vec![Read],
                vec![Consume, Restore, End],
                vec![End],
            ],
        ),
        (
            "generation_early_return_cleanup",
            Storage::Dead,
            vec![vec![1], vec![2, 3], vec![0], vec![]],
            vec![vec![Live, Initialize], vec![Read], vec![End], vec![Read]],
        ),
        (
            "dead_or_available_not_revived",
            Storage::Dead,
            vec![vec![1, 2], vec![3], vec![3], vec![]],
            vec![
                vec![Idle],
                vec![Live, Initialize],
                vec![Idle],
                vec![Restore, Read],
            ],
        ),
        (
            "use_after_end_before_branch",
            Storage::Available,
            vec![vec![1], vec![2, 3], vec![], vec![]],
            vec![vec![End], vec![Read], vec![Idle], vec![Idle]],
        ),
    ];
    let mut comparisons = 0usize;
    for (seed, initial, successors, events) in cases {
        let graph = model::Graph {
            entry: 0,
            actions: vec![model::Action::Idle; successors.len()],
            successors,
        };
        let faults = model::lifecycle(&graph, initial, &events);
        assert!(lifecycle_shape(initial, &events));
        assert!(model::reachable(&graph));
        for prologue in 0..=graph.actions.len() {
            for length in [0, 3] {
                for probe in ARRAY_PROBES {
                    let raw = array_raw(
                        lifecycle_graph_raw(&graph, initial, &events, prologue, span),
                        probe,
                        length,
                    );
                    let actual = probe_array(&raw, &sources);
                    assert!(lifecycle_matches(&actual, true, true, faults), "array lifecycle graph mismatch: seed={seed}, prologue={prologue}, length={length}, probe={probe:?}, faults={faults:?}, actual={actual:?}");
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 240);
    println!("array lifecycle graphs: 8 model subjects, 240 raw probe comparisons across 5 prologue positions, lengths 0/3, and read/write/length; verifier availability only, no runtime bounds claim");
}

#[test]
fn array_model_ordered_capabilities_probe_every_argument_boundary() {
    let (sources, span) = context();
    let mut subjects = 0usize;
    let mut comparisons = [0usize; 5];
    let mut accepted = [0usize; 3];
    for (arity, count) in comparisons.iter_mut().enumerate() {
        for partition in model::partitions(arity) {
            let roots = partition.iter().max().map_or(0, |root| root + 1);
            for modes in 0..(1usize << arity) {
                let exclusive: Vec<_> = (0..arity)
                    .map(|position| modes & (1 << position) != 0)
                    .collect();
                for boundary in 0..=arity {
                    for root in 0..roots {
                        subjects += 1;
                        for (probe_index, probe) in ARRAY_PROBES.into_iter().enumerate() {
                            let model_probe = if probe == ArrayProbe::Write {
                                model::Probe::Write
                            } else {
                                model::Probe::Read
                            };
                            let expected = model::instrumented_aliases(
                                &partition,
                                &exclusive,
                                boundary,
                                root,
                                model_probe,
                            );
                            let source = instrumented_alias_raw(
                                &partition,
                                &exclusive,
                                boundary,
                                root,
                                model_probe,
                                span,
                            );
                            // Writes were already emitted at the requested boundary.
                            // Callee shared-reference reads remain readable operations.
                            let read = if probe == ArrayProbe::Length {
                                ArrayProbe::Length
                            } else {
                                ArrayProbe::Read
                            };
                            let raw = array_raw(source, read, 3);
                            let actual = probe_array(&raw, &sources);
                            let matched = if expected {
                                actual.is_ok()
                            } else {
                                matches!(&actual, Err(failure) if failure.kind == OwnedFailureKind::Ownership(Violation::LoanConflict))
                            };
                            assert!(matched, "array ordered capability mismatch: partition={partition:?}, modes={exclusive:?}, boundary={boundary}, root={root}, probe={probe:?}, expected={expected}, actual={actual:?}");
                            *count += 1;
                            accepted[probe_index] += usize::from(expected);
                        }
                    }
                }
            }
        }
    }
    assert_eq!(subjects, 3_320);
    assert_eq!(comparisons, [0, 12, 108, 960, 8_880]);
    assert_eq!(comparisons.iter().sum::<usize>(), 9_960);
    assert_eq!(accepted[0], accepted[2]);
    assert!(accepted[1] < accepted[0]);
    println!("array ordered capabilities: 3320 partition/mode/boundary/root subjects, 9960 raw probe comparisons; accepted read/write/length={accepted:?}; immediate acquisition retained through invocation");
}

#[test]
fn array_model_nested_reborrows_preserve_independent_capability_outcomes() {
    use nested_model::{Mode, Outcome, Tuple, Variant};
    let (sources, span) = context();
    let mut counts = [[[0usize; 4]; 3]; 2];
    let mut subjects = 0usize;
    for (variant_index, variant) in [
        Variant::OwnerBase,
        Variant::SharedAliasing,
        Variant::SharedDistinct,
    ]
    .into_iter()
    .enumerate()
    {
        for parent in [Mode::Shared, Mode::Exclusive] {
            for first in [Mode::Shared, Mode::Exclusive] {
                for second in [Mode::Shared, Mode::Exclusive] {
                    for request_child in [false, true] {
                        for timing in 0..4 {
                            for probe in 0..4 {
                                for probe_authority in 0..3 {
                                    let tuple = Tuple {
                                        parent,
                                        first,
                                        second,
                                        request_child,
                                        timing,
                                        probe,
                                        probe_authority,
                                    };
                                    let model = nested_model::program(tuple, variant);
                                    let expected = nested_model::evaluate(&model);
                                    subjects += 1;
                                    for (read_index, read) in [ArrayProbe::Read, ArrayProbe::Length]
                                        .into_iter()
                                        .enumerate()
                                    {
                                        let raw = array_raw(nested_raw(&model, span), read, 3);
                                        let actual = probe_array(&raw, &sources);
                                        let (outcome, matched) = match expected {
                                            Outcome::Accept => (0, actual.is_ok()),
                                            Outcome::Provenance => (
                                                1,
                                                matches!(&actual, Err(failure) if failure.kind == OwnedFailureKind::Malformed(Malformed::Id)),
                                            ),
                                            Outcome::Permission => (
                                                2,
                                                matches!(&actual, Err(failure) if failure.kind == OwnedFailureKind::Ownership(Violation::Permission)),
                                            ),
                                            Outcome::Conflict => (
                                                3,
                                                matches!(&actual, Err(failure) if failure.kind == OwnedFailureKind::Ownership(Violation::LoanConflict)),
                                            ),
                                        };
                                        assert!(matched, "array nested capability mismatch: variant={variant:?}, tuple={tuple:?}, read={read:?}, expected={expected:?}, actual={actual:?}");
                                        counts[read_index][variant_index][outcome] += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(subjects, 2_304);
    assert_eq!(
        counts,
        [[[52, 640, 62, 14], [14, 544, 210, 0], [14, 544, 210, 0]]; 2]
    );
    assert_eq!(counts.iter().flatten().flatten().sum::<usize>(), 4_608);
    println!("array nested capabilities: 2304 inherited independent model subjects, 4608 actual raw probe comparisons using ReadIndex/ArrayLength plus existing writes mapped to WriteIndex; per access/variant [accept,provenance,permission,conflict]={counts:?}");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArrayOwnerClass {
    Mutable,
    Immutable,
    Parameter,
    Temporary,
    CallResult,
}

/// Start with one available subject, then apply an optional consume/end and an
/// access. A result is established by a real owned argument/return/Invoke edge.
fn array_class_source(
    class: ArrayOwnerClass,
    prefix: &[model::Event],
    span: Span,
) -> RawOwnedProgram {
    let mut events = prefix.to_vec();
    events.push(model::Event::Read);
    let mut raw = lifecycle_raw(model::Storage::Available, &events, span);
    let f = &mut raw.functions[0];
    match class {
        ArrayOwnerClass::Mutable => {}
        ArrayOwnerClass::Immutable => f.owners[0].kind = OwnerKind::Local { mutable: false },
        ArrayOwnerClass::Temporary => f.owners[0].kind = OwnerKind::Temporary,
        ArrayOwnerClass::Parameter => {
            f.owners[0].kind = OwnerKind::Parameter { position: 0 };
            f.parameters.push(ParameterBinding::Owned(OwnerPlaceId(0)));
            f.blocks[0].statements.retain(|statement| {
                !matches!(
                    statement.kind,
                    OwnedInstruction::StorageLive(_) | OwnedInstruction::Construct { .. }
                )
            });
        }
        ArrayOwnerClass::CallResult => {
            let continuation = f.blocks[0].statements.split_off(4);
            f.owners[0].kind = OwnerKind::CallResult {
                call: CallSiteId(0),
            };
            f.owners.push(owner(OwnerKind::Temporary, span));
            f.owners.push(owner(
                OwnerKind::StagedArgument {
                    call: CallSiteId(0),
                    argument: 0,
                },
                span,
            ));
            f.calls.push(CallDecl {
                target: hir::DefId(1),
                arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(2))],
                result: CallResult::Owned(OwnerPlaceId(0)),
                parent: None,
                span,
            });
            f.blocks = vec![
                block(
                    vec![
                        scalar(0, Rvalue::Unit, span),
                        scalar(1, Rvalue::I32(7), span),
                        instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), span),
                        construct(1, span),
                        instruction(OwnedInstruction::OpenCall(CallSiteId(0)), span),
                        instruction(
                            OwnedInstruction::PrepareOwned {
                                call: CallSiteId(0),
                                argument: 0,
                                source: OwnerPlaceId(1),
                            },
                            span,
                        ),
                    ],
                    OwnedTerminatorKind::Invoke {
                        call: CallSiteId(0),
                        continuation: BlockId(1),
                    },
                    span,
                ),
                block(
                    continuation,
                    OwnedTerminatorKind::ReturnScalar(operand(0, span)),
                    span,
                ),
            ];
            let mut helper = empty_function(1, at(span, 1_000));
            helper.result = ValueTy::Owned(AggregateTy::Record(RecordId(0)));
            helper
                .parameters
                .push(ParameterBinding::Owned(OwnerPlaceId(0)));
            helper
                .owners
                .push(owner(OwnerKind::Parameter { position: 0 }, helper.span));
            helper.blocks.push(block(
                vec![],
                OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
                helper.span,
            ));
            raw.functions.push(helper);
        }
    }
    raw
}

#[test]
fn array_model_owner_classes_read_write_length_after_consume_or_end() {
    let (sources, span) = context();
    let prefixes = [vec![], vec![model::Event::Consume], vec![model::Event::End]];
    let mut comparisons = 0usize;
    let mut outcomes = [0usize; 3];
    for class in [
        ArrayOwnerClass::Mutable,
        ArrayOwnerClass::Immutable,
        ArrayOwnerClass::Parameter,
        ArrayOwnerClass::Temporary,
        ArrayOwnerClass::CallResult,
    ] {
        for prefix in &prefixes {
            let available = prefix
                .iter()
                .try_fold(model::Storage::Available, |state, &event| {
                    model::transition(state, event)
                })
                .and_then(|state| model::transition(state, model::Event::Read));
            for length in [0, 3] {
                for probe in ARRAY_PROBES {
                    let raw = array_raw(array_class_source(class, prefix, span), probe, length);
                    let actual = probe_array(&raw, &sources);
                    // Declaration permissions are checked before storage flow.
                    // The independent class table grants owner writes only to a
                    // mutable Local; a write never restores a moved subject.
                    let (outcome, matched) = if probe == ArrayProbe::Write
                        && class != ArrayOwnerClass::Mutable
                    {
                        (
                            1,
                            matches!(&actual, Err(failure) if failure.kind == OwnedFailureKind::Ownership(Violation::Permission)),
                        )
                    } else if available.is_err() {
                        (
                            2,
                            matches!(&actual, Err(failure) if failure.kind == OwnedFailureKind::Ownership(Violation::Unavailable)),
                        )
                    } else {
                        (0, actual.is_ok())
                    };
                    assert!(matched, "array owner class mismatch: class={class:?}, prefix={prefix:?}, length={length}, probe={probe:?}, actual={actual:?}");
                    comparisons += 1;
                    outcomes[outcome] += 1;
                }
            }
        }
    }
    assert_eq!((comparisons, outcomes), (90, [22, 24, 44]));
    println!("array owner classes: 15 class/state subjects, 90 raw probe comparisons across lengths 0/3 and read/write/length; [accepted,permission,unavailable]={outcomes:?}");
}

#[test]
fn array_model_staged_access_and_reference_permissions_match_class_table() {
    let (sources, span) = context();
    let mut staged = 0usize;
    let mut references = 0usize;
    let mut accepted_references = 0usize;
    for length in [0, 3] {
        for probe in ARRAY_PROBES {
            let mut source = array_class_source(ArrayOwnerClass::CallResult, &[], span);
            let caller = &mut source.functions[0];
            let mut access = caller.blocks[1].statements.pop().unwrap();
            let OwnedInstruction::ReadField { base, .. } = &mut access.kind else {
                panic!("class fixture ends in its requested access");
            };
            *base = AccessBase::Owner(OwnerPlaceId(2));
            caller.blocks[0].statements.push(access);
            let raw = array_raw(source, probe, length);
            let actual = probe_array(&raw, &sources);
            assert!(
                matches!(&actual, Err(failure) if failure.kind == OwnedFailureKind::Malformed(Malformed::OwnerClass)),
                "array staged access mismatch: length={length}, probe={probe:?}, actual={actual:?}"
            );
            staged += 1;
            for kind in [BorrowKind::Shared, BorrowKind::Exclusive] {
                let mut f = empty_function(0, span);
                f.parameters
                    .push(ParameterBinding::Reference(ReferenceParamId(0)));
                f.references.push(ReferenceDecl {
                    referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(
                        RecordId(0),
                    )))
                    .unwrap(),
                    kind,
                    position: 0,
                    span,
                });
                f.locals = vec![local(hir::Ty::Unit, span), local(hir::Ty::I32, span)];
                f.blocks.push(block(
                    vec![
                        scalar(0, Rvalue::Unit, span),
                        instruction(
                            OwnedInstruction::ReadField {
                                destination: LocalId(1),
                                base: AccessBase::Parameter(ReferenceParamId(0)),
                                field: field(),
                            },
                            span,
                        ),
                    ],
                    OwnedTerminatorKind::ReturnScalar(operand(0, span)),
                    span,
                ));
                let raw = array_raw(
                    RawOwnedProgram {
                        records: vec![record(span)],
                        functions: vec![f],
                    },
                    probe,
                    length,
                );
                let actual = probe_array(&raw, &sources);
                let expected = probe != ArrayProbe::Write || kind == BorrowKind::Exclusive;
                let matched = if expected {
                    actual.is_ok()
                } else {
                    matches!(&actual, Err(failure) if failure.kind == OwnedFailureKind::Ownership(Violation::Permission))
                };
                assert!(matched, "array reference class mismatch: kind={kind:?}, length={length}, probe={probe:?}, actual={actual:?}");
                references += 1;
                accepted_references += usize::from(expected);
            }
        }
    }
    assert_eq!((staged, references, accepted_references), (6, 12, 10));
    println!("array staged/reference classes: 6 staged raw probe rejections and 12 reference raw probe comparisons (10 accepted, 2 shared-write denials)");
}
