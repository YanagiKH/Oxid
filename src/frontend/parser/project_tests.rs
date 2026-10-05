use super::*;
use crate::frontend::source::{SourceFileId, SourceMap};

fn source(text: &str) -> SourceMap {
    let mut map = SourceMap::new();
    map.add("candidate.ox".into(), text.into());
    map
}
fn candidate(map: &SourceMap) -> Program {
    let source = map.get(SourceFileId(0));
    parse_with_mode(
        source,
        super::super::lexer::lex(source).unwrap(),
        SourceMode::ProjectCandidate,
    )
    .unwrap()
}
fn error(text: &str, mode: SourceMode) -> Diagnostic {
    let map = source(text);
    let file = map.get(SourceFileId(0));
    parse_with_mode(file, super::super::lexer::lex(file).unwrap(), mode)
        .unwrap_err()
        .remove(0)
}
fn counted(
    text: &str,
    limit: usize,
    allocator: &mut Allocator,
) -> Result<(Program, usize), Vec<Diagnostic>> {
    let map = source(text);
    let file = map.get(SourceFileId(0));
    parse_counted(
        file,
        super::super::lexer::lex(file).unwrap(),
        SourceMode::ProjectCandidate,
        limit,
        allocator,
    )
}

#[test]
fn private_project_grammar_preserves_path_origins_and_visibility() {
    let map = source("pub mod child; use crate::child::R as Alias; pub struct R { pub n:i32, flag:bool } pub fn take(x:&mut crate::child::R)->crate::child::R { let value:crate::child::R = crate::child::R { n:1, flag:true }; crate::child::f(&mut value); return value; }");
    let p = candidate(&map);
    assert!(p.uses_project_syntax());
    assert!(p.belongs_to(map.get(SourceFileId(0))));
    assert!(p.validate_spans_and_ids(|span| map.try_text(span).is_some()));
    assert!(matches!(
        p.items.as_slice(),
        [
            ItemId::Module(0),
            ItemId::Import(0),
            ItemId::Struct(0),
            ItemId::Function(0)
        ]
    ));
    assert!(p.modules[0].public.is_some());
    assert!(p.functions[0].public.is_some());
    assert!(p.records[0].public.is_some());
    assert!(p.records[0].fields[0].public.is_some());
    assert!(p.records[0].fields[1].public.is_none());
    assert_eq!(map.text(p.imports[0].alias), "Alias");
    for path in &p.paths {
        assert!(map.text(path.span).starts_with("crate::child::"));
    }
    assert_eq!(p.paths.len(), 6);
    assert_eq!(p.path_segments.len(), 18);
}

#[test]
fn contextual_words_remain_identifiers_and_import_aliases() {
    let map = source("fn crate()->i32{return 1;} fn as()->i32{return 2;} fn self()->i32{return 3;} struct crate { as:i32 } use crate::as as crate; fn f()->(){let crate:i32=1; let as:i32=crate; crate::crate::as(); crate::self::x(); return;}");
    let p = candidate(&map);
    assert_eq!(map.text(p.functions[0].name), "crate");
    assert_eq!(map.text(p.functions[1].name), "as");
    assert_eq!(map.text(p.functions[2].name), "self");
    assert_eq!(map.text(p.records[0].name), "crate");
    assert_eq!(map.text(p.imports[0].alias), "crate");
    assert_eq!(
        p.path_segments(PathId(1))
            .unwrap()
            .iter()
            .map(|span| map.text(*span))
            .collect::<Vec<_>>(),
        ["crate", "crate", "as"]
    );
    assert_eq!(
        error(
            "fn f()->i32{return 5 as i32;}",
            SourceMode::ProjectCandidate
        )
        .code,
        "E0101"
    );
}

#[test]
fn double_colon_requires_adjacency_without_new_token_kind() {
    let map = source("use crate /*a*/ :: /*b*/ self ::x;");
    let p = candidate(&map);
    assert_eq!(
        p.tokens
            .iter()
            .filter(|token| token.kind == Kind::Colon)
            .count(),
        4
    );
    assert_eq!(p.path_segments(PathId(0)).unwrap().len(), 3);
    for text in [
        "use crate: :f;",
        "use crate:/*split*/:f;",
        "use crate::m: :f;",
    ] {
        let d = error(text, SourceMode::ProjectCandidate);
        assert_eq!(d.code, "E0100", "{text}");
        assert_eq!(d.primary.unwrap().end - d.primary.unwrap().start, 1);
    }
    for mode in [
        SourceMode::OwnedCandidate,
        SourceMode::ModuleCandidate,
        SourceMode::ProjectCandidate,
    ] {
        let d = error("fn main()::i32 {}", mode);
        assert_eq!(
            (
                d.code,
                d.message.as_str(),
                d.primary.unwrap().start,
                d.primary.unwrap().end
            ),
            (
                "E0100",
                "function requires an explicit return type after `->`",
                9,
                10
            )
        );
    }
}

#[test]
fn private_extensions_do_not_open_compatibility_modes() {
    for text in [
        "use crate::f;",
        "pub fn f()->(){return;}",
        "pub struct R {}",
        "struct R {pub n:i32}",
        "fn f()->(){crate::f();return;}",
    ] {
        for mode in [SourceMode::OwnedCandidate, SourceMode::ModuleCandidate] {
            let _ = error(text, mode);
        }
    }
    let map = source("pub mod child;");
    let file = map.get(SourceFileId(0));
    assert!(parse_with_mode(
        file,
        super::super::lexer::lex(file).unwrap(),
        SourceMode::ModuleCandidate
    )
    .is_ok());
    assert_eq!(
        error("pub mod child;", SourceMode::OwnedCandidate).code,
        "E0101"
    );
}

#[test]
fn unsupported_project_shapes_stay_closed() {
    for text in [
        "pub use crate::f;",
        "pub let value:i32=1;",
        "pub(crate) fn f()->(){return;}",
        "pub pub fn f()->(){return;}",
        "mod child {}",
        "use crate::*;",
        "use crate::{f};",
        "use {crate::f};",
        "use *;",
        "struct R {n:self::R}",
        "fn f()->(){mod child;return;}",
        "fn f()->(){use crate::f;return;}",
        "fn f()->(){crate::f;return;}",
        "fn f()->(){m::f();return;}",
        "fn f()->(){self::f();return;}",
        "fn f()->(){super::f();return;}",
    ] {
        assert_eq!(
            error(text, SourceMode::ProjectCandidate).code,
            "E0101",
            "{text}"
        );
    }
    for text in [
        "fn f()->(){g(&crate::state);return;}",
        "fn f()->(){crate::state=1;return;}",
        "fn f()->(){crate::state.n;return;}",
        "struct R {n:super::R}",
        "fn f()->(){if crate::R {} { return; } return;}",
    ] {
        let _ = error(text, SourceMode::ProjectCandidate);
    }
}

#[test]
fn qualified_borrow_places_are_private_unsupported_syntax_only() {
    for borrow in [
        "&crate::x",
        "&mut crate::x",
        "&*crate::x",
        "&mut *crate::x",
        "&crate /*before*/ :: /*after*/ x",
    ] {
        let text = format!("fn f()->(){{g({borrow});return;}}");
        let at = text.find("::").unwrap();
        let d = error(&text, SourceMode::ProjectCandidate);
        assert_eq!((d.code, d.stage), ("E0101", "parse"), "{borrow}");
        assert_eq!(d.primary.unwrap().start, at);
        assert_eq!(d.primary.unwrap().end, at + 1);
        for mode in [SourceMode::OwnedCandidate, SourceMode::ModuleCandidate] {
            let d = error(&text, mode);
            assert_eq!(
                (d.code, d.stage, d.message.as_str()),
                (
                    "E0100",
                    "parse",
                    "borrow must be the complete call argument"
                )
            );
            assert_eq!(d.primary.unwrap().start, at);
            assert_eq!(d.primary.unwrap().end, at + 1);
        }
    }
    for borrow in ["&crate: :x", "&crate:/*split*/:x"] {
        let text = format!("fn f()->(){{g({borrow});return;}}");
        let d = error(&text, SourceMode::ProjectCandidate);
        assert_eq!(
            (d.code, d.stage, d.message.as_str()),
            (
                "E0100",
                "parse",
                "borrow must be the complete call argument"
            )
        );
    }
    let map = source("fn f()->(){g(&crate);g(&mut crate);g(&*crate);g(&mut *crate);return;}");
    let file = map.get(SourceFileId(0));
    for mode in [
        SourceMode::OwnedCandidate,
        SourceMode::ModuleCandidate,
        SourceMode::ProjectCandidate,
    ] {
        let p = parse_with_mode(file, super::super::lexer::lex(file).unwrap(), mode).unwrap();
        assert!(!p.uses_project_syntax());
        assert!(p.paths.is_empty());
    }
}

#[test]
fn qualified_projection_fields_are_private_unsupported_syntax_only() {
    for projection in [
        "x.crate::n;",
        "x.crate::n = 1;",
        "x.crate /*before*/ :: /*after*/ n;",
        "x.crate /*before*/ :: /*after*/ n = 1;",
    ] {
        let text = format!("fn f()->(){{{projection}return;}}");
        let at = text.find("::").unwrap();
        let d = error(&text, SourceMode::ProjectCandidate);
        assert_eq!((d.code, d.stage), ("E0101", "parse"), "{projection}");
        assert_eq!(d.primary.unwrap().start, at);
        assert_eq!(d.primary.unwrap().end, at + 1);
        for mode in [SourceMode::OwnedCandidate, SourceMode::ModuleCandidate] {
            let d = error(&text, mode);
            assert_eq!(
                (d.code, d.stage, d.message.as_str()),
                ("E0100", "parse", "statement requires `;`")
            );
            assert_eq!(d.primary.unwrap().start, at);
            assert_eq!(d.primary.unwrap().end, at + 1);
        }
    }
    for projection in [
        "x.crate: :n;",
        "x.crate:/*split*/:n;",
        "x.crate: :n = 1;",
        "x.crate:/*split*/:n = 1;",
    ] {
        let text = format!("fn f()->(){{{projection}return;}}");
        let d = error(&text, SourceMode::ProjectCandidate);
        assert_eq!(
            (d.code, d.stage, d.message.as_str()),
            ("E0100", "parse", "statement requires `;`")
        );
    }
    let map = source("fn f()->(){x.crate;x.crate=1;return;}");
    let file = map.get(SourceFileId(0));
    for mode in [
        SourceMode::OwnedCandidate,
        SourceMode::ModuleCandidate,
        SourceMode::ProjectCandidate,
    ] {
        let p = parse_with_mode(file, super::super::lexer::lex(file).unwrap(), mode).unwrap();
        assert!(!p.uses_project_syntax());
        assert!(matches!(p.expressions[0].kind, ExprKind::FieldRead { .. }));
        assert!(matches!(
            p.functions[0].blocks[0].body[1].kind,
            StmtKind::FieldAssign { .. }
        ));
    }
}

#[test]
fn missing_path_and_import_punctuation_is_syntax_error() {
    for text in [
        "use crate;",
        "use crate::;",
        "use crate::f as ;",
        "use crate::f",
        "fn f(x:crate::)->(){return;}",
    ] {
        assert_eq!(
            error(text, SourceMode::ProjectCandidate).code,
            "E0100",
            "{text}"
        );
    }
}

#[test]
fn path_segment_limit_precedes_global_node_limit_at_same_segment() {
    let q34 = format!("use crate{};", "::x".repeat(33));
    let (_, nodes) = counted(&q34, 35, &mut Allocator::default()).unwrap();
    assert_eq!(nodes, 35); // One import plus contextual crate and 33 endpoint segments.
    let d = counted(&q34, 34, &mut Allocator::default())
        .unwrap_err()
        .remove(0);
    assert_eq!(d.message, "syntax node limit exceeded");
    let q35 = format!("use crate{};", "::x".repeat(34));
    for limit in [35, MAX_NODES] {
        let mut allocator = Allocator::default();
        let d = counted(&q35, limit, &mut allocator).unwrap_err().remove(0);
        assert_eq!(d.message, "qualified path segment limit exceeded");
        assert_eq!(d.primary.unwrap().start, q35.len() - 2);
        assert_eq!(
            allocator
                .trace
                .iter()
                .filter(|event| event.kind == "absolute path segments")
                .count(),
            34
        );
    }
}

#[test]
fn public_modifiers_add_no_syntax_nodes() {
    let plain = "struct R {n:i32} fn f()->(){return;}";
    let public = "pub struct R {pub n:i32} pub fn f()->(){return;}";
    let (_, a) = counted(plain, MAX_NODES, &mut Allocator::default()).unwrap();
    let (_, b) = counted(public, MAX_NODES, &mut Allocator::default()).unwrap();
    assert_eq!(a, b);
    assert_eq!(a, 4);
}

#[test]
fn every_new_path_import_reserve_is_fallible() {
    let input = "use crate::child::f as alias;";
    let mut allocator = Allocator::default();
    counted(input, MAX_NODES, &mut allocator).unwrap();
    assert_eq!(
        allocator
            .trace
            .iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>(),
        [
            "absolute path segments",
            "absolute path segments",
            "absolute path segments",
            "absolute paths",
            "import declarations",
            "import items"
        ]
    );
    for fail_at in 1..=allocator.attempts {
        let mut allocator = Allocator {
            fail_at: Some(fail_at),
            ..Allocator::default()
        };
        let d = counted(input, MAX_NODES, &mut allocator)
            .unwrap_err()
            .remove(0);
        assert_eq!(
            (d.code, d.stage, d.message.as_str()),
            ("E0400", "parse", "project syntax allocation failed")
        );
        assert_eq!(allocator.attempts, fail_at);
        assert!(!allocator.trace.last().unwrap().success);
    }
}

#[test]
fn minimal_import_all_five_reserve_positions_fail_with_original_origins() {
    let input = "use crate::f;";
    // Two segment appends plus the path, import and lexical-item appends.
    // Expected origins follow the grammar's token/declaration spans.
    let expected = [
        (
            "absolute path segments",
            1,
            std::mem::size_of::<super::super::source::Span>(),
            4,
            9,
        ),
        (
            "absolute path segments",
            2,
            std::mem::size_of::<super::super::source::Span>(),
            11,
            12,
        ),
        (
            "absolute paths",
            1,
            std::mem::size_of::<AbsolutePath>(),
            4,
            12,
        ),
        (
            "import declarations",
            1,
            std::mem::size_of::<ImportDecl>(),
            0,
            13,
        ),
        ("import items", 1, std::mem::size_of::<ItemId>(), 0, 13),
    ];
    let mut success = Allocator::default();
    let (p, nodes) = counted(input, 3, &mut success).unwrap();
    assert_eq!(nodes, 3);
    assert_eq!(
        (
            p.paths.len(),
            p.path_segments.len(),
            p.imports.len(),
            p.items.len()
        ),
        (1, 2, 1, 1)
    );
    assert_eq!(success.attempts, 5);
    assert_eq!(success.trace.len(), 5);
    for (event, &(kind, length, bytes, _, _)) in success.trace.iter().zip(&expected) {
        assert_eq!(
            (event.kind, event.length, event.element_bytes, event.success),
            (kind, length, bytes, true)
        );
    }
    for fail_at in 1..=5 {
        let mut allocator = Allocator {
            fail_at: Some(fail_at),
            ..Allocator::default()
        };
        let diagnostics = counted(input, 3, &mut allocator).unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        let d = &diagnostics[0];
        assert_eq!(
            (d.code, d.stage, d.message.as_str()),
            ("E0400", "parse", "project syntax allocation failed")
        );
        let span = d.primary.unwrap();
        let (kind, length, bytes, start, end) = expected[fail_at - 1];
        assert_eq!(
            (span.file, span.start, span.end),
            (SourceFileId(0), start, end)
        );
        assert_eq!(allocator.attempts, fail_at);
        assert_eq!(allocator.trace.len(), fail_at);
        assert!(allocator.trace[..fail_at - 1]
            .iter()
            .all(|event| event.success));
        let event = allocator.trace.last().unwrap();
        assert_eq!(
            (event.kind, event.length, event.element_bytes, event.success),
            (kind, length, bytes, false)
        );
    }
}

#[test]
fn import_and_each_path_node_gate_precede_injected_reserve() {
    // Limits 0/1/2 stop at the import/contextual-root/endpoint node respectively.
    for (limit, attempts, start, end) in [(0, 0, 0, 3), (1, 0, 4, 9), (2, 1, 11, 12)] {
        let mut allocator = Allocator {
            fail_at: Some(attempts + 1),
            ..Allocator::default()
        };
        let diagnostics = counted("use crate::f;", limit, &mut allocator).unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        let d = &diagnostics[0];
        assert_eq!(
            (d.code, d.stage, d.message.as_str()),
            ("E0400", "parse", "syntax node limit exceeded")
        );
        let span = d.primary.unwrap();
        assert_eq!((span.start, span.end), (start, end));
        assert_eq!(allocator.attempts, attempts);
        assert_eq!(allocator.trace.len(), attempts);
        assert!(allocator.trace.iter().all(|event| event.success));
    }
}

#[test]
fn path_q_gate_precedes_injected_reserve_even_at_node_limit() {
    let input = format!("use crate{};", "::x".repeat(34));
    for node_limit in [35, MAX_NODES] {
        let mut allocator = Allocator {
            fail_at: Some(35),
            ..Allocator::default()
        };
        let diagnostics = counted(&input, node_limit, &mut allocator).unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        let d = &diagnostics[0];
        assert_eq!(
            (d.code, d.stage, d.message.as_str()),
            ("E0400", "parse", "qualified path segment limit exceeded")
        );
        let span = d.primary.unwrap();
        assert_eq!((span.start, span.end), (input.len() - 2, input.len() - 1));
        assert_eq!(allocator.attempts, 34);
        assert_eq!(allocator.trace.len(), 34);
        assert!(allocator
            .trace
            .iter()
            .all(|event| event.success && event.kind == "absolute path segments"));
    }
}

#[test]
fn checked_segment_count_overflow_precedes_all_admission_and_reserves() {
    let map = source("crate");
    let file = map.get(SourceFileId(0));
    let mut allocator = Allocator {
        fail_at: Some(1),
        ..Allocator::default()
    };
    let mut parser = Parser {
        source: file,
        allocator: &mut allocator,
        mode: SourceMode::ProjectCandidate,
        arrays: ArraySyntaxPolicy::Closed,
        enums: EnumSyntaxPolicy::Closed,
        storage: enums::SyntaxStorage::default(),
        project_recovery: false,
        tokens: super::super::lexer::lex(file).unwrap(),
        cursor: 0,
        expressions: Vec::new(),
        paths: Vec::new(),
        path_segments: Vec::new(),
        heights: Vec::new(),
        nodes: 0,
        node_limit: 0,
    };
    let mut count = usize::MAX;
    let d = parser
        .path_segment(&mut count, file.span(0, 5))
        .unwrap_err();
    assert_eq!(
        (d.code, d.stage, d.message.as_str()),
        ("E0400", "parse", "project syntax count overflow")
    );
    assert_eq!(d.primary, Some(file.span(0, 5)));
    assert_eq!(parser.nodes, 0);
    assert!(parser.path_segments.is_empty());
    assert_eq!(count, usize::MAX);
    assert_eq!(allocator.attempts, 0);
    assert!(allocator.trace.is_empty());
}

#[test]
fn immutable_source_provenance_and_all_span_owner_checks() {
    let map = source("pub fn f(x:&R)->(){g(&*x); if true {return;} return;} struct R {pub n:i32}");
    let mut p = candidate(&map);
    let file = map.get(SourceFileId(0));
    assert!(p.belongs_to(file));
    assert!(!p.belongs_to(source(file.text()).get(SourceFileId(0))));
    let mut visited = Vec::new();
    assert!(p.validate_spans_and_ids(|span| {
        visited.push(span);
        file.try_text(span).is_some()
    }));
    assert!(visited.len() > p.tokens.len());
    p.functions[0].blocks[0].body[0].span.file = SourceFileId(1);
    assert!(!p.validate_spans_and_ids(|span| file.try_text(span).is_some()));
    p.functions[0].blocks[0].body[0].span.file = SourceFileId(0);
    p.functions[0].body = BodyBlockId(usize::MAX);
    assert!(!p.validate_spans_and_ids(|span| file.try_text(span).is_some()));
}

#[test]
fn source_identity_rejects_reused_empty_buffer_after_original_owner_drops() {
    let (program, empty_address) = {
        let map = source("");
        let address = map.get(SourceFileId(0)).text().as_ptr();
        (candidate(&map), address)
    };
    let replacement = source("");
    let file = replacement.get(SourceFileId(0));
    // Empty strings deterministically share the same dangling byte address.
    assert_eq!(empty_address, file.text().as_ptr());
    assert!(!program.belongs_to(file));
}
