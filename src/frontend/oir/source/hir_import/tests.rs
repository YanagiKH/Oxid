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
    changed[9] = 129;
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

const RICH: &str = "fn f(x:i32)->i32{return x;}fn main()->i32{let mut n=f(1);while false{n=n+1;}if true{return n;}else{return f(2);}}";
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
    let text = format!("pub {RICH}");
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
    std::fs::write(&file, &text).unwrap();
    let project =
        ProjectSources::load_typed(file.to_str().unwrap(), ProjectLimits::default()).unwrap();
    assert_eq!(project.syntax_flavor(), SyntaxFlavor::ProjectSyntax);
    let owner = SourceOwner::project(&project);
    let Err(Rejection::Disabled(facts)) = denied_probe(owner, text.as_bytes(), &frame(text.len()))
    else {
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
