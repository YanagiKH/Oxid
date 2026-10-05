//! Independently authored Unit4A requirements tests. No execution witness is created.
use super::{hir::*, resolve, typeck};
use crate::frontend::{
    ast,
    diagnostic::Diagnostic,
    lexer, owned_diagnostic as display, parser,
    source::{SourceMap, Span},
};

fn with_source<R>(text: &str, f: impl FnOnce(&crate::frontend::source::SourceFile) -> R) -> R {
    let mut map = SourceMap::new();
    let id = map.add("review-🦀.ox".into(), text.into());
    f(map.get(id))
}
fn parsed(text: &str) -> Result<ast::Program, Vec<Diagnostic>> {
    with_source(text, |source| {
        parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::OwnedCandidate,
        )
    })
}
fn diagnosed(text: &str) -> Vec<Diagnostic> {
    with_source(text, |source| {
        let ast = parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::OwnedCandidate,
        )
        .unwrap();
        match resolve::resolve(source, &ast) {
            Err(errors) => errors,
            Ok(resolved) => typeck::check(resolved).unwrap_err(),
        }
    })
}
fn accepted(text: &str) {
    with_source(text, |source| {
        let ast = parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::OwnedCandidate,
        )
        .unwrap();
        let checked = typeck::check(resolve::resolve(source, &ast).unwrap()).unwrap();
        for view in checked.functions() {
            assert!(view.block_flow(view.hir().body).returns_only());
            for i in 0..view.hir().expressions.len() {
                view.expression_ty(ExprId(i));
            }
            for i in 0..view.hir().bindings.len() {
                view.binding_ty(BindingId(i));
            }
            for i in 0..view.hir().blocks.len() {
                view.block_flow(BodyBlockId(i));
            }
        }
    });
}
fn slice(text: &str, span: Span) -> &str {
    &text[span.start..span.end]
}

#[test]
fn heldout_condition_name_can_share_record_name_at_all_precedence_levels() {
    for cond in [
        "Packet",
        "!Packet",
        "true && Packet",
        "false || Packet",
        "Packet == true",
        "false != Packet",
    ] {
        for keyword in ["if", "while"] {
            let text = format!("struct Packet {{}} fn inspect(Packet: bool) -> () {{ {keyword} {cond} {{ Packet; }} return; }}");
            accepted(&text);
        }
    }
    for cond in [
        "inspect(Packet {})",
        "(inspect(Packet {}))",
        "true && inspect(Packet {})",
    ] {
        accepted(&format!("fn inspect(p: Packet) -> bool {{ return true; }} struct Packet {{}} fn run() -> () {{ if {cond} {{ return; }} return; }}"));
    }
}

#[test]
fn heldout_reference_grammar_cross_product_stays_argument_only() {
    for borrow in ["&v", "&mut v", "&*p", "&mut *p"] {
        parsed(&format!("fn f() -> () {{ call({borrow}); return; }}")).unwrap();
        // RFC 0022 adds only complete named-root field paths at this seam;
        // scalar/record leaves and exact-array formals are still rejected by typing.
        parsed(&format!("fn f() -> () {{ call({borrow}.item); return; }}")).unwrap();
        for expression in [
            format!("({borrow})"),
            format!("{borrow} == v"),
            format!("({borrow}).item"),
            format!("{borrow}.item[0]"),
            format!("{borrow}.item()"),
            format!("{borrow}()"),
            format!("{borrow} + 0"),
        ] {
            assert!(
                parsed(&format!("fn f() -> () {{ call({expression}); return; }}")).is_err(),
                "{expression}"
            );
        }
        for statement in [
            format!("let r = {borrow};"),
            format!("return {borrow};"),
            format!("{borrow};"),
        ] {
            assert!(
                parsed(&format!("fn f() -> () {{ {statement} }}")).is_err(),
                "{statement}"
            );
        }
    }
    for ty in ["&Packet", "&mut Packet"] {
        assert!(parsed(&format!("struct Packet {{ v: {ty} }}")).is_err());
        assert!(parsed(&format!("fn f() -> {ty} {{ return; }}")).is_err());
        assert!(parsed(&format!("fn f() -> () {{ let r: {ty} = 0; return; }}")).is_err());
    }
}

#[test]
fn heldout_postfix_and_list_boundaries_are_closed() {
    for expr in [
        "(v).item",
        "call().item",
        "(v.item).next",
        "v.item()",
        "(*p).item",
        "Packet { item }",
        "Packet { item: 1, ..v }",
        "Packet { item: 1 } . item",
        "call(&v,)",
        "call(1,)",
    ] {
        assert!(
            parsed(&format!("fn f() -> () {{ {expr}; return; }}")).is_err(),
            "{expr}"
        );
    }
    accepted("struct Packet { item: i32, } fn f() -> Packet { return Packet { item: 4, }; }");
}

#[test]
fn heldout_nominality_forward_declarations_and_lexical_namespaces() {
    accepted("fn make(v: Packet) -> Packet { return v; } fn Packet() -> Packet { return Packet {}; } struct Packet {} fn entry() -> Packet { return make(Packet()); }");
    accepted("struct Packet {} fn entry(Packet: Packet) -> Packet { return Packet; }");
    accepted("struct Packet {} fn entry() -> () { if true { let Packet = 1; } else { let Packet = 2; } let Packet = 3; return; }");
    for (source, expected) in [
        ("struct Packet {} fn entry() -> () { let Packet = Packet; return; }", "E0200"),
        ("struct Packet {} fn entry() -> () { let make = Packet {}; return; } fn make() -> () { return; }", "E0201"),
        ("struct Packet {} fn entry(make: Packet) -> () { return; } fn make() -> () { return; }", "E0201"),
        ("struct Packet {} fn entry(p: Packet) -> () { while false { let p = 1; } return; }", "E0201"),
        ("struct Packet {} struct Other {} fn entry() -> Packet { return Other {}; }", "E0300"),
        ("fn entry() -> () { let absent: Missing = 0; return; } struct Packet {}", "E0202"),
    ] { assert_eq!(diagnosed(source)[0].code, expected, "{source}"); }
}

#[test]
fn heldout_literal_permutations_preserve_source_order_and_nominal_field_ids() {
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let names = ["count", "ready", "token"];
        let values = ["9", "true", "()"];
        let fields = order
            .iter()
            .map(|&i| format!("{}: {}", names[i], values[i]))
            .collect::<Vec<_>>()
            .join(",");
        let text = format!("struct Packet {{ count: i32, ready: bool, token: () }} fn entry() -> Packet {{ return Packet {{ {fields} }}; }}");
        with_source(&text, |source| {
            let ast = parser::parse_with_mode(
                source,
                lexer::lex(source).unwrap(),
                parser::SourceMode::OwnedCandidate,
            )
            .unwrap();
            let typed = typeck::check(resolve::resolve(source, &ast).unwrap()).unwrap();
            let view = typed.functions().next().unwrap();
            let literal = view
                .hir()
                .expressions
                .iter()
                .find_map(|e| {
                    if let ExprKind::StructLiteral { fields, .. } = &e.kind {
                        Some(fields)
                    } else {
                        None
                    }
                })
                .unwrap();
            assert_eq!(
                literal.iter().map(|f| f.field.index).collect::<Vec<_>>(),
                order
            );
            assert_eq!(
                literal
                    .iter()
                    .map(|f| slice(&text, view.hir().expressions[f.value.0].span))
                    .collect::<Vec<_>>(),
                order.map(|i| values[i])
            );
        });
    }
}

#[test]
fn heldout_mixed_signature_binding_positions_and_projection_views_are_complete() {
    let text="struct Packet { count: i32, ready: bool } fn work(a: i32, p: &Packet, q: Packet, r: &mut Packet, z: bool) -> Packet { let mut copy = q; copy.count = a; r.ready = z; let seen = p.count; return copy; }";
    with_source(text, |source| {
        let ast = parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::OwnedCandidate,
        )
        .unwrap();
        let typed = typeck::check(resolve::resolve(source, &ast).unwrap()).unwrap();
        let view = typed.functions().next().unwrap();
        let expected = [
            ParameterTy::Value(ValueTy::Scalar(Ty::I32)),
            ParameterTy::Reference {
                referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(0))),
                kind: BorrowKind::Shared,
            },
            ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(0)))),
            ParameterTy::Reference {
                referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(0))),
                kind: BorrowKind::Exclusive,
            },
            ParameterTy::Value(ValueTy::Scalar(Ty::Bool)),
        ];
        assert_eq!(view.signature().params, expected);
        for (i, expected) in expected.into_iter().enumerate() {
            assert_eq!(view.binding_ty(BindingId(i)), expected);
            assert_eq!(view.hir().bindings[i].parameter_position, Some(i));
        }
        let p = view.statement_projection(view.hir().body, 1).unwrap();
        assert_eq!(p.base, AccessBase::Owner(BindingId(5)));
        assert_eq!(p.field.index, 0);
        let p = view.statement_projection(view.hir().body, 2).unwrap();
        assert_eq!(
            p.base,
            AccessBase::Reference {
                binding: BindingId(3),
                kind: BorrowKind::Exclusive
            }
        );
        assert_eq!(p.field.index, 1);
        let read = view
            .hir()
            .expressions
            .iter()
            .position(|e| matches!(e.kind, ExprKind::FieldRead { .. }))
            .unwrap();
        assert_eq!(
            view.expression_projection(ExprId(read)).unwrap().base,
            AccessBase::Reference {
                binding: BindingId(1),
                kind: BorrowKind::Shared
            }
        );
        assert_eq!(view.expression_ty(ExprId(read)), ValueTy::Scalar(Ty::I32));
        assert!(typed.entry().is_none());
    });
}

#[test]
fn heldout_permission_requests_survive_source_typing_without_ownership_authority() {
    for body in [
        "p.count = 1;",
        "sink(&mut *p);",
        "read(&*p, &*p);",
        "sink(&mut *p); sink(&mut *p);",
    ] {
        accepted(&format!("struct Packet {{ count: i32 }} fn sink(q: &mut Packet) -> () {{ return; }} fn read(a: &Packet,b: &Packet) -> () {{ return; }} fn test(p: &Packet) -> () {{ {body} return; }}"));
    }
    accepted("struct Packet {} fn pair(a: &Packet,b: Packet) -> () { return; } fn test() -> () { let p = Packet {}; pair(&p,p); p; return; }");
    for source in [
        "struct Packet { count: i32 } fn test(p: Packet) -> () { p.count = 0; return; }",
        "struct Packet {} fn sink(p: &mut Packet) -> () { return; } fn test(p: Packet) -> () { sink(&mut p); return; }",
    ] { let d=diagnosed(source); assert_eq!(d[0].code,"E0304"); assert_eq!(d[0].secondary.len(),1); }
}

#[test]
fn heldout_bare_references_and_wrong_place_categories_fail_before_lowering() {
    for use_site in ["((p));", "let x = ((p));", "return ((p));", "take((p));"] {
        let text=format!("struct Packet {{}} fn take(p: Packet) -> Packet {{ return p; }} fn test(p: &Packet) -> Packet {{ {use_site} return Packet {{}}; }}");
        let d = diagnosed(&text);
        assert_eq!(d[0].code, "E0312");
        assert_eq!(slice(&text, d[0].primary.unwrap()), "p");
    }
    for (parameter, argument, code) in [
        ("p: &Packet", "&p", "E0312"),
        ("p: &mut Packet", "&mut p", "E0312"),
        ("p: Packet", "&*p", "E0312"),
        ("p: bool", "&p", "E0300"),
    ] {
        let text=format!("struct Packet {{}} fn take(v: &Packet) -> () {{ return; }} fn test({parameter}) -> () {{ take({argument}); return; }}");
        assert_eq!(diagnosed(&text)[0].code, code);
    }
}

#[test]
fn heldout_unreachable_and_return_flow_matrix() {
    for body in [
        "if true { return; } else { return; }",
        "while false { if true { break; } else { continue; } } return;",
        "while true { while true { break; } continue; } return;",
    ] {
        accepted(&format!("struct Packet {{}} fn test() -> () {{ {body} }}"));
    }
    for body in [
        "if true { return; }",
        "while true { return; }",
        "while false {}",
    ] {
        assert_eq!(
            diagnosed(&format!("struct Packet {{}} fn test() -> () {{ {body} }}"))[0].code,
            "E0302"
        );
    }
    for body in [
        "if true { return; } else { return; } 1;",
        "while true { if true { break; } else { continue; } 1; } return;",
        "while true { while true { break; 1; } } return;",
    ] {
        assert_eq!(
            diagnosed(&format!("struct Packet {{}} fn test() -> () {{ {body} }}"))[0].code,
            "E0303"
        );
    }
}

#[test]
fn heldout_lazy_and_unused_typing_and_resolve_phase_order() {
    for body in [
        "false && (1 == true)",
        "true || (1 + false == 2)",
        "false && hidden(1)",
    ] {
        let text=format!("struct Packet {{}} fn hidden(v: bool) -> bool {{ return v; }} fn unused() -> bool {{ return {body}; }} fn main() -> () {{ return; }}");
        assert_eq!(diagnosed(&text)[0].code, "E0300");
    }
    let text="struct Packet {} fn first() -> () { let bad: bool = 1; return; } fn later() -> () { let missing: NotAType = 0; return; }";
    assert_eq!(diagnosed(text)[0].code, "E0202");
    let text="struct Packet { count: i32 } fn first() -> () { let p = Packet { count: true }; return; } fn later() -> () { missing; return; }";
    assert_eq!(diagnosed(text)[0].code, "E0200");
}

#[test]
fn heldout_utf8_crlf_borrow_and_diagnostic_spans_do_not_follow_display_truncation() {
    let text="/* 🦀 */\r\nstruct Packet { count: i32 }\r\nfn read(p: &Packet) -> () { return; }\r\nfn test(p: &Packet) -> () { read(& /* 雪 */ * /* é */ p); return; }";
    with_source(text, |source| {
        let ast = parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::OwnedCandidate,
        )
        .unwrap();
        let typed = typeck::check(resolve::resolve(source, &ast).unwrap()).unwrap();
        let test = typed.functions().nth(1).unwrap();
        let args = test
            .hir()
            .expressions
            .iter()
            .find_map(|e| {
                if let ExprKind::Call { args, .. } = &e.kind {
                    Some(args)
                } else {
                    None
                }
            })
            .unwrap();
        let Argument::Borrow {
            span,
            name_span,
            star_span,
            ..
        } = args[0]
        else {
            panic!()
        };
        assert_eq!(slice(text, span), "& /* 雪 */ * /* é */ p");
        assert_eq!(slice(text, name_span), "p");
        assert_eq!(slice(text, star_span.unwrap()), "*");
        assert_eq!(source.location(span.start).0, 4);
    });
    let long = "q".repeat(4096);
    let text = format!("// 🦀\r\nstruct Packet {{}} fn test() -> () {{ {long}; return; }}");
    let d = diagnosed(&text);
    assert_eq!(
        d[0].primary.unwrap().end - d[0].primary.unwrap().start,
        4096
    );
    assert_eq!(slice(&text, d[0].primary.unwrap()), long);
    assert!(d[0].message.len() < 100);
    assert!(d[0].message.contains(&format!("{}...", "q".repeat(61))));
}

#[test]
fn heldout_arity_limit_is_inclusive_for_mixed_argument_kinds() {
    for count in [255, 256, 257] {
        let args = (0..count)
            .map(|i| if i % 2 == 0 { "&p" } else { "0" })
            .collect::<Vec<_>>()
            .join(",");
        let result = parsed(&format!("fn test() -> () {{ take({args}); return; }}"));
        assert_eq!(result.is_ok(), count <= 256);
        if let Err(d) = result {
            assert_eq!(d[0].code, "E0400");
        }
        let params = (0..count)
            .map(|i| format!("p{i}: {}", if i % 2 == 0 { "&Packet" } else { "i32" }))
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            parsed(&format!("fn test({params}) -> () {{ return; }}")).is_ok(),
            count <= 256
        );
    }
}

#[test]
fn heldout_total_tree_height_counts_literals_groups_and_borrow_leaves() {
    for leaf in ["Packet {}", "p.count", "take(&mut *p)"] {
        let leaf_height = if leaf.starts_with("take") { 2 } else { 1 };
        for height in [63, 64, 65] {
            let n = height - leaf_height;
            let expression = "(".repeat(n) + leaf + &")".repeat(n);
            let result = parsed(&format!("fn test() -> () {{ {expression}; return; }}"));
            assert_eq!(result.is_ok(), height <= 64, "{leaf}: {height}");
        }
    }
    for n in [62, 63, 64] {
        let expr = "!".repeat(n) + "take(&p)";
        assert_eq!(
            parsed(&format!("fn test() -> () {{ {expr}; return; }}")).is_ok(),
            n + 2 <= 64
        );
    }
}

#[test]
fn heldout_block_depth_limit_includes_the_function_body() {
    for n in [62, 63, 64] {
        let text = "struct Packet {} fn test() -> () { ".to_string()
            + &"if true { ".repeat(n)
            + &"} ".repeat(n)
            + "return; }";
        assert_eq!(parsed(&text).is_ok(), n < 64);
    }
}

#[test]
fn heldout_node_counts_are_exact_for_mixed_constructs() {
    // Record+field=2; function+parameter=2; let+literal+field+number=4;
    // field assignment+number=2; call statement+call+borrow=3; return=1.
    let text="struct Packet { n: i32 } fn test(p: &Packet) -> () { let mut q = Packet { n: 1 }; q.n = 2; test(&q); return; }";
    with_source(text, |source| {
        for limit in [13, 14] {
            let result = parser::parse_with_lowered_node_limit(
                source,
                lexer::lex(source).unwrap(),
                parser::SourceMode::OwnedCandidate,
                limit,
            );
            assert_eq!(result.is_ok(), limit == 14);
            if let Err(d) = result {
                assert_eq!(d[0].code, "E0400");
            }
        }
    });
}

#[test]
fn heldout_lexer_token_and_token_byte_ceilings_are_inclusive() {
    for count in [99_999, 100_000, 100_001] {
        with_source(&";".repeat(count), |source| {
            let result = lexer::lex(source);
            assert_eq!(result.is_ok(), count <= 100_000);
            if let Ok(tokens) = result {
                assert_eq!(tokens.len(), count + 1);
            }
        });
    }
    for count in [65_535, 65_536, 65_537] {
        with_source(&"a".repeat(count), |source| {
            assert_eq!(lexer::lex(source).is_ok(), count <= 65_536);
        });
    }
}

#[test]
fn heldout_record_count_and_field_count_ceilings() {
    for count in [4096, 4097] {
        let text = (0..count)
            .map(|i| format!("struct Packet{i} {{}}\n"))
            .collect::<String>();
        with_source(&text, |source| {
            let ast = parser::parse_with_mode(
                source,
                lexer::lex(source).unwrap(),
                parser::SourceMode::OwnedCandidate,
            )
            .unwrap();
            let result = resolve::resolve(source, &ast);
            assert_eq!(result.is_ok(), count == 4096);
            if let Err(d) = result {
                assert_eq!(d[0].code, "E0400");
            }
        });
    }
    for count in [1024, 1025] {
        let fields = (0..count)
            .map(|i| format!("f{i}: ()"))
            .collect::<Vec<_>>()
            .join(",");
        let text = format!("struct Packet {{ {fields} }}");
        with_source(&text, |source| {
            let ast = parser::parse_with_mode(
                source,
                lexer::lex(source).unwrap(),
                parser::SourceMode::OwnedCandidate,
            )
            .unwrap();
            assert_eq!(resolve::resolve(source, &ast).is_ok(), count == 1024);
        });
    }
}

#[test]
fn heldout_diagnostic_100_cap_stops_before_101st_formatting_in_each_pass() {
    for kind in [0, 1, 2] {
        let mut measured = Vec::new();
        for count in [100, 101] {
            let text = match kind {
                0 => "struct 0 ".repeat(count),
                1 => "struct Packet {} ".repeat(count + 1),
                _ => {
                    "struct Packet { value: i32 } ".to_string()
                        + &(0..count)
                            .map(|i| format!("fn task{i}() -> Packet {{ return Packet {{}}; }}"))
                            .collect::<String>()
                }
            };
            let (errors, metrics) = display::measure_formatting(|| {
                if kind == 0 {
                    parsed(&text).unwrap_err()
                } else {
                    diagnosed(&text)
                }
            });
            assert_eq!(errors.len(), 100, "pass {kind}, inputs {count}");
            measured.push((
                metrics.allocations,
                metrics.copied_bytes,
                metrics.displayed_names,
                metrics.name_bytes,
            ));
        }
        assert_eq!(
            measured[0], measured[1],
            "pass {kind} constructed the 101st diagnostic"
        );
    }
}

#[test]
fn heldout_original_large_schema_fixture_has_bounded_text_with_full_origins() {
    // Regenerate the independent design-review fixture byte for byte so this
    // regression remains portable when integrated into the repository.
    let fields = (0..1024)
        .map(|i| format!("f{i:04}_{}: i32", "x".repeat(894)))
        .collect::<Vec<_>>()
        .join(",\n");
    let functions = (0..100)
        .map(|i| format!("fn f{i}() -> () {{ T {{}}; return; }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let text = format!("struct T {{\n{fields}\n}}\n{functions}");
    assert_eq!(text.len(), 932_069);
    let (errors, stats) = display::measure_formatting(|| diagnosed(&text));
    assert_eq!(errors.len(), 100);
    assert_eq!(stats.displayed_names, 800);
    assert_eq!(stats.name_bytes, 51_200);
    assert_eq!(stats.copied_bytes, 56_100);
    for d in errors {
        assert_eq!(d.message.len(), 561);
        assert_eq!(slice(&text, d.primary.unwrap()), "T {}");
        assert!(d.message.ends_with("1016 more omitted"));
    }
}

#[test]
fn heldout_formatter_utf8_component_segmentation_is_irrelevant() {
    for unit in ["a", "é", "雪", "🦀"] {
        for split in [0, 1, 61, 255, 1020, 1024, 1025] {
            let text = unit.repeat(1100);
            let offset = (split.min(text.len())..=text.len())
                .find(|&n| text.is_char_boundary(n))
                .unwrap();
            let (a, b) = text.split_at(offset);
            let actual = display::diagnostic("E0300", "type", format_args!("{a}{b}"), None);
            let expected = display::diagnostic("E0300", "type", format_args!("{text}"), None);
            assert_eq!(actual.message, expected.message, "{unit}: {split}");
            assert!(actual.message.len() <= 1024);
            assert!(actual.message.ends_with("..."));
        }
    }
}

#[test]
fn heldout_scalar_ast_and_diagnostics_match_candidate_mode_without_owned_markers() {
    let scalar="/* struct Packet{} &mut x.y */ fn helper(as: bool,n: i32) -> bool { let mut k = n; while k < 3 { k = k + 1; if as { continue; } else { break; } } return !as || k == 4; }";
    for scalar in [scalar, "fn f() -> () { -x; return; }"] {
        with_source(scalar, |source| {
            let old = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
            let candidate = parser::parse_with_mode(
                source,
                lexer::lex(source).unwrap(),
                parser::SourceMode::OwnedCandidate,
            )
            .unwrap();
            assert!(!candidate.uses_owned_syntax(source));
            assert_eq!(format!("{old:?}"), format!("{candidate:?}"));
        });
    }
    for text in [
        "fn f() -> () { 1.2; return; }",
        "fn f() -> () { +x; return; }",
        "fn f() -> () { true < false < true; return; }",
    ] {
        with_source(text, |source| {
            let old = parser::parse(source, lexer::lex(source).unwrap()).unwrap_err();
            let new = parser::parse_with_mode(
                source,
                lexer::lex(source).unwrap(),
                parser::SourceMode::OwnedCandidate,
            )
            .unwrap_err();
            assert_eq!(old[0].code, new[0].code);
            assert_eq!(old[0].message, new[0].message);
            assert_eq!(old[0].primary, new[0].primary);
        });
    }
}

#[test]
fn heldout_shared_count_admission_checks_aggregate_fields_without_certifying_layout() {
    use crate::frontend::oir::owned_types::admit_declaration_counts;
    for count in [65_535, 65_536, 65_537] {
        let mut counts = vec![1024; count / 1024];
        if count % 1024 != 0 {
            counts.push(count % 1024);
        }
        let result = admit_declaration_counts(counts.into_iter());
        assert_eq!(result.is_ok(), count <= 65_536);
        if let Ok(usage) = result {
            assert_eq!(usage.fields, count);
            assert_eq!(usage.layout_bytes, 0);
        }
    }
}

#[test]
fn heldout_long_scalar_rejection_preserves_shared_formatting_after_activation() {
    // Shared old-scalar token diagnostics have their own source/token bounds.
    // Activation preserves both production rendering and the explicit old mode.
    let literal = format!("\"{}\"", "q".repeat(4096));
    let text = format!("fn f() -> () {{ {literal}; return; }}");
    with_source(&text, |source| {
        let old = parser::parse(source, lexer::lex(source).unwrap()).unwrap_err();
        let candidate = parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::OwnedCandidate,
        )
        .unwrap_err();
        assert_eq!(old[0].code, "E0101");
        assert_eq!(
            old[0].message,
            format!("unsupported typed-preview construct `{literal}`")
        );
        assert!(old[0].message.len() > 4096);
        assert_eq!(candidate[0].message, old[0].message);
        let scalar = parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::ScalarOnly,
        )
        .unwrap_err();
        assert_eq!(scalar[0].message, old[0].message);
        assert_eq!(scalar[0].primary, old[0].primary);
        assert_eq!(old[0].primary, candidate[0].primary);
        assert_eq!(slice(&text, old[0].primary.unwrap()), literal);
    });
}
