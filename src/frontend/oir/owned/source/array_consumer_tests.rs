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

fn emit(text: &str) -> Result<String, Vec<Diagnostic>> {
    let (sources, ast) = parsed(text);
    program::emit_array_source(
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
fn source_array_consumers_preserve_index_and_allocation_limits() {
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
        let errors = program::emit_array_source(
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
fn source_array_consumers_use_the_supplied_work_meter() {
    let (sources, ast) = parsed("fn main()->i32{let a=[2];return a[0];}");
    let work = WorkMeter::new(0);
    let errors = program::run_array_source(
        owner(&sources, &ast),
        IndexLimits::default(),
        &work,
        &mut Allocator::default(),
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "E0400");
    assert!(work.used() > 0);
    let work = WorkMeter::new(0);
    let errors = program::emit_array_source(
        owner(&sources, &ast),
        IndexLimits::default(),
        &work,
        &mut Allocator::default(),
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "E0400");
    assert!(work.used() > 0);
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

#[test]
fn source_array_native_emits_owned_transfers_borrows_and_checked_indexing() {
    let text = "fn make()->[i32;2]{return [6,9];} \
        fn bump(a:&mut [i32;2])->(){a[1]=a[0]+a[1];return;} \
        fn main()->i32{let mut a=make();bump(&mut a);return a[1];}";
    assert_eq!(run(text).unwrap(), Scalar::I32(15));
    let module = emit(text).unwrap();
    assert!(module.contains("define i32 @main()"));
    assert!(module.contains("@__oxid_owned_fn_"));
    assert!(module.contains("icmp sge i32"));
    assert!(module.contains("icmp slt i32"));
    assert!(module.contains("getelementptr i8"));
    assert!(module.contains(r"\45\30\36\30\36")); // LLVM-escaped E0606
}

#[test]
fn source_array_native_accepts_zero_length_bool_and_unit_arrays() {
    for text in [
        "fn main()->i32{let a:[bool;0]=[];let b=a;return b.len();}",
        "fn main()->bool{let a=[false,true];return a[1];}",
        "fn main()->(){let a=[(),()];return a[0];}",
    ] {
        assert!(emit(text).unwrap().contains("define i32 @main()"), "{text}");
    }
}

#[test]
fn source_array_native_preserves_frontend_failures_and_entry_restrictions() {
    for text in [
        "fn main()->i32{return missing[0];}",
        "fn main()->i32{let a=[1];return a[false];}",
        "fn main()->i32{let a=[1];let b=a;return a[0];}",
    ] {
        let reference = run(text).unwrap_err();
        let native = emit(text).unwrap_err();
        assert_eq!(
            (
                native[0].code,
                native[0].stage,
                &native[0].message,
                native[0].primary
            ),
            (
                reference[0].code,
                reference[0].stage,
                &reference[0].message,
                reference[0].primary
            )
        );
    }
    for text in [
        "fn other()->i32{let a=[1];return a[0];}",
        "fn main(n:i32)->i32{let a=[1];return a[n];}",
        "fn main()->[i32;1]{return [1];}",
    ] {
        assert_eq!(run(text).unwrap_err()[0].code, "E0600");
        assert_eq!(emit(text).unwrap_err()[0].code, "E0700");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn source_array_project_uses_original_root_main_and_child_diagnostic_origins() {
    use crate::frontend::project::{ProjectLimits, ProjectSources};
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "oxid-source-array-consumers-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&directory).unwrap();
    let root = directory.join("main.ox");
    fs::write(
        directory.join("child.ox"),
        "pub fn make()->[i32;2]{return [12,17];} pub fn main()->i32{return 99;}",
    )
    .unwrap();
    for (text, expected) in [
        ("mod child; fn helper()->i32{return 88;} fn main()->i32{let a=crate::child::make();return a[1];}", Some(17)),
        ("mod child; use crate::child::main;", None),
    ] {
        fs::write(&root, text).unwrap();
        let project = ProjectSources::load_array_candidate(root.to_str().unwrap(), ProjectLimits::default(), &mut Allocator::default()).unwrap();
        let result = program::run_array_source(SourceOwner::project(&project), IndexLimits::default(), &WorkMeter::default(), &mut Allocator::default());
        let native = program::emit_array_source(SourceOwner::project(&project), IndexLimits::default(), &WorkMeter::default(), &mut Allocator::default());
        match expected {
            Some(value) => {
                assert_eq!(result.unwrap(), Scalar::I32(value));
                assert!(native.unwrap().contains("define i32 @main()"));
            }
            None => {
                assert_eq!(result.unwrap_err()[0].code, "E0600");
                assert_eq!(native.unwrap_err()[0].code, "E0700");
            }
        }
    }
    fs::write(
        &root,
        "mod child; fn main()->i32{return crate::child::read();}",
    )
    .unwrap();
    fs::write(
        directory.join("child.ox"),
        "pub fn read()->i32{let a=[1];return a[1];}",
    )
    .unwrap();
    let project = ProjectSources::load_array_candidate(
        root.to_str().unwrap(),
        ProjectLimits::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    let errors = program::run_array_source(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "E0606");
    let span = errors[0].primary.unwrap();
    assert_eq!(span.file, SourceFileId(1));
    assert_eq!(project.sources().get(span.file).text_at(span), "a[1]");
    fs::remove_dir_all(directory).unwrap();
}
