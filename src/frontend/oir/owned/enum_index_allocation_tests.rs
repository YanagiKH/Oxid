//! Independent C2a index lifetimes through the existing real heap observer.
//! Source fixtures and instrumentation are outside every measured interval.
//! No executable/source-admission witness is constructed by these controls.
use super::source::reviewer_source::{integration_enabled, integration_measured};
use crate::frontend::{
    declaration_index::{
        collect_enum_candidate, DeclarationIndex, IndexLimits, SourceOwner, WorkMeter,
    },
    diagnostic::Diagnostic,
    project::{budget::Allocator, ProjectLimits, ProjectSources},
};
use std::{
    fs,
    mem::size_of,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const REQUESTS: usize = 16;
const RETAINED_ROWS: usize = 12;
const MIXED: &[(&str, &str)] = &[
    (
        "main.ox",
        "fn pre()->i32{return 0;} enum RootE{Z,U(())} pub struct RootR{pub x:i32} pub mod branch; pub enum RootTail{B(bool)} use crate::branch::Pair as Imported; use crate::branch::ChildR as ImportedR; fn main()->i32{return 0;}",
    ),
    (
        "branch.ox",
        "pub struct ChildR{pub flag:bool} pub fn Pair()->i32{return 1;} pub enum Pair{No,Number(i32)} pub mod leaf; pub enum Last{Done} pub fn child_after()->(){return;}",
    ),
    (
        "branch/leaf.ox",
        "pub enum LeafE{L} pub struct LeafR{pub unit:()} pub fn leaf_fn()->(){return;}",
    ),
];
const ENUM_FREE: &[(&str, &str)] = &[("main.ox", "fn main()->(){return;}")];
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-c2a-lifetimes-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        for (name, text) in files {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        Self(root)
    }

    fn load(&self) -> ProjectSources {
        ProjectSources::load_enum_index_candidate(
            self.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
            &mut Allocator::default(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[derive(Clone, Copy, Debug)]
struct Row {
    name: &'static str,
    length: usize,
    width: usize,
    capacity: usize,
}
impl Row {
    fn requested(self) -> usize {
        self.length.checked_mul(self.width).unwrap()
    }

    fn heap(self) -> usize {
        self.capacity.checked_mul(self.width).unwrap()
    }
}

/// Independent standard-library probes use literal repr(C) row widths. No
/// producer count, plan, reserve trace, or private table supplies the oracle.
/// New/changed enum, variant, and module carriers have exact reservation;
/// legacy carriers retain their existing non-exact capacity convention.
/// The probes run before tracking and retain no heap of their own.
fn row<const WORDS: usize>(name: &'static str, length: usize, exact: bool) -> Row {
    let mut values = Vec::<[u32; WORDS]>::new();
    if exact {
        values.try_reserve_exact(length).unwrap();
    } else {
        values.try_reserve(length).unwrap();
    }
    Row {
        name,
        length,
        width: size_of::<[u32; WORDS]>(),
        capacity: values.capacity(),
    }
}

fn inventory(lengths: [usize; REQUESTS]) -> [Row; REQUESTS] {
    [
        row::<9>("index originals", lengths[0], false),
        row::<1>("index original order", lengths[1], false),
        row::<3>("index functions", lengths[2], false),
        row::<7>("index records", lengths[3], false),
        row::<4>("index fields", lengths[4], false),
        row::<5>("index enums", lengths[5], true),
        row::<3>("index variants", lengths[6], true),
        row::<17>("index modules", lengths[7], true),
        row::<1>("index children", lengths[8], false),
        row::<5>("index imports", lengths[9], false),
        row::<4>("index aliases", lengths[10], false),
        row::<1>("index alias order", lengths[11], false),
        row::<1>("index merge workspace", lengths[12], false),
        row::<1>("index target order", lengths[13], false),
        row::<2>("index target seen", lengths[14], false),
        row::<1>("index child cursors", lengths[15], false),
    ]
}

fn mixed_inventory() -> [Row; REQUESTS] {
    // Source order: 5 functions, 3 records/fields, 5 enums/7 variants,
    // 3 modules, 2 imports; 15 originals and 2 child links.
    inventory([15, 15, 5, 3, 3, 5, 7, 3, 2, 2, 2, 2, 15, 2, 2, 3])
}

fn enum_free_inventory() -> [Row; REQUESTS] {
    inventory([1, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 1])
}

fn heap(rows: &[Row]) -> usize {
    rows.iter().map(|row| row.heap()).sum()
}

fn real_allocations(rows: &[Row]) -> usize {
    rows.iter().filter(|row| row.capacity != 0).count()
}

fn prepared_allocator(fail_at: Option<usize>, rows: usize) -> Allocator {
    let mut allocator = Allocator {
        fail_at,
        ..Allocator::default()
    };
    allocator.observer_trace_bound(rows).unwrap();
    allocator
}

fn complete_trace(allocator: &Allocator, capacity: usize, expected: &[Row]) {
    assert!(!allocator.observer_trace_overflow);
    assert_eq!(allocator.trace.capacity(), capacity);
    assert_eq!(allocator.attempts, expected.len());
    assert_eq!(allocator.trace.len(), expected.len());
    for (position, (event, row)) in allocator.trace.iter().zip(expected).enumerate() {
        assert_eq!(
            (event.kind, event.length, event.element_bytes, event.success),
            (
                row.name,
                row.length,
                row.width,
                allocator.fail_at != Some(position + 1)
            )
        );
    }
}

fn quiet_work(work: &WorkMeter) {
    assert!(!work.observing());
    assert!(work.events.borrow().is_empty());
    assert!(work.observations.borrow().is_empty());
}

fn sources_survive(project: &ProjectSources) {
    for module in project.modules() {
        let file = project.sources().get(module.file);
        let ast = project.try_file_ast(module.file).unwrap();
        assert!(ast.belongs_to(file));
        assert!(ast.validate_spans_and_ids(|span| file.try_text(span).is_some()));
    }
}

fn payload<T>(values: &Vec<T>) -> usize {
    values.capacity().checked_mul(size_of::<T>()).unwrap()
}

fn diagnostic_payload(error: &Diagnostic) -> usize {
    error.message.capacity()
        + payload(&error.secondary)
        + error
            .secondary
            .iter()
            .map(|(_, text)| text.capacity())
            .sum::<usize>()
        + payload(&error.notes)
        + error.notes.iter().map(String::capacity).sum::<usize>()
}

#[test]
fn enum_index_lifecycle_collection_freeze_and_drop_match_independent_row_capacities() {
    for (name, files, rows, requested_retained, requested_scratch) in [
        ("mixed", MIXED, mixed_inventory(), 1268, 96),
        ("enum-free", ENUM_FREE, enum_free_inventory(), 120, 8),
    ] {
        let fixture = Fixture::new(files);
        let project = fixture.load();
        let retained = heap(&rows[..RETAINED_ROWS]);
        let scratch = heap(&rows[RETAINED_ROWS..]);
        assert_eq!(
            rows[..RETAINED_ROWS]
                .iter()
                .map(|r| r.requested())
                .sum::<usize>(),
            requested_retained
        );
        assert_eq!(
            rows[RETAINED_ROWS..]
                .iter()
                .map(|r| r.requested())
                .sum::<usize>(),
            requested_scratch
        );
        let mut allocator = prepared_allocator(None, REQUESTS);
        let trace_capacity = allocator.trace.capacity();
        let work = WorkMeter::default();
        let (result, (calls, live, peak)) = integration_measured(|| {
            collect_enum_candidate(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &work,
                &mut allocator,
            )
        });
        let facts = result.unwrap();
        complete_trace(&allocator, trace_capacity, &rows);
        assert_eq!(calls, real_allocations(&rows));
        assert_eq!(live, isize::try_from(retained + scratch).unwrap());
        assert_eq!(peak, live);
        // This plan charges row lengths and the enclosing stack header. Actual
        // legacy Vec capacity rounding is deliberately a separate observation.
        assert_eq!(
            facts.plan().retained,
            (size_of::<DeclarationIndex<'_>>() + requested_retained) as u64
        );
        assert_eq!(facts.enum_count(), rows[5].length);
        quiet_work(&work);

        let (result, (freeze_calls, freeze_live, freeze_peak)) =
            integration_measured(|| facts.finish(&work, &mut allocator));
        let index = result.unwrap();
        assert_eq!(
            (freeze_calls, freeze_live, freeze_peak),
            (0, -isize::try_from(scratch).unwrap(), 0)
        );
        assert_eq!(live + freeze_live, isize::try_from(retained).unwrap());
        complete_trace(&allocator, trace_capacity, &rows);
        assert_eq!(index.enum_count(), rows[5].length);
        assert_eq!(index.function_count(), rows[2].length);
        assert_eq!(index.record_count(), rows[3].length);
        assert_eq!(index.enum_variant_counts().sum::<usize>(), rows[6].length);
        quiet_work(&work);

        let ((), (drop_calls, drop_live, drop_peak)) = integration_measured(|| drop(index));
        assert_eq!(
            (drop_calls, drop_live, drop_peak),
            (0, -isize::try_from(retained).unwrap(), 0)
        );
        assert_eq!(live + freeze_live + drop_live, 0);
        sources_survive(&project);
        assert!(!integration_enabled());
        println!(
            "enum index {name}: seam requests={}, real allocations={calls}, requested row bytes={requested_retained}+{requested_scratch}, actual requested capacities={retained}+{scratch}, collection peak={peak}, freeze delta={freeze_live}, drop delta={drop_live}",
            allocator.attempts
        );

        // A fresh complete ownership interval independently establishes final
        // live zero, without adding signed snapshots from separate intervals.
        let mut allocator = prepared_allocator(None, REQUESTS);
        let trace_capacity = allocator.trace.capacity();
        let work = WorkMeter::default();
        let ((), (calls, live, peak)) = integration_measured(|| {
            let facts = collect_enum_candidate(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &work,
                &mut allocator,
            )
            .unwrap();
            drop(facts.finish(&work, &mut allocator).unwrap());
        });
        complete_trace(&allocator, trace_capacity, &rows);
        assert_eq!(calls, real_allocations(&rows));
        assert_eq!(live, 0);
        assert_eq!(peak, isize::try_from(retained + scratch).unwrap());
        quiet_work(&work);
    }
}

#[test]
fn enum_index_lifecycle_all_reserve_failures_leave_only_diagnostic_then_zero() {
    for (name, files, rows) in [
        ("mixed", MIXED, mixed_inventory()),
        ("enum-free", ENUM_FREE, enum_free_inventory()),
    ] {
        let fixture = Fixture::new(files);
        let project = fixture.load();
        let origin = SourceOwner::project(&project).eof();
        for fail_at in 1..=REQUESTS {
            let mut allocator = prepared_allocator(Some(fail_at), REQUESTS);
            let trace_capacity = allocator.trace.capacity();
            let work = WorkMeter::default();
            let (result, (calls, live, peak)) = integration_measured(|| {
                collect_enum_candidate(
                    SourceOwner::project(&project),
                    IndexLimits::default(),
                    &work,
                    &mut allocator,
                )
            });
            let error = result.unwrap_err();
            assert_eq!((error.code, error.stage), ("E0400", "resolve-project"));
            assert_eq!(error.message, "declaration index allocation failed");
            assert_eq!(error.primary, Some(origin));
            assert!(error.secondary.is_empty() && error.notes.is_empty());
            let diagnostic_heap = size_of::<Diagnostic>() + diagnostic_payload(&error);
            assert_eq!(
                live,
                isize::try_from(diagnostic_heap).unwrap(),
                "{name} request {fail_at}: partial rows survived"
            );
            let prefix_calls = real_allocations(&rows[..fail_at - 1]);
            assert!(
                calls > prefix_calls,
                "diagnostic allocations are real but not index requests"
            );
            complete_trace(&allocator, trace_capacity, &rows[..fail_at]);
            quiet_work(&work);
            let ((), (drop_calls, drop_live, drop_peak)) = integration_measured(|| drop(error));
            assert_eq!((drop_calls, drop_live, drop_peak), (0, -live, 0));
            assert_eq!(live + drop_live, 0);

            // Capacity-overflow injection is a real fallible Vec reserve error,
            // not a claim about recovering a process-wide OOM. A zero-row seam
            // request remains an ordinal even though its successful form would
            // make no global allocation. Error strings/boxes are dropped here.
            let mut allocator = prepared_allocator(Some(fail_at), REQUESTS);
            let trace_capacity = allocator.trace.capacity();
            let work = WorkMeter::default();
            let ((), (_, final_live, _)) = integration_measured(|| {
                drop(
                    collect_enum_candidate(
                        SourceOwner::project(&project),
                        IndexLimits::default(),
                        &work,
                        &mut allocator,
                    )
                    .unwrap_err(),
                );
            });
            assert_eq!(final_live, 0, "{name} request {fail_at}");
            complete_trace(&allocator, trace_capacity, &rows[..fail_at]);
            quiet_work(&work);
            sources_survive(&project);
            assert!(!integration_enabled());
            println!(
                "enum index {name} failure {fail_at} ({}): successful real index allocations={prefix_calls}, total real calls={calls}, diagnostic-only live={live}, peak including diagnostic={peak}, after drop=0",
                rows[fail_at - 1].name
            );
        }
    }
}

#[test]
fn enum_index_lifecycle_runtime_finalization_errors_drop_all_index_owners() {
    let duplicate = "enum E{AA,AB,AA}fn main()->(){return;}";
    let paired =
        "mod m;use crate::m::Pair as bool;use crate::m::Pair as Good;fn main()->(){return;}";
    type FailureCase<'a> = (
        &'a str,
        &'a [(&'a str, &'a str)],
        [usize; REQUESTS],
        &'a str,
        usize,
    );
    let cases: [FailureCase<'_>; 2] = [
        (
            "duplicate variants",
            &[("main.ox", duplicate)],
            [2, 2, 1, 0, 0, 1, 3, 1, 0, 0, 0, 0, 2, 0, 0, 1],
            "E0201",
            1,
        ),
        (
            "paired alias builtin collision",
            &[
                ("main.ox", paired),
                ("m.ox", "pub enum Pair{V}pub fn Pair()->i32{return 1;}"),
            ],
            [4, 4, 2, 0, 0, 1, 1, 2, 1, 2, 2, 2, 4, 2, 2, 2],
            "E0202",
            0,
        ),
    ];
    for (name, files, lengths, code, labels) in cases {
        let rows = inventory(lengths);
        let fixture = Fixture::new(files);
        let project = fixture.load();
        let expected_start = if code == "E0201" {
            duplicate.rfind("AA").unwrap()
        } else {
            paired.find("bool").unwrap()
        };
        let mut allocator = prepared_allocator(None, REQUESTS);
        let trace_capacity = allocator.trace.capacity();
        let work = WorkMeter::default();
        let (errors, (calls, live, peak)) = integration_measured(|| {
            collect_enum_candidate(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &work,
                &mut allocator,
            )
            .unwrap()
            .finish(&work, &mut allocator)
            .unwrap_err()
        });
        complete_trace(&allocator, trace_capacity, &rows);
        assert_eq!(errors.len(), 1);
        let error = &errors[0];
        assert_eq!((error.code, error.stage), (code, "resolve"));
        let primary = error.primary.unwrap();
        assert_eq!((primary.file.0, primary.start), (0, expected_start));
        assert_eq!(
            project.try_text(primary),
            Some(if code == "E0201" { "AA" } else { "bool" })
        );
        assert_eq!(error.secondary.len(), labels);
        if code == "E0201" {
            assert_eq!(error.secondary[0].0.start, duplicate.find("AA").unwrap());
            assert_eq!(project.try_text(error.secondary[0].0), Some("AA"));
        }
        let diagnostics_heap =
            payload(&errors) + errors.iter().map(diagnostic_payload).sum::<usize>();
        assert_eq!(
            live,
            isize::try_from(diagnostics_heap).unwrap(),
            "{name}: index or scratch survived finish failure"
        );
        assert!(calls > real_allocations(&rows));
        quiet_work(&work);
        let ((), (drop_calls, drop_live, drop_peak)) = integration_measured(|| drop(errors));
        assert_eq!((drop_calls, drop_live, drop_peak), (0, -live, 0));
        assert_eq!(live + drop_live, 0);
        sources_survive(&project);

        let mut allocator = prepared_allocator(None, REQUESTS);
        let trace_capacity = allocator.trace.capacity();
        let work = WorkMeter::default();
        let ((), (_, final_live, _)) = integration_measured(|| {
            let facts = collect_enum_candidate(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &work,
                &mut allocator,
            )
            .unwrap();
            drop(facts.finish(&work, &mut allocator).unwrap_err());
        });
        assert_eq!(final_live, 0);
        complete_trace(&allocator, trace_capacity, &rows);
        quiet_work(&work);
        assert!(!integration_enabled());
        println!("enum index {name}: real calls={calls}, diagnostic-only live={live}, peak including diagnostic={peak}, after drop=0");
    }
}

#[test]
fn enum_index_lifecycle_trace_exhaustion_is_not_production_allocation_failure() {
    let fixture = Fixture::new(MIXED);
    let project = fixture.load();
    let rows = mixed_inventory();
    for trace_rows in [0, REQUESTS - 1, REQUESTS] {
        let mut allocator = prepared_allocator(None, trace_rows);
        let trace_capacity = allocator.trace.capacity();
        let work = WorkMeter::default();
        let ((), (calls, live, peak)) = integration_measured(|| {
            let facts = collect_enum_candidate(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &work,
                &mut allocator,
            )
            .unwrap();
            drop(facts.finish(&work, &mut allocator).unwrap());
        });
        assert_eq!(allocator.attempts, REQUESTS);
        assert_eq!(allocator.trace.capacity(), trace_capacity);
        assert_eq!(allocator.trace.len(), trace_rows);
        assert_eq!(allocator.observer_trace_overflow, trace_rows < REQUESTS);
        assert!(allocator.trace.iter().all(|event| event.success));
        assert_eq!(calls, real_allocations(&rows));
        assert_eq!(live, 0);
        assert_eq!(peak, isize::try_from(heap(&rows)).unwrap());
        quiet_work(&work);
        assert!(!integration_enabled());
    }
}

#[test]
fn enum_index_lifecycle_two_file_loader_success_and_every_reserve_failure_drop() {
    const TRACE_ROWS: usize = 512;
    let fixture = Fixture::new(&[("main.ox", "mod a;enum E{V}"), ("a.ox", "enum F{W}")]);
    let entry = fixture.0.join("main.ox");
    let entry = entry.to_str().unwrap();
    let mut baseline = prepared_allocator(None, TRACE_ROWS);
    let baseline_capacity = baseline.trace.capacity();
    let (result, (calls, live, peak)) = integration_measured(|| {
        ProjectSources::load_enum_index_candidate(entry, ProjectLimits::default(), &mut baseline)
    });
    let project = result.unwrap();
    assert!(live > 0 && peak >= live);
    assert_eq!(project.modules().len(), 2);
    assert_eq!(project.enum_handles().count(), 2);
    sources_survive(&project);
    assert!(!baseline.observer_trace_overflow);
    assert_eq!(baseline.trace.capacity(), baseline_capacity);
    assert_eq!(baseline.trace.len(), baseline.attempts);
    assert!(baseline.trace.iter().all(|event| event.success));
    for label in [
        "source bytes",
        "file ASTs",
        "module headers",
        "enum declarations",
        "enum declaration variants",
    ] {
        assert!(
            baseline.trace.iter().any(|event| event.kind == label),
            "{label}"
        );
    }
    let ((), (drop_calls, drop_live, drop_peak)) = integration_measured(|| drop(project));
    assert_eq!((drop_calls, drop_live, drop_peak), (0, -live, 0));
    assert_eq!(live + drop_live, 0);
    println!(
        "enum candidate loader success: seam requests={}, real calls={calls}, retained requested heap={live}, logical peak={peak}, drop delta={drop_live}",
        baseline.attempts
    );

    let mut allocator = prepared_allocator(None, TRACE_ROWS);
    let trace_capacity = allocator.trace.capacity();
    let ((), (_, final_live, _)) = integration_measured(|| {
        drop(
            ProjectSources::load_enum_index_candidate(
                entry,
                ProjectLimits::default(),
                &mut allocator,
            )
            .unwrap(),
        );
    });
    assert_eq!(final_live, 0);
    assert_eq!(allocator.attempts, baseline.attempts);
    assert_eq!(allocator.trace.capacity(), trace_capacity);
    assert!(!allocator.observer_trace_overflow);

    let mut maximum_failure_peak = 0;
    for fail_at in 1..=baseline.attempts {
        let mut allocator = prepared_allocator(Some(fail_at), TRACE_ROWS);
        let trace_capacity = allocator.trace.capacity();
        let (receipt, (failure_calls, final_live, failure_peak)) = integration_measured(|| {
            let mut failure = ProjectSources::load_enum_index_candidate(
                entry,
                ProjectLimits::default(),
                &mut allocator,
            )
            .unwrap_err();
            let receipt = (
                failure.diagnostics[0].code,
                failure.diagnostics[0].stage,
                failure.diagnostics.iter().all(|error| {
                    error
                        .primary
                        .is_none_or(|span| failure.sources.try_text(span).is_some())
                        && error
                            .secondary
                            .iter()
                            .all(|(span, _)| failure.sources.try_text(*span).is_some())
                }),
            );
            // LoadFailure takes the caller's Allocator on error. Move that
            // existing observer trace back out before dropping failure-owned
            // source/diagnostic storage, so its premeasurement allocation does
            // not produce a negative baseline and disguise a leak.
            std::mem::swap(&mut allocator, &mut failure.allocator);
            drop(failure);
            receipt
        });
        assert_eq!(receipt.0, "E0400");
        assert!(
            matches!(receipt.1, "source-project" | "parse"),
            "{}",
            receipt.1
        );
        assert!(receipt.2);
        assert_eq!(final_live, 0, "loader failure {fail_at}");
        assert!(failure_calls > 0);
        assert!(!allocator.observer_trace_overflow);
        assert_eq!(allocator.trace.capacity(), trace_capacity);
        assert_eq!(allocator.trace.len(), allocator.attempts);
        // Candidate parsing stops at resource failure, retaining exactly the
        // stable prefix through the injected failed reservation.
        assert_eq!(allocator.attempts, fail_at);
        for (position, (actual, expected)) in allocator.trace[..fail_at]
            .iter()
            .zip(&baseline.trace[..fail_at])
            .enumerate()
        {
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
                    position + 1 != fail_at
                ),
                "loader failure {fail_at}"
            );
        }
        assert_eq!(
            allocator
                .trace
                .iter()
                .filter(|event| !event.success)
                .count(),
            1
        );
        maximum_failure_peak = maximum_failure_peak.max(failure_peak);
        assert!(!integration_enabled());
    }
    println!(
        "enum candidate loader: swept {} observed reserve ordinals; failure-owned source/AST/diagnostic payload drops to zero with caller trace preserved; maximum failure logical peak={maximum_failure_peak}",
        baseline.attempts
    );
}
