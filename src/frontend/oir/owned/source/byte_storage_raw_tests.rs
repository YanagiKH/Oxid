//! RFC0031 adversarial descriptors; raw bytes and equal layouts are not authority.
use super::super::*;
use super::{
    association,
    byte_storage_tests::{with_raw, BYTE},
};

fn slot(element: hir::Ty, length: usize) -> AggregateSlot {
    AggregateSlot::try_from_aggregate(AggregateTy::FixedArray(
        FixedArrayTy::check(element, length).unwrap(),
    ))
    .unwrap()
}

#[test]
fn byte_storage_raw_rechecks_empty_element_identity_and_call_direction() {
    for n in [0, 1] {
        for (name, original) in [
            ("bool", hir::Ty::Bool),
            ("()", hir::Ty::Unit),
            ("u8", hir::Ty::U8),
        ] {
            let text = format!("fn len(p:&[{name}])->i32{{return p.len();}}fn relay(a:[{name};{n}])->i32{{return len(&a);}}");
            with_raw(&text, |sources, _, raw| {
                assert!(verified::verify_owned(raw, sources).is_ok());
            });
            for element in [hir::Ty::Bool, hir::Ty::Unit, hir::Ty::U8] {
                if element == original {
                    continue;
                }
                for mutation in 0..3 {
                    with_raw(&text, |sources, _, mut raw| {
                        let other = BorrowedSlot::check(BorrowedTy::ScalarSlice(element)).unwrap();
                        match mutation {
                            0 => raw.functions[1].loans[0].referent = other,
                            1 => raw.functions[0].references[0].referent = other,
                            _ => {
                                raw.functions[1].loans[0].referent = other;
                                raw.functions[0].references[0].referent = other;
                            }
                        }
                        let error = verified::verify_owned(raw, sources).unwrap_err();
                        assert!(
                            matches!(error.kind, OwnedFailureKind::Malformed(_)),
                            "N={n}, {original:?}->{element:?}, mutation={mutation}"
                        );
                    });
                }
            }
        }
    }
    // Conversion-free zero-length controls independently exercise the descriptor
    // failures, without relying on refusal of bare conversion instructions.
    let text =
        "fn len(p:&[u8])->i32{return p.len();}fn main()->i32{let a:[u8;0]=[];return len(&a);}";
    with_raw(text, |sources, _, raw| {
        assert!(verified::verify_owned(raw, sources).is_ok());
    });
    for mutation in 0..6 {
        with_raw(text, |sources, _, mut raw| {
            let other = BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::Bool)).unwrap();
            match mutation {
                0 => raw.functions[1].loans[0].referent = other,
                1 => raw.functions[0].references[0].referent = other,
                2 => {
                    raw.functions[1].loans[0].referent = other;
                    raw.functions[0].references[0].referent = other;
                }
                3 => {
                    raw.functions[1].loans[0].authority =
                        AccessBase::Owner(OwnerPlaceId(usize::MAX))
                }
                4 => {
                    raw.functions[1].loans[0].projection = vec![FieldId {
                        record: RecordId(0),
                        index: 0,
                    }]
                }
                _ => raw.functions[1].loans[0].kind = BorrowKind::Exclusive,
            }
            assert!(
                verified::verify_owned(raw, sources).is_err(),
                "mutation {mutation}"
            );
        });
    }
    let text = "fn fixed(p:&[u8;0])->i32{return p.len();}fn relay(p:&[u8])->i32{return p.len();}fn main()->i32{let a:[u8;0]=[];return relay(&a);}";
    with_raw(text, |sources, _, mut raw| {
        raw.functions[1].references[0].referent =
            BorrowedSlot::check(BorrowedTy::Exact(slot(hir::Ty::U8, 0).aggregate())).unwrap();
        assert!(verified::verify_owned(raw, sources).is_err());
    });
}

#[test]
fn byte_storage_raw_array_identity_scalar_tags_and_lengths_are_rechecked() {
    let text = "fn f(b:u8)->i32{let mut a=[b,b];a[1]=b;let c=a[0];return a.len();}";
    for mutation in 0..7 {
        with_raw(text, |sources, _, mut raw| {
            let f = &mut raw.functions[0];
            match mutation {
                0 => f.owners[0].aggregate = slot(hir::Ty::Bool, 2),
                1 => f.owners[0].aggregate = slot(hir::Ty::U8, 1),
                2 => {
                    f.owners[0].aggregate =
                        AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap()
                }
                _ => {
                    for block in &mut f.blocks {
                        for instruction in &mut block.statements {
                            match &mut instruction.kind {
                                OwnedInstruction::ConstructArray { elements, .. }
                                    if mutation == 3 =>
                                {
                                    elements.pop();
                                }
                                OwnedInstruction::WriteIndex { index, value, .. }
                                    if mutation == 4 =>
                                {
                                    *value = *index
                                }
                                OwnedInstruction::ReadIndex {
                                    destination, index, ..
                                } if mutation == 5 => index.local = *destination,
                                OwnedInstruction::ReadIndex { destination, .. }
                                    if mutation == 6 =>
                                {
                                    f.locals[destination.0].ty = hir::Ty::Bool
                                }
                                _ => (),
                            }
                        }
                    }
                }
            }
            let result = verified::verify_owned(raw, sources);
            assert!(result.is_err(), "mutation {mutation}");
        });
    }
}

#[test]
fn byte_storage_raw_record_byte_fields_are_rejected_even_unused_and_nested() {
    use crate::frontend::oir::owned_types::{RawFieldDecl, RawRecordDecl};
    let (sources, s) = super::super::consumer_fixtures::context();
    for n in [0, 1, 1024] {
        for nested in [false, true] {
            let mut records = vec![RawRecordDecl {
                id: RecordId(0),
                span: s(0),
                fields: vec![RawFieldDecl {
                    id: FieldId {
                        record: RecordId(0),
                        index: 0,
                    },
                    ty: ParameterTy::Value(ValueTy::Owned(slot(hir::Ty::U8, n).aggregate())),
                    span: s(1),
                }],
            }];
            if nested {
                records.push(RawRecordDecl {
                    id: RecordId(1),
                    span: s(2),
                    fields: vec![RawFieldDecl {
                        id: FieldId {
                            record: RecordId(1),
                            index: 0,
                        },
                        ty: ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(0)))),
                        span: s(3),
                    }],
                });
            }
            let raw = RawOwnedProgram {
                builtins: BuiltinOrigins::None,
                enums: vec![],
                records,
                functions: vec![],
            };
            assert!(
                verified::verify_owned(raw, &sources).is_err(),
                "N={n}, nested={nested}"
            );
        }
    }
}

#[test]
fn byte_storage_source_rejects_foreign_identical_text_origins() {
    let text = format!("{BYTE} fn main()->i32{{let mut a=[byte(128)];a[0]=byte(255);let b=a[0];return a.len()+b.to_i32();}}");
    let (mut sources, ast) = super::byte_storage_tests::parsed(&text);
    let foreign = sources.add("same-text-other-file.ox".into(), text.clone());
    let file = sources.get(crate::frontend::source::SourceFileId(0));
    let typed = super::typeck::check(super::resolve::resolve_in_map(file, &ast, &sources).unwrap())
        .unwrap();
    for opcode in 0..4 {
        let mut raw = super::lower::lower(&typed).unwrap();
        let mut changed = false;
        for block in &mut raw.functions[1].blocks {
            for statement in &mut block.statements {
                if matches!(
                    (&statement.kind, opcode),
                    (OwnedInstruction::ConstructArray { .. }, 0)
                        | (OwnedInstruction::WriteIndex { .. }, 1)
                        | (OwnedInstruction::ReadIndex { .. }, 2)
                        | (OwnedInstruction::ArrayLength { .. }, 3)
                ) {
                    statement.span.file = foreign;
                    assert!(sources.is_valid_span(statement.span));
                    changed = true;
                    break;
                }
            }
            if changed {
                break;
            }
        }
        assert!(changed);
        assert!(
            association::associate(raw, &typed).is_err(),
            "opcode {opcode}"
        );
    }
}

#[test]
fn byte_storage_historical_raw_mutation_cannot_change_trusted_lowering() {
    let text = "fn main()->i32{let a:[u8;0]=[];return a.len();}";
    with_raw(text, |_, typed, mut forged| {
        // This is the original crate-internal gap: all zero-width descriptors
        // can be changed together without changing raw shape or span validity.
        // The historical cfg(test) seam deliberately remains observable, but
        // production has no function taking these forged raw operations.
        for owner in &mut forged.functions[0].owners {
            owner.aggregate = slot(hir::Ty::Bool, 0);
        }
        let historical = association::associate(forged, typed).unwrap();
        assert!(verified::verify_associated(historical).is_ok());
        let current = association::lower_and_associate(typed).unwrap();
        assert!(current.program().functions[0]
            .owners
            .iter()
            .all(|owner| owner.aggregate() == slot(hir::Ty::U8, 0).aggregate()));
        assert!(verified::verify_associated(current).is_ok());
    });
}
