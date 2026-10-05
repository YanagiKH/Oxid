//! Private syntax/resource qualification, with all executable routes still closed.
use super::*;
use crate::frontend::{
    declaration_index::{collect_originals, IndexLimits, SourceOwner, WorkMeter},
    lexer,
    source::{SourceFileId, SourceMap, SourceView},
};
use std::mem::size_of;

fn source(text: &str) -> SourceMap {
    let mut sources = SourceMap::new();
    sources.add("enums.ox".into(), text.into());
    sources
}
fn parse_candidate(
    sources: &SourceMap,
    limit: usize,
    allocator: &mut Allocator,
    storage: &mut enums::SyntaxStorage,
) -> Result<(Program, usize), Vec<Diagnostic>> {
    let file = sources.get(SourceFileId(0));
    parse_enum_candidate_counted(
        file,
        lexer::lex(file).unwrap(),
        SourceMode::ProjectCandidate,
        limit,
        allocator,
        storage,
    )
}
fn candidate(sources: &SourceMap) -> Program {
    parse_candidate(
        sources,
        MAX_NODES,
        &mut Allocator::default(),
        &mut enums::SyntaxStorage::default(),
    )
    .unwrap()
    .0
}
fn rejected(text: &str) {
    let sources = source(text);
    assert!(
        parse_candidate(
            &sources,
            MAX_NODES,
            &mut Allocator::default(),
            &mut enums::SyntaxStorage::default()
        )
        .is_err(),
        "{text}"
    );
}

#[test]
fn enum_candidate_retains_declarations_match_paths_and_origins() {
    let sources = source("enum E { Z, B(bool), I(i32), U(()), } fn f(e:E)->(){match e { E::I(v)=>{v;},E::Z=>{},E::U(u)=>{},E::B(b)=>{}, }}");
    let file = sources.get(SourceFileId(0));
    let program = candidate(&sources);
    assert!(program.belongs_to(file));
    assert!(program.validate_spans_and_ids(|at| file.try_text(at).is_some()));
    assert!(!program.uses_project_syntax());
    assert!(program.uses_owned_syntax(file));
    assert_eq!(program.enums.len(), 1);
    let variants = &program.enums[0].variants;
    assert_eq!(variants.len(), 4);
    assert!(variants[0].payload.is_none());
    assert!(matches!(
        variants[1].payload.unwrap().kind,
        ScalarTypeSyntax::Bool
    ));
    assert!(matches!(
        variants[2].payload.unwrap().kind,
        ScalarTypeSyntax::I32
    ));
    assert!(matches!(
        variants[3].payload.unwrap().kind,
        ScalarTypeSyntax::Unit
    ));
    assert_eq!(file.text_at(variants[3].payload.unwrap().span), "()");
    let function = &program.functions[0];
    let StmtKind::Match { scrutinee, arms } = &function.blocks[0].body[0].kind else {
        panic!("match")
    };
    assert_eq!(file.text_at(*scrutinee), "e");
    assert_eq!(arms.len(), 4);
    assert_eq!(file.text_at(program.paths[arms[0].variant.0].span), "E::I");
    assert_eq!(file.text_at(arms[0].binding.unwrap()), "v");
    assert!(arms[1].binding.is_none());
    assert_eq!(function.blocks.len(), 5);
    assert_eq!(program.paths.len(), 4);
    assert_eq!(program.path_segments.len(), 8);
    let mut end = 0;
    for path in &program.paths {
        assert_eq!(path.root, PathRoot::LocalType);
        assert_eq!(path.segment_start, end);
        end += usize::from(path.segment_len);
    }
    let owner = SourceOwner::original(file, &program, SourceView::Single(file)).unwrap();
    let mut allocator = Allocator::default();
    let error = collect_originals(
        owner,
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut allocator,
    )
    .unwrap_err();
    assert_eq!(error.code, "E0101");
    assert_eq!(allocator.attempts, 0);
}

#[test]
fn enum_candidate_qualified_values_preserve_parentheses_and_argument_order() {
    let sources = source(
        "fn f()->(){E::Z;E::Z();E::U(());crate::m::f(1,2,&r);crate::m::E::I(3);crate::m::R{x:4};}",
    );
    let file = sources.get(SourceFileId(0));
    let program = candidate(&sources);
    assert!(program.validate_spans_and_ids(|at| file.try_text(at).is_some()));
    let values: Vec<_> = program
        .expressions
        .iter()
        .filter_map(|expr| {
            if let ExprKind::QualifiedValue { path, args } = &expr.kind {
                Some((*path, args))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(values.len(), 5);
    assert!(values[0].1.is_none());
    assert!(values[1].1.as_ref().unwrap().is_empty());
    let Argument::Value(unit) = values[2].1.as_ref().unwrap()[0] else {
        panic!("unit")
    };
    assert!(matches!(program.expressions[unit.0].kind, ExprKind::Unit));
    let args = values[3].1.as_ref().unwrap();
    assert_eq!(args.len(), 3);
    for (position, spelling) in ["1", "2"].iter().enumerate() {
        let Argument::Value(id) = args[position] else {
            panic!("value")
        };
        assert_eq!(file.text_at(program.expressions[id.0].span), *spelling);
    }
    assert!(matches!(args[2], Argument::Borrow { .. }));
    assert_eq!(
        file.text_at(program.paths[values[3].0 .0].span),
        "crate::m::f"
    );
    assert!(program.expressions.iter().any(|expr| matches!(
        expr.kind,
        ExprKind::StructLiteral {
            record: ItemPath::Absolute(_),
            ..
        }
    )));
}

#[test]
fn enum_candidate_refines_only_exact_unsupported_keywords_and_adjacent_arrow() {
    let text = "enum E{V} fn f()->(){match e{E::V=>{}}}";
    let sources = source(text);
    let file = sources.get(SourceFileId(0));
    let tokens = lexer::lex(file).unwrap();
    for spelling in ["enum", "match"] {
        assert!(tokens
            .iter()
            .any(|token| token.kind == Kind::Unsupported && file.text_at(token.span) == spelling));
    }
    assert!(tokens.windows(2).any(|pair| pair[0].kind == Kind::Equal
        && pair[1].kind == Kind::Greater
        && pair[0].span.end == pair[1].span.start));
    let program = candidate(&sources);
    assert_eq!(program.tokens.len(), tokens.len());
    for (a, b) in program.tokens.iter().zip(&tokens) {
        assert_eq!((a.kind, a.span), (b.kind, b.span));
    }
    for arrow in ["= >", "=/*x*/>", "==>", "->"] {
        rejected(&format!("fn f()->(){{match e{{E::V{arrow}{{}}}}}}"));
    }
    for punctuation in ["E: :V", "E:/*x*/:V"] {
        rejected(&format!("fn f()->(){{{punctuation};}}"));
    }
    for text in ["type E{}", "for e{}", "fn f()->(){loop{}}"] {
        rejected(text);
    }
    let spaced = source("fn f()->(){match e{E /*x*/ :: /*y*/ V /*z*/ => /*w*/ {}}}");
    assert!(candidate(&spaced).validate_spans_and_ids(|at| spaced.try_text(at).is_some()));
}

#[test]
fn enum_candidate_keeps_production_and_array_candidate_entrypoints_closed() {
    let sources = source("enum E{V} fn f()->(){E::V;match e{E::V=>{}}}");
    let file = sources.get(SourceFileId(0));
    for arrays in [
        ArraySyntaxPolicy::Closed,
        ArraySyntaxPolicy::Enabled,
        ArraySyntaxPolicy::Candidate,
    ] {
        assert!(parse_counted_with_arrays(
            file,
            lexer::lex(file).unwrap(),
            SourceMode::ProjectCandidate,
            MAX_NODES,
            &mut Allocator::default(),
            arrays
        )
        .is_err());
    }
    assert!(crate::frontend::format::format_source(file).is_err());
    let ordinary = source("use crate::f; fn f()->i32{return crate::f();}");
    let file = ordinary.get(SourceFileId(0));
    let mut allocator = Allocator::default();
    let program = parse_counted(
        file,
        lexer::lex(file).unwrap(),
        SourceMode::ProjectCandidate,
        MAX_NODES,
        &mut allocator,
    )
    .unwrap()
    .0;
    assert!(program
        .expressions
        .iter()
        .any(|expr| matches!(expr.kind, ExprKind::Call { .. })));
    assert!(!program
        .expressions
        .iter()
        .any(|expr| matches!(expr.kind, ExprKind::QualifiedValue { .. })));
    assert!(allocator.trace.iter().all(|event| matches!(
        event.kind,
        "absolute path segments" | "absolute paths" | "import declarations" | "import items"
    )));
}

#[test]
fn enum_candidate_rejects_out_of_scope_declarations_and_patterns() {
    for declaration in [
        "enum E{}",
        "enum E{V()}",
        "enum E{V(i64)}",
        "enum E{V(R)}",
        "enum E{V(&i32)}",
        "enum E{V([i32;1])}",
        "enum E{V(i32,bool)}",
        "enum E{pub V}",
        "enum E{V=0}",
    ] {
        rejected(declaration);
    }
    for pattern in [
        "V",
        "_",
        "E::V()",
        "E::V(_)",
        "E::V(mut v)",
        "E::V(a,b)",
        "E::V(E::W(v))",
        "E::V|E::W",
        "E::V if true",
    ] {
        rejected(&format!("fn f()->(){{match e{{{pattern}=>{{}}}}}}"));
    }
    for scrutinee in ["(e)", "e.x", "f()", "E::V", "true"] {
        rejected(&format!("fn f()->(){{match {scrutinee}{{E::V=>{{}}}}}}"));
    }
    for text in [
        "fn f()->(){match e{}}",
        "fn f()->(){match e{E::V=>return;}}",
        "fn f()->(){let x=match e{E::V=>{}};}",
        "fn f()->(){match e{E::V=>{} E::W=>{}}}",
        "fn f()->(){E::V{};}",
        "fn f()->(){E::V::W;}",
    ] {
        rejected(text);
    }
}

#[test]
fn enum_candidate_exact_node_and_preallocation_failure_order() {
    for (text, nodes) in [
        ("enum E{V}", 2),
        ("fn f()->(){E::V;}", 5),
        ("fn f()->(){match e{E::V=>{}}}", 5),
    ] {
        let sources = source(text);
        let (_, actual) = parse_candidate(
            &sources,
            nodes,
            &mut Allocator::default(),
            &mut enums::SyntaxStorage::default(),
        )
        .unwrap();
        assert_eq!(actual, nodes);
        let mut allocator = Allocator::default();
        let error = parse_candidate(
            &sources,
            nodes - 1,
            &mut allocator,
            &mut enums::SyntaxStorage::default(),
        )
        .unwrap_err();
        assert_eq!(error[0].code, "E0400");
        assert_eq!(error[0].message, "syntax node limit exceeded");
        assert!(!allocator
            .trace
            .iter()
            .any(|event| event.kind == "enum declarations"
                || event.kind == "enum match arms"
                || event.kind == "syntax expressions"));
    }
    let sources = source("enum E{V}");
    let mut allocator = Allocator::default();
    assert!(parse_candidate(
        &sources,
        1,
        &mut allocator,
        &mut enums::SyntaxStorage::default()
    )
    .is_err());
    assert_eq!(
        allocator.attempts, 0,
        "declaration/member node gates precede retention"
    );
}

#[test]
fn enum_candidate_member_arm_path_and_depth_boundaries() {
    for count in [1usize, 256, 257] {
        let declaration = format!(
            "enum E{{{}}}",
            (0..count).map(|n| format!("V{n},")).collect::<String>()
        );
        let arms = format!("fn f()->(){{match e{{{}}}}}", "E::V=>{},".repeat(count));
        for (text, expected) in [
            (&declaration, "enum variant limit exceeded (256)"),
            (&arms, "match arm limit exceeded (256)"),
        ] {
            let sources = source(text);
            let result = parse_candidate(
                &sources,
                MAX_NODES,
                &mut Allocator::default(),
                &mut enums::SyntaxStorage::default(),
            );
            if count <= 256 {
                assert!(result.is_ok(), "{result:?}");
            } else {
                assert_eq!(result.unwrap_err()[0].message, expected);
            }
        }
    }
    for count in [33usize, 34, 35] {
        let sources = source(&format!(
            "fn f()->(){{crate{}::V;}}",
            "::m".repeat(count - 2)
        ));
        let result = parse_candidate(
            &sources,
            MAX_NODES,
            &mut Allocator::default(),
            &mut enums::SyntaxStorage::default(),
        );
        assert_eq!(result.is_ok(), count <= 34);
        if let Ok((program, _)) = result {
            assert_eq!(usize::from(program.paths[0].segment_len), count);
        }
    }
    for depth in [63usize, 64] {
        let text = format!(
            "fn f()->(){{{}return;{}}}",
            "match e{E::V=>{".repeat(depth),
            "}}".repeat(depth)
        );
        let sources = source(&text);
        let mut allocator = Allocator::default();
        let result = parse_candidate(
            &sources,
            MAX_NODES,
            &mut allocator,
            &mut enums::SyntaxStorage::default(),
        );
        assert_eq!(result.is_ok(), depth == 63);
        if depth == 64 {
            assert_eq!(
                result.unwrap_err()[0].message,
                "statement block nesting limit exceeded"
            );
            assert!(!allocator
                .trace
                .iter()
                .any(|event| event.kind == "syntax blocks" && event.length > 64));
        }
        let expression = format!(
            "fn f()->(){{{}1{};}}",
            "E::V(".repeat(depth),
            ")".repeat(depth)
        );
        let sources = source(&expression);
        assert_eq!(
            parse_candidate(
                &sources,
                MAX_NODES,
                &mut Allocator::default(),
                &mut enums::SyntaxStorage::default()
            )
            .is_ok(),
            depth == 63
        );
    }
}

fn retained_capacity(program: &Program) -> usize {
    let mut bytes = program.functions.capacity() * size_of::<Function>()
        + program.records.capacity() * size_of::<StructDecl>()
        + program.enums.capacity() * size_of::<EnumDecl>()
        + program.items.capacity() * size_of::<ItemId>()
        + program.expressions.capacity() * size_of::<Expr>()
        + program.paths.capacity() * size_of::<QualifiedPath>()
        + program.path_segments.capacity() * size_of::<crate::frontend::source::Span>()
        + program.modules.capacity() * size_of::<ModuleDecl>()
        + program.imports.capacity() * size_of::<ImportDecl>();
    for record in &program.records {
        bytes += record.fields.capacity() * size_of::<StructField>();
    }
    for enumeration in &program.enums {
        bytes += enumeration.variants.capacity() * size_of::<EnumVariantSyntax>();
    }
    for function in &program.functions {
        bytes += function.params.capacity() * size_of::<Param>()
            + function.blocks.capacity() * size_of::<BodyBlock>();
        for block in &function.blocks {
            bytes += block.body.capacity() * size_of::<Stmt>();
            for statement in &block.body {
                if let StmtKind::Match { arms, .. } = &statement.kind {
                    bytes += arms.capacity() * size_of::<MatchArmSyntax>();
                }
            }
        }
    }
    for expression in &program.expressions {
        bytes += match &expression.kind {
            ExprKind::Call { args, .. } => args.capacity() * size_of::<Argument>(),
            ExprKind::QualifiedValue { args, .. } => args
                .as_ref()
                .map_or(0, |args| args.capacity() * size_of::<Argument>()),
            ExprKind::StructLiteral { fields, .. } => fields.capacity() * size_of::<FieldInit>(),
            ExprKind::ArrayLiteral { elements } => elements.capacity() * size_of::<ExprId>(),
            _ => 0,
        };
    }
    bytes
}

#[test]
fn enum_candidate_all_affected_vectors_are_fallible_and_capacity_observed() {
    let sources=source("mod m;use crate::m::f as alias;enum E{V(i32),Z,U(()),B(bool)} struct R{x:i32} fn f(e:E,r:&R)->(){let a=[1,2,3,4,5];let r2=R{x:1};plain(&*r,1,2,3,4);crate::m::call(1,2,3,4,5);let e2=E::V(-(1+2));match e{E::V(v)=>{v;},E::Z=>{},E::U(u)=>{},E::B(b)=>{}}}");
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(1000).unwrap();
    let mut storage = enums::SyntaxStorage::default();
    let (program, _) = parse_candidate(&sources, MAX_NODES, &mut allocator, &mut storage).unwrap();
    assert_eq!(storage.retained_capacity, retained_capacity(&program));
    assert_eq!(storage.scratch_capacity, 0);
    assert!(storage.peak_capacity_bound >= storage.retained_capacity);
    assert_eq!(storage.growths, allocator.attempts);
    assert!(!allocator.observer_trace_overflow);
    for kind in [
        "syntax modules",
        "syntax imports",
        "syntax items",
        "enum declarations",
        "enum declaration variants",
        "enum match arms",
        "syntax functions",
        "syntax records",
        "syntax record fields",
        "syntax parameters",
        "syntax blocks",
        "syntax statements",
        "syntax expressions",
        "syntax expression heights",
        "qualified paths",
        "qualified path segments",
        "qualified value arguments",
        "syntax call arguments",
        "syntax literal fields",
        "array literal elements",
        "syntax unary prefixes",
    ] {
        assert!(
            allocator.trace.iter().any(|event| event.kind == kind),
            "missing {kind}"
        );
    }
    println!(
        "c1b-storage {storage:?} parser-bytes={}",
        size_of::<Parser<'_>>()
    );
    for fail_at in 1..=allocator.attempts {
        let mut failed = Allocator {
            fail_at: Some(fail_at),
            ..Allocator::default()
        };
        failed.observer_trace_bound(1000).unwrap();
        let mut observed = enums::SyntaxStorage::default();
        let error = parse_candidate(&sources, MAX_NODES, &mut failed, &mut observed).unwrap_err();
        assert_eq!(error[0].code, "E0400", "attempt {fail_at}");
        assert_eq!(
            failed.attempts, fail_at,
            "no allocation after resource failure"
        );
        assert!(!failed.trace.last().unwrap().success);
        assert_eq!(observed.scratch_capacity, 0);
    }
}
