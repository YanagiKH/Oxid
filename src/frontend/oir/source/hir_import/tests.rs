use super::*;
use crate::frontend::{
    lexer, parser,
    project::{ProjectLimits, ProjectSources, SyntaxFlavor},
    source::{SourceMap, SourceView},
};

fn frame(source_len: usize) -> [u8; SUCCESS_BYTES] {
    let mut bytes = [0; SUCCESS_BYTES];
    bytes[..4].copy_from_slice(b"OPA1");
    bytes[10] = u8::try_from(source_len).unwrap();
    bytes[OPA_BYTES..OPA_BYTES + 4].copy_from_slice(b"STF1");
    bytes
}
fn parsed(text: &str) -> (SourceMap, crate::frontend::ast::Program) {
    let mut sources = SourceMap::new();
    let id = sources.add("import-probe.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    (sources, ast)
}
fn original<'s>(sources: &'s SourceMap, ast: &'s crate::frontend::ast::Program) -> SourceOwner<'s> {
    SourceOwner::original(
        sources.get(crate::frontend::source::SourceFileId(0)),
        ast,
        SourceView::Map(sources),
    )
    .unwrap()
}

fn project(text: &str) -> ProjectSources {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("oxid-hir-import-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.clone());
    let file = directory.join("main.ox");
    std::fs::write(&file, text).unwrap();
    ProjectSources::load_typed(file.to_str().unwrap(), ProjectLimits::default()).unwrap()
}

#[test]
fn checked_hir_import_empty_valid_success_still_denied() {
    let (sources, ast) = parsed("");
    let result = denied_probe(original(&sources, &ast), b"", &frame(0));
    let Err(Rejection::Disabled(facts)) = result else {
        panic!("private import must remain denied")
    };
    assert_eq!(facts.requested, Counts::default());
    assert_eq!(facts.candidate_request_bytes, 0);
    assert_eq!(facts.canonical_payload_bytes, 0);
    assert_eq!(
        facts.hypothetical_hir_pair_bytes,
        2 * size_of::<hir::Program>()
    );
}

#[test]
fn checked_hir_import_framing_and_inactive_controls() {
    let bytes = frame(0);
    for length in [0, 3, OPA_BYTES, SUCCESS_BYTES - 1] {
        assert!(matches!(
            Wire::decode(&bytes[..length], 0),
            Err(Boundary::Frame)
        ));
    }
    let mut extra = bytes.to_vec();
    extra.push(0);
    assert!(matches!(Wire::decode(&extra, 0), Err(Boundary::Frame)));
    for index in [
        0,
        4,
        5,
        6,
        7,
        8,
        9,
        10,
        OPA_BYTES,
        OPA_BYTES + 4,
        OPA_BYTES + 5,
        OPA_BYTES + 6,
        SUCCESS_BYTES - 1,
    ] {
        let mut changed = bytes;
        changed[index] = 1;
        assert!(
            matches!(Wire::decode(&changed, 0), Err(Boundary::Frame)),
            "byte {index}"
        );
    }
    for column in 0..5 {
        for cell in [0, 128] {
            for plane in 0..4 {
                let mut changed = bytes;
                changed[COLUMN_STARTS[column] + plane * CELLS + cell] = 1;
                assert!(matches!(Wire::decode(&changed, 0), Err(Boundary::Frame)));
            }
        }
    }
    let mut changed = bytes;
    changed[8] = 129;
    changed[OPA_BYTES + 5] = 129;
    assert!(matches!(Wire::decode(&changed, 0), Err(Boundary::Frame)));
}

#[test]
fn checked_hir_import_active_words_are_untrusted_signed_data() {
    let mut bytes = frame(0);
    bytes[8] = 1;
    bytes[9] = 1;
    bytes[OPA_BYTES + 5] = 1;
    for value in [i32::MIN, -1, 0, i32::MAX] {
        for (plane, byte) in value.to_le_bytes().into_iter().enumerate() {
            bytes[COLUMN_STARTS[3] + plane * CELLS] = byte;
        }
        let wire = Wire::decode(&bytes, 0).unwrap();
        assert_eq!(wire.word(3, 0).unwrap(), value);
        assert_eq!(wire.word(5, 0), Err(Boundary::Frame));
        assert_eq!(wire.word(0, CELLS), Err(Boundary::Frame));
    }
    // Passing these framing checks does not validate active rows or permit import.
    let (sources, ast) = parsed("");
    assert!(matches!(
        denied_probe(original(&sources, &ast), b"", &bytes),
        Err(Rejection::Disabled(_))
    ));
}

#[test]
fn checked_hir_import_real_owner_and_source_checks_precede_wire() {
    let (sources, ast) = parsed("");
    assert!(matches!(
        denied_probe(original(&sources, &ast), b"x", b""),
        Err(Rejection::Boundary(Boundary::Source))
    ));
    let (replacement, _) = parsed("");
    let file = replacement.get(crate::frontend::source::SourceFileId(0));
    assert!(SourceOwner::original(file, &ast, SourceView::Map(&replacement)).is_err());
    let file = sources.get(crate::frontend::source::SourceFileId(0));
    assert!(SourceOwner::original(file, &ast, SourceView::Map(&replacement)).is_err());
    for text in [" ".repeat(129), "// é".to_owned()] {
        let (sources, ast) = parsed(&text);
        assert!(matches!(
            denied_probe(original(&sources, &ast), text.as_bytes(), b""),
            Err(Rejection::Boundary(Boundary::Domain))
        ));
    }
}

const RICH: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import/rich-source.txt"
));
const RICH_WIRE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import/rich-success.bin"
));
#[test]
fn checked_hir_import_canonical_storage_is_observed_but_never_imported() {
    let (sources, ast) = parsed(RICH);
    // Deliberately incomplete active rows: this is a passive canonical-HIR
    // measurement, not a claim that this frame describes RICH.
    let Err(Rejection::Disabled(facts)) = denied_probe(
        original(&sources, &ast),
        RICH.as_bytes(),
        &frame(RICH.len()),
    ) else {
        panic!("denial required")
    };
    assert_eq!(facts.requested, Counts([2, 2, 1, 2, 11, 5, 7, 2]));
    assert!(facts.canonical_payload_bytes >= facts.candidate_request_bytes);
    assert_eq!(
        facts.hypothetical_hir_pair_bytes,
        facts.canonical_payload_bytes
            + facts.candidate_request_bytes
            + 2 * size_of::<hir::Program>()
    );
    let text = "fn f()->i32{return true;}";
    let (sources, ast) = parsed(text);
    assert!(matches!(
        denied_probe(
            original(&sources, &ast),
            text.as_bytes(),
            &frame(text.len())
        ),
        Err(Rejection::Disabled(_))
    ));
    // Above is ill-typed: the precursor must not have reached the type checker.
}

#[test]
fn checked_hir_import_public_source_uses_genuine_project_owner() {
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/public-source.txt"
    ));
    let public_wire = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/public-success.bin"
    ));
    let project = project(text);
    assert_eq!(project.syntax_flavor(), SyntaxFlavor::ProjectSyntax);
    let owner = SourceOwner::project(&project);
    let source = owner.file(ModuleId(0)).unwrap();
    let ast = owner.ast(ModuleId(0)).unwrap();
    let forced_original =
        SourceOwner::original(source, ast, SourceView::Map(project.sources())).unwrap();
    assert!(matches!(
        denied_probe(forced_original, text.as_bytes(), b""),
        Err(Rejection::Boundary(Boundary::Source))
    ));

    let Err(Rejection::Disabled(facts)) = denied_probe(owner, text.as_bytes(), public_wire) else {
        panic!("denial required")
    };
    assert_eq!(facts.requested, Counts([2, 2, 1, 2, 11, 5, 7, 2]));
}

#[test]
fn checked_hir_import_plan_counts_capacity_and_checked_arithmetic() {
    let program = hir::Program {
        signatures: Vec::with_capacity(7),
        functions: Vec::with_capacity(9),
    };
    let plan = StoragePlan::describe(&program).unwrap();
    assert_eq!(plan.requested, Counts::default());
    assert_eq!(plan.canonical_capacity.0[0], program.signatures.capacity());
    assert_eq!(plan.canonical_capacity.0[1], program.functions.capacity());
    assert_eq!(
        plan.canonical_payload_bytes,
        program.signatures.capacity() * size_of::<hir::Signature>()
            + program.functions.capacity() * size_of::<hir::Function>()
    );
    assert!(plan.canonical_payload_bytes > 0);
    let mut counts = Counts([usize::MAX; 8]);
    assert_eq!(counts.add(0, 1), Err(Boundary::Overflow));
    assert_eq!(counts.payload_bytes(), Err(Boundary::Overflow));
}

#[test]
fn checked_hir_import_actual_layouts() {
    println!("HIR_IMPORT_LAYOUT owner={} wire={} bound={} counts={} plan={} facts={} rejection={} result={} source_result={} plan_result={} program={} elements={:?}",
        size_of::<SourceOwner<'static>>(), size_of::<Wire<'static>>(), size_of::<BoundObservation<'static, 'static>>(), size_of::<Counts>(), size_of::<StoragePlan>(), size_of::<ProbeFacts>(), size_of::<Rejection>(), size_of::<Result<Infallible, Rejection>>(), size_of::<Result<BoundObservation<'static, 'static>, Boundary>>(), size_of::<Result<StoragePlan, Boundary>>(), size_of::<hir::Program>(), ELEMENT_BYTES);
    assert_eq!(
        size_of::<Result<Infallible, Rejection>>(),
        size_of::<Rejection>()
    );
}

#[test]
fn checked_hir_import_actual_producer_frame_has_distinct_count_and_head() {
    assert_eq!(
        (
            RICH_WIRE.len(),
            RICH_WIRE[8],
            RICH_WIRE[9],
            RICH_WIRE[OPA_BYTES + 5]
        ),
        (2607, 29, 1, 29)
    );
    let wire = Wire::decode(RICH_WIRE, RICH.len()).unwrap();
    assert_eq!(wire.rows, 29);
    let (sources, ast) = parsed(RICH);
    let owner = original(&sources, &ast);
    let bound = BoundObservation::bind(owner, RICH.as_bytes(), RICH_WIRE).unwrap();
    assert!(std::ptr::eq(
        bound.source.as_ptr(),
        sources
            .get(crate::frontend::source::SourceFileId(0))
            .text()
            .as_ptr()
    ));
    let Err(Rejection::Disabled(facts)) = denied_probe(owner, RICH.as_bytes(), RICH_WIRE) else {
        panic!("denial required")
    };
    assert_eq!(facts.rows, 29);
    assert_eq!(facts.requested, Counts([2, 2, 1, 2, 11, 5, 7, 2]));
    let mut changed = RICH_WIRE.to_vec();
    changed.swap(8, 9);
    assert!(matches!(
        Wire::decode(&changed, RICH.len()),
        Err(Boundary::Frame)
    ));
    for head in [0, 30, 255] {
        let mut changed = RICH_WIRE.to_vec();
        changed[9] = head;
        assert!(matches!(
            Wire::decode(&changed, RICH.len()),
            Err(Boundary::Frame)
        ));
    }
}

#[test]
fn checked_hir_import_excluded_ast_domains_do_not_reach_resolution() {
    for text in [
        "struct S{}",
        "enum E{A}",
        "fn f(x:&i32)->(){return;}",
        "fn f()->(){let x=[1];return;}",
        "fn f()->(){let x=1;g(&x);return;}",
    ] {
        let project = project(text);
        assert!(
            matches!(
                denied_probe(SourceOwner::project(&project), text.as_bytes(), b""),
                Err(Rejection::Boundary(Boundary::Domain))
            ),
            "{text}"
        );
    }
}

#[test]
fn checked_hir_import_complete_result_layout_observations() {
    use std::mem::align_of;
    macro_rules! layout {
        ($name:literal, $ty:ty) => {
            println!(
                "HIR_IMPORT_CARRIER {} size={} align={}",
                $name,
                size_of::<$ty>(),
                align_of::<$ty>()
            );
        };
    }
    layout!("wire-result", Result<Wire<'static>, Boundary>);
    layout!(
        "bound-result",
        Result<BoundObservation<'static, 'static>, Boundary>
    );
    layout!("canonical-result", Result<hir::Program, Vec<Diagnostic>>);
    layout!("mapped-canonical-result", Result<hir::Program, Rejection>);
    layout!(
        "owner-file-result",
        Result<&'static crate::frontend::source::SourceFile, Box<Diagnostic>>
    );
    layout!(
        "owner-ast-result",
        Result<&'static crate::frontend::ast::Program, Box<Diagnostic>>
    );
    layout!("plan-result", Result<StoragePlan, Boundary>);
    layout!("word-result", Result<i32, Boundary>);
    layout!("scalar-result", Result<usize, Boundary>);
    layout!("probe-result", Result<Infallible, Rejection>);
    layout!(
        "existing-checked-source",
        crate::frontend::oir::source::CheckedSourceProgram<'static>
    );
    let (sources, ast) = parsed(RICH);
    let canonical = hir::resolve_sources(original(&sources, &ast)).unwrap();
    let plan = StoragePlan::describe(&canonical).unwrap();
    println!("HIR_IMPORT_RETAINED {:?}", plan);
}

#[test]
fn checked_hir_import_original_route_rejects_stale_public_summary() {
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/public-source.txt"
    ));
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/public-success.bin"
    ));
    let mut sources = SourceMap::new();
    let id = sources.add("stale-public-summary.ox".into(), text.into());
    let file = sources.get(id);
    let genuine = lexer::lex(file).unwrap();
    let mut altered = genuine.clone();
    for token in &mut altered {
        if token.kind == lexer::Kind::Pub {
            token.kind = lexer::Kind::Trivia;
        }
    }
    let mut program = parser::parse(file, altered).unwrap();
    assert!(!program.uses_project_syntax());
    program.functions[0].public = Some(
        genuine
            .iter()
            .find(|t| t.kind == lexer::Kind::Pub)
            .unwrap()
            .span,
    );
    program.tokens = genuine;
    assert!(matches!(
        BoundObservation::bind(original(&sources, &program), text.as_bytes(), bytes),
        Err(Boundary::Source)
    ));
}

#[test]
fn checked_hir_import_scan_preflight_bounds_mutable_arenas() {
    let text = "fn f(x:i32)->i32{return x;}";
    for kind in 0..3 {
        let (sources, mut program) = parsed(text);
        match kind {
            0 => {
                let token = program.tokens[0].clone();
                program.tokens.resize(CELLS + 1, token);
            }
            1 => program.items.resize(MAX_ROWS + 1, ast::ItemId::Function(0)),
            _ => {
                let name = program.functions[0].params[0].name;
                let ty = program.functions[0].params[0].ty;
                program.functions[0]
                    .params
                    .resize_with(MAX_ROWS + 1, || ast::Param { name, ty });
            }
        }
        assert!(matches!(
            BoundObservation::bind(
                original(&sources, &program),
                text.as_bytes(),
                &frame(text.len())
            ),
            Err(Boundary::Domain)
        ));
    }
    let text = "fn f()->(){f();return;}";
    let (sources, mut program) = parsed(text);
    if let ast::ExprKind::Call { args, .. } = &mut program.expressions[0].kind {
        args.resize_with(MAX_ROWS + 1, || ast::Argument::Value(ast::ExprId(0)));
    } else {
        panic!("fixture must contain the call");
    }
    assert!(matches!(
        BoundObservation::bind(
            original(&sources, &program),
            text.as_bytes(),
            &frame(text.len())
        ),
        Err(Boundary::Domain)
    ));
}

#[test]
fn checked_hir_import_oversized_equal_length_capture_stops_before_bytes() {
    let text = " ".repeat(MAX_ROWS + 1);
    let (sources, program) = parsed(&text);
    let captured = vec![b'\t'; MAX_ROWS + 1];
    assert!(matches!(
        BoundObservation::bind(original(&sources, &program), &captured, &[]),
        Err(Boundary::Domain)
    ));
    assert!(matches!(
        BoundObservation::bind(original(&sources, &program), &captured[..MAX_ROWS], &[]),
        Err(Boundary::Source)
    ));
}
