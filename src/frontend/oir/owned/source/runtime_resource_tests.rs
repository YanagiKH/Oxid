//! Registered below execute::tests so private runtime layout stays private.
use super::super::*;
use crate::frontend::oir::owned::source::resource_fixtures as source;

fn resource_failure(
    case: &source::CheckedSource,
    limits: Limits,
    name: &'static str,
    origin: Span,
) {
    let error = run_limits(&case.witness, Some(case.entry), limits).unwrap_err();
    resource_error(case, error, name, origin);
}

fn resource_error(
    case: &source::CheckedSource,
    error: OwnedRunFailure,
    name: &'static str,
    origin: Span,
) {
    assert_eq!(
        error,
        OwnedRunFailure::Resource(plan::AdmissionFailure::new(name, Some(origin)))
    );
    let diagnostic = error.diagnostic(&case.sources);
    assert_eq!(
        (diagnostic.code, diagnostic.stage),
        ("E0605", "oir-owned-run")
    );
    assert_eq!(
        diagnostic.message,
        format!("owned execution resource limit: {name}")
    );
    assert_eq!(diagnostic.primary, Some(origin));
}

fn recursive_census(case: &source::CheckedSource, padding: usize, main_padding: usize) {
    let plan = ExecutionPlan::build(&case.witness).unwrap();
    assert_eq!(
        plan.function(hir::DefId(0)).usage(),
        plan::FrameUsage {
            scalar_slots: 9 + padding,
            arguments: 2,
            references: 1,
            loans: 1,
            calls: 1,
            expanded_cells: 11 + padding + 10 + 14 + 2,
            reference_bytes: (11 + padding) * size_of::<Option<Scalar>>() + 80 + 112 + 16,
            native_bytes: (11 + padding) * 8 + 16,
            ..Default::default()
        }
    );
    assert_eq!(
        plan.function(case.entry).usage(),
        plan::FrameUsage {
            scalar_slots: 3 + main_padding,
            arguments: 2,
            owners: 2,
            owner_cells: 2,
            payload_bytes: 8,
            loans: 1,
            calls: 1,
            expanded_cells: 5 + main_padding + 2 + 2 * 4 + 14 + 2,
            reference_bytes: (5 + main_padding) * size_of::<Option<Scalar>>() + 8 + 64 + 112 + 16,
            native_bytes: (5 + main_padding) * 8 + 16,
            ..Default::default()
        }
    );
}

#[test]
fn source_shared_reborrow_reaches_1024_frames_and_rejects_attempt_1025() {
    for extra in [false, true] {
        let case = source::checked(&source::recursive(1022 + usize::from(extra), 0, 0));
        recursive_census(&case, 0, 0);
        let mut events = Vec::new();
        let result = run_observed(&case.witness, case.entry, Limits::default(), &mut events);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Enter(..)))
                .count(),
            1024
        );
        if extra {
            let origin = case.span("recur(&*p,n-1)");
            let error = result.unwrap_err();
            assert_eq!(error, OwnedRunFailure::Scalar(RunFailure::Frames(origin)));
            let diagnostic = error.diagnostic(&case.sources);
            assert_eq!((diagnostic.code, diagnostic.stage), ("E0602", "oir-run"));
            assert_eq!(diagnostic.message, "live call-frame limit exceeded");
            assert_eq!(diagnostic.primary, Some(origin));
        } else {
            assert_eq!(result, Ok(Scalar::I32(7)));
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, Event::Return(..)))
                    .count(),
                1024
            );
            assert_eq!(
                events
                    .iter()
                    .filter_map(|e| if let Event::Charge(_, n) = e {
                        Some(n)
                    } else {
                        None
                    })
                    .sum::<usize>(),
                51_195
            );
        }
        // The identical source-derived witness also reaches native whole-graph
        // admission, whose recursion rejection precedes native execution.
        let diagnostic =
            native::native_module(&case.witness, Some(case.entry), &case.sources).unwrap_err();
        assert_eq!(
            (diagnostic.code, diagnostic.stage),
            ("E0700", "native-admission")
        );
        assert_eq!(diagnostic.message, "native preview does not support recursive call graphs, including unused functions and unchosen branches");
        assert_eq!(diagnostic.primary, Some(case.name("recur")));
    }
}

#[test]
fn source_actual_200000_live_cells_are_inclusive_and_200001_is_denied() {
    // Successor physical-view boundary: main X31+169=200, plus999
    // activations of recur X37+163=200. S172+999*172=172000; frames1000.
    // Each padding literal adds one activation fuel cell and one scalar step.
    // The unchanged unpadded template costs50*999+45=49995, so the
    // smaller source costs49995+2*169+999*2*163=376007. No earlier gate masks X.
    let exact = source::checked(&source::recursive(998, 163, 169));
    recursive_census(&exact, 163, 169);
    let mut events = Vec::new();
    assert_eq!(
        run_observed(&exact.witness, exact.entry, Limits::default(), &mut events),
        Ok(Scalar::I32(7))
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Enter(..)))
            .count(),
        1000
    );
    assert_eq!(
        events
            .iter()
            .filter_map(|e| if let Event::Charge(_, n) = e {
                Some(n)
            } else {
                None
            })
            .sum::<usize>(),
        49_995 + 2 * 169 + 999 * 2 * 163
    );
    let over = source::checked(&source::recursive(998, 163, 170));
    recursive_census(&over, 163, 170);
    let mut events = Vec::new();
    let error =
        run_observed(&over.witness, over.entry, Limits::default(), &mut events).unwrap_err();
    // The1000th activation would raise live X to200001, so it must never
    // install; the recursive leaf has not returned either.
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Enter(..)))
            .count(),
        999
    );
    assert!(!events.iter().any(|e| matches!(e, Event::Return(..))));
    resource_error(
        &over,
        error,
        "live expanded cells",
        over.span("recur(&*p,n-1)"),
    );
}

#[test]
fn source_owner_classes_have_exact_lowered_live_cell_and_byte_seams() {
    let case = source::checked(source::OWNER_CLASSES);
    source::assert_owner_classes(&ExecutionPlan::build(&case.witness).unwrap());
    // Physical-view census: mainX42/readX11/relayX10, D320/88/72.
    // The read path now maximizes cells53 and active bytes408; owners and
    // all call argument snapshots remain allocated in each frame.
    let bytes = 2 * size_of::<Frame>() + size_of::<Scalar>() + 320 + 88;
    assert_eq!(bytes, 960);
    let limits = Limits {
        frames: 2,
        cells: 53,
        bytes,
        ..Limits::default()
    };
    assert_eq!(
        run_limits(&case.witness, Some(case.entry), limits),
        Ok(Scalar::I32(7))
    );
    let origin = case.span("read(&x)");
    resource_failure(
        &case,
        Limits {
            cells: 52,
            ..limits
        },
        "live expanded cells",
        origin,
    );
    resource_failure(
        &case,
        Limits {
            bytes: bytes - 1,
            ..limits
        },
        "live requested bytes",
        origin,
    );
}

#[test]
fn source_frozen_batch_has_exact_lowered_frame_cell_and_byte_seams() {
    // Exact checked-in RFC source, SHA256 dbbadef9a035e3af62aab6ae2ca0ac19531684e4ad8ef25bc4d81a4fa7c07db2.
    // Preserve the frozen source and fuel schedule, while deriving successor
    // physical usage from its source census with references10/80, loans14/112.
    // main S26 A6 P32 O8 L3 C5 => X148, D1056; dispatch S4 A2 R1 L1
    // C1 => X32, D256; commit S10 R1 => X20, D160. Peak is200/1472.
    let case = source::checked(source::BATCH);
    let bytes = 3 * size_of::<Frame>() + size_of::<Scalar>() + 1056 + 256 + 160;
    assert_eq!(bytes, 2296);
    let limits = Limits {
        frames: 3,
        cells: 200,
        bytes,
        ..Limits::default()
    };
    assert_eq!(
        run_limits(&case.witness, Some(case.entry), limits),
        Ok(Scalar::I32(816))
    );
    let origin = case.span("commit(&mut *state, job)");
    let error = run_limits(
        &case.witness,
        Some(case.entry),
        Limits {
            frames: 2,
            ..limits
        },
    )
    .unwrap_err();
    assert_eq!(error, OwnedRunFailure::Scalar(RunFailure::Frames(origin)));
    let diagnostic = error.diagnostic(&case.sources);
    assert_eq!(
        (diagnostic.code, diagnostic.stage, diagnostic.primary),
        ("E0602", "oir-run", Some(origin))
    );
    resource_failure(
        &case,
        Limits {
            cells: 199,
            ..limits
        },
        "live expanded cells",
        origin,
    );
    resource_failure(
        &case,
        Limits {
            bytes: bytes - 1,
            ..limits
        },
        "live requested bytes",
        origin,
    );
}

#[test]
fn production_requested_byte_cap_is_masked_by_the_expanded_cell_cap() {
    // Record payload incl. alignment is <=4 bytes per owner cell. Scalar
    // snapshots are8bytes/cell and runtime metadata is exactly8bytes/cell:
    // owners32/4, references80/10, loans112/14, calls16/2. Thus Dref<=8*X.
    // The production byte cap cannot be independently reached under X200000;
    // the preceding tests deliberately establish only lowered byte seams.
    assert_eq!(size_of::<Option<Scalar>>(), 8);
    assert_eq!(
        (
            size_of::<OwnerRuntime>(),
            size_of::<ReferenceHandle>(),
            size_of::<LoanRuntime>(),
            size_of::<CallRuntime>()
        ),
        (32, 80, 112, 16)
    );
    let upper =
        plan::MAX_FRAMES * size_of::<Frame>() + size_of::<Scalar>() + 8 * plan::MAX_EXPANDED_CELLS;
    assert_eq!(upper, 1_878_536);
    assert!(upper < plan::MAX_REFERENCE_BYTES);
}
