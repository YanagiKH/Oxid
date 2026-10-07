//! Actual paid Emit boundaries. No external native tool is invoked.
use super::{candidate, leaf, SourceOwner};
use crate::frontend::{
    declaration_index::IndexLimits,
    lexer,
    oir::{
        native::{emit_work, private_emit},
        owned,
    },
    parser,
    project::budget::Allocator,
    source::{SourceMap, SourceView},
    typeck,
};
const TEXT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import/rich-source.txt"
));
const WIRE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import/rich-success.bin"
));
// Independently measured f44 rich main.ox endpoints, not derived from the call under test.
const FIXED: u64 = 138_865;
const HIR_PAIR: u64 = 3_472 + 2_241;
const LLVM_BYTES: u64 = 14_325;
const WORK: u64 = 10_069_187;

#[derive(Clone, Copy, Debug)]
enum Expected {
    Text,
    EarlyWork,
    LateWork,
    SourceBytes,
    CandidateBytes,
    OutputBytes,
}

#[test]
fn checked_hir_import_emit_actual_work_and_final_text_byte_endpoints() {
    let mut sources = SourceMap::new();
    let id = sources.add("main.ox".into(), TEXT.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    let defaults = IndexLimits::default();
    for (limits, expected) in [
        (
            IndexLimits {
                work: 0,
                ..defaults
            },
            Expected::EarlyWork,
        ),
        (
            IndexLimits {
                work: 32_767,
                ..defaults
            },
            Expected::EarlyWork,
        ),
        (
            IndexLimits {
                work: WORK - 1,
                ..defaults
            },
            Expected::LateWork,
        ),
        (
            IndexLimits {
                work: WORK,
                ..defaults
            },
            Expected::Text,
        ),
        (
            IndexLimits {
                scratch: FIXED - 1,
                ..defaults
            },
            Expected::SourceBytes,
        ),
        (
            IndexLimits {
                retained: FIXED + HIR_PAIR - 1,
                ..defaults
            },
            Expected::CandidateBytes,
        ),
        (
            IndexLimits {
                retained: FIXED + LLVM_BYTES - 1,
                ..defaults
            },
            Expected::OutputBytes,
        ),
        (
            IndexLimits {
                retained: FIXED + LLVM_BYTES,
                scratch: FIXED,
                work: WORK,
            },
            Expected::Text,
        ),
    ] {
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(32).unwrap();
        let capacity = allocator.trace.capacity();
        let checker = typeck::measurement::begin();
        let mut matched = false;
        let observed = owned::hir_import_measure_allocations(|| {
            let result = leaf::emit(owner, TEXT.as_bytes(), WIRE, &mut allocator, limits);
            matched = match (expected, result) {
                (Expected::Text, Ok(output)) => {
                    assert_eq!(output.total_work, WORK);
                    assert_eq!(output.artifact.bytes as u64, LLVM_BYTES);
                    assert_eq!(output.artifact.capacity as u64, LLVM_BYTES);
                    assert_eq!(
                        output.artifact.verified.candidate.allocation.fixed_bytes as u64,
                        FIXED
                    );
                    drop(output);
                    true
                }
                (
                    Expected::EarlyWork | Expected::LateWork,
                    Err(leaf::VerifyRejected::Terminal(candidate::VerifyRejected::EmitWork(
                        emit_work::Failure::Work,
                    ))),
                ) => true,
                (
                    Expected::SourceBytes,
                    Err(leaf::VerifyRejected::Source(leaf::Rejected::Budget)),
                ) => true,
                (
                    Expected::CandidateBytes,
                    Err(leaf::VerifyRejected::Terminal(candidate::VerifyRejected::Candidate(
                        super::allocation::Failure::Admission,
                    ))),
                ) => true,
                (
                    Expected::OutputBytes,
                    Err(leaf::VerifyRejected::Terminal(candidate::VerifyRejected::Native(
                        private_emit::Failure::Budget,
                    ))),
                ) => true,
                other => panic!("unexpected actual Emit endpoint: {other:?}"),
            };
        });
        let checked = checker.finish();
        assert!(matched);
        assert_eq!(
            observed.2, 0,
            "all output/error payloads dropped: {expected:?}"
        );
        let expected_attempts = match expected {
            Expected::Text => 17,
            Expected::LateWork | Expected::OutputBytes => 16,
            Expected::EarlyWork | Expected::SourceBytes | Expected::CandidateBytes => 0,
        };
        assert_eq!(allocator.attempts, expected_attempts);
        assert_eq!(allocator.trace.capacity(), capacity);
        assert!(!allocator.observer_trace_overflow);
        assert_eq!(
            allocator
                .trace
                .iter()
                .filter(|row| row.kind == "private LLVM text")
                .count(),
            usize::from(matches!(expected, Expected::Text))
        );
        if expected_attempts == 0 {
            assert_eq!(checked.frame_bytes, 0);
        } else {
            assert!(checked.frame_bytes > 0);
        }
        assert!(owned::hir_import_allocation_observers_idle());
        println!("HIR_IMPORT_EMIT_RESOURCE {expected:?} work={} retained={} scratch={} attempts={} allocation={observed:?}", limits.work, limits.retained, limits.scratch, allocator.attempts);
    }
}

#[test]
fn checked_hir_import_emit_conservative_long_path_rejects_before_final_reserve() {
    let mut sources = SourceMap::new();
    let id = sources.add("x".repeat(3_366), TEXT.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(32).unwrap();
    let mut matched = false;
    let observed = owned::hir_import_measure_allocations(|| {
        matched = matches!(
            leaf::emit(
                owner,
                TEXT.as_bytes(),
                WIRE,
                &mut allocator,
                IndexLimits::default()
            ),
            Err(leaf::VerifyRejected::Terminal(
                candidate::VerifyRejected::EmitWork(emit_work::Failure::Work)
            ))
        );
    });
    assert!(matched);
    assert_eq!(allocator.attempts, 16);
    assert!(allocator
        .trace
        .iter()
        .all(|row| row.kind != "private LLVM text"));
    assert_eq!(observed.2, 0);
    let ordinary = super::super::check_source(source, &ast, &sources)
        .unwrap()
        .native_module()
        .unwrap();
    assert!(!ordinary.is_empty());
    println!(
        "HIR_IMPORT_EMIT_LONG_PATH private_work_rejected ordinary_bytes={} allocation={observed:?}",
        ordinary.len()
    );
}
