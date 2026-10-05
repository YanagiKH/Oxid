//! Checked unary negation through real source parsing, typing and lowering.
//! These cases are separate from the frozen qualification source oracles.
use super::*;
use crate::frontend::{ast, format, lexer, parser, project::budget::Allocator};

fn parsed(text: &str) -> (SourceMap, ast::Program) {
    let mut sources = SourceMap::new();
    let id = sources.add("unary-雪.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    (sources, ast)
}

fn raw(text: &str) -> (SourceMap, Program) {
    let (sources, ast) = parsed(text);
    let source = sources.get(crate::frontend::source::SourceFileId(0));
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    (sources, lower::lower(&typed).unwrap())
}

#[test]
fn signed_decimal_stays_one_literal_with_identical_origins_and_fuel() {
    for (expression, value) in [
        ("-1", -1),
        ("- /* 雪 */ 1", -1),
        ("- // 🦀\r\n 2147483648", i32::MIN),
        ("-0002147483648", i32::MIN),
    ] {
        let text = format!("fn main() -> i32 {{ return {expression}; }}");
        let (sources, ast) = parsed(&text);
        assert_eq!(ast.expressions.len(), 1);
        let literal = &ast.expressions[0];
        let ast::ExprKind::Number { negative, digits } = literal.kind else {
            panic!("signed decimal became a unary expression")
        };
        assert!(negative);
        assert!(text[digits.start..digits.end].bytes().all(|b| b.is_ascii_digit()));
        assert_eq!(&text[literal.span.start..literal.span.end], expression);
        let typed = typeck::check(
            hir::resolve(sources.get(literal.span.file), &ast).unwrap(),
        )
        .unwrap();
        let raw = lower::lower(&typed).unwrap();
        assert_eq!(raw.functions[0].locals.len(), 1);
        assert_eq!(raw.functions[0].blocks[0].statements.len(), 1);
        let assignment = raw.functions[0].blocks[0].statements[0].assignment();
        assert_eq!(assignment.value, Rvalue::I32(value));
        assert_eq!(assignment.span, literal.span);
        let end = raw.functions[0].blocks[0].terminator.as_ref().unwrap().span;
        let verified = verify::verify(raw, &sources).unwrap();
        assert_eq!(execute::run_with_fuel(&verified, hir::DefId(0), 2), Err(RunFailure::Fuel(literal.span)));
        assert_eq!(execute::run_with_fuel(&verified, hir::DefId(0), 3), Err(RunFailure::Fuel(end)));
        assert_eq!(execute::run_with_fuel(&verified, hir::DefId(0), 4), Ok(Scalar::I32(value)));
    }
}

#[test]
fn unary_preserves_operand_operator_and_expression_provenance() {
    let text = "// 雪\r\nfn main() -> i32 { return - /* 🦀 */ (-2147483648); }";
    let (sources, ast) = parsed(text);
    let expression = ast.expressions.last().unwrap();
    let ast::ExprKind::Negate { operand, operator_span } = expression.kind else {
        panic!("checked unary AST node")
    };
    assert_eq!(&text[operator_span.start..operator_span.end], "-");
    assert_eq!(&text[expression.span.start..expression.span.end], "- /* 🦀 */ (-2147483648)");
    assert_eq!(&text[ast.expressions[operand.0].span.start..ast.expressions[operand.0].span.end], "(-2147483648)");
    let resolved = hir::resolve(sources.get(expression.span.file), &ast).unwrap();
    let hir::ExprKind::Negate { operator_span: hir_operator, operand: hir_operand } = resolved.functions[0].expressions.last().unwrap().kind else {
        panic!("checked unary HIR node")
    };
    assert_eq!(hir_operator, operator_span);
    assert_eq!(resolved.functions[0].expressions[hir_operand.0].span, ast.expressions[operand.0].span);
    let typed = typeck::check(resolved).unwrap();
    let raw = lower::lower(&typed).unwrap();
    let assignments = &raw.functions[0].blocks[0].statements;
    assert_eq!(assignments.len(), 3); // literal, unchanged grouping copy, negation
    let assignment = assignments.last().unwrap().assignment();
    let Rvalue::CheckedNegateI32 { operand: read, operator_span: oir_operator } = assignment.value else {
        panic!("checked unary OIR node")
    };
    assert_eq!(assignment.span, expression.span);
    assert_eq!(read.span, ast.expressions[operand.0].span);
    assert_eq!(oir_operator, operator_span);
    let verified = verify::verify(raw, &sources).unwrap();
    assert_eq!(verified.run(Some(hir::DefId(0))), Err(RunFailure::Overflow(operator_span)));
}

#[test]
fn unary_adds_exactly_one_assignment_one_slot_and_two_fuel() {
    for (expression, expected, locals, fuel) in [
        ("-1", -1, 1, 4),
        ("--1", 1, 2, 6),
        ("---1", -1, 3, 8),
        ("(1)", 1, 2, 6),
        ("-(1)", -1, 3, 8),
        ("-(-1)", 1, 3, 8),
    ] {
        let (sources, raw) = raw(&format!("fn main() -> i32 {{ return {expression}; }}"));
        assert_eq!(raw.functions[0].locals.len(), locals, "{expression}");
        assert_eq!(raw.functions[0].blocks[0].statements.len(), locals, "{expression}");
        let end = raw.functions[0].blocks[0].terminator.as_ref().unwrap().span;
        let verified = verify::verify(raw, &sources).unwrap();
        assert_eq!(execute::run_with_fuel(&verified, hir::DefId(0), fuel - 1), Err(RunFailure::Fuel(end)), "{expression}");
        assert_eq!(execute::run_with_fuel(&verified, hir::DefId(0), fuel), Ok(Scalar::I32(expected)), "{expression}");
    }
}

#[test]
fn fuel_is_charged_before_negating_min_and_before_outer_prefixes() {
    for (expression, fuel, unary_index) in [("--2147483648", 5, 1), ("-(-2147483648)", 7, 2), ("--(-2147483648)", 8, 2)] {
        let (sources, raw) = raw(&format!("fn main() -> i32 {{ return {expression}; }}"));
        let unary = raw.functions[0].blocks[0].statements[unary_index].assignment();
        let Rvalue::CheckedNegateI32 { operator_span, .. } = unary.value else { panic!() };
        let unary_span = unary.span;
        let verified = verify::verify(raw, &sources).unwrap();
        assert_eq!(execute::run_with_fuel(&verified, hir::DefId(0), fuel - 1), Err(RunFailure::Fuel(unary_span)), "{expression}");
        assert_eq!(execute::run_with_fuel(&verified, hir::DefId(0), fuel), Err(RunFailure::Overflow(operator_span)), "{expression}");
    }
}

#[test]
fn call_operand_is_lowered_once_and_has_the_exact_execution_cost() {
    let text = "fn value() -> i32 { return 7; } fn main() -> i32 { return -value(); }";
    let (sources, raw) = raw(text);
    let main = &raw.functions[1];
    assert_eq!(main.locals.len(), 2);
    assert_eq!(main.blocks.iter().filter(|b| matches!(b.terminator.as_ref().unwrap().kind, TerminatorKind::Call { .. })).count(), 1);
    assert_eq!(main.blocks.iter().map(|b| b.statements.len()).sum::<usize>(), 1);
    let end = main.blocks.last().unwrap().terminator.as_ref().unwrap().span;
    let verified = verify::verify(raw, &sources).unwrap();
    assert_eq!(execute::run_with_fuel(&verified, hir::DefId(1), 8), Err(RunFailure::Fuel(end)));
    assert_eq!(execute::run_with_fuel(&verified, hir::DefId(1), 9), Ok(Scalar::I32(-7)));
}

#[test]
fn scalar_and_owned_source_dispatch_share_precedence_and_first_error_order() {
    for prefix in ["", "struct Unused {} "] {
        for (expression, expected) in [("-x * 3 + 10", 4), ("20 / -x", -10), ("20 % -x", 0), ("--x", 2), ("1--x", 3), ("-(-x + 5)", -3)] {
            let text = format!("{prefix}fn main() -> i32 {{ let x = 2; return {expression}; }}");
            let (sources, ast) = parsed(&text);
            let checked = check_source(sources.get(crate::frontend::source::SourceFileId(0)), &ast, &sources).unwrap();
            assert_eq!(checked.run().unwrap(), Scalar::I32(expected), "{text}");
        }
        for (expression, marker) in [("-(1 / 0)", "/"), ("-(2147483647 + 1)", "+"), ("-(-2147483648) + 1 / 0", "-(-"), ("1 / 0 + -(-2147483648)", "/")] {
            let text = format!("{prefix}fn main() -> i32 {{ return {expression}; }}");
            let (sources, ast) = parsed(&text);
            let checked = check_source(sources.get(crate::frontend::source::SourceFileId(0)), &ast, &sources).unwrap();
            let error = checked.run().unwrap_err();
            let start = text.find("return ").unwrap() + 7 + expression.find(marker).unwrap();
            assert_eq!(error.code, if marker == "/" { "E0607" } else { "E0604" });
            assert_eq!(error.primary.unwrap(), sources.get(crate::frontend::source::SourceFileId(0)).span(start, start + 1));
        }
    }
}

#[test]
fn unary_nodes_obey_exact_lowered_parser_node_budget_in_all_source_modes() {
    for mode in [parser::SourceMode::ScalarOnly, parser::SourceMode::OwnedCandidate, parser::SourceMode::ModuleCandidate, parser::SourceMode::ProjectCandidate] {
        for (expression, nodes) in [("-1", 3), ("- /* 雪 */ 1", 3), ("--1", 4), ("-(1)", 5), ("-x", 4), ("!-x", 5)] {
            let text = format!("fn main() -> i32 {{ return {expression}; }}");
            let mut sources = SourceMap::new();
            let id = sources.add("budget.ox".into(), text);
            let source = sources.get(id);
            let (_, actual) = parser::parse_counted(source, lexer::lex(source).unwrap(), mode, nodes, &mut Allocator::default()).unwrap();
            assert_eq!(actual, nodes, "{expression}");
            let errors = parser::parse_counted(source, lexer::lex(source).unwrap(), mode, nodes - 1, &mut Allocator::default()).unwrap_err();
            assert_eq!((errors[0].code, errors[0].stage), ("E0400", "parse"), "{expression}");
        }
    }
}

#[test]
fn formatting_retains_tokens_unary_tree_shape_and_runtime_result() {
    for prefix in ["", "struct Marker{} "] {
        let text = format!("{prefix}fn main()->i32{{let x=2;return - /* 雪 */ ( x + - 1 ) * -- x;}}");
        let (sources, ast) = parsed(&text);
        let source = sources.get(crate::frontend::source::SourceFileId(0));
        let before = check_source(source, &ast, &sources).unwrap().run().unwrap();
        let formatted = format::format_source(source).unwrap();
        assert!(formatted.contains("- /* 雪 */ (x + -1) * --x"), "{formatted}");
        let (after_sources, after_ast) = parsed(&formatted);
        let after_source = after_sources.get(crate::frontend::source::SourceFileId(0));
        assert_eq!(format::format_source(after_source).unwrap(), formatted);
        assert_eq!(after_ast.expressions.len(), ast.expressions.len());
        assert_eq!(after_ast.expressions.iter().filter(|e| matches!(e.kind, ast::ExprKind::Negate { .. })).count(), 3);
        assert_eq!(check_source(after_source, &after_ast, &after_sources).unwrap().run().unwrap(), before);
        assert_eq!(before, Scalar::I32(-2));
    }
}
