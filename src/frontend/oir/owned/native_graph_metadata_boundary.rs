//! Identical child of native_graph_baseline in both source authorities.
use super::*;

const BOUNDARY_BUDGETS: [[usize; 7]; 4] = [
    [0, 343, 344, 345, 33007, 33008, 33009],
    [0, 727, 728, 729, 33319, 33320, 33321],
    [0, 2167, 2168, 2169, 34567, 34568, 34569],
    [0, 439, 440, 441, 33007, 33008, 33009],
];

fn boundary_phase_oracle(observation: &NativeObservation, trace: &Trace, fixture: usize, budget: usize, missing: bool) {
    let m = &observation.metrics;
    assert_eq!(m.failed_allocation, None);
    if missing {
        // Parent inventory reserves two NativeObservation carriers; here only
        // one is live, and the spare covers this fixed zero-metrics oracle.
        assert!(size_of::<NativeMetrics>() <= size_of::<NativeObservation>());
        equal_metrics(m, &NativeMetrics::default());
        let error = observation.result.as_ref().unwrap_err();
        assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
        assert_eq!(error.message, "native compile requires a declared zero-argument main");
        assert_eq!(error.primary, None);
        return;
    }
    let functions = [1usize, 2, 6, 1][fixture];
    let bound = trace.rows.iter().find(|event| event.site == Site::Bounds).unwrap();
    let raw_bound_bytes = bound.capacity_after.checked_mul(bound.element_bytes).unwrap();
    let b = |i: usize| trace.rows[i].capacity_after.checked_mul(trace.rows[i].element_bytes).unwrap();
    let add = |a: usize, b: usize| a.checked_add(b).unwrap();
    let scratch = match fixture {
        0 => (0..6).map(b).try_fold(0usize, usize::checked_add).unwrap(),
        1 => add((0..5).map(b).try_fold(0usize, usize::checked_add).unwrap(),
            add(b(5), b(6)).max(add(b(8), b(9)))),
        2 => {
            let fixed = [0, 1, 6, 8].into_iter().map(b).try_fold(0usize, usize::checked_add).unwrap();
            let initial = add(fixed, b(7));
            let mut peak = initial.max(add(add(initial, b(9)), b(10)));
            let final_ready = add(fixed, b(15));
            for start in [16, 19, 22, 25, 28] {
                peak = peak.max(add(add(final_ready, b(start)), b(start + 1)));
            }
            peak
        }
        3 => add(add((0..4).map(b).try_fold(0usize, usize::checked_add).unwrap(), b(4)),
            (5..14).map(b).max().unwrap()),
        _ => unreachable!(),
    };
    assert_eq!(m.admission_scratch_peak, scratch);
    let admission = m.plan_bytes.checked_add(m.admission_scratch_peak).unwrap();
    match &observation.result {
        Ok(text) => {
            assert!(admission <= budget && m.metadata_peak <= budget);
            assert_eq!(m.retained_bound_bytes, functions.checked_mul(size_of::<Bound>()).unwrap());
            assert!(text.len() <= [8192, 16384, 16384, 16384][fixture]);
            assert_eq!(m.count_bytes, text.len());
            assert_eq!(m.render_bytes, text.len());
        }
        Err(error) => {
            assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
            assert_eq!(error.primary, Some(Span { file: crate::frontend::source::SourceFileId(0), start: 0, end: 1 }));
            let (phase, amount) = error.message.strip_prefix("native owned ").unwrap().split_once(" metadata bytes limit exceeded (").unwrap();
            let amount = amount.strip_suffix(')').unwrap();
            assert_eq!(amount.parse::<usize>().unwrap(), budget);
            assert!(!amount.is_empty() && (amount == "0" || !amount.starts_with('0')));
            assert_eq!((m.count_bytes, m.render_bytes), (0, 0));
            match phase {
                "admission" => {
                    assert!(admission > budget);
                    assert_eq!(m.metadata_peak, admission);
                    assert_eq!(m.metadata_admitted_bytes, 0);
                    assert_eq!(m.retained_bound_bytes, raw_bound_bytes);
                    assert_eq!(m.allocation_attempts, 0);
                }
                "diagnostic" => {
                    assert!(admission <= budget && m.metadata_admitted_bytes > budget);
                    assert_eq!(m.retained_bound_bytes, functions.checked_mul(size_of::<Bound>()).unwrap());
                    // The request is recorded before this gate, peak need not reach it.
                    assert_eq!(m.allocation_attempts, 0);
                }
                "emission" => {
                    assert!(admission <= budget && m.metadata_admitted_bytes <= budget);
                    assert_eq!(m.retained_bound_bytes, functions.checked_mul(size_of::<Bound>()).unwrap());
                    let requested = m.plan_bytes.checked_add(m.retained_bound_bytes).unwrap()
                        .checked_add(m.lookup_bytes).unwrap().checked_add(m.message_header_bytes).unwrap()
                        .checked_add(m.emitter_transient_bound).unwrap();
                    assert!(requested > budget && m.metadata_peak >= requested);
                }
                _ => panic!("outside closed metadata gate scope"),
            }
        }
    }
}

fn run_boundary(witness: &VerifiedOwnedProgram, sources: &SourceMap, fixture: usize, fixture_bytes: usize) {
    let (fuel, ir_bytes, rows, trace_cap) = [(17, 8192, ROWS, TRACE_BYTES_CAP),
        (52, OUTPUT_PAYLOAD_CAP, RELAY_ROWS, RELAY_TRACE_BYTES_CAP),
        (52, OUTPUT_PAYLOAD_CAP, STAR_ROWS, STAR_TRACE_BYTES_CAP),
        (20, OUTPUT_PAYLOAD_CAP, CFG_FANOUT_ROWS, CFG_FANOUT_TRACE_BYTES_CAP)][fixture];
    let mut totals = BoundaryCount { lines: 0, bytes: 0 };
    for case in 0..8 {
        let budget = if case == 7 { 0 } else { BOUNDARY_BUDGETS[fixture][case] };
        let mut control = NativeControl { fuel, ..NativeControl::default() };
        control.limits.ir_bytes = ir_bytes;
        control.limits.metadata_bytes = budget;
        assert_eq!(control.limits.cost, Limits::DEFAULT.cost);
        let input = Input { witness, sources, entry: (case != 7).then_some(hir::DefId(0)), control };
        warm_tls();
        let action = || invoke(&input);
        let carriers = carrier_inventory(&action);
        // Additional named roles only; fresh ELF layout/lowering must verify them.
        let boundary_roles = boundary_serializer_carriers()
            .checked_add(size_of::<[[usize; 7]; 4]>()).unwrap()
            // Fixed schedule oracle (13 words) plus driver/phase scalar roles.
            .checked_add(64 * size_of::<usize>()).unwrap();
        let named_carriers = carriers.fixed_upper_bound.checked_add(boundary_roles).unwrap();
        assert!(named_carriers <= ADDITIONAL_FIXED_CAP);
        if case == 0 { totals = emit_boundary_fixture_begin(fixture, fixture_bytes, named_carriers); }
        let (observation, trace) = observe::observe(rows, trace_cap, action).unwrap();
        match fixture {
            0 => trace_oracle(&trace, case == 7),
            1 => relay_trace_oracle(&trace, case == 7),
            2 => { let _ = star_trace_oracle(&trace, case == 7); }
            3 => { let _ = cfg_fanout_trace_oracle(&trace, case == 7); }
            _ => unreachable!(),
        }
        let count = emit_boundary_case(BoundaryCaseIdentity { fixture, case, budget, entry_present: case != 7 },
            &observation, &trace, sources);
        boundary_phase_oracle(&observation, &trace, fixture, budget, case == 7);
        totals.lines = totals.lines.checked_add(count.lines).unwrap();
        totals.bytes = totals.bytes.checked_add(count.bytes).unwrap();
        drop(trace);
        drop(observation);
    }
    let final_count = emit_boundary_fixture_end(fixture, 8, totals);
    assert!(final_count.bytes <= 786944);
}

macro_rules! boundary_fixture {
    ($name:ident, $fixture:ident, $accessor:ident, $index:expr, $result:expr, $costs:expr) => {
        #[test]
        fn $name() {
            let (sources, raw, schedule) = consumer_fixtures::$fixture();
            assert_eq!(schedule.entry, hir::DefId(0));
            assert_eq!(schedule.result, $result);
            let costs = $costs;
            assert_eq!(schedule.events.len(), costs.len());
            for (i, ((span, actual), expected)) in schedule.events.iter().zip(costs).enumerate() {
                assert_eq!(*actual, expected);
                assert_eq!(*span, Span { file: crate::frontend::source::SourceFileId(0), start: 2 * i, end: 2 * i + 1 });
            }
            assert_eq!(schedule.fuel(), [17, 52, 52, 20][$index]);
            drop(schedule);
            let source = sources.get(crate::frontend::source::SourceFileId(0));
            assert_eq!(source.path(), "raw-owned-consumers.ox");
            assert_eq!(source.text().len(), 8192);
            assert!(source.text().as_bytes().chunks_exact(2).all(|pair| pair == b"x\n"));
            let witness = verified::verify_owned(raw, &sources).unwrap();
            let fixture_bytes = sources.observer_capacity_bytes().unwrap()
                .checked_add(witness.$accessor().unwrap()).unwrap();
            assert!(fixture_bytes <= FIXTURE_PAYLOAD_CAP);
            run_boundary(&witness, &sources, $index, fixture_bytes);
        }
    };
}
boundary_fixture!(native_metadata_boundary_empty, empty_record, observer_empty_record_capacity_bytes,
    0, Scalar::Unit, [7, 1, 2, 2, 2, 1, 2]);
boundary_fixture!(native_metadata_boundary_relay, owned_relay, observer_owned_relay_capacity_bytes,
    1, Scalar::I32(73), [21, 1, 1, 2, 2, 2, 8, 3, 1, 2, 2, 2, 5]);
boundary_fixture!(native_metadata_boundary_star, fixed_owned_relay_star, observer_fixed_owned_relay_star_capacity_bytes,
    2, Scalar::I32(73), [21, 1, 1, 2, 2, 2, 8, 3, 1, 2, 2, 2, 5]);
boundary_fixture!(native_metadata_boundary_cfg, fixed_empty_record_cfg_fanout, observer_fixed_empty_record_cfg_fanout_capacity_bytes,
    3, Scalar::Unit, [8, 1, 2, 2, 2, 1, 1, 1, 2]);

// Source-only section for the shared native_graph_metadata_boundary child module.
// The assembled child begins with `use super::*;`. No existing source is edited.
const BOUNDARY_LINE_CAP: usize = 256;
const BOUNDARY_CASE_LINES: usize = 384;
const BOUNDARY_CHUNK_BYTES: usize = 64;
const BOUNDARY_RESULT_CAP: usize = 16384;
const BOUNDARY_RENDER_CAP: usize = 1024;
const BOUNDARY_MESSAGE_CAP: usize = 512;
const BOUNDARY_STATIC_TEXT_CAP: usize = 128;
const _: () = assert!(size_of::<usize>() == 8);

#[derive(Clone, Copy)]
struct BoundaryCaseIdentity {
    fixture: usize,
    case: usize,
    budget: usize,
    entry_present: bool,
}

#[derive(Clone, Copy, Default)]
struct BoundaryCount {
    lines: usize,
    bytes: usize,
}

struct BoundaryLine {
    bytes: [u8; BOUNDARY_LINE_CAP],
    used: usize,
}

impl BoundaryLine {
    fn new(tag: &[u8]) -> Self {
        let mut line = Self { bytes: [0; BOUNDARY_LINE_CAP], used: 0 };
        line.raw(b"NB1|");
        line.raw(tag);
        line
    }

    fn raw(&mut self, bytes: &[u8]) {
        let end = self.used.checked_add(bytes.len()).unwrap();
        assert!(end <= BOUNDARY_LINE_CAP, "boundary receipt line cap");
        self.bytes[self.used..end].copy_from_slice(bytes);
        self.used = end;
    }

    fn text(&mut self, bytes: &[u8]) {
        self.raw(b"|");
        self.raw(bytes);
    }

    fn number(&mut self, mut value: usize) {
        let mut decimal = [0u8; 20];
        let mut start = decimal.len();
        loop {
            start -= 1;
            decimal[start] = b'0' + (value % 10) as u8;
            value /= 10;
            if value == 0 { break; }
        }
        self.text(&decimal[start..]);
    }

    fn emit(mut self, count: &mut BoundaryCount) {
        self.raw(b"\n");
        let next_lines = count.lines.checked_add(1).unwrap();
        let next_bytes = count.bytes.checked_add(self.used).unwrap();
        assert!(next_lines <= BOUNDARY_CASE_LINES, "boundary receipt case line cap");
        // Direct bytes: no formatting String or dynamically sized receipt buffer.
        // The run gate must use --nocapture and one exact selector per process.
        let stdout = std::io::stdout();
        let mut locked = stdout.lock();
        std::io::Write::write_all(&mut locked, &self.bytes[..self.used])
            .expect("boundary receipt stdout");
        count.lines = next_lines;
        count.bytes = next_bytes;
    }
}

fn boundary_chunk_count(bytes: usize) -> usize {
    bytes.checked_add(BOUNDARY_CHUNK_BYTES - 1).unwrap() / BOUNDARY_CHUNK_BYTES
}

fn boundary_hex(kind: &[u8], bytes: &[u8], count: &mut BoundaryCount) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (index, chunk) in bytes.chunks(BOUNDARY_CHUNK_BYTES).enumerate() {
        let mut line = BoundaryLine::new(b"H");
        line.text(kind);
        line.number(index);
        line.number(chunk.len());
        line.raw(b"|");
        for byte in chunk {
            line.raw(&[HEX[(byte >> 4) as usize], HEX[(byte & 15) as usize]]);
        }
        line.emit(count);
    }
}

fn boundary_payload(kind: &[u8], bytes: &[u8], capacity: Option<usize>, count: &mut BoundaryCount) {
    let mut line = BoundaryLine::new(b"P");
    line.text(kind);
    line.number(bytes.len());
    line.text(if capacity.is_some() { b"c" } else { b"b" });
    line.number(capacity.unwrap_or(0));
    line.number(boundary_chunk_count(bytes.len()));
    line.emit(count);
    boundary_hex(kind, bytes, count);
}

fn boundary_metrics(metrics: &NativeMetrics, count: &mut BoundaryCount) {
    // Exhaustive destructuring intentionally makes every added field a compile error.
    let NativeMetrics {
        occurrences, unique, row_sizes, occurrence_bytes, lookup_bytes,
        message_header_bytes, message_bytes, plan_bytes, admission_scratch_peak,
        retained_bound_bytes, metadata_admitted_bytes, metadata_peak,
        diagnostic_transient_peak, emitter_transient_bound, count_call_scratch_peak,
        render_call_scratch_peak, allocation_attempts, failed_allocation,
        sort_comparisons, inventory_rows, prefix_preflight_rows, coordinate_rows,
        source_prefix_bound, source_bytes, source_scalars, message_count_bytes,
        message_render_bytes, transfer_cells, transfer_inventory_visits, count_bytes,
        render_bytes, count_expansions, count_expansion_kinds, render_expansion_kinds,
        render_expansions, count_ordinary_visits, render_ordinary_visits,
        count_predecessor_visits, render_predecessor_visits,
        count_borrow_projection_visits, render_borrow_projection_visits,
    } = metrics;
    macro_rules! scalar {
        ($index:expr, $field:ident) => {{
            let mut line = BoundaryLine::new(b"M");
            line.number($index);
            line.text(stringify!($field).as_bytes());
            line.text(b"u");
            line.number(*$field);
            line.emit(count);
        }};
    }
    macro_rules! array {
        ($index:expr, $field:ident, $width:expr) => {{
            let _: &[usize; $width] = $field;
            let mut line = BoundaryLine::new(b"M");
            line.number($index);
            line.text(stringify!($field).as_bytes());
            line.text(b"a");
            line.number($width);
            for value in $field { line.number(*value); }
            line.emit(count);
        }};
    }
    scalar!(0, occurrences);
    scalar!(1, unique);
    array!(2, row_sizes, 4);
    scalar!(3, occurrence_bytes);
    scalar!(4, lookup_bytes);
    scalar!(5, message_header_bytes);
    scalar!(6, message_bytes);
    scalar!(7, plan_bytes);
    scalar!(8, admission_scratch_peak);
    scalar!(9, retained_bound_bytes);
    scalar!(10, metadata_admitted_bytes);
    scalar!(11, metadata_peak);
    scalar!(12, diagnostic_transient_peak);
    scalar!(13, emitter_transient_bound);
    scalar!(14, count_call_scratch_peak);
    scalar!(15, render_call_scratch_peak);
    scalar!(16, allocation_attempts);
    let mut failure = BoundaryLine::new(b"M");
    failure.number(17);
    failure.text(b"failed_allocation");
    match failed_allocation {
        None => failure.text(b"n"),
        Some(value) => {
            failure.text(b"s");
            failure.number(value.len());
            failure.number(boundary_chunk_count(value.len()));
        }
    }
    failure.emit(count);
    if let Some(value) = failed_allocation {
        assert!(value.len() <= BOUNDARY_STATIC_TEXT_CAP, "boundary failure text cap");
        boundary_hex(b"failure", value.as_bytes(), count);
    }
    array!(18, sort_comparisons, 4);
    scalar!(19, inventory_rows);
    scalar!(20, prefix_preflight_rows);
    scalar!(21, coordinate_rows);
    scalar!(22, source_prefix_bound);
    scalar!(23, source_bytes);
    scalar!(24, source_scalars);
    scalar!(25, message_count_bytes);
    scalar!(26, message_render_bytes);
    scalar!(27, transfer_cells);
    scalar!(28, transfer_inventory_visits);
    scalar!(29, count_bytes);
    scalar!(30, render_bytes);
    scalar!(31, count_expansions);
    array!(32, count_expansion_kinds, 3);
    array!(33, render_expansion_kinds, 3);
    scalar!(34, render_expansions);
    scalar!(35, count_ordinary_visits);
    scalar!(36, render_ordinary_visits);
    scalar!(37, count_predecessor_visits);
    scalar!(38, render_predecessor_visits);
    scalar!(39, count_borrow_projection_visits);
    scalar!(40, render_borrow_projection_visits);
}

fn boundary_site(site: Site) -> usize {
    match site {
        Site::Callers => 0, Site::Remaining => 1, Site::CallerEdges => 2,
        Site::CallReady => 3, Site::Bounds => 4, Site::Incoming => 5, Site::CfgReady => 6,
    }
}

fn boundary_trace(trace: &Trace, count: &mut BoundaryCount) {
    let Trace { rows, limit, overflow, requested_bytes, actual_capacity,
        actual_bytes, observer_storage_cap } = trace;
    assert!(rows.len() <= 31 && *limit <= 38, "boundary trace row bound");
    assert_eq!(*actual_capacity, rows.capacity());
    assert_eq!(*actual_bytes, actual_capacity.checked_mul(size_of::<Event>()).unwrap());
    assert_eq!(*requested_bytes, limit.checked_mul(size_of::<Event>()).unwrap());
    assert!(*actual_bytes <= *observer_storage_cap && *observer_storage_cap <= 4096);
    assert!(!*overflow && rows.len() <= *limit);
    let mut line = BoundaryLine::new(b"T");
    for value in [rows.len(), *limit, usize::from(*overflow), *requested_bytes,
        *actual_capacity, *actual_bytes, *observer_storage_cap] { line.number(value); }
    line.emit(count);
    // Keep raw Bound vector capacity separate from phase-dependent retained metric.
    let mut bounds_seen = 0usize;
    let mut bounds_bytes = 0usize;
    for event in rows {
        if event.site == Site::Bounds {
            bounds_seen = bounds_seen.checked_add(1).unwrap();
            bounds_bytes = event.capacity_after.checked_mul(event.element_bytes).unwrap();
        }
    }
    assert!(bounds_seen <= 1, "boundary multiple bounds vectors");
    let mut line = BoundaryLine::new(b"B");
    line.number(bounds_seen);
    line.number(bounds_bytes);
    line.emit(count);
}

fn boundary_events(trace: &Trace, count: &mut BoundaryCount) {
    for (index, event) in trace.rows.iter().enumerate() {
        let Event { site, owner, actor, len_before, capacity_before,
            len_after, capacity_after, element_bytes } = event;
        let mut line = BoundaryLine::new(b"G");
        for value in [index, boundary_site(*site), *owner, *actor, *len_before,
            *capacity_before, *len_after, *capacity_after, *element_bytes,
            capacity_after.checked_mul(*element_bytes).unwrap()] { line.number(value); }
        line.emit(count);
    }
}

fn boundary_result(result: &NativeResult, sources: &SourceMap, count: &mut BoundaryCount) {
    match result {
        Ok(ir) => {
            assert!(ir.capacity() <= BOUNDARY_RESULT_CAP, "boundary result capacity cap");
            let mut line = BoundaryLine::new(b"R");
            line.text(b"ok");
            line.number(ir.capacity());
            line.emit(count);
            boundary_payload(b"ir", ir.as_bytes(), Some(ir.capacity()), count);
        }
        Err(diagnostic) => {
            let FrontendDiagnostic { code, stage, message, primary, secondary, notes } = &**diagnostic;
            // Record actual counts/capacities even if a future shape invalidates scope.
            let mut line = BoundaryLine::new(b"D");
            for value in [secondary.len(), secondary.capacity(), notes.len(), notes.capacity()] {
                line.number(value);
            }
            line.emit(count);
            assert!(secondary.is_empty() && notes.is_empty(), "boundary diagnostic shape");
            assert!(message.capacity() <= BOUNDARY_MESSAGE_CAP, "boundary message capacity cap");
            assert!(code.len() <= BOUNDARY_STATIC_TEXT_CAP && stage.len() <= BOUNDARY_STATIC_TEXT_CAP,
                "boundary diagnostic static text cap");
            let payload = result_payload(result);
            assert!(payload <= BOUNDARY_RESULT_CAP, "boundary result capacity cap");
            let mut line = BoundaryLine::new(b"R");
            line.text(b"err");
            line.number(payload);
            line.emit(count);
            let mut line = BoundaryLine::new(b"S");
            line.number(usize::from(primary.is_some()));
            if let Some(Span { file, start, end }) = primary {
                line.number(file.0);
                line.number(*start);
                line.number(*end);
            }
            line.emit(count);
            boundary_payload(b"code", code.as_bytes(), None, count);
            boundary_payload(b"stage", stage.as_bytes(), None, count);
            boundary_payload(b"message", message.as_bytes(), Some(message.capacity()), count);
            // The one full human String is deliberately named, admitted, then dropped.
            let rendered: String = diagnostic.render_human(sources);
            assert!(rendered.capacity() <= BOUNDARY_RENDER_CAP, "boundary rendered capacity cap");
            boundary_payload(b"render", rendered.as_bytes(), Some(rendered.capacity()), count);
            drop(rendered);
        }
    }
}

fn emit_boundary_case(id: BoundaryCaseIdentity, observation: &NativeObservation,
    trace: &Trace, sources: &SourceMap) -> BoundaryCount
{
    assert!(id.fixture < 4 && id.case < 8);
    assert_eq!(id.entry_present, id.case != 7);
    assert!(id.budget <= Limits::DEFAULT.metadata_bytes);
    if !id.entry_present { assert_eq!(id.budget, 0); }
    let NativeObservation { result, metrics } = observation;
    let mut count = BoundaryCount::default();
    let mut line = BoundaryLine::new(b"C");
    for value in [id.fixture, id.case, id.budget, usize::from(id.entry_present)] { line.number(value); }
    line.emit(&mut count);
    boundary_trace(trace, &mut count);
    boundary_metrics(metrics, &mut count);
    boundary_events(trace, &mut count);
    boundary_result(result, sources, &mut count);
    let mut line = BoundaryLine::new(b"E");
    // Counts are exact preceding-frame counts, excluding this footer itself.
    for value in [id.fixture, id.case, count.lines, count.bytes,
        usize::from(metrics.failed_allocation.is_none())] { line.number(value); }
    line.emit(&mut count);
    count
}

fn emit_boundary_fixture_begin(fixture: usize, fixture_bytes: usize,
    named_carriers: usize) -> BoundaryCount
{
    assert!(fixture < 4 && fixture_bytes <= FIXTURE_PAYLOAD_CAP);
    assert!(named_carriers <= ADDITIONAL_FIXED_CAP);
    let mut count = BoundaryCount::default();
    let mut line = BoundaryLine::new(b"F");
    for value in [fixture, fixture_bytes, named_carriers] { line.number(value); }
    line.emit(&mut count);
    count
}

fn emit_boundary_fixture_end(fixture: usize, cases: usize, totals: BoundaryCount) -> BoundaryCount {
    assert!(fixture < 4 && cases == 8);
    assert!(totals.lines <= 1 + 8 * BOUNDARY_CASE_LINES);
    assert!(totals.bytes <= BOUNDARY_LINE_CAP * (1 + 8 * BOUNDARY_CASE_LINES));
    let mut count = BoundaryCount::default();
    let mut line = BoundaryLine::new(b"Z");
    for value in [fixture, cases, totals.lines, totals.bytes] { line.number(value); }
    line.emit(&mut count);
    BoundaryCount { lines: totals.lines.checked_add(count.lines).unwrap(),
        bytes: totals.bytes.checked_add(count.bytes).unwrap() }
}

// Named Rust-carrier contribution only; every fresh ELF needs its own layout
// and concrete closure/iterator inspection. This is not a stack high-water mark.
fn boundary_serializer_carriers() -> usize {
    let parts = [4 * size_of::<BoundaryLine>(), size_of::<[u8; 20]>(),
        size_of::<[u8; 2]>(), size_of::<BoundaryCaseIdentity>(),
        4 * size_of::<BoundaryCount>(), size_of::<std::slice::Chunks<'_, u8>>(),
        size_of::<std::iter::Enumerate<std::slice::Chunks<'_, u8>>>(),
        size_of::<std::slice::Iter<'_, u8>>(), size_of::<std::slice::Iter<'_, Event>>(),
        size_of::<std::iter::Enumerate<std::slice::Iter<'_, Event>>>(),
        size_of::<std::slice::Iter<'_, usize>>(), size_of::<std::array::IntoIter<usize, 10>>(),
        size_of::<std::array::IntoIter<usize, 7>>(), size_of::<std::array::IntoIter<usize, 5>>(),
        2 * size_of::<std::array::IntoIter<usize, 4>>(),
        size_of::<std::array::IntoIter<usize, 3>>(),
        size_of::<std::io::Stdout>(), size_of::<std::io::StdoutLock<'static>>(),
        size_of::<std::io::Result<()>>(), size_of::<Option<usize>>(),
        size_of::<String>(), 8 * size_of::<&[u8]>(), 8 * size_of::<usize>(),
        41 * size_of::<&usize>(), 8 * size_of::<&usize>(), 7 * size_of::<&usize>(),
        7 * size_of::<&usize>(),
        size_of::<[usize; 32]>(), size_of::<[usize; 32]>()];
    assert!(parts.len() <= 32);
    parts.into_iter().try_fold(0usize, usize::checked_add).unwrap()
}
