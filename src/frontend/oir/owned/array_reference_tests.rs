//! Unit2C scalar-sequence oracle. Expected costs are inventoried from the
//! accepted contract, never from ExecutionPlan or an executable witness.
use super::consumer_fixtures as raw;
use super::execute::{
    Event, Limits, ObservationControl, OwnedRunFailure, ReferenceObservation,
    StorageObservationKind as StorageKind,
};
use super::storage::{LoanKey, OwnerKey};
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AccessCase {
    Read,
    Write,
    Length,
}

#[derive(Clone, Copy, Debug)]
enum Seed {
    Distinct,
    Alternate,
}

fn values(ty: hir::Ty, n: usize, seed: Seed) -> Vec<Scalar> {
    (0..n)
        .map(|i| match (ty, seed) {
            (hir::Ty::I32, Seed::Distinct) => Scalar::I32(i as i32 * 37 - 91),
            (hir::Ty::I32, Seed::Alternate) => Scalar::I32([i32::MIN, i32::MAX, 0, -1][i % 4]),
            (hir::Ty::Bool, Seed::Distinct) => Scalar::Bool(i % 2 == 0),
            (hir::Ty::Bool, Seed::Alternate) => Scalar::Bool(i % 2 != 0),
            (hir::Ty::Unit, _) => Scalar::Unit,
        })
        .collect()
}

fn replacement(ty: hir::Ty, seed: Seed) -> Scalar {
    match (ty, seed) {
        (hir::Ty::I32, Seed::Distinct) => Scalar::I32(i32::MAX),
        (hir::Ty::I32, Seed::Alternate) => Scalar::I32(i32::MIN),
        (hir::Ty::Bool, Seed::Distinct) => Scalar::Bool(false),
        (hir::Ty::Bool, Seed::Alternate) => Scalar::Bool(true),
        (hir::Ty::Unit, _) => Scalar::Unit,
    }
}

fn literal(value: Scalar) -> Rvalue {
    match value {
        Scalar::Bool(v) => Rvalue::Bool(v),
        Scalar::I32(v) => Rvalue::I32(v),
        Scalar::Unit => Rvalue::Unit,
    }
}

fn array(ty: hir::Ty, n: usize) -> AggregateTy {
    AggregateTy::FixedArray(FixedArrayTy::check(ty, n).unwrap())
}

fn owner(ty: hir::Ty, n: usize, kind: OwnerKind, span: Span) -> OwnerDecl {
    OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(array(ty, n)).unwrap(),
        kind,
        span,
    }
}

fn block(statements: Vec<OwnedStatement>, kind: OwnedTerminatorKind, span: Span) -> OwnedBlock {
    OwnedBlock {
        merge: None,
        span,
        statements,
        terminator: raw::end(kind, span),
    }
}

fn key(frame: u64, owner: u64, generation: u64) -> OwnerKey {
    OwnerKey {
        frame,
        activation: frame + 1,
        owner,
        generation,
    }
}

struct Step {
    span: Span,
    cost: usize,
    events: Vec<Event>,
}

struct SequenceSchedule {
    steps: Vec<Step>,
    result: Result<Scalar, OwnedRunFailure>,
}

impl SequenceSchedule {
    fn new(result: Result<Scalar, OwnedRunFailure>) -> Self {
        Self {
            steps: vec![],
            result,
        }
    }

    fn push(&mut self, span: Span, cost: usize, events: Vec<Event>) {
        self.steps.push(Step { span, cost, events });
    }

    fn fuel(&self) -> usize {
        self.steps.iter().map(|step| step.cost).sum()
    }

    fn paid(&self, fuel: usize, span: Span) -> bool {
        let mut prefix = 0;
        for step in &self.steps {
            prefix += step.cost;
            if step.span == span {
                return fuel >= prefix;
            }
        }
        false
    }

    fn check(&self, observation: &ReferenceObservation, fuel: usize) {
        assert!(!observation.truncated, "fuel={fuel}");
        assert!(observation.events.len() <= 16_384);
        assert!(observation.storage.len() <= 64);
        assert!(
            observation
                .storage
                .iter()
                .map(|row| row.bytes.len())
                .sum::<usize>()
                <= 262_144
        );
        for snapshot in &observation.storage {
            assert!(!snapshot.poisoned);
            assert!(snapshot.bytes.len() <= 4096);
            assert_eq!(snapshot.guards_before, snapshot.guards_after);
        }
        let mut remaining = fuel;
        let mut events = vec![];
        let mut failure = None;
        for step in &self.steps {
            if remaining < step.cost {
                failure = Some(OwnedRunFailure::Scalar(RunFailure::Fuel(step.span)));
                break;
            }
            remaining -= step.cost;
            events.push(Event::Charge(step.span, step.cost));
            events.extend(step.events.iter().cloned());
        }
        match failure {
            Some(error) => assert_eq!(observation.result, Err(error), "fuel={fuel}"),
            None => assert_eq!(observation.result, self.result, "fuel={fuel}"),
        }
        assert_eq!(observation.events, events, "fuel={fuel}");
        assert_eq!(observation.remaining_fuel, remaining, "fuel={fuel}");
    }
}

fn observe(raw: RawOwnedProgram, sources: &SourceMap, fuel: usize) -> ReferenceObservation {
    verified::probe_array_reference(
        raw,
        sources,
        budget::Limits::DEFAULT,
        Some(hir::DefId(0)),
        Limits {
            fuel,
            ..Limits::default()
        },
        ObservationControl::default(),
    )
    .expect("the authoritative verifier must admit each oracle fixture")
}

fn core_raw(
    ty: hir::Ty,
    n: usize,
    index: i32,
    access: AccessCase,
    seed: Seed,
    s: &impl Fn(usize) -> Span,
) -> RawOwnedProgram {
    let result_ty = if access == AccessCase::Length {
        hir::Ty::I32
    } else {
        ty
    };
    let result = n + if access == AccessCase::Length { 0 } else { 2 };
    let mut f = raw::function(0, ValueTy::Scalar(result_ty), s(0));
    f.locals = (0..n).map(|_| raw::scalar(ty, s(0))).collect();
    if access != AccessCase::Length {
        f.locals
            .extend([raw::scalar(hir::Ty::I32, s(0)), raw::scalar(ty, s(0))]);
    }
    f.locals.push(raw::scalar(result_ty, s(0)));
    f.owners = vec![owner(ty, n, OwnerKind::Local { mutable: true }, s(0))];
    let mut statements: Vec<_> = values(ty, n, seed)
        .into_iter()
        .enumerate()
        .map(|(i, value)| raw::assign(i, literal(value), s(1 + i)))
        .collect();
    if access != AccessCase::Length {
        statements.extend([
            raw::assign(n, Rvalue::I32(index), s(1100)),
            raw::assign(n + 1, literal(replacement(ty, seed)), s(1101)),
        ]);
    }
    statements.extend([
        raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(1102)),
        raw::instruction(
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(0),
                elements: (0..n).map(|i| raw::operand(i, s(1103))).collect(),
            },
            s(1103),
        ),
    ]);
    let read = OwnedInstruction::ReadIndex {
        destination: LocalId(result),
        base: AccessBase::Owner(OwnerPlaceId(0)),
        index: raw::operand(n, s(1104)),
    };
    match access {
        AccessCase::Read => statements.push(raw::instruction(read, s(1104))),
        AccessCase::Write => statements.extend([
            raw::instruction(
                OwnedInstruction::WriteIndex {
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    index: raw::operand(n, s(1104)),
                    value: raw::operand(n + 1, s(1104)),
                },
                s(1104),
            ),
            raw::instruction(read, s(1105)),
        ]),
        AccessCase::Length => statements.push(raw::instruction(
            OwnedInstruction::ArrayLength {
                destination: LocalId(result),
                base: AccessBase::Owner(OwnerPlaceId(0)),
            },
            s(1104),
        )),
    }
    f.blocks = vec![block(
        statements,
        OwnedTerminatorKind::ReturnScalar(raw::operand(result, s(1106))),
        s(1106),
    )];
    RawOwnedProgram {
        enums: vec![],
        records: vec![],
        functions: vec![f],
    }
}

fn core_schedule(
    ty: hir::Ty,
    n: usize,
    index: i32,
    access: AccessCase,
    seed: Seed,
    s: &impl Fn(usize) -> Span,
) -> SequenceSchedule {
    let width = n.max(1);
    let valid = index >= 0 && (index as usize) < n;
    let sequence = values(ty, n, seed);
    let result = if access == AccessCase::Length {
        Ok(Scalar::I32(n as i32))
    } else if !valid {
        Err(OwnedRunFailure::Bounds(s(1104)))
    } else if access == AccessCase::Read {
        Ok(sequence[index as usize])
    } else {
        Ok(replacement(ty, seed))
    };
    let mut schedule = SequenceSchedule::new(result);
    // S=N+1 for length, N+3 otherwise; A=R=L=C=0, O=1, P=w.
    let scalar_slots = n + if access == AccessCase::Length { 1 } else { 3 };
    schedule.push(
        s(0),
        1 + scalar_slots + width + 4,
        vec![Event::Enter(hir::DefId(0), 1)],
    );
    for i in 0..n {
        schedule.push(s(1 + i), 1, vec![]);
    }
    if access != AccessCase::Length {
        schedule.push(s(1100), 1, vec![]);
        schedule.push(s(1101), 1, vec![]);
    }
    schedule.push(s(1102), 1, vec![]);
    schedule.push(s(1103), 1 + width, vec![]);
    let root = key(0, 0, 2);
    let event = match access {
        AccessCase::Read if valid => {
            vec![Event::ReadIndex(
                root,
                index as usize,
                sequence[index as usize],
            )]
        }
        AccessCase::Write if valid => {
            vec![Event::WriteIndex(
                root,
                index as usize,
                replacement(ty, seed),
            )]
        }
        AccessCase::Length => vec![Event::ArrayLength(root, n)],
        _ => vec![],
    };
    schedule.push(s(1104), 1, event);
    if access != AccessCase::Length && !valid {
        return schedule;
    }
    if access == AccessCase::Write {
        schedule.push(
            s(1105),
            1,
            vec![Event::ReadIndex(
                root,
                index as usize,
                replacement(ty, seed),
            )],
        );
    }
    schedule.push(s(1106), 1 + width, vec![Event::Return(hir::DefId(0))]);
    schedule
}

fn indexes(n: usize) -> Vec<i32> {
    let mut indexes = vec![i32::MIN, -1];
    indexes.extend((0..=n).map(|i| i as i32));
    indexes.push(i32::MAX);
    indexes.sort_unstable();
    indexes.dedup();
    indexes
}

fn payload(ty: hir::Ty, sequence: &[Scalar]) -> Vec<u8> {
    if sequence.is_empty() {
        return vec![0; if ty == hir::Ty::I32 { 4 } else { 1 }];
    }
    let mut bytes = vec![];
    for value in sequence {
        match value {
            Scalar::Bool(v) => bytes.push(u8::from(*v)),
            Scalar::I32(v) => bytes.extend(v.to_le_bytes()),
            Scalar::Unit => bytes.push(0),
        }
    }
    bytes
}

fn core_storage(
    observation: &ReferenceObservation,
    case: (hir::Ty, usize, i32, AccessCase, Seed),
    s: &impl Fn(usize) -> Span,
    schedule: &SequenceSchedule,
    fuel: usize,
) {
    let (ty, n, index, access, seed) = case;
    let installed = schedule.paid(fuel, s(0));
    let live = schedule.paid(fuel, s(1102));
    let constructed = schedule.paid(fuel, s(1103));
    let written = access == AccessCase::Write
        && index >= 0
        && (index as usize) < n
        && schedule.paid(fuel, s(1104));
    let original = values(ty, n, seed);
    let mut final_sequence = original.clone();
    if written {
        final_sequence[index as usize] = replacement(ty, seed);
    }
    let bytes = if constructed {
        payload(ty, &final_sequence)
    } else {
        vec![0; n.max(1) * if ty == hir::Ty::I32 { 4 } else { 1 }]
    };
    assert_eq!(
        observation.storage.len(),
        usize::from(constructed) + usize::from(written) + usize::from(installed)
    );
    for snapshot in &observation.storage {
        match snapshot.kind {
            StorageKind::Construction => {
                assert!(constructed);
                assert_eq!(snapshot.key, key(0, 0, 1));
                assert_eq!(snapshot.state, 1);
                assert_eq!(snapshot.bytes, payload(ty, &original));
            }
            StorageKind::IndexWrite => {
                assert!(written);
                assert_eq!(snapshot.key, key(0, 0, 2));
                assert_eq!(snapshot.state, 2);
                assert_eq!(snapshot.bytes, bytes);
            }
            StorageKind::Return | StorageKind::Failure => {
                let state = if constructed { 2 } else { u64::from(live) };
                assert_eq!(snapshot.key, key(0, 0, state));
                assert_eq!(snapshot.state, state);
                assert_eq!(snapshot.bytes, bytes);
                assert_eq!(
                    snapshot.kind == StorageKind::Return,
                    observation.result.is_ok()
                );
            }
            _ => panic!("unexpected storage path in the single-owner oracle"),
        }
    }
}

#[test]
fn unit2c_sequence_oracle_90_tuples_180_accesses_and_15_lengths_every_fuel() {
    let (sources, s) = raw::context();
    let (mut tuples, mut accesses, mut lengths, mut runs) = (0, 0, 0, 0);
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in 0..=4 {
            let indices = indexes(n);
            assert_eq!(indices.len(), n + 4);
            for index in indices {
                tuples += 1;
                for access in [AccessCase::Read, AccessCase::Write] {
                    let schedule = core_schedule(ty, n, index, access, Seed::Distinct, &s);
                    for fuel in 0..=schedule.fuel() {
                        let observation = observe(
                            core_raw(ty, n, index, access, Seed::Distinct, &s),
                            &sources,
                            fuel,
                        );
                        schedule.check(&observation, fuel);
                        core_storage(
                            &observation,
                            (ty, n, index, access, Seed::Distinct),
                            &s,
                            &schedule,
                            fuel,
                        );
                        runs += 1;
                    }
                    accesses += 1;
                }
            }
            let schedule = core_schedule(ty, n, 0, AccessCase::Length, Seed::Distinct, &s);
            for fuel in 0..=schedule.fuel() {
                let observation = observe(
                    core_raw(ty, n, 0, AccessCase::Length, Seed::Distinct, &s),
                    &sources,
                    fuel,
                );
                schedule.check(&observation, fuel);
                core_storage(
                    &observation,
                    (ty, n, 0, AccessCase::Length, Seed::Distinct),
                    &s,
                    &schedule,
                    fuel,
                );
                runs += 1;
            }
            lengths += 1;
        }
    }
    assert_eq!((tuples, accesses, lengths), (90, 180, 15));
    // Independently summed finite families, including budget zero in every run.
    assert_eq!(runs, 4_842);
}

#[test]
fn unit2c_extreme_payloads_and_both_boolean_patterns_every_fuel() {
    let (sources, s) = raw::context();
    let mut fixtures = 0;
    let mut runs = 0;
    for ty in [hir::Ty::Bool, hir::Ty::I32] {
        for index in 0..4 {
            for access in [AccessCase::Read, AccessCase::Write] {
                let schedule = core_schedule(ty, 4, index, access, Seed::Alternate, &s);
                for fuel in 0..=schedule.fuel() {
                    let observation = observe(
                        core_raw(ty, 4, index, access, Seed::Alternate, &s),
                        &sources,
                        fuel,
                    );
                    schedule.check(&observation, fuel);
                    core_storage(
                        &observation,
                        (ty, 4, index, access, Seed::Alternate),
                        &s,
                        &schedule,
                        fuel,
                    );
                    runs += 1;
                }
                fixtures += 1;
            }
        }
    }
    assert_eq!((fixtures, runs), (16, 568));
}

#[test]
fn unit2c_maximum_width_36_accesses_and_three_lengths_boundary_fuel() {
    let (sources, s) = raw::context();
    let mut fixtures = 0;
    let mut runs = 0;
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for access in [AccessCase::Read, AccessCase::Write, AccessCase::Length] {
            let indices = if access == AccessCase::Length {
                vec![0]
            } else {
                vec![i32::MIN, -1, 0, 1023, 1024, i32::MAX]
            };
            for index in indices {
                let schedule = core_schedule(ty, 1024, index, access, Seed::Distinct, &s);
                let mut budgets = vec![0];
                let mut prefix = 0;
                // Activation, constructor, final operation, and return boundaries.
                for step in &schedule.steps {
                    if [s(0), s(1103), s(1104), s(1105), s(1106)].contains(&step.span) {
                        budgets.extend([prefix + step.cost - 1, prefix + step.cost]);
                    }
                    prefix += step.cost;
                }
                budgets.extend([schedule.fuel() - 1, schedule.fuel()]);
                budgets.sort_unstable();
                budgets.dedup();
                for fuel in budgets {
                    let observation = observe(
                        core_raw(ty, 1024, index, access, Seed::Distinct, &s),
                        &sources,
                        fuel,
                    );
                    schedule.check(&observation, fuel);
                    core_storage(
                        &observation,
                        (ty, 1024, index, access, Seed::Distinct),
                        &s,
                        &schedule,
                        fuel,
                    );
                    runs += 1;
                }
                fixtures += 1;
            }
        }
    }
    assert_eq!((fixtures, runs), (39, 270));
}

fn relay_raw(ty: hir::Ty, n: usize, s: &impl Fn(usize) -> Span) -> RawOwnedProgram {
    let result_ty = if n == 0 { hir::Ty::I32 } else { ty };
    let mut f = raw::function(0, ValueTy::Scalar(result_ty), s(0));
    f.locals = (0..n).map(|_| raw::scalar(ty, s(0))).collect();
    f.locals.extend([
        raw::scalar(hir::Ty::I32, s(0)),
        raw::scalar(hir::Ty::I32, s(0)),
        raw::scalar(result_ty, s(0)),
    ]);
    f.owners = [
        OwnerKind::Local { mutable: true },
        OwnerKind::Temporary,
        OwnerKind::Local { mutable: true },
        OwnerKind::Temporary,
        OwnerKind::StagedArgument {
            call: CallSiteId(0),
            argument: 0,
        },
        OwnerKind::CallResult {
            call: CallSiteId(0),
        },
    ]
    .into_iter()
    .map(|kind| owner(ty, n, kind, s(0)))
    .collect();
    f.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(4))],
        result: CallResult::Owned(OwnerPlaceId(5)),
        parent: None,
        span: s(113),
    }];
    let mut statements: Vec<_> = values(ty, n, Seed::Alternate)
        .into_iter()
        .enumerate()
        .map(|(i, value)| raw::assign(i, literal(value), s(i + 1)))
        .collect();
    statements.push(raw::assign(n, Rvalue::I32(0), s(100)));
    for (owner_id, live_span, construct_span, reverse) in
        [(0, 101, 102, false), (1, 103, 104, true)]
    {
        statements.push(raw::instruction(
            OwnedInstruction::StorageLive(OwnerPlaceId(owner_id)),
            s(live_span),
        ));
        statements.push(raw::instruction(
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(owner_id),
                elements: (0..n)
                    .map(|i| raw::operand(if reverse { n - 1 - i } else { i }, s(construct_span)))
                    .collect(),
            },
            s(construct_span),
        ));
    }
    statements.extend([
        raw::instruction(
            OwnedInstruction::ArrayLength {
                destination: LocalId(n + 1),
                base: AccessBase::Owner(OwnerPlaceId(1)),
            },
            s(105),
        ),
        raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(2)), s(106)),
        raw::instruction(
            OwnedInstruction::MoveInitialize {
                destination: OwnerPlaceId(2),
                source: OwnerPlaceId(1),
            },
            s(107),
        ),
        raw::instruction(
            OwnedInstruction::Replace {
                destination: OwnerPlaceId(0),
                source: OwnerPlaceId(2),
            },
            s(108),
        ),
        raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(3)), s(109)),
        raw::instruction(
            OwnedInstruction::MoveInitialize {
                destination: OwnerPlaceId(3),
                source: OwnerPlaceId(0),
            },
            s(110),
        ),
        raw::instruction(
            OwnedInstruction::Replace {
                destination: OwnerPlaceId(0),
                source: OwnerPlaceId(3),
            },
            s(111),
        ),
        raw::instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(113)),
        raw::instruction(
            OwnedInstruction::PrepareOwned {
                call: CallSiteId(0),
                argument: 0,
                source: OwnerPlaceId(0),
            },
            s(114),
        ),
    ]);
    f.blocks.push(block(
        statements,
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        s(115),
    ));
    f.blocks.push(block(
        vec![raw::instruction(
            if n == 0 {
                OwnedInstruction::ArrayLength {
                    destination: LocalId(n + 2),
                    base: AccessBase::Owner(OwnerPlaceId(5)),
                }
            } else {
                OwnedInstruction::ReadIndex {
                    destination: LocalId(n + 2),
                    base: AccessBase::Owner(OwnerPlaceId(5)),
                    index: raw::operand(n, s(118)),
                }
            },
            s(118),
        )],
        OwnedTerminatorKind::ReturnScalar(raw::operand(n + 2, s(119))),
        s(119),
    ));
    let mut callee = raw::function(1, ValueTy::Owned(array(ty, n)), s(120));
    callee.locals = vec![raw::scalar(hir::Ty::I32, s(120))];
    callee.parameters = vec![ParameterBinding::Owned(OwnerPlaceId(0))];
    callee.owners = vec![owner(ty, n, OwnerKind::Parameter { position: 0 }, s(120))];
    callee.blocks = vec![block(
        vec![raw::instruction(
            OwnedInstruction::ArrayLength {
                destination: LocalId(0),
                base: AccessBase::Owner(OwnerPlaceId(0)),
            },
            s(116),
        )],
        OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
        s(117),
    )];
    RawOwnedProgram {
        enums: vec![],
        records: vec![],
        functions: vec![f, callee],
    }
}

fn relay_schedule(ty: hir::Ty, n: usize, s: &impl Fn(usize) -> Span) -> SequenceSchedule {
    let width = n.max(1);
    let mut sequence = values(ty, n, Seed::Alternate);
    sequence.reverse();
    let result = if n == 0 { Scalar::I32(0) } else { sequence[0] };
    let mut schedule = SequenceSchedule::new(Ok(result));
    // Root S=N+3, A=1, O=6, P=6w, C=1, R=L=0.
    schedule.push(
        s(0),
        n + 31 + 6 * width,
        vec![Event::Enter(hir::DefId(0), 1)],
    );
    for i in 0..n {
        schedule.push(s(i + 1), 1, vec![]);
    }
    for (span, cost) in [
        (100, 1),
        (101, 1),
        (102, 1 + width),
        (103, 1),
        (104, 1 + width),
    ] {
        schedule.push(s(span), cost, vec![]);
    }
    schedule.push(s(105), 1, vec![Event::ArrayLength(key(0, 1, 2), n)]);
    schedule.push(s(106), 1, vec![]);
    schedule.push(
        s(107),
        1 + width,
        vec![Event::Transfer(key(0, 1, 2), key(0, 2, 2))],
    );
    schedule.push(
        s(108),
        1 + 2 * width,
        vec![Event::Transfer(key(0, 2, 2), key(0, 0, 3))],
    );
    schedule.push(s(109), 1, vec![]);
    schedule.push(
        s(110),
        1 + width,
        vec![Event::Transfer(key(0, 0, 3), key(0, 3, 2))],
    );
    schedule.push(
        s(111),
        1 + 2 * width,
        vec![Event::Transfer(key(0, 3, 2), key(0, 0, 5))],
    );
    schedule.push(s(113), 2, vec![]);
    schedule.push(
        s(114),
        1 + width,
        vec![Event::Transfer(key(0, 0, 5), key(0, 4, 2))],
    );
    // Callee S=1, O=1, P=w: X=5+w. Invoke=1+1+X+w.
    schedule.push(
        s(115),
        7 + 2 * width,
        vec![
            Event::Transfer(key(0, 4, 2), key(1, 0, 1)),
            Event::Enter(hir::DefId(1), 2),
        ],
    );
    schedule.push(s(116), 1, vec![Event::ArrayLength(key(1, 0, 1), n)]);
    schedule.push(
        s(117),
        1 + 2 * width,
        vec![
            Event::Transfer(key(1, 0, 1), key(0, 5, 1)),
            Event::Return(hir::DefId(1)),
        ],
    );
    schedule.push(
        s(118),
        1,
        vec![if n == 0 {
            Event::ArrayLength(key(0, 5, 1), 0)
        } else {
            Event::ReadIndex(key(0, 5, 1), 0, sequence[0])
        }],
    );
    schedule.push(s(119), 2 + 6 * width, vec![Event::Return(hir::DefId(0))]);
    schedule
}

fn relay_storage(
    observation: &ReferenceObservation,
    ty: hir::Ty,
    n: usize,
    s: &impl Fn(usize) -> Span,
    schedule: &SequenceSchedule,
    fuel: usize,
) {
    let paid = |span| schedule.paid(fuel, s(span));
    let original = values(ty, n, Seed::Alternate);
    let mut reversed = original.clone();
    reversed.reverse();
    let copied = payload(ty, &reversed);
    let empty = vec![0; n.max(1) * if ty == hir::Ty::I32 { 4 } else { 1 }];
    let mut states = [0_u64; 6];
    let mut generations = [0_u64; 6];
    // This abstract lifetime ledger deliberately contains no offsets/layout queries.
    for (span, owner, state) in [
        (101, 0, 1),
        (102, 0, 2),
        (103, 1, 1),
        (104, 1, 2),
        (106, 2, 1),
        (107, 1, 3),
        (107, 2, 2),
        (108, 2, 3),
        (108, 0, 2),
        (109, 3, 1),
        (110, 0, 3),
        (110, 3, 2),
        (111, 3, 3),
        (111, 0, 2),
        (113, 4, 1),
        (114, 0, 3),
        (114, 4, 2),
        (115, 4, 0),
        (117, 5, 2),
    ] {
        if paid(span) {
            states[owner] = state;
            generations[owner] += 1;
        }
    }
    let construction_count = usize::from(paid(102)) + usize::from(paid(104));
    let transfer_count = [107, 108, 110, 111, 114, 117]
        .into_iter()
        .filter(|&span| paid(span))
        .count();
    let incoming_count = usize::from(paid(115));
    let child_return_count = usize::from(paid(117));
    let final_count = if paid(0) {
        6 + usize::from(paid(115) && !paid(117))
    } else {
        0
    };
    assert_eq!(
        observation.storage.len(),
        construction_count + transfer_count + incoming_count + child_return_count + final_count
    );
    for snapshot in &observation.storage {
        match snapshot.kind {
            StorageKind::Construction => {
                assert_eq!(snapshot.state, 1);
                assert_eq!(snapshot.key.generation, 1);
                assert_eq!(
                    snapshot.bytes,
                    if snapshot.key.owner == 0 {
                        payload(ty, &original)
                    } else {
                        copied.clone()
                    }
                );
            }
            StorageKind::Transfer => {
                let state = match (snapshot.key.owner, snapshot.key.generation) {
                    (2..=4, 1) => 1,
                    (0, 2) => 2,
                    (0, 4) => 3,
                    (5, 0) => 0,
                    other => panic!("unexpected transfer destination {other:?}"),
                };
                assert_eq!(snapshot.state, state);
                assert_eq!(snapshot.bytes, copied);
            }
            StorageKind::Incoming => {
                assert_eq!(snapshot.key, key(1, 0, 1));
                assert_eq!(snapshot.state, 2);
                assert_eq!(snapshot.bytes, copied);
            }
            StorageKind::Return | StorageKind::Failure if snapshot.key.frame == 1 => {
                let returned = snapshot.kind == StorageKind::Return;
                assert_eq!(snapshot.key, key(1, 0, if returned { 2 } else { 1 }));
                assert_eq!(snapshot.state, if returned { 3 } else { 2 });
                assert_eq!(snapshot.bytes, copied);
            }
            StorageKind::Return | StorageKind::Failure => {
                let id = snapshot.key.owner as usize;
                assert_eq!(snapshot.key, key(0, id as u64, generations[id]));
                assert_eq!(snapshot.state, states[id]);
                let initialized = [102, 104, 107, 110, 114, 117][id];
                let expected = if id == 0 && paid(102) && !paid(108) {
                    payload(ty, &original)
                } else if paid(initialized) {
                    copied.clone()
                } else {
                    empty.clone()
                };
                assert_eq!(snapshot.bytes, expected);
            }
            _ => panic!("unexpected relay storage operation"),
        }
    }
}

#[test]
fn unit2c_whole_transfer_call_and_self_replacement_sequences_every_fuel() {
    let (sources, s) = raw::context();
    let mut fixtures = 0;
    let mut runs = 0;
    for ty in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in [0, 2] {
            let schedule = relay_schedule(ty, n, &s);
            for fuel in 0..=schedule.fuel() {
                let observation = observe(relay_raw(ty, n, &s), &sources, fuel);
                schedule.check(&observation, fuel);
                relay_storage(&observation, ty, n, &s, &schedule, fuel);
                runs += 1;
            }
            fixtures += 1;
        }
    }
    assert_eq!(fixtures, 6);
    assert_eq!(runs, 591);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HelperCase {
    Success,
    FinalBounds,
    RhsBounds,
    IndexOverflow,
}

fn reference(kind: BorrowKind, position: usize, span: Span) -> ReferenceDecl {
    ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(array(hir::Ty::I32, 2))).unwrap(),
        kind,
        position,
        span,
    }
}

fn helper_raw(case: HelperCase, s: &impl Fn(usize) -> Span) -> RawOwnedProgram {
    let mut f = raw::function(0, ValueTy::Scalar(hir::Ty::I32), s(0));
    f.locals = (0..6).map(|_| raw::scalar(hir::Ty::I32, s(0))).collect();
    f.owners = vec![owner(
        hir::Ty::I32,
        2,
        OwnerKind::Local { mutable: true },
        s(0),
    )];
    f.calls = vec![
        CallDecl {
            target: hir::DefId(1),
            arguments: vec![
                ArgumentSlot::Borrow(LoanId(0)),
                ArgumentSlot::Borrow(LoanId(1)),
            ],
            result: CallResult::Scalar(LocalId(2)),
            parent: None,
            span: s(6),
        },
        CallDecl {
            target: hir::DefId(2),
            arguments: vec![ArgumentSlot::Borrow(LoanId(2))],
            result: CallResult::Scalar(LocalId(3)),
            parent: None,
            span: s(14),
        },
    ];
    f.loans = [
        (0, 0, BorrowKind::Shared, 7),
        (0, 1, BorrowKind::Shared, 8),
        (1, 0, BorrowKind::Exclusive, 15),
    ]
    .into_iter()
    .map(|(call, argument, kind, span)| LoanDecl {
        projection: Vec::new(),
        call: CallSiteId(call),
        argument,
        authority: AccessBase::Owner(OwnerPlaceId(0)),
        kind,
        referent: BorrowedSlot::check(BorrowedTy::Exact(array(hir::Ty::I32, 2))).unwrap(),
        span: s(span),
    })
    .collect();
    f.blocks = vec![
        block(
            vec![
                raw::assign(0, Rvalue::I32(10), s(1)),
                raw::assign(1, Rvalue::I32(20), s(2)),
                raw::assign(5, Rvalue::I32(1), s(3)),
                raw::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(4)),
                raw::instruction(
                    OwnedInstruction::ConstructArray {
                        destination: OwnerPlaceId(0),
                        elements: vec![raw::operand(0, s(5)), raw::operand(1, s(5))],
                    },
                    s(5),
                ),
                raw::instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(6)),
                raw::instruction(
                    OwnedInstruction::PrepareBorrow {
                        call: CallSiteId(0),
                        argument: 0,
                        loan: LoanId(0),
                    },
                    s(7),
                ),
                raw::instruction(
                    OwnedInstruction::PrepareBorrow {
                        call: CallSiteId(0),
                        argument: 1,
                        loan: LoanId(1),
                    },
                    s(8),
                ),
            ],
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            s(9),
        ),
        block(
            vec![
                raw::instruction(OwnedInstruction::OpenCall(CallSiteId(1)), s(14)),
                raw::instruction(
                    OwnedInstruction::PrepareBorrow {
                        call: CallSiteId(1),
                        argument: 0,
                        loan: LoanId(2),
                    },
                    s(15),
                ),
            ],
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(1),
                continuation: BlockId(2),
            },
            s(16),
        ),
        block(
            vec![
                raw::instruction(
                    OwnedInstruction::WriteIndex {
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        index: raw::operand(3, s(25)),
                        value: raw::operand(2, s(25)),
                    },
                    s(25),
                ),
                raw::instruction(
                    OwnedInstruction::ReadIndex {
                        destination: LocalId(4),
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        index: raw::operand(5, s(26)),
                    },
                    s(26),
                ),
            ],
            OwnedTerminatorKind::ReturnScalar(raw::operand(4, s(27))),
            s(27),
        ),
    ];
    let mut rhs = raw::function(1, ValueTy::Scalar(hir::Ty::I32), s(30));
    rhs.locals = (0..3).map(|_| raw::scalar(hir::Ty::I32, s(30))).collect();
    rhs.parameters = vec![
        ParameterBinding::Reference(ReferenceParamId(0)),
        ParameterBinding::Reference(ReferenceParamId(1)),
    ];
    rhs.references = vec![
        reference(BorrowKind::Shared, 0, s(30)),
        reference(BorrowKind::Shared, 1, s(30)),
    ];
    rhs.blocks = vec![block(
        vec![
            raw::assign(
                0,
                Rvalue::I32(if case == HelperCase::RhsBounds { 2 } else { 0 }),
                s(10),
            ),
            raw::instruction(
                OwnedInstruction::ArrayLength {
                    destination: LocalId(2),
                    base: AccessBase::Parameter(ReferenceParamId(0)),
                },
                s(11),
            ),
            raw::instruction(
                OwnedInstruction::ReadIndex {
                    destination: LocalId(1),
                    base: AccessBase::Parameter(ReferenceParamId(1)),
                    index: raw::operand(0, s(12)),
                },
                s(12),
            ),
        ],
        OwnedTerminatorKind::ReturnScalar(raw::operand(1, s(13))),
        s(13),
    )];
    let mut index = raw::function(2, ValueTy::Scalar(hir::Ty::I32), s(31));
    index.locals = (0..6).map(|_| raw::scalar(hir::Ty::I32, s(31))).collect();
    index.parameters = vec![ParameterBinding::Reference(ReferenceParamId(0))];
    index.references = vec![reference(BorrowKind::Exclusive, 0, s(31))];
    let mut statements = vec![
        raw::assign(0, Rvalue::I32(0), s(17)),
        raw::assign(1, Rvalue::I32(99), s(18)),
        raw::instruction(
            OwnedInstruction::WriteIndex {
                base: AccessBase::Parameter(ReferenceParamId(0)),
                index: raw::operand(0, s(19)),
                value: raw::operand(1, s(19)),
            },
            s(19),
        ),
        raw::assign(
            2,
            Rvalue::I32(if case == HelperCase::FinalBounds {
                2
            } else {
                1
            }),
            s(20),
        ),
    ];
    if case == HelperCase::IndexOverflow {
        statements.extend([
            raw::assign(3, Rvalue::I32(i32::MAX), s(21)),
            raw::assign(4, Rvalue::I32(1), s(22)),
            raw::assign(
                5,
                Rvalue::CheckedI32 {
                    op: hir::ArithmeticOp::Add,
                    left: raw::operand(3, s(23)),
                    right: raw::operand(4, s(23)),
                    operator_span: s(23),
                },
                s(23),
            ),
        ]);
    }
    index.blocks = vec![block(
        statements,
        OwnedTerminatorKind::ReturnScalar(raw::operand(2, s(24))),
        s(24),
    )];
    RawOwnedProgram {
        enums: vec![],
        records: vec![],
        functions: vec![f, rhs, index],
    }
}

fn loan(id: u64) -> LoanKey {
    LoanKey {
        frame: 0,
        activation: 1,
        loan: id,
        instance: 1,
    }
}

fn helper_schedule(case: HelperCase, s: &impl Fn(usize) -> Span) -> SequenceSchedule {
    let result = match case {
        HelperCase::Success => Ok(Scalar::I32(10)),
        HelperCase::FinalBounds => Err(OwnedRunFailure::Bounds(s(25))),
        HelperCase::RhsBounds => Err(OwnedRunFailure::Bounds(s(12))),
        HelperCase::IndexOverflow => Err(OwnedRunFailure::Scalar(RunFailure::Overflow(s(23)))),
    };
    let mut schedule = SequenceSchedule::new(result);
    let root = key(0, 0, 2);
    // Root S=6,A=3,P=2,O=1,L=3,C=2,R=0, hence X=55.
    schedule.push(s(0), 56, vec![Event::Enter(hir::DefId(0), 1)]);
    for (span, cost) in [(1, 1), (2, 1), (3, 1), (4, 1), (5, 3), (6, 1)] {
        schedule.push(s(span), cost, vec![]);
    }
    schedule.push(
        s(7),
        1,
        vec![Event::Acquire(loan(0), root, BorrowKind::Shared)],
    );
    schedule.push(
        s(8),
        1,
        vec![Event::Acquire(loan(1), root, BorrowKind::Shared)],
    );
    // RHS helper X=S3+8R2=19, two aliases add one pair check.
    schedule.push(s(9), 23, vec![Event::Enter(hir::DefId(1), 2)]);
    schedule.push(s(10), 1, vec![]);
    schedule.push(s(11), 1, vec![Event::ArrayLength(root, 2)]);
    schedule.push(
        s(12),
        1,
        if case == HelperCase::RhsBounds {
            vec![]
        } else {
            vec![Event::ReadIndex(root, 0, Scalar::I32(10))]
        },
    );
    if case == HelperCase::RhsBounds {
        return schedule;
    }
    schedule.push(
        s(13),
        3,
        vec![
            Event::Release(loan(0)),
            Event::Release(loan(1)),
            Event::Return(hir::DefId(1)),
        ],
    );
    schedule.push(s(14), 1, vec![]);
    schedule.push(
        s(15),
        1,
        vec![Event::Acquire(loan(2), root, BorrowKind::Exclusive)],
    );
    // Index helper X=S6+8R1=14; invocation costs1+1+14.
    schedule.push(s(16), 16, vec![Event::Enter(hir::DefId(2), 3)]);
    schedule.push(s(17), 1, vec![]);
    schedule.push(s(18), 1, vec![]);
    schedule.push(s(19), 1, vec![Event::WriteIndex(root, 0, Scalar::I32(99))]);
    schedule.push(s(20), 1, vec![]);
    if case == HelperCase::IndexOverflow {
        for span in [21, 22, 23] {
            schedule.push(s(span), 1, vec![]);
        }
        return schedule;
    }
    schedule.push(
        s(24),
        2,
        vec![Event::Release(loan(2)), Event::Return(hir::DefId(2))],
    );
    schedule.push(
        s(25),
        1,
        if case == HelperCase::FinalBounds {
            vec![]
        } else {
            vec![Event::WriteIndex(root, 1, Scalar::I32(10))]
        },
    );
    if case == HelperCase::FinalBounds {
        return schedule;
    }
    schedule.push(s(26), 1, vec![Event::ReadIndex(root, 1, Scalar::I32(10))]);
    // Return costs1+P2+L3+C2.
    schedule.push(s(27), 8, vec![Event::Return(hir::DefId(0))]);
    schedule
}

fn helper_storage(
    observation: &ReferenceObservation,
    case: HelperCase,
    schedule: &SequenceSchedule,
    fuel: usize,
    s: &impl Fn(usize) -> Span,
) {
    let paid = |span| schedule.paid(fuel, s(span));
    let constructed = paid(5);
    let helper_mutation = paid(19);
    let final_write = case == HelperCase::Success && paid(25);
    let final_values = if final_write {
        [Scalar::I32(99), Scalar::I32(10)]
    } else if helper_mutation {
        [Scalar::I32(99), Scalar::I32(20)]
    } else {
        [Scalar::I32(10), Scalar::I32(20)]
    };
    assert_eq!(
        observation.storage.len(),
        usize::from(constructed)
            + usize::from(helper_mutation)
            + usize::from(final_write)
            + usize::from(paid(0))
    );
    let mut writes = 0;
    for snapshot in &observation.storage {
        match snapshot.kind {
            StorageKind::Construction => {
                assert_eq!((snapshot.key, snapshot.state), (key(0, 0, 1), 1));
                assert_eq!(
                    snapshot.bytes,
                    payload(hir::Ty::I32, &[Scalar::I32(10), Scalar::I32(20)])
                );
            }
            StorageKind::IndexWrite => {
                let expected = if writes == 0 {
                    [Scalar::I32(99), Scalar::I32(20)]
                } else {
                    [Scalar::I32(99), Scalar::I32(10)]
                };
                assert_eq!((snapshot.key, snapshot.state), (key(0, 0, 2), 2));
                assert_eq!(snapshot.bytes, payload(hir::Ty::I32, &expected));
                writes += 1;
            }
            StorageKind::Return | StorageKind::Failure => {
                let state = if constructed { 2 } else { u64::from(paid(4)) };
                assert_eq!((snapshot.key, snapshot.state), (key(0, 0, state), state));
                assert_eq!(
                    snapshot.bytes,
                    if constructed {
                        payload(hir::Ty::I32, &final_values)
                    } else {
                        vec![0; 8]
                    }
                );
                assert_eq!(
                    snapshot.kind == StorageKind::Return,
                    observation.result.is_ok()
                );
            }
            _ => panic!("unexpected helper storage observation"),
        }
    }
    assert_eq!(
        writes,
        usize::from(helper_mutation) + usize::from(final_write)
    );
}

#[test]
fn unit2c_rhs_snapshot_aliases_and_helper_mutations_survive_final_failures_every_fuel() {
    let (sources, s) = raw::context();
    let mut fixtures = 0;
    let mut runs = 0;
    for (case, sufficient) in [
        (HelperCase::Success, 129),
        (HelperCase::FinalBounds, 120),
        (HelperCase::RhsBounds, 92),
        (HelperCase::IndexOverflow, 120),
    ] {
        let schedule = helper_schedule(case, &s);
        assert_eq!(schedule.fuel(), sufficient);
        for fuel in 0..=sufficient {
            let observation = observe(helper_raw(case, &s), &sources, fuel);
            schedule.check(&observation, fuel);
            helper_storage(&observation, case, &schedule, fuel, &s);
            runs += 1;
        }
        fixtures += 1;
    }
    assert_eq!((fixtures, runs), (4, 465));
}

#[path = "array_reference_boundary_tests.rs"]
mod boundary;
