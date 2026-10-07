//! Direct seam controls cannot enable the compiled Emit transport.
use super::super::{ast_compare, BoundObservation, OPA_BYTES, SUCCESS_BYTES};
use super::*;
use crate::frontend::{
    declaration_index::SourceOwner,
    lexer,
    oir::{execute, owned},
    parser,
    source::{SourceMap, SourceView},
    typeck,
};

#[test]
fn checked_hir_import_emit_all_candidate_seams_deny_before_consumers() {
    let mut sources = SourceMap::new();
    let id = sources.add("empty-denied-emit.ox".into(), String::new());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    // Synthetic empty framing only. All source/HIR values are genuinely made
    // by the ordinary parser/resolver, and no test bypasses the Emit gate.
    let mut wire = [0u8; SUCCESS_BYTES];
    wire[..4].copy_from_slice(b"OPA1");
    wire[OPA_BYTES..OPA_BYTES + 4].copy_from_slice(b"STF1");
    let bound = BoundObservation::bind(owner, b"", &wire).unwrap();
    let syntax = ast_compare::compare(&bound).unwrap();
    for seam in 0..3 {
        let canonical = hir::resolve_sources(owner).unwrap();
        let comparison = compare_candidate(
            &syntax,
            &canonical,
            &mut Allocator::default(),
            0,
            IndexLimits::default(),
        )
        .unwrap();
        let plan = verify_terminal::WorkPlan::calculate_request(
            comparison.allocation.requested,
            0,
            Request::Emit,
        )
        .unwrap();
        let work = WorkMeter::new(0);
        let context = verify_terminal::Context {
            request: Request::Emit,
            work: &work,
            origin: source.span(0, 0),
        };
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
        let mut denied = false;
        let observed = owned::hir_import_measure_allocations(|| {
            denied = match seam {
                0 => matches!(
                    request_candidate(
                        Request::Emit,
                        &syntax,
                        canonical,
                        &mut allocator,
                        0,
                        limits,
                        &work,
                        source.span(0, 0)
                    ),
                    Err(VerifyRejected::Disabled)
                ),
                1 => matches!(
                    construct(
                        &syntax,
                        CanonicalInput::Verify { canonical, context },
                        &mut allocator,
                        0,
                        limits
                    ),
                    Err(VerifyRejected::Disabled)
                ),
                _ => matches!(
                    verify_terminal::run(
                        context,
                        &syntax,
                        canonical,
                        comparison,
                        plan,
                        &mut allocator,
                        limits
                    ),
                    Err(VerifyRejected::Disabled)
                ),
            };
        });
        assert!(denied);
        assert_eq!(observed, (0, 0, 0, 0));
        assert_eq!(allocator.attempts, 0);
        assert_eq!(work.used(), 0);
        assert_eq!(checker.finish().frame_bytes, 0);
        assert_eq!(runtime.finish(), execute::measurement::Snapshot::default());
    }
}
