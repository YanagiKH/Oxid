//! Independently authored raw vector descriptors and scalar range mutations.
//! Mutations must be rejected before resource execution.
use super::*;
use crate::frontend::builtin_catalog::{experimental, vector, BuiltinItem};

const ELEMENTS: [hir::Ty; 17] = [
    hir::Ty::Bool,
    hir::Ty::I32,
    hir::Ty::U8,
    hir::Ty::Unit,
    hir::Ty::I64,
    hir::Ty::I8,
    hir::Ty::I16,
    hir::Ty::U16,
    hir::Ty::U32,
    hir::Ty::U64,
    hir::Ty::Isize,
    hir::Ty::Usize,
    hir::Ty::I128,
    hir::Ty::U128,
    hir::Ty::F32,
    hir::Ty::F64,
    hir::Ty::Char,
];
const OPERATIONS: [vector::Operation; 9] = [
    vector::Operation::New,
    vector::Operation::Len,
    vector::Operation::Capacity,
    vector::Operation::TryReserve,
    vector::Operation::TryPush,
    vector::Operation::TryExtend,
    vector::Operation::TryResize,
    vector::Operation::Truncate,
    vector::Operation::Take,
];

fn statement(kind: OwnedInstruction, span: Span) -> OwnedStatement {
    OwnedStatement {
        kind,
        span,
        diagnostic_origins: None,
    }
}
fn terminator(kind: OwnedTerminatorKind, span: Span) -> Option<OwnedTerminator> {
    Some(OwnedTerminator {
        kind,
        span,
        diagnostic_origins: None,
    })
}
fn function(id: usize, result: ValueTy, span: Span) -> RawOwnedFunction {
    RawOwnedFunction {
        id: hir::DefId(id),
        span,
        result,
        parameters: vec![],
        locals: vec![],
        places: vec![],
        owners: vec![],
        references: vec![],
        calls: vec![],
        loans: vec![],
        matches: vec![],
        entry: BlockId(0),
        blocks: vec![],
    }
}

/// No catalog signature/member builder supplies these expected declarations.
fn vector_body(id: usize, key: vector::Function, span: Span) -> RawOwnedFunction {
    let element = key.element();
    use vector::Operation::*;
    let result = match key.operation() {
        New | Take => ValueTy::Owned(AggregateTy::Vector(key.element())),
        Len | Capacity => ValueTy::Scalar(hir::Ty::I32),
        Truncate => ValueTy::Scalar(hir::Ty::Unit),
        TryReserve | TryPush | TryExtend | TryResize => {
            ValueTy::Owned(AggregateTy::Enum(EnumId(0)))
        }
    };
    let mut raw = function(id, result, span);
    if key.operation() != New {
        raw.parameters
            .push(ParameterBinding::Reference(ReferenceParamId(0)));
        raw.references.push(ReferenceDecl {
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Vector(key.element())))
                .unwrap(),
            kind: if matches!(key.operation(), Len | Capacity) {
                BorrowKind::Shared
            } else {
                BorrowKind::Exclusive
            },
            position: 0,
            span,
        });
    }
    let scalar_parameters: &[hir::Ty] = match key.operation() {
        TryReserve | Truncate => &[hir::Ty::I32],
        TryPush => std::slice::from_ref(&element),
        TryResize => &[hir::Ty::I32, key.element()],
        _ => &[],
    };
    for ty in scalar_parameters {
        raw.parameters
            .push(ParameterBinding::Scalar(LocalId(raw.locals.len())));
        raw.locals.push(LocalDecl {
            ty: *ty,
            kind: LocalKind::Parameter,
            span,
        });
    }
    if key.operation() == TryExtend {
        raw.parameters
            .push(ParameterBinding::Reference(ReferenceParamId(1)));
        raw.references.push(ReferenceDecl {
            referent: BorrowedSlot::check(BorrowedTy::ScalarSlice(key.element())).unwrap(),
            kind: BorrowKind::Shared,
            position: 1,
            span,
        });
    }
    let mut body = vec![];
    let (destination, end) = match result {
        ValueTy::Owned(aggregate) => {
            raw.owners.push(OwnerDecl {
                aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
                kind: OwnerKind::Temporary,
                span,
            });
            body.push(statement(
                OwnedInstruction::StorageLive(OwnerPlaceId(0)),
                span,
            ));
            (
                CallResult::Owned(OwnerPlaceId(0)),
                OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
            )
        }
        ValueTy::Scalar(ty) => {
            let local = LocalId(raw.locals.len());
            raw.locals.push(LocalDecl {
                ty,
                kind: LocalKind::Temporary,
                span,
            });
            (
                CallResult::Scalar(local),
                OwnedTerminatorKind::ReturnScalar(Operand { local, span }),
            )
        }
    };
    body.push(statement(
        OwnedInstruction::Vector {
            operation: key,
            destination,
        },
        span,
    ));
    raw.blocks.push(OwnedBlock {
        span,
        merge: None,
        statements: body,
        terminator: terminator(end, span),
    });
    raw
}
fn program(keys: &[vector::Function]) -> (SourceMap, RawOwnedProgram) {
    let (sources, at) = consumer_fixtures::context();
    let span = at(0);
    let mut raw = RawOwnedProgram {
        builtins: BuiltinOrigins::None,
        vector_functions: keys.to_vec(),
        enums: vec![],
        records: vec![],
        functions: vec![],
    };
    let mut grow = false;
    for key in keys {
        raw.builtins = raw.builtins.admit(BuiltinItem::Vector(key.item()));
        grow |= matches!(
            key.operation(),
            vector::Operation::TryReserve
                | vector::Operation::TryPush
                | vector::Operation::TryExtend
                | vector::Operation::TryResize
        );
        raw.functions
            .push(vector_body(raw.functions.len(), *key, span));
    }
    if grow {
        raw.enums.push(RawEnumDecl {
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
        });
    }
    (sources, raw)
}
pub(super) fn one_element_program(element: hir::Ty) -> (SourceMap, RawOwnedProgram) {
    program(&OPERATIONS.map(|operation| vector::Function::new(operation, element).unwrap()))
}

#[test]
fn vector_raw_all_operations_and_elements_require_complete_proof() {
    let keys: Vec<_> = ELEMENTS
        .into_iter()
        .flat_map(|element| {
            OPERATIONS.map(|operation| vector::Function::new(operation, element).unwrap())
        })
        .collect();
    assert_eq!(keys.len(), 153);
    let (sources, raw) = program(&keys);
    let witness = verify_owned(raw, &sources).unwrap();
    for (id, key) in keys.iter().enumerate() {
        assert_eq!(witness.vector_operation(hir::DefId(id)), Some(*key));
    }
    assert_eq!(witness.vector_operation(hir::DefId(keys.len())), None);
    assert_eq!(witness.vector_operation(hir::DefId(usize::MAX)), None);
}

#[test]
fn vector_raw_claim_key_signature_body_and_site_mutations_fail() {
    type Mutation = (&'static str, fn(&mut RawOwnedProgram));
    let mutations: &[Mutation] = &[
        ("missing claim", |r| {
            r.vector_functions.clear();
        }),
        ("missing template", |r| {
            r.builtins = BuiltinOrigins::None;
        }),
        ("element key", |r| {
            r.vector_functions[0] =
                vector::Function::new(r.vector_functions[0].operation(), hir::Ty::U64).unwrap();
        }),
        ("operation key", |r| {
            r.vector_functions[0] =
                vector::Function::new(vector::Operation::Take, r.vector_functions[0].element())
                    .unwrap();
        }),
        ("duplicate key", |r| {
            r.vector_functions.push(r.vector_functions[0]);
            let mut duplicate = r.functions[0].clone();
            duplicate.id = hir::DefId(1);
            r.functions.push(duplicate);
        }),
        ("function ID", |r| {
            r.functions[0].id = hir::DefId(1);
        }),
        ("result type", |r| {
            r.functions[0].result = ValueTy::Scalar(hir::Ty::I32);
        }),
        ("owner type", |r| {
            r.functions[0].owners[0].aggregate =
                AggregateSlot::try_from_aggregate(AggregateTy::Vector(hir::Ty::U64)).unwrap();
        }),
        ("owner class", |r| {
            r.functions[0].owners[0].kind = OwnerKind::Local { mutable: true };
        }),
        ("missing lifetime", |r| {
            r.functions[0].blocks[0].statements.remove(0);
        }),
        ("extra instruction", |r| {
            let extra = r.functions[0].blocks[0].statements[1].clone();
            r.functions[0].blocks[0].statements.push(extra);
        }),
        ("scalar destination", |r| {
            if let OwnedInstruction::Vector { destination, .. } =
                &mut r.functions[0].blocks[0].statements[1].kind
            {
                *destination = CallResult::Scalar(LocalId(0));
            }
        }),
        ("invented block", |r| {
            let b = r.functions[0].blocks[0].clone();
            r.functions[0].blocks.push(b);
        }),
        ("wrong entry", |r| {
            r.functions[0].entry = BlockId(1);
        }),
        ("wrong anchor", |r| {
            r.functions[0].blocks[0].statements[1].span.start += 1;
        }),
        ("invented diagnostic origins", |r| {
            let row = &mut r.functions[0].blocks[0].statements[1];
            row.diagnostic_origins = Some(DiagnosticOrigins {
                primary: row.span,
                cause: row.span,
            });
        }),
    ];
    let key = vector::Function::new(vector::Operation::New, hir::Ty::I64).unwrap();
    for (name, mutate) in mutations {
        let (sources, mut raw) = program(&[key]);
        mutate(&mut raw);
        assert!(verify_owned(raw, &sources).is_err(), "{name}");
    }
    for operation in [
        vector::Operation::Take,
        vector::Operation::TryPush,
        vector::Operation::TryExtend,
    ] {
        let (sources, mut raw) =
            program(&[vector::Function::new(operation, hir::Ty::I64).unwrap()]);
        raw.functions[0].references[0].kind = BorrowKind::Shared;
        assert!(
            verify_owned(raw, &sources).is_err(),
            "exclusive parameter: {operation:?}"
        );
    }
    let (sources, mut raw) =
        program(&[vector::Function::new(vector::Operation::TryPush, hir::Ty::I64).unwrap()]);
    raw.functions[0].locals[0].ty = hir::Ty::U64;
    assert!(
        verify_owned(raw, &sources).is_err(),
        "same-width scalar parameter"
    );
}

#[test]
fn vector_raw_matching_ordinary_and_unreachable_bodies_never_gain_identity() {
    let key = vector::Function::new(vector::Operation::New, hir::Ty::I64).unwrap();
    for unreachable in [false, true] {
        let (sources, mut raw) = program(&[key]);
        let mut ordinary = raw.functions[0].clone();
        if unreachable {
            let invalid = ordinary.blocks[0].clone();
            ordinary.locals.push(LocalDecl {
                ty: hir::Ty::I32,
                kind: LocalKind::Temporary,
                span: ordinary.span,
            });
            ordinary.result = ValueTy::Scalar(hir::Ty::I32);
            ordinary.blocks[0] = OwnedBlock {
                span: ordinary.span,
                merge: None,
                statements: vec![consumer_fixtures::assign(0, Rvalue::I32(0), ordinary.span)],
                terminator: terminator(
                    OwnedTerminatorKind::ReturnScalar(Operand {
                        local: LocalId(0),
                        span: ordinary.span,
                    }),
                    ordinary.span,
                ),
            };
            ordinary.blocks.push(invalid);
        }
        raw.functions[0].id = hir::DefId(1);
        raw.functions.insert(0, ordinary);
        assert!(
            builtins::check(&raw).is_ok(),
            "canonical suffix remains intact"
        );
        let failure = verify_owned(raw, &sources).unwrap_err();
        assert_eq!(
            failure.kind,
            OwnedFailureKind::Malformed(Malformed::CanonicalSite)
        );
    }
}

#[test]
fn vector_suffix_preserves_fixed_builtin_ids_and_experimental_rank() {
    let (sources, mut raw) =
        builtin_output_fixtures::inventory(BuiltinOrigins::ReadStdinWriteStdout);
    let span = raw.functions[0].span;
    raw.builtins = raw
        .builtins
        .admit(BuiltinItem::Experimental(experimental::Item::ArgCount))
        .admit(BuiltinItem::Vector(vector::Item::New));
    let mut count = function(3, ValueTy::Scalar(hir::Ty::I32), span);
    count.locals.push(LocalDecl {
        ty: hir::Ty::I32,
        kind: LocalKind::Temporary,
        span,
    });
    count.blocks.push(OwnedBlock {
        span,
        merge: None,
        statements: vec![statement(
            OwnedInstruction::Experimental {
                operation: experimental::Function::ArgCount,
                destination: CallResult::Scalar(LocalId(0)),
            },
            span,
        )],
        terminator: terminator(
            OwnedTerminatorKind::ReturnScalar(Operand {
                local: LocalId(0),
                span,
            }),
            span,
        ),
    });
    raw.functions.push(count);
    let key = vector::Function::new(vector::Operation::New, hir::Ty::I64).unwrap();
    raw.vector_functions.push(key);
    raw.functions.push(vector_body(4, key, span));
    let witness = verify_owned(raw, &sources).unwrap();
    assert_eq!(witness.builtin_function(), Some(hir::DefId(1)));
    assert_eq!(witness.builtin_output_function(), Some(hir::DefId(2)));
    assert_eq!(
        witness.experimental_function(experimental::Function::ArgCount),
        Some(hir::DefId(3))
    );
    assert_eq!(witness.vector_operation(hir::DefId(4)), Some(key));
    for id in 0..4 {
        assert_eq!(witness.vector_operation(hir::DefId(id)), None);
    }
}

#[test]
fn vector_raw_capacity_work_and_metadata_refuse_before_allocating() {
    let (_, raw) = one_element_program(hir::Ty::I64);
    // Nine independently specified bodies cost 106 visits; their duplicate
    // comparisons cost 9*8/2. This is not computed from the checker signature.
    assert_eq!(
        builtins::vector_descriptor_visits(&raw.vector_functions).unwrap(),
        142
    );
    let usage = budget::preflight(&raw, budget::Limits::DEFAULT).unwrap();
    for limits in [
        budget::Limits {
            work: usage.work - 1,
            ..budget::Limits::DEFAULT
        },
        budget::Limits {
            metadata: usage.metadata_bytes - 1,
            ..budget::Limits::DEFAULT
        },
    ] {
        budget::fail_allocation_after(0, || {
            assert!(matches!(
                budget::preflight(&raw, limits).unwrap_err().kind,
                OwnedFailureKind::Resource(_)
            ));
        });
    }
    let (sources, exact) = one_element_program(hir::Ty::I64);
    verify_with_limits(
        exact,
        &sources,
        budget::Limits {
            work: usage.work,
            metadata: usage.metadata_bytes,
            ..budget::Limits::DEFAULT
        },
    )
    .unwrap();
    let key = vector::Function::new(vector::Operation::New, hir::Ty::I64).unwrap();
    let (_, narrow) = program(&[key]);
    let (_, mut spare) = program(&[key]);
    spare.vector_functions.reserve_exact(7);
    let first = budget::preflight(&narrow, budget::Limits::DEFAULT).unwrap();
    let second = budget::preflight(&spare, budget::Limits::DEFAULT).unwrap();
    assert_eq!(
        second.metadata_bytes - first.metadata_bytes,
        (spare.vector_functions.capacity() - narrow.vector_functions.capacity())
            * std::mem::size_of::<vector::Function>()
    );
    let (_, mut over) = program(&[key]);
    over.vector_functions.resize(154, key);
    budget::fail_allocation_after(0, || {
        assert!(budget::preflight(&over, budget::Limits::DEFAULT).is_err())
    });
}

/// The caller receives an authoritative view and selects a range for another
/// ordinary function. No vector operation/template is needed for array/slice T.
fn range_program(
    element: hir::Ty,
    authority: BorrowedTy,
    projected: bool,
) -> (SourceMap, RawOwnedProgram) {
    let (sources, at) = consumer_fixtures::context();
    let mut raw = RawOwnedProgram {
        builtins: BuiltinOrigins::None,
        vector_functions: vec![],
        enums: vec![],
        records: vec![],
        functions: vec![],
    };
    if matches!(authority, BorrowedTy::Exact(AggregateTy::Vector(_))) {
        raw.builtins = raw.builtins.admit(BuiltinItem::Vector(vector::Item::Vec));
    } else if authority == BorrowedTy::Exact(AggregateTy::ByteBuf) {
        raw.builtins = raw
            .builtins
            .admit(BuiltinItem::Experimental(experimental::Item::ByteBuf));
    }
    let (incoming, projection) = if projected {
        let BorrowedTy::Exact(aggregate) = authority else {
            panic!("test projection requires owned field");
        };
        let field = FieldId {
            record: RecordId(0),
            index: 0,
        };
        raw.records.push(RawRecordDecl {
            is_copy: false,
            id: RecordId(0),
            span: at(0),
            fields: vec![RawFieldDecl {
                id: field,
                ty: ParameterTy::Value(ValueTy::Owned(aggregate)),
                span: at(0),
            }],
        });
        (
            BorrowedTy::Exact(AggregateTy::Record(RecordId(0))),
            vec![field],
        )
    } else {
        (authority, vec![])
    };
    let mut caller = function(0, ValueTy::Scalar(hir::Ty::I32), at(0));
    caller.parameters = vec![
        ParameterBinding::Reference(ReferenceParamId(0)),
        ParameterBinding::Scalar(LocalId(0)),
        ParameterBinding::Scalar(LocalId(1)),
    ];
    caller.references.push(ReferenceDecl {
        referent: BorrowedSlot::check(incoming).unwrap(),
        kind: BorrowKind::Shared,
        position: 0,
        span: at(0),
    });
    caller.locals = vec![
        LocalDecl {
            ty: hir::Ty::I32,
            kind: LocalKind::Parameter,
            span: at(0),
        },
        LocalDecl {
            ty: hir::Ty::I32,
            kind: LocalKind::Parameter,
            span: at(0),
        },
        LocalDecl {
            ty: hir::Ty::I32,
            kind: LocalKind::Temporary,
            span: at(4),
        },
    ];
    caller.calls.push(CallDecl {
        target: hir::DefId(1),
        arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
        result: CallResult::Scalar(LocalId(2)),
        parent: None,
        span: at(1),
    });
    caller.loans.push(LoanDecl {
        projection,
        region: LoanRegion::Call {
            call: CallSiteId(0),
            argument: 0,
        },
        authority: AccessBase::Parameter(ReferenceParamId(0)),
        kind: BorrowKind::Shared,
        referent: BorrowedSlot::check(BorrowedTy::ScalarSlice(element)).unwrap(),
        span: at(2),
    });
    caller.blocks.push(OwnedBlock {
        span: at(0),
        merge: None,
        statements: vec![
            statement(OwnedInstruction::OpenCall(CallSiteId(0)), at(1)),
            statement(
                OwnedInstruction::PrepareBorrowRange {
                    call: CallSiteId(0),
                    argument: 0,
                    loan: LoanId(0),
                    start: Operand {
                        local: LocalId(0),
                        span: at(2),
                    },
                    end: Operand {
                        local: LocalId(1),
                        span: at(2),
                    },
                },
                at(2),
            ),
        ],
        terminator: terminator(
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(1),
            },
            at(3),
        ),
    });
    caller.blocks.push(OwnedBlock {
        span: at(4),
        merge: None,
        statements: vec![],
        terminator: terminator(
            OwnedTerminatorKind::ReturnScalar(Operand {
                local: LocalId(2),
                span: at(4),
            }),
            at(4),
        ),
    });
    let mut callee = function(1, ValueTy::Scalar(hir::Ty::I32), at(5));
    callee
        .parameters
        .push(ParameterBinding::Reference(ReferenceParamId(0)));
    callee.references.push(ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::ScalarSlice(element)).unwrap(),
        kind: BorrowKind::Shared,
        position: 0,
        span: at(5),
    });
    callee.locals.push(LocalDecl {
        ty: hir::Ty::I32,
        kind: LocalKind::Temporary,
        span: at(6),
    });
    callee.blocks.push(OwnedBlock {
        span: at(5),
        merge: None,
        statements: vec![statement(
            OwnedInstruction::ArrayLength {
                destination: LocalId(0),
                base: AccessBase::Parameter(ReferenceParamId(0)),
            },
            at(6),
        )],
        terminator: terminator(
            OwnedTerminatorKind::ReturnScalar(Operand {
                local: LocalId(0),
                span: at(7),
            }),
            at(7),
        ),
    });
    raw.functions = vec![caller, callee];
    (sources, raw)
}

#[test]
fn scalar_ranges_keep_all_seventeen_exact_elements_without_vector_imports() {
    for element in ELEMENTS {
        let (sources, raw) = range_program(element, BorrowedTy::ScalarSlice(element), false);
        assert_eq!(raw.builtins, BuiltinOrigins::None);
        verify_owned(raw, &sources).unwrap();
        for length in [0, 1, 1024] {
            for projected in [false, true] {
                let authority = BorrowedTy::Exact(AggregateTy::FixedArray(
                    FixedArrayTy::check(element, length).unwrap(),
                ));
                let (sources, raw) = range_program(element, authority, projected);
                assert_eq!(raw.builtins, BuiltinOrigins::None);
                verify_owned(raw, &sources).unwrap();
            }
        }
        for projected in [false, true] {
            let (sources, raw) = range_program(
                element,
                BorrowedTy::Exact(AggregateTy::Vector(element)),
                projected,
            );
            assert!(raw.vector_functions.is_empty());
            verify_owned(raw, &sources).unwrap();
        }
    }
}

#[test]
fn scalar_ranges_preserve_exclusive_authority_for_every_element() {
    for element in ELEMENTS {
        for authority in [
            BorrowedTy::ScalarSlice(element),
            BorrowedTy::Exact(AggregateTy::FixedArray(
                FixedArrayTy::check(element, 4).unwrap(),
            )),
            BorrowedTy::Exact(AggregateTy::Vector(element)),
        ] {
            let (sources, mut raw) = range_program(element, authority, false);
            raw.functions[0].references[0].kind = BorrowKind::Exclusive;
            raw.functions[0].loans[0].kind = BorrowKind::Exclusive;
            raw.functions[1].references[0].kind = BorrowKind::Exclusive;
            verify_owned(raw, &sources).unwrap();
        }
    }
}

#[test]
fn scalar_range_forgery_cannot_reinterpret_types_or_promote_permissions() {
    for authority in [
        BorrowedTy::ScalarSlice(hir::Ty::I64),
        BorrowedTy::Exact(AggregateTy::FixedArray(
            FixedArrayTy::check(hir::Ty::I64, 4).unwrap(),
        )),
        BorrowedTy::Exact(AggregateTy::Vector(hir::Ty::I64)),
    ] {
        let (sources, raw) = range_program(hir::Ty::U64, authority, false);
        assert!(
            verify_owned(raw, &sources).is_err(),
            "same-width element substitution"
        );
    }
    let (sources, raw) = range_program(hir::Ty::U8, BorrowedTy::Exact(AggregateTy::ByteBuf), false);
    verify_owned(raw, &sources).unwrap();
    let (sources, raw) =
        range_program(hir::Ty::I32, BorrowedTy::Exact(AggregateTy::ByteBuf), false);
    assert!(
        verify_owned(raw, &sources).is_err(),
        "ByteBuf remains byte-only"
    );
    let (sources, mut raw) =
        range_program(hir::Ty::I64, BorrowedTy::ScalarSlice(hir::Ty::I64), false);
    raw.functions[0].loans[0].kind = BorrowKind::Exclusive;
    raw.functions[1].references[0].kind = BorrowKind::Exclusive;
    assert!(
        verify_owned(raw, &sources).is_err(),
        "shared incoming authority cannot become exclusive"
    );
    let (sources, mut raw) =
        range_program(hir::Ty::I64, BorrowedTy::ScalarSlice(hir::Ty::I64), false);
    raw.functions[0].locals[0].ty = hir::Ty::U32;
    assert!(
        verify_owned(raw, &sources).is_err(),
        "range index stays i32"
    );
    let (sources, mut raw) =
        range_program(hir::Ty::I64, BorrowedTy::ScalarSlice(hir::Ty::I64), false);
    raw.functions[0].loans[0].referent = BorrowedSlot::check(BorrowedTy::Exact(
        AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I64, 4).unwrap()),
    ))
    .unwrap();
    assert!(
        verify_owned(raw, &sources).is_err(),
        "range result cannot forge an exact extent"
    );
}

#[test]
fn vector_raw_allocator_refusal_never_produces_a_witness() {
    let (sources, raw) = one_element_program(hir::Ty::I64);
    let failure = budget::fail_allocation_after(0, || verify_owned(raw, &sources)).unwrap_err();
    assert_eq!(
        failure.kind,
        OwnedFailureKind::Resource("injected allocation failure")
    );
}
