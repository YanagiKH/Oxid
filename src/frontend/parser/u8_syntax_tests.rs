//! RFC0030 grammar and actual bounded parser controls; admission review is separate.
use super::*;
use crate::frontend::{
    lexer,
    source::{SourceFileId, SourceMap},
};
use std::mem::{align_of, size_of};

fn source(text: &str) -> SourceMap {
    let mut map = SourceMap::new();
    map.add("u8-syntax.ox".into(), text.into());
    map
}
fn typed(
    map: &SourceMap,
    mode: SourceMode,
    limit: usize,
    allocator: &mut Allocator,
    storage: &mut SyntaxStorage,
) -> Result<(Program, usize), Vec<Diagnostic>> {
    let file = map.get(SourceFileId(0));
    parse_typed_counted(
        file,
        lexer::lex(file).unwrap(),
        mode,
        limit,
        allocator,
        storage,
    )
}
fn parsed(map: &SourceMap) -> Program {
    typed(
        map,
        SourceMode::ProjectCandidate,
        MAX_NODES,
        &mut Allocator::default(),
        &mut SyntaxStorage::default(),
    )
    .unwrap()
    .0
}

#[test]
fn u8_named_conversion_has_a_real_child_and_exact_trivia_origins_on_every_mode() {
    let map = source("// 雪\r\nfn f(x:i32,b:u8)->i32{return (x /*é*/ . /*dot*/ to_u8_checked /*name*/ ( /*empty*/ )).to_i32();}");
    // The outer group deliberately cannot become another receiver.
    let file = map.get(SourceFileId(0));
    let errors = typed(
        &map,
        SourceMode::ProjectCandidate,
        MAX_NODES,
        &mut Allocator::default(),
        &mut SyntaxStorage::default(),
    )
    .unwrap_err();
    assert_eq!(
        errors[0].primary,
        Some(file.span(
            file.text().rfind('.').unwrap(),
            file.text().rfind('.').unwrap() + 1
        ))
    );
    let text = "// 雪\r\nfn f(x:i32,b:u8)->i32{x /*é*/ . /*dot*/ to_u8_checked /*name*/ ( /*empty*/ );return (b.to_i32());}";
    let map = source(text);
    let file = map.get(SourceFileId(0));
    for mode in [
        SourceMode::ScalarOnly,
        SourceMode::OwnedCandidate,
        SourceMode::ModuleCandidate,
        SourceMode::ProjectCandidate,
    ] {
        let ast = typed(
            &map,
            mode,
            MAX_NODES,
            &mut Allocator::default(),
            &mut SyntaxStorage::default(),
        )
        .unwrap()
        .0;
        assert!(ast.belongs_to(file));
        assert!(ast.validate_spans_and_ids(|span| file.try_text(span).is_some()));
        let conversions: Vec<_> = ast
            .expressions
            .iter()
            .enumerate()
            .filter_map(|(index, expr)| {
                if let ExprKind::Conversion {
                    op,
                    operand,
                    name_span,
                } = expr.kind
                {
                    Some((index, op, operand, name_span, expr.span))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(conversions.len(), 2);
        for ((index, op, operand, name_span, whole), (want_op, name, receiver)) in
            conversions.into_iter().zip([
                (ConversionOp::ToU8Checked, "to_u8_checked", "x"),
                (ConversionOp::ToI32, "to_i32", "b"),
            ])
        {
            assert_eq!(op, want_op);
            assert!(operand.0 < index);
            let child = &ast.expressions[operand.0];
            assert!(matches!(child.kind, ExprKind::Name(span) if span == child.span));
            assert_eq!(file.text_at(child.span), receiver);
            assert_eq!(file.text_at(name_span), name);
            assert_eq!(whole.start, child.span.start);
            assert!(file.text_at(whole).ends_with(')'));
        }
        assert_eq!(
            file.text_at(ast.expressions[1].span),
            "x /*é*/ . /*dot*/ to_u8_checked /*name*/ ( /*empty*/ )"
        );
    }
}

#[test]
fn u8_argument_missing_close_and_excluded_receiver_diagnostics_pin_tokens() {
    for (expression, origin) in [
        ("x.to_i32(1)", "1"),
        ("x.to_i32(y)", "y"),
        ("x.to_u8_checked(true)", "true"),
        ("x.to_i32(())", "("),
        ("x.to_i32(&x)", "&"),
        ("x.to_i32(,)", ","),
        ("(x).to_i32()", "."),
        ("(x+1).to_u8_checked()", "."),
        ("f().to_u8_checked()", "."),
        ("255.to_u8_checked()", "."),
        ("x.to_i32().to_u8_checked()", ".to_u8_checked"),
        ("x.to_i32()()", "()("),
        ("r.field.to_i32()", ".to_i32"),
        ("crate::m::x.to_i32()", "."),
    ] {
        let text = format!("fn f()->i32{{return {expression};}}");
        let map = source(&text);
        let file = map.get(SourceFileId(0));
        let errors = typed(
            &map,
            SourceMode::ProjectCandidate,
            MAX_NODES,
            &mut Allocator::default(),
            &mut SyntaxStorage::default(),
        )
        .unwrap_err();
        assert_eq!(errors.len(), 1, "{expression}: {errors:?}");
        assert!(errors[0].secondary.is_empty() && errors[0].notes.is_empty());
        let offset = text.find(expression).unwrap()
            + if origin == "(" && expression == "x.to_i32(())" {
                expression.find("((").unwrap() + 1
            } else if origin == "()(" {
                expression.find(origin).unwrap() + 2
            } else {
                expression.find(origin).unwrap()
            };
        let length = if origin.starts_with('.') || origin == "()(" {
            1
        } else {
            origin.len()
        };
        assert_eq!(
            (errors[0].code, errors[0].stage),
            ("E0101", "parse"),
            "{expression}: {errors:?}"
        );
        assert_eq!(
            errors[0].primary,
            Some(file.span(offset, offset + length)),
            "{expression}: {errors:?}"
        );
        assert_eq!(
            errors[0].message,
            format!(
                "unsupported typed-preview construct `{}`",
                &text[offset..offset + length]
            )
        );
    }
    for text in [
        "fn f()->i32{return x.to_i32(",
        "fn f()->i32{return x.to_i32(;}",
        "fn f()->i32{return x.to_i32(}",
        "fn f()->i32{return x.to_i32(]}",
    ] {
        let map = source(text);
        let file = map.get(SourceFileId(0));
        let errors = typed(
            &map,
            SourceMode::ProjectCandidate,
            MAX_NODES,
            &mut Allocator::default(),
            &mut SyntaxStorage::default(),
        )
        .unwrap_err();
        assert_eq!(errors.len(), 1, "{text}: {errors:?}");
        assert!(errors[0].secondary.is_empty() && errors[0].notes.is_empty());
        let start = text.find("to_i32(").unwrap() + "to_i32(".len();
        assert_eq!((errors[0].code, errors[0].stage), ("E0100", "parse"));
        assert_eq!(errors[0].message, "conversion requires `)`");
        assert_eq!(
            errors[0].primary,
            Some(file.span(start, (start + 1).min(text.len())))
        );
    }
}

#[test]
fn u8_bare_fields_ordinary_calls_and_len_byte_storage_successor() {
    let map = source("struct R{to_i32:i32,to_u8_checked:i32,u8:i32}fn to_i32(x:i32)->i32{return x;}fn f(r:R,a:[i32;0])->i32{r.to_i32;r.to_u8_checked;r.u8;a.len();return to_i32(1);}");
    let ast = parsed(&map);
    assert_eq!(
        ast.expressions
            .iter()
            .filter(|e| matches!(e.kind, ExprKind::FieldRead { .. }))
            .count(),
        3
    );
    assert_eq!(
        ast.expressions
            .iter()
            .filter(|e| matches!(e.kind, ExprKind::ArrayLength { .. }))
            .count(),
        1
    );
    assert_eq!(
        ast.expressions
            .iter()
            .filter(|e| matches!(e.kind, ExprKind::Call { .. }))
            .count(),
        1
    );
    assert!(!ast
        .expressions
        .iter()
        .any(|e| matches!(e.kind, ExprKind::Conversion { .. })));
    // RFC0030's exact historical expectation for these six forms was
    // E0101/parse, at the two-byte u8 token, with this unchanged message.
    // RFC0031 intentionally supersedes precisely this array/slice domain;
    // enum and conversion exclusions below remain active checks.
    const PREDECESSOR: (&str, &str, &str) =
        ("E0101", "parse", "unsupported typed-preview construct `u8`");
    for ty in [
        "[u8;0]",
        "[u8;1]",
        "&[u8;0]",
        "&mut [u8;0]",
        "&[u8]",
        "&mut [u8]",
    ] {
        let text = format!("fn f(a:{ty})->(){{return;}}");
        let map = source(&text);
        let file = map.get(SourceFileId(0));
        let (ast, _) = typed(
            &map,
            SourceMode::ProjectCandidate,
            MAX_NODES,
            &mut Allocator::default(),
            &mut SyntaxStorage::default(),
        )
        .unwrap_or_else(|errors| panic!("RFC0031 successor of {PREDECESSOR:?}: {errors:?}"));
        assert!(ast.belongs_to(file));
        assert!(ast.uses_owned_syntax(file));
        assert_eq!(file.text_at(ast.functions[0].params[0].ty.span), ty);
    }
    let map = source("enum E{V(u8)}");
    let errors = typed(
        &map,
        SourceMode::ProjectCandidate,
        MAX_NODES,
        &mut Allocator::default(),
        &mut SyntaxStorage::default(),
    )
    .unwrap_err();
    assert_eq!((errors[0].code, errors[0].stage), ("E0100", "parse"));
    assert_eq!(
        errors[0].message,
        "only bool, i32 and () enum payloads are supported"
    );
}

#[test]
fn u8_exact_height_node_and_failed_prefix_allocation_controls() {
    for (groups, accepted) in [(62, true), (63, false)] {
        let map = source(&format!(
            "fn f(x:i32)->u8{{return {}x.to_u8_checked(){};}}",
            "(".repeat(groups),
            ")".repeat(groups)
        ));
        let result = typed(
            &map,
            SourceMode::ProjectCandidate,
            MAX_NODES,
            &mut Allocator::default(),
            &mut SyntaxStorage::default(),
        );
        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            assert_eq!(
                result.unwrap_err()[0].message,
                "expression nesting limit exceeded"
            );
        }
    }
    let map = source("fn f(x:i32)->u8{return x.to_u8_checked();}");
    let mut baseline = Allocator::default();
    let mut storage = SyntaxStorage::default();
    let (ast, nodes) = typed(
        &map,
        SourceMode::ProjectCandidate,
        MAX_NODES,
        &mut baseline,
        &mut storage,
    )
    .unwrap();
    assert_eq!(ast.expressions.len(), 2);
    assert!(typed(
        &map,
        SourceMode::ProjectCandidate,
        nodes,
        &mut Allocator::default(),
        &mut SyntaxStorage::default()
    )
    .is_ok());
    let errors = typed(
        &map,
        SourceMode::ProjectCandidate,
        nodes - 1,
        &mut Allocator::default(),
        &mut SyntaxStorage::default(),
    )
    .unwrap_err();
    assert_eq!(errors[0].message, "syntax node limit exceeded");
    assert!(storage.retained_capacity >= ast.expressions.capacity() * size_of::<Expr>());
    // Height scratch is released before the successful parser result escapes.
    assert_eq!(storage.scratch_capacity, 0);
    assert!(baseline
        .trace
        .iter()
        .any(|event| event.kind == "syntax expression heights"
            && event.success
            && event.length >= ast.expressions.len()
            && event.element_bytes == size_of::<usize>()));
    assert!(storage.peak_capacity_bound >= storage.retained_capacity);
    for attempt in 1..=baseline.attempts {
        let mut allocator = Allocator {
            fail_at: Some(attempt),
            ..Allocator::default()
        };
        assert!(
            typed(
                &map,
                SourceMode::ProjectCandidate,
                MAX_NODES,
                &mut allocator,
                &mut SyntaxStorage::default()
            )
            .is_err(),
            "reserve {attempt}"
        );
    }
    println!("U8_SYNTAX_SUCCESSOR nodes={nodes} expressions={} retained={} scratch={} peak={} growths={} parser_size={} parser_align={} expression_size={} expression_align={} conversion_size={} conversion_option_size={} parse_result_size={}",
        ast.expressions.len(), storage.retained_capacity, storage.scratch_capacity, storage.peak_capacity_bound, storage.growths,
        size_of::<Parser<'_>>(), align_of::<Parser<'_>>(), size_of::<Expr>(), align_of::<Expr>(), size_of::<ConversionOp>(), size_of::<Option<ConversionOp>>(), size_of::<Result<(Program,usize),Vec<Diagnostic>>>());
    assert_eq!(MAX_NESTING, 64);
    assert_eq!(MAX_NODES, 100_000);
}
