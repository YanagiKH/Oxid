//! Selected private source-storage failures, not a process-wide OOM claim.
//! Frozen parsed sources and hand-derived requests; no successful probe run
//! supplies an ordinal, shape, owner, checker context, or expected capacity.
use super::super::reviewer_source as heap;
use super::*;
use crate::frontend::{
    lexer, oir::owned::reviewer_origins as raw, parser, project::budget::real_null_observer as null,
};
use std::{alloc::Layout, mem::size_of};

// Existing rich-success batch A, A4, 67 bytes, SHA-256:
// 7e4e77f1779d55275c51be91dbe8f169e42340acd5e44c9882dfa1f43c3d8b71
const GROUPED_ARRAY: &str = "enum Unused{V} fn main()->i32{let a:[i32;0]=(([]));return a.len();}";
// Existing projection-success batch B, B1, 224 bytes, SHA-256:
// bab1974afaee4bc84d33756cd810a286ab112926f9077bef94255630d15b4470
const PROJECTED_BORROW: &str = "enum Unused{V} struct R{n:i32,a:[i32;2]} fn read(xs:&[i32],ys:&mut[i32])->i32{return xs[0]+ys.len();} fn main()->i32{let mut r=R{n:1,a:[2,3]};let whole=[4,5];r.n=6;r.a[0]=7;return r.n+r.a[1]+r.a.len()+read(&whole,&mut r.a);}";
const TINY: &str = "enum Unused{V} fn main()->i32{return 0;}";
const ALLOCATION_MESSAGE: &str = "affected HIR allocation failed";
const SENTINEL: [u8; 3] = [29, 53, 89];

fn with_index<T>(text: &str, action: impl FnOnce(&DeclarationIndex<'_>) -> T) -> T {
    let mut sources = SourceMap::new();
    let file = sources.add("enum-storage-failure.ox".into(), text.into());
    let source = sources.get(file);
    let ast = parser::parse_enum_candidate_counted(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut Allocator::default(),
        &mut Default::default(),
    )
    .unwrap()
    .0;
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = index::collect_enum_candidate(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
    assert_eq!(index.enum_count(), 1);
    action(&index)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FailureFacts {
    failed: bool,
    positive_facts: bool,
    errors: usize,
    code: Option<&'static str>,
    stage: Option<&'static str>,
    primary: Option<Span>,
    message_matches: bool,
    no_related: bool,
}

fn failure_facts(
    result: Result<Option<EnumTypeStorageObservation>, Vec<Diagnostic>>,
    expected_message: &str,
) -> FailureFacts {
    // Only fixed facts leave this helper. Even diagnostics are dropped before
    // the enclosing measured interval ends; no formatting or clone is needed.
    match result {
        Ok(facts) => FailureFacts {
            failed: false,
            positive_facts: facts.is_some(),
            errors: 0,
            code: None,
            stage: None,
            primary: None,
            message_matches: false,
            no_related: false,
        },
        Err(errors) => {
            let first = errors.first();
            let facts = FailureFacts {
                failed: true,
                positive_facts: false,
                errors: errors.len(),
                code: first.map(|error| error.code),
                stage: first.map(|error| error.stage),
                primary: first.and_then(|error| error.primary),
                message_matches: first.is_some_and(|error| error.message == expected_message),
                no_related: errors
                    .iter()
                    .all(|error| error.secondary.is_empty() && error.notes.is_empty()),
            };
            drop(errors);
            facts
        }
    }
}

fn assert_failure(facts: FailureFacts, stage: &'static str, primary: Span) {
    assert_eq!(
        facts,
        FailureFacts {
            failed: true,
            positive_facts: false,
            errors: 1,
            code: Some("E0400"),
            stage: Some(stage),
            primary: Some(primary),
            message_matches: true,
            no_related: true,
        }
    );
}

fn prepare_allocator(preused: bool, fail_at: Option<usize>) -> (Allocator, Vec<u8>) {
    let mut allocator = Allocator {
        fail_at,
        ..Allocator::default()
    };
    // This backing and the sentinel remain alive outside every heap window.
    allocator.observer_trace_bound(64).unwrap();
    let mut sentinel = Vec::new();
    if preused {
        allocator
            .vector_exact(&mut sentinel, SENTINEL.len(), "source failure sentinel")
            .unwrap();
        sentinel.extend_from_slice(&SENTINEL);
    }
    (allocator, sentinel)
}

fn assert_preserved(
    allocator: &Allocator,
    sentinel: &Vec<u8>,
    preused: bool,
    capacities: (usize, usize),
    work: &WorkMeter,
) {
    assert_eq!(
        (allocator.trace.capacity(), sentinel.capacity()),
        capacities
    );
    assert_eq!(
        sentinel.as_slice(),
        if preused { &SENTINEL[..] } else { &[][..] }
    );
    if preused {
        let row = &allocator.trace[0];
        assert_eq!(
            (row.kind, row.length, row.element_bytes, row.success),
            ("source failure sentinel", 3, size_of::<u8>(), true)
        );
    }
    assert!(!allocator.observer_trace_overflow);
    assert!(!work.observing());
    assert!(work.events.borrow().is_empty() && work.observations.borrow().is_empty());
    assert!(!heap::integration_enabled() && !raw::integration_enabled());
}

fn target<T>(attempt: usize, kind: &'static str, slots: usize) -> null::Target {
    null::Target {
        attempt,
        kind,
        slots,
        element_bytes: size_of::<T>(),
        layout: Layout::array::<T>(slots).unwrap(),
    }
}

fn assert_terminal_request(allocator: &Allocator, prefix: usize, target: null::Target) {
    assert_eq!(allocator.attempts, target.attempt);
    assert_eq!(allocator.trace.len(), target.attempt);
    let suffix = &allocator.trace[prefix..];
    let (last, earlier) = suffix.split_last().unwrap();
    assert!(earlier.iter().all(|row| row.success));
    assert_eq!(
        (last.kind, last.length, last.element_bytes, last.success),
        (target.kind, target.slots, target.element_bytes, false)
    );
    assert_eq!(target.layout.size(), target.slots * target.element_bytes);
}

fn check_real_null(text: &str, preused: bool, target: null::Target, primary_range: (usize, usize)) {
    with_index(text, |index| {
        let (mut allocator, sentinel) = prepare_allocator(preused, None);
        let before = allocator.attempts;
        assert_eq!(before, usize::from(preused));
        assert_eq!(allocator.trace.len(), before);
        let capacities = (allocator.trace.capacity(), sentinel.capacity());
        let work = WorkMeter::default();
        // Only index/work references enter the controller. The private probe
        // itself freshly constructs and drops every source/checker owner.
        // There is no reset, move, replacement, or fail_at mutation in scope.
        let action = |allocator: &mut Allocator| {
            raw::integration_counted(|| {
                heap::integration_measured(|| {
                    failure_facts(
                        probe_enum_type_storage(index, &work, allocator),
                        ALLOCATION_MESSAGE,
                    )
                })
            })
        };
        let control_bytes = null::selection_carriers_bytes(&action);
        let (((facts, stats), raw_calls), report) =
            null::with_selected(&mut allocator, target, action).unwrap();
        assert_failure(
            facts,
            "resolve",
            Span {
                file: index.sources().eof().file,
                start: primary_range.0,
                end: primary_range.1,
            },
        );
        assert_eq!(report.target, target);
        assert!(report.selected && report.matched && report.fired);
        assert_eq!(report.rejection, None);
        assert_eq!(
            report.actual,
            Some(null::GlobalEvent {
                operation: null::Operation::Alloc,
                layout: target.layout,
                new_size: None,
            })
        );
        // The exact null is one raw attempt, with no successful heap charge.
        // Diagnostic allocations succeed after the one-shot gate has disarmed.
        assert_eq!(raw_calls, stats.0 + 1);
        assert_eq!(stats.1, 0);
        assert!(stats.0 > 0 && stats.2 > 0);
        assert_eq!(allocator.fail_at, None);
        assert_terminal_request(&allocator, before, target);
        assert_preserved(&allocator, &sentinel, preused, capacities, &work);
        if text == PROJECTED_BORROW {
            // B1's frozen paths have whole ordinals 50,51,52,53,54,56.
            // The first five have entered their statement/expression caches
            // before the final borrowed path fails. Source audit establishes
            // those insertions; the trace is only reservation evidence.
            let mut found = 0;
            for (offset, row) in allocator.trace[before..].iter().enumerate() {
                if row.kind == "paid typed projection fields" {
                    assert!(found < 6);
                    assert_eq!(offset + 1, [50, 51, 52, 53, 54, 56][found]);
                    assert_eq!((row.length, row.element_bytes), (1, size_of::<FieldId>()));
                    assert_eq!(row.success, found != 5);
                    found += 1;
                }
            }
            assert_eq!(found, 6);
        }
        println!(
            "C3_SELECTED_SOURCE_NULL absolute={} before={before} kind={} slots={} width={} layout={:?} control_bytes={control_bytes} live={} peak={}",
            target.attempt, target.kind, target.slots, target.element_bytes,
            target.layout, stats.1, stats.2
        );
    });
}

#[test]
fn c3_t1_selected_source_null_early_typed_stage_drops_resolver_and_bodies() {
    assert_eq!(GROUPED_ARRAY.len(), 67);
    // A4: resolver 14, then Bodies 1, then BindingStage 1. N=1, so the
    // absolute fresh request16 has Layout::array::<Option<ParameterTy>>(1).
    check_real_null(
        GROUPED_ARRAY,
        false,
        target::<Option<ParameterTy>>(16, "paid typed binding stage", 1),
        (66, 67),
    );
}

#[test]
fn c3_t1_selected_source_null_late_finalization_drops_stages_and_finals() {
    // A4's typed request 12 is ExpressionFinal after BindingFinal/FlowFinal.
    // Four HIR expressions are [], Group, Group, ArrayLength. b=1, R=14:
    // absolute request = 1+14+12=27, Layout::array::<ValueTy>(4).
    check_real_null(
        GROUPED_ARRAY,
        true,
        target::<ValueTy>(27, "paid typed final expressions", 4),
        (66, 67),
    );
}

#[test]
fn c3_t1_selected_source_null_borrow_path_drops_earlier_retained_paths() {
    assert_eq!(PROJECTED_BORROW.len(), 224);
    assert_eq!(&PROJECTED_BORROW[220..221], "a");
    // Frozen B1: resolver 28; borrowed r.a is typed 28, after five retained
    // paths and call actuals. b=1 gives absolute 57; the field span is 220..221.
    check_real_null(
        PROJECTED_BORROW,
        true,
        target::<FieldId>(57, "paid typed projection fields", 1),
        (220, 221),
    );
}

#[test]
fn c3_t1_selected_source_capacity_failure_twin_drops_same_late_state() {
    with_index(GROUPED_ARRAY, |index| {
        let target = target::<ValueTy>(27, "paid typed final expressions", 4);
        // Configure before use and never select a real-null controller here.
        // This is try_reserve_exact(usize::MAX)'s logical capacity failure.
        let (mut allocator, sentinel) = prepare_allocator(true, Some(target.attempt));
        let capacities = (allocator.trace.capacity(), sentinel.capacity());
        let work = WorkMeter::default();
        let ((facts, stats), raw_calls) = raw::integration_counted(|| {
            heap::integration_measured(|| {
                failure_facts(
                    probe_enum_type_storage(index, &work, &mut allocator),
                    ALLOCATION_MESSAGE,
                )
            })
        });
        assert_failure(
            facts,
            "resolve",
            Span {
                file: index.sources().eof().file,
                start: 66,
                end: 67,
            },
        );
        // Capacity overflow never reaches the global allocator for the failed
        // request, unlike the selected real-null twin's one extra raw call.
        assert_eq!(raw_calls, stats.0);
        assert_eq!(stats.1, 0);
        assert!(stats.0 > 0 && stats.2 > 0);
        assert_eq!(allocator.fail_at, Some(27));
        assert_terminal_request(&allocator, 1, target);
        assert_preserved(&allocator, &sentinel, true, capacities, &work);
    });
}

#[test]
fn c3_t1_selected_source_work_failure_drops_complete_resolver_before_typing() {
    with_index(TINY, |index| {
        let (mut allocator, sentinel) = prepare_allocator(false, None);
        let capacities = (allocator.trace.capacity(), sentinel.capacity());
        // Hand-derived tiny-source ledger: preflight 4 + i32 result query 7 +
        // local recount 3 + name inventory 1 + completed-HIR inventory 13 +
        // resolver reconciliation 18 =46. The 18-kind resolver (already at
        // a697) includes the MatchArms scalar visit even with zero arms.
        // All 13 resolver requests have succeeded. The next one-unit debit is
        // prepare's first typed-storage visit at EOF, before any typed reserve.
        let work = WorkMeter::new(46);
        let ((facts, stats), raw_calls) = raw::integration_counted(|| {
            heap::integration_measured(|| {
                failure_facts(
                    probe_enum_type_storage(index, &work, &mut allocator),
                    "declaration index work limit exceeded",
                )
            })
        });
        assert_failure(facts, "resolve-project", index.sources().eof());
        assert_eq!(work.used(), 46);
        assert_eq!(allocator.attempts, 13);
        assert_eq!(allocator.trace.len(), 13);
        assert!(allocator.trace.iter().all(|row| row.success));
        assert!(allocator
            .trace
            .iter()
            .all(|row| row.kind.starts_with("paid HIR ")));
        assert_eq!(raw_calls, stats.0);
        assert_eq!(stats.1, 0);
        assert!(stats.0 > 0 && stats.2 > 0);
        assert_preserved(&allocator, &sentinel, false, capacities, &work);
    });
}
