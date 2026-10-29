use super::*;
use crate::frontend::oir::owned::{
    plan::{self, ExecutionPlan},
    verified::{PlanWitness, ProjectionAccess, ProjectionRequest, VerifiedOwnedVectorProgram},
};
use std::mem::size_of;

fn owned() -> (SourceMap, VerifiedOwnedVectorProgram, [Function; 8]) {
    let (sources, raw, keys, imports) = fixture(None);
    let wrapper = prepare(&sources, raw, &keys, imports);
    let witness = VerifiedOwnedVectorProgram::verify(
        wrapper,
        &sources,
        budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    (sources, witness, keys)
}

fn moved(owner: VerifiedOwnedVectorProgram) -> VerifiedOwnedVectorProgram {
    owner
}

#[test]
fn owned_vector_witness_shared_active_resources_scalar_siblings_and_exact_projection() {
    let (sources, mut raw, _, _) = fixture(None);
    raw.functions.clear();
    raw.enums.clear();
    raw.vector_functions.clear();
    raw.builtins = BuiltinOrigins::None.admit(BuiltinItem::Vector(vector::Item::Vec));
    let span = raw.records[0].span;
    let vector = AggregateTy::OwnedRecordVector(RecordId(0));
    let record = RecordId(raw.records.len());
    raw.records.push(RawRecordDecl {
        id: record,
        is_copy: false,
        span,
        fields: [ValueTy::Owned(vector), ValueTy::Scalar(hir::Ty::I32)]
            .into_iter()
            .enumerate()
            .map(|(index, ty)| RawFieldDecl {
                id: FieldId { record, index },
                ty: ParameterTy::Value(ty),
                span,
            })
            .collect(),
    });
    let enumeration = EnumId(raw.enums.len());
    raw.enums.push(RawEnumDecl {
        id: enumeration,
        span,
        variants: [
            Some(ValueTy::Owned(vector)),
            Some(ValueTy::Owned(AggregateTy::Record(record))),
            None,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, payload)| RawVariantDecl {
            id: VariantId { enumeration, index },
            payload: payload.map(ParameterTy::Value),
            span,
        })
        .collect(),
    });
    let field = FieldId { record, index: 1 };
    let base = AccessBase::Parameter(ReferenceParamId(0));
    let mut user = consumer_fixtures::function(0, ValueTy::Scalar(hir::Ty::I32), span);
    user.locals
        .push(consumer_fixtures::scalar(hir::Ty::I32, span));
    user.parameters
        .push(ParameterBinding::Reference(ReferenceParamId(0)));
    user.references.push(ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(record))).unwrap(),
        kind: BorrowKind::Shared,
        position: 0,
        span,
    });
    user.blocks.push(OwnedBlock {
        span,
        merge: None,
        statements: vec![consumer_fixtures::instruction(
            OwnedInstruction::ReadField {
                destination: LocalId(0),
                base,
                field,
            },
            span,
        )],
        terminator: consumer_fixtures::end(
            OwnedTerminatorKind::ReturnScalar(consumer_fixtures::operand(0, span)),
            span,
        ),
    });
    insert_user(&mut raw, user);
    let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
    let witness = VerifiedOwnedVectorProgram::verify(
        wrapper,
        &sources,
        budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    let permission = witness
        .projection_permission(
            hir::DefId(0),
            BlockId(0),
            0,
            ProjectionRequest::Field {
                base,
                field,
                write: false,
            },
        )
        .unwrap();
    assert!(permission.permits(
        &witness,
        hir::DefId(0),
        BlockId(0),
        0,
        base,
        ProjectionAccess::Read
    ));
    let (_, other, _) = owned();
    assert!(!permission.permits(
        &other,
        hir::DefId(0),
        BlockId(0),
        0,
        base,
        ProjectionAccess::Read
    ));
    assert!(!permission.permits(
        &witness,
        hir::DefId(0),
        BlockId(0),
        1,
        base,
        ProjectionAccess::Read
    ));
    assert!(witness
        .projection_permission(
            hir::DefId(0),
            BlockId(0),
            0,
            ProjectionRequest::Field {
                base,
                field,
                write: true
            }
        )
        .is_none());
    let scalars = witness
        .declarations()
        .leaves(AggregateTy::Record(record))
        .unwrap()
        .map(|leaf| (leaf.offset, leaf.ty))
        .collect::<Vec<_>>();
    assert_eq!(scalars, [(16, hir::Ty::I32)]);
    let direct =
        PlanWitness::resource_leaves(&witness, vector, |_| Err(DeclarationError::TypeMismatch))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
    assert_eq!(direct.len(), 1);
    assert_eq!(
        (direct[0].offset, direct[0].kind),
        (0, ResourceKind::OwnedRecordVector(0))
    );
    assert!(witness
        .declarations()
        .resource_leaves(vector, |_| Ok(0))
        .is_err());
    for tag in 0..3 {
        let resources =
            PlanWitness::resource_leaves(&witness, AggregateTy::Enum(enumeration), |_| Ok(tag))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
        if tag == 2 {
            assert!(resources.is_empty());
        } else {
            assert_eq!(resources.len(), 1);
            assert_eq!(
                (resources[0].offset, resources[0].kind),
                (8, ResourceKind::OwnedRecordVector(0))
            );
        }
    }
    assert!(
        PlanWitness::resource_leaves(&witness, AggregateTy::Enum(enumeration), |_| Ok(3))
            .unwrap()
            .next()
            .unwrap()
            .is_err()
    );
    assert!(PlanWitness::contains_resources(&witness, vector).unwrap());
    assert_eq!(size_of::<ResourceKind>(), 8);
    assert_eq!(size_of::<ResourceLeaf>(), 16);
    assert_eq!(
        size_of::<
            crate::frontend::oir::owned::verified::ProjectionPermission<
                '_,
                VerifiedOwnedVectorProgram,
            >,
        >(),
        size_of::<crate::frontend::oir::owned::verified::ProjectionPermission<'_>>()
    );
}

#[test]
fn owned_vector_witness_owns_finite_bounds_after_moves_and_refuses_foreign_facades() {
    let (_, witness, keys) = owned();
    let (_, other, _) = owned();
    let witness = moved(moved(witness));
    let view = witness.elements();
    assert!(view.belongs_to(witness.declarations()));
    assert!(!view.belongs_to(other.declarations()));
    let element = view.element(RecordId(0)).unwrap();
    assert_eq!(element.size(), 4);
    assert_eq!(element.record(), RecordId(0));
    assert!(std::ptr::eq(element.declarations(), witness.declarations()));
    assert!(view.element(RecordId(1)).is_err()); // Copy nominal
    assert!(view.element(RecordId(usize::MAX)).is_err());
    assert_eq!(witness.fixed_builtin_base(), Some(0));
    let old = vector::Function::new(vector::Operation::New, hir::Ty::I32).unwrap();
    assert_eq!(witness.vector_operation(hir::DefId(0)), Some(old));
    assert_eq!(witness.owned_vector_operation(hir::DefId(0)), None);
    for (rank, key) in keys.into_iter().enumerate() {
        assert_eq!(
            witness.owned_vector_operation(hir::DefId(rank + 1)),
            Some(key)
        );
        assert_eq!(witness.vector_operation(hir::DefId(rank + 1)), None);
    }
    assert_eq!(witness.owned_vector_operation(hir::DefId(usize::MAX)), None);
    assert!(witness.uses_owned_vectors());
}

#[test]
fn owned_vector_witness_exact_and_one_less_logical_metadata_and_work() {
    let (_, baseline, _) = owned();
    let usage = baseline.usage();
    for mutation in 0..3 {
        let (sources, raw, keys, imports) = fixture(None);
        let wrapper = prepare(&sources, raw, &keys, imports);
        let mut limits = budget::Limits {
            metadata: usage.metadata_bytes,
            work: usage.work,
            ..budget::Limits::DEFAULT
        };
        if mutation == 1 {
            limits.metadata -= 1;
        }
        if mutation == 2 {
            limits.work -= 1;
        }
        let result = VerifiedOwnedVectorProgram::verify(
            wrapper,
            &sources,
            limits,
            &mut Allocator::default(),
        );
        if mutation == 0 {
            let exact = result.expect("exact full owning proof logical scope");
            assert_eq!(exact.usage().metadata_bytes, usage.metadata_bytes);
            assert_eq!(exact.usage().work, usage.work);
        } else {
            assert!(matches!(
                result,
                Err(OwnedFailure {
                    kind: OwnedFailureKind::Resource(_),
                    ..
                })
            ));
        }
    }
}

#[test]
fn owned_vector_witness_both_real_bound_allocations_fail_without_a_seal() {
    for ordinal in 1..=2 {
        let (sources, raw, keys, imports) = fixture(None);
        let wrapper = prepare(&sources, raw, &keys, imports);
        let mut allocator = Allocator {
            fail_at: Some(ordinal),
            ..Allocator::default()
        };
        let result = VerifiedOwnedVectorProgram::verify(
            wrapper,
            &sources,
            budget::Limits::DEFAULT,
            &mut allocator,
        );
        assert!(matches!(
            result,
            Err(OwnedFailure {
                kind: OwnedFailureKind::Resource(_),
                ..
            })
        ));
        assert_eq!(allocator.attempts, ordinal);
        assert!(!allocator.trace.last().unwrap().success);
    }
}

#[test]
fn owned_vector_witness_cannot_skip_unreachable_shape_cfg_or_actual_move_flow() {
    for mutation in 0..3 {
        let (sources, mut raw, keys, imports) = fixture(None);
        let span = raw.records[0].span;
        let mut user = identity(span);
        match mutation {
            0 => user.blocks.push(OwnedBlock {
                merge: None,
                span,
                statements: vec![consumer_fixtures::assign(usize::MAX, Rvalue::I32(1), span)],
                terminator: consumer_fixtures::end(
                    OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
                    span,
                ),
            }),
            1 => {
                user.locals
                    .push(consumer_fixtures::scalar(hir::Ty::I32, span));
                user.blocks[0].statements.push(consumer_fixtures::assign(
                    0,
                    Rvalue::Copy(consumer_fixtures::operand(0, span)),
                    span,
                ));
            }
            _ => user.blocks[0]
                .statements
                .push(consumer_fixtures::instruction(
                    OwnedInstruction::Discard(OwnerPlaceId(0)),
                    span,
                )),
        }
        insert_user(&mut raw, user);
        let wrapper = prepare(&sources, raw, &keys, imports);
        let result = VerifiedOwnedVectorProgram::verify(
            wrapper,
            &sources,
            budget::Limits::DEFAULT,
            &mut Allocator::default(),
        );
        match mutation {
            0 => assert!(matches!(
                result,
                Err(OwnedFailure {
                    kind: OwnedFailureKind::Malformed(Malformed::Scalar(_)),
                    ..
                })
            )),
            1 => assert!(matches!(
                result,
                Err(OwnedFailure {
                    kind: OwnedFailureKind::Malformed(Malformed::Scalar(
                        FailureKind::Uninitialized
                    )),
                    ..
                })
            )),
            _ => assert!(matches!(
                result,
                Err(OwnedFailure {
                    kind: OwnedFailureKind::Ownership(Violation::Unavailable),
                    ..
                })
            )),
        }
    }
}

#[test]
fn owned_vector_witness_zero_keys_remains_extended_and_true_legacy_keeps_old_path() {
    let (sources, mut raw, _, _) = fixture(None);
    let span = raw.records[0].span;
    raw.builtins = BuiltinOrigins::None.admit(BuiltinItem::Vector(vector::Item::Vec));
    raw.enums.clear();
    raw.functions = vec![identity(span)];
    raw.vector_functions.clear();
    let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
    let witness = VerifiedOwnedVectorProgram::verify(
        wrapper,
        &sources,
        budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    assert!(witness.uses_owned_vectors());
    assert_eq!(witness.owned_vector_operation(hir::DefId(0)), None);
    let plan = ExecutionPlan::build(&witness).unwrap();
    assert!(plan.uses_owned_vectors());
    assert!(plan.uses_dynamic_resources());
    assert_eq!(plan.function(hir::DefId(0)).usage().owner_cells, 3);
    assert_eq!(plan.function(hir::DefId(0)).usage().payload_bytes, 16);
    assert_eq!(plan.owner_width(hir::DefId(0), OwnerPlaceId(0)), 3);
    assert!(plan::native_storage::NativeStoragePlan::checked(&plan, true).is_ok());
    let (sources, raw, _) = consumer_fixtures::shared_read();
    let legacy = verified::verify_owned(raw, &sources).unwrap();
    let legacy_plan = ExecutionPlan::build(&legacy).unwrap();
    assert!(!legacy_plan.uses_owned_vectors());
    assert_eq!(ExecutionPlan::compiler_work_request(&legacy).unwrap(), 0);
    assert_eq!(size_of::<ExecutionPlan<'_>>(), 40);
    assert_eq!(
        size_of::<ExecutionPlan<'_, VerifiedOwnedVectorProgram>>(),
        40
    );
    assert_eq!(size_of::<RawOwnedProgram>(), 112);
    assert!(execute::run(&legacy, Some(hir::DefId(0))).is_ok());
    let (sources, raw, _) = consumer_fixtures::shared_read();
    let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
    let mut allocator = Allocator::default();
    assert!(VerifiedOwnedVectorProgram::verify(
        wrapper,
        &sources,
        budget::Limits::DEFAULT,
        &mut allocator,
    )
    .is_err());
    assert_eq!(allocator.attempts, 0);
}

#[test]
fn owned_vector_witness_plan_keeps_descriptor_bearing_record_and_enum_owners() {
    let (sources, mut raw, _, _) = fixture(None);
    let span = raw.records[0].span;
    let vector = AggregateTy::OwnedRecordVector(RecordId(0));
    let container = RecordId(raw.records.len());
    raw.records.push(RawRecordDecl {
        id: container,
        is_copy: false,
        span,
        fields: vec![RawFieldDecl {
            id: FieldId {
                record: container,
                index: 0,
            },
            ty: ParameterTy::Value(ValueTy::Owned(vector)),
            span,
        }],
    });
    raw.enums = vec![RawEnumDecl {
        id: EnumId(0),
        span,
        variants: vec![RawVariantDecl {
            id: VariantId {
                enumeration: EnumId(0),
                index: 0,
            },
            payload: Some(ParameterTy::Value(ValueTy::Owned(vector))),
            span,
        }],
    }];
    raw.builtins = BuiltinOrigins::None.admit(BuiltinItem::Vector(vector::Item::Vec));
    raw.vector_functions.clear();
    raw.functions.clear();
    for aggregate in [AggregateTy::Record(container), AggregateTy::Enum(EnumId(0))] {
        let mut user = identity(span);
        user.id = hir::DefId(raw.functions.len());
        user.result = ValueTy::Owned(aggregate);
        user.owners[0].aggregate = AggregateSlot::try_from_aggregate(aggregate).unwrap();
        raw.functions.push(user);
    }
    let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
    let witness = VerifiedOwnedVectorProgram::verify(
        wrapper,
        &sources,
        budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    assert!(witness.elements().element(RecordId(0)).is_ok());
    assert!(witness.elements().element(container).is_err());
    let plan = ExecutionPlan::build(&witness).unwrap();
    assert_eq!(plan.function(hir::DefId(0)).usage().payload_bytes, 16);
    // Enum tag4 followed by payload aligned to8, with a16-byte descriptor.
    assert_eq!(plan.function(hir::DefId(1)).usage().payload_bytes, 24);
    assert!(plan::native_storage::NativeStoragePlan::checked(&plan, true).is_ok());
    assert!(plan::native_storage::NativeInventories::checked(&plan).is_ok());
}

#[test]
fn owned_vector_witness_shared_plan_exact_one_less_preflight_and_every_allocation() {
    let (_, witness, _) = owned();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let work = ExecutionPlan::compiler_work_request(&witness).unwrap();
    let metadata = plan.metadata_bytes();
    assert!(metadata > witness.usage().metadata_bytes);
    assert!(plan::native_storage::NativeStoragePlan::checked(&plan, true).is_ok());
    assert!(plan::native_storage::NativeInventories::checked(&plan).is_ok());
    assert!(ExecutionPlan::build_with_test_budgets(&witness, metadata, work).is_ok());
    for (bytes, visits, name) in [
        (metadata - 1, work, "owned plan bytes"),
        (metadata, work - 1, "owned vector plan work"),
    ] {
        let failure = plan::fail_allocation_after(0, || {
            ExecutionPlan::build_with_test_budgets(&witness, bytes, visits)
        })
        .unwrap_err();
        assert_eq!(failure.name, name);
    }
    // Each real table reserve is reached. Every failed partial table is
    // ordinary metadata; the already sealed owner remains usable unchanged.
    let attempts = 1 + witness.functions().len() * 4;
    for failure in 0..attempts {
        let error =
            plan::fail_allocation_after(failure, || ExecutionPlan::build(&witness)).unwrap_err();
        assert_eq!(error.name, "injected owned allocation failure");
        assert!(witness.elements().element(RecordId(0)).is_ok());
    }
    assert!(plan::fail_allocation_after(attempts, || ExecutionPlan::build(&witness)).is_ok());
    for function in witness.functions() {
        for statement in function.blocks.iter().flat_map(|block| &block.statements) {
            if let OwnedInstruction::OwnedVector { operation, .. } = statement.kind {
                assert_eq!(plan.statement_cost(function.id, &statement.kind), 4);
                let terminator = function.blocks[0].terminator.as_ref().unwrap();
                assert_eq!(
                    plan.owned_vector_delivery_cost(function.id, operation),
                    Some(4 + plan.terminator_cost(function.id, &terminator.kind))
                );
                assert_eq!(
                    plan.owned_vector_delivery_cost(hir::DefId(0), operation),
                    None
                );
                assert!(plan::native_vector_layout::indexed(
                    witness.declarations(),
                    function,
                    &statement.kind
                )
                .is_err());
            }
        }
    }
}
