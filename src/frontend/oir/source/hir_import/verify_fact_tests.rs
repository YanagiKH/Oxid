//! Supplied STF1 facts are compared only against genuinely checked candidate HIR.
//! These controls use the private Verify leaf and return fixed facts/rejections.
//! Original-owner fixtures apply on every host. Genuine ProjectSources loading
//! is qualified on Linux only; the public fixture is explicitly ignored elsewhere.
use super::{candidate, leaf, SourceOwner, Wire, CELLS, COLUMN_STARTS};
use crate::frontend::{
    declaration_index::IndexLimits,
    lexer,
    oir::{lower, verify},
    parser,
    project::budget::Allocator,
    source::{SourceMap, SourceView},
    typeck,
};

macro_rules! fixture {
    ($name:literal, $rows:literal, $reserves:literal) => {
        Fixture {
            name: $name,
            source: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import/",
                $name,
                "-source.txt"
            )),
            bytes: include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/checked_hir_import/",
                $name,
                "-success.bin"
            )),
            rows: $rows,
            reserves: $reserves,
        }
    };
}

#[derive(Clone, Copy)]
struct Fixture {
    name: &'static str,
    source: &'static str,
    bytes: &'static [u8],
    rows: usize,
    reserves: usize,
}

const ORIGINALS: &[Fixture] = &[
    fixture!("rich", 29, 16),
    fixture!("scalar-arithmetic", 19, 7),
    fixture!("scalar-boolean", 15, 7),
    fixture!("scalar-comparison", 31, 7),
    fixture!("scalar-unit", 13, 7),
    fixture!("scalar-loop", 15, 10),
    fixture!("scalar-assignment", 12, 7),
];

fn allocator() -> Allocator {
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(32).unwrap();
    allocator
}

fn assert_reserves(observed: &Allocator, expected: usize, trace_capacity: usize) {
    assert_eq!(observed.attempts, expected);
    assert_eq!(observed.trace.len(), expected);
    assert!(observed.trace.iter().all(|event| event.success));
    assert_eq!(observed.trace.capacity(), trace_capacity);
    assert!(!observed.observer_trace_overflow);
}

fn assert_no_lower_observations(snapshot: lower::measurement::Snapshot) {
    assert_eq!(snapshot.local_map_capacity_max, 0);
    assert_eq!(snapshot.expression_map_capacity_max, 0);
    assert_eq!(snapshot.loop_targets_capacity_max, 0);
    assert_eq!(snapshot.active_loops_capacity_max, 0);
    assert_eq!(snapshot.frames_capacity_max, 0);
    assert_eq!(snapshot.expression_frames_capacity_max, 0);
    assert_eq!(snapshot.scratch_payload_envelope_bytes, 0);
}

fn changed_fact(fixture: Fixture, cell: usize, value: i32) -> Vec<u8> {
    let original = Wire::decode(fixture.bytes, fixture.source.len()).unwrap();
    assert!(cell < usize::from(original.rows));
    assert_ne!(original.word(4, cell).unwrap(), value);
    let mut changed = fixture.bytes.to_vec();
    for (plane, byte) in value.to_le_bytes().into_iter().enumerate() {
        changed[COLUMN_STARTS[4] + plane * CELLS + cell] = byte;
    }
    let decoded = Wire::decode(&changed, fixture.source.len()).unwrap();
    assert_eq!(decoded.rows, original.rows);
    assert_eq!(decoded.word(4, cell).unwrap(), value);
    // Preserve source/OPA framing, all resolution cells, every other active
    // fact, and all inactive storage. Only one complete supplied fact changes.
    for (index, (&before, &after)) in fixture.bytes.iter().zip(&changed).enumerate() {
        if !(0..4).any(|plane| index == COLUMN_STARTS[4] + plane * CELLS + cell) {
            assert_eq!(before, after, "{} byte {index}", fixture.name);
        }
    }
    changed
}

fn reject_changed_fact(
    fixture: Fixture,
    owner: SourceOwner<'_>,
    baseline: &Allocator,
    cell: usize,
    value: i32,
) {
    let changed = changed_fact(fixture, cell, value);
    let mut observed = allocator();
    let trace_capacity = observed.trace.capacity();
    let type_guard = typeck::measurement::begin();
    let lower_guard = lower::measurement::begin();
    let verify_guard = verify::measurement::begin();
    let result = leaf::verify(
        owner,
        fixture.source.as_bytes(),
        &changed,
        &mut observed,
        IndexLimits::default(),
    );
    let typed = type_guard.finish();
    let lowered = lower_guard.finish();
    let verified = verify_guard.finish();
    match result {
        Err(leaf::VerifyRejected::Terminal(candidate::VerifyRejected::TypedMismatch {
            cells,
        })) => assert_eq!(cells, fixture.rows, "{} cell {cell}", fixture.name),
        other => panic!(
            "{} fact cell {cell} changed to {value} must reach complete typed comparison: {other:?}",
            fixture.name
        ),
    }
    assert_reserves(&observed, fixture.reserves, trace_capacity);
    for (actual, expected) in observed.trace.iter().zip(&baseline.trace) {
        assert_eq!(
            (
                actual.kind,
                actual.length,
                actual.element_bytes,
                actual.success
            ),
            (
                expected.kind,
                expected.length,
                expected.element_bytes,
                expected.success
            ),
            "{} cell {cell}: fact changes must preserve candidate reserves",
            fixture.name
        );
    }
    // The exact terminal rejection is emitted before lower::lower in run().
    // Genuine checker observations must exist, while inherited lower/verify
    // observations remain empty. These are stage controls, not heap-peak claims.
    assert!(typed.frame_bytes > 0);
    assert!(typed.bodies_capacity > 0);
    assert_no_lower_observations(lowered);
    assert_eq!(verified.successful_functions, 0);
    assert_eq!(verified.excluded_failed_functions, 0);
    assert_eq!(verified.per_function_capacity_max_bytes, 0);
}

struct Coverage {
    kinds: [bool; 37],
    types: [bool; 4],
    flow_masks: [bool; 16],
    cells: usize,
    typed: usize,
    flows: usize,
    unused: usize,
    mutations: usize,
}

impl Default for Coverage {
    fn default() -> Self {
        Self {
            kinds: [false; 37],
            types: [false; 4],
            flow_masks: [false; 16],
            cells: 0,
            typed: 0,
            flows: 0,
            unused: 0,
            mutations: 0,
        }
    }
}

fn exercise_fixture(fixture: Fixture, owner: SourceOwner<'_>, coverage: &mut Coverage) {
    let wire = Wire::decode(fixture.bytes, fixture.source.len()).unwrap();
    assert_eq!(usize::from(wire.rows), fixture.rows);
    let mut baseline = allocator();
    let trace_capacity = baseline.trace.capacity();
    let lower_guard = lower::measurement::begin();
    let verify_guard = verify::measurement::begin();
    let facts = leaf::verify(
        owner,
        fixture.source.as_bytes(),
        fixture.bytes,
        &mut baseline,
        IndexLimits::default(),
    )
    .unwrap();
    let lowered = lower_guard.finish();
    let verified = verify_guard.finish();
    assert!(facts.verified.candidate.equal);
    assert_eq!(facts.verified.typed_cells, fixture.rows);
    assert_eq!(
        facts.verified.candidate.allocation.reserves,
        fixture.reserves
    );
    assert_reserves(&baseline, fixture.reserves, trace_capacity);
    // Positive controls establish that the same fixtures reach both stages
    // with their original facts and that the observers are active.
    assert!(lowered.scratch_payload_envelope_bytes > 0);
    assert_eq!(verified.successful_functions, facts.verified.functions);
    assert!(verified.successful_functions > 0);
    assert_eq!(verified.excluded_failed_functions, 0);

    let before = coverage.mutations;
    for cell in 0..fixture.rows {
        let kind = usize::try_from(wire.word(0, cell).unwrap() & 63).unwrap();
        let supplied = wire.word(4, cell).unwrap();
        coverage.kinds[kind] = true;
        coverage.cells += 1;
        match kind {
            1 | 2 | 6 | 7 | 15..=36 => {
                assert!((1..=3).contains(&supplied));
                coverage.typed += 1;
                coverage.types[usize::try_from(supplied).unwrap()] = true;
                // Every other scalar type is plausible framing, but neither
                // a function, local nor expression fact may be substituted.
                for value in 1..=3 {
                    if value != supplied {
                        reject_changed_fact(fixture, owner, &baseline, cell, value);
                        coverage.mutations += 1;
                    }
                }
            }
            5 => {
                assert!((0..=15).contains(&supplied));
                coverage.flows += 1;
                coverage.flow_masks[usize::try_from(supplied).unwrap()] = true;
                // Exercise all sixteen complete outcome masks, including
                // changes that preserve fallthrough but flip exit outcomes.
                for value in 0..=15 {
                    if value != supplied {
                        reject_changed_fact(fixture, owner, &baseline, cell, value);
                        coverage.mutations += 1;
                        coverage.flow_masks[usize::try_from(value).unwrap()] = true;
                    }
                }
            }
            3 | 4 | 8..=14 => {
                assert_eq!(supplied, 0);
                coverage.unused += 1;
                reject_changed_fact(fixture, owner, &baseline, cell, 1);
                coverage.mutations += 1;
            }
            _ => panic!("unexpected scalar row kind {kind}"),
        }
        // Wire words are signed and planar. Every active role also rejects
        // high-plane/sign changes; comparing only a low byte cannot pass.
        for value in [256, -1, i32::MIN, i32::MAX] {
            reject_changed_fact(fixture, owner, &baseline, cell, value);
            coverage.mutations += 1;
        }
    }
    println!(
        "HIR_IMPORT_VERIFY_FACTS {} rows={} mutations={} reserves={}",
        fixture.name,
        fixture.rows,
        coverage.mutations - before,
        fixture.reserves
    );
}

#[test]
fn checked_hir_import_verify_every_active_scalar_fact_rejects_before_lower() {
    let mut coverage = Coverage::default();
    for &fixture in ORIGINALS {
        let mut sources = SourceMap::new();
        let id = sources.add(format!("verify-{}.ox", fixture.name), fixture.source.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        exercise_fixture(fixture, owner, &mut coverage);
    }
    assert_eq!(coverage.cells, 134);
    assert_eq!(
        (coverage.typed, coverage.flows, coverage.unused),
        (82, 14, 38)
    );
    assert_eq!(coverage.mutations, 948);
    assert!(coverage.kinds[1..=36].iter().all(|&seen| seen));
    assert!(coverage.types[1..=3].iter().all(|&seen| seen));
    assert!(coverage.flow_masks.iter().all(|&seen| seen));
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "genuine ProjectSources admission is Linux-only; no original-owner substitution qualifies public syntax"
)]
fn checked_hir_import_verify_public_project_facts_reject_before_lower() {
    let fixture = fixture!("public", 29, 16);
    let project = super::tests::project(fixture.source);
    assert_eq!(
        project.syntax_flavor(),
        crate::frontend::project::SyntaxFlavor::ProjectSyntax
    );
    let owner = SourceOwner::project(&project);
    assert_eq!(owner.count(), 1);
    let mut coverage = Coverage::default();
    exercise_fixture(fixture, owner, &mut coverage);
    assert_eq!(coverage.cells, 29);
    assert_eq!(
        (coverage.typed, coverage.flows, coverage.unused),
        (15, 5, 9)
    );
    assert_eq!(coverage.mutations, 230);
}
