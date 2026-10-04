use super::{lexer, parser, source::SourceMap};

#[test]
fn candidate_parses_scalar_field_and_empty_records() {
    let mut sources = SourceMap::new();
    let id = sources.add("owned.ox".into(), "struct Empty {} struct Cell { value: i32, flag: bool, empty: (), } fn relay(x: Cell) -> Cell { return x; } fn main() -> () { let mut x = Cell { empty: (), flag: true, value: 1, }; x.value = 2; relay(x); return; }".into());
    let source = sources.get(id);
    assert!(parser::parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate
    )
    .is_ok());
}

#[test]
fn candidate_parses_explicit_borrow_and_reborrow_arguments() {
    let mut sources = SourceMap::new();
    let id = sources.add("owned.ox".into(), "struct Cell {} fn read(p: &Cell) -> () { return; } fn relay(p: &mut Cell) -> () { read(&*p); return; } fn main() -> () { let mut x = Cell {}; read(&x); relay(&mut x); return; }".into());
    let source = sources.get(id);
    assert!(parser::parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate
    )
    .is_ok());
}

fn parsed(
    text: &str,
    mode: parser::SourceMode,
) -> Result<super::ast::Program, Vec<super::diagnostic::Diagnostic>> {
    let mut sources = SourceMap::new();
    let id = sources.add("é-雪.ox".into(), text.into());
    let source = sources.get(id);
    parser::parse_with_mode(source, lexer::lex(source).unwrap(), mode)
}
#[test]
fn grammar_rejects_reference_storage_unsupported_roots_and_partial_borrows() {
    for text in [
        "struct C { p: &C }",
        "fn f() -> &C { return; }",
        "fn f() -> () { let x: &C = 0; return; }",
        "fn f() -> () { let x = &p; return; }",
        "fn f() -> () { (&p); return; }",
        "fn f() -> () { f((&p)); return; }",
        "fn f() -> () { f(&(p)); return; }",
        "fn f() -> () { f(&p.field); return; }",
        "fn f() -> () { f(&C {}); return; }",
        "fn f() -> () { f(&make()); return; }",
        "fn f() -> () { f(&*p.field); return; }",
        "fn f() -> () { f(&p + 1); return; }",
        "fn f() -> () { (p).field; return; }",
        "fn f() -> () { (*p).field; return; }",
        "fn f() -> () { make().field; return; }",
        "fn f() -> () { (p.field).next; return; }",
        "fn f() -> () { p.method(); return; }",
        "fn f() -> () { *p = 1; return; }",
        "fn f() -> () { C { field }; return; }",
        "fn f() -> () { C { ..base }; return; }",
        "fn f(x: i32,) -> () { return; }",
        "fn f() -> () { f(1,); return; }",
        "fn f() -> () { f(&p,); return; }",
        "fn f(mut p: C) -> () { return; }",
        "fn f(p: &&C) -> () { return; }",
        "fn f(p: &mut &C) -> () { return; }",
    ] {
        assert!(
            parsed(text, parser::SourceMode::OwnedCandidate).is_err(),
            "{text}"
        );
    }
}
#[test]
fn scalar_mode_keeps_original_negative_boundary_diagnostics() {
    for (text, token, code) in [
        ("struct C {}", "struct", "E0101"),
        ("fn f(p: &bool) -> () { return; }", "&", "E0101"),
        ("fn f() -> () { p.field; return; }", ".", "E0101"),
        ("fn f() -> () { f(&p); return; }", "&", "E0101"),
        ("fn f() -> () { C {}; return; }", "{", "E0100"),
    ] {
        let errors = parsed(text, parser::SourceMode::ScalarOnly).unwrap_err();
        assert_eq!(errors[0].code, code);
        let span = errors[0].primary.unwrap();
        assert_eq!(&text[span.start..span.end], token);
        if code == "E0101" {
            assert_eq!(
                errors[0].message,
                format!("unsupported typed-preview construct `{token}`")
            );
        }
    }
}
#[test]
fn condition_root_propagates_through_unary_and_binary_operators() {
    for condition in [
        "flag",
        "!flag",
        "true && flag",
        "false || !flag",
        "1 + n < 2",
        "n * 2 == 4",
    ] {
        let text = format!("fn f() -> () {{ if {condition} {{ return; }} while {condition} {{ break; }} return; }}");
        parsed(&text, parser::SourceMode::OwnedCandidate).unwrap();
    }
    for condition in [
        "C {}",
        "!C {}",
        "true && C {}",
        "false || C {}",
        "1 == C {}",
        "1 + C {}",
    ] {
        let text = format!("fn f() -> () {{ if {condition} {{ return; }} return; }}");
        assert!(
            parsed(&text, parser::SourceMode::OwnedCandidate).is_err(),
            "{condition}"
        );
    }
    for condition in [
        "(C {})",
        "use_cell(C {})",
        "true && use_cell(C {})",
        "!use_cell(C {})",
    ] {
        parsed(
            &format!("fn f() -> () {{ if {condition} {{ return; }} return; }}"),
            parser::SourceMode::OwnedCandidate,
        )
        .unwrap();
    }
}
#[test]
fn exact_spans_keep_comments_crlf_field_order_and_borrow_operators() {
    use super::ast::*;
    let text = "// é雪\r\nstruct C { x: i32, y: bool, } fn f(p: & /* α */ mut C) -> () { let mut c = C { y: true, x: 1 }; c /* β */ . x = 2; c.y; f(& /* γ */ mut * /* δ */ p); return; }";
    let ast = parsed(text, parser::SourceMode::OwnedCandidate).unwrap();
    let slice = |span: super::source::Span| &text[span.start..span.end];
    assert_eq!(slice(ast.records[0].span), "struct C { x: i32, y: bool, }");
    assert_eq!(slice(ast.records[0].end), "}");
    assert_eq!(slice(ast.functions[0].params[0].ty.span), "& /* α */ mut C");
    let body = &ast.functions[0].blocks[0].body;
    let StmtKind::FieldAssign {
        target_span,
        operator_span,
        ..
    } = body[1].kind
    else {
        panic!()
    };
    assert_eq!(slice(target_span), "c /* β */ . x");
    assert_eq!(slice(operator_span), "=");
    assert_eq!(slice(body[1].span), "c /* β */ . x = 2;");
    let literal = ast
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
        literal
            .iter()
            .map(|field| slice(field.name))
            .collect::<Vec<_>>(),
        ["y", "x"]
    );
    let call = ast
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
        place: BorrowPlace::ForwardedParameter { name, star_span },
        mutable,
    } = call[0]
    else {
        panic!()
    };
    assert!(mutable);
    assert_eq!(slice(span), "& /* γ */ mut * /* δ */ p");
    assert_eq!(slice(name), "p");
    assert_eq!(slice(star_span), "*");
}
#[test]
fn owned_routing_is_syntax_based_and_scalar_ast_is_mode_identical() {
    for text in [
        "fn f(x: bool, n: i32, u: ( /* unit */ )) -> () { let mut a: bool = x && n < 1; if a { a = false; } while a { break; } return; }",
        "// struct C {} &mut field.x\nfn main() -> i32 { return 1; }",
    ] {
        let scalar = parsed(text, parser::SourceMode::ScalarOnly).unwrap();
        let candidate = parsed(text, parser::SourceMode::OwnedCandidate).unwrap();
        assert_eq!(format!("{scalar:?}"), format!("{candidate:?}"));
        let mut sources = SourceMap::new(); let id = sources.add("x".into(), text.into());
        assert!(!candidate.uses_owned_syntax(sources.get(id)));
    }
    for text in [
        "struct C {}",
        "fn f() -> Unknown { return; }",
        "fn f(p: &Unknown) -> () { return; }",
        "fn f() -> () { let x: Unknown = 0; return; }",
        "fn f() -> () { if false { C {}; } return; }",
        "fn f() -> bool { return false && p.field; }",
        "fn f() -> () { f(&p); return; }",
    ] {
        let ast = parsed(text, parser::SourceMode::OwnedCandidate).unwrap();
        let mut sources = SourceMap::new();
        let id = sources.add("x".into(), text.into());
        assert!(ast.uses_owned_syntax(sources.get(id)), "{text}");
    }
}
#[test]
fn expression_nesting_includes_borrow_leaves_and_literal_fields() {
    for leaf in ["x", "&x", "&*p", "&mut x", "&mut *p"] {
        for count in [63, 64] {
            let expression = "f(".repeat(count) + leaf + &")".repeat(count);
            let text = format!("fn f() -> () {{ {expression}; return; }}");
            let result = parsed(&text, parser::SourceMode::OwnedCandidate);
            assert_eq!(result.is_ok(), count == 63, "count={count} leaf={leaf}");
            if let Err(errors) = result {
                assert_eq!(errors[0].code, "E0400");
            }
        }
    }
    for count in [63, 64] {
        let expression = "C { x: ".repeat(count) + "1" + &" }".repeat(count);
        let text = format!("fn f() -> () {{ {expression}; return; }}");
        assert_eq!(
            parsed(&text, parser::SourceMode::OwnedCandidate).is_ok(),
            count == 63
        );
    }
}
#[test]
fn recovery_at_failed_function_or_record_always_makes_progress() {
    for text in [
        "fn fn fn",
        "struct struct struct",
        "struct C { fn good() -> () { return; }",
        "fn f() -> () { struct C {}",
    ] {
        let errors = parsed(text, parser::SourceMode::OwnedCandidate).unwrap_err();
        assert!(!errors.is_empty() && errors.len() <= parser::MAX_DIAGNOSTICS);
    }
}
#[test]
fn syntax_node_charges_preserve_scalar_counts_and_charge_new_leaves() {
    // Independent hand counts: function + parameter + statements + expressions;
    // records, record fields, literal fields and borrow arguments each add one.
    for (text, nodes) in [
        ("fn f() -> () { return; }", 2),
        ("fn f(x: i32) -> i32 { let y = x + 1; return y; }", 8),
        ("struct C {}", 1),
        ("struct C { a: i32, b: bool }", 3),
        ("fn f() -> () { C { a: 1, b: true }; return; }", 8),
        ("fn f() -> () { f(&p); return; }", 5),
        ("fn f() -> () { p.x; return; }", 4),
        ("fn f() -> () { p.x = 1; return; }", 4),
    ] {
        for limit in [nodes - 1, nodes] {
            let mut sources = SourceMap::new();
            let id = sources.add("x".into(), text.into());
            let source = sources.get(id);
            let result = parser::parse_with_lowered_node_limit(
                source,
                lexer::lex(source).unwrap(),
                parser::SourceMode::OwnedCandidate,
                limit,
            );
            assert_eq!(
                result.is_ok(),
                limit == nodes,
                "limit={limit}, nodes={nodes}, {text}"
            );
            if let Err(errors) = result {
                assert_eq!(errors[0].code, "E0400");
            }
        }
    }
}
#[test]
fn borrow_arguments_and_reference_parameters_keep_256_limit() {
    for count in [256, 257] {
        let params = (0..count)
            .map(|i| format!("p{i}: &C"))
            .collect::<Vec<_>>()
            .join(",");
        let args = vec!["&x"; count].join(",");
        for text in [
            format!("fn f({params}) -> () {{ return; }}"),
            format!("fn f() -> () {{ f({args}); return; }}"),
        ] {
            let result = parsed(&text, parser::SourceMode::OwnedCandidate);
            assert_eq!(result.is_ok(), count == 256);
            if let Err(errors) = result {
                assert_eq!(errors[0].code, "E0400");
            }
        }
    }
}
