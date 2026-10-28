use super::*;
use crate::frontend::{
    declaration_index::{collect_originals, IndexLimits, SourceOwner, WorkMeter},
    source::{SourceFileId, SourceMap, SourceView},
};

fn parse_copy(map: &SourceMap) -> Result<Program, Vec<Diagnostic>> {
    let source = map.get(SourceFileId(0));
    parse_typed_counted(
        source,
        crate::frontend::lexer::lex(source).unwrap(),
        SourceMode::ProjectCandidate,
        MAX_NODES,
        &mut Allocator::default(),
        &mut Default::default(),
    )
    .map(|(program, _)| program)
}

#[test]
fn checked_copy_attribute_is_closed_and_preserves_source_and_formatting() {
    for text in [
        "#[derive(Copy)]struct P{x:i32}fn main()->i32{return 0;}",
        "# /* marker */ [ derive ( Copy ) ] pub struct P<T:Copy>{pub value:T}fn main()->i32{return 0;}",
    ] {
        let mut map = SourceMap::new();
        let id = map.add("copy.ox".into(), text.into());
        let program = parse_copy(&map).unwrap();
        assert!(program.has_parsed_copy_records());
        let mark = program.records[0].derive_copy.unwrap();
        assert!(map.get(id).text_at(mark).starts_with('#'));
        assert!(map.get(id).text_at(mark).ends_with(']'));
        assert!(program.belongs_to(map.get(id)));
        let formatted = crate::frontend::format::format_source(map.get(id));
        assert!(formatted.is_ok(), "{formatted:?}");
    }
    for text in [
        "#[derive(Clone)]struct P{x:i32}",
        "#[derive(Copy,Clone)]struct P{x:i32}",
        "#[derive(Copy)]#[derive(Copy)]struct P{x:i32}",
        "#[derive(Copy)]enum E{Empty}",
        "#[derive(Copy)]fn main()->i32{return 0;}",
        "#[Copy]struct P{x:i32}",
        "#[derive(Copy)]struct P{x:&i32}",
    ] {
        let mut map = SourceMap::new();
        map.add("invalid-copy.ox".into(), text.into());
        assert!(parse_copy(&map).is_err(), "{text}");
    }
}

#[test]
fn checked_copy_marks_cannot_be_added_removed_moved_or_rewritten_in_public_ast() {
    for mutation in 0..6 {
        let text = if mutation == 0 {
            "struct P{x:i32}fn main()->i32{return 0;}"
        } else {
            "#[derive(Copy)]struct P{x:i32}struct Q{x:i32}fn main()->i32{return 0;}"
        };
        let mut map = SourceMap::new();
        let id = map.add("copy-authority.ox".into(), text.into());
        let source = map.get(id);
        let mut program = parse_copy(&map).unwrap();
        match mutation {
            0 => program.records[0].derive_copy = Some(program.records[0].name),
            1 => program.records[0].derive_copy = None,
            2 => {
                program.records[1].derive_copy = program.records[0].derive_copy.take();
            }
            3 => program.records[0].derive_copy = Some(program.records[0].name),
            4 => program.records[1].derive_copy = program.records[0].derive_copy,
            5 => {
                program.records.clear();
                program
                    .items
                    .retain(|item| !matches!(item, ItemId::Struct(_)));
                assert!(program.uses_owned_syntax(source));
            }
            _ => unreachable!(),
        }
        assert!(
            program.belongs_to(source),
            "cheap provenance does not walk mutable declarations"
        );
        let owner = SourceOwner::original(source, &program, SourceView::Map(&map)).unwrap();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let errors = match collect_originals(owner, IndexLimits::default(), &work, &mut allocator) {
            Err(error) => vec![*error],
            Ok(facts) => facts.finish(&work, &mut allocator).unwrap_err(),
        };
        assert_eq!(errors[0].code, "E0500", "mutation={mutation}: {errors:?}");
    }
}

#[test]
fn checked_copy_attribute_does_not_open_closed_or_historical_candidate_grammars() {
    let mut map = SourceMap::new();
    let id = map.add(
        "closed-copy.ox".into(),
        "#[derive(Copy)]struct P{x:i32}fn main()->i32{return 0;}".into(),
    );
    let source = map.get(id);
    assert!(parse_counted(
        source,
        crate::frontend::lexer::lex(source).unwrap(),
        SourceMode::OwnedCandidate,
        MAX_NODES,
        &mut Allocator::default()
    )
    .is_err());
    assert!(parse_counted_with_arrays(
        source,
        crate::frontend::lexer::lex(source).unwrap(),
        SourceMode::OwnedCandidate,
        MAX_NODES,
        &mut Allocator::default(),
        ArraySyntaxPolicy::Candidate
    )
    .is_err());
    assert!(parse_builtin_candidate_counted(
        source,
        crate::frontend::lexer::lex(source).unwrap(),
        SourceMode::ProjectCandidate,
        MAX_NODES,
        &mut Allocator::default(),
        &mut Default::default()
    )
    .is_err());
    assert!(parse_output_candidate_counted(
        source,
        crate::frontend::lexer::lex(source).unwrap(),
        SourceMode::ProjectCandidate,
        MAX_NODES,
        &mut Allocator::default(),
        &mut Default::default()
    )
    .is_err());
}
