//! Raw enum consumer fixtures and independent language-fuel oracles.
//! No source lowerer, execution plan, or production cost helper is used here.
use super::consumer_fixtures as f;
use super::*;

pub(super) const MIXED: [Option<hir::Ty>; 4] = [
    None,
    Some(hir::Ty::Bool),
    Some(hir::Ty::I32),
    Some(hir::Ty::Unit),
];
pub(super) fn at(span: Span, ordinal: usize) -> Span {
    Span {
        start: span.start + 2 * ordinal,
        end: span.start + 2 * ordinal + 1,
        ..span
    }
}
pub(super) fn variant(index: usize) -> VariantId {
    VariantId {
        enumeration: EnumId(0),
        index,
    }
}
pub(super) fn enumeration(payloads: &[Option<hir::Ty>], span: Span) -> RawEnumDecl {
    RawEnumDecl {
        id: EnumId(0),
        span,
        variants: payloads
            .iter()
            .enumerate()
            .map(|(index, &payload)| RawVariantDecl {
                id: variant(index),
                payload: payload.map(|ty| ParameterTy::Value(ValueTy::Scalar(ty))),
                span,
            })
            .collect(),
    }
}
pub(super) fn owner(kind: OwnerKind, span: Span) -> OwnerDecl {
    OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Enum(EnumId(0))).unwrap(),
        kind,
        span,
    }
}
pub(super) fn block(
    statements: Vec<OwnedStatement>,
    kind: OwnedTerminatorKind,
    span: Span,
) -> OwnedBlock {
    OwnedBlock {
        merge: None,
        span,
        statements,
        terminator: f::end(kind, span),
    }
}
fn scalar_value(ty: hir::Ty) -> Scalar {
    match ty {
        hir::Ty::Bool => Scalar::Bool(true),
        hir::Ty::I32 => Scalar::I32(-71),
        hir::Ty::Unit => Scalar::Unit,
    }
}
fn fallback(ty: hir::Ty) -> Scalar {
    match ty {
        hir::Ty::Bool => Scalar::Bool(false),
        hir::Ty::I32 => Scalar::I32(17),
        hir::Ty::Unit => Scalar::Unit,
    }
}
pub(super) fn literal(value: Scalar) -> Rvalue {
    match value {
        Scalar::Bool(value) => Rvalue::Bool(value),
        Scalar::I32(value) => Rvalue::I32(value),
        Scalar::Unit => Rvalue::Unit,
    }
}
fn local(function: &mut RawOwnedFunction, ty: hir::Ty, kind: LocalKind, span: Span) -> LocalId {
    let id = LocalId(function.locals.len());
    function.locals.push(LocalDecl { ty, kind, span });
    id
}
/// Assign fallback and (when present) the constructor operand exactly once.
/// Each arm returns its own binding when that matches the chosen return type.
/// Spans: fallback1, payload2, live3, constructor4, match5, end6, return7.
pub(super) fn mixed_case(
    payloads: &[Option<hir::Ty>],
    order: &[usize],
    constructed: usize,
    result: hir::Ty,
    span: Span,
) -> (RawOwnedProgram, f::Schedule) {
    assert_eq!(payloads.len(), order.len());
    let n = order.len();
    let mut function = f::function(0, ValueTy::Scalar(result), span);
    function.locals.push(f::scalar(result, span));
    function
        .owners
        .push(owner(OwnerKind::Local { mutable: false }, span));
    let mut preceding = vec![f::assign(0, literal(fallback(result)), at(span, 1))];
    let payload = payloads[constructed].map(|ty| {
        let id = local(&mut function, ty, LocalKind::Temporary, at(span, 2));
        preceding.push(f::assign(id.0, literal(scalar_value(ty)), at(span, 2)));
        f::operand(id.0, at(span, 4))
    });
    preceding.extend([
        f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), at(span, 3)),
        f::instruction(
            OwnedInstruction::ConstructEnum {
                destination: OwnerPlaceId(0),
                variant: variant(constructed),
                payload,
            },
            at(span, 4),
        ),
    ]);
    function.matches.push(MatchDecl {
        source: OwnerPlaceId(0),
        span: at(span, 5),
        arms: order
            .iter()
            .enumerate()
            .map(|(arm, &index)| MatchArm {
                variant: variant(index),
                dispatch: BlockId(arm),
                entry: BlockId(n + arm),
            })
            .collect(),
    });
    for arm in 0..n {
        function.blocks.push(block(
            if arm == 0 {
                std::mem::take(&mut preceding)
            } else {
                vec![]
            },
            OwnedTerminatorKind::MatchDispatch {
                match_id: MatchId(0),
                arm,
            },
            at(span, 5),
        ));
    }
    for (arm, &index) in order.iter().enumerate() {
        let destination =
            payloads[index].map(|ty| local(&mut function, ty, LocalKind::Binding, at(span, 5)));
        let returned = if payloads[index] == Some(result) {
            destination.unwrap().0
        } else {
            0
        };
        function.blocks.push(block(
            vec![
                f::instruction(
                    OwnedInstruction::ConsumeVariant {
                        match_id: MatchId(0),
                        arm,
                        destination,
                    },
                    at(span, 5),
                ),
                f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), at(span, 6)),
            ],
            OwnedTerminatorKind::ReturnScalar(f::operand(returned, at(span, 7))),
            at(span, 7),
        ));
    }
    // Root activation: one entry unit, each scalar local, two enum value
    // cells, and four fixed owner-state cells. This inventory is independent.
    let mut events = vec![(span, 7 + function.locals.len()), (at(span, 1), 1)];
    if payload.is_some() {
        events.push((at(span, 2), 1));
    }
    events.extend([(at(span, 3), 1), (at(span, 4), 3)]);
    let selected = order
        .iter()
        .position(|&index| index == constructed)
        .unwrap();
    events.extend((0..=selected).map(|_| (at(span, 5), 1)));
    events.extend([(at(span, 5), 3), (at(span, 6), 3), (at(span, 7), 3)]);
    let expected = if payloads[constructed] == Some(result) {
        scalar_value(result)
    } else {
        fallback(result)
    };
    (
        RawOwnedProgram {
            builtins: BuiltinOrigins::None,
            enums: vec![enumeration(payloads, span)],
            records: vec![],
            functions: vec![function],
        },
        f::Schedule {
            entry: hir::DefId(0),
            result: expected,
            events,
        },
    )
}

/// Every whole transfer: move, available replacement, argument staging,
/// callee incoming parameter, return, and call-result move to named match local.
/// Spans: initial constructor4, move9, replacement constructor11, replace12,
/// open13, prepare14, invoke15, callee return16, result move18, match30,
/// cleanup31, return32. Callee function origin50.
pub(super) fn relay_case(
    constructed: usize,
    result: hir::Ty,
    span: Span,
) -> (RawOwnedProgram, f::Schedule) {
    let (mut raw, base) = mixed_case(&MIXED, &[3, 0, 2, 1], constructed, result, span);
    let function = &mut raw.functions[0];
    let payload = match function.blocks[0].statements.last().unwrap().kind {
        OwnedInstruction::ConstructEnum { payload, .. } => payload,
        _ => unreachable!(),
    };
    function.owners.extend([
        owner(OwnerKind::Local { mutable: true }, at(span, 8)),
        owner(OwnerKind::Temporary, at(span, 10)),
        owner(
            OwnerKind::StagedArgument {
                call: CallSiteId(0),
                argument: 0,
            },
            at(span, 13),
        ),
        owner(
            OwnerKind::CallResult {
                call: CallSiteId(0),
            },
            at(span, 15),
        ),
        owner(OwnerKind::Local { mutable: false }, at(span, 17)),
    ]);
    function.calls.push(CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(3))],
        result: CallResult::Owned(OwnerPlaceId(4)),
        parent: None,
        span: at(span, 13),
    });
    let mut preceding = std::mem::take(&mut function.blocks[0].statements);
    preceding.extend([
        f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), at(span, 8)),
        f::instruction(
            OwnedInstruction::MoveInitialize {
                destination: OwnerPlaceId(1),
                source: OwnerPlaceId(0),
            },
            at(span, 9),
        ),
        f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(2)), at(span, 10)),
        f::instruction(
            OwnedInstruction::ConstructEnum {
                destination: OwnerPlaceId(2),
                variant: variant(constructed),
                payload,
            },
            at(span, 11),
        ),
        f::instruction(
            OwnedInstruction::Replace {
                destination: OwnerPlaceId(1),
                source: OwnerPlaceId(2),
            },
            at(span, 12),
        ),
        f::instruction(OwnedInstruction::OpenCall(CallSiteId(0)), at(span, 13)),
        f::instruction(
            OwnedInstruction::PrepareOwned {
                call: CallSiteId(0),
                argument: 0,
                source: OwnerPlaceId(1),
            },
            at(span, 14),
        ),
    ]);
    // Shift existing canonical blocks by one; the new entry invokes the relay.
    function.blocks.insert(
        0,
        block(
            preceding,
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            at(span, 15),
        ),
    );
    for arm in &mut function.matches[0].arms {
        arm.dispatch.0 += 1;
        arm.entry.0 += 1;
    }
    function.matches[0].source = OwnerPlaceId(5);
    function.matches[0].span = at(span, 30);
    function.blocks[1].statements = vec![
        f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(5)), at(span, 17)),
        f::instruction(
            OwnedInstruction::MoveInitialize {
                destination: OwnerPlaceId(5),
                source: OwnerPlaceId(4),
            },
            at(span, 18),
        ),
    ];
    for arm in 0..4 {
        function.blocks[1 + arm].terminator.as_mut().unwrap().span = at(span, 30);
        let entry = &mut function.blocks[5 + arm];
        entry.statements[0].span = at(span, 30);
        entry.statements.truncate(1);
        for owner in [0, 1, 2, 4, 5] {
            entry.statements.push(f::instruction(
                OwnedInstruction::StorageEnd(OwnerPlaceId(owner)),
                at(span, 31),
            ));
        }
        let returned = match entry.terminator.as_ref().unwrap().kind {
            OwnedTerminatorKind::ReturnScalar(value) => value.local.0,
            _ => unreachable!(),
        };
        entry.terminator = f::end(
            OwnedTerminatorKind::ReturnScalar(f::operand(returned, at(span, 32))),
            at(span, 32),
        );
    }
    let locals = function.locals.len();
    let mut callee = f::function(
        1,
        ValueTy::Owned(AggregateTy::Enum(EnumId(0))),
        at(span, 50),
    );
    callee
        .parameters
        .push(ParameterBinding::Owned(OwnerPlaceId(0)));
    callee
        .owners
        .push(owner(OwnerKind::Parameter { position: 0 }, at(span, 50)));
    callee.blocks.push(block(
        vec![],
        OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
        at(span, 16),
    ));
    raw.functions.push(callee);
    // Six owners each contribute 2+4 cells, one argument snapshot, one
    // call-state pair, all scalar locals, and one root entry unit.
    let mut events = vec![(span, 40 + locals), (at(span, 1), 1)];
    if payload.is_some() {
        events.push((at(span, 2), 1));
    }
    events.extend(
        [
            (3, 1),
            (4, 3),
            (8, 1),
            (9, 3),
            (10, 1),
            (11, 3),
            (12, 5),
            (13, 2),
            (14, 3),
            (15, 10),
            (16, 5),
            (17, 1),
            (18, 3),
        ]
        .map(|(i, cost)| (at(span, i), cost)),
    );
    let selected = [3, 0, 2, 1]
        .iter()
        .position(|&index| index == constructed)
        .unwrap();
    events.extend((0..=selected).map(|_| (at(span, 30), 1)));
    events.push((at(span, 30), 3));
    events.extend((0..5).map(|_| (at(span, 31), 3)));
    events.push((at(span, 32), 14));
    (
        raw,
        f::Schedule {
            entry: base.entry,
            result: base.result,
            events,
        },
    )
}

/// Three iterations restore the owner lifetime before the backedge. Dispatch
/// edges are essential to the cycle. The i32 binding increments a mutable place.
pub(super) fn loop_case(span: Span) -> (RawOwnedProgram, f::Schedule) {
    let mut function = f::function(0, ValueTy::Scalar(hir::Ty::I32), span);
    for ty in [
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::Bool,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
    ] {
        function.locals.push(f::scalar(ty, span));
    }
    function.locals[5].kind = LocalKind::Binding;
    function.places.push(PlaceDecl {
        ty: hir::Ty::I32,
        span: at(span, 4),
    });
    function
        .owners
        .push(owner(OwnerKind::Local { mutable: false }, at(span, 9)));
    let place = |i| Place {
        id: PlaceId(0),
        span: at(span, i),
    };
    function.blocks.push(block(
        vec![
            f::assign(0, Rvalue::I32(0), at(span, 1)),
            f::assign(1, Rvalue::I32(1), at(span, 2)),
            f::assign(2, Rvalue::I32(3), at(span, 3)),
            f::instruction(
                OwnedInstruction::Scalar(Statement::Initialize {
                    place: place(4),
                    value: f::operand(0, at(span, 4)),
                    span: at(span, 4),
                }),
                at(span, 4),
            ),
        ],
        OwnedTerminatorKind::Goto(BlockId(1)),
        at(span, 5),
    ));
    function.blocks.push(block(
        vec![
            f::assign(3, Rvalue::Load(place(6)), at(span, 6)),
            f::assign(
                4,
                Rvalue::CompareScalar {
                    op: hir::ComparisonOp::Less,
                    left: f::operand(3, at(span, 7)),
                    right: f::operand(2, at(span, 7)),
                    operator_span: at(span, 7),
                },
                at(span, 7),
            ),
        ],
        OwnedTerminatorKind::Branch {
            condition: f::operand(4, at(span, 8)),
            then_block: BlockId(2),
            else_block: BlockId(5),
        },
        at(span, 8),
    ));
    function.matches.push(MatchDecl {
        source: OwnerPlaceId(0),
        span: at(span, 11),
        arms: vec![
            MatchArm {
                variant: variant(0),
                dispatch: BlockId(2),
                entry: BlockId(4),
            },
            MatchArm {
                variant: variant(1),
                dispatch: BlockId(3),
                entry: BlockId(6),
            },
        ],
    });
    function.blocks.push(block(
        vec![
            f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), at(span, 9)),
            f::instruction(
                OwnedInstruction::ConstructEnum {
                    destination: OwnerPlaceId(0),
                    variant: variant(1),
                    payload: Some(f::operand(3, at(span, 10))),
                },
                at(span, 10),
            ),
        ],
        OwnedTerminatorKind::MatchDispatch {
            match_id: MatchId(0),
            arm: 0,
        },
        at(span, 11),
    ));
    function.blocks.push(block(
        vec![],
        OwnedTerminatorKind::MatchDispatch {
            match_id: MatchId(0),
            arm: 1,
        },
        at(span, 11),
    ));
    let arm_block = |arm, binding, source, destination| {
        block(
            vec![
                f::instruction(
                    OwnedInstruction::ConsumeVariant {
                        match_id: MatchId(0),
                        arm,
                        destination: binding,
                    },
                    at(span, 11),
                ),
                f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), at(span, 12)),
                f::assign(
                    destination,
                    Rvalue::CheckedI32 {
                        op: hir::ArithmeticOp::Add,
                        left: f::operand(source, at(span, 13)),
                        right: f::operand(1, at(span, 13)),
                        operator_span: at(span, 13),
                    },
                    at(span, 13),
                ),
                f::instruction(
                    OwnedInstruction::Scalar(Statement::Store {
                        place: place(14),
                        value: f::operand(destination, at(span, 14)),
                        operator_span: at(span, 14),
                        span: at(span, 14),
                    }),
                    at(span, 14),
                ),
            ],
            OwnedTerminatorKind::Goto(BlockId(1)),
            at(span, 15),
        )
    };
    function.blocks.push(arm_block(0, None, 3, 7));
    function.blocks.push(block(
        vec![],
        OwnedTerminatorKind::ReturnScalar(f::operand(3, at(span, 16))),
        at(span, 16),
    ));
    function.blocks.push(arm_block(1, Some(LocalId(5)), 5, 6));
    // 8 immutable scalar locals + 1 place + 2 value cells + 4 state
    // cells + 1 entry unit = 16. Return visits the two value cells.
    let mut events = vec![(span, 16)];
    events.extend((1..=5).map(|i| (at(span, i), 1)));
    for _ in 0..3 {
        events.extend(
            [
                (6, 1),
                (7, 1),
                (8, 1),
                (9, 1),
                (10, 3),
                (11, 1),
                (11, 1),
                (11, 3),
                (12, 3),
                (13, 1),
                (14, 1),
                (15, 1),
            ]
            .map(|(i, cost)| (at(span, i), cost)),
        );
    }
    events.extend([(6, 1), (7, 1), (8, 1), (16, 3)].map(|(i, cost)| (at(span, i), cost)));
    (
        RawOwnedProgram {
            builtins: BuiltinOrigins::None,
            enums: vec![enumeration(&[None, Some(hir::Ty::I32)], span)],
            records: vec![],
            functions: vec![function],
        },
        f::Schedule {
            entry: hir::DefId(0),
            result: Scalar::I32(3),
            events,
        },
    )
}

/// Explicit discard followed by payload-free teardown. The second slot stays
/// uninitialized until StorageEnd, and the third is left uninitialized at return.
pub(super) fn discard_case(payload: Option<hir::Ty>, span: Span) -> (RawOwnedProgram, f::Schedule) {
    let mut function = f::function(0, ValueTy::Scalar(hir::Ty::Unit), span);
    function.locals.push(f::scalar(hir::Ty::Unit, span));
    let mut statements = vec![f::assign(0, Rvalue::Unit, at(span, 1))];
    let operand = payload.map(|ty| {
        function.locals.push(f::scalar(ty, span));
        statements.push(f::assign(1, literal(scalar_value(ty)), at(span, 2)));
        f::operand(1, at(span, 4))
    });
    for _ in 0..3 {
        function
            .owners
            .push(owner(OwnerKind::Local { mutable: false }, span));
    }
    statements.extend([
        f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), at(span, 3)),
        f::instruction(
            OwnedInstruction::ConstructEnum {
                destination: OwnerPlaceId(0),
                variant: variant(0),
                payload: operand,
            },
            at(span, 4),
        ),
        f::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), at(span, 5)),
        f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), at(span, 6)),
        f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), at(span, 8)),
        f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(1)), at(span, 9)),
        f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(2)), at(span, 10)),
    ]);
    function.blocks.push(block(
        statements,
        OwnedTerminatorKind::ReturnScalar(f::operand(0, at(span, 7))),
        at(span, 7),
    ));
    let mut events = vec![
        (span, 20 + usize::from(payload.is_some())),
        (at(span, 1), 1),
    ];
    if payload.is_some() {
        events.push((at(span, 2), 1));
    }
    events.extend(
        [
            (3, 1),
            (4, 3),
            (5, 3),
            (6, 3),
            (8, 1),
            (9, 3),
            (10, 1),
            (7, 7),
        ]
        .map(|(i, cost)| (at(span, i), cost)),
    );
    (
        RawOwnedProgram {
            builtins: BuiltinOrigins::None,
            enums: vec![enumeration(&[payload], span)],
            records: vec![],
            functions: vec![function],
        },
        f::Schedule {
            entry: hir::DefId(0),
            result: Scalar::Unit,
            events,
        },
    )
}

/// A bounded cycle covers each payload kind as both old and new storage.
pub(super) const REPLACEMENT_PAIRS: [(usize, usize); 4] = [(0, 1), (1, 2), (2, 3), (3, 0)];

/// Replace an available or moved destination with a different variant, then
/// consume the new tag/payload. Spans: fallback1, new payload2, old live3,
/// match5, cleanup6, return7, old payload8, old constructor9, new live10,
/// new constructor11, replace12, optional old discard19.
pub(super) fn replacement_case(
    initial: usize,
    replacement: usize,
    moved_destination: bool,
    span: Span,
) -> (RawOwnedProgram, f::Schedule) {
    assert_ne!(initial, replacement);
    let result = MIXED[replacement].unwrap_or(hir::Ty::I32);
    let (mut raw, base) = mixed_case(&MIXED, &[3, 0, 2, 1], replacement, result, span);
    let function = &mut raw.functions[0];
    function.owners[0].kind = OwnerKind::Local { mutable: true };
    function
        .owners
        .push(owner(OwnerKind::Temporary, at(span, 10)));
    let mut new_statements = std::mem::take(&mut function.blocks[0].statements);
    let new_constructor = new_statements.pop().unwrap();
    let OwnedInstruction::ConstructEnum { payload, .. } = new_constructor.kind else {
        unreachable!("mixed fixture constructor")
    };
    new_statements.pop(); // The old destination has its own StorageLive below.
    let new_payload = if payload.is_some() {
        Some(new_statements.pop().unwrap())
    } else {
        None
    };
    let mut statements = new_statements; // The independently assigned fallback.
    let old_payload = MIXED[initial].map(|ty| {
        let id = local(function, ty, LocalKind::Temporary, at(span, 8));
        statements.push(f::assign(id.0, literal(scalar_value(ty)), at(span, 8)));
        f::operand(id.0, at(span, 9))
    });
    statements.extend([
        f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), at(span, 3)),
        f::instruction(
            OwnedInstruction::ConstructEnum {
                destination: OwnerPlaceId(0),
                variant: variant(initial),
                payload: old_payload,
            },
            at(span, 9),
        ),
    ]);
    if moved_destination {
        statements.push(f::instruction(
            OwnedInstruction::Discard(OwnerPlaceId(0)),
            at(span, 19),
        ));
    }
    statements.extend(new_payload);
    statements.extend([
        f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), at(span, 10)),
        f::instruction(
            OwnedInstruction::ConstructEnum {
                destination: OwnerPlaceId(1),
                variant: variant(replacement),
                payload,
            },
            at(span, 11),
        ),
        f::instruction(
            OwnedInstruction::Replace {
                destination: OwnerPlaceId(0),
                source: OwnerPlaceId(1),
            },
            at(span, 12),
        ),
    ]);
    function.blocks[0].statements = statements;
    for entry in &mut function.blocks[4..] {
        entry.statements.push(f::instruction(
            OwnedInstruction::StorageEnd(OwnerPlaceId(1)),
            at(span, 6),
        ));
    }
    // One root entry + two (2-value + 4-state) owners + every scalar local.
    // The written arm order, cleanup widths and return inventory are fixed.
    let mut events = vec![(span, 13 + function.locals.len()), (at(span, 1), 1)];
    if old_payload.is_some() {
        events.push((at(span, 8), 1));
    }
    events.extend([(at(span, 3), 1), (at(span, 9), 3)]);
    if moved_destination {
        events.push((at(span, 19), 3));
    }
    if payload.is_some() {
        events.push((at(span, 2), 1));
    }
    events.extend([(at(span, 10), 1), (at(span, 11), 3), (at(span, 12), 5)]);
    let selected = [3, 0, 2, 1]
        .iter()
        .position(|&index| index == replacement)
        .unwrap();
    events.extend((0..=selected).map(|_| (at(span, 5), 1)));
    events.extend([
        (at(span, 5), 3),
        (at(span, 6), 3),
        (at(span, 6), 3),
        (at(span, 7), 5),
    ]);
    (
        raw,
        f::Schedule {
            entry: base.entry,
            result: base.result,
            events,
        },
    )
}

/// Two owned inputs: an i32 enum first and a bool/unit enum second. Caller
/// owners0/1 stage into owners2/3; the callee owns parameters0/1. Spans:
/// scalar1/2, live3/5, constructor4/6, open7, prepare8/9, Invoke10,
/// cleanup11/12, caller return13, callee origin30, first body effect40,
/// callee discard41/42 and return43. Incoming faults retain origin30.
pub(super) fn two_owned_case(second: usize, span: Span) -> (RawOwnedProgram, f::Schedule) {
    assert!(second == 1 || second == 3);
    let mut caller = f::function(0, ValueTy::Scalar(hir::Ty::I32), span);
    caller.locals = vec![
        f::scalar(hir::Ty::I32, at(span, 1)),
        f::scalar(MIXED[second].unwrap(), at(span, 2)),
        f::scalar(hir::Ty::I32, at(span, 10)),
    ];
    for _ in 0..2 {
        caller
            .owners
            .push(owner(OwnerKind::Local { mutable: false }, span));
    }
    for argument in 0..2 {
        caller.owners.push(owner(
            OwnerKind::StagedArgument {
                call: CallSiteId(0),
                argument,
            },
            at(span, 7),
        ));
    }
    caller.calls.push(CallDecl {
        target: hir::DefId(1),
        arguments: vec![
            ArgumentSlot::Owned(OwnerPlaceId(2)),
            ArgumentSlot::Owned(OwnerPlaceId(3)),
        ],
        result: CallResult::Scalar(LocalId(2)),
        parent: None,
        span: at(span, 7),
    });
    let mut statements = vec![
        f::assign(0, Rvalue::I32(-71), at(span, 1)),
        f::assign(
            1,
            literal(scalar_value(MIXED[second].unwrap())),
            at(span, 2),
        ),
    ];
    for (destination, constructed, live, construct) in [(0, 2, 3, 4), (1, second, 5, 6)] {
        statements.extend([
            f::instruction(
                OwnedInstruction::StorageLive(OwnerPlaceId(destination)),
                at(span, live),
            ),
            f::instruction(
                OwnedInstruction::ConstructEnum {
                    destination: OwnerPlaceId(destination),
                    variant: variant(constructed),
                    payload: Some(f::operand(destination, at(span, construct))),
                },
                at(span, construct),
            ),
        ]);
    }
    statements.push(f::instruction(
        OwnedInstruction::OpenCall(CallSiteId(0)),
        at(span, 7),
    ));
    for argument in 0..2 {
        statements.push(f::instruction(
            OwnedInstruction::PrepareOwned {
                call: CallSiteId(0),
                argument,
                source: OwnerPlaceId(argument),
            },
            at(span, 8 + argument),
        ));
    }
    caller.blocks.push(block(
        statements,
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        at(span, 10),
    ));
    caller.blocks.push(block(
        vec![
            f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), at(span, 11)),
            f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(1)), at(span, 12)),
        ],
        OwnedTerminatorKind::ReturnScalar(f::operand(2, at(span, 13))),
        at(span, 13),
    ));
    let mut callee = f::function(1, ValueTy::Scalar(hir::Ty::I32), at(span, 30));
    callee.locals.push(f::scalar(hir::Ty::I32, at(span, 40)));
    for position in 0..2 {
        callee
            .parameters
            .push(ParameterBinding::Owned(OwnerPlaceId(position)));
        callee
            .owners
            .push(owner(OwnerKind::Parameter { position }, at(span, 30)));
    }
    callee.blocks.push(block(
        vec![
            f::assign(0, Rvalue::I32(73), at(span, 40)),
            f::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), at(span, 41)),
            f::instruction(OwnedInstruction::Discard(OwnerPlaceId(1)), at(span, 42)),
        ],
        OwnedTerminatorKind::ReturnScalar(f::operand(0, at(span, 43))),
        at(span, 43),
    ));
    // Root: 1 entry + 3 locals + 4*6 owner cells + 2 argument snapshots
    // + 2 call-state cells = 32. Invoke: 1 + 2 arguments + (1 local +
    // 2*6 owner cells) child activation + 2*2 incoming value cells = 20.
    // Callee return costs 1+4; caller return costs 1+8+1 call = 10.
    let mut events = vec![(span, 32)];
    events.extend(
        [
            (1, 1),
            (2, 1),
            (3, 1),
            (4, 3),
            (5, 1),
            (6, 3),
            (7, 3),
            (8, 3),
            (9, 3),
            (10, 20),
            (40, 1),
            (41, 3),
            (42, 3),
            (43, 5),
            (11, 3),
            (12, 3),
            (13, 10),
        ]
        .map(|(ordinal, cost)| (at(span, ordinal), cost)),
    );
    (
        RawOwnedProgram {
            builtins: BuiltinOrigins::None,
            enums: vec![enumeration(&MIXED, span)],
            records: vec![],
            functions: vec![caller, callee],
        },
        f::Schedule {
            entry: hir::DefId(0),
            result: Scalar::I32(73),
            events,
        },
    )
}
