//! Independent R2 raw OIR fixtures. No source producer, layout success flag,
//! canonical-match builder, or consumer plan constructs these programs.
use super::consumer_fixtures as f;
use super::*;

fn array(length: usize) -> AggregateTy {
    AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, length).unwrap())
}
fn slot(aggregate: AggregateTy) -> AggregateSlot {
    AggregateSlot::try_from_aggregate(aggregate).unwrap()
}
fn owner(aggregate: AggregateTy, kind: OwnerKind, span: Span) -> OwnerDecl {
    OwnerDecl {
        aggregate: slot(aggregate),
        kind,
        span,
    }
}
fn variant(index: usize) -> VariantId {
    VariantId {
        enumeration: EnumId(0),
        index,
    }
}
fn block(statements: Vec<OwnedStatement>, end: OwnedTerminatorKind, span: Span) -> OwnedBlock {
    OwnedBlock {
        merge: None,
        statements,
        terminator: f::end(end, span),
        span,
    }
}

// Locals: 19, 23, index 0, index 1, left, right, sum, nullary result -7.
// Owners: constructor payload temporary, enum binding, extracted array binding.
// Dispatch blocks 0/1 and guarded entries 2/3 are deliberately authored here.
fn array_case(span: Span, nullary: bool) -> RawOwnedProgram {
    let mut function = f::function(0, ValueTy::Scalar(hir::Ty::I32), span);
    function.locals = (0..8).map(|_| f::scalar(hir::Ty::I32, span)).collect();
    function.owners = vec![
        owner(array(2), OwnerKind::Temporary, span),
        owner(
            AggregateTy::Enum(EnumId(0)),
            OwnerKind::Local { mutable: false },
            span,
        ),
        owner(array(2), OwnerKind::Local { mutable: false }, span),
    ];
    function.matches = vec![MatchDecl {
        source: OwnerPlaceId(1),
        arms: vec![
            MatchArm {
                variant: variant(0),
                dispatch: BlockId(0),
                entry: BlockId(2),
            },
            MatchArm {
                variant: variant(1),
                dispatch: BlockId(1),
                entry: BlockId(3),
            },
        ],
        span,
    }];
    let construct = if nullary {
        OwnedInstruction::ConstructEnum {
            destination: OwnerPlaceId(1),
            variant: variant(1),
            payload: None,
        }
    } else {
        OwnedInstruction::ConstructOwnedEnum {
            destination: OwnerPlaceId(1),
            variant: variant(0),
            payload: OwnerPlaceId(0),
        }
    };
    function.blocks = vec![
        block(
            vec![
                f::assign(0, Rvalue::I32(19), span),
                f::assign(1, Rvalue::I32(23), span),
                f::assign(2, Rvalue::I32(0), span),
                f::assign(3, Rvalue::I32(1), span),
                f::assign(7, Rvalue::I32(-7), span),
                f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), span),
                f::instruction(
                    OwnedInstruction::ConstructArray {
                        destination: OwnerPlaceId(0),
                        elements: vec![f::operand(0, span), f::operand(1, span)],
                    },
                    span,
                ),
                f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), span),
                f::instruction(construct, span),
            ],
            OwnedTerminatorKind::MatchDispatch {
                match_id: MatchId(0),
                arm: 0,
            },
            span,
        ),
        block(
            vec![],
            OwnedTerminatorKind::MatchDispatch {
                match_id: MatchId(0),
                arm: 1,
            },
            span,
        ),
        block(
            vec![
                f::instruction(
                    OwnedInstruction::ConsumeOwnedVariant {
                        match_id: MatchId(0),
                        arm: 0,
                        destination: OwnerPlaceId(2),
                    },
                    span,
                ),
                f::instruction(
                    OwnedInstruction::ReadIndex {
                        destination: LocalId(4),
                        base: AccessBase::Owner(OwnerPlaceId(2)),
                        index: f::operand(2, span),
                    },
                    span,
                ),
                f::instruction(
                    OwnedInstruction::ReadIndex {
                        destination: LocalId(5),
                        base: AccessBase::Owner(OwnerPlaceId(2)),
                        index: f::operand(3, span),
                    },
                    span,
                ),
                f::assign(
                    6,
                    Rvalue::CheckedI32 {
                        op: hir::ArithmeticOp::Add,
                        left: f::operand(4, span),
                        right: f::operand(5, span),
                        operator_span: span,
                    },
                    span,
                ),
                f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(2)), span),
                f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(1)), span),
                f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), span),
            ],
            OwnedTerminatorKind::ReturnScalar(f::operand(6, span)),
            span,
        ),
        block(
            vec![
                f::instruction(
                    OwnedInstruction::ConsumeVariant {
                        match_id: MatchId(0),
                        arm: 1,
                        destination: None,
                    },
                    span,
                ),
                f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(1)), span),
                f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(0)), span),
            ],
            OwnedTerminatorKind::ReturnScalar(f::operand(7, span)),
            span,
        ),
    ];
    RawOwnedProgram {
        builtins: BuiltinOrigins::None,
        records: vec![],
        enums: vec![RawEnumDecl {
            id: EnumId(0),
            span,
            variants: vec![
                RawVariantDecl {
                    id: variant(0),
                    payload: Some(ParameterTy::Value(ValueTy::Owned(array(2)))),
                    span,
                },
                RawVariantDecl {
                    id: variant(1),
                    payload: None,
                    span,
                },
            ],
        }],
        functions: vec![function],
    }
}

fn record(id: usize, span: Span) -> RawRecordDecl {
    let mut declaration = f::record(&[hir::Ty::I32, hir::Ty::I32], span);
    declaration.id = RecordId(id);
    for field in &mut declaration.fields {
        field.id.record = RecordId(id);
    }
    declaration
}

fn record_case(span: Span) -> RawOwnedProgram {
    let mut raw = array_case(span, false);
    raw.records = vec![record(0, span), record(1, span)];
    raw.enums[0].variants[0].payload = Some(ParameterTy::Value(ValueTy::Owned(
        AggregateTy::Record(RecordId(0)),
    )));
    let function = &mut raw.functions[0];
    function.owners[0].aggregate = slot(AggregateTy::Record(RecordId(0)));
    function.owners[2].aggregate = slot(AggregateTy::Record(RecordId(0)));
    function.blocks[0].statements[6].kind = OwnedInstruction::Construct {
        destination: OwnerPlaceId(0),
        fields: (0..2)
            .map(|index| {
                (
                    FieldId {
                        record: RecordId(0),
                        index,
                    },
                    f::operand(index, span),
                )
            })
            .collect(),
    };
    for index in 0..2 {
        function.blocks[2].statements[1 + index].kind = OwnedInstruction::ReadField {
            destination: LocalId(4 + index),
            base: AccessBase::Owner(OwnerPlaceId(2)),
            field: FieldId {
                record: RecordId(0),
                index,
            },
        };
    }
    raw
}

fn reject(raw: RawOwnedProgram, sources: &SourceMap, label: &str) -> OwnedFailure {
    match verified::verify_owned(raw, sources) {
        Err(error) => error,
        Ok(_) => panic!("accepted malformed owned enum: {label}"),
    }
}

#[test]
fn owned_enum_raw_array_record_and_nullary_reference_results() {
    let (sources, s) = f::context();
    for (raw, expected) in [
        (array_case(s(0), false), 42),
        (array_case(s(0), true), -7),
        (record_case(s(0)), 42),
    ] {
        let witness = verified::verify_owned(raw, &sources).unwrap();
        assert_eq!(
            execute::run(&witness, Some(hir::DefId(0))),
            Ok(Scalar::I32(expected))
        );
    }
}

#[test]
fn owned_enum_raw_payload_and_binding_nominal_types_are_exact() {
    let (sources, s) = f::context();
    for binding in [false, true] {
        let mut raw = record_case(s(0));
        if binding {
            raw.functions[0].owners[2].aggregate = slot(AggregateTy::Record(RecordId(1)));
        } else {
            raw.enums[0].variants[0].payload = Some(ParameterTy::Value(ValueTy::Owned(
                AggregateTy::Record(RecordId(1)),
            )));
        }
        assert_eq!(
            reject(raw, &sources, "different nominal record").kind,
            OwnedFailureKind::Malformed(Malformed::Declaration(DeclarationError::TypeMismatch)),
        );
    }
    let mut raw = array_case(s(0), false);
    raw.enums[0].variants[0].payload = Some(ParameterTy::Value(ValueTy::Owned(array(3))));
    assert_eq!(
        reject(raw, &sources, "different array length").kind,
        OwnedFailureKind::Malformed(Malformed::Declaration(DeclarationError::TypeMismatch)),
    );
}

#[test]
fn owned_enum_raw_payload_declarations_cannot_store_references_or_invalid_ids() {
    let (sources, s) = f::context();
    for payload in [
        ParameterTy::Reference {
            referent: BorrowedTy::Exact(array(2)),
            kind: BorrowKind::Shared,
        },
        ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(99)))),
        ParameterTy::Value(ValueTy::Owned(AggregateTy::Enum(EnumId(99)))),
    ] {
        let mut raw = array_case(s(0), false);
        raw.enums[0].variants[0].payload = Some(payload);
        assert!(matches!(
            reject(raw, &sources, "invalid stored payload").kind,
            OwnedFailureKind::Malformed(Malformed::Declaration(_)),
        ));
    }
}

#[test]
fn owned_enum_raw_constructor_consumes_one_complete_available_temporary() {
    let (sources, s) = f::context();
    let mut raw = array_case(s(0), false);
    raw.functions[0].owners[0].kind = OwnerKind::Local { mutable: false };
    assert_eq!(
        reject(raw, &sources, "unstaged payload binding").kind,
        OwnedFailureKind::Malformed(Malformed::OwnerClass),
    );
    let mut raw = array_case(s(0), false);
    raw.functions[0].blocks[0].statements.insert(
        8,
        f::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), s(0)),
    );
    assert_eq!(
        reject(raw, &sources, "moved constructor source").kind,
        OwnedFailureKind::Ownership(Violation::Unavailable),
    );
    let mut raw = array_case(s(0), false);
    raw.functions[0].locals.push(f::scalar(hir::Ty::I32, s(0)));
    raw.functions[0].blocks[0].statements.push(f::instruction(
        OwnedInstruction::ReadIndex {
            destination: LocalId(8),
            base: AccessBase::Owner(OwnerPlaceId(0)),
            index: f::operand(2, s(0)),
        },
        s(0),
    ));
    assert_eq!(
        reject(raw, &sources, "read payload after construction").kind,
        OwnedFailureKind::Ownership(Violation::Unavailable),
    );
    let mut raw = array_case(s(0), false);
    if let OwnedInstruction::ConstructOwnedEnum { payload, .. } =
        &mut raw.functions[0].blocks[0].statements[8].kind
    {
        *payload = OwnerPlaceId(1);
    }
    assert_eq!(
        reject(raw, &sources, "constructor source aliases destination").kind,
        OwnedFailureKind::Malformed(Malformed::OwnerClass),
    );
}

#[test]
fn owned_enum_raw_consume_is_first_unique_guarded_lifetime_start() {
    let (sources, s) = f::context();
    for defect in 0..7 {
        let mut raw = array_case(s(0), false);
        let function = &mut raw.functions[0];
        match defect {
            0 => {
                function.blocks[2].statements.remove(0);
            }
            1 => {
                let duplicate = function.blocks[2].statements[0].clone();
                function.blocks[2].statements.insert(1, duplicate);
            }
            2 => {
                function.locals.push(f::scalar(hir::Ty::I32, s(0)));
                function.blocks[2]
                    .statements
                    .insert(0, f::assign(8, Rvalue::I32(0), s(0)));
            }
            3 => function.blocks[0].statements.push(f::instruction(
                OwnedInstruction::StorageLive(OwnerPlaceId(2)),
                s(0),
            )),
            4 => {
                function.blocks[2].statements[0].kind = OwnedInstruction::ConsumeOwnedVariant {
                    match_id: MatchId(0),
                    arm: 1,
                    destination: OwnerPlaceId(2),
                }
            }
            5 => {
                function.blocks[2].statements[0].kind = OwnedInstruction::ConsumeOwnedVariant {
                    match_id: MatchId(0),
                    arm: 0,
                    destination: OwnerPlaceId(1),
                }
            }
            6 => {
                function.blocks[0].terminator = f::end(OwnedTerminatorKind::Goto(BlockId(2)), s(0))
            }
            _ => unreachable!(),
        }
        assert_eq!(
            reject(raw, &sources, "malformed consuming entry").kind,
            OwnedFailureKind::Malformed(Malformed::CanonicalSite),
            "defect {defect}",
        );
    }
    for kind in [OwnerKind::Temporary, OwnerKind::Local { mutable: true }] {
        let mut raw = array_case(s(0), false);
        raw.functions[0].owners[2].kind = kind;
        assert_eq!(
            reject(raw, &sources, "payload binding must be immutable local").kind,
            OwnedFailureKind::Malformed(Malformed::Binding),
        );
    }
}

#[test]
fn owned_enum_raw_scalar_opcodes_do_not_authorize_owned_payloads() {
    let (sources, s) = f::context();
    for constructor in [false, true] {
        let mut raw = array_case(s(0), false);
        if constructor {
            raw.functions[0].blocks[0].statements[8].kind = OwnedInstruction::ConstructEnum {
                destination: OwnerPlaceId(1),
                variant: variant(0),
                payload: None,
            };
        } else {
            raw.functions[0].blocks[2].statements[0].kind = OwnedInstruction::ConsumeVariant {
                match_id: MatchId(0),
                arm: 0,
                destination: None,
            };
        }
        assert_eq!(
            reject(raw, &sources, "scalar opcode with owned payload").kind,
            OwnedFailureKind::Malformed(Malformed::Declaration(DeclarationError::TypeMismatch)),
        );
    }
}

#[test]
fn owned_enum_raw_extraction_consumes_the_enum_and_binding_use_obeys_moves() {
    let (sources, s) = f::context();
    for original_enum in [true, false] {
        let mut raw = array_case(s(0), false);
        // The first instruction has already moved the selected payload into
        // owner 2. Reusing enum owner 1 must fail; discarding owner 2 and then
        // reading its cells must also fail. Neither is a duplicate consume site.
        let owner = if original_enum { 1 } else { 2 };
        raw.functions[0].blocks[2].statements.insert(
            1,
            f::instruction(OwnedInstruction::Discard(OwnerPlaceId(owner)), s(0)),
        );
        assert_eq!(
            reject(raw, &sources, "reuse after selected payload transfer").kind,
            OwnedFailureKind::Ownership(Violation::Unavailable),
            "original enum: {original_enum}",
        );
    }
}

#[test]
fn owned_enum_raw_match_exhaustiveness_and_enum_availability_are_independent() {
    let (sources, s) = f::context();
    for duplicate in [false, true] {
        let mut raw = array_case(s(0), false);
        if duplicate {
            raw.functions[0].matches[0].arms[1].variant = variant(0);
        } else {
            raw.functions[0].matches[0].arms.pop();
        }
        assert_eq!(
            reject(raw, &sources, "invalid exhaustive arm table").kind,
            OwnedFailureKind::Malformed(Malformed::Type),
        );
    }
    let mut raw = array_case(s(0), false);
    raw.functions[0].blocks[0].statements.push(f::instruction(
        OwnedInstruction::Discard(OwnerPlaceId(1)),
        s(0),
    ));
    assert_eq!(
        reject(raw, &sources, "matching a consumed enum").kind,
        OwnedFailureKind::Ownership(Violation::Unavailable),
    );
}

fn join_case(span: Span, returning: bool, nullary: bool) -> RawOwnedProgram {
    let mut raw = array_case(span, nullary);
    let function = &mut raw.functions[0];
    function
        .owners
        .push(owner(array(2), OwnerKind::Local { mutable: false }, span));
    function.locals.push(f::scalar(hir::Ty::I32, span));
    function.blocks[0].statements.extend([
        f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(3)), span),
        f::instruction(
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(3),
                elements: vec![f::operand(0, span), f::operand(1, span)],
            },
            span,
        ),
    ]);
    function.blocks[2].statements.push(f::instruction(
        OwnedInstruction::Discard(OwnerPlaceId(3)),
        span,
    ));
    if !returning {
        function.blocks[2].terminator = f::end(OwnedTerminatorKind::Goto(BlockId(4)), span);
    }
    function.blocks[3].terminator = f::end(OwnedTerminatorKind::Goto(BlockId(4)), span);
    function.blocks.push(block(
        vec![
            f::instruction(
                OwnedInstruction::ReadIndex {
                    destination: LocalId(8),
                    base: AccessBase::Owner(OwnerPlaceId(3)),
                    index: f::operand(2, span),
                },
                span,
            ),
            f::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(3)), span),
        ],
        OwnedTerminatorKind::ReturnScalar(f::operand(8, span)),
        span,
    ));
    raw
}

#[test]
fn owned_enum_raw_only_continuing_arms_contribute_to_join_availability() {
    let (sources, s) = f::context();
    assert_eq!(
        reject(
            join_case(s(0), false, false),
            &sources,
            "partially consumed outer owner"
        )
        .kind,
        OwnedFailureKind::Ownership(Violation::Unavailable),
    );
    for (nullary, expected) in [(false, 42), (true, 19)] {
        let witness = verified::verify_owned(join_case(s(0), true, nullary), &sources).unwrap();
        assert_eq!(
            execute::run(&witness, Some(hir::DefId(0))),
            Ok(Scalar::I32(expected))
        );
    }
}
