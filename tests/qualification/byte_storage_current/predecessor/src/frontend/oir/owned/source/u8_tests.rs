//! RFC 0030 owned source controls. Expected values and paid event counts are independent.
use super::super::*;
use super::{association, lower, resolve, typeck};
use crate::frontend::{lexer, parser};

pub(in crate::frontend::oir::owned) fn raw_source(text: &str) -> (SourceMap, RawOwnedProgram) {
    let mut sources = SourceMap::new();
    let file = sources.add("owned-u8-source.ox".into(), text.into());
    let source = sources.get(file);
    let ast = parser::parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve(source, &ast).unwrap()).unwrap();
    let raw = lower::lower(&typed).unwrap();
    (sources, raw)
}
pub(in crate::frontend::oir::owned) fn verify_source_raw(
    raw: RawOwnedProgram,
    sources: &SourceMap,
) -> Result<verified::VerifiedOwnedProgram, Box<crate::frontend::diagnostic::Diagnostic>> {
    let source = sources.get(crate::frontend::source::SourceFileId(0));
    let ast = parser::parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve_in_map(source, &ast, sources).unwrap()).unwrap();
    let associated = association::associate(raw, &typed)?;
    verified::verify_associated(associated).map_err(|e| super::diagnostic::verify(&e, sources))
}

fn with_raw<T>(
    text: &str,
    action: impl FnOnce(
        &SourceMap,
        RawOwnedProgram,
        &crate::frontend::declaration_index::DeclarationIndex<'_>,
    ) -> T,
) -> T {
    let mut sources = SourceMap::new();
    let file = sources.add("owned-u8-source.ox".into(), text.into());
    let source = sources.get(file);
    let ast = parser::parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve(source, &ast).unwrap()).unwrap();
    let raw = lower::lower(&typed).unwrap();
    association::check(&raw, typed.index(), &sources).unwrap();
    action(&sources, raw, typed.index())
}
fn run(text: &str) -> Scalar {
    with_raw(text, |sources, raw, _| {
        let entry = raw
            .functions
            .iter()
            .find(|f| sources.get(f.span.file).text_at(f.span) == "main")
            .unwrap()
            .id;
        execute::run(&verify_source_raw(raw, sources).unwrap(), Some(entry)).unwrap()
    })
}
fn errors(text: &str) -> Vec<crate::frontend::diagnostic::Diagnostic> {
    let mut sources = SourceMap::new();
    let file = sources.add("owned-u8-static.ox".into(), text.into());
    let source = sources.get(file);
    let (ast, _) = parser::parse_counted_with_arrays(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut crate::frontend::project::budget::Allocator::default(),
        parser::ArraySyntaxPolicy::Enabled,
    )
    .unwrap();
    match resolve::resolve(source, &ast) {
        Err(errors) => errors,
        Ok(resolved) => typeck::check(resolved).unwrap_err(),
    }
}

#[test]
fn owned_u8_source_helper_mutation_and_unsigned_edges() {
    let text="struct R {} fn byte(x:i32)->u8{return x.to_u8_checked();} fn pass(b:u8)->u8{return b;} fn main()->i32{let a=byte(127);let mut b=pass(byte(128));let c=b;b=byte(255);if a<c && b>a && b>=c && a<=c && a!=b && b==b{return b.to_i32();}return -1;}";
    assert_eq!(run(text), Scalar::I32(255));
    assert_eq!(run("struct R {} fn main()->i32{let x=127;let mut b=x.to_u8_checked();let a=b;let y=128;b=y.to_u8_checked();return a.to_i32();}"),Scalar::I32(127));
}

#[test]
fn owned_u8_source_all_roundtrips() {
    assert_eq!(run("struct R {} fn main()->i32{let mut n=0;while n<256{let b=n.to_u8_checked();let r=b.to_i32();if r!=n{return -1;}n=n+1;}return n;}"),Scalar::I32(256));
}

#[test]
fn owned_u8_source_static_rejections_preserve_legacy_recursive_schedule() {
    let legacy = errors("struct R {} fn main()->bool{return () == (true + 1);}");
    assert_eq!(legacy.len(), 1);
    assert_eq!(
        (legacy[0].code, legacy[0].stage, legacy[0].message.as_str()),
        ("E0300", "type", "equality requires i32 or bool operands")
    );
    let text = "struct R { x: u8 } fn main()->(){return;}";
    let e = errors(text);
    assert_eq!(e.len(), 1);
    assert_eq!(
        (e[0].code, e[0].stage, e[0].message.as_str()),
        ("E0202", "resolve", "u8 record fields are not supported")
    );
    assert_eq!(e[0].primary.unwrap().start, text.find("u8").unwrap());
    for expression in [
        "b+ b",
        "b-b",
        "b*b",
        "b/b",
        "b%b",
        "-b",
        "!b",
        "b&&b",
        "b||b",
        "b==1",
        "1==b",
        "b<1",
        "1<b",
        "b.to_u8_checked()",
        "x.to_i32()",
        "[b]",
    ] {
        let text = format!(
            "struct R {{}} fn main()->(){{let x=1;let b=x.to_u8_checked();{expression};return;}}"
        );
        let e = errors(&text);
        assert_eq!(e[0].code, "E0300", "{expression}: {e:?}");
    }
    for body in [
        "let b:u8=1;return;",
        "let x=true;let b=x.to_u8_checked();return;",
        "let x=();let b=x.to_u8_checked();return;",
    ] {
        assert_eq!(
            errors(&format!("struct R {{}} fn main()->(){{{body}}}"))[0].code,
            "E0300"
        );
    }
    let e = errors("struct R {} fn f(x:&R)->u8{return x.to_u8_checked();} fn main()->(){return;}");
    assert_eq!(e[0].code, "E0312");
}

#[test]
fn owned_u8_source_snapshot_conversion_and_exact_fuel() {
    for value in [255, 256] {
        let text=format!("struct R {{}} fn main()->i32{{let x={value};let b=x.to_u8_checked();return b.to_i32();}}");
        with_raw(&text, |sources, raw, _| {
            let f = &raw.functions[0];
            assert_eq!(
                (
                    f.locals.len(),
                    f.places.len(),
                    f.owners.len(),
                    f.blocks.len()
                ),
                (7, 0, 0, 1)
            );
            let statements = &f.blocks[0].statements;
            assert_eq!(statements.len(), 7);
            let OwnedInstruction::Scalar(Statement::Assign(narrow)) = &statements[3].kind else {
                panic!()
            };
            let Rvalue::CheckedI32ToU8 {
                operand, name_span, ..
            } = narrow.value
            else {
                panic!()
            };
            let OwnedInstruction::Scalar(Statement::Assign(snapshot)) = &statements[2].kind else {
                panic!()
            };
            assert_eq!(operand.local, snapshot.destination);
            assert_ne!(narrow.destination, snapshot.destination);
            assert_eq!(sources.get(operand.span.file).text_at(operand.span), "x");
            assert_eq!(
                sources.get(name_span.file).text_at(name_span),
                "to_u8_checked"
            );
            assert_eq!(
                sources.get(narrow.span.file).text_at(narrow.span),
                "x.to_u8_checked()"
            );
            let receiver = operand.span;
            let conversion = narrow.span;
            let widen = statements[6].span;
            let ret = f.blocks[0].terminator.as_ref().unwrap().span;
            let witness = verify_source_raw(raw, sources).unwrap();
            for (fuel, expected) in [
                (10, Err(RunFailure::Fuel(receiver))),
                (11, Err(RunFailure::Fuel(conversion))),
                (
                    12,
                    if value == 256 {
                        Err(RunFailure::ByteRange(name_span))
                    } else {
                        Err(RunFailure::Fuel(statements_span(&witness, 4)))
                    },
                ),
            ] {
                assert_eq!(
                    execute::run_limits(
                        &witness,
                        Some(hir::DefId(0)),
                        execute::Limits {
                            fuel,
                            ..Default::default()
                        }
                    ),
                    expected.map_err(execute::OwnedRunFailure::Scalar)
                );
            }
            if value == 255 {
                for (fuel, expected) in [
                    (14, Err(RunFailure::Fuel(widen))),
                    (15, Err(RunFailure::Fuel(ret))),
                    (16, Ok(Scalar::I32(255))),
                ] {
                    assert_eq!(
                        execute::run_limits(
                            &witness,
                            Some(hir::DefId(0)),
                            execute::Limits {
                                fuel,
                                ..Default::default()
                            }
                        ),
                        expected.map_err(execute::OwnedRunFailure::Scalar)
                    );
                }
            }
        });
    }
}
fn statements_span(witness: &verified::VerifiedOwnedProgram, index: usize) -> Span {
    witness.functions()[0].blocks[0].statements[index].span
}

#[test]
fn owned_u8_source_entry_is_rejected_before_resource_plans() {
    with_raw(
        "struct R {} fn main()->u8{let x=1;return x.to_u8_checked();}",
        |sources, raw, _| {
            let span = raw.functions[0].span;
            let witness = verify_source_raw(raw, sources).unwrap();
            let e = plan::fail_allocation_after(0, || {
                execute::run_limits(
                    &witness,
                    Some(hir::DefId(0)),
                    execute::Limits {
                        fuel: 0,
                        ..Default::default()
                    },
                )
            })
            .unwrap_err()
            .diagnostic(sources);
            assert_eq!(
                (e.code, e.stage, e.primary),
                ("E0600", "oir-run", Some(span))
            );
            let e = native::native_module(&witness, Some(hir::DefId(0)), sources).unwrap_err();
            assert_eq!(
                (e.code, e.stage, e.primary),
                ("E0700", "native-admission", Some(span))
            );
        },
    );
}

#[test]
fn owned_u8_containing_layout_inventory() {
    use std::mem::{align_of, size_of};
    macro_rules! report {($($ty:ty),+)=>{$(println!("RFC0030 owned layout {} size={} align={}",stringify!($ty),size_of::<$ty>(),align_of::<$ty>());)+}}
    report!(super::hir::ExprKind,super::hir::Expr,super::hir::Function,super::hir::Signature,super::hir::Binding,Option<super::hir::Expr>,ValueTy,ParameterTy,Rvalue,Assign,Statement,OwnedInstruction,OwnedStatement,OwnedBlock,RawOwnedFunction,RawOwnedProgram,Scalar,Option<Scalar>,Result<Scalar,execute::OwnedRunFailure>,execute::OwnedRunFailure,plan::FrameUsage);
}

#[test]
fn owned_u8_source_auth_rejects_in_bounds_swaps_and_forged_binding_reads() {
    const TEXT:&str="struct R {} fn main()->i32{let x=1;let y=2;let a=x.to_u8_checked();let b=y.to_u8_checked();let c=a.to_i32();return b.to_i32();}";
    for mutation in 0..9 {
        with_raw(TEXT, |sources, mut raw, index| {
            let f = &mut raw.functions[0];
            let positions: Vec<_> = f.blocks[0]
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
                .collect();
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
                operand.local = LocalId(1); // y is same-typed, but is not the named x receiver.
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
                    8 => name_span.file = crate::frontend::source::SourceFileId(1),
                    _ => unreachable!(),
                }
            }
            let e = association::check(&raw, index, sources).unwrap_err();
            assert_eq!(
                (e.code, e.stage, e.primary),
                ("E0500", "oir-project-bind", None),
                "mutation={mutation}"
            );
        });
    }
}

#[test]
fn owned_u8_bare_raw_conversion_cannot_acquire_executable_witness() {
    with_raw(
        "struct R {} fn main()->i32{let x=1;let b=x.to_u8_checked();return b.to_i32();}",
        |sources, raw, _| {
            let error = verify_owned(raw, sources).unwrap_err();
            assert_eq!(error.primary.get(), None);
            assert_eq!(
                error.kind,
                OwnedFailureKind::Malformed(Malformed::Scalar(
                    FailureKind::UnauthenticatedConversion
                ))
            );
        },
    );
}

#[test]
fn owned_u8_seeded_enum_source_allows_byte_locals_inside_arms() {
    let text="enum E{V(i32),N} fn read(e:E)->u8{match e{E::V(x)=>{let b=x.to_u8_checked();return b;},E::N=>{let x=0;return x.to_u8_checked();},}} fn main()->i32{let b=read(E::V(255));return b.to_i32();}";
    let mut sources = SourceMap::new();
    let file = sources.add("owned-u8-enum.ox".into(), text.into());
    let source = sources.get(file);
    let (ast, _) = parser::parse_typed_counted(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut crate::frontend::project::budget::Allocator::default(),
        &mut Default::default(),
    )
    .unwrap();
    let owner = crate::frontend::declaration_index::SourceOwner::original(
        source,
        &ast,
        crate::frontend::source::SourceView::Map(&sources),
    )
    .unwrap();
    let (program, entry) = super::program::check_enum_source(owner).unwrap();
    assert_eq!(program.run(entry, &sources).unwrap(), Scalar::I32(255));
    assert!(program
        .native_module(entry, &sources)
        .unwrap()
        .contains("zext i8"));
}

#[test]
fn owned_u8_static_duplicate_occurrence_cannot_reuse_authentication() {
    with_raw(
        "struct R {} fn main()->i32{let x=1;let b=x.to_u8_checked();return b.to_i32();}",
        |sources, mut raw, index| {
            let f = &mut raw.functions[0];
            let mut snapshot = f.blocks[0].statements[2].clone();
            let mut conversion = f.blocks[0].statements[3].clone();
            let OwnedInstruction::Scalar(Statement::Assign(read)) = &mut snapshot.kind else {
                unreachable!()
            };
            let read_old = read.destination;
            read.destination = LocalId(f.locals.len());
            f.locals.push(f.locals[read_old.0].clone());
            let OwnedInstruction::Scalar(Statement::Assign(assign)) = &mut conversion.kind else {
                unreachable!()
            };
            let old = assign.destination;
            assign.destination = LocalId(f.locals.len());
            f.locals.push(f.locals[old.0].clone());
            let Rvalue::CheckedI32ToU8 { operand, .. } = &mut assign.value else {
                unreachable!()
            };
            operand.local = read.destination;
            f.blocks[0].statements.extend([snapshot, conversion]);
            let error = association::check(&raw, index, sources).unwrap_err();
            assert_eq!(
                (error.code, error.stage, error.primary),
                ("E0500", "oir-project-bind", None)
            );
        },
    );
}

#[test]
fn owned_u8_tracker_seed_overlay_has_exact_unchanged_ceiling() {
    let requested =
        crate::frontend::oir::source::association::conversion_seen_bytes(1, 65).unwrap();
    let extra = requested - crate::frontend::oir::source::association::conversion_carrier_bytes();
    let cap = super::hir_budget::MAX_HIR_BYTES;
    assert_eq!(
        super::budget::admit_conversion_scratch(cap - extra, extra).unwrap(),
        cap
    );
    assert!(super::budget::admit_conversion_scratch(cap - extra + 1, extra).is_err());
    assert!(super::budget::admit_conversion_scratch(usize::MAX, extra).is_err());
    println!("RFC0030 owned tracker owners=1 expressions=65 requested={requested} incremental={extra} unchanged_cap={cap}");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn owned_u8_real_native_helpers_and_all_roundtrips() {
    for (text,result) in [
        ("struct R {} fn byte(x:i32)->u8{return x.to_u8_checked();} fn pass(x:u8)->u8{return x;} fn main()->i32{let b=pass(byte(255));return b.to_i32();}",255),
        ("struct R {} fn main()->i32{let x=127;let mut b=x.to_u8_checked();let a=b;let y=128;b=y.to_u8_checked();return a.to_i32();}",127),
        ("struct R {} fn main()->i32{let mut n=0;while n<256{let b=n.to_u8_checked();let r=b.to_i32();if r!=n{return -1;}n=n+1;}return n;}",256),
    ] {
        with_raw(text,|sources,raw,_|{
            let entry=raw.functions.iter().find(|f|sources.get(f.span.file).text_at(f.span)=="main").unwrap().id;
            let witness=verify_source_raw(raw,sources).unwrap();
            let module=native::native_module(&witness,Some(entry),sources).unwrap();
            super::super::super::negation_raw_tests::run_native(&module,&Ok(Scalar::I32(result)),sources);
        });
    }
}

#[test]
fn owned_u8_ordinary_source_tracker_has_real_exact_admission() {
    let mut sources = SourceMap::new();
    let file = sources.add(
        "owned-u8-ordinary-admission.ox".into(),
        "struct R {} fn main()->i32{let x=255;let b=x.to_u8_checked();return b.to_i32();}".into(),
    );
    let source = sources.get(file);
    let ast = parser::parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve_in_map(source, &ast, &sources).unwrap()).unwrap();
    assert_eq!(typed.source_storage_bytes(), None);
    let seed = typed.conversion_association_storage_bytes().unwrap();
    let request =
        crate::frontend::oir::source::association::conversion_seen_bytes(1, ast.expressions.len())
            .unwrap();
    let extra = request - crate::frontend::oir::source::association::conversion_carrier_bytes();
    let exact = seed + extra;
    for (limit, succeeds) in [(exact - 1, false), (exact, true)] {
        let raw = lower::lower(&typed).unwrap();
        let before_work = typed.work().used();
        let before_reserve = association::CONVERSION_RESERVE_ENTRIES.with(|entries| entries.get());
        let result =
            super::budget::with_conversion_limit(limit, || association::associate(raw, &typed));
        assert_eq!(result.is_ok(), succeeds);
        assert!(
            typed.work().used() > before_work,
            "the existing owner meter pays the complete plan walk"
        );
        assert_eq!(
            association::CONVERSION_RESERVE_ENTRIES.with(|entries| entries.get()) - before_reserve,
            usize::from(succeeds)
        );
        if let Ok(associated) = result {
            assert_eq!(
                execute::run(
                    &verified::verify_associated(associated).unwrap(),
                    Some(hir::DefId(0))
                )
                .unwrap(),
                Scalar::I32(255)
            );
        }
    }
    println!("RFC0030 ordinary owned source seed={seed} tracker_increment={extra} exact_association_bytes={exact}; one_less_rejects_before_tracker_constructor");
}

#[test]
fn owned_u8_ordinary_overlay_includes_live_projection_and_zero_route_skips_plan() {
    for conversions in [false, true] {
        let mut sources = SourceMap::new();
        let tail = if conversions {
            "let b=x.to_u8_checked();return b.to_i32();"
        } else {
            "return x;"
        };
        let file = sources.add(
            "owned-u8-projection-admission.ox".into(),
            format!("struct R{{n:i32}} fn main()->i32{{let r=R{{n:255}};let x=r.n;{tail}}}"),
        );
        let source = sources.get(file);
        let ast = parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::OwnedCandidate,
        )
        .unwrap();
        let typed =
            typeck::check(resolve::resolve_in_map(source, &ast, &sources).unwrap()).unwrap();
        let raw = lower::lower(&typed).unwrap();
        if conversions {
            let plain = super::hir_budget::preflight_conversion_hir(typed.index(), typed.work())
                .unwrap()
                .total;
            let seed = typed.conversion_association_storage_bytes().unwrap();
            let capacity_excess = typed.conversion_capacity_excess().unwrap();
            assert!(
                seed > plain + capacity_excess,
                "live projection payload must remain in the containing envelope"
            );
            let requested = crate::frontend::oir::source::association::conversion_seen_bytes(
                1,
                ast.expressions.len(),
            )
            .unwrap();
            let exact = seed + requested
                - crate::frontend::oir::source::association::conversion_carrier_bytes();
            let result =
                super::budget::with_conversion_limit(exact, || association::associate(raw, &typed));
            assert!(result.is_ok());
            println!(
                "RFC0030 ordinary owned projection overlay={} exact_association_bytes={exact}",
                seed - plain - capacity_excess
            );
        } else {
            let work = typed.work().used();
            let reserves = association::CONVERSION_RESERVE_ENTRIES.with(|entries| entries.get());
            let associated =
                super::budget::with_conversion_limit(0, || association::associate(raw, &typed))
                    .unwrap();
            assert_eq!(typed.work().used(), work);
            assert_eq!(
                association::CONVERSION_RESERVE_ENTRIES.with(|entries| entries.get()),
                reserves
            );
            assert_eq!(
                execute::run(
                    &verified::verify_associated(associated).unwrap(),
                    Some(hir::DefId(0))
                )
                .unwrap(),
                Scalar::I32(255)
            );
        }
    }
}

#[test]
fn owned_u8_forged_raw_on_nonconversion_owner_still_pays_before_tracker() {
    let mut sources = SourceMap::new();
    let file = sources.add(
        "owned-u8-forged-nonconversion.ox".into(),
        "struct R{} fn main()->i32{return 1;}".into(),
    );
    let source = sources.get(file);
    let ast = parser::parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve_in_map(source, &ast, &sources).unwrap()).unwrap();
    assert!(ast.expressions.iter().all(|expression| !matches!(
        expression.kind,
        crate::frontend::ast::ExprKind::Conversion { .. }
    )));
    let mut raw = lower::lower(&typed).unwrap();
    let function = &mut raw.functions[0];
    let OwnedInstruction::Scalar(Statement::Assign(assign)) =
        &mut function.blocks[0].statements[0].kind
    else {
        unreachable!()
    };
    assign.value = Rvalue::CheckedI32ToU8 {
        operand: Operand {
            local: assign.destination,
            span: assign.span,
        },
        name_span: assign.span,
        source_expr: crate::frontend::ast::ExprId(0),
    };
    let before = association::CONVERSION_RESERVE_ENTRIES.with(|count| count.get());
    let result = super::budget::with_conversion_limit(0, || association::associate(raw, &typed));
    assert!(result.is_err());
    assert_eq!(
        association::CONVERSION_RESERVE_ENTRIES.with(|count| count.get()),
        before
    );
}
