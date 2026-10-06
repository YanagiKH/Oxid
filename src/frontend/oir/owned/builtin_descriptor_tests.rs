//! Independently authored raw descriptors. The checker supplies no fixture
//! builder, and successful descriptor checks never grant execution authority.
use super::*;

fn anchor(start: usize) -> Span {
    Span {
        file: crate::frontend::source::SourceFileId(0),
        start,
        end: start + 1,
    }
}

fn enumeration(id: usize, span: Span) -> RawEnumDecl {
    RawEnumDecl {
        id: EnumId(id),
        span,
        variants: vec![
            RawVariantDecl {
                id: VariantId {
                    enumeration: EnumId(id),
                    index: 0,
                },
                payload: Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32))),
                span,
            },
            RawVariantDecl {
                id: VariantId {
                    enumeration: EnumId(id),
                    index: 1,
                },
                payload: None,
                span,
            },
            RawVariantDecl {
                id: VariantId {
                    enumeration: EnumId(id),
                    index: 2,
                },
                payload: None,
                span,
            },
        ],
    }
}

fn statement(kind: OwnedInstruction, span: Span) -> OwnedStatement {
    OwnedStatement {
        kind,
        span,
        diagnostic_origins: None,
    }
}

fn function(id: usize, enumeration: usize, span: Span) -> RawOwnedFunction {
    RawOwnedFunction {
        id: hir::DefId(id),
        span,
        result: ValueTy::Owned(AggregateTy::Enum(EnumId(enumeration))),
        parameters: vec![ParameterBinding::Reference(ReferenceParamId(0))],
        locals: vec![],
        places: vec![],
        owners: vec![OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Enum(EnumId(enumeration)))
                .unwrap(),
            kind: OwnerKind::Temporary,
            span,
        }],
        references: vec![ReferenceDecl {
            referent: BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::I32)).unwrap(),
            kind: BorrowKind::Exclusive,
            position: 0,
            span,
        }],
        calls: vec![],
        loans: vec![],
        matches: vec![],
        entry: BlockId(0),
        blocks: vec![OwnedBlock {
            merge: None,
            span,
            statements: vec![
                statement(OwnedInstruction::StorageLive(OwnerPlaceId(0)), span),
                statement(
                    OwnedInstruction::ReadStdin {
                        buffer: ReferenceParamId(0),
                        destination: OwnerPlaceId(0),
                    },
                    span,
                ),
            ],
            terminator: Some(OwnedTerminator {
                kind: OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
                span,
                diagnostic_origins: None,
            }),
        }],
    }
}

fn canonical(builtins: BuiltinOrigins) -> RawOwnedProgram {
    RawOwnedProgram {
        builtins,
        enums: vec![enumeration(0, anchor(0))],
        records: vec![],
        functions: if builtins == BuiltinOrigins::ReadStdin {
            vec![function(0, 0, anchor(2))]
        } else {
            vec![]
        },
    }
}

fn assert_bad(raw: &RawOwnedProgram, kind: Malformed, label: &str) -> OwnedFailure {
    let error = builtins::check(raw).expect_err(label);
    assert_eq!(error.kind, OwnedFailureKind::Malformed(kind), "{label}");
    assert_eq!(error.related, Origin::NONE, "{label}");
    assert_eq!(error.declaration, Origin::NONE, "{label}");
    assert!(error.context.is_none(), "{label}");
    error
}

#[test]
fn builtin_descriptor_empty_origin_returns_no_identity_without_visiting_rows() {
    let mut raw = canonical(BuiltinOrigins::ReadStdin);
    raw.builtins = BuiltinOrigins::None;
    // Even a matching descriptor has no builtin identity without the claim.
    assert_eq!(
        builtins::check(&raw).unwrap(),
        builtins::BuiltinIds {
            enumeration: None,
            function: None,
        }
    );
    // This intentionally malformed source prefix still belongs to the ordinary
    // proof. The absent builtin fast path must not inspect or validate it.
    raw.enums[0].id = EnumId(usize::MAX);
    raw.enums[0].variants.clear();
    raw.functions[0].id = hir::DefId(usize::MAX);
    raw.functions[0].blocks.clear();
    budget::fail_allocation_after(0, || {
        assert_eq!(
            builtins::check(&raw).unwrap(),
            builtins::BuiltinIds {
                enumeration: None,
                function: None,
            }
        );
    });
    raw.enums.clear();
    raw.functions.clear();
    assert_eq!(builtins::check(&raw).unwrap().enumeration, None);
}

#[test]
fn builtin_descriptor_claim_selects_only_the_canonical_suffix() {
    let status = canonical(BuiltinOrigins::ReadStatus);
    let stdin = canonical(BuiltinOrigins::ReadStdin);
    budget::fail_allocation_after(0, || {
        assert_eq!(
            builtins::check(&status).unwrap(),
            builtins::BuiltinIds {
                enumeration: Some(EnumId(0)),
                function: None,
            }
        );
        assert_eq!(
            builtins::check(&stdin).unwrap(),
            builtins::BuiltinIds {
                enumeration: Some(EnumId(0)),
                function: Some(hir::DefId(0)),
            }
        );
    });
    let mut prefixed = RawOwnedProgram {
        builtins: BuiltinOrigins::ReadStdin,
        // The same shape is legal as a source-origin prefix and does not imply
        // a second builtin. Only the explicit suffix receives builtin identity.
        enums: vec![enumeration(0, anchor(4)), enumeration(1, anchor(0))],
        records: vec![],
        functions: vec![function(0, 1, anchor(4)), function(1, 1, anchor(2))],
    };
    // Source-prefix validation is intentionally outside this helper. The new
    // opcode in this prefix is denied by the ordinary shape proof, not inferred
    // to be another builtin from its matching body.
    assert_eq!(
        builtins::check(&prefixed).unwrap(),
        builtins::BuiltinIds {
            enumeration: Some(EnumId(1)),
            function: Some(hir::DefId(1)),
        }
    );
    prefixed.builtins = BuiltinOrigins::ReadStatus;
    assert_eq!(
        builtins::check(&prefixed).unwrap(),
        builtins::BuiltinIds {
            enumeration: Some(EnumId(1)),
            function: None,
        }
    );
}

#[test]
fn builtin_descriptor_missing_claimed_rows_fail_before_suffix_arithmetic() {
    for origin in [BuiltinOrigins::ReadStatus, BuiltinOrigins::ReadStdin] {
        let mut raw = canonical(origin);
        raw.enums.clear();
        assert_eq!(
            assert_bad(&raw, Malformed::Binding, "missing enum").primary,
            Origin::NONE
        );
    }
    let mut raw = canonical(BuiltinOrigins::ReadStdin);
    raw.functions.clear();
    assert_eq!(
        assert_bad(&raw, Malformed::Binding, "missing function").primary,
        Origin::NONE
    );
}

#[test]
fn builtin_descriptor_enum_rejects_identity_count_order_and_payload_mutations() {
    let mutations: &[(&str, fn(&mut RawOwnedProgram))] = &[
        ("enum ordinal", |p| p.enums[0].id = EnumId(1)),
        ("missing member", |p| {
            p.enums[0].variants.pop();
        }),
        ("empty members", |p| p.enums[0].variants.clear()),
        ("extra member", |p| {
            let v = p.enums[0].variants[2];
            p.enums[0].variants.push(v);
        }),
        ("reordered members", |p| p.enums[0].variants.swap(0, 1)),
        ("member parent", |p| {
            p.enums[0].variants[1].id.enumeration = EnumId(1)
        }),
        ("member ordinal", |p| p.enums[0].variants[2].id.index = 1),
        ("missing eof payload", |p| {
            p.enums[0].variants[0].payload = None
        }),
        ("bool eof payload", |p| {
            p.enums[0].variants[0].payload =
                Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::Bool)))
        }),
        ("unit eof payload", |p| {
            p.enums[0].variants[0].payload =
                Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::Unit)))
        }),
        ("aggregate eof payload", |p| {
            p.enums[0].variants[0].payload = Some(ParameterTy::Value(ValueTy::Owned(
                AggregateTy::Enum(EnumId(0)),
            )))
        }),
        ("reference eof payload", |p| {
            p.enums[0].variants[0].payload = Some(ParameterTy::Reference {
                referent: BorrowedTy::ScalarSlice(hir::Ty::I32),
                kind: BorrowKind::Exclusive,
            })
        }),
        ("full payload", |p| {
            p.enums[0].variants[1].payload = Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32)))
        }),
        ("error payload", |p| {
            p.enums[0].variants[2].payload = Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32)))
        }),
        ("duplicate trailing enum", |p| {
            p.enums.push(enumeration(0, anchor(0)))
        }),
        ("noncanonical trailing enum", |p| {
            let mut e = enumeration(1, anchor(0));
            e.variants.clear();
            p.enums.push(e);
        }),
    ];
    for &(label, mutate) in mutations {
        let mut raw = canonical(BuiltinOrigins::ReadStatus);
        mutate(&mut raw);
        assert_bad(&raw, Malformed::Binding, label);
    }
}

#[test]
fn builtin_descriptor_signature_rejects_binding_and_slot_mutations() {
    let mutations: &[(&str, fn(&mut RawOwnedFunction))] = &[
        ("function ordinal", |f| f.id = hir::DefId(1)),
        ("scalar result", |f| {
            f.result = ValueTy::Scalar(hir::Ty::I32)
        }),
        ("foreign enum result", |f| {
            f.result = ValueTy::Owned(AggregateTy::Enum(EnumId(1)))
        }),
        ("record result", |f| {
            f.result = ValueTy::Owned(AggregateTy::Record(RecordId(0)))
        }),
        ("missing parameter", |f| f.parameters.clear()),
        ("extra parameter", |f| {
            f.parameters
                .push(ParameterBinding::Reference(ReferenceParamId(0)))
        }),
        ("wrong parameter slot", |f| {
            f.parameters[0] = ParameterBinding::Reference(ReferenceParamId(1))
        }),
        ("owned parameter", |f| {
            f.parameters[0] = ParameterBinding::Owned(OwnerPlaceId(0))
        }),
        ("scalar parameter", |f| {
            f.parameters[0] = ParameterBinding::Scalar(LocalId(0))
        }),
        ("missing reference", |f| f.references.clear()),
        ("extra reference", |f| {
            f.references.push(f.references[0].clone())
        }),
        ("reference position", |f| f.references[0].position = 1),
        ("shared reference", |f| {
            f.references[0].kind = BorrowKind::Shared
        }),
        ("bool slice", |f| {
            f.references[0].referent =
                BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::Bool)).unwrap()
        }),
        ("unit slice", |f| {
            f.references[0].referent =
                BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::Unit)).unwrap()
        }),
        ("exact array", |f| {
            f.references[0].referent = BorrowedSlot::check(BorrowedTy::Exact(
                AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, 4).unwrap()),
            ))
            .unwrap()
        }),
        ("missing owner", |f| f.owners.clear()),
        ("extra owner", |f| f.owners.push(f.owners[0].clone())),
        ("foreign enum owner", |f| {
            f.owners[0].aggregate =
                AggregateSlot::try_from_aggregate(AggregateTy::Enum(EnumId(1))).unwrap()
        }),
        ("record owner", |f| {
            f.owners[0].aggregate =
                AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap()
        }),
    ];
    for &(label, mutate) in mutations {
        let mut raw = canonical(BuiltinOrigins::ReadStdin);
        mutate(&mut raw.functions[0]);
        assert_eq!(
            assert_bad(&raw, Malformed::Binding, label).primary.get(),
            Some(anchor(2)),
            "{label}"
        );
    }
    for kind in [
        OwnerKind::Parameter { position: 0 },
        OwnerKind::Local { mutable: true },
        OwnerKind::Local { mutable: false },
        OwnerKind::StagedArgument {
            call: CallSiteId(0),
            argument: 0,
        },
        OwnerKind::CallResult {
            call: CallSiteId(0),
        },
    ] {
        let mut raw = canonical(BuiltinOrigins::ReadStdin);
        raw.functions[0].owners[0].kind = kind;
        assert_bad(&raw, Malformed::Binding, "non-temporary owner");
    }
    let mut raw = canonical(BuiltinOrigins::ReadStdin);
    raw.functions.push(function(0, 0, anchor(2)));
    assert_bad(&raw, Malformed::Binding, "duplicate trailing function");
}

#[test]
fn builtin_descriptor_rejects_extra_function_rows() {
    let mutations: &[(&str, fn(&mut RawOwnedFunction))] = &[
        ("local", |f| {
            f.locals.push(LocalDecl {
                ty: hir::Ty::I32,
                kind: LocalKind::Temporary,
                span: f.span,
            })
        }),
        ("place", |f| {
            f.places.push(PlaceDecl {
                ty: hir::Ty::I32,
                span: f.span,
            })
        }),
        ("call", |f| {
            f.calls.push(CallDecl {
                target: hir::DefId(0),
                arguments: vec![],
                result: CallResult::Owned(OwnerPlaceId(0)),
                parent: None,
                span: f.span,
            })
        }),
        ("loan", |f| {
            f.loans.push(LoanDecl {
                call: CallSiteId(0),
                argument: 0,
                authority: AccessBase::Parameter(ReferenceParamId(0)),
                projection: vec![],
                kind: BorrowKind::Exclusive,
                referent: BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::I32)).unwrap(),
                span: f.span,
            })
        }),
        ("match", |f| {
            f.matches.push(MatchDecl {
                source: OwnerPlaceId(0),
                arms: vec![],
                span: f.span,
            })
        }),
    ];
    for &(label, mutate) in mutations {
        let mut raw = canonical(BuiltinOrigins::ReadStdin);
        mutate(&mut raw.functions[0]);
        assert_bad(&raw, Malformed::Binding, label);
    }
}

#[test]
fn builtin_descriptor_rejects_every_body_site_mutation() {
    let mutations: &[(&str, fn(&mut RawOwnedFunction))] = &[
        ("wrong entry", |f| f.entry = BlockId(1)),
        ("no blocks", |f| f.blocks.clear()),
        ("extra unreachable block", |f| {
            f.blocks.push(f.blocks[0].clone())
        }),
        ("merge", |f| {
            f.blocks[0].merge = Some(BoolMerge {
                operator_span: f.span,
                destination: LocalId(0),
                incoming: [MergeInput {
                    predecessor: BlockId(0),
                    value: Operand {
                        local: LocalId(0),
                        span: f.span,
                    },
                }; 2],
                span: f.span,
            })
        }),
        ("no statements", |f| f.blocks[0].statements.clear()),
        ("missing input", |f| {
            f.blocks[0].statements.pop();
        }),
        ("extra input", |f| {
            let s = f.blocks[0].statements[1].clone();
            f.blocks[0].statements.push(s);
        }),
        ("reordered statements", |f| {
            f.blocks[0].statements.swap(0, 1)
        }),
        ("wrong live owner", |f| {
            f.blocks[0].statements[0].kind = OwnedInstruction::StorageLive(OwnerPlaceId(1))
        }),
        ("discard instead of live", |f| {
            f.blocks[0].statements[0].kind = OwnedInstruction::Discard(OwnerPlaceId(0))
        }),
        ("wrong input reference", |f| {
            f.blocks[0].statements[1].kind = OwnedInstruction::ReadStdin {
                buffer: ReferenceParamId(1),
                destination: OwnerPlaceId(0),
            }
        }),
        ("wrong input owner", |f| {
            f.blocks[0].statements[1].kind = OwnedInstruction::ReadStdin {
                buffer: ReferenceParamId(0),
                destination: OwnerPlaceId(1),
            }
        }),
        ("ordinary constructor", |f| {
            f.blocks[0].statements[1].kind = OwnedInstruction::ConstructEnum {
                destination: OwnerPlaceId(0),
                variant: VariantId {
                    enumeration: EnumId(0),
                    index: 1,
                },
                payload: None,
            }
        }),
        ("missing terminator", |f| f.blocks[0].terminator = None),
        ("wrong return owner", |f| {
            f.blocks[0].terminator.as_mut().unwrap().kind =
                OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(1))
        }),
        ("goto terminator", |f| {
            f.blocks[0].terminator.as_mut().unwrap().kind = OwnedTerminatorKind::Goto(BlockId(0))
        }),
        ("invoke terminator", |f| {
            f.blocks[0].terminator.as_mut().unwrap().kind = OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(0),
            }
        }),
        ("scalar return", |f| {
            f.blocks[0].terminator.as_mut().unwrap().kind =
                OwnedTerminatorKind::ReturnScalar(Operand {
                    local: LocalId(0),
                    span: f.span,
                })
        }),
    ];
    for &(label, mutate) in mutations {
        let mut raw = canonical(BuiltinOrigins::ReadStdin);
        mutate(&mut raw.functions[0]);
        assert_eq!(
            assert_bad(&raw, Malformed::CanonicalSite, label)
                .primary
                .get(),
            Some(anchor(2)),
            "{label}"
        );
    }
}

#[test]
fn builtin_descriptor_requires_common_enum_and_function_anchors() {
    for member in 0..3 {
        let mut raw = canonical(BuiltinOrigins::ReadStatus);
        raw.enums[0].variants[member].span = anchor(4);
        assert_eq!(
            assert_bad(&raw, Malformed::CanonicalSite, "member anchor")
                .primary
                .get(),
            Some(anchor(4))
        );
    }
    let mutations: &[(&str, fn(&mut RawOwnedFunction))] = &[
        ("reference anchor", |f| f.references[0].span = anchor(4)),
        ("owner anchor", |f| f.owners[0].span = anchor(4)),
        ("block anchor", |f| f.blocks[0].span = anchor(4)),
        ("live anchor", |f| {
            f.blocks[0].statements[0].span = anchor(4)
        }),
        ("input anchor", |f| {
            f.blocks[0].statements[1].span = anchor(4)
        }),
        ("return anchor", |f| {
            f.blocks[0].terminator.as_mut().unwrap().span = anchor(4)
        }),
    ];
    for &(label, mutate) in mutations {
        let mut raw = canonical(BuiltinOrigins::ReadStdin);
        mutate(&mut raw.functions[0]);
        assert_eq!(
            assert_bad(&raw, Malformed::CanonicalSite, label)
                .primary
                .get(),
            Some(anchor(4)),
            "{label}"
        );
    }
    let mut raw = canonical(BuiltinOrigins::ReadStatus);
    raw.enums[0].span = anchor(4);
    assert_bad(&raw, Malformed::CanonicalSite, "changed enum parent anchor");
    let mut raw = canonical(BuiltinOrigins::ReadStdin);
    raw.functions[0].span = anchor(4);
    assert_bad(
        &raw,
        Malformed::CanonicalSite,
        "changed function parent anchor",
    );
}

#[test]
fn builtin_descriptor_rejects_diagnostic_origin_overrides_even_at_the_same_anchor() {
    for origin_span in [anchor(2), anchor(4)] {
        for site in 0..3 {
            let mut raw = canonical(BuiltinOrigins::ReadStdin);
            let origins = Some(DiagnosticOrigins {
                primary: origin_span,
                cause: origin_span,
            });
            let block = &mut raw.functions[0].blocks[0];
            if site < 2 {
                block.statements[site].diagnostic_origins = origins;
            } else {
                block.terminator.as_mut().unwrap().diagnostic_origins = origins;
            }
            assert_bad(&raw, Malformed::CanonicalSite, "diagnostic override");
        }
    }
}

#[test]
fn builtin_descriptor_acceptance_does_not_open_any_witness_entrypoint() {
    let mut sources = SourceMap::new();
    sources.add("builtin-descriptor.ox".into(), "abcdef".into());
    for origin in [BuiltinOrigins::ReadStatus, BuiltinOrigins::ReadStdin] {
        let raw = canonical(origin);
        assert!(builtins::check(&raw).is_ok());
        budget::fail_allocation_after(0, || {
            let limits = budget::Limits::DEFAULT;
            let expected = OwnedFailureKind::Malformed(Malformed::Binding);
            assert_eq!(
                verify_owned(canonical(origin), &sources).unwrap_err().kind,
                expected
            );
            assert_eq!(
                verify_with_limits(canonical(origin), &sources, limits)
                    .unwrap_err()
                    .kind,
                expected
            );
            assert_eq!(
                verified::probe_enum_validation(&raw, &sources, limits)
                    .unwrap_err()
                    .kind,
                expected
            );
            assert_eq!(
                verified::probe_array_validation(&raw, &sources, limits)
                    .unwrap_err()
                    .kind,
                expected
            );
            assert_eq!(
                verified::probe_array_reference(
                    canonical(origin),
                    &sources,
                    limits,
                    None,
                    execute::Limits::default(),
                    execute::ObservationControl::default(),
                )
                .unwrap_err()
                .kind,
                expected
            );
            assert_eq!(
                verified::probe_array_native(
                    canonical(origin),
                    &sources,
                    limits,
                    None,
                    &sources,
                    native::NativeControl::default(),
                )
                .err()
                .expect("builtin native probe remains denied")
                .kind,
                expected
            );
        });
    }
}
