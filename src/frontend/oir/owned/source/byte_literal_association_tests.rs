//! Focused association mutations. These tests do not grant raw execution authority.
use super::*;
use crate::frontend::{ast, lexer, parser};

const TEXT: &str = r#"fn main()->i32{let a=b"A\x00\xFF";return 0;}"#;

fn with_raw(
    text: &str,
    edit_ast: impl FnOnce(&mut ast::Program),
    action: impl FnOnce(&mut RawOwnedProgram, &super::super::typeck::TypedOwnedProgram<'_>, &SourceMap),
) {
    let mut sources = SourceMap::new();
    let file = sources.add("byte-literal-authentication.ox".into(), text.into());
    let source = sources.get(file);
    let (mut ast, _) = parser::parse_typed_counted(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::ProjectCandidate,
        parser::MAX_NODES,
        &mut crate::frontend::project::budget::Allocator::default(),
        &mut parser::SyntaxStorage::default(),
    )
    .unwrap();
    edit_ast(&mut ast);
    let typed = super::super::typeck::check(
        super::super::resolve::resolve_in_map(source, &ast, &sources).unwrap(),
    )
    .unwrap();
    let mut raw = super::super::lower::lower(&typed).unwrap();
    check_impl(&raw, typed.index(), &sources, false, false, Some(&typed)).unwrap();
    action(&mut raw, &typed, &sources);
}

fn site(function: &RawOwnedFunction) -> (usize, usize) {
    function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, body)| {
            body.statements
                .iter()
                .position(|statement| {
                    matches!(
                        statement.kind,
                        OwnedInstruction::Scalar(Statement::Assign(Assign {
                            value: Rvalue::ByteConst { .. },
                            ..
                        }))
                    )
                })
                .map(|position| (block, position))
        })
        .unwrap()
}
fn assignment(statement: &mut OwnedStatement) -> &mut Assign {
    let OwnedInstruction::Scalar(Statement::Assign(assign)) = &mut statement.kind else {
        panic!("byte assignment")
    };
    assign
}
fn reject(
    raw: &RawOwnedProgram,
    typed: &super::super::typeck::TypedOwnedProgram<'_>,
    sources: &SourceMap,
) {
    let error = check_impl(raw, typed.index(), sources, false, false, Some(typed)).unwrap_err();
    assert_eq!((error.code, error.stage), ("E0500", "oir-project-bind"));
}

#[test]
fn byte_literal_association_accepts_fixed_independent_atom_oracle() {
    with_raw(
        TEXT,
        |_| {},
        |raw, _, _| {
            let function = &raw.functions[0];
            let (block, start) = site(function);
            let expected = [(65, "A"), (0, "\\x00"), (255, "\\xFF")];
            for (ordinal, (value, spelling)) in expected.iter().enumerate() {
                let statement = &function.blocks[block].statements[start + ordinal];
                let OwnedInstruction::Scalar(Statement::Assign(assign)) = &statement.kind else {
                    panic!()
                };
                let Rvalue::ByteConst {
                    value: actual,
                    ordinal: claimed,
                    ..
                } = assign.value
                else {
                    panic!()
                };
                assert_eq!((actual, claimed), (*value, ordinal));
                assert_eq!(&TEXT[assign.span.start..assign.span.end], *spelling);
            }
        },
    );
}

#[test]
fn byte_literal_association_rejects_every_scalar_witness_component() {
    for mutation in 0..11 {
        with_raw(
            TEXT,
            |_| {},
            |raw, typed, sources| {
                let function = &mut raw.functions[0];
                let (block, start) = site(function);
                let instruction = &mut function.blocks[block].statements[start + 1];
                let assign = assignment(instruction);
                let local = assign.destination;
                match mutation {
                    0 => {
                        let Rvalue::ByteConst { value, .. } = &mut assign.value else {
                            panic!()
                        };
                        *value = 1;
                    }
                    1 => {
                        let Rvalue::ByteConst { ordinal, .. } = &mut assign.value else {
                            panic!()
                        };
                        *ordinal = 0;
                    }
                    2 => {
                        let Rvalue::ByteConst { source_expr, .. } = &mut assign.value else {
                            panic!()
                        };
                        *source_expr = ast::ExprId(usize::MAX);
                    }
                    3 => assign.span.start += 1,
                    4 => instruction.span.end -= 1,
                    5 => {
                        instruction
                            .diagnostic_origins
                            .as_mut()
                            .unwrap()
                            .primary
                            .start += 1
                    }
                    6 => function.locals[local.0].ty = hir::Ty::I32,
                    7 => function.locals[local.0].kind = LocalKind::Binding,
                    8 => function.locals[local.0].span.start += 1,
                    9 => instruction.diagnostic_origins = None,
                    10 => assign.span.file = crate::frontend::source::SourceFileId(usize::MAX),
                    _ => unreachable!(),
                }
                reject(raw, typed, sources);
            },
        );
    }
}

#[test]
fn byte_literal_association_rejects_array_operand_and_owner_mutations() {
    for mutation in 0..9 {
        with_raw(
            TEXT,
            |_| {},
            |raw, typed, sources| {
                let function = &mut raw.functions[0];
                let (block, start) = site(function);
                let construct = &mut function.blocks[block].statements[start + 4];
                let OwnedInstruction::ConstructArray {
                    destination,
                    elements,
                } = &mut construct.kind
                else {
                    panic!()
                };
                let owner = *destination;
                match mutation {
                    0 => elements.swap(0, 1),
                    1 => elements[1] = elements[0],
                    2 => {
                        elements.pop();
                    }
                    3 => elements[0].span.end += 1,
                    4 => function.owners[owner.0].kind = OwnerKind::Local { mutable: false },
                    5 => function.owners[owner.0].span.start += 1,
                    6 => {
                        function.owners[owner.0].aggregate = AggregateSlot::try_from_aggregate(
                            AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, 3).unwrap()),
                        )
                        .unwrap()
                    }
                    7 => construct.span.end -= 1,
                    8 => construct.diagnostic_origins = None,
                    _ => unreachable!(),
                }
                reject(raw, typed, sources);
            },
        );
    }
}

#[test]
fn byte_literal_association_rejects_orphan_gap_missing_live_and_replay() {
    for mutation in 0..5 {
        with_raw(
            TEXT,
            |_| {},
            |raw, typed, sources| {
                let function = &mut raw.functions[0];
                let (block, start) = site(function);
                let statements = &mut function.blocks[block].statements;
                match mutation {
                    0 => {
                        let orphan = statements[start].clone();
                        statements.push(orphan);
                    }
                    1 => {
                        let gap = statements[start + 3].clone();
                        statements.insert(start + 1, gap);
                    }
                    2 => {
                        statements.remove(start + 3);
                    }
                    3 => {
                        statements.remove(start + 1);
                    }
                    4 => {
                        let replay = statements[start..start + 5].to_vec();
                        statements.splice(start + 5..start + 5, replay);
                    }
                    _ => unreachable!(),
                }
                reject(raw, typed, sources);
            },
        );
    }
}

#[test]
fn byte_literal_association_rejects_same_text_from_another_function() {
    let text = r#"fn first()->i32{let a=b"A\x00\xFF";return 0;} fn main()->i32{let b=b"A\x00\xFF";return 0;}"#;
    with_raw(
        text,
        |_| {},
        |raw, typed, sources| {
            let donor = raw.functions[0].clone();
            let identity = (raw.functions[1].id, raw.functions[1].span);
            raw.functions[1] = donor;
            raw.functions[1].id = identity.0;
            raw.functions[1].span = identity.1;
            reject(raw, typed, sources);
        },
    );
}

#[test]
fn byte_literal_association_requires_structural_occurrence_not_span_containment() {
    // Add an unreachable AST node with the identical, valid original token
    // span. Bounds, spelling, type and enclosing-span checks would all pass.
    with_raw(
        TEXT,
        |ast| {
            let original = ast
                .expressions
                .iter()
                .find(|expression| matches!(expression.kind, ast::ExprKind::ByteLiteral { .. }))
                .unwrap();
            let span = original.span;
            ast.expressions.push(ast::Expr {
                kind: ast::ExprKind::ByteLiteral { decoded_len: 3 },
                span,
            });
        },
        |raw, typed, sources| {
            let ast = typed
                .index()
                .sources()
                .try_ast_borrowed(crate::frontend::project::ModuleId(0))
                .unwrap();
            let orphan = ast::ExprId(ast.expressions.len() - 1);
            let function = &mut raw.functions[0];
            let (block, start) = site(function);
            for statement in &mut function.blocks[block].statements[start..start + 3] {
                let Rvalue::ByteConst { source_expr, .. } = &mut assignment(statement).value else {
                    panic!()
                };
                *source_expr = orphan;
            }
            reject(raw, typed, sources);
        },
    );
}

#[test]
fn byte_literal_association_rejects_synthetic_and_unpaid_raw_routes() {
    with_raw(
        TEXT,
        |_| {},
        |raw, typed, sources| {
            assert!(check(raw, typed.index(), sources).is_err());
            let mut visitor = Visitor::validate(sources);
            visitor.file(raw.functions[0].span.file);
            assert!(function_with_literals(
                &raw.functions[0],
                &mut visitor,
                false,
                None,
                FunctionSource::Synthetic,
                &mut ConversionSeen::empty(),
                &mut None,
                &mut 0,
                None,
            )
            .is_err());
        },
    );
}

#[test]
fn byte_literal_association_does_not_authorize_a_forged_conversion() {
    let text = r#"fn main()->i32{let a=b"A";let x=1;let b=x.to_u8_checked();return b.to_i32();}"#;
    with_raw(
        text,
        |_| {},
        |raw, typed, sources| {
            let mut changed = false;
            for block in &mut raw.functions[0].blocks {
                for statement in &mut block.statements {
                    if let OwnedInstruction::Scalar(Statement::Assign(Assign {
                        value: Rvalue::CheckedI32ToU8 { name_span, .. },
                        ..
                    })) = &mut statement.kind
                    {
                        name_span.start += 1;
                        changed = true;
                    }
                }
            }
            assert!(changed);
            reject(raw, typed, sources);
        },
    );
}

#[test]
fn byte_literal_association_rejects_identical_text_from_another_source_revision() {
    with_raw(
        TEXT,
        |_| {},
        |raw, typed, _| {
            let mut other = SourceMap::new();
            other.add("byte-literal-authentication.ox".into(), TEXT.into());
            // Byte offsets, SourceFileId and text all agree, but source identity does not.
            reject(raw, typed, &other);
        },
    );
}
