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
        let mut sum = main.locals.len();
        main.locals.push(f::scalar(hir::Ty::I32, s(30)));
        let mut statements = vec![f::assign(sum, Rvalue::I32(0), s(30))];
        // Each SSA temporary is assigned once. Even at capacity 1024 this is a
        // bounded, acyclic observation with no mutable scalar or loop machinery.
        for index in 0..capacity {
            let offset = main.locals.len();
            let value = offset + 1;
            let next_sum = offset + 2;
            let origin = s(100 + index * 3);
            main.locals
                .extend((0..3).map(|_| f::scalar(hir::Ty::I32, origin)));
            statements.extend([
                f::assign(offset, Rvalue::I32(index as i32), origin),
                f::instruction(
                    OwnedInstruction::ReadIndex {
                        destination: LocalId(value),
                        base: AccessBase::Owner(OwnerPlaceId(0)),
                        index: f::operand(offset, origin),
                    },
                    origin,
                ),
                f::assign(
                    next_sum,
                    Rvalue::CheckedI32 {
                        op: hir::ArithmeticOp::Add,
                        left: f::operand(sum, origin),
                        right: f::operand(value, origin),
                        operator_span: origin,
                    },
                    origin,
                ),
            ]);
            sum = next_sum;
        }
        statements.extend(cleanup_buffer(s(3200)));
        main.blocks.push(e::block(
            statements,
            OwnedTerminatorKind::ReturnScalar(f::operand(sum, s(3201))),
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
