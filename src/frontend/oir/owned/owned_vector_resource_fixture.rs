//! Real resource-bearing callers share the same all-eight ordinary schedule.
use super::*;
use crate::frontend::builtin_catalog::experimental as buffer;

fn owned_result(result: CallResult) -> OwnerPlaceId {
    match result {
        CallResult::Owned(owner) => owner,
        CallResult::Scalar(_) => panic!("resource test call returned a scalar"),
    }
}

fn buffer_value(caller: &mut Caller, value: u8) -> OwnerPlaceId {
    let signature = buffer::Function::New.signature(|_| caller.grow);
    let created = owned_result(caller.call_signature(
        hir::DefId(1),
        signature.parameters(),
        signature.result,
        &[],
    ));
    let local = caller.owner(AggregateTy::ByteBuf, OwnerKind::Local { mutable: true });
    caller.statement(OwnedInstruction::MoveInitialize {
        destination: local,
        source: created,
    });
    let byte = caller.byte_constant(value);
    let signature = buffer::Function::Push.signature(|_| caller.grow);
    let status = owned_result(caller.call_signature(
        hir::DefId(2),
        signature.parameters(),
        signature.result,
        &[Input::Borrow(local), Input::Scalar(byte)],
    ));
    caller.statement(OwnedInstruction::Discard(status));
    let payload = caller.owner(AggregateTy::ByteBuf, OwnerKind::Temporary);
    caller.statement(OwnedInstruction::MoveInitialize {
        destination: payload,
        source: local,
    });
    caller.statement(OwnedInstruction::StorageEnd(local));
    payload
}

fn payload_record(
    caller: &mut Caller,
    value: Operand,
    enumerated: bool,
    ordinal: usize,
) -> OwnerPlaceId {
    let payload = if enumerated && ordinal == 1 {
        let owner = caller.owner(AggregateTy::Enum(EnumId(0)), OwnerKind::Temporary);
        caller.statement(OwnedInstruction::ConstructEnum {
            destination: owner,
            variant: VariantId {
                enumeration: EnumId(0),
                index: 2,
            },
            payload: None,
        });
        owner
    } else {
        let buffer = buffer_value(caller, 7 + ordinal as u8);
        if enumerated {
            let owner = caller.owner(AggregateTy::Enum(EnumId(0)), OwnerKind::Temporary);
            caller.statement(OwnedInstruction::ConstructOwnedEnum {
                destination: owner,
                variant: VariantId {
                    enumeration: EnumId(0),
                    index: 0,
                },
                payload: buffer,
            });
            owner
        } else {
            buffer
        }
    };
    let owner = caller.owner(AggregateTy::Record(RecordId(0)), OwnerKind::Temporary);
    caller.statement(OwnedInstruction::ConstructComposite {
        destination: owner,
        fields: vec![
            (
                FieldId {
                    record: RecordId(0),
                    index: 0,
                },
                FieldInitializer::Scalar(value),
            ),
            (
                FieldId {
                    record: RecordId(0),
                    index: 1,
                },
                FieldInitializer::Owned(payload),
            ),
        ],
    });
    owner
}

/// Both payload paths are ordinary checked declarations. The nullary second
/// record deliberately owns no inactive ByteBuf/I128 payload.
pub(in crate::frontend::oir::owned) fn raw(
    enumerated: bool,
) -> (
    SourceMap,
    RawOwnedProgram,
    [Function; 8],
    OwnedVectorImportClaims,
    i32,
) {
    raw_with_schedule(enumerated, Schedule::TruncateThenTake)
}

fn raw_with_schedule(
    enumerated: bool,
    schedule: Schedule,
) -> (
    SourceMap,
    RawOwnedProgram,
    [Function; 8],
    OwnedVectorImportClaims,
    i32,
) {
    let (sources, mut raw, keys, imports) = fixture(None);
    let span = raw.records[0].span;
    let grow = if enumerated { EnumId(1) } else { EnumId(0) };
    if enumerated {
        raw.enums[0].id = grow;
        for variant in &mut raw.enums[0].variants {
            variant.id.enumeration = grow;
        }
        for function in &mut raw.functions {
            if function.result == ValueTy::Owned(AggregateTy::Enum(EnumId(0))) {
                function.result = ValueTy::Owned(AggregateTy::Enum(grow));
            }
            for owner in &mut function.owners {
                if owner.aggregate() == AggregateTy::Enum(EnumId(0)) {
                    owner.aggregate =
                        AggregateSlot::try_from_aggregate(AggregateTy::Enum(grow)).unwrap();
                }
            }
        }
        raw.enums.insert(
            0,
            RawEnumDecl {
                id: EnumId(0),
                span,
                variants: [
                    Some(ValueTy::Owned(AggregateTy::ByteBuf)),
                    Some(ValueTy::Scalar(hir::Ty::I128)),
                    None,
                ]
                .into_iter()
                .enumerate()
                .map(|(index, payload)| RawVariantDecl {
                    id: VariantId {
                        enumeration: EnumId(0),
                        index,
                    },
                    payload: payload.map(ParameterTy::Value),
                    span,
                })
                .collect(),
            },
        );
    }
    raw.records[0].fields.push(RawFieldDecl {
        id: FieldId {
            record: RecordId(0),
            index: 1,
        },
        ty: ParameterTy::Value(ValueTy::Owned(if enumerated {
            AggregateTy::Enum(EnumId(0))
        } else {
            AggregateTy::ByteBuf
        })),
        span,
    });
    raw.builtins = raw
        .builtins
        .admit(BuiltinItem::Experimental(buffer::Item::New))
        .admit(BuiltinItem::Experimental(buffer::Item::Push));
    let mut caller = Caller::new(span, keys);
    caller.owned_start = 4;
    caller.grow = grow;
    let main = all_operations_main(
        caller,
        |caller, value, ordinal| payload_record(caller, value, enumerated, ordinal),
        schedule,
    );
    for function in &mut raw.functions {
        function.id.0 += 3;
    }
    for (id, operation) in [(2, buffer::Function::Push), (1, buffer::Function::New)] {
        let signature = operation.signature(|_| grow);
        let builtin = function(
            id,
            signature.parameters().to_vec(),
            signature.result,
            span,
            |destination| OwnedInstruction::Experimental {
                operation,
                destination,
            },
        );
        raw.functions.insert(0, builtin);
    }
    raw.functions.insert(0, main);
    (sources, raw, keys, imports, 58)
}

fn sealed(enumerated: bool) -> (SourceMap, VerifiedOwnedVectorProgram, [Function; 8], i32) {
    sealed_with_schedule(enumerated, Schedule::TruncateThenTake)
}

fn sealed_with_schedule(
    enumerated: bool,
    schedule: Schedule,
) -> (SourceMap, VerifiedOwnedVectorProgram, [Function; 8], i32) {
    let (sources, raw, keys, imports, expected) = match schedule {
        Schedule::TruncateThenTake => raw(enumerated),
        Schedule::TakeThenTruncate => raw_with_schedule(enumerated, schedule),
    };
    let wrapper = prepare(&sources, raw, &keys, imports);
    let witness = VerifiedOwnedVectorProgram::verify(
        wrapper,
        &sources,
        budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    (sources, witness, keys, expected)
}
pub(in crate::frontend::oir::owned) fn buffers(
) -> (SourceMap, VerifiedOwnedVectorProgram, [Function; 8], i32) {
    sealed(false)
}
pub(in crate::frontend::oir::owned) fn enum_buffers(
) -> (SourceMap, VerifiedOwnedVectorProgram, [Function; 8], i32) {
    sealed(true)
}

pub(in crate::frontend::oir::owned) fn buffers_with_live_take(
) -> (SourceMap, VerifiedOwnedVectorProgram, [Function; 8], i32) {
    sealed_with_schedule(false, Schedule::TakeThenTruncate)
}

#[test]
fn owned_vector_witness_resource_callers_use_live_buffer_builtins_and_joint_enum_alignment() {
    for enumerated in [false, true] {
        let (_, witness, keys, expected) = sealed(enumerated);
        assert_eq!(expected, 58);
        let element = witness.elements().element(RecordId(0)).unwrap();
        assert_eq!(element.identity_capacity(), 1);
        assert_eq!(element.size(), if enumerated { 48 } else { 24 });
        if enumerated {
            assert_eq!(
                witness
                    .declarations()
                    .enums()
                    .variant(
                        EnumId(0),
                        VariantId {
                            enumeration: EnumId(0),
                            index: 0
                        }
                    )
                    .unwrap()
                    .payload_offset(),
                Some(16)
            );
        }
        for key in keys {
            let target = witness
                .functions()
                .iter()
                .find(|function| witness.owned_vector_operation(function.id) == Some(key))
                .unwrap()
                .id;
            assert!(witness.functions()[0]
                .calls
                .iter()
                .any(|call| call.target == target));
        }
        let push = crate::frontend::oir::owned::verified::PlanWitness::experimental_function(
            &witness,
            buffer::Function::Push,
        )
        .unwrap();
        assert_eq!(
            witness.functions()[0]
                .calls
                .iter()
                .filter(|call| call.target == push)
                .count(),
            if enumerated { 1 } else { 2 }
        );
    }
}

#[test]
fn owned_vector_vm_resource_callers_release_live_children_before_terminal_cleanup() {
    for enumerated in [false, true] {
        let (_, witness, _, expected) = sealed(enumerated);
        check_vm_resources(
            &witness,
            expected,
            if enumerated { 2 } else { 3 },
            if enumerated { 552 } else { 360 },
        );
    }
}

#[test]
fn owned_vector_vm_live_buffer_take_retains_child_through_ordinary_move_and_truncate() {
    let (_, witness, _, expected) = buffers_with_live_take();
    check_vm_resources(&witness, expected, 3, 360);
}

fn check_vm_resources(
    witness: &VerifiedOwnedVectorProgram,
    expected: i32,
    allocations: u64,
    peak_bytes: u64,
) {
    use crate::frontend::build_config::ExecutionPolicy;
    use crate::frontend::oir::owned::{byte_buffer::testing, execute};
    let ((result, remaining), observation) = testing::capture(testing::Control::default(), || {
        execute::observe_witness(
            witness,
            Some(hir::DefId(0)),
            execute::Limits::default(),
            ExecutionPolicy::Bounded,
        )
    });
    assert_eq!(result, Ok(Scalar::I32(expected)));
    assert_eq!(observation.before_cleanup.live_allocations, 0);
    assert_eq!(observation.before_cleanup.allocations, allocations);
    assert_eq!(
        observation.before_cleanup.allocations,
        observation.before_cleanup.releases
    );
    assert_eq!(observation.before_cleanup.peak_bytes, peak_bytes);
    assert_eq!(observation.before_cleanup, observation.after_cleanup);
    let exact = execute::Limits::default().fuel - remaining;
    for (fuel, succeeds) in [(exact, true), (exact - 1, false)] {
        let (result, _) = execute::observe_witness(
            witness,
            Some(hir::DefId(0)),
            execute::Limits {
                fuel,
                ..execute::Limits::default()
            },
            ExecutionPolicy::Bounded,
        );
        if succeeds {
            assert_eq!(result, Ok(Scalar::I32(expected)));
        } else {
            assert!(matches!(
                result,
                Err(execute::OwnedRunFailure::Scalar(RunFailure::Fuel(_)))
            ));
        }
    }
    let (result, remaining) = execute::observe_witness(
        witness,
        Some(hir::DefId(0)),
        execute::Limits {
            fuel: 0,
            ..execute::Limits::default()
        },
        ExecutionPolicy::Hosted,
    );
    assert_eq!(result, Ok(Scalar::I32(expected)));
    assert_eq!(remaining, crate::frontend::build_config::HOSTED_FUEL);
}
