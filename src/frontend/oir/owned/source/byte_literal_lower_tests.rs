//! Focused byte-literal oracles. These do not run resource failure probes.
use super::super::*;
use super::{association, lower, resolve, typeck};
use crate::frontend::{lexer, parser};

fn with_typed<T>(text: &str, action: impl FnOnce(&typeck::TypedOwnedProgram<'_>) -> T) -> T {
    let mut sources = SourceMap::new();
    let file = sources.add("byte-literal-lower.ox".into(), text.into());
    let source = sources.get(file);
    let (ast, _) = parser::parse_typed_counted(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::ProjectCandidate,
        parser::MAX_NODES,
        &mut crate::frontend::project::budget::Allocator::default(),
        &mut parser::SyntaxStorage::default(),
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve_in_map(source, &ast, &sources).unwrap()).unwrap();
    action(&typed)
}

#[test]
fn byte_literal_lower_preserves_fixed_values_and_atom_origins() {
    let text = r#"fn main()->i32{let bytes=b"A\0\x7f\x80\xFF\n\r\t\\\"";let byte=bytes[4];return byte.to_i32();}"#;
    with_typed(text, |typed| {
        let raw = lower::lower(typed).unwrap();
        let function = &raw.functions[0];
        let mut actual = Vec::new();
        for statement in &function.blocks[0].statements {
            if let OwnedInstruction::Scalar(Statement::Assign(assign)) = &statement.kind {
                if let Rvalue::ByteConst { value, ordinal, .. } = assign.value {
                    assert_eq!(ordinal, actual.len());
                    assert_eq!(function.locals[assign.destination.0].ty, hir::Ty::U8);
                    assert_eq!(function.locals[assign.destination.0].span, assign.span);
                    assert_eq!(statement.span, assign.span);
                    actual.push((value, typed.text(assign.span)));
                }
            }
        }
        assert_eq!(
            actual,
            vec![
                (65, "A"),
                (0, r"\0"),
                (127, r"\x7f"),
                (128, r"\x80"),
                (255, r"\xFF"),
                (10, r"\n"),
                (13, r"\r"),
                (9, r"\t"),
                (92, r"\\"),
                (34, "\\\"")
            ]
        );
        let associated = association::associate(raw, typed).unwrap();
        let verified = verified::verify_associated(associated).unwrap();
        assert_eq!(
            execute::run(&verified, Some(hir::DefId(0))).unwrap(),
            Scalar::I32(255)
        );
    });
}

#[test]
fn byte_literal_empty_is_intrinsic_owned_u8_without_scalar_witness() {
    with_typed(
        r#"fn main()->i32{let bytes=b"";let moved=bytes;return moved.len();}"#,
        |typed| {
            let raw = lower::lower(typed).unwrap();
            let mut arrays = 0;
            for function in &raw.functions {
                for block in &function.blocks {
                    for statement in &block.statements {
                        match &statement.kind {
                            OwnedInstruction::ConstructArray {
                                destination,
                                elements,
                            } => {
                                arrays += 1;
                                assert!(elements.is_empty());
                                assert_eq!(
                                    function.owners[destination.0].aggregate(),
                                    AggregateTy::FixedArray(
                                        FixedArrayTy::check(hir::Ty::U8, 0).unwrap()
                                    )
                                );
                            }
                            OwnedInstruction::Scalar(Statement::Assign(Assign {
                                value: Rvalue::ByteConst { .. },
                                ..
                            })) => panic!("empty literal emitted a scalar witness"),
                            _ => {}
                        }
                    }
                }
            }
            assert_eq!(arrays, 1);
            let associated = association::associate(raw, typed).unwrap();
            let verified = verified::verify_associated(associated).unwrap();
            assert_eq!(
                execute::run(&verified, Some(hir::DefId(0))).unwrap(),
                Scalar::I32(0)
            );
        },
    );
}

#[test]
fn byte_literal_lower_counts_match_materialized_inventory() {
    with_typed(
        r#"fn main()->i32{let a=b"ab";let empty=b"";return a.len()+empty.len();}"#,
        |typed| {
            let raw = lower::lower(typed).unwrap();
            for (view, function) in typed.functions().zip(&raw.functions) {
                let count = lower::count_preflight_function(&view).unwrap();
                assert_eq!(count.locals, function.locals.len());
                assert_eq!(count.owners, function.owners.len());
                assert_eq!(
                    count.statements,
                    function
                        .blocks
                        .iter()
                        .map(|b| b.statements.len())
                        .sum::<usize>()
                );
                assert_eq!(count.constructed_elements, 2);
            }
        },
    );
}

#[test]
fn byte_literal_intrinsic_type_does_not_change_ordinary_empty_inference() {
    for text in [
        r#"fn main()->(){let a:[i32;1]=b"a";return;}"#,
        r#"fn main()->(){let a:[u8;2]=b"a";return;}"#,
        r#"fn main()->(){let a=[];return;}"#,
    ] {
        let mut sources = SourceMap::new();
        let file = sources.add("byte-literal-type.ox".into(), text.into());
        let source = sources.get(file);
        let (ast, _) = parser::parse_typed_counted(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::ProjectCandidate,
            parser::MAX_NODES,
            &mut crate::frontend::project::budget::Allocator::default(),
            &mut parser::SyntaxStorage::default(),
        )
        .unwrap();
        let errors =
            typeck::check(resolve::resolve_in_map(source, &ast, &sources).unwrap()).unwrap_err();
        assert_eq!(errors[0].code, "E0300");
    }
}
