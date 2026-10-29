//! Complete raw proof before checking the emitted preflight/commit boundary.
use super::*;
use crate::frontend::oir::owned::owned_vector_descriptors::{self, OwnedVectorImportClaims};
use crate::frontend::oir::owned::verified::VerifiedOwnedVectorProgram;
use crate::frontend::project::budget::Allocator;

fn record(id: usize, values: &[AggregateTy], span: Span) -> RawRecordDecl {
    RawRecordDecl {
        id: RecordId(id),
        is_copy: false,
        span,
        fields: values
            .iter()
            .enumerate()
            .map(|(index, aggregate)| RawFieldDecl {
                id: FieldId {
                    record: RecordId(id),
                    index,
                },
                ty: ParameterTy::Value(ValueTy::Owned(*aggregate)),
                span,
            })
            .collect(),
    }
}

fn fixture(destructure: bool, nesting: usize) -> (SourceMap, VerifiedOwnedVectorProgram, usize) {
    let (sources, mut raw, _, _) = owned_vector_descriptors::tests::fixture(None);
    let span = raw.records[0].span;
    let vector = AggregateTy::OwnedRecordVector(RecordId(0));
    raw.builtins =
        BuiltinOrigins::None.admit(crate::frontend::builtin_catalog::BuiltinItem::Vector(
            crate::frontend::builtin_catalog::vector::Item::Vec,
        ));
    raw.vector_functions.clear();
    raw.enums = vec![RawEnumDecl {
        id: EnumId(0),
        span,
        variants: vec![
            RawVariantDecl {
                id: VariantId {
                    enumeration: EnumId(0),
                    index: 0,
                },
                payload: Some(ParameterTy::Value(ValueTy::Owned(vector))),
                span,
            },
            RawVariantDecl {
                id: VariantId {
                    enumeration: EnumId(0),
                    index: 1,
                },
                payload: None,
                span,
            },
        ],
    }];
    let child = match nesting {
        0 => vector,
        1 => AggregateTy::Enum(EnumId(0)),
        _ => {
            raw.records
                .push(record(2, &[AggregateTy::Enum(EnumId(0))], span));
            AggregateTy::Record(RecordId(2))
        }
    };
    let subject = if destructure {
        let id = raw.records.len();
        raw.records.push(record(id, &[vector, child], span));
        AggregateTy::Record(RecordId(id))
    } else {
        child
    };
    let mut function = consumer_fixtures::function(0, ValueTy::Scalar(hir::Ty::I32), span);
    function
        .locals
        .push(consumer_fixtures::scalar(hir::Ty::I32, span));
    let owner = |aggregate, kind| OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
        kind,
        span,
    };
    let statement = |kind| consumer_fixtures::instruction(kind, span);
    let mut statements = vec![consumer_fixtures::assign(0, Rvalue::I32(0), span)];
    let index;
    if destructure {
        function
            .parameters
            .push(ParameterBinding::Owned(OwnerPlaceId(0)));
        function.owners = vec![
            owner(subject, OwnerKind::Parameter { position: 0 }),
            owner(vector, OwnerKind::Local { mutable: false }),
            owner(child, OwnerKind::Local { mutable: false }),
        ];
        let AggregateTy::Record(id) = subject else {
            unreachable!()
        };
        statements.push(statement(OwnedInstruction::StorageLive(OwnerPlaceId(1))));
        statements.push(statement(OwnedInstruction::StorageLive(OwnerPlaceId(2))));
        index = statements.len();
        statements.push(statement(OwnedInstruction::DestructureRecord {
            source: OwnerPlaceId(0),
            fields: vec![
                (
                    FieldId {
                        record: id,
                        index: 0,
                    },
                    OwnerPlaceId(1),
                ),
                (
                    FieldId {
                        record: id,
                        index: 1,
                    },
                    OwnerPlaceId(2),
                ),
            ],
        }));
    } else {
        function.parameters = vec![
            ParameterBinding::Owned(OwnerPlaceId(0)),
            ParameterBinding::Owned(OwnerPlaceId(1)),
        ];
        function.owners = vec![
            owner(subject, OwnerKind::Parameter { position: 0 }),
            owner(subject, OwnerKind::Parameter { position: 1 }),
            owner(subject, OwnerKind::Local { mutable: true }),
        ];
        statements.push(statement(OwnedInstruction::StorageLive(OwnerPlaceId(2))));
        statements.push(statement(OwnedInstruction::MoveInitialize {
            destination: OwnerPlaceId(2),
            source: OwnerPlaceId(0),
        }));
        index = statements.len();
        statements.push(statement(OwnedInstruction::Replace {
            destination: OwnerPlaceId(2),
            source: OwnerPlaceId(1),
        }));
    }
    function.blocks.push(OwnedBlock {
        merge: None,
        span,
        statements,
        terminator: consumer_fixtures::end(
            OwnedTerminatorKind::ReturnScalar(consumer_fixtures::operand(0, span)),
            span,
        ),
    });
    let mut main = consumer_fixtures::function(1, ValueTy::Scalar(hir::Ty::I32), span);
    main.locals
        .push(consumer_fixtures::scalar(hir::Ty::I32, span));
    main.blocks.push(OwnedBlock {
        merge: None,
        span,
        statements: vec![consumer_fixtures::assign(0, Rvalue::I32(0), span)],
        terminator: consumer_fixtures::end(
            OwnedTerminatorKind::ReturnScalar(consumer_fixtures::operand(0, span)),
            span,
        ),
    });
    raw.functions = vec![function, main];
    let wrapper = owned_vector_descriptors::tests::prepare(
        &sources,
        raw,
        &[],
        OwnedVectorImportClaims::empty(),
    );
    let witness = VerifiedOwnedVectorProgram::verify(
        wrapper,
        &sources,
        budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    (sources, witness, index)
}

fn emitted(destructure: bool, nesting: usize) -> String {
    let (sources, witness, index) = fixture(destructure, nesting);
    let plan = ExecutionPlan::build(&witness).unwrap();
    let storage = NativeStoragePlan::checked(&plan, true).unwrap();
    let diagnostics =
        Diagnostics::new(&plan, hir::DefId(1), &sources, true, MAX_DIAGNOSTIC_BYTES).unwrap();
    let mut output = Emission {
        text: Some(String::new()),
        ..Emission::count(MAX_IR_BYTES)
    };
    emit_statement(
        &storage.function(hir::DefId(0)),
        "commit",
        &witness.functions()[0].blocks[0].statements[index],
        &diagnostics,
        &mut output,
    );
    assert!(!output.exceeded);
    output.text.unwrap()
}

fn paid(line: &str) -> bool {
    line.contains("call i32 @oxid_owned_vector_validate(")
        || line.contains("call i32 @__oxid_composed_validate_")
}

#[test]
fn owned_vector_native_destructure_completes_all_paid_validation_before_any_move() {
    for nesting in 0..3 {
        let text = emitted(true, nesting);
        let first_move = text.find("zeroinitializer, ptr").unwrap();
        assert_eq!(
            text[..first_move].lines().filter(|line| paid(line)).count(),
            2
        );
        assert!(
            !text[first_move..].lines().any(paid),
            "paid validation after first move, nesting{nesting}:\n{text}"
        );
    }
}

#[test]
fn owned_vector_native_replace_never_rechecks_fuel_after_old_owner_release() {
    for nesting in 0..3 {
        let text = emitted(false, nesting);
        let old_release = text
            .find("call i32 @oxid_owned_vector_release(")
            .or_else(|| text.find("call void @__oxid_composed_drop_"))
            .unwrap();
        assert!(text[..old_release].lines().any(paid));
        assert!(
            !text[old_release..].lines().any(paid),
            "paid validation after old release, nesting{nesting}:\n{text}"
        );
    }
}
