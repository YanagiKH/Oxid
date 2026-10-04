use super::*;
use crate::frontend::source::{SourceFileId, SourceMap};

#[test]
fn original_recovery_is_preserved_before_project_grammar_entry() {
    println!("PARSER_LAYOUT {}", std::mem::size_of::<Parser<'_>>());
    // These inputs never enter a new project production. Compare every
    // diagnostic field through both public renderers, including ordering.
    let cases = [
        "fn pub() -> i32 { return 0; }\nfn good() -> i32 { return 1; }\n",
        "struct R { pub: i32 }\nfn pub()->i32{return 0;}\n",
        "struct R { pub pub n: i32 }\nfn pub()->i32{return 0;}\n",
        "struct R { pub",
        "struct R { pub /* trailing trivia */",
        "struct R { n: crate: :T }\nfn pub()->i32{return 0;}\n",
        "struct R { n: crate:/*note*/:T }\nfn pub()->i32{return 0;}\n",
        "fn main()::i32 {}\nfn pub()->i32{return 0;}\n",
        "fn bad(\nmod skipped;\nuse crate::skipped;\npub fn pub()->i32{return 0;}\n",
    ];
    let mut mismatches = Vec::new();
    for text in cases {
        let mut map = SourceMap::new();
        map.add("activation.ox".into(), text.into());
        let file = map.get(SourceFileId(0));
        let diagnostics = |mode| {
            parse_with_mode(file, crate::frontend::lexer::lex(file).unwrap(), mode).unwrap_err()
        };
        let original = diagnostics(SourceMode::OwnedCandidate);
        let project = diagnostics(SourceMode::ProjectCandidate);
        let render = |diagnostics: &[Diagnostic]| {
            diagnostics
                .iter()
                .map(|d| (d.render_json(&map), d.render_human(&map)))
                .collect::<Vec<_>>()
        };
        let original = render(&original);
        let project = render(&project);
        println!("SOURCE {text:?}\nORIGINAL {original:#?}\nPROJECT {project:#?}");
        if original != project {
            mismatches.push(text);
        }
    }
    assert!(
        mismatches.is_empty(),
        "old recovery differences: {mismatches:#?}"
    );
}

#[test]
fn recognized_project_attempts_keep_project_recovery_even_when_denied() {
    // The first grammar error and the two later reserved-name errors are the
    // intentional migration vector, including paths rejected before lowering.
    let cases = [
        ("mod ;", "E0100", "expected module name", ";"),
        ("use crate::;", "E0100", "expected item path segment", ";"),
        (
            "struct R { pub n: }",
            "E0100",
            "expected `bool`, `i32` or `()` type",
            "}",
        ),
        (
            "struct R { n: super::T }",
            "E0101",
            "only absolute item paths beginning with `crate::` are supported",
            "super",
        ),
        (
            "struct R { n: self::T }",
            "E0101",
            "only absolute item paths beginning with `crate::` are supported",
            "self",
        ),
        (
            "fn f()->(){ self::f(); }",
            "E0101",
            "only absolute item paths beginning with `crate::` are supported",
            "self",
        ),
        (
            "fn f()->(){ g(&crate::x); }",
            "E0101",
            "qualified borrow places are unavailable in typed-preview",
            ":",
        ),
        (
            "fn f()->(){ x.crate::n; }",
            "E0101",
            "qualified field names are unavailable in typed-preview",
            ":",
        ),
    ];
    for (prefix, code, message, spelling) in cases {
        let text = format!("{prefix}\nfn pub()->i32{{return 0;}}\nfn good()->i32{{return 1;}}\n");
        let mut map = SourceMap::new();
        map.add("activation.ox".into(), text.clone());
        let file = map.get(SourceFileId(0));
        let diagnostics = parse_with_mode(
            file,
            crate::frontend::lexer::lex(file).unwrap(),
            SourceMode::ProjectCandidate,
        )
        .unwrap_err();
        let first = text.find(spelling).unwrap();
        let reserved = text.find("fn pub").unwrap() + 3;
        let expected = [
            (code, message, first, first + spelling.len()),
            (
                "E0101",
                "unsupported typed-preview construct `pub`",
                reserved,
                reserved + 3,
            ),
            (
                "E0101",
                "unsupported typed-preview construct `pub`",
                reserved,
                reserved + 3,
            ),
        ];
        assert_eq!(diagnostics.len(), expected.len(), "{text}");
        for (diagnostic, (code, message, start, end)) in diagnostics.iter().zip(expected) {
            assert_eq!(diagnostic.code, code, "{text}");
            assert_eq!(diagnostic.stage, "parse", "{text}");
            assert_eq!(diagnostic.message, message, "{text}");
            assert_eq!(diagnostic.primary, Some(file.span(start, end)), "{text}");
            assert!(diagnostic.secondary.is_empty(), "{text}");
            assert!(diagnostic.notes.is_empty(), "{text}");
        }
    }
}

#[test]
fn successful_original_syntax_retains_ast_local_ids_nodes_and_provenance() {
    for text in [
        "",
        "// pub mod use crate::\r\nfn crate()->i32{return 2;} fn as()->i32{return crate();}",
        "struct R { n:i32 } fn main()->i32{let r:R=R{n:5};return r.n;}",
        "fn bool()->bool{return true;} fn i32()->i32{return 1;}",
    ] {
        let mut map = SourceMap::new();
        map.add("activation.ox".into(), text.into());
        let file = map.get(SourceFileId(0));
        let parse = |mode| {
            parse_counted(
                file,
                crate::frontend::lexer::lex(file).unwrap(),
                mode,
                MAX_NODES,
                &mut Allocator::default(),
            )
            .unwrap()
        };
        let (original, original_nodes) = parse(SourceMode::OwnedCandidate);
        let (project, project_nodes) = parse(SourceMode::ProjectCandidate);
        assert_eq!(original_nodes, project_nodes, "{text}");
        assert_eq!(format!("{original:?}"), format!("{project:?}"), "{text}");
        assert!(original.belongs_to(file));
        assert!(project.belongs_to(file));
        assert!(!project.uses_project_syntax());
    }
}
