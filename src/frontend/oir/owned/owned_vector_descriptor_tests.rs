//! Contract-shaped raw fixtures; no production lowerer or seal is used.
use super::*;
use crate::frontend::builtin_catalog::BuiltinItem;

#[path = "owned_vector_context_tests.rs"]
mod context_tests;

#[path = "owned_vector_query_tests.rs"]
mod query_tests;

#[path = "owned_vector_signature_tests.rs"]
mod signature_tests;

#[path = "owned_vector_proof_tests.rs"]
mod proof_tests;

#[path = "owned_vector_declaration_tests.rs"]
mod declaration_tests;

#[path = "owned_vector_execution_fixture.rs"]
pub(in crate::frontend::oir::owned) mod execution_fixture;

fn signature(operation: Operation) -> (Vec<ParameterTy>, ValueTy) {
    let vector = AggregateTy::OwnedRecordVector(RecordId(0));
    let shared = ParameterTy::Reference {
        referent: BorrowedTy::Exact(vector),
        kind: BorrowKind::Shared,
    };
    let exclusive = ParameterTy::Reference {
        referent: BorrowedTy::Exact(vector),
        kind: BorrowKind::Exclusive,
    };
    let count = ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32));
    match operation {
        Operation::New => (vec![], ValueTy::Owned(vector)),
        Operation::Len | Operation::Capacity => (vec![shared], ValueTy::Scalar(hir::Ty::I32)),
        Operation::TryReserve => (
            vec![exclusive, count],
            ValueTy::Owned(AggregateTy::Enum(EnumId(0))),
        ),
        Operation::PushReserved => (
            vec![
                exclusive,
                ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(0)))),
            ],
            ValueTy::Scalar(hir::Ty::Unit),
        ),
        Operation::PopLast => (
            vec![exclusive],
            ValueTy::Owned(AggregateTy::Record(RecordId(0))),
        ),
        Operation::Truncate => (vec![exclusive, count], ValueTy::Scalar(hir::Ty::Unit)),
        Operation::Take => (vec![exclusive], ValueTy::Owned(vector)),
    }
}

pub(in crate::frontend::oir::owned) fn function(
    id: usize,
    parameters: Vec<ParameterTy>,
    result: ValueTy,
    span: Span,
    opcode: impl FnOnce(CallResult) -> OwnedInstruction,
) -> RawOwnedFunction {
    let mut raw = consumer_fixtures::function(id, result, span);
    for (position, parameter) in parameters.into_iter().enumerate() {
        let binding = match parameter {
            ParameterTy::Value(ValueTy::Scalar(ty)) => {
                let id = LocalId(raw.locals.len());
                raw.locals.push(LocalDecl {
                    ty,
                    kind: LocalKind::Parameter,
                    span,
                });
                ParameterBinding::Scalar(id)
            }
            ParameterTy::Value(ValueTy::Owned(aggregate)) => {
                let id = OwnerPlaceId(raw.owners.len());
                raw.owners.push(OwnerDecl {
                    aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
                    kind: OwnerKind::Parameter { position },
                    span,
                });
                ParameterBinding::Owned(id)
            }
            ParameterTy::Reference { referent, kind } => {
                let id = ReferenceParamId(raw.references.len());
                raw.references.push(ReferenceDecl {
                    referent: BorrowedSlot::check(referent).unwrap(),
                    kind,
                    position,
                    span,
                });
                ParameterBinding::Reference(id)
            }
        };
        raw.parameters.push(binding);
    }
    let mut statements = Vec::new();
    let (destination, terminator) = match result {
        ValueTy::Scalar(ty) => {
            let id = LocalId(raw.locals.len());
            raw.locals.push(LocalDecl {
                ty,
                kind: LocalKind::Temporary,
                span,
            });
            (
                CallResult::Scalar(id),
                OwnedTerminatorKind::ReturnScalar(Operand { local: id, span }),
            )
        }
        ValueTy::Owned(aggregate) => {
            let id = OwnerPlaceId(raw.owners.len());
            raw.owners.push(OwnerDecl {
                aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
                kind: OwnerKind::Temporary,
                span,
            });
            statements.push(consumer_fixtures::instruction(
                OwnedInstruction::StorageLive(id),
                span,
            ));
            (CallResult::Owned(id), OwnedTerminatorKind::ReturnOwned(id))
        }
    };
    statements.push(consumer_fixtures::instruction(opcode(destination), span));
    raw.blocks.push(OwnedBlock {
        span,
        merge: None,
        statements,
        terminator: consumer_fixtures::end(terminator, span),
    });
    raw
}

pub(in crate::frontend::oir::owned) fn fixture(
    skip: Option<vector::Item>,
) -> (
    SourceMap,
    RawOwnedProgram,
    [Function; 8],
    OwnedVectorImportClaims,
) {
    let mut sources = SourceMap::new();
    let file = sources.add(
        "owned-vector-raw.ox".into(),
        "raw records and functions".into(),
    );
    let span = sources.get(file).span(0, 3);
    let mut origin = BuiltinOrigins::None;
    for item in [
        vector::Item::Vec,
        vector::Item::New,
        vector::Item::Len,
        vector::Item::Capacity,
        vector::Item::TryReserve,
        vector::Item::Truncate,
        vector::Item::Take,
    ] {
        if Some(item) != skip {
            origin = origin.admit(BuiltinItem::Vector(item));
        }
    }
    let records = [false, true]
        .into_iter()
        .enumerate()
        .map(|(index, is_copy)| RawRecordDecl {
            id: RecordId(index),
            is_copy,
            span,
            fields: vec![RawFieldDecl {
                id: FieldId {
                    record: RecordId(index),
                    index: 0,
                },
                ty: ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32)),
                span,
            }],
        })
        .collect();
    let grow = RawEnumDecl {
        id: EnumId(0),
        span,
        variants: (0..4)
            .map(|index| RawVariantDecl {
                id: VariantId {
                    enumeration: EnumId(0),
                    index,
                },
                payload: None,
                span,
            })
            .collect(),
    };
    let old = vector::Function::new(vector::Operation::New, hir::Ty::I32).unwrap();
    let keys = Operation::ALL.map(|operation| Function::record(operation, RecordId(0)).unwrap());
    let mut functions = vec![function(
        0,
        vec![],
        ValueTy::Owned(AggregateTy::Vector(hir::Ty::I32)),
        span,
        |destination| OwnedInstruction::Vector {
            operation: old,
            destination,
        },
    )];
    for (rank, key) in keys.into_iter().enumerate() {
        let (parameters, result) = signature(key.operation());
        functions.push(function(
            rank + 1,
            parameters,
            result,
            span,
            |destination| OwnedInstruction::OwnedVector {
                operation: key,
                destination,
            },
        ));
    }
    (
        sources,
        RawOwnedProgram {
            builtins: origin,
            vector_functions: vec![old],
            enums: vec![grow],
            records,
            functions,
        },
        keys,
        OwnedVectorImportClaims::empty()
            .admit(Item::PushReserved)
            .admit(Item::PopLast),
    )
}

pub(in crate::frontend::oir::owned) fn prepare(
    sources: &SourceMap,
    program: RawOwnedProgram,
    keys: &[Function],
    imports: OwnedVectorImportClaims,
) -> RawOwnedVectorProgram {
    RawOwnedVectorProgram::prepare(
        program,
        imports,
        keys,
        sources,
        budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap()
}

pub(in crate::frontend::oir::owned) fn verified_fixture() -> (
    SourceMap,
    verified::VerifiedOwnedVectorProgram,
    [Function; 8],
) {
    let (sources, raw, keys, imports) = fixture(None);
    let wrapper = prepare(&sources, raw, &keys, imports);
    let witness = verified::VerifiedOwnedVectorProgram::verify(
        wrapper,
        &sources,
        budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    (sources, witness, keys)
}

#[test]
fn owned_vector_raw_wrapper_proves_all_eight_contract_rows_on_the_checked_old_prefix() {
    let (sources, raw, keys, imports) = fixture(None);
    let old_ids = builtins::check_candidate_prefix(&raw, 1).unwrap();
    assert_eq!(old_ids.vector_function_at(0), Some(hir::DefId(0)));
    assert!(builtins::check_candidate(&raw).is_err());
    for key in keys {
        let (parameters, result) = signature(key.operation());
        let concrete = key.concrete_signature(EnumId(0));
        assert_eq!(concrete.parameters(), parameters.as_slice());
        assert_eq!(concrete.result, result);
    }
    let wrapper = prepare(&sources, raw, &keys, imports);
    let usage = wrapper.check(&sources, budget::Limits::DEFAULT).unwrap();
    assert_eq!(usage.owned_functions, 8);
    assert_eq!(
        usage.domains,
        FunctionDomains {
            fixed_start: 0,
            old_start: 0,
            owned_start: 1,
            end: 9
        }
    );
    assert_eq!(usage.domains.old_at(0), Some(hir::DefId(0)));
    assert_eq!(usage.domains.old_at(1), None);
    for rank in 0..8 {
        assert_eq!(usage.domains.owned_at(rank), Some(hir::DefId(rank + 1)));
    }
    assert_eq!(usage.domains.owned_at(8), None);
    assert_eq!(usage.domains.owned_at(usize::MAX), None);
    assert!(verify_owned(wrapper.program, &sources).is_err());
}

#[test]
fn owned_vector_raw_wrapper_metadata_work_and_real_claim_reservation_are_prepaid() {
    for denied in 0..2 {
        let (sources, raw, keys, imports) = fixture(None);
        let request =
            RawOwnedVectorProgram::requested(&raw, keys.len(), budget::Limits::DEFAULT).unwrap();
        let mut limits = budget::Limits::DEFAULT;
        if denied == 0 {
            limits.metadata = request.metadata_bytes - 1;
        } else {
            limits.work = request.work - 1;
        }
        let mut allocator = Allocator::default();
        let result =
            RawOwnedVectorProgram::prepare(raw, imports, &keys, &sources, limits, &mut allocator);
        assert!(matches!(
            result,
            Err(OwnedFailure {
                kind: OwnedFailureKind::Resource(
                    "owned vector candidate metadata" | "owned vector candidate work"
                ),
                ..
            })
        ));
        assert_eq!(allocator.attempts, 0);
    }
    let (sources, raw, keys, imports) = fixture(None);
    let mut allocator = Allocator {
        fail_at: Some(1),
        ..Default::default()
    };
    assert!(matches!(
        RawOwnedVectorProgram::prepare(
            raw,
            imports,
            &keys,
            &sources,
            budget::Limits::DEFAULT,
            &mut allocator
        ),
        Err(OwnedFailure {
            kind: OwnedFailureKind::Resource("owned vector raw key allocation"),
            ..
        })
    ));
    assert_eq!(allocator.attempts, 1);

    let (sources, raw, keys, imports) = fixture(None);
    let request =
        RawOwnedVectorProgram::requested(&raw, keys.len(), budget::Limits::DEFAULT).unwrap();
    let mut limits = budget::Limits::DEFAULT;
    limits.metadata = request.metadata_bytes;
    limits.work = request.work;
    let mut allocator = Allocator::default();
    let wrapper =
        RawOwnedVectorProgram::prepare(raw, imports, &keys, &sources, limits, &mut allocator)
            .unwrap();
    assert_eq!(allocator.attempts, 1);
    assert_eq!(wrapper.claims.functions.capacity(), 8);
    assert_eq!(wrapper.check(&sources, limits).unwrap(), request);
    limits.metadata -= 1;
    assert!(wrapper.check(&sources, limits).is_err());
}

#[test]
fn owned_vector_raw_wrapper_domains_and_claim_rosters_reject_underflow_and_duplicates() {
    let (_, mut raw, _, _) = fixture(None);
    assert!(FunctionDomains::check(&raw, 10).is_err());
    raw.vector_functions.resize(10, raw.vector_functions[0]);
    assert!(FunctionDomains::check(&raw, 0).is_err());
    raw.functions.clear();
    raw.vector_functions.clear();
    raw.builtins = raw.builtins.admit(BuiltinItem::Function(
        crate::frontend::builtin_catalog::BuiltinFunction::ReadStdin,
    ));
    assert!(FunctionDomains::check(&raw, 0).is_err());
    enum Mutation {
        Duplicate,
        Order,
        CopyRecord,
        UnknownRecord,
    }
    for mutation in [
        Mutation::Duplicate,
        Mutation::Order,
        Mutation::CopyRecord,
        Mutation::UnknownRecord,
    ] {
        let (sources, raw, mut keys, imports) = fixture(None);
        match mutation {
            Mutation::Duplicate => keys[1] = keys[0],
            Mutation::Order => keys.reverse(),
            Mutation::CopyRecord => {
                keys[0] = Function::record(Operation::New, RecordId(1)).unwrap()
            }
            Mutation::UnknownRecord => {
                keys[0] = Function::record(Operation::New, RecordId(4095)).unwrap()
            }
        }
        assert!(prepare(&sources, raw, &keys, imports)
            .check(&sources, budget::Limits::DEFAULT)
            .is_err());
    }
}

#[test]
fn owned_vector_raw_wrapper_requires_shared_extra_imports_and_canonical_status() {
    for item in [
        vector::Item::Len,
        vector::Item::Capacity,
        vector::Item::Truncate,
        vector::Item::Take,
    ] {
        let (sources, raw, keys, imports) = fixture(Some(item));
        assert!(prepare(&sources, raw, &keys, imports)
            .check(&sources, budget::Limits::DEFAULT)
            .is_err());
    }
    for imports in [
        OwnedVectorImportClaims::empty().admit(Item::PushReserved),
        OwnedVectorImportClaims::empty().admit(Item::PopLast),
    ] {
        let (sources, raw, keys, _) = fixture(None);
        assert!(prepare(&sources, raw, &keys, imports)
            .check(&sources, budget::Limits::DEFAULT)
            .is_err());
    }
    let (sources, mut raw, keys, imports) = fixture(None);
    raw.enums[0].variants.pop();
    assert!(prepare(&sources, raw, &keys, imports)
        .check(&sources, budget::Limits::DEFAULT)
        .is_err());
    let (sources, mut raw, keys, imports) = fixture(None);
    raw.enums.clear();
    assert!(prepare(&sources, raw, &keys, imports)
        .check(&sources, budget::Limits::DEFAULT)
        .is_err());
    let (sources, raw, keys, _) = fixture(None);
    let mut allocator = Allocator::default();
    assert!(RawOwnedVectorProgram::prepare(
        raw,
        OwnedVectorImportClaims(4),
        &keys,
        &sources,
        budget::Limits::DEFAULT,
        &mut allocator
    )
    .is_err());
    assert_eq!(allocator.attempts, 0);
}

#[test]
fn owned_vector_raw_wrapper_canonical_mutations_never_gain_instruction_authority() {
    enum Mutation {
        Id,
        Result,
        Mode,
        ParameterPosition,
        ParameterOwner,
        Arity,
        InputNominal,
        ReferenceNominal,
        Destination,
        Storage,
        Family,
        ExtraLocal,
        ExtraBlock,
        Terminator,
        Origin,
        Span,
    }
    for mutation in [
        Mutation::Id,
        Mutation::Result,
        Mutation::Mode,
        Mutation::ParameterPosition,
        Mutation::ParameterOwner,
        Mutation::Arity,
        Mutation::InputNominal,
        Mutation::ReferenceNominal,
        Mutation::Destination,
        Mutation::Storage,
        Mutation::Family,
        Mutation::ExtraLocal,
        Mutation::ExtraBlock,
        Mutation::Terminator,
        Mutation::Origin,
        Mutation::Span,
    ] {
        let (sources, mut raw, keys, imports) = fixture(None);
        let span = raw.functions[1].span;
        match mutation {
            Mutation::Id => raw.functions[1].id = hir::DefId(37),
            Mutation::Result => raw.functions[1].result = ValueTy::Scalar(hir::Ty::Unit),
            Mutation::Mode => raw.functions[5].references[0].kind = BorrowKind::Shared,
            Mutation::ParameterPosition => raw.functions[5].references[0].position = 1,
            Mutation::ParameterOwner => raw.functions[5].owners[0].kind = OwnerKind::Temporary,
            Mutation::Arity => {
                raw.functions[5].parameters.pop();
            }
            Mutation::InputNominal => {
                raw.functions[5].owners[0].aggregate =
                    AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(1))).unwrap()
            }
            Mutation::ReferenceNominal => {
                raw.functions[5].references[0].referent = BorrowedSlot::check(BorrowedTy::Exact(
                    AggregateTy::OwnedRecordVector(RecordId(1)),
                ))
                .unwrap()
            }
            Mutation::Destination => {
                raw.functions[1].blocks[0].statements[1].kind = OwnedInstruction::OwnedVector {
                    operation: keys[0],
                    destination: CallResult::Owned(OwnerPlaceId(37)),
                }
            }
            Mutation::Storage => {
                raw.functions[1].blocks[0].statements.remove(0);
            }
            Mutation::Family => {
                raw.functions[1].blocks[0].statements[1].kind = OwnedInstruction::Vector {
                    operation: raw.vector_functions[0],
                    destination: CallResult::Owned(OwnerPlaceId(0)),
                }
            }
            Mutation::ExtraLocal => raw.functions[1].locals.push(LocalDecl {
                ty: hir::Ty::I32,
                kind: LocalKind::Temporary,
                span,
            }),
            Mutation::ExtraBlock => {
                let block = raw.functions[1].blocks[0].clone();
                raw.functions[1].blocks.push(block);
            }
            Mutation::Terminator => {
                raw.functions[1].blocks[0].terminator.as_mut().unwrap().kind =
                    OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(37))
            }
            Mutation::Origin => {
                raw.functions[1].blocks[0].statements[1].diagnostic_origins =
                    Some(DiagnosticOrigins {
                        primary: span,
                        cause: span,
                    })
            }
            Mutation::Span => {
                raw.functions[1].span = Span {
                    start: 1,
                    end: 3,
                    ..span
                }
            }
        }
        assert!(prepare(&sources, raw, &keys, imports)
            .check(&sources, budget::Limits::DEFAULT)
            .is_err());
    }
}

#[test]
fn owned_vector_raw_wrapper_rejects_an_operation_outside_its_claimed_suffix() {
    let (sources, mut raw, keys, imports) = fixture(None);
    let span = raw.functions[0].span;
    let user = function(
        0,
        vec![],
        ValueTy::Scalar(hir::Ty::I32),
        span,
        |destination| OwnedInstruction::OwnedVector {
            operation: keys[1],
            destination,
        },
    );
    for function in &mut raw.functions {
        function.id.0 += 1;
    }
    raw.functions.insert(0, user);
    assert!(prepare(&sources, raw, &keys, imports)
        .check(&sources, budget::Limits::DEFAULT)
        .is_err());
}

#[test]
fn owned_vector_raw_empty_extension_preserves_the_old_core_and_representations() {
    let (sources, raw, _) = consumer_fixtures::shared_read();
    let wrapper = prepare(&sources, raw, &[], OwnedVectorImportClaims::empty());
    assert!(wrapper.declarations.is_none());
    assert_eq!(wrapper.claims.functions.capacity(), 0);
    assert_eq!(
        wrapper
            .check(&sources, budget::Limits::DEFAULT)
            .unwrap()
            .owned_functions,
        0
    );
    assert!(verify_owned(wrapper.program, &sources).is_ok());
    #[cfg(target_pointer_width = "64")]
    {
        // The unchanged catalog is nine bytes, padded to sixteen beside
        // the four 24-byte Vec headers: 16 + 4 * 24 = 112.
        assert_eq!(size_of::<RawOwnedProgram>(), 112);
        assert_eq!(size_of::<OwnedInstruction>(), 128);
        assert_eq!(size_of::<Vec<vector::Function>>(), 24);
        assert_eq!(size_of::<vector::Function>(), 2);
        assert_eq!(size_of::<Function>(), 2);
    }
}

#[test]
fn owned_vector_raw_wrapper_checks_actual_returned_facade_at_exact_retention_boundary() {
    for resource_record in [false, true] {
        for one_less in [false, true] {
            let (sources, mut raw, keys, imports) = fixture(None);
            if resource_record {
                raw.records[0].fields[0].ty =
                    ParameterTy::Value(ValueTy::Owned(AggregateTy::ByteBuf));
                raw.builtins = raw.builtins.admit(BuiltinItem::Experimental(
                    crate::frontend::builtin_catalog::experimental::Item::ByteBuf,
                ));
            }
            let independent =
                Declarations::check_combined(&raw.records, &raw.enums, &sources).unwrap();
            assert_eq!(independent.has_resource_declarations(), resource_record);
            let actual = independent.retained_storage_bytes().unwrap();
            assert!(actual > size_of::<Declarations>());
            drop(independent);
            let request =
                RawOwnedVectorProgram::requested(&raw, keys.len(), budget::Limits::DEFAULT)
                    .unwrap();
            let mut limits = budget::Limits::DEFAULT;
            limits.metadata = request.metadata_bytes;
            limits.work = request.work;
            let mut allocator = Allocator::default();
            let result = RawOwnedVectorProgram::prepare_with_retained_limit(
                raw,
                imports,
                &keys,
                &sources,
                limits,
                actual - usize::from(one_less),
                &mut allocator,
            );
            // This deliberately tests the post-return gate. The helper's
            // entire logical scope was prepaid before the real key reserve.
            assert_eq!(allocator.attempts, 1);
            if one_less {
                assert!(matches!(
                    result,
                    Err(OwnedFailure {
                        kind: OwnedFailureKind::Resource(
                            "owned vector returned declaration storage"
                        ),
                        ..
                    })
                ));
            } else {
                let wrapper = result.unwrap();
                assert_eq!(
                    wrapper
                        .declarations
                        .as_ref()
                        .unwrap()
                        .retained_storage_bytes()
                        .unwrap(),
                    actual
                );
                assert_eq!(wrapper.check(&sources, limits).unwrap(), request);
            }
        }
    }
}

#[test]
fn owned_vector_raw_ordinary_verification_rejects_even_an_old_typed_impostor() {
    let (sources, mut raw, _) = consumer_fixtures::shared_read();
    let declarations = Declarations::check_combined(&raw.records, &raw.enums, &sources).unwrap();
    assert!(shape::signatures(&raw, &declarations, &sources).is_ok());
    let span = raw.functions[0].blocks[0].statements[0].span;
    raw.functions[0].blocks[0].statements[0].kind = OwnedInstruction::OwnedVector {
        operation: Function::record(Operation::Len, RecordId(0)).unwrap(),
        destination: CallResult::Scalar(LocalId(0)),
    };
    let mut meter = budget::Meter {
        visits: 0,
        ceiling: budget::Limits::DEFAULT.work,
    };
    let failure = shape::check(&raw.functions[0], &raw, &declarations, &sources, &mut meter)
        .err()
        .unwrap();
    assert_eq!(
        failure.kind,
        OwnedFailureKind::Malformed(Malformed::CanonicalSite)
    );
    assert_eq!(failure.primary.get(), Some(span));
    assert!(verify_owned(raw, &sources).is_err());
}
