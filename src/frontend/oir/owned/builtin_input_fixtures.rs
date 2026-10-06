//! Independent raw input programs. These rows are untrusted verifier inputs,
//! not source associations, builtin identities, or executable witnesses.
use super::consumer_fixtures as f;
use super::enum_consumer_fixtures as e;
use super::*;

pub(super) const SENTINEL: i32 = -7;
pub(super) const FULL: i32 = -1;
pub(super) const IO_ERROR: i32 = -2;
pub(super) const CAPACITIES: [usize; 5] = [0, 1, 3, 129, 1024];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Observation {
    /// Eof(n) returns n, Full returns -1, and IoError returns -2.
    Status,
    /// Sum all cells after consuming the result, including the unchanged tail.
    Checksum,
}

fn cleanup_buffer(span: Span) -> [OwnedStatement; 2] {
    [
        f::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), span),
        f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), span),
    ]
}

/// Main is DefId(0); the sole canonical trailing builtin is DefId(1).
/// Owners in main are the mutable array, call result, and named match subject.
/// The single exclusive loan covers the complete fixed array as an i32 slice.
pub(super) fn program(
    capacity: usize,
    observation: Observation,
) -> (SourceMap, RawOwnedProgram, hir::DefId) {
    assert!(
        CAPACITIES.contains(&capacity),
        "unknown input fixture capacity"
    );
    let (sources, s) = f::context();
    let mut main = f::function(0, ValueTy::Scalar(hir::Ty::I32), s(0));
    main.locals = vec![
        f::scalar(hir::Ty::I32, s(1)),
        f::scalar(hir::Ty::I32, s(2)),
        f::scalar(hir::Ty::I32, s(3)),
        LocalDecl {
            ty: hir::Ty::I32,
            kind: LocalKind::Binding,
            span: s(20),
        },
    ];
    main.owners = vec![
        OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(AggregateTy::FixedArray(
                FixedArrayTy::check(hir::Ty::I32, capacity).unwrap(),
            ))
            .unwrap(),
            kind: OwnerKind::Local { mutable: true },
            span: s(4),
        },
        e::owner(
            OwnerKind::CallResult {
                call: CallSiteId(0),
            },
            s(8),
        ),
        e::owner(OwnerKind::Local { mutable: false }, s(9)),
    ];
    main.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
        result: CallResult::Owned(OwnerPlaceId(1)),
        parent: None,
        span: s(6),
    }];
    main.loans = vec![LoanDecl {
        call: CallSiteId(0),
        argument: 0,
        authority: AccessBase::Owner(OwnerPlaceId(0)),
        projection: vec![],
        kind: BorrowKind::Exclusive,
        referent: BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::I32)).unwrap(),
        span: s(7),
    }];
    main.matches = vec![MatchDecl {
        source: OwnerPlaceId(2),
        arms: (0..3)
            .map(|arm| MatchArm {
                variant: e::variant(arm),
                dispatch: BlockId(1 + arm),
                entry: BlockId(4 + arm),
            })
            .collect(),
        span: s(20),
    }];
    main.blocks.push(e::block(
        vec![
            f::assign(0, Rvalue::I32(SENTINEL), s(1)),
            f::assign(1, Rvalue::I32(FULL), s(2)),
            f::assign(2, Rvalue::I32(IO_ERROR), s(3)),
            f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(4)),
            f::instruction(
                OwnedInstruction::ConstructArray {
                    destination: OwnerPlaceId(0),
                    elements: vec![f::operand(0, s(5)); capacity],
                },
                s(5),
            ),
            f::instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(6)),
            f::instruction(
                OwnedInstruction::PrepareBorrow {
                    call: CallSiteId(0),
                    argument: 0,
                    loan: LoanId(0),
                },
                s(7),
            ),
        ],
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        s(8),
    ));
    for arm in 0..3 {
        main.blocks.push(e::block(
            if arm == 0 {
                vec![
                    f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(2)), s(9)),
                    f::instruction(
                        OwnedInstruction::MoveInitialize {
                            destination: OwnerPlaceId(2),
                            source: OwnerPlaceId(1),
                        },
                        s(10),
                    ),
                    f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(1)), s(11)),
                ]
            } else {
                vec![]
            },
            OwnedTerminatorKind::MatchDispatch {
                match_id: MatchId(0),
                arm,
            },
            s(20),
        ));
    }
    for arm in 0..3 {
        let mut statements = vec![
            f::instruction(
                OwnedInstruction::ConsumeVariant {
                    match_id: MatchId(0),
                    arm,
                    destination: (arm == 0).then_some(LocalId(3)),
                },
                s(20),
            ),
            f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(2)), s(21)),
        ];
        let terminator = match observation {
            Observation::Status => {
                statements.extend(cleanup_buffer(s(22)));
                OwnedTerminatorKind::ReturnScalar(f::operand(if arm == 0 { 3 } else { arm }, s(23)))
            }
            Observation::Checksum => OwnedTerminatorKind::Goto(BlockId(7)),
        };
        main.blocks.push(e::block(statements, terminator, s(23)));
    }
    if observation == Observation::Checksum {
        // Ten temporaries and two mutable places suffice at every capacity.
        // The header checks index < capacity before each read, including the
        // zero-capacity case. Only the index and running sum cross the backedge.
        main.locals.extend(
            [
                hir::Ty::I32,  // 4: zero
                hir::Ty::I32,  // 5: one
                hir::Ty::I32,  // 6: capacity
                hir::Ty::I32,  // 7: current index
                hir::Ty::Bool, // 8: loop condition
                hir::Ty::I32,  // 9: cell
                hir::Ty::I32,  // 10: current sum
                hir::Ty::I32,  // 11: next sum
                hir::Ty::I32,  // 12: next index
                hir::Ty::I32,  // 13: result
            ]
            .into_iter()
            .map(|ty| f::scalar(ty, s(30))),
        );
        main.places = vec![
            PlaceDecl {
                ty: hir::Ty::I32,
                span: s(33),
            },
            PlaceDecl {
                ty: hir::Ty::I32,
                span: s(34),
            },
        ];
        let place = |id, span| Place {
            id: PlaceId(id),
            span,
        };
        main.blocks.push(e::block(
            vec![
                f::assign(4, Rvalue::I32(0), s(30)),
                f::assign(5, Rvalue::I32(1), s(31)),
                f::assign(6, Rvalue::I32(capacity as i32), s(32)),
                f::instruction(
                    OwnedInstruction::Scalar(Statement::Initialize {
                        place: place(0, s(33)),
                        value: f::operand(4, s(33)),
                        span: s(33),
                    }),
                    s(33),
                ),
                f::instruction(
                    OwnedInstruction::Scalar(Statement::Initialize {
                        place: place(1, s(34)),
                        value: f::operand(4, s(34)),
                        span: s(34),
                    }),
                    s(34),
                ),
            ],
            OwnedTerminatorKind::Goto(BlockId(8)),
            s(35),
        ));
        main.blocks.push(e::block(
            vec![
                f::assign(7, Rvalue::Load(place(0, s(36))), s(36)),
                f::assign(
                    8,
                    Rvalue::CompareScalar {
                        op: hir::ComparisonOp::Less,
                        left: f::operand(7, s(37)),
                        right: f::operand(6, s(37)),
                        operator_span: s(37),
                    },
                    s(37),
                ),
            ],
            OwnedTerminatorKind::Branch {
                condition: f::operand(8, s(38)),
                then_block: BlockId(9),
                else_block: BlockId(10),
            },
            s(38),
        ));
        main.blocks.push(e::block(
            vec![
                f::instruction(
                    OwnedInstruction::ReadIndex {
                        destination: LocalId(9),
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        index: f::operand(7, s(39)),
                    },
                    s(39),
                ),
                f::assign(10, Rvalue::Load(place(1, s(40))), s(40)),
                f::assign(
                    11,
                    Rvalue::CheckedI32 {
                        op: hir::ArithmeticOp::Add,
                        left: f::operand(10, s(41)),
                        right: f::operand(9, s(41)),
                        operator_span: s(41),
                    },
                    s(41),
                ),
                f::instruction(
                    OwnedInstruction::Scalar(Statement::Store {
                        place: place(1, s(42)),
                        value: f::operand(11, s(42)),
                        operator_span: s(42),
                        span: s(42),
                    }),
                    s(42),
                ),
                f::assign(
                    12,
                    Rvalue::CheckedI32 {
                        op: hir::ArithmeticOp::Add,
                        left: f::operand(7, s(43)),
                        right: f::operand(5, s(43)),
                        operator_span: s(43),
                    },
                    s(43),
                ),
                f::instruction(
                    OwnedInstruction::Scalar(Statement::Store {
                        place: place(0, s(44)),
                        value: f::operand(12, s(44)),
                        operator_span: s(44),
                        span: s(44),
                    }),
                    s(44),
                ),
            ],
            OwnedTerminatorKind::Goto(BlockId(8)),
            s(45),
        ));
        let mut statements = vec![f::assign(13, Rvalue::Load(place(1, s(46))), s(46))];
        statements.extend(cleanup_buffer(s(3200)));
        main.blocks.push(e::block(
            statements,
            OwnedTerminatorKind::ReturnScalar(f::operand(13, s(3201))),
            s(3201),
        ));
    }

    let anchor = s(4095);
    let mut builtin = f::function(1, ValueTy::Owned(AggregateTy::Enum(EnumId(0))), anchor);
    builtin.parameters = vec![ParameterBinding::Reference(ReferenceParamId(0))];
    builtin.references = vec![ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::I32)).unwrap(),
        kind: BorrowKind::Exclusive,
        position: 0,
        span: anchor,
    }];
    builtin.owners = vec![e::owner(OwnerKind::Temporary, anchor)];
    builtin.blocks = vec![e::block(
        vec![
            f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), anchor),
            f::instruction(
                OwnedInstruction::ReadStdin {
                    buffer: ReferenceParamId(0),
                    destination: OwnerPlaceId(0),
                },
                anchor,
            ),
        ],
        OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
        anchor,
    )];
    (
        sources,
        RawOwnedProgram {
            builtins: BuiltinOrigins::ReadStdin,
            enums: vec![e::enumeration(&[Some(hir::Ty::I32), None, None], anchor)],
            records: vec![],
            functions: vec![main, builtin],
        },
        hir::DefId(0),
    )
}

/// Same frame inventory, with an ordinary nullary constructor in place of the
/// input operation. It has no builtin claim and cannot read host input.
pub(super) fn ordinary_control(
    capacity: usize,
    observation: Observation,
) -> (SourceMap, RawOwnedProgram, hir::DefId) {
    let (sources, mut raw, entry) = program(capacity, observation);
    raw.builtins = BuiltinOrigins::None;
    raw.functions[1].blocks[0].statements[1].kind = OwnedInstruction::ConstructEnum {
        destination: OwnerPlaceId(0),
        variant: e::variant(1),
        payload: None,
    };
    (sources, raw, entry)
}
