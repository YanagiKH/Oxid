//! Inert raw output candidates. All consumer authority still comes from the
//! production verifier; these constructors confer no builtin identity.
use super::consumer_fixtures as f;
use super::enum_consumer_fixtures as e;
use super::*;

pub(super) fn enumeration(id: usize, payload_member: usize, span: Span) -> RawEnumDecl {
    RawEnumDecl {
        id: EnumId(id),
        span,
        variants: (0..3)
            .map(|index| RawVariantDecl {
                id: VariantId {
                    enumeration: EnumId(id),
                    index,
                },
                payload: (index == payload_member)
                    .then_some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32))),
                span,
            })
            .collect(),
    }
}

fn builtin(id: usize, enumeration: usize, output: bool, span: Span) -> RawOwnedFunction {
    let mut function = f::function(
        id,
        ValueTy::Owned(AggregateTy::Enum(EnumId(enumeration))),
        span,
    );
    function.parameters = vec![ParameterBinding::Reference(ReferenceParamId(0))];
    function.references = vec![ReferenceDecl {
        position: 0,
        referent: BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::I32)).unwrap(),
        kind: if output {
            BorrowKind::Shared
        } else {
            BorrowKind::Exclusive
        },
        span,
    }];
    function.owners = vec![OwnerDecl {
        kind: OwnerKind::Temporary,
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Enum(EnumId(enumeration)))
            .unwrap(),
        span,
    }];
    function.blocks = vec![e::block(
        vec![
            f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), span),
            f::instruction(
                if output {
                    OwnedInstruction::WriteStdout {
                        buffer: ReferenceParamId(0),
                        destination: OwnerPlaceId(0),
                    }
                } else {
                    OwnedInstruction::ReadStdin {
                        buffer: ReferenceParamId(0),
                        destination: OwnerPlaceId(0),
                    }
                },
                span,
            ),
        ],
        OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
        span,
    )];
    function
}

/// One ordinary enum and one ordinary function precede independently ranked
/// suffixes. The ordinary enum deliberately has the output status shape.
pub(super) fn inventory(inventory: BuiltinOrigins) -> (SourceMap, RawOwnedProgram) {
    let (sources, s) = f::context();
    let mut main = f::function(0, ValueTy::Scalar(hir::Ty::I32), s(0));
    main.locals = vec![f::scalar(hir::Ty::I32, s(1))];
    main.blocks = vec![e::block(
        vec![f::assign(0, Rvalue::I32(0), s(1))],
        OwnedTerminatorKind::ReturnScalar(f::operand(0, s(2))),
        s(2),
    )];
    // Independently authored finite case inventory: no catalog/rank helper.
    let (read_enum, read_function, write_enum, write_function) = match inventory {
        BuiltinOrigins::None => (false, false, false, false),
        BuiltinOrigins::ReadStatus => (true, false, false, false),
        BuiltinOrigins::ReadStdin => (true, true, false, false),
        BuiltinOrigins::WriteStatus => (false, false, true, false),
        BuiltinOrigins::WriteStdout => (false, false, true, true),
        BuiltinOrigins::ReadStatusWriteStatus => (true, false, true, false),
        BuiltinOrigins::ReadStatusWriteStdout => (true, false, true, true),
        BuiltinOrigins::ReadStdinWriteStatus => (true, true, true, false),
        BuiltinOrigins::ReadStdinWriteStdout => (true, true, true, true),
    };
    let mut raw = RawOwnedProgram {
        builtins: inventory,
        records: vec![],
        enums: vec![enumeration(0, 2, s(10))],
        functions: vec![main],
    };
    if read_enum {
        raw.enums.push(enumeration(1, 0, s(100)));
    }
    if write_enum {
        raw.enums
            .push(enumeration(if read_enum { 2 } else { 1 }, 2, s(200)));
    }
    if read_function {
        raw.functions.push(builtin(1, 1, false, s(300)));
    }
    if write_function {
        raw.functions.push(builtin(
            if read_function { 2 } else { 1 },
            if read_enum { 2 } else { 1 },
            true,
            s(400),
        ));
    }
    (sources, raw)
}

/// A shared borrow of an immutable, fully initialized array is sufficient.
/// The caller disposes of the returned status through ordinary ownership.
pub(super) fn program(capacity: usize) -> (SourceMap, RawOwnedProgram, hir::DefId) {
    let (sources, mut raw) = inventory(BuiltinOrigins::WriteStdout);
    let main = &mut raw.functions[0];
    let span = main.span;
    let s = |index| e::at(span, index);
    let array = AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, capacity).unwrap());
    main.owners = vec![
        OwnerDecl {
            kind: OwnerKind::Local { mutable: false },
            aggregate: AggregateSlot::try_from_aggregate(array).unwrap(),
            span: s(2),
        },
        OwnerDecl {
            kind: OwnerKind::CallResult {
                call: CallSiteId(0),
            },
            aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Enum(EnumId(1))).unwrap(),
            span: s(4),
        },
    ];
    main.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
        result: CallResult::Owned(OwnerPlaceId(1)),
        parent: None,
        span: s(4),
    }];
    main.loans = vec![LoanDecl {
        call: CallSiteId(0),
        argument: 0,
        authority: AccessBase::Owner(OwnerPlaceId(0)),
        projection: vec![],
        kind: BorrowKind::Shared,
        referent: BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::I32)).unwrap(),
        span: s(5),
    }];
    main.blocks = vec![
        e::block(
            vec![
                f::assign(0, Rvalue::I32(37), s(1)),
                f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), s(2)),
                f::instruction(
                    OwnedInstruction::ConstructArray {
                        destination: OwnerPlaceId(0),
                        elements: vec![f::operand(0, s(3)); capacity],
                    },
                    s(3),
                ),
                f::instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(4)),
                f::instruction(
                    OwnedInstruction::PrepareBorrow {
                        call: CallSiteId(0),
                        argument: 0,
                        loan: LoanId(0),
                    },
                    s(5),
                ),
            ],
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            s(6),
        ),
        e::block(
            vec![
                f::instruction(OwnedInstruction::Discard(OwnerPlaceId(1)), s(7)),
                f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(1)), s(8)),
                f::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), s(9)),
                f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), s(10)),
            ],
            OwnedTerminatorKind::ReturnScalar(f::operand(0, s(11))),
            s(11),
        ),
    ];
    (sources, raw, hir::DefId(0))
}

pub(super) fn ordinary_control(capacity: usize) -> (SourceMap, RawOwnedProgram, hir::DefId) {
    let (sources, mut raw, entry) = program(capacity);
    raw.builtins = BuiltinOrigins::None;
    raw.functions[1].blocks[0].statements[1].kind = OwnedInstruction::ConstructEnum {
        destination: OwnerPlaceId(0),
        variant: VariantId {
            enumeration: EnumId(1),
            index: 0,
        },
        payload: None,
    };
    (sources, raw, entry)
}

/// Reuse the established ordinary projection/forwarding graphs. Their status
/// payload consumer moves to the independently specified output member two;
/// no execution expectations are inherited from the input fixture.
fn output_view_graph(mut raw: RawOwnedProgram) -> RawOwnedProgram {
    raw.builtins = BuiltinOrigins::WriteStdout;
    raw.enums[0].variants[0].payload = None;
    raw.enums[0].variants[2].payload = Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32)));
    for function in &mut raw.functions {
        for reference in &mut function.references {
            reference.kind = BorrowKind::Shared;
        }
        for loan in &mut function.loans {
            loan.kind = BorrowKind::Shared;
        }
        for descriptor in &mut function.matches {
            for arm in &mut descriptor.arms {
                arm.variant.index = [2, 0, 1][arm.variant.index];
            }
        }
        for block in &mut function.blocks {
            for statement in &mut block.statements {
                if let OwnedInstruction::ReadStdin {
                    buffer,
                    destination,
                } = statement.kind
                {
                    statement.kind = OwnedInstruction::WriteStdout {
                        buffer,
                        destination,
                    };
                }
            }
        }
    }
    raw
}

pub(super) fn projected_program() -> (SourceMap, RawOwnedProgram, hir::DefId) {
    let (sources, raw, entry) = super::builtin_input_fixtures::projected_record_program();
    (sources, output_view_graph(raw), entry)
}

pub(super) fn forwarded_program() -> (SourceMap, RawOwnedProgram, hir::DefId) {
    let (sources, raw, entry) = super::builtin_input_fixtures::forwarded_program(
        super::builtin_input_fixtures::Observation::Status,
    );
    (sources, output_view_graph(raw), entry)
}
