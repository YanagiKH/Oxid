//! Ordinary canonical scalar emission observations, compiled only for tests.
//!
//! Fixed TLS scalars sample the existing emitter; there is no allocating trace,
//! allocator replacement, private Emit entry, importer ledger, or native tool.
//! Source/AST/verified OIR are the prepared baseline. Capacities below describe
//! only retained Bounds, diagnostic Vec/String payloads, and exit-label payloads
//! at their sample sites. They exclude BTreeMap node bytes, admission scratch,
//! Diagnostic boxes, other transient Strings, allocator internals, stack, and RSS.
//! Map operation counts do not measure comparisons or qualify a library tariff.
//! Human-count bytes describe successful write_human calls; render_human itself
//! invokes write_human internally, so its completed bytes are recorded separately.
//! These representative events test stated formula inequalities, not every
//! format substitution, the whole work formula, or a CPU/time/allocation bound.
use super::*;
use crate::frontend::{declaration_index::WorkMeter, lexer, parser};
use std::{cell::RefCell, marker::PhantomData, mem::size_of};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Events {
    constructs: usize,
    human_counts: usize,
    counted_human_bytes: usize,
    human_renders: usize,
    rendered_human_bytes: usize,
    max_human_bytes: usize,
    escapes: usize,
    escaped_bytes: usize,
    contains: usize,
    inserts: usize,
    gets: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Pass {
    starts: usize,
    finishes: usize,
    bytes: usize,
    initial_capacity: usize,
    final_capacity: usize,
    label_frames: usize,
    labels: usize,
    max_label_payload_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Observation {
    // Preparation, authoritative count pass, authoritative render pass.
    phase: usize,
    events: [Events; 3],
    passes: [Pass; 2],
    overflow: bool,
    guarded: bool,
    bounds_capacity_bytes: usize,
    message_count: usize,
    message_vec_capacity_bytes: usize,
    message_string_capacity_bytes: usize,
    map_entries: usize,
}

thread_local! {
    static CURRENT: RefCell<Option<Observation>> = const { RefCell::new(None) };
}

// Never moved to another thread: Drop must clear the TLS that begin armed.
struct Guard(PhantomData<*mut ()>);

fn begin() -> Guard {
    CURRENT.with(|slot| {
        let mut slot = slot.borrow_mut();
        // Check before replacing: a rejected nested arm preserves its owner.
        assert!(slot.is_none(), "native observation already armed");
        *slot = Some(Observation::default());
    });
    Guard(PhantomData)
}

impl Guard {
    fn finish(self) -> Observation {
        CURRENT.with(|slot| slot.borrow_mut().take().expect("armed observation"))
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        CURRENT.with(|slot| *slot.borrow_mut() = None);
    }
}

fn update(f: impl FnOnce(&mut Observation)) {
    CURRENT.with(|slot| {
        if let Some(observation) = slot.borrow_mut().as_mut() {
            f(observation);
        }
    });
}

fn add(value: &mut usize, amount: usize, overflow: &mut bool) {
    match value.checked_add(amount) {
        Some(total) => *value = total,
        None => *overflow = true,
    }
}

fn capacity_bytes(capacity: usize, element: usize, overflow: &mut bool) -> usize {
    capacity.checked_mul(element).unwrap_or_else(|| {
        *overflow = true;
        0
    })
}

pub(super) enum Event {
    Construct,
    HumanCount,
    HumanRender,
    Escape,
    EscapedByte,
    Contains,
    Insert,
    Get,
}

pub(super) fn event(event: Event, amount: usize) {
    update(|observation| {
        let events = &mut observation.events[observation.phase];
        let value = match event {
            Event::Construct => &mut events.constructs,
            Event::HumanCount => &mut events.human_counts,
            Event::HumanRender => &mut events.human_renders,
            Event::Escape => &mut events.escapes,
            Event::EscapedByte => &mut events.escaped_bytes,
            Event::Contains => &mut events.contains,
            Event::Insert => &mut events.inserts,
            Event::Get => &mut events.gets,
        };
        add(value, amount, &mut observation.overflow);
    });
}

pub(super) fn human_bytes(render: bool, bytes: usize) {
    update(|observation| {
        let events = &mut observation.events[observation.phase];
        events.max_human_bytes = events.max_human_bytes.max(bytes);
        let value = if render {
            &mut events.rendered_human_bytes
        } else {
            &mut events.counted_human_bytes
        };
        add(value, bytes, &mut observation.overflow);
    });
}

pub(super) fn retained(
    bounds: &Vec<Bound>,
    diagnostics: Option<&GuardedDiagnostics>,
    guarded: bool,
) {
    update(|observation| {
        observation.guarded = guarded;
        observation.bounds_capacity_bytes = capacity_bytes(
            bounds.capacity(),
            size_of::<Bound>(),
            &mut observation.overflow,
        );
        if let Some(diagnostics) = diagnostics {
            observation.message_count = diagnostics.messages.len();
            observation.map_entries = diagnostics.ids.len();
            observation.message_vec_capacity_bytes = capacity_bytes(
                diagnostics.messages.capacity(),
                size_of::<String>(),
                &mut observation.overflow,
            );
            for message in &diagnostics.messages {
                add(
                    &mut observation.message_string_capacity_bytes,
                    message.capacity(),
                    &mut observation.overflow,
                );
            }
        }
    });
}

pub(super) fn start_pass(render: bool, out: &Emission) {
    update(|observation| {
        let index = usize::from(render);
        observation.phase = index + 1;
        let pass = &mut observation.passes[index];
        add(&mut pass.starts, 1, &mut observation.overflow);
        pass.initial_capacity = out.text.as_ref().map_or(0, String::capacity);
    });
}

pub(super) fn finish_pass(render: bool, out: &Emission) {
    update(|observation| {
        let pass = &mut observation.passes[usize::from(render)];
        add(&mut pass.finishes, 1, &mut observation.overflow);
        pass.bytes = out.len;
        pass.final_capacity = out.text.as_ref().map_or(0, String::capacity);
        observation.phase = 0;
    });
}

pub(super) fn labels(labels: &Vec<String>) {
    update(|observation| {
        let mut payload = capacity_bytes(
            labels.capacity(),
            size_of::<String>(),
            &mut observation.overflow,
        );
        for label in labels {
            add(&mut payload, label.capacity(), &mut observation.overflow);
        }
        let pass = &mut observation.passes[observation.phase - 1];
        add(&mut pass.label_frames, 1, &mut observation.overflow);
        add(&mut pass.labels, labels.len(), &mut observation.overflow);
        pass.max_label_payload_bytes = pass.max_label_payload_bytes.max(payload);
    });
}

fn is_inactive() -> bool {
    CURRENT.with(|slot| slot.borrow().is_none())
}

fn ordinary(path: &str, text: &str) -> (VerifiedProgram, SourceMap) {
    assert!(text.len() <= 128);
    let mut sources = SourceMap::new();
    let id = sources.add(path.to_owned(), text.to_owned());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    (lower_and_verify(&typed, &sources).unwrap(), sources)
}

fn dimensions(program: &VerifiedProgram) -> (emit_work::Dimensions, u64) {
    // This ordinary-source probe has its own test meter, outside observation.
    // It does not reuse, authorize, or replace any private importer meter.
    let meter = WorkMeter::default();
    let origin = program.program.functions[0].span;
    meter.debit(137, origin, "observation prior work").unwrap();
    let dimensions = emit_work::scan(
        program,
        emit_work::Upper {
            functions: 256,
            blocks: 4_096,
            slots: 8_192,
            definitions: 8_192,
        },
        &meter,
        origin,
    )
    .unwrap();
    let scan =
        4_096 + 128 * dimensions.functions + 256 * dimensions.blocks + 128 * dimensions.statements;
    assert_eq!(meter.used(), 137 + scan);
    (dimensions, scan)
}

fn observe(
    label: &str,
    path: &str,
    text: &str,
    entry: usize,
    guarded: bool,
) -> (emit_work::Dimensions, Observation, u64) {
    let (program, sources) = ordinary(path, text);
    let (dimensions, scan) = dimensions(&program);
    let cost = emit_cost::calculate(dimensions, path.len()).unwrap();
    assert!(is_inactive());
    let expected = program
        .native_module(Some(hir::DefId(entry)), &sources)
        .unwrap();
    assert!(is_inactive());
    let guard = begin();
    let actual = program
        .native_module(Some(hir::DefId(entry)), &sources)
        .unwrap();
    let observation = guard.finish();
    assert!(is_inactive());
    assert_eq!(actual, expected, "armed ordinary Result bytes: {label}");
    assert!(!observation.overflow);
    assert_eq!(observation.guarded, guarded);
    assert_eq!(observation.phase, 0);
    let count = observation.passes[0];
    let render = observation.passes[1];
    assert_eq!(
        (count.starts, count.finishes, render.starts, render.finishes),
        (1, 1, 1, 1)
    );
    assert_eq!((count.bytes, render.bytes), (actual.len(), actual.len()));
    assert_eq!((count.initial_capacity, count.final_capacity), (0, 0));
    assert!(render.initial_capacity >= count.bytes);
    assert_eq!(render.initial_capacity, render.final_capacity);
    assert_eq!(render.final_capacity, actual.capacity());
    let d = dimensions;
    let k = d.arithmetic_failures as usize;
    let guards = 1 + d.merges + d.statements + d.blocks;
    let attempts = (guards + d.arithmetic_failures) as usize;
    let human_bound = 71 + 6 * path.len();
    let preparation = observation.events[0];
    if guarded {
        assert!(d.maybe_cyclic);
        let unique = observation.message_count;
        assert!(unique > 0 && unique <= attempts);
        assert_eq!(preparation.contains, attempts);
        assert_eq!(
            (
                preparation.constructs,
                preparation.human_counts,
                preparation.human_renders,
                preparation.inserts
            ),
            (unique, unique, unique, unique)
        );
        assert_eq!(
            preparation.counted_human_bytes,
            preparation.rendered_human_bytes
        );
        assert_eq!(
            (
                preparation.escapes,
                preparation.escaped_bytes,
                preparation.gets
            ),
            (0, 0, 0)
        );
        assert_eq!(observation.map_entries, unique);
        assert!(observation.message_vec_capacity_bytes >= unique * size_of::<String>());
        assert!(observation.message_string_capacity_bytes >= preparation.rendered_human_bytes);
    } else {
        assert_eq!(preparation, Events::default());
        assert_eq!(observation.message_count, 0);
        assert_eq!(observation.message_vec_capacity_bytes, 0);
        assert_eq!(observation.message_string_capacity_bytes, 0);
        assert_eq!(observation.map_entries, 0);
    }
    // The proposal's finite-template byte envelope, excluding actual escaped
    // diagnostic bytes. This is an inequality test, never a serializer/oracle.
    let (g, j) = if guarded {
        (guards, attempts as u64)
    } else {
        (0, d.arithmetic_failures)
    };
    let template = 2_048
        + 128 * d.functions
        + 32 * d.parameters
        + 64 * d.places
        + 128 * d.blocks
        + 256 * d.merges
        + 2_048 * d.statements
        + 128 * d.calls
        + 32 * d.arguments
        + 1_024 * g
        + 256 * j;
    for (events, pass) in observation.events[1..].iter().zip(observation.passes) {
        assert_eq!(
            (events.contains, events.inserts, events.human_counts),
            (0, 0, 0)
        );
        if guarded {
            assert_eq!((events.constructs, events.human_renders), (0, 0));
            assert_eq!(events.gets, attempts);
            assert_eq!(events.escapes, observation.message_count);
            assert_eq!(events.escaped_bytes, preparation.rendered_human_bytes);
        } else {
            assert_eq!((events.constructs, events.human_renders), (2 * k, 2 * k));
            assert_eq!((events.gets, events.escapes), (0, k));
            assert_eq!(events.rendered_human_bytes, 2 * events.escaped_bytes);
        }
        assert!(events.max_human_bytes <= human_bound);
        assert!((events.escaped_bytes as u64) <= j * human_bound as u64);
        let fixed = pass.bytes.checked_sub(3 * events.escaped_bytes).unwrap();
        assert!((fixed as u64) <= template, "template bytes: {label}");
        assert_eq!(pass.label_frames as u64, d.functions);
        assert_eq!(pass.labels as u64, d.blocks);
        assert!(pass.max_label_payload_bytes > 0);
    }
    assert!(preparation.max_human_bytes <= human_bound);
    assert!(observation.bounds_capacity_bytes >= d.functions as usize * size_of::<Bound>());
    println!("ORDINARY_EMIT {label} path_utf8={} dimensions={d:?} scan={scan} passive_cost={cost:?} observed={observation:?} observer_bytes={} excluded_baseline_and_unobserved_allocations=true", path.len(), size_of::<RefCell<Option<Observation>>>());
    (d, observation, scan + cost.body)
}

const RICH: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked_hir_import/rich-source.txt"
));

#[test]
fn ordinary_emit_events_match_dimensions_and_finite_bounds() {
    for (label, text, entry, guarded) in [
        ("rich", RICH, 1, true),
        ("division", "fn main()->i32{return 4/2;}", 0, false),
        ("remainder", "fn main()->i32{return 4%2;}", 0, false),
        ("loop", "fn main()->i32{while false{}return 0;}", 0, true),
        (
            "unused-loop",
            "fn idle()->(){while false{}return;}fn main()->i32{return 0;}",
            1,
            true,
        ),
        ("merge", "fn main()->bool{return 4/2==2&&true;}", 0, false),
        (
            "guarded-merge",
            "fn main()->bool{while false{}return 4/2==2&&true;}",
            0,
            true,
        ),
        (
            "backward-acyclic",
            "fn main()->bool{return true&&(true&&false);}",
            0,
            false,
        ),
        ("literal", "fn main()->i32{return 0;}", 0, false),
        ("unit", "fn main()->(){return;}", 0, false),
    ] {
        let (d, _, total) = observe(label, "main.ox", text, entry, guarded);
        assert!(total < 256_000_000);
        match label {
            "rich" => {
                assert_eq!(
                    d,
                    emit_work::Dimensions {
                        functions: 2,
                        locals: 12,
                        places: 1,
                        parameters: 1,
                        blocks: 9,
                        statements: 11,
                        merges: 0,
                        calls: 2,
                        arguments: 2,
                        edges: 8,
                        arithmetic_failures: 1,
                        maybe_cyclic: true
                    }
                );
                assert_eq!(total, 8_721_664);
            }
            "division" | "remainder" => assert_eq!(
                d,
                emit_work::Dimensions {
                    functions: 1,
                    locals: 3,
                    blocks: 1,
                    statements: 3,
                    arithmetic_failures: 2,
                    ..emit_work::Dimensions::default()
                }
            ),
            "loop" => assert_eq!(
                d,
                emit_work::Dimensions {
                    functions: 1,
                    locals: 2,
                    blocks: 4,
                    statements: 2,
                    edges: 4,
                    maybe_cyclic: true,
                    ..emit_work::Dimensions::default()
                }
            ),
            "literal" => assert_eq!(
                d,
                emit_work::Dimensions {
                    functions: 1,
                    locals: 1,
                    blocks: 1,
                    statements: 1,
                    ..emit_work::Dimensions::default()
                }
            ),
            "backward-acyclic" => assert!(d.maybe_cyclic),
            "unused-loop" => assert_eq!((d.functions, d.calls), (2, 0)),
            "merge" | "guarded-merge" => assert_eq!(d.merges, 1),
            _ => {}
        }
    }
}

#[test]
fn ordinary_emit_control_unicode_paths_obey_human_and_llvm_byte_bounds() {
    // ASCII and multibyte controls, ordinary Unicode, quotes and backslashes.
    // Prepared before arming; this already-owned path exceeds source length.
    let path = "\0\u{7f}\u{80}雪🙂\"\\\n\t".repeat(256);
    assert!(path.len() > 3_367);
    let (_, _, division_work) = observe(
        "escaped-division",
        &path,
        "fn main()->i32{return 4/2;}",
        0,
        false,
    );
    assert!(division_work < 256_000_000);
    let (_, _, rich_work) = observe("escaped-rich", &path, RICH, 1, true);
    // Ordinary emission is still allowed. The disconnected passive proposal
    // would deny this work, even before adding existing importer work.
    assert!(rich_work > 256_000_000);
}

#[test]
fn ordinary_emit_observer_is_inactive_reset_and_unwind_safe() {
    assert!(is_inactive());
    event(Event::Construct, 1);
    assert!(is_inactive());
    let guard = begin();
    event(Event::Construct, 1);
    assert!(std::panic::catch_unwind(begin).is_err());
    assert_eq!(guard.finish().events[0].constructs, 1);
    assert!(is_inactive());
    assert!(std::panic::catch_unwind(|| {
        let _guard = begin();
        event(Event::Get, 1);
        panic!("observation unwind control");
    })
    .is_err());
    assert!(is_inactive());
    assert_eq!(begin().finish(), Observation::default());
    let guard = begin();
    event(Event::Construct, usize::MAX);
    event(Event::Construct, 1);
    assert!(guard.finish().overflow);
    assert_eq!(begin().finish(), Observation::default());
}
