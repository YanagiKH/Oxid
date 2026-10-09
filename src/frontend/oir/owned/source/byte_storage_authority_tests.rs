//! RFC0031 residual source-authority controls. Expected origins come from source
//! spelling and frozen predecessor contracts, never from emitted successor text.
use super::super::*;
use super::{association, byte_storage_tests::with_raw};
use crate::frontend::{lexer, parser, source::SourceFileId};

#[test]
fn byte_storage_trusted_lowering_keeps_distinct_read_write_origins() {
    let text = "fn main()->i32{let n=128;let x=255;let lo=n.to_u8_checked();let hi=x.to_u8_checked();let mut a=[lo,hi];a[1]=a[0];let c=a[1];return c.to_i32();}";
    let origin = |container: &str, access: &str| {
        let start = text.find(container).unwrap() + container.find(access).unwrap();
        Span {
            file: SourceFileId(0),
            start,
            end: start + access.len(),
        }
    };
    let expected = [
        ("read", origin("a[1]=a[0];", "a[0]")),
        ("write", origin("a[1]=a[0];", "a[1]")),
        ("read", origin("let c=a[1];", "a[1]")),
    ];
    assert_ne!(expected[0].1, expected[1].1);
    assert_ne!(expected[1].1, expected[2].1);
    with_raw(text, |_, typed, _| {
        // Only the production typed-owner constructor supplies these operations.
        // No promise is made that the historical raw mutation seam compares
        // arbitrary same-file opcodes with an independently rebuilt AST.
        let associated = association::lower_and_associate(typed).unwrap();
        let actual = associated
            .program()
            .functions
            .iter()
            .flat_map(|f| &f.blocks)
            .flat_map(|b| &b.statements)
            .filter_map(|s| match s.kind {
                OwnedInstruction::ReadIndex { .. } => Some(("read", s.primary_span())),
                OwnedInstruction::WriteIndex { .. } => Some(("write", s.primary_span())),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        let witness = verified::verify_associated(associated).unwrap();
        assert_eq!(
            execute::run(&witness, typed.entry()).unwrap(),
            Scalar::I32(128)
        );
    });
}

#[test]
fn byte_storage_array_conversion_authority_rejects_nine_forged_origins() {
    // Mutations and E0500/oir-project-bind/None are the immutable b5455ad
    // owned_u8_source_auth_rejects_in_bounds_swaps_and_forged_binding_reads
    // contract, now with array construction and indexed-read named widening.
    let text = "fn main()->i32{let x=1;let y=2;let a=x.to_u8_checked();let b=y.to_u8_checked();let bytes=[a,b];let c=bytes[0];let d=c.to_i32();return b.to_i32();}";
    with_raw(text, |_, typed, _| {
        let witness =
            verified::verify_associated(association::lower_and_associate(typed).unwrap()).unwrap();
        assert!(witness.functions()[0].blocks[0]
            .statements
            .iter()
            .any(|s| matches!(s.kind, OwnedInstruction::ConstructArray { .. })));
        assert!(witness.functions()[0].blocks[0]
            .statements
            .iter()
            .any(|s| matches!(s.kind, OwnedInstruction::ReadIndex { .. })));
        assert_eq!(
            execute::run(&witness, typed.entry()).unwrap(),
            Scalar::I32(2)
        );
    });
    for mutation in 0..9 {
        with_raw(text, |_, typed, mut raw| {
            let f = &mut raw.functions[0];
            let positions = f.blocks[0]
                .statements
                .iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    matches!(
                        s.kind,
                        OwnedInstruction::Scalar(Statement::Assign(Assign {
                            value: Rvalue::CheckedI32ToU8 { .. },
                            ..
                        }))
                    )
                    .then_some(i)
                })
                .collect::<Vec<_>>();
            assert_eq!(positions.len(), 2);
            let second = match &f.blocks[0].statements[positions[1]].kind {
                OwnedInstruction::Scalar(Statement::Assign(a)) => a.clone(),
                _ => unreachable!(),
            };
            let Rvalue::CheckedI32ToU8 {
                source_expr: other_id,
                operand: other_operand,
                name_span: other_name,
            } = second.value
            else {
                unreachable!()
            };
            if mutation == 6 {
                let OwnedInstruction::Scalar(Statement::Assign(snapshot)) =
                    &mut f.blocks[0].statements[positions[0] - 1].kind
                else {
                    unreachable!()
                };
                let Rvalue::Copy(operand) = &mut snapshot.value else {
                    unreachable!()
                };
                operand.local = LocalId(1); // same-typed y is not the named x receiver
            } else {
                let OwnedInstruction::Scalar(Statement::Assign(first)) =
                    &mut f.blocks[0].statements[positions[0]].kind
                else {
                    unreachable!()
                };
                let Rvalue::CheckedI32ToU8 {
                    source_expr,
                    operand,
                    name_span,
                } = &mut first.value
                else {
                    unreachable!()
                };
                match mutation {
                    0 => *name_span = other_name,
                    1 => operand.span = other_operand.span,
                    2 => *source_expr = other_id,
                    3 => first.span = second.span,
                    4 => source_expr.0 = usize::MAX,
                    5 => {
                        first.value = Rvalue::U8ToI32 {
                            source_expr: *source_expr,
                            operand: *operand,
                            name_span: *name_span,
                        }
                    }
                    7 => first.destination = other_operand.local,
                    8 => name_span.file = SourceFileId(1),
                    _ => unreachable!(),
                }
            }
            let error = association::associate(raw, typed)
                .err()
                .expect("forged conversion authority");
            assert_eq!(
                (error.code, error.stage, error.primary),
                ("E0500", "oir-project-bind", None),
                "mutation={mutation}"
            );
            assert!(error.secondary.is_empty());
            assert!(error.notes.is_empty());
        });
    }
}

#[test]
fn byte_storage_checked_facade_native_diagnostics_keep_own_source_map() {
    let text =
        "fn main()->i32{let n=128;let b=n.to_u8_checked();let a=[b];let v=a[1];return v.to_i32();}";
    let make = |name: &str, prefix: bool| {
        let mut sources = SourceMap::new();
        if prefix {
            sources.add("unused-map-entry.ox".into(), "".into());
        }
        let id = sources.add(name.into(), text.into());
        let source = sources.get(id);
        let (ast, _) = parser::parse_typed_counted(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::OwnedCandidate,
            parser::MAX_NODES,
            &mut crate::frontend::project::budget::Allocator::default(),
            &mut parser::SyntaxStorage::default(),
        )
        .unwrap();
        (sources, ast, id)
    };
    let (left_map, left_ast, left_id) = make("left-byte-map.ox", false);
    let (right_map, right_ast, right_id) = make("right-byte-map.ox", true);
    assert_ne!(left_id, right_id);
    let left =
        crate::frontend::oir::check_source(left_map.get(left_id), &left_ast, &left_map).unwrap();
    let right = crate::frontend::oir::check_source(right_map.get(right_id), &right_ast, &right_map)
        .unwrap();
    let start = text.find("a[1]").unwrap();
    let expected = |name: &str| {
        format!(
            "error[E0606] (oir-owned-run): array index out of bounds\n  --> {name}:1:{}\n",
            start + 1
        )
    };
    let encoded = |message: &str| {
        message
            .bytes()
            .map(|byte| format!("\\{byte:02X}"))
            .collect::<String>()
    };
    // The frozen diagnostic writer supplies the human template; LLVM constant
    // byte escaping is computed directly, independently of the emitter result.
    for (checked, sources, id, name, other) in [
        (
            &left,
            &left_map,
            left_id,
            "left-byte-map.ox",
            "right-byte-map.ox",
        ),
        (
            &right,
            &right_map,
            right_id,
            "right-byte-map.ox",
            "left-byte-map.ox",
        ),
    ] {
        let failure = checked.run().unwrap_err();
        assert_eq!(
            (failure.code, failure.stage, failure.primary),
            (
                "E0606",
                "oir-owned-run",
                Some(Span {
                    file: id,
                    start,
                    end: start + 4
                })
            )
        );
        assert_eq!(failure.render_human(sources), expected(name));
        // The actual sealed facade takes no alternate map. This is not a claim
        // that a crate-private raw renderer authenticates arbitrary SourceMaps.
        let llvm = checked.native_module().unwrap();
        assert!(llvm.contains(&encoded(&expected(name))));
        assert!(!llvm.contains(&encoded(&expected(other))));
    }
}
