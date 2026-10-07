//! Private Run entry, compile-budget and candidate execution controls.
use super::super::{ast_compare, leaf, BoundObservation, OPA_BYTES, SUCCESS_BYTES};
use super::*;
use crate::frontend::{
    declaration_index::{collect_originals, SourceOwner},
    lexer,
    oir::{execute, owned},
    parser,
    source::{SourceMap, SourceView},
    typeck,
};

const RICH_SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import/rich-source.txt"
));
const RICH_WIRE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import/rich-success.bin"
));

#[test]
fn checked_hir_import_run_outer_budget_precedes_every_allocation() {
    let mut sources = SourceMap::new();
    let id = sources.add("run-denial.ox".into(), RICH_SOURCE.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    for (capture, wire) in [
        (RICH_SOURCE.as_bytes(), RICH_WIRE),
        (RICH_SOURCE.as_bytes(), &[][..]),
        (&[][..], RICH_WIRE),
    ] {
        {
            let limits = IndexLimits {
                retained: 0,
                scratch: 0,
                work: 0,
            };
            let mut allocator = Allocator {
                fail_at: Some(1),
                ..Allocator::default()
            };
            let checker = typeck::measurement::begin();
            let runtime = execute::measurement::begin();
            let mut disabled = false;
            let observed = owned::hir_import_measure_allocations(|| {
                disabled = matches!(
                    leaf::run(owner, capture, wire, &mut allocator, limits),
                    Err(leaf::VerifyRejected::Source(leaf::Rejected::Budget))
                );
            });
            assert!(disabled);
            assert_eq!(observed, (0, 0, 0, 0));
            assert_eq!(allocator.attempts, 0);
            assert_eq!(checker.finish().frame_bytes, 0);
            assert_eq!(runtime.finish(), execute::measurement::Snapshot::default());
        }
    }
}

#[test]
fn checked_hir_import_run_candidate_zero_work_precedes_reserves() {
    let mut sources = SourceMap::new();
    let id = sources.add("empty-run-denial.ox".into(), String::new());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    // Explicitly synthetic empty source/OPA/STF framing. It conveys no Run
    // permission; the genuine resolver still constructs the canonical input.
    let mut wire = [0u8; SUCCESS_BYTES];
    wire[..4].copy_from_slice(b"OPA1");
    wire[OPA_BYTES..OPA_BYTES + 4].copy_from_slice(b"STF1");
    let bound = BoundObservation::bind(owner, b"", &wire).unwrap();
    let syntax = ast_compare::compare(&bound).unwrap();
    let canonical = hir::resolve_sources(owner).unwrap();
    assert_eq!(canonical.signatures.capacity(), 0);
    assert_eq!(canonical.functions.capacity(), 0);
    let mut allocator = Allocator {
        fail_at: Some(1),
        ..Allocator::default()
    };
    let work = WorkMeter::new(0);
    let checker = typeck::measurement::begin();
    let runtime = execute::measurement::begin();
    let mut disabled = false;
    let observed = owned::hir_import_measure_allocations(|| {
        disabled = matches!(
            request_candidate(
                Request::Run,
                &syntax,
                canonical,
                &mut allocator,
                0,
                IndexLimits {
                    retained: 0,
                    scratch: 0,
                    work: 0
                },
                &work,
                source.span(0, 0)
            ),
            Err(VerifyRejected::Candidate(Failure::Admission))
        );
    });
    assert!(disabled);
    assert_eq!(observed, (0, 0, 0, 0));
    assert_eq!(allocator.attempts, 0);
    assert_eq!(work.used(), 0);
    assert_eq!(checker.finish().frame_bytes, 0);
    assert_eq!(runtime.finish(), execute::measurement::Snapshot::default());
}

#[test]
fn checked_hir_import_run_carrier_preserves_verify_result_and_work() {
    let mut sources = SourceMap::new();
    let id = sources.add("verify-only.ox".into(), RICH_SOURCE.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    let runtime = execute::measurement::begin();
    let facts = leaf::verify(
        owner,
        RICH_SOURCE.as_bytes(),
        RICH_WIRE,
        &mut Allocator::default(),
        IndexLimits::default(),
    )
    .unwrap();
    assert!(facts.verified.runtime.is_none());
    assert_eq!(facts.verified.entry_work, 0);
    assert_eq!(facts.total_work, 1_276_867);
    assert_eq!(runtime.finish(), execute::measurement::Snapshot::default());
}

// These controls inspect only the entry projection. They use genuine source
// owners and canonical resolution, without candidate construction or execution.
fn equivalent_entry(owner: SourceOwner<'_>, expected: Option<hir::DefId>) {
    let root = owner.ast(ModuleId(0)).unwrap();
    let source = owner.file(ModuleId(0)).unwrap();
    let candidate = hir::resolve_sources(owner).unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_originals(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
    let projected = verify_terminal::root_entry(root, source, &candidate).unwrap();
    assert_eq!(projected, expected);
    assert_eq!(projected, index.root_original_main());
}

#[test]
fn checked_hir_import_run_original_root_entry_projection() {
    for (text, expected) in [
        ("", None),
        ("fn library()->i32{return 7;}", None),
        ("fn main()->i32{return 7;}", Some(hir::DefId(0))),
        (
            "fn helper()->i32{return 7;}fn main()->i32{return helper();}",
            Some(hir::DefId(1)),
        ),
        // Arity remains an ordinary runtime entry failure, not entry selection.
        ("fn main(x:i32)->i32{return x;}", Some(hir::DefId(0))),
    ] {
        let mut sources = SourceMap::new();
        let id = sources.add("run-root.ox".into(), text.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        equivalent_entry(
            SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap(),
            expected,
        );
    }
}

#[test]
fn checked_hir_import_run_root_entry_checks_complete_function_identity() {
    let text = "fn main()->i32{return 7;}fn helper()->i32{return 8;}";
    let mut sources = SourceMap::new();
    let id = sources.add("run-identity.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    let mut candidate = hir::resolve_sources(owner).unwrap();
    assert_eq!(
        verify_terminal::root_entry(&ast, source, &candidate).unwrap(),
        Some(hir::DefId(0))
    );
    // The entire identity sequence is checked, including functions after main.
    candidate.functions[1].id = hir::DefId(0);
    assert!(matches!(
        verify_terminal::root_entry(&ast, source, &candidate),
        Err(Failure::Shape)
    ));
    candidate.functions[1].id = hir::DefId(1);
    candidate.signatures[1].span = candidate.signatures[0].span;
    assert!(matches!(
        verify_terminal::root_entry(&ast, source, &candidate),
        Err(Failure::Shape)
    ));
    candidate.signatures[1].span = ast.functions[1].name;
    candidate.signatures.pop();
    assert!(matches!(
        verify_terminal::root_entry(&ast, source, &candidate),
        Err(Failure::Shape)
    ));
}

// Genuine project loading follows the compiler's existing Linux host policy.
// No fabricated ProjectSources, original-owner relabeling or numeric file ID.
#[cfg(target_os = "linux")]
#[test]
fn checked_hir_import_run_project_root_entry_projection() {
    use crate::frontend::project::{ProjectLimits, ProjectSources};
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    for (text, expected) in [
        ("pub fn main()->i32{return 7;}", Some(hir::DefId(0))),
        (
            "fn helper()->i32{return 7;}pub fn main()->i32{return helper();}",
            Some(hir::DefId(1)),
        ),
        (
            "pub fn library()->i32{return helper();}fn helper()->i32{return 7;}",
            None,
        ),
        ("pub fn main(x:i32)->i32{return x;}", Some(hir::DefId(0))),
        ("", None),
    ] {
        let directory = std::env::temp_dir().join(format!(
            "oxid-run-root-entry-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
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
        let project =
            ProjectSources::load_typed(file.to_str().unwrap(), ProjectLimits::default()).unwrap();
        assert_eq!(project.modules().len(), 1);
        equivalent_entry(SourceOwner::project(&project), expected);
    }
}

#[test]
fn checked_hir_import_run_work_and_complete_carriers_only() {
    let counts = super::super::Counts([2, 2, 1, 2, 11, 5, 7, 2]);
    let verify = verify_terminal::WorkPlan::calculate_request(counts, 29, Request::Verify).unwrap();
    let run = verify_terminal::WorkPlan::calculate_request(counts, 29, Request::Run).unwrap();
    assert_eq!(verify.entry_work, 0);
    assert_eq!(verify.total, 1_009_440);
    assert_eq!(run.entry_work, 256 * (2 + MAX_ROWS as u64 + 2));
    assert_eq!(run.total, verify.total + run.entry_work);
    assert!(verify_terminal::WorkPlan::calculate_request(
        super::super::Counts([MAX_ROWS + 1; 8]),
        MAX_ROWS,
        Request::Run
    )
    .is_err());
    assert!(
        verify_terminal::WorkPlan::calculate_request(counts, MAX_ROWS + 1, Request::Run).is_err()
    );
    println!("HIR_IMPORT_RUN_CARRIERS request={} canonical_input={} completion={} completion_result={} context={} plan={} terminal_facts={} terminal_rejected={} fixed_facts_result={} builder_named={} terminal_named={} candidate_named={} terminal_outcome={} terminal_result={}",
        size_of::<Request>(), size_of::<CanonicalInput<'_, '_>>(), size_of::<Completion>(), size_of::<Result<Completion, VerifyRejected>>(),
        size_of::<verify_terminal::Context<'_>>(), size_of::<verify_terminal::WorkPlan>(), size_of::<VerifyFacts>(), size_of::<VerifyRejected>(), size_of::<Result<VerifyFacts, VerifyRejected>>(),
        builder_named_bytes().unwrap(), verify_terminal::named_bytes().unwrap(), verify_named_bytes().unwrap(), size_of::<Outcome>(), size_of::<Result<Outcome, VerifyRejected>>());
}

#[test]
fn checked_hir_import_run_rich_uses_verified_candidate_and_drops_owners() {
    let mut sources = SourceMap::new();
    let id = sources.add("private-run.ox".into(), RICH_SOURCE.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    let mut allocator = Allocator::default();
    // Caller-retained validation trace is prepared outside the owner interval.
    allocator.observer_trace_bound(32).unwrap();
    let trace_capacity = allocator.trace.capacity();
    let mut result = None;
    let observed = owned::hir_import_measure_allocations(|| {
        result = Some(leaf::run(
            owner,
            RICH_SOURCE.as_bytes(),
            RICH_WIRE,
            &mut allocator,
            IndexLimits::default(),
        ));
    });
    let facts = result.unwrap().unwrap();
    assert_eq!(
        facts.verified.runtime.as_ref(),
        Some(&Ok(crate::frontend::oir::Scalar::I32(1)))
    );
    assert_eq!(facts.verified.functions, 2);
    assert_eq!(facts.verified.typed_cells, 29);
    assert_eq!(facts.verified.entry_work, 33_792);
    assert_eq!(facts.total_work, 1_310_659);
    assert_eq!(allocator.attempts, 16);
    assert_eq!(allocator.trace.capacity(), trace_capacity);
    assert!(!allocator.observer_trace_overflow);
    assert_eq!(observed.2, 0);
    assert!(observed.0 > 16 && observed.3 > 147_456);
    println!("HIR_IMPORT_PRIVATE_RUN {facts:?} allocations={observed:?}");
}

#[test]
fn checked_hir_import_run_exact_compile_budget_precedes_execution() {
    let mut sources = SourceMap::new();
    let id = sources.add("run-budget.ox".into(), RICH_SOURCE.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    let baseline = leaf::run(
        owner,
        RICH_SOURCE.as_bytes(),
        RICH_WIRE,
        &mut Allocator::default(),
        IndexLimits::default(),
    )
    .unwrap();
    let receipt = baseline.verified.candidate.allocation;
    let exact = IndexLimits {
        retained: receipt.affected_bytes as u64,
        scratch: receipt.fixed_bytes as u64,
        work: baseline.total_work,
    };
    assert_eq!(exact.work, 1_310_659);
    let accepted = leaf::run(
        owner,
        RICH_SOURCE.as_bytes(),
        RICH_WIRE,
        &mut Allocator::default(),
        exact,
    )
    .unwrap();
    assert_eq!(accepted.verified.runtime, baseline.verified.runtime);
    for limits in [
        IndexLimits {
            retained: exact.retained - 1,
            ..exact
        },
        IndexLimits {
            scratch: exact.scratch - 1,
            ..exact
        },
        IndexLimits {
            work: exact.work - 1,
            ..exact
        },
    ] {
        let mut allocator = Allocator::default();
        let checker = typeck::measurement::begin();
        let runtime = execute::measurement::begin();
        let rejected = leaf::run(
            owner,
            RICH_SOURCE.as_bytes(),
            RICH_WIRE,
            &mut allocator,
            limits,
        );
        assert!(matches!(
            rejected,
            Err(leaf::VerifyRejected::Source(leaf::Rejected::Budget))
                | Err(leaf::VerifyRejected::Terminal(VerifyRejected::Candidate(
                    Failure::Admission
                )))
        ));
        assert_eq!(allocator.attempts, 0);
        assert_eq!(checker.finish().frame_bytes, 0);
        assert_eq!(runtime.finish(), execute::measurement::Snapshot::default());
    }
}
