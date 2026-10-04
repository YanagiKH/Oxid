//! Hand-authored source programs exercise the existing consumers end to end.
use super::{program, resolve, typeck};
use crate::frontend::{
    ast,
    declaration_index::{IndexLimits, SourceOwner, WorkMeter},
    diagnostic::Diagnostic,
    lexer,
    oir::Scalar,
    parser,
    project::budget::Allocator,
    source::{SourceFileId, SourceMap, SourceView},
};

fn parsed(text: &str) -> (SourceMap, ast::Program) {
    let mut sources = SourceMap::new();
    let file = sources.add("arrays.ox".into(), text.into());
    let source = sources.get(file);
    let (ast, _) = parser::parse_counted_with_arrays(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut Allocator::default(),
        parser::ArraySyntaxPolicy::Candidate,
    )
    .unwrap();
    (sources, ast)
}

fn owner<'s>(sources: &'s SourceMap, ast: &'s ast::Program) -> SourceOwner<'s> {
    SourceOwner::original(sources.get(SourceFileId(0)), ast, SourceView::Map(sources)).unwrap()
}

fn run(text: &str) -> Result<Scalar, Vec<Diagnostic>> {
    let (sources, ast) = parsed(text);
    program::run_array_source(
        owner(&sources, &ast),
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
}

#[test]
fn source_array_reference_reads_writes_moves_and_returns() {
    let text = "fn relay(a:[i32;3])->[i32;3]{return a;} \
        fn main()->i32{let a=[4,7,9];let mut b=relay(a);b[1]=b[0]+b[2];return b[1]+b.len();}";
    assert_eq!(run(text).unwrap(), Scalar::I32(16));
}

#[test]
fn source_array_reference_call_borrows_end_before_the_next_access() {
    let text = "fn bump(a:&mut [i32;2])->(){a[0]=a[0]+a[1];return;} \
        fn read(a:&[i32;2])->i32{return a[0];} \
        fn main()->i32{let mut a=[5,8];bump(&mut a);return read(&a)+a[1];}";
    assert_eq!(run(text).unwrap(), Scalar::I32(21));
}

#[test]
fn source_array_reference_zero_length_and_scalar_elements() {
    for (text, expected) in [
        (
            "fn main()->i32{let a:[i32;0]=[];let b=a;return b.len();}",
            Scalar::I32(0),
        ),
        (
            "fn main()->bool{let mut a=[true,false];a[1]=a[0];return a[1];}",
            Scalar::Bool(true),
        ),
        ("fn main()->(){let a=[(),()];return a[1];}", Scalar::Unit),
    ] {
        assert_eq!(run(text).unwrap(), expected, "{text}");
    }
}

#[test]
fn source_array_reference_bounds_report_the_original_access() {
    for text in [
        "fn main()->i32{let a=[7];return a[-1];}",
        "fn main()->i32{let a=[7];return a[1];}",
        "fn main()->i32{let a:[i32;0]=[];return a[0];}",
    ] {
        let (sources, ast) = parsed(text);
        let errors = program::run_array_source(
            owner(&sources, &ast),
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(
            (errors[0].code, errors[0].stage),
            ("E0606", "oir-owned-run")
        );
        let at = errors[0].primary.unwrap();
        assert!(sources.is_valid_span(at));
        assert!(sources.get(at.file).text_at(at).starts_with("a["));
    }
}

#[test]
fn source_array_reference_preserves_resolution_typing_and_ownership_errors() {
    for (text, code, stage) in [
        ("fn main()->i32{return missing[0];}", "E0200", "resolve"),
        ("fn main()->i32{let a=[1];return a[true];}", "E0300", "type"),
        ("fn main()->i32{let a=[1];let b=a;return a[0];}", "E0310", "ownership"),
        ("fn clash(a:&mut [i32;1],b:&[i32;1])->(){return;} fn main()->(){let mut a=[1];clash(&mut a,&a);return;}", "E0311", "ownership"),
    ] {
        let errors = run(text).unwrap_err();
        assert_eq!((errors[0].code, errors[0].stage), (code, stage), "{text}");
    }
}

#[test]
fn source_array_reference_preserves_index_and_allocation_limits() {
    let (sources, ast) = parsed("fn main()->i32{let a=[1];return a[0];}");
    for limits in [
        IndexLimits {
            retained: 0,
            ..IndexLimits::default()
        },
        IndexLimits {
            scratch: 0,
            ..IndexLimits::default()
        },
        IndexLimits {
            work: 0,
            ..IndexLimits::default()
        },
    ] {
        let errors = program::run_array_source(
            owner(&sources, &ast),
            limits,
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap_err();
        assert_eq!(errors[0].code, "E0400");
    }
    let errors = program::run_array_source(
        owner(&sources, &ast),
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut Allocator {
            fail_at: Some(1),
            ..Allocator::default()
        },
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "E0400");
}

#[test]
fn source_array_consumer_admission_does_not_unlock_source_program() {
    for text in [
        "",
        "fn main()->i32{return 3;}",
        "fn main()->i32{let a=[3];return a[0];}",
    ] {
        let (sources, ast) = parsed(text);
        let work = WorkMeter::default();
        for resolved in [
            resolve::resolve_array_types(
                owner(&sources, &ast),
                IndexLimits::default(),
                &work,
                &mut Allocator::default(),
            ),
            resolve::resolve_array_pipeline(
                owner(&sources, &ast),
                IndexLimits::default(),
                &work,
                &mut Allocator::default(),
            ),
            resolve::resolve_array_consumer(
                owner(&sources, &ast),
                IndexLimits::default(),
                &work,
                &mut Allocator::default(),
            ),
        ] {
            let typed = typeck::check(resolved.unwrap()).unwrap();
            let errors = program::check_typed(&typed).unwrap_err();
            assert_eq!(
                (errors[0].code, errors[0].stage),
                ("E0500", "oir-owned-lower")
            );
        }
    }
}

#[test]
fn source_array_reference_rejects_an_owner_without_a_source_map() {
    let (sources, ast) = parsed("fn main()->i32{let a=[2];return a[0];}");
    let source = sources.get(SourceFileId(0));
    let owner = SourceOwner::original(source, &ast, SourceView::Single(source)).unwrap();
    let errors = program::run_array_source(
        owner,
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "E0500");
}
