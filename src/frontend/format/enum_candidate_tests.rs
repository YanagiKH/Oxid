//! Private enum formatting; ordinary format_source remains closed.
use super::*;
use crate::frontend::source::SourceFileId;

fn source(text: &str) -> SourceMap {
    let mut sources = SourceMap::new();
    sources.add("enum-format.ox".into(), text.into());
    sources
}
fn formatted(text: &str) -> String {
    let sources = source(text);
    format_enum_candidate_observed(sources.get(SourceFileId(0)), &mut Allocator::default())
        .0
        .unwrap()
}

#[test]
fn enum_formatter_private_roundtrip_keeps_public_gate_closed() {
    let text = "enum E{V(i32),Z}fn f(e:E)->(){match e{E::V(v)=>{crate::m::f(v,&mut *r);},E::Z=>{E::Z();},}}";
    let expected = "enum E { V(i32), Z } fn f(e: E) -> () { match e { E::V(v) => { crate::m::f(v, &mut *r); }, E::Z => { E::Z(); }, } }\n";
    let output = formatted(text);
    assert_eq!(output, expected);
    assert_eq!(formatted(&output), output);
    let sources = source(text);
    assert!(format_source(sources.get(SourceFileId(0))).is_err());
    let (_, metrics) =
        format_enum_candidate_observed(sources.get(SourceFileId(0)), &mut Allocator::default());
    assert_eq!(metrics.parse_calls, 2);
    assert_eq!(metrics.output_heap, output.len());
    assert_eq!(metrics.roles_heap, text.len());
    assert!(metrics.source_owner_heap > metrics.output_heap);
    assert!(metrics.phase_peak_heap_bound >= metrics.emit_live_heap);
    assert!(metrics.phase_peak_heap_bound >= metrics.reparse_live_heap);
}

#[test]
fn enum_formatter_preserves_arrow_tokens_comments_and_forwarded_borrows() {
    let text = concat!(
        "/* enum match => :: { } */\r\nenum E{Z,U(()),I(i32),B(bool)}\n",
        "fn f(e:E)->(){\nmatch e{\n",
        "E/* type */::/* variant */I(v)/* before */=>/* after */{crate::m::call(&*r,&mut *r);},\n",
        "E::Z=>{E::Z;E::Z();},E::U(u)=>{E::U(());},E::B(b)=>{return;},\n}}",
    );
    let sources = source(text);
    let file = sources.get(SourceFileId(0));
    let output = formatted(text);
    assert!(output.contains("=>"));
    assert!(!output.contains("= >"));
    assert!(output.contains("call(&*r, &mut *r)"));
    let candidates = source(&output);
    let candidate = candidates.get(SourceFileId(0));
    assert!(same_projection(
        file,
        &lexer::lex(file).unwrap(),
        candidate,
        &lexer::lex(candidate).unwrap()
    ));
    assert_eq!(formatted(&output), output);
    for bad_arrow in ["= >", "=/*gap*/>", "=\n>", "==>", "->"] {
        let text = format!("fn f()->(){{match x{{E::V{bad_arrow}{{}}}}}}");
        let sources = source(&text);
        let mut allocator = Allocator::default();
        let (result, metrics) =
            format_enum_candidate_observed(sources.get(SourceFileId(0)), &mut allocator);
        assert!(result.is_err());
        assert_eq!(metrics.parse_calls, 1);
        assert!(
            !allocator
                .trace
                .iter()
                .any(|event| event.kind == "formatter token roles"
                    || event.kind == "formatted source")
        );
    }
}

fn ast_heap(program: &Program) -> usize {
    use crate::frontend::ast::*;
    use std::mem::size_of;
    fn bytes<T>(values: &Vec<T>) -> usize {
        values.capacity() * size_of::<T>()
    }
    let mut result = bytes(&program.tokens)
        + bytes(&program.functions)
        + bytes(&program.expressions)
        + bytes(&program.records)
        + bytes(&program.enums)
        + bytes(&program.items)
        + bytes(&program.modules)
        + bytes(&program.paths)
        + bytes(&program.path_segments)
        + bytes(&program.imports);
    for declaration in &program.enums {
        result += bytes(&declaration.variants);
    }
    for declaration in &program.records {
        result += bytes(&declaration.fields);
    }
    for function in &program.functions {
        result += bytes(&function.params) + bytes(&function.blocks);
        for block in &function.blocks {
            result += bytes(&block.body);
            for statement in &block.body {
                if let StmtKind::Match { arms, .. } = &statement.kind {
                    result += bytes(arms);
                }
            }
        }
    }
    for expression in &program.expressions {
        result += match &expression.kind {
            ExprKind::Call { args, .. }
            | ExprKind::QualifiedValue {
                args: Some(args), ..
            } => bytes(args),
            ExprKind::StructLiteral { fields, .. } => bytes(fields),
            ExprKind::ArrayLiteral { elements } => bytes(elements),
            _ => 0,
        };
    }
    result
}

#[test]
fn enum_formatter_actual_capacity_and_coexistence_match_independent_inventory() {
    let text = "pub enum E{I(i32),Z,U(())}\nfn f(e:E)->(){\nlet a=[1,2,3];match e{E::I(v)=>{crate::m::f(v,&mut *r);},E::Z=>{E::Z();},E::U(u)=>{E::U(());}}}";
    let sources = source(text);
    let file = sources.get(SourceFileId(0));
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(4096).unwrap();
    let (result, metrics) = format_enum_candidate_observed(file, &mut allocator);
    let output = result.unwrap();
    assert_eq!(metrics.parse_calls, 2);
    let (first, _) =
        parse_with_syntax(file, &mut Allocator::default(), FormatSyntax::EnumCandidate).unwrap();
    let candidates = source(&output);
    let (second, _) = parse_with_syntax(
        candidates.get(SourceFileId(0)),
        &mut Allocator::default(),
        FormatSyntax::EnumCandidate,
    )
    .unwrap();
    assert_eq!(metrics.first_ast_heap, ast_heap(&first));
    assert_eq!(metrics.second_ast_heap, ast_heap(&second));
    assert_eq!(
        metrics.first_token_heap,
        first.tokens.capacity() * std::mem::size_of::<Token>()
    );
    assert_eq!(
        metrics.second_token_heap,
        second.tokens.capacity() * std::mem::size_of::<Token>()
    );
    let line_count = output.bytes().filter(|&byte| byte == b'\n').count() + 1;
    let owner = std::mem::size_of::<SourceFile>()
        + output.capacity()
        + line_count * std::mem::size_of::<usize>();
    assert_eq!(metrics.source_owner_heap, owner);
    assert_eq!(
        metrics.emit_live_heap,
        ast_heap(&first) + text.len() + output.capacity()
    );
    assert_eq!(
        metrics.reparse_live_heap,
        ast_heap(&first) + owner + ast_heap(&second)
    );
    assert_eq!(metrics.roles_heap, text.len());
    assert_eq!(output.capacity(), output.len());
    assert!(!allocator.observer_trace_overflow);
    for (kind, length, width) in [
        ("formatter token roles", text.len(), 1),
        ("formatted source", output.len(), 1),
        (
            "formatted source line starts",
            line_count,
            std::mem::size_of::<usize>(),
        ),
        (
            "formatted source files",
            1,
            std::mem::size_of::<SourceFile>(),
        ),
    ] {
        let events: Vec<_> = allocator
            .trace
            .iter()
            .filter(|event| event.kind == kind)
            .collect();
        assert_eq!(events.len(), 1);
        assert_eq!(
            (events[0].length, events[0].element_bytes, events[0].success),
            (length, width, true)
        );
    }
    println!(
        "c1c-capacities {metrics:?} program-header={} source-map-header={}",
        std::mem::size_of::<Program>(),
        std::mem::size_of::<SourceMap>()
    );
}

#[test]
fn enum_formatter_each_observed_reserve_failure_returns_no_output() {
    let text = "enum E{V(i32),Z}fn f(e:E)->(){match e{E::V(v)=>{E::V(v);},E::Z=>{},}}";
    let sources = source(text);
    let file = sources.get(SourceFileId(0));
    let mut baseline = Allocator::default();
    baseline.observer_trace_bound(4096).unwrap();
    let (result, _) = format_enum_candidate_observed(file, &mut baseline);
    result.unwrap();
    let second_start = baseline
        .trace
        .iter()
        .position(|event| event.kind == "formatted source files")
        .unwrap()
        + 2;
    for fail_at in 1..=baseline.attempts {
        let mut allocator = Allocator {
            fail_at: Some(fail_at),
            ..Allocator::default()
        };
        allocator.observer_trace_bound(4096).unwrap();
        let (result, metrics) = format_enum_candidate_observed(file, &mut allocator);
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|error| error.code == "E0400"));
        assert_eq!(allocator.attempts, fail_at);
        assert!(!allocator.trace.last().unwrap().success);
        assert!(!allocator.observer_trace_overflow);
        if fail_at >= second_start {
            assert_eq!(metrics.parse_calls, 2);
            assert_eq!((errors[0].stage, errors[0].primary), ("format", None));
            assert_eq!(
                errors[0].message,
                "formatted source exceeds lexer or parser resource limits"
            );
        }
        assert_eq!(file.text(), text);
    }
}

#[test]
fn enum_formatter_delimiter_work_and_reparse_resource_gates_remain_separate() {
    let text = "enum E{V}fn f()->(){match e{E::V=>{}}}";
    let sources = source(text);
    let file = sources.get(SourceFileId(0));
    for (delimiters, work_bytes, success) in [
        (3, text.len(), true),
        (2, text.len(), false),
        (3, text.len() - 1, false),
    ] {
        let (result, metrics) = format_with_syntax(
            file,
            &mut Allocator::default(),
            Limits {
                delimiters,
                work_bytes,
            },
            FormatSyntax::EnumCandidate,
        );
        assert_eq!(result.is_ok(), success);
        if let Err(errors) = result {
            assert_eq!((errors[0].code, errors[0].stage), ("E0400", "format"));
            assert_eq!(metrics.parse_calls, 1);
        }
    }
    // Both parser depth dimensions fit; their combined delimiter depth need not.
    for matches in [32usize, 33] {
        let text = format!(
            "fn f()->(){{{}{}1{};{}}}",
            "match e{E::V=>{".repeat(matches),
            "(".repeat(63),
            ")".repeat(63),
            "}}".repeat(matches)
        );
        let sources = source(&text);
        let file = sources.get(SourceFileId(0));
        assert!(
            parse_with_syntax(file, &mut Allocator::default(), FormatSyntax::EnumCandidate).is_ok()
        );
        let (result, _) = format_enum_candidate_observed(file, &mut Allocator::default());
        assert_eq!(result.is_ok(), matches == 32);
        if let Err(errors) = result {
            assert_eq!(errors[0].message, "formatter delimiter limit exceeded");
        }
    }
    // Formatting indentation can exceed the unchanged lexer token byte cap on reparse.
    let text = format!("fn f()->(){{{}return;}}", "\n".repeat(65_533));
    let sources = source(&text);
    let (result, metrics) =
        format_enum_candidate_observed(sources.get(SourceFileId(0)), &mut Allocator::default());
    let errors = result.unwrap_err();
    assert_eq!(metrics.parse_calls, 2);
    assert_eq!(
        (errors[0].code, errors[0].stage, errors[0].primary),
        ("E0400", "format", None)
    );
}

#[test]
fn enum_formatter_exact_output_cap_and_owner_spare_capacity_controls() {
    let comment = format!("/*{}*/", "x".repeat(65_531));
    let input = comment.repeat(16);
    let output = formatted(&input);
    assert_eq!(output.len(), MAX_SOURCE_BYTES);
    let too_large = format!("/*{}*/{}", "x".repeat(65_532), comment.repeat(15));
    let sources = source(&too_large);
    let mut allocator = Allocator::default();
    let (result, _) = format_enum_candidate_observed(sources.get(SourceFileId(0)), &mut allocator);
    assert_eq!(result.unwrap_err()[0].stage, "format");
    assert!(!allocator
        .trace
        .iter()
        .any(|event| event.kind == "formatted source"));
    let mut text = String::with_capacity(32);
    text.push('x');
    let mut owner = SourceMap::new();
    let mut allocator = Allocator::default();
    assert!(owner
        .try_add_format_candidate(text, &mut allocator)
        .is_err());
    assert_eq!(allocator.attempts, 0);
    for text in ["", "x\n", "a\nb\n"] {
        let mut allocator = Allocator::default();
        let mut owned = String::new();
        allocator
            .string(&mut owned, text.len(), "test text")
            .unwrap();
        owned.push_str(text);
        let mut owner = SourceMap::new();
        let id = owner
            .try_add_format_candidate(owned, &mut allocator)
            .unwrap();
        let count = text.bytes().filter(|&byte| byte == b'\n').count() + 1;
        assert_eq!(
            owner.heap_capacity_bytes(),
            Some(
                std::mem::size_of::<SourceFile>()
                    + text.len()
                    + count * std::mem::size_of::<usize>()
            )
        );
        assert_eq!(owner.get(id).text(), text);
    }
}
