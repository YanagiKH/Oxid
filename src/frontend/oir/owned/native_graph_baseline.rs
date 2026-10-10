//! First closed native-graph baseline: one published fixture, three controls.
//! This test module is not executed by the compile-only gate.
use super::*;
use super::graph_observe::{self as observe, Event, Site, Trace};
use super::super::consumer_fixtures;
use std::mem::{size_of, size_of_val};

const ROWS: usize = 6;
const TRACE_BYTES_CAP: usize = 1024;
const FIXTURE_PAYLOAD_CAP: usize = 256 * 1024;
const OUTPUT_PAYLOAD_CAP: usize = 16 * 1024;
const RENDERED_DIAGNOSTIC_CAP: usize = 1024;
const ADDITIONAL_FIXED_CAP: usize = 8192;
// Static carrier identity from the predecessor ELF, not a runtime measurement.
// The successor's TLS symbol and lowering must independently confirm this.
const TLS_ELF_CARRIER_BYTES: usize = 88;

type FrontendDiagnostic = crate::frontend::diagnostic::Diagnostic;
type NativeResult = Result<String, Box<FrontendDiagnostic>>;
type ObservedResult = Result<(NativeObservation, Trace), &'static str>;

struct Input<'a> {
    witness: &'a VerifiedOwnedProgram,
    sources: &'a SourceMap,
    entry: Option<hir::DefId>,
    control: NativeControl,
}

#[inline(never)]
fn invoke(input: &Input<'_>) -> NativeObservation {
    run_array_observed(input.witness, input.entry, input.sources, input.control)
}

#[derive(Debug)]
struct CarrierInventory {
    input: usize,
    action: usize,
    observation: usize,
    observed_result: usize,
    native_result: usize,
    accounting: usize,
    diagnostic: usize,
    event: usize,
    observer_roles: [usize; 6],
    fixed_upper_bound: usize,
}

#[inline(never)]
fn carrier_inventory<F: FnOnce() -> NativeObservation>(action: &F) -> CarrierInventory {
    let roles = observe::layout_roles();
    // Conservative named-carrier coexistence: retained baseline + new result,
    // one Accounting + input/control/closure, TLS once, local trace/result moves,
    // Ref/RefMut and Event copies. This is NOT a machine-stack high-water bound.
    let parts = [TLS_ELF_CARRIER_BYTES, size_of::<Input<'_>>(), size_of_val(action),
        2 * size_of::<NativeObservation>(), size_of::<ObservedResult>(),
        size_of::<Accounting>(), size_of::<NativeControl>(), size_of::<NativeResult>(),
        2 * size_of::<Trace>(), roles[3], roles[4], roles[5], 3 * size_of::<Event>(),
        size_of::<CarrierInventory>(), size_of::<SourceMap>(), size_of::<VerifiedOwnedProgram>(),
        size_of::<[usize; 32]>(),
        size_of::<FrontendDiagnostic>(), size_of::<String>(), 16 * size_of::<usize>()];
    let fixed_upper_bound = parts.into_iter().try_fold(0usize, usize::checked_add).unwrap();
    CarrierInventory { input: size_of::<Input<'_>>(), action: size_of_val(action),
        observation: size_of::<NativeObservation>(), observed_result: size_of::<ObservedResult>(),
        native_result: size_of::<NativeResult>(), accounting: size_of::<Accounting>(),
        diagnostic: size_of::<FrontendDiagnostic>(), event: size_of::<Event>(),
        observer_roles: roles, fixed_upper_bound }
}

fn warm_tls() {
    let ((), trace) = observe::observe(0, 0, || ()).expect("separate TLS warmup");
    assert!(trace.rows.is_empty());
    assert!(!trace.overflow);
    assert_eq!(trace.actual_bytes, 0);
}

fn equal_metrics(left: &NativeMetrics, right: &NativeMetrics) {
    macro_rules! compare {
        ($($field:ident),+ $(,)?) => {{
            // Exhaustive destructuring refuses a future unlisted metric field.
            let NativeMetrics { $($field: _,)+ } = left;
            $(assert_eq!(left.$field, right.$field, stringify!($field));)+
        }};
    }
    compare!(occurrences,
        unique,
        row_sizes,
        occurrence_bytes,
        lookup_bytes,
        message_header_bytes,
        message_bytes,
        plan_bytes,
        admission_scratch_peak,
        retained_bound_bytes,
        metadata_admitted_bytes,
        metadata_peak,
        diagnostic_transient_peak,
        emitter_transient_bound,
        count_call_scratch_peak,
        render_call_scratch_peak,
        allocation_attempts,
        failed_allocation,
        sort_comparisons,
        inventory_rows,
        prefix_preflight_rows,
        coordinate_rows,
        source_prefix_bound,
        source_bytes,
        source_scalars,
        message_count_bytes,
        message_render_bytes,
        transfer_cells,
        transfer_inventory_visits,
        count_bytes,
        render_bytes,
        count_expansions,
        count_expansion_kinds,
        render_expansion_kinds,
        render_expansions,
        count_ordinary_visits,
        render_ordinary_visits,
        count_predecessor_visits,
        render_predecessor_visits,
        count_borrow_projection_visits,
        render_borrow_projection_visits);
}

fn equal_diagnostic(left: &FrontendDiagnostic, right: &FrontendDiagnostic) {
    let FrontendDiagnostic { code: _, stage: _, message: _, primary: _, secondary: _, notes: _ } = left;
    assert_eq!(left.code, right.code);
    assert_eq!(left.stage, right.stage);
    assert_eq!(left.message, right.message);
    assert_eq!(left.primary, right.primary);
    assert_eq!(left.secondary, right.secondary);
    assert_eq!(left.notes, right.notes);
}

fn result_payload(result: &NativeResult) -> usize {
    match result {
        Ok(text) => text.capacity(),
        Err(diagnostic) => {
            assert!(diagnostic.secondary.is_empty() && diagnostic.notes.is_empty());
            size_of::<FrontendDiagnostic>().checked_add(diagnostic.message.capacity()).unwrap()
                .checked_add(diagnostic.secondary.capacity().checked_mul(size_of::<(Span, String)>()).unwrap()).unwrap()
                .checked_add(diagnostic.notes.capacity().checked_mul(size_of::<String>()).unwrap()).unwrap()
        }
    }
}

fn equal_result(left: &NativeResult, right: &NativeResult) {
    match (left, right) {
        (Ok(a), Ok(b)) => assert_eq!(a.as_bytes(), b.as_bytes()),
        (Err(a), Err(b)) => equal_diagnostic(a, b),
        _ => panic!("disabled/enabled result kind changed"),
    }
}

fn trace_oracle(trace: &Trace, empty: bool) {
    assert_eq!(trace.limit, ROWS);
    assert_eq!(trace.requested_bytes, ROWS * size_of::<Event>());
    assert_eq!(trace.actual_capacity, trace.rows.capacity());
    assert_eq!(trace.actual_bytes, trace.actual_capacity.checked_mul(size_of::<Event>()).unwrap());
    assert_eq!(trace.observer_storage_cap, TRACE_BYTES_CAP);
    assert!(trace.actual_bytes <= TRACE_BYTES_CAP && !trace.overflow);
    if empty { assert!(trace.rows.is_empty()); return; }
    let expected = [(Site::Callers, usize::MAX, size_of::<Vec<usize>>()),
        (Site::Remaining, usize::MAX, size_of::<usize>()),
        (Site::CallReady, usize::MAX, size_of::<usize>()),
        (Site::Bounds, usize::MAX, size_of::<Bound>()),
        (Site::Incoming, 0, size_of::<usize>()),
        (Site::CfgReady, 0, size_of::<usize>())];
    assert_eq!(trace.rows.len(), expected.len());
    for (event, (site, owner, element_bytes)) in trace.rows.iter().zip(expected) {
        assert_eq!((event.site, event.owner, event.actor), (site, owner, usize::MAX));
        assert_eq!((event.len_before, event.capacity_before, event.len_after), (0, 0, 1));
        assert_eq!(event.element_bytes, element_bytes);
        assert!(event.capacity_after >= event.len_after);
    }
}

#[test]
fn native_graph_empty_record_disabled_enabled_first_baseline() {
    let (sources, raw, schedule) = consumer_fixtures::empty_record();
    assert_eq!(schedule.entry, hir::DefId(0));
    assert_eq!(schedule.result, Scalar::Unit);
    assert_eq!(schedule.fuel(), 17);
    assert_eq!(schedule.events.iter().map(|x| x.1).collect::<Vec<_>>(), [7, 1, 2, 2, 2, 1, 2]);
    let entry = schedule.entry;
    drop(schedule);
    let source = sources.get(crate::frontend::source::SourceFileId(0));
    assert_eq!(source.path(), "raw-owned-consumers.ox");
    assert_eq!(source.text().len(), 8192);
    assert!(source.text().as_bytes().chunks_exact(2).all(|pair| pair == b"x\n"));
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let fixture_bytes = sources.observer_capacity_bytes().unwrap()
        .checked_add(witness.observer_empty_record_capacity_bytes().unwrap()).unwrap();
    assert!(fixture_bytes <= FIXTURE_PAYLOAD_CAP);
    for case in 0..3 {
        let mut control = NativeControl { fuel: 17, ..NativeControl::default() };
        control.limits.ir_bytes = 8192;
        if case != 0 { control.limits.metadata_bytes = 0; }
        let input = Input { witness: &witness, sources: &sources,
            entry: (case != 2).then_some(entry), control };
        warm_tls();
        let baseline = invoke(&input);
        assert!(result_payload(&baseline.result) <= OUTPUT_PAYLOAD_CAP);
        warm_tls();
        let action = || invoke(&input);
        let carriers = carrier_inventory(&action);
        assert!(carriers.fixed_upper_bound <= ADDITIONAL_FIXED_CAP);
        assert_eq!(carriers.event, 64);
        assert_eq!(carriers.action, size_of::<&Input<'_>>());
        let carrier_signature = [carriers.input, carriers.action, carriers.observation,
            carriers.observed_result, carriers.native_result, carriers.accounting,
            carriers.diagnostic, carriers.event];
        assert_eq!(carriers.observer_roles, [72, 72, 80, 16, 16, 0]);
        let (candidate, trace) = observe::observe(ROWS, TRACE_BYTES_CAP, action).unwrap();
        assert!(result_payload(&candidate.result) <= OUTPUT_PAYLOAD_CAP);
        equal_result(&baseline.result, &candidate.result);
        equal_metrics(&baseline.metrics, &candidate.metrics);
        trace_oracle(&trace, case == 2);
        match case {
            0 => {
                let text = candidate.result.as_ref().unwrap();
                assert_eq!(candidate.metrics.count_bytes, text.len());
                assert_eq!(candidate.metrics.render_bytes, text.len());
                assert!(!text.contains("%fuel = alloca"));
            }
            1 | 2 => {
                let error = candidate.result.as_ref().unwrap_err();
                assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
                assert!(error.secondary.is_empty() && error.notes.is_empty());
                let expected = if case == 1 {
                    assert_eq!(error.primary, Some(Span { file: crate::frontend::source::SourceFileId(0), start: 0, end: 1 }));
                    assert_eq!(error.message, "native owned admission metadata bytes limit exceeded (0)");
                    "error[E0700] (native-admission): native owned admission metadata bytes limit exceeded (0)\n  --> raw-owned-consumers.ox:1:1\n"
                } else {
                    assert_eq!(error.primary, None);
                    assert_eq!(error.message, "native compile requires a declared zero-argument main");
                    assert_eq!(candidate.metrics.plan_bytes, 0);
                    "error[E0700] (native-admission): native compile requires a declared zero-argument main\n"
                };
                let rendered = error.render_human(&sources);
                assert!(rendered.capacity() <= RENDERED_DIAGNOSTIC_CAP);
                assert_eq!(rendered, expected);
                drop(rendered);
                assert_eq!(candidate.metrics.allocation_attempts, 0);
                assert_eq!((candidate.metrics.count_bytes, candidate.metrics.render_bytes), (0, 0));
            }
            _ => unreachable!(),
        }
        // Deliberately outside action: this exposes all qualified carrier types
        // in the fresh driver artifact and retains an auditable small receipt.
        println!("native_graph_first case={case} fixture_bytes={fixture_bytes} carriers={carriers:?} signature={carrier_signature:?} trace_actual_bytes={} baseline_payload={} candidate_payload={}",
            trace.actual_bytes, result_payload(&baseline.result), result_payload(&candidate.result));
        // Fixed receipt only after the observed action and all case/parity checks.
        // No intermediate String, extra graph traversal, or native invocation.
        assert!(trace.rows.len() <= ROWS);
        println!("native_graph_metrics_v1 case={case} rows={} admission_scratch_peak={} retained_bound_bytes={} plan_bytes={} metadata_admitted_bytes={} metadata_peak={}",
            trace.rows.len(), candidate.metrics.admission_scratch_peak,
            candidate.metrics.retained_bound_bytes, candidate.metrics.plan_bytes,
            candidate.metrics.metadata_admitted_bytes, candidate.metrics.metadata_peak);
        for (row, event) in trace.rows.iter().enumerate() {
            // Exhaustive pattern makes a future Event field require receipt review.
            let Event { site, owner, actor, len_before, capacity_before,
                len_after, capacity_after, element_bytes } = event;
            println!("native_graph_capacity_v1 case={case} row={row} site={site:?} owner={owner} actor={actor} len_before={len_before} capacity_before={capacity_before} len_after={len_after} capacity_after={capacity_after} element_bytes={element_bytes}");
        }
        // Baseline/candidate/trace drop here before the next case's warmup.
    }
}

#[test]
fn native_graph_observer_panic_cleanup_and_thread_isolation() {
    warm_tls();
    let unwind = std::panic::catch_unwind(|| {
        let _ = observe::observe(1, TRACE_BYTES_CAP, || panic!("fixed observer cleanup control"));
    });
    assert!(unwind.is_err());
    warm_tls();
    let parent_values = vec![1usize];
    // One fixed child is a helper control, never part of native observation or
    // an allocation-free claim. Its creation, TLS and fixture allocations count.
    let ((), parent_trace) = observe::observe(1, TRACE_BYTES_CAP, || {
        let child = std::thread::Builder::new().stack_size(2 * 1024 * 1024).spawn(|| {
            warm_tls();
            let child_values = vec![2usize];
            observe::observe(1, TRACE_BYTES_CAP, || {
                observe::record(Site::Remaining, 7, 8, (0, 0), &child_values);
            }).unwrap().1
        }).unwrap();
        let child_trace = child.join().unwrap();
        assert_eq!(child_trace.rows.len(), 1);
        assert_eq!((child_trace.rows[0].owner, child_trace.rows[0].actor), (7, 8));
        observe::record(Site::Remaining, 1, 2, (0, 0), &parent_values);
    }).unwrap();
    assert_eq!(parent_trace.rows.len(), 1);
    assert_eq!((parent_trace.rows[0].owner, parent_trace.rows[0].actor), (1, 2));
    warm_tls();
}
