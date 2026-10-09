//! Byte-specific raw CFG, staging and suspended-parent controls. No new producer.
use super::super::*;
use super::byte_storage_tests::{parsed, with_raw};
use crate::frontend::source::SourceFileId;

#[test]
fn byte_storage_cfg_requires_array_scalar_operands_to_dominate() {
    let simple = "fn f(b:u8)->i32{let a=[b];return a.len();}";
    with_raw(simple, |sources, _, mut raw| {
        let f = &mut raw.functions[0];
        let construction = f.blocks[0]
            .statements
            .iter()
            .position(|s| matches!(s.kind, OwnedInstruction::ConstructArray { .. }))
            .unwrap();
        let OwnedInstruction::ConstructArray { elements, .. } =
            &f.blocks[0].statements[construction].kind
        else {
            unreachable!()
        };
        let operand = elements[0];
        let definition = f.blocks[0].statements.iter().position(|s| matches!(&s.kind, OwnedInstruction::Scalar(Statement::Assign(a)) if a.destination == operand.local)).unwrap();
        assert!(definition < construction);
        let moved = f.blocks[0].statements.remove(definition);
        f.blocks[0].statements.insert(construction, moved);
        let error = verified::verify_owned(raw, sources).unwrap_err();
        assert_eq!(
            error.kind,
            OwnedFailureKind::Malformed(Malformed::Scalar(FailureKind::Uninitialized))
        );
        assert_eq!(error.primary.get(), Some(operand.span));
    });
    let diamond =
        "fn f(flag:bool,b:u8)->i32{let mut a=[b];if flag{a[0]=b;}else{a[0]=b;}return a.len();}";
    with_raw(diamond, |sources, _, raw| {
        assert!(verified::verify_owned(raw, sources).is_ok());
    });
    with_raw(diamond, |sources, _, mut raw| {
        let f = &mut raw.functions[0];
        let writes: Vec<_> = f
            .blocks
            .iter()
            .enumerate()
            .flat_map(|(block, b)| {
                b.statements
                    .iter()
                    .enumerate()
                    .filter_map(move |(statement, s)| match s.kind {
                        OwnedInstruction::WriteIndex { value, .. } => {
                            Some((block, statement, value))
                        }
                        _ => None,
                    })
            })
            .collect();
        assert_eq!(writes.len(), 2);
        assert_ne!(writes[0].0, writes[1].0);
        // The byte copy produced on one branch cannot initialize the sibling.
        let (block, statement, original) = writes[1];
        let OwnedInstruction::WriteIndex { value, .. } =
            &mut f.blocks[block].statements[statement].kind
        else {
            unreachable!()
        };
        value.local = writes[0].2.local;
        let error = verified::verify_owned(raw, sources).unwrap_err();
        assert_eq!(
            error.kind,
            OwnedFailureKind::Malformed(Malformed::Scalar(FailureKind::Uninitialized))
        );
        assert_eq!(error.primary.get(), Some(original.span));
    });
}

#[test]
fn byte_storage_staged_owners_require_each_argument_once() {
    for n in [0, 1] {
        let text = format!("fn take(a:[u8;{n}],b:[u8;{n}])->i32{{return a.len()+b.len();}}fn relay(a:[u8;{n}],b:[u8;{n}])->i32{{return take(a,b);}}");
        with_raw(&text, |sources, _, raw| {
            assert!(verified::verify_owned(raw, sources).is_ok());
        });
        for mutation in 0..4 {
            with_raw(&text, |sources, _, mut raw| {
                let f = &mut raw.functions[1];
                let preparations: Vec<_> = f.blocks[0]
                    .statements
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| match s.kind {
                        OwnedInstruction::PrepareOwned { source, .. } => Some((i, source)),
                        _ => None,
                    })
                    .collect();
                assert_eq!(preparations.len(), 2);
                let expected = match mutation {
                    0 => {
                        f.blocks[0].statements.remove(preparations[1].0);
                        OwnedFailureKind::Malformed(Malformed::CanonicalSite)
                    }
                    1 => {
                        let duplicate = f.blocks[0].statements[preparations[0].0].clone();
                        f.blocks[0]
                            .statements
                            .insert(preparations[0].0 + 1, duplicate);
                        OwnedFailureKind::Malformed(Malformed::CanonicalSite)
                    }
                    2 => {
                        let OwnedInstruction::PrepareOwned { source, .. } =
                            &mut f.blocks[0].statements[preparations[1].0].kind
                        else {
                            unreachable!()
                        };
                        *source = preparations[0].1;
                        OwnedFailureKind::Ownership(Violation::Unavailable)
                    }
                    _ => {
                        f.calls[0].arguments[1] = f.calls[0].arguments[0];
                        OwnedFailureKind::Malformed(Malformed::Binding)
                    }
                };
                let error = verified::verify_owned(raw, sources).unwrap_err();
                assert_eq!(error.kind, expected, "N={n}, mutation={mutation}");
            });
        }
    }
}

#[test]
fn byte_storage_raw_parent_access_is_suspended_after_child_staging() {
    let text = "fn sink(p:&mut[u8],n:i32)->i32{return n;}fn relay(p:&mut[u8],b:u8)->i32{return sink(&mut *p,0);}";
    with_raw(text, |sources, _, raw| {
        assert!(verified::verify_owned(raw, sources).is_ok());
    });
    for operation in 0..3 {
        with_raw(text, |sources, _, mut raw| {
            let f = &mut raw.functions[1];
            let span = f.span;
            let index = LocalId(f.locals.len());
            f.locals.push(LocalDecl {
                ty: hir::Ty::I32,
                kind: LocalKind::Temporary,
                span,
            });
            let result = LocalId(f.locals.len());
            f.locals.push(LocalDecl {
                ty: if operation == 0 {
                    hir::Ty::U8
                } else {
                    hir::Ty::I32
                },
                kind: LocalKind::Temporary,
                span,
            });
            let byte = f
                .parameters
                .iter()
                .find_map(|p| match p {
                    ParameterBinding::Scalar(local) => Some(*local),
                    _ => None,
                })
                .unwrap();
            let prepare = f.blocks[0]
                .statements
                .iter()
                .position(|s| matches!(s.kind, OwnedInstruction::PrepareBorrow { .. }))
                .unwrap();
            f.blocks[0].statements.insert(
                prepare,
                super::super::consumer_fixtures::assign(index.0, Rvalue::I32(0), span),
            );
            let base = AccessBase::Parameter(ReferenceParamId(0));
            let kind = match operation {
                0 => OwnedInstruction::ReadIndex {
                    destination: result,
                    base,
                    index: Operand { local: index, span },
                },
                1 => OwnedInstruction::ArrayLength {
                    destination: result,
                    base,
                },
                _ => {
                    // Supply a definition for the otherwise-unused diagnostic
                    // result local; no uninitialized-local error may mask loans.
                    f.blocks[0].statements.insert(
                        prepare,
                        super::super::consumer_fixtures::assign(result.0, Rvalue::I32(0), span),
                    );
                    OwnedInstruction::WriteIndex {
                        base,
                        index: Operand { local: index, span },
                        value: Operand { local: byte, span },
                    }
                }
            };
            let prepare = f.blocks[0]
                .statements
                .iter()
                .position(|s| matches!(s.kind, OwnedInstruction::PrepareBorrow { .. }))
                .unwrap();
            f.blocks[0].statements.insert(
                prepare + 1,
                super::super::consumer_fixtures::instruction(kind, span),
            );
            let error = verified::verify_owned(raw, sources).unwrap_err();
            assert_eq!(
                error.kind,
                OwnedFailureKind::Ownership(Violation::LoanConflict),
                "operation={operation}"
            );
            assert_eq!(error.primary.get(), Some(span));
        });
    }
}

#[test]
fn byte_storage_source_later_parent_reads_and_writes_refuse_before_execution() {
    for later in ["p.len()", "read(&*p)", "write(&mut *p)"] {
        let text = format!("fn sink(p:&mut[u8],n:i32)->i32{{return n;}}fn read(p:&[u8])->i32{{let b=p[0];return b.to_i32();}}fn write(p:&mut[u8])->i32{{let b=p[0];p[0]=b;return 0;}}fn relay(p:&mut[u8])->i32{{return sink(&mut *p,{later});}}");
        let (sources, ast) = parsed(&text);
        let errors =
            super::program::check_source(sources.get(SourceFileId(0)), &ast, &sources).unwrap_err();
        assert_eq!(
            (errors[0].code, errors[0].stage),
            ("E0311", "ownership"),
            "{later}"
        );
        let primary = errors[0].primary.unwrap();
        assert_eq!(primary.file, SourceFileId(0));
        let expected = match later {
            "p.len()" => "p.len()",
            "read(&*p)" => "&*p",
            _ => "&mut *p",
        };
        assert_eq!(&text[primary.start..primary.end], expected);
        assert!(primary.start > text.find("sink(&mut *p,").unwrap());
    }
}

#[test]
fn byte_storage_native_plan_keeps_exact_same_shaped_witness_identity() {
    use super::super::plan::native_storage::NativeStoragePlan;
    let text = "fn relay(a:[u8;1])->[u8;1]{return a;}fn f(b:u8)->i32{let a=[b];let x=relay(a);return x.len();}";
    with_raw(text, |sources_a, _, raw_a| {
        let witness_a = verified::verify_owned(raw_a, sources_a).unwrap();
        let plan_a = plan::ExecutionPlan::build(&witness_a).unwrap();
        with_raw(text, |sources_b, _, raw_b| {
            let witness_b = verified::verify_owned(raw_b, sources_b).unwrap();
            let plan_b = plan::ExecutionPlan::build(&witness_b).unwrap();
            assert!(!std::ptr::eq(sources_a, sources_b));
            assert_eq!(plan_a.metadata_bytes(), plan_b.metadata_bytes());
            for (expected, other) in [(&plan_a, &plan_b), (&plan_b, &plan_a)] {
                for guarded in [false, true] {
                    let storage = NativeStoragePlan::checked(expected, guarded).unwrap();
                    assert!(std::ptr::eq(storage.execution(), expected));
                    assert!(!std::ptr::eq(storage.execution(), other));
                    assert_eq!(storage.guarded(), guarded);
                    for raw in expected.witness().functions() {
                        assert!(raw.owners.iter().all(|owner| matches!(owner.aggregate(), AggregateTy::FixedArray(array) if array.element() == hir::Ty::U8 && array.length() == 1)));
                        let function = storage.function(raw.id);
                        assert!(std::ptr::eq(function.raw(), raw));
                        assert!(std::ptr::eq(function.execution(), expected));
                        assert!(!std::ptr::eq(
                            function.execution().witness(),
                            other.witness()
                        ));
                        assert!(!std::ptr::eq(
                            function.raw(),
                            &other.witness().functions()[raw.id.0]
                        ));
                    }
                }
            }
        });
    });
}

#[test]
fn byte_storage_raw_invalid_storage_and_scalar_ids_fail_at_shape() {
    let text = "fn f(b:u8)->i32{let mut a=[b];a[0]=b;let v=a[0];return a.len();}";
    with_raw(text, |sources, _, raw| {
        assert!(verified::verify_owned(raw, sources).is_ok());
    });
    for mutation in 0..9 {
        with_raw(text, |sources, _, mut raw| {
            let mut expected = None;
            for instruction in &mut raw.functions[0].blocks[0].statements {
                let span = instruction.span;
                match &mut instruction.kind {
                    OwnedInstruction::ConstructArray { destination, .. } if mutation == 0 => {
                        *destination = OwnerPlaceId(usize::MAX);
                    }
                    OwnedInstruction::ReadIndex { base, .. } if mutation == 1 => {
                        *base = AccessBase::Owner(OwnerPlaceId(usize::MAX));
                    }
                    OwnedInstruction::WriteIndex { base, .. } if mutation == 2 => {
                        *base = AccessBase::Owner(OwnerPlaceId(usize::MAX));
                    }
                    OwnedInstruction::ArrayLength { base, .. } if mutation == 3 => {
                        *base = AccessBase::Owner(OwnerPlaceId(usize::MAX));
                    }
                    OwnedInstruction::ReadIndex { destination, .. } if mutation == 4 => {
                        *destination = LocalId(usize::MAX);
                    }
                    OwnedInstruction::WriteIndex { value, .. } if mutation == 5 => {
                        value.local = LocalId(usize::MAX);
                        expected = Some(value.span);
                        break;
                    }
                    OwnedInstruction::ReadIndex { base, .. } if mutation == 6 => {
                        *base = AccessBase::Parameter(ReferenceParamId(usize::MAX));
                    }
                    OwnedInstruction::WriteIndex { base, .. } if mutation == 7 => {
                        *base = AccessBase::Parameter(ReferenceParamId(usize::MAX));
                    }
                    OwnedInstruction::ArrayLength { base, .. } if mutation == 8 => {
                        *base = AccessBase::Parameter(ReferenceParamId(usize::MAX));
                    }
                    _ => continue,
                }
                expected = Some(span);
                break;
            }
            let expected = expected.unwrap();
            let error = verified::verify_owned(raw, sources).unwrap_err();
            assert_eq!(
                error.kind,
                OwnedFailureKind::Malformed(Malformed::Id),
                "mutation={mutation}"
            );
            assert_eq!(error.primary.get(), Some(expected), "mutation={mutation}");
        });
    }
}

#[test]
fn byte_storage_raw_owner_slot_roles_are_not_layout_compatible() {
    // A genuine parameter owner has the same array descriptor but cannot be a
    // constructor/move-initialization destination. Its parameter binding is
    // left intact, so the operation's OwnerClass refusal is not masked by IDs.
    let text = "fn f(a:[u8;1],b:u8)->i32{let x=[b];return x.len();}";
    for construction in [false, true] {
        with_raw(text, |sources, _, mut raw| {
            let f = &mut raw.functions[0];
            let parameter = f
                .parameters
                .iter()
                .find_map(|p| match p {
                    ParameterBinding::Owned(owner) => Some(*owner),
                    _ => None,
                })
                .unwrap();
            let target = f.blocks[0]
                .statements
                .iter_mut()
                .find(|s| {
                    matches!(
                        (&s.kind, construction),
                        (OwnedInstruction::ConstructArray { .. }, true)
                            | (OwnedInstruction::MoveInitialize { .. }, false)
                    )
                })
                .unwrap();
            match &mut target.kind {
                OwnedInstruction::ConstructArray { destination, .. }
                | OwnedInstruction::MoveInitialize { destination, .. } => *destination = parameter,
                _ => unreachable!(),
            }
            let span = target.span;
            let error = verified::verify_owned(raw, sources).unwrap_err();
            assert_eq!(
                error.kind,
                OwnedFailureKind::Malformed(Malformed::OwnerClass)
            );
            assert_eq!(error.primary.get(), Some(span));
        });
    }
    // A valid staged argument is not an ordinary byte owner even before the
    // call is opened. Keep the call descriptor and its role binding canonical.
    let text = "fn take(a:[u8;1])->i32{return a.len();}fn f(b:u8)->i32{let mut a=[b];a[0]=b;let v=a[0];let n=a.len();return take(a);}";
    with_raw(text, |sources, _, raw| {
        assert!(verified::verify_owned(raw, sources).is_ok());
    });
    for operation in 0..4 {
        with_raw(text, |sources, _, mut raw| {
            let f = &mut raw.functions[1];
            let ArgumentSlot::Owned(staged) = f.calls[0].arguments[0] else {
                unreachable!()
            };
            let target = f.blocks[0]
                .statements
                .iter_mut()
                .find(|s| {
                    matches!(
                        (&s.kind, operation),
                        (OwnedInstruction::ConstructArray { .. }, 0)
                            | (OwnedInstruction::ReadIndex { .. }, 1)
                            | (OwnedInstruction::WriteIndex { .. }, 2)
                            | (OwnedInstruction::ArrayLength { .. }, 3)
                    )
                })
                .unwrap();
            match &mut target.kind {
                OwnedInstruction::ConstructArray { destination, .. } => *destination = staged,
                OwnedInstruction::ReadIndex { base, .. }
                | OwnedInstruction::WriteIndex { base, .. }
                | OwnedInstruction::ArrayLength { base, .. } => *base = AccessBase::Owner(staged),
                _ => unreachable!(),
            }
            let span = target.span;
            let error = verified::verify_owned(raw, sources).unwrap_err();
            assert_eq!(
                error.kind,
                OwnedFailureKind::Malformed(Malformed::OwnerClass),
                "operation={operation}"
            );
            assert_eq!(error.primary.get(), Some(span));
        });
    }
}

#[test]
fn byte_storage_prepared_owner_cannot_escape_without_its_invocation() {
    use super::super::consumer_fixtures as fixture;
    for n in [0, 1, 1024] {
        let text = format!("fn take(a:[u8;{n}])->i32{{return a.len();}}fn f(a:[u8;{n}],flag:bool)->i32{{return take(a);}}");
        with_raw(&text, |sources, _, raw| {
            assert!(verified::verify_owned(raw, sources).is_ok());
        });
        with_raw(&text, |sources, _, mut raw| {
            let f = &mut raw.functions[1];
            let flag = f
                .parameters
                .iter()
                .find_map(|p| match p {
                    ParameterBinding::Scalar(id) => Some(*id),
                    _ => None,
                })
                .unwrap();
            let span = f.span;
            let result = LocalId(f.locals.len());
            f.locals.push(fixture::scalar(hir::Ty::I32, span));
            f.blocks[0]
                .statements
                .insert(0, fixture::assign(result.0, Rvalue::I32(0), span));
            assert!(f.blocks[0]
                .statements
                .iter()
                .any(|s| matches!(s.kind, OwnedInstruction::PrepareOwned { .. })));
            let invoke = f.blocks[0].terminator.take().unwrap();
            assert!(matches!(invoke.kind, OwnedTerminatorKind::Invoke { .. }));
            let escape = BlockId(f.blocks.len());
            let consume = BlockId(f.blocks.len() + 1);
            f.blocks[0].terminator = fixture::end(
                OwnedTerminatorKind::Branch {
                    condition: Operand { local: flag, span },
                    then_block: escape,
                    else_block: consume,
                },
                span,
            );
            f.blocks.push(OwnedBlock {
                merge: None,
                span,
                statements: vec![],
                terminator: fixture::end(
                    OwnedTerminatorKind::ReturnScalar(Operand {
                        local: result,
                        span,
                    }),
                    span,
                ),
            });
            f.blocks.push(OwnedBlock {
                merge: None,
                span,
                statements: vec![],
                terminator: Some(invoke),
            });
            // One canonical preparation and invocation remain reachable. The
            // added branch exits with a real available staged byte owner and
            // open call, rather than deleting or duplicating a canonical site.
            let error = verified::verify_owned(raw, sources).unwrap_err();
            assert_eq!(
                error.kind,
                OwnedFailureKind::Ownership(Violation::ActiveAtExit),
                "N={n}"
            );
            assert_eq!(error.primary.get(), Some(span));
        });
    }
}
