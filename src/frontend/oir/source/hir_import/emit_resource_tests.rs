//! Actual paid Emit boundaries. No external native tool is invoked.
use super::u8_resource_successor::{EMIT_FIXED_GROWTH, RICH_RESERVATION_WORK, V2_RESERVATION_WORK};
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
// Independently measured rich main.ox endpoints after complete versioned
// protocol/endpoint carriers plus 264 bytes of shared source-helper transports;
// not derived from the call under test.
const PREDECESSOR_FIXED: u64 = 140_203;
const FIXED: u64 = PREDECESSOR_FIXED + EMIT_FIXED_GROWTH;
const HIR_PAIR: u64 = 3_472 + 2_241;
const LLVM_BYTES: u64 = 14_325;
const PREDECESSOR_WORK: u64 = 10_069_187;
const WORK: u64 = PREDECESSOR_WORK + RICH_RESERVATION_WORK;

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
fn checked_hir_import_emit_actual_work_and_final_text_byte_endpoints_u8_admission_successor() {
    let mut sources = SourceMap::new();
    let id = sources.add("main.ox".into(), TEXT.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    let defaults = IndexLimits::default();
    for (limits, expected) in [
        // The shared named-bank successor tightens v1 byte admission too.
        // Explicitly refuse the predecessor endpoint without allocation.
        (
            IndexLimits {
                scratch: 138_865,
                ..defaults
            },
            Expected::SourceBytes,
        ),
        (
            IndexLimits {
                scratch: PREDECESSOR_FIXED,
                ..defaults
            },
            Expected::SourceBytes,
        ),
        (
            IndexLimits {
                work: PREDECESSOR_WORK,
                ..defaults
            },
            Expected::LateWork,
        ),
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
fn checked_hir_import_emit_conservative_long_path_actual_work_endpoints_u8_admission_successor() {
    for path_bytes in [3_365, 3_366] {
        let mut sources = SourceMap::new();
        let id = sources.add("x".repeat(path_bytes), TEXT.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        // Both ordinary outputs are baseline and succeed under default policy.
        let ordinary = super::super::check_source(source, &ast, &sources)
            .unwrap()
            .native_module()
            .unwrap();
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(32).unwrap();
        let mut matched = false;
        let observed = owned::hir_import_measure_allocations(|| {
            let result = leaf::emit(
                owner,
                TEXT.as_bytes(),
                WIRE,
                &mut allocator,
                IndexLimits::default(),
            );
            matched = match (path_bytes, result) {
                (3_365, Ok(output)) => {
                    assert_eq!(output.total_work, 255_928_515 + RICH_RESERVATION_WORK);
                    assert_eq!(output.artifact.text, ordinary);
                    assert_eq!(output.artifact.bytes, output.artifact.capacity);
                    drop(output);
                    true
                }
                (
                    3_366,
                    Err(leaf::VerifyRejected::Terminal(candidate::VerifyRejected::EmitWork(
                        emit_work::Failure::Work,
                    ))),
                ) => true,
                other => panic!("unexpected actual long-path endpoint: {other:?}"),
            };
        });
        assert!(matched);
        assert_eq!(
            allocator.attempts,
            if path_bytes == 3_365 { 17 } else { 16 }
        );
        assert_eq!(
            allocator
                .trace
                .iter()
                .filter(|row| row.kind == "private LLVM text")
                .count(),
            usize::from(path_bytes == 3_365)
        );
        assert_eq!(observed.2, 0);
        println!("HIR_IMPORT_EMIT_LONG_PATH bytes={path_bytes} private_admitted={} ordinary_bytes={} allocation={observed:?}", path_bytes == 3_365, ordinary.len());
    }
}

#[test]
fn checked_hir_import_v2_actual_emit_exact_and_minus_one_endpoints_u8_admission_successor() {
    const TEXT2: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import_v2/source-255.txt"
    ));
    const WIRE2: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import_v2/success-255.bin"
    ));
    assert_eq!(TEXT2.len(), 255);
    let mut sources = SourceMap::new();
    let id = sources.add("main.ox".into(), TEXT2.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    let mut allocator = Allocator::default();
    let baseline = leaf::emit(
        owner,
        TEXT2.as_bytes(),
        WIRE2,
        &mut allocator,
        IndexLimits::default(),
    )
    .unwrap();
    assert_eq!(baseline.source_work, 190_160);
    assert_eq!(baseline.artifact.verified.entry_work, 256 * (1 + 255 + 2));
    let fixed = baseline.artifact.verified.candidate.allocation.fixed_bytes as u64;
    assert_eq!(fixed, FIXED);
    let bytes = baseline.artifact.bytes as u64;
    let retained = (fixed + bytes).max(
        baseline
            .artifact
            .verified
            .candidate
            .allocation
            .affected_bytes as u64,
    );
    let work = baseline.total_work;
    // Independently measured authentic v2 fixture; keep the endpoints visible.
    assert_eq!(
        (fixed, retained, bytes, work),
        (
            140_203 + EMIT_FIXED_GROWTH,
            141_739 + EMIT_FIXED_GROWTH,
            690,
            1_004_093 + V2_RESERVATION_WORK
        )
    );
    drop(baseline);
    let exact = IndexLimits {
        scratch: fixed,
        retained,
        work,
    };
    let accepted = leaf::emit(
        owner,
        TEXT2.as_bytes(),
        WIRE2,
        &mut Allocator::default(),
        exact,
    )
    .unwrap();
    assert_eq!(accepted.artifact.bytes as u64, bytes);
    assert_eq!(accepted.artifact.capacity as u64, bytes);
    assert_eq!(accepted.total_work, work);
    for lower in [
        IndexLimits {
            work: work - 1,
            ..exact
        },
        IndexLimits {
            scratch: fixed - 1,
            ..exact
        },
        IndexLimits {
            retained: exact.retained - 1,
            ..exact
        },
    ] {
        assert!(leaf::emit(
            owner,
            TEXT2.as_bytes(),
            WIRE2,
            &mut Allocator::default(),
            lower
        )
        .is_err());
    }
    println!("HIR_IMPORT_V2 actual fixed={fixed} retained={retained} llvm={bytes} work={work}");
}
