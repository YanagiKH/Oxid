//! Opt-in artifact export only. External tools run in the bounded Python driver.
//! This remains a child of the private text tests; no production entry is added.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use super::*;
use crate::frontend::oir::Scalar;
use std::{fs, io::Write, path::Path};

fn save(root: &Path, name: &str, bytes: impl AsRef<[u8]>) {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(name))
        .unwrap()
        .write_all(bytes.as_ref())
        .unwrap();
}

fn export(root: &Path, name: &str, text: &str, wire: &[u8], error: Option<(&str, usize, usize)>) {
    let root = root.join(name);
    fs::create_dir(&root).expect("never overwrite qualification evidence");
    let (output, ordinary, status, stdout, stderr) = {
        let mut sources = SourceMap::new();
        let id = sources.add("private-native.ox".into(), text.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(32).unwrap();
        let output = leaf::emit(
            owner,
            text.as_bytes(),
            wire,
            &mut allocator,
            IndexLimits::default(),
        )
        .unwrap();
        assert_output_facts(&output, wire);
        assert_eq!(
            allocator.attempts,
            output.artifact.verified.candidate.allocation.reserves + 1
        );
        assert_eq!(
            allocator
                .trace
                .iter()
                .filter(|row| row.kind == "private LLVM text")
                .count(),
            1
        );
        assert!(allocator.trace.iter().all(|row| row.success));
        assert!(!allocator.observer_trace_overflow);
        let checked = check_source(source, &ast, &sources).unwrap();
        let ordinary = checked.native_module().unwrap();
        let (status, stdout, stderr) = match (checked.run(), error) {
            (Ok(value), None) => {
                assert_eq!(value, Scalar::I32(1));
                (0, b"1\n".to_vec(), Vec::new())
            }
            (Err(actual), Some((code, start, end))) => {
                assert_eq!(actual.code, code);
                assert_eq!(actual.stage, "oir-run");
                assert_eq!(
                    actual.message,
                    match code {
                        "E0604" => "checked i32 arithmetic overflow",
                        "E0607" => "checked i32 division by zero",
                        _ => panic!("unknown frozen diagnostic"),
                    }
                );
                assert_eq!(
                    actual.primary.map(|span| (span.start, span.end)),
                    Some((start, end))
                );
                (1, Vec::new(), actual.render_human(&sources).into_bytes())
            }
            (actual, expected) => panic!("frozen reference mismatch: {actual:?}, {expected:?}"),
        };
        (output, ordinary, status, stdout, stderr)
    };
    // Actual source map/path/text, AST, owner and ordinary checked program have
    // dropped. Both owned artifacts are consumed only now, without rewriting.
    assert_eq!(output.artifact.text, ordinary);
    save(&root, "source.ox", text);
    save(&root, "observation.bin", wire);
    save(&root, "private.ll", &output.artifact.text);
    save(&root, "ordinary.ll", ordinary);
    save(&root, "expected.status", status.to_string());
    save(&root, "expected.stdout", stdout);
    save(&root, "expected.stderr", stderr);
    save(
        &root,
        "admission.txt",
        format!(
            "bytes={} capacity={} work={} candidate_reserves={} final_reserves=1\n",
            output.artifact.bytes,
            output.artifact.capacity,
            output.total_work,
            output.artifact.verified.candidate.allocation.reserves
        ),
    );
}

#[test]
#[ignore = "explicit private native gate; use scripts/verify_checked_hir_import_native.py"]
fn checked_hir_import_emit_export_native_artifacts() {
    let root = fs::canonicalize(
        std::env::var_os("OXID_HIR_IMPORT_NATIVE_EVIDENCE")
            .expect("fresh evidence directory required"),
    )
    .unwrap();
    assert!(root.is_dir());
    save(&root, "exporter.rs", include_str!("emit_native_tests.rs"));
    save(
        &root,
        "runtime.c",
        include_str!("../../../../../native/typed_preview.c"),
    );
    export(&root, "rich", RICH_SOURCE, RICH_WIRE, None);
    let [(overflow, overflow_wire), (division, division_wire)] = arithmetic_frames();
    export(
        &root,
        "overflow",
        overflow,
        &overflow_wire,
        Some(("E0604", 32, 33)),
    );
    export(
        &root,
        "division",
        division,
        &division_wire,
        Some(("E0607", 23, 24)),
    );
}
