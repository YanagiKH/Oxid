use super::composition_reference_tests::batch;
use super::consumer_fixtures as raw;
use super::*;

fn error(raw: RawOwnedProgram, sources: &SourceMap) -> OwnedFailure {
    verify_owned(raw, sources).unwrap_err()
}
fn first_projection(raw: &mut RawOwnedProgram) -> &mut OwnedStatement {
    raw.functions[0].blocks[2]
        .statements
        .iter_mut()
        .find(|s| matches!(s.kind, OwnedInstruction::ReadProjection { .. }))
        .unwrap()
}

#[test]
fn raw_projection_rejects_wrong_nominal_hops_unknown_fields_and_non_scalar_endpoints() {
    for path in [
        vec![],
        vec![FieldId {
            record: RecordId(0),
            index: 0,
        }],
        vec![
            FieldId {
                record: RecordId(1),
                index: 0,
            },
            FieldId {
                record: RecordId(1),
                index: 0,
            },
        ],
        vec![FieldId {
            record: RecordId(1),
            index: 999,
        }],
        vec![FieldId {
            record: RecordId(1),
            index: 0,
        }],
        vec![FieldId {
            record: RecordId(1),
            index: 1,
        }],
        vec![
            FieldId {
                record: RecordId(1),
                index: 0
            };
            65
        ],
    ] {
        let (sources, mut program) = batch();
        let OwnedInstruction::ReadProjection { path: actual, .. } =
            &mut first_projection(&mut program).kind
        else {
            unreachable!()
        };
        *actual = path;
        assert!(matches!(
            error(program, &sources).kind,
            OwnedFailureKind::Malformed(_) | OwnedFailureKind::Resource(_)
        ));
    }
}

#[test]
fn raw_projection_rejects_index_on_scalar_and_length_of_record() {
    for length in [false, true] {
        let (sources, mut program) = batch();
        let instruction = first_projection(&mut program);
        if length {
            instruction.kind = OwnedInstruction::ProjectionLength {
                destination: LocalId(5),
                base: AccessBase::Owner(OwnerPlaceId(7)),
                path: vec![FieldId {
                    record: RecordId(1),
                    index: 0,
                }],
            };
        } else if let OwnedInstruction::ReadProjection { index, .. } = &mut instruction.kind {
            *index = Some(raw::operand(0, instruction.span));
        }
        assert_eq!(
            error(program, &sources).kind,
            OwnedFailureKind::Malformed(Malformed::Type)
        );
    }
}

#[test]
fn scalar_legacy_instruction_cannot_extract_an_aggregate_field() {
    let (sources, mut program) = batch();
    first_projection(&mut program).kind = OwnedInstruction::ReadField {
        destination: LocalId(5),
        base: AccessBase::Owner(OwnerPlaceId(7)),
        field: FieldId {
            record: RecordId(1),
            index: 0,
        },
    };
    assert_eq!(
        error(program, &sources).kind,
        OwnedFailureKind::Malformed(Malformed::Type)
    );
}

#[test]
fn raw_constructor_rejects_duplicate_owned_consumption_and_unstaged_owner() {
    for duplicate in [false, true] {
        let (sources, mut program) = batch();
        if duplicate {
            program.records[1].fields[1].ty =
                ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(0))));
        }
        let i = program.functions[0].blocks[0]
            .statements
            .iter_mut()
            .find(|i| matches!(i.kind, OwnedInstruction::ConstructComposite { .. }))
            .unwrap();
        let OwnedInstruction::ConstructComposite { fields, .. } = &mut i.kind else {
            unreachable!()
        };
        if duplicate {
            fields[1].1 = FieldInitializer::Owned(OwnerPlaceId(1));
        } else {
            fields[0].1 = FieldInitializer::Owned(OwnerPlaceId(0));
        }
        assert_eq!(
            error(program, &sources).kind,
            OwnedFailureKind::Malformed(if duplicate {
                Malformed::Binding
            } else {
                Malformed::OwnerClass
            })
        );
    }
}

#[test]
fn raw_constructor_checks_all_fields_and_types_before_ownership() {
    for mutation in 0..4 {
        let (sources, mut program) = batch();
        let i = program.functions[0].blocks[0]
            .statements
            .iter_mut()
            .find(|i| matches!(i.kind, OwnedInstruction::ConstructComposite { .. }))
            .unwrap();
        let OwnedInstruction::ConstructComposite { fields, .. } = &mut i.kind else {
            unreachable!()
        };
        match mutation {
            0 => {
                fields.pop();
            }
            1 => {
                fields[1].0 = fields[0].0;
            }
            2 => {
                fields[0].1 = FieldInitializer::Scalar(raw::operand(0, i.span));
            }
            3 => {
                fields[0].1 = FieldInitializer::Owned(OwnerPlaceId(usize::MAX));
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            error(program, &sources).kind,
            OwnedFailureKind::Malformed(_)
        ));
    }
}

#[test]
fn earlier_child_staging_move_is_visible_to_later_initializer_work() {
    let (sources, mut program) = batch();
    let f = &mut program.functions[0];
    let s = f.span;
    let local = f.locals.len();
    f.locals.push(raw::scalar(hir::Ty::I32, s));
    let after_move = f.blocks[0]
        .statements
        .iter()
        .position(|i| {
            matches!(
                i.kind,
                OwnedInstruction::MoveInitialize {
                    source: OwnerPlaceId(0),
                    ..
                }
            )
        })
        .unwrap()
        + 1;
    f.blocks[0].statements.insert(
        after_move,
        raw::instruction(
            OwnedInstruction::ReadField {
                destination: LocalId(local),
                base: AccessBase::Owner(OwnerPlaceId(0)),
                field: FieldId {
                    record: RecordId(0),
                    index: 0,
                },
            },
            s,
        ),
    );
    assert_eq!(
        error(program, &sources).kind,
        OwnedFailureKind::Ownership(Violation::Unavailable)
    );
}

#[test]
fn nested_reads_and_writes_still_require_live_whole_root_and_exclusive_permission() {
    let (sources, mut program) = batch();
    let span = first_projection(&mut program).span;
    program.functions[0].blocks[2].statements.insert(
        0,
        raw::instruction(OwnedInstruction::Discard(OwnerPlaceId(7)), span),
    );
    assert_eq!(
        error(program, &sources).kind,
        OwnedFailureKind::Ownership(Violation::Unavailable)
    );

    let (sources, mut program) = batch();
    program.functions[0].loans[0].kind = BorrowKind::Shared;
    program.functions[2].references[0].kind = BorrowKind::Shared;
    assert_eq!(
        error(program, &sources).kind,
        OwnedFailureKind::Ownership(Violation::Permission)
    );
}

#[test]
fn raw_composed_record_stored_references_remain_rejected() {
    let (sources, mut program) = batch();
    let field = &mut program.records[1].fields[0];
    let id = field.id;
    field.ty = ParameterTy::Reference {
        referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(0))),
        kind: BorrowKind::Shared,
    };
    assert_eq!(
        error(program, &sources).kind,
        OwnedFailureKind::Malformed(Malformed::Declaration(DeclarationError::NonScalarField(id)))
    );
}
