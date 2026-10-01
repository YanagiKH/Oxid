use super::*;
use crate::frontend::{hir, lexer, parser, source::SourceMap};

fn pipeline(text: &str) -> Result<TypedProgram, Vec<Diagnostic>> {
    let mut sources = SourceMap::new();
    let id = sources.add("branches.ox".into(), text.into());
    let source = sources.get(id);
    let tokens = lexer::lex(source).map_err(|error| vec![*error])?;
    let ast = parser::parse(source, tokens)?;
    check(hir::resolve(source, &ast)?)
}

fn assert_error(text: &str, code: &str, marked: &str) -> Diagnostic {
    let diagnostics = pipeline(text).unwrap_err();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let error = diagnostics.into_iter().next().unwrap();
    assert_eq!(error.code, code, "{error:?}");
    let span = error.primary.unwrap();
    assert_eq!(&text[span.start..span.end], marked, "{error:?}");
    error
}

#[test]
fn boolean_branches_accept_both_returns_early_returns_and_empty_arms() {
    for text in [
        "fn f(flag: bool) -> bool { if flag { return true; } else { return false; } }",
        "fn f(flag: bool) -> bool { if flag { return true; } return false; }",
        "fn f(flag: bool) -> () { if flag {} else {} return; }",
        "fn f(flag: bool) -> () { if flag { return; } else {} return; }",
        "fn f(flag: bool) -> () { if flag {} else { return; } return; }",
        "fn f(flag: bool) -> () { if flag { if flag { return; } else { return; } } else { return; } }",
        "fn f(flag: bool) -> () { if flag { if flag { return; } } return; }",
    ] {
        assert!(pipeline(text).is_ok(), "{text}: {:?}", pipeline(text));
    }
}

#[test]
fn boolean_branch_scopes_allow_sibling_and_closed_child_reuse() {
    let text = "fn f(flag: bool) -> () { if flag { let item = flag; item; } else { let item = (); item; } let item = true; item; return; }";
    let typed = pipeline(text).unwrap();
    let view = typed.functions().next().unwrap();
    assert_eq!(
        (0..4)
            .map(|id| view.local_ty(LocalId(id)))
            .collect::<Vec<_>>(),
        [Ty::Bool, Ty::Bool, Ty::Unit, Ty::Bool]
    );
    assert_eq!(
        format!("{typed:?}"),
        format!("{:?}", pipeline(text).unwrap())
    );
}

#[test]
fn boolean_branch_local_names_cannot_escape_or_cross_siblings() {
    assert_error(
        "fn f() -> () { if true { let child = (); } child; return; }",
        "E0200",
        "child",
    );
    assert_error(
        "fn f() -> () { if true { let child = (); } else { child; } return; }",
        "E0200",
        "child",
    );
    assert_error(
        "fn f() -> () { if true { let child = child; } return; }",
        "E0200",
        "child",
    );
}

#[test]
fn boolean_branch_active_ancestor_and_function_shadowing_report_original_name() {
    for (text, first) in [
        (
            "fn f(arg: bool) -> () { if true { let arg = (); } return; }",
            "arg:",
        ),
        (
            "fn f() -> () { let outer = (); if true { let outer = (); } return; }",
            "outer =",
        ),
        (
            "fn f() -> () { if true { let same = (); let same = (); } return; }",
            "same =",
        ),
        ("fn f() -> () { if true { let f = (); } return; }", "f()"),
    ] {
        let error = pipeline(text).unwrap_err().remove(0);
        assert_eq!(error.code, "E0201", "{error:?}");
        assert_eq!(error.secondary.len(), 1);
        assert_eq!(error.secondary[0].0.start, text.find(first).unwrap());
    }
}

#[test]
fn boolean_branch_conditions_require_bool_and_literal_arms_are_both_checked() {
    assert_error("fn f() -> () { if (()) {} return; }", "E0300", "(())");
    assert_error(
        "fn f() -> bool { if true { return true; } else { return (); } }",
        "E0300",
        "()",
    );
    assert_error(
        "fn f() -> bool { if false { return (); } else { return true; } }",
        "E0300",
        "()",
    );
}

#[test]
fn boolean_branch_missing_paths_still_require_explicit_terminal_return() {
    for text in [
        "fn f() -> bool { if true { return true; } }",
        "fn f() -> () { if true {} else {} }",
        "fn f() -> () { if true { if false { return; } } else { return; } }",
    ] {
        let error = assert_error(text, "E0302", "}");
        assert_eq!(error.primary.unwrap().start, text.len() - 1);
        assert_eq!(
            error.message,
            "function requires an explicit terminal return"
        );
    }
}

#[test]
fn boolean_branch_terminal_flow_rejects_following_statement_in_its_scope() {
    assert_error(
        "fn f() -> () { if true { return; } else { return; } true; }",
        "E0303",
        "true;",
    );
    assert_error(
        "fn f() -> () { if true { return; false; } return; }",
        "E0303",
        "false;",
    );
    assert_error(
        "fn f() -> () { if true { if true { return; } else { return; } (); } return; }",
        "E0303",
        "();",
    );
}

fn nested_source(branches: usize, expression_groups: usize) -> String {
    format!(
        "fn f() -> () {{ {}{}true{}; {} return; }}",
        "if true { ".repeat(branches),
        "(".repeat(expression_groups),
        ")".repeat(expression_groups),
        "} ".repeat(branches)
    )
}

#[test]
fn boolean_statement_nesting_includes_function_body_and_is_separate_from_expressions() {
    assert!(pipeline(&nested_source(63, 63)).is_ok());
    let text = nested_source(64, 0);
    let error = assert_error(&text, "E0400", "{");
    assert_eq!(error.message, "statement block nesting limit exceeded");
    let error = assert_error(&nested_source(63, 64), "E0400", "true");
    assert_eq!(error.message, "expression nesting limit exceeded");
}

#[test]
fn boolean_branch_grammar_rejects_else_if_if_expressions_naked_blocks_and_trailing_semicolon() {
    for text in [
        "fn f() -> () { if true {} else if false {} return; }",
        "fn f() -> () { let value = if true {} else {}; return; }",
        "fn f() -> () { { return; } }",
        "fn f() -> () { if true {}; return; }",
        "fn f() -> () { if true {} else return; }",
    ] {
        assert!(pipeline(text).is_err(), "{text}");
    }
}

#[test]
fn boolean_branch_parse_recovery_reaches_later_functions_and_diagnostic_cap() {
    let text = "fn first() -> () { if true { let x = ; } } fn second() -> () { if true {} else return; } fn last() -> () { return; }";
    let errors = pipeline(text).unwrap_err();
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert_eq!(
        errors[1].primary.unwrap().start,
        text.find("return;").unwrap()
    );
    let text = (0..120)
        .map(|index| format!("fn bad{index}() -> () {{ if true {{ let x = ; }} }} "))
        .collect::<String>();
    assert_eq!(pipeline(&text).unwrap_err().len(), parser::MAX_DIAGNOSTICS);
    assert_error("fn f() -> () { if true {", "E0100", "");
}

#[test]
fn boolean_expression_and_local_ids_follow_depth_first_source_order() {
    let text = "fn f() -> () { if pred(true) { let item = yes(false); item; } else { let item = no(()); item; } after(); return; } fn pred(x: bool) -> bool { return x; } fn yes(x: bool) -> bool { return x; } fn no(x: ()) -> () { return x; } fn after() -> () { return; }";
    let typed = pipeline(text).unwrap();
    let view = typed.functions().next().unwrap();
    let expressions = &view.hir().expressions;
    assert_eq!(
        expressions
            .iter()
            .map(|expr| &text[expr.span.start..expr.span.end])
            .collect::<Vec<_>>(),
        [
            "true",
            "pred(true)",
            "false",
            "yes(false)",
            "item",
            "()",
            "no(())",
            "item",
            "after()"
        ]
    );
    assert!(matches!(expressions[4].kind, ExprKind::Local(LocalId(0))));
    assert!(matches!(expressions[7].kind, ExprKind::Local(LocalId(1))));
    assert_eq!(view.local_ty(LocalId(0)), Ty::Bool);
    assert_eq!(view.local_ty(LocalId(1)), Ty::Unit);
}

#[test]
fn boolean_block_spans_and_complete_flow_tables_cover_every_arm() {
    let text = "// 雪\r\nfn f(flag: bool) -> () { if flag { if flag { return; } else { return; } } else {} return; }";
    let typed = pipeline(text).unwrap();
    let view = typed.functions().next().unwrap();
    let function = view.hir();
    assert_eq!(function.body, BodyBlockId(0));
    assert_eq!(function.blocks.len(), 5);
    assert_eq!(
        (0..5)
            .map(|id| view.block_flow(BodyBlockId(id)).returns_only())
            .collect::<Vec<_>>(),
        [true, true, true, true, false]
    );
    let root = &function.blocks[0];
    assert_eq!(
        &text[root.span.start..root.span.end],
        "{ if flag { if flag { return; } else { return; } } else {} return; }"
    );
    assert_eq!(root.end, function.end);
    for block in &function.blocks {
        assert_eq!(&text[block.span.start..block.span.start + 1], "{");
        assert_eq!(&text[block.end.start..block.end.end], "}");
        assert_eq!(block.span.end, block.end.end);
    }
    assert_eq!(
        &text[root.body[0].span.start..root.body[0].span.end],
        "if flag { if flag { return; } else { return; } } else {}"
    );
    assert!(matches!(
        root.body[0].kind,
        StmtKind::If {
            then_block: BodyBlockId(1),
            else_block: Some(BodyBlockId(4)),
            ..
        }
    ));
    assert!(matches!(
        function.blocks[1].body[0].kind,
        StmtKind::If {
            then_block: BodyBlockId(2),
            else_block: Some(BodyBlockId(3)),
            ..
        }
    ));
}
