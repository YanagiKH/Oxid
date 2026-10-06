//! Producer resource checks, frozen from proposal v3 before candidate execution.
//! Fixture vectors and SourceMaps belong to the caller, not the native ledger.
//! No native executable or producer fuel schedule is used by these tests.
use super::super::*;
use super::{append_cycle, block, fixtures};
use crate::frontend::source::SourceFileId;

const ENTRY: hir::DefId = hir::DefId(0);

fn span(file: SourceFileId, start: usize, end: usize) -> Span {
    Span { file, start, end }
}

fn scalar_raw(spans: &[Span]) -> RawOwnedProgram {
    assert!(!spans.is_empty());
    let origin = spans[0];
    let mut function = fixtures::function(0, ValueTy::Scalar(hir::Ty::I32), origin);
    function.locals = (0..spans.len() + 2)
        .map(|_| fixtures::scalar(hir::Ty::I32, origin))
        .collect();
    let mut statements = vec![
        fixtures::assign(0, Rvalue::I32(10), origin),
        fixtures::assign(1, Rvalue::I32(20), origin),
    ];
    for (index, &origin) in spans.iter().enumerate() {
        statements.push(fixtures::assign(
            index + 2,
            Rvalue::CheckedI32 {
                op: hir::ArithmeticOp::Add,
                left: fixtures::operand(0, origin),
                right: fixtures::operand(1, origin),
                operator_span: origin,
            },
            origin,
        ));
    }
    function.blocks.push(block(
        statements,
        OwnedTerminatorKind::ReturnScalar(fixtures::operand(spans.len() + 1, origin)),
        origin,
    ));
    RawOwnedProgram {
        builtins: BuiltinOrigins::None,
        enums: vec![],
        records: vec![],
        functions: vec![function],
    }
}

fn ordinary(kind: FailureKind, origin: Span, sources: &SourceMap) -> String {
    match kind {
        FailureKind::Fuel => RunFailure::Fuel(origin).diagnostic(sources),
        FailureKind::Overflow => RunFailure::Overflow(origin).diagnostic(sources),
        FailureKind::DivisionByZero => RunFailure::DivisionByZero(origin).diagnostic(sources),
        FailureKind::Bounds => execute::OwnedRunFailure::Bounds(origin).diagnostic(sources),
        FailureKind::EnumTag => {
            execute::OwnedRunFailure::Invariant("enum tag", Some(origin)).diagnostic(sources)
        }
        FailureKind::EnumPayload => {
            execute::OwnedRunFailure::Invariant("enum payload", Some(origin)).diagnostic(sources)
        }
    }
    .render_human(sources)
}

fn assert_denial(error: &Diagnostic, fragment: &str) {
    assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
    assert!(error.message.contains(fragment), "{}", error.message);
}

fn assert_diagnostic_ledger(metrics: &NativeMetrics, occurrences: usize, unique: usize) {
    assert_eq!(metrics.row_sizes, [64, 40, 24, 48]);
    assert_eq!((metrics.occurrences, metrics.unique), (occurrences, unique));
    assert_eq!(metrics.inventory_rows, occurrences);
    assert_eq!(metrics.prefix_preflight_rows, unique);
    assert_eq!(metrics.coordinate_rows, unique);
    assert_eq!(metrics.occurrence_bytes, 64 * occurrences);
    assert_eq!(metrics.lookup_bytes, 40 * unique);
    assert_eq!(metrics.message_header_bytes, 24 * unique);
    assert_eq!(metrics.message_count_bytes, metrics.message_bytes);
    assert_eq!(metrics.message_render_bytes, metrics.message_bytes);
    assert_eq!(metrics.source_bytes, metrics.source_prefix_bound);
    assert!(metrics.diagnostic_transient_peak <= size_of::<Diagnostic>() + 64);
    assert!(metrics.metadata_peak <= metrics.metadata_admitted_bytes);
    assert!(metrics.metadata_admitted_bytes <= plan::MAX_PLAN_BYTES);
    let logarithm = usize::BITS as usize - occurrences.max(1).leading_zeros() as usize;
    for comparisons in metrics.sort_comparisons {
        // A deliberately loose sorting-work bound; no std sort schedule oracle.
        assert!(comparisons <= 64 * occurrences.max(1) * (logarithm + 1));
    }
}

#[test]
fn array_native_resource_real_rows_and_factory_transient_are_charged() {
    assert_eq!(size_of::<DiagnosticOccurrence>(), 64);
    assert_eq!(size_of::<DiagnosticLookup>(), 40);
    assert_eq!(size_of::<String>(), 24);
    assert_eq!(size_of::<Bound>(), 48);
    let (sources, s) = fixtures::context();
    let witness = verified::verify_owned(scalar_raw(&[s(7)]), &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let mut accounting = Accounting::default();
    let diagnostics = Diagnostics::new_accounted(
        &plan,
        ENTRY,
        &sources,
        true,
        MAX_DIAGNOSTIC_BYTES,
        plan::MAX_PLAN_BYTES,
        &mut accounting,
    )
    .unwrap();
    // One entry, three statements, one terminator and one overflow occurrence.
    assert_diagnostic_ledger(&accounting.metrics, 6, 2);
    eprintln!(
        "Unit2D measured Diagnostic size: {}; diagnostic ledger: {:?}",
        size_of::<Diagnostic>(),
        accounting.metrics
    );
    assert_eq!(accounting.metrics.plan_bytes, 184);
    assert_eq!(accounting.metrics.retained_bound_bytes, 48);
    assert_eq!(
        accounting.metrics.metadata_admitted_bytes,
        184 + 48 + 128 * 6 + size_of::<Diagnostic>() + 64
    );
    let transient = [FailureKind::Fuel, FailureKind::Overflow]
        .into_iter()
        .map(|kind| size_of::<Diagnostic>() + kind.diagnostic(s(7), &sources).message.capacity())
        .max()
        .unwrap();
    assert_eq!(accounting.metrics.diagnostic_transient_peak, transient);
    assert_eq!(
        accounting.metrics.metadata_peak,
        184 + 48 + 64 * 6 + 64 * 2 + transient
    );
    assert_eq!(
        accounting.metrics.message_bytes,
        diagnostics.messages.iter().map(String::len).sum::<usize>()
    );
}

#[test]
fn array_native_resource_prelocated_shared_formatter_matches_exact_coordinates() {
    let mut sources = SourceMap::new();
    let first = sources.add("a\"\\\n\t\u{1b}.ox".into(), "é\tA\r\n中e\u{301}\n🦀".into());
    let second = sources.add("other\r.ox".into(), "\t\r\n".into());
    // Coordinates count Unicode scalars, including CR, tabs and combining marks.
    // They are hand-written from the input, never derived from native locations.
    let cases = [
        (span(first, 0, 0), 1, 1),
        (span(first, 2, 3), 1, 2),
        (span(first, 4, 5), 1, 4),
        (span(first, 5, 6), 1, 5),
        (span(first, 6, 9), 2, 1),
        (span(first, 10, 12), 2, 3),
        (span(first, 13, 17), 3, 1),
        (span(first, 17, 17), 3, 2),
        (span(second, 1, 2), 1, 2),
        (span(second, 3, 3), 2, 1),
    ];
    for kind in [
        FailureKind::Fuel,
        FailureKind::Overflow,
        FailureKind::Bounds,
    ] {
        for (origin, line, column) in cases {
            let row = DiagnosticOccurrence {
                key: (kind, origin.file.0, origin.start, origin.end),
                encounter: 0,
                line,
                column,
                rendered_len: 0,
            };
            let mut text = String::new();
            row.write(&sources, &mut text, &mut Accounting::default())
                .unwrap();
            assert_eq!(text, ordinary(kind, origin, &sources));
            assert!(text.ends_with(&format!(":{line}:{column}\n")));
            assert!(!text.contains('\u{1b}'));
        }
    }
    assert_eq!(
        ordinary(FailureKind::Bounds, span(first, 10, 12), &sources),
        "error[E0606] (oir-owned-run): array index out of bounds\n  --> a\"\\\\n\\t\\u{1b}.ox:2:3\n"
    );
}

#[test]
fn array_native_resource_filtered_primaries_never_request_a_location() {
    let mut sources = SourceMap::new();
    let file = sources.add("utf8.ox".into(), "é".into());
    let invalid = [
        span(file, 2, 1),
        span(file, 0, 3),
        span(file, 1, 2),
        span(file, 0, 1),
        span(SourceFileId(17), 0, 0),
    ];
    for origin in invalid {
        for kind in [
            FailureKind::Fuel,
            FailureKind::Overflow,
            FailureKind::Bounds,
        ] {
            let row = DiagnosticOccurrence {
                key: (kind, origin.file.0, origin.start, origin.end),
                encounter: 0,
                line: 0,
                column: 0,
                rendered_len: 0,
            };
            let mut text = String::new();
            row.write(&sources, &mut text, &mut Accounting::default())
                .unwrap();
            assert_eq!(text, ordinary(kind, origin, &sources));
            assert!(!text.contains("-->"));
            let diagnostic = kind.diagnostic(origin, &sources);
            let mut calls = 0;
            diagnostic
                .write_human_with_locations(&mut String::new(), |_| {
                    calls += 1;
                    Err(std::fmt::Error)
                })
                .unwrap();
            assert_eq!(calls, 0);
        }
    }
    let inconsistent = DiagnosticOccurrence {
        key: (FailureKind::Bounds, file.0, 0, 2),
        encounter: 0,
        line: 0,
        column: 0,
        rendered_len: 0,
    };
    assert!(inconsistent
        .write(&sources, &mut String::new(), &mut Accounting::default())
        .is_err());
}

#[test]
fn array_native_resource_full_keys_keep_first_encounter_and_end_distinctions() {
    let mut sources = SourceMap::new();
    let file = sources.add("encounters.ox".into(), "0123456789abcdef".into());
    let origins = [
        span(file, 9, 10),
        span(file, 2, 3),
        span(file, 9, 11),
        span(file, 9, 10),
    ];
    let witness = verified::verify_owned(scalar_raw(&origins), &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let mut accounting = Accounting::default();
    let diagnostics = Diagnostics::new_accounted(
        &plan,
        ENTRY,
        &sources,
        true,
        MAX_DIAGNOSTIC_BYTES,
        plan::MAX_PLAN_BYTES,
        &mut accounting,
    )
    .unwrap();
    assert_diagnostic_ledger(&accounting.metrics, 12, 6);
    for (index, origin) in origins[..3].iter().enumerate() {
        for (offset, kind) in [FailureKind::Fuel, FailureKind::Overflow]
            .into_iter()
            .enumerate()
        {
            let expected = ordinary(kind, *origin, &sources);
            assert_eq!(
                diagnostics.get(kind, *origin),
                (2 * index + offset, expected.len())
            );
            assert_eq!(diagnostics.messages[2 * index + offset], expected);
        }
    }
    assert_eq!(diagnostics.messages[0], diagnostics.messages[4]);
    assert_eq!(diagnostics.messages[1], diagnostics.messages[5]);
    assert_eq!(accounting.metrics.source_prefix_bound, 9);
    assert_eq!(accounting.metrics.source_scalars, 9);
}

#[test]
fn array_native_resource_supplied_maps_filter_without_losing_keys_or_ids() {
    let mut verification = SourceMap::new();
    let file = verification.add("verified.ox".into(), "xxxxxxxxxxxxxxxx".into());
    let origins = [span(file, 1, 2), span(file, 4, 5), span(file, 4, 6)];
    let witness = verified::verify_owned(scalar_raw(&origins), &verification).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let mut alternate = SourceMap::new();
    alternate.add("alternate\n.ox".into(), "a\né\tZxxxxxxxxxx".into());
    let mut short = SourceMap::new();
    short.add("short.ox".into(), "xx".into());
    let mut utf8 = SourceMap::new();
    utf8.add("non-boundary.ox".into(), "éééééééé".into());
    for (rendering, prefix, scalars) in [
        (alternate, 4, 3),
        (SourceMap::new(), 0, 0),
        (short, 1, 1),
        (utf8, 4, 2),
    ] {
        let mut accounting = Accounting::default();
        let diagnostics = Diagnostics::new_accounted(
            &plan,
            ENTRY,
            &rendering,
            false,
            MAX_DIAGNOSTIC_BYTES,
            plan::MAX_PLAN_BYTES,
            &mut accounting,
        )
        .unwrap();
        assert_diagnostic_ledger(&accounting.metrics, 3, 3);
        assert_eq!(
            (
                accounting.metrics.source_bytes,
                accounting.metrics.source_scalars
            ),
            (prefix, scalars)
        );
        for (id, &origin) in origins.iter().enumerate() {
            let expected = ordinary(FailureKind::Overflow, origin, &rendering);
            assert_eq!(
                diagnostics.get(FailureKind::Overflow, origin),
                (id, expected.len())
            );
            assert_eq!(diagnostics.messages[id], expected);
        }
    }
}

#[test]
fn array_native_resource_multifile_prefix_is_sum_of_surviving_maxima() {
    let mut sources = SourceMap::new();
    let a = sources.add("first.ox".into(), "é\tA\r\n中e\u{301}\n🦀".into());
    let b = sources.add("second.ox".into(), "x\r\n🦀z".into());
    let origins = [
        span(b, 7, 8),
        span(a, 13, 17),
        span(a, 2, 3),
        span(b, 3, 7),
        span(a, 13, 13),
    ];
    let witness = verified::verify_owned(scalar_raw(&origins), &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let mut accounting = Accounting::default();
    let diagnostics = Diagnostics::new_accounted(
        &plan,
        ENTRY,
        &sources,
        false,
        MAX_DIAGNOSTIC_BYTES,
        plan::MAX_PLAN_BYTES,
        &mut accounting,
    )
    .unwrap();
    assert_diagnostic_ledger(&accounting.metrics, 5, 5);
    assert_eq!(accounting.metrics.source_prefix_bound, 13 + 7);
    assert_eq!(accounting.metrics.source_scalars, 9 + 4);
    for (id, origin) in origins.into_iter().enumerate() {
        assert_eq!(
            diagnostics.messages[id],
            ordinary(FailureKind::Overflow, origin, &sources)
        );
    }
}

#[test]
fn array_native_resource_raw_prefix_above_loader_cap_is_walked_once() {
    let mut sources = SourceMap::new();
    let prefix = 1024 * 1024 + 31;
    let file = sources.add("caller-owned-large.ox".into(), "x".repeat(prefix + 256));
    for increasing in [false, true] {
        let origins: Vec<_> = (0..96)
            .map(|i| {
                if increasing {
                    span(file, prefix + i, prefix + i + 1)
                } else {
                    span(file, prefix, prefix + i + 1)
                }
            })
            .collect();
        let witness = verified::verify_owned(scalar_raw(&origins), &sources).unwrap();
        let plan = ExecutionPlan::build(&witness).unwrap();
        let mut accounting = Accounting::default();
        let diagnostics = Diagnostics::new_accounted(
            &plan,
            ENTRY,
            &sources,
            false,
            MAX_DIAGNOSTIC_BYTES,
            plan::MAX_PLAN_BYTES,
            &mut accounting,
        )
        .unwrap();
        assert_diagnostic_ledger(&accounting.metrics, 96, 96);
        let expected = prefix + if increasing { 95 } else { 0 };
        assert_eq!(accounting.metrics.source_prefix_bound, expected);
        assert_eq!(accounting.metrics.source_scalars, expected);
        assert!(accounting.metrics.source_bytes > 1024 * 1024);
        assert!(diagnostics.messages[0].ends_with(&format!(":1:{}\n", prefix + 1)));
        assert!(diagnostics.messages[95].ends_with(&format!(":1:{}\n", expected + 1)));
    }
}

#[test]
fn array_native_resource_exact_metadata_and_text_caps_precede_payload_failure() {
    let (sources, s) = fixtures::context();
    let origins = [s(9), s(2), s(9)];
    let witness = verified::verify_owned(scalar_raw(&origins), &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let text = ordinary(FailureKind::Overflow, s(9), &sources).len()
        + ordinary(FailureKind::Overflow, s(2), &sources).len();
    // Plan184 + retained Bound48 + K3*(occurrence64+lookup40+header24)
    // + the conservatively admitted simultaneous diagnostic box/message.
    let metadata = 184 + 48 + 3 * 128 + size_of::<Diagnostic>() + 64;
    let mut exact = Accounting::default();
    let diagnostics =
        Diagnostics::new_accounted(&plan, ENTRY, &sources, false, text, metadata, &mut exact)
            .unwrap();
    assert_diagnostic_ledger(&exact.metrics, 3, 2);
    assert_eq!(
        diagnostics.messages.iter().map(String::len).sum::<usize>(),
        text
    );
    assert_eq!(exact.metrics.metadata_admitted_bytes, metadata);

    let mut fail_metadata = Accounting {
        fail_after: Some(0),
        ..Accounting::default()
    };
    let error = Diagnostics::new_accounted(
        &plan,
        ENTRY,
        &sources,
        false,
        text,
        metadata - 1,
        &mut fail_metadata,
    )
    .err()
    .unwrap();
    assert_denial(&error, "diagnostic metadata bytes");
    assert_eq!(fail_metadata.metrics.allocation_attempts, 0);
    assert_eq!(fail_metadata.metrics.failed_allocation, None);

    let mut fail_text = Accounting {
        fail_after: Some(3),
        ..Accounting::default()
    };
    let error = Diagnostics::new_accounted(
        &plan,
        ENTRY,
        &sources,
        false,
        text - 1,
        metadata,
        &mut fail_text,
    )
    .err()
    .unwrap();
    assert_denial(&error, "diagnostic bytes");
    assert_eq!(fail_text.metrics.allocation_attempts, 3);
    assert_eq!(fail_text.metrics.failed_allocation, None);
    assert_eq!(fail_text.metrics.message_render_bytes, 0);
}

#[test]
fn array_native_resource_every_diagnostic_reservation_fails_then_succeeds() {
    let (sources, s) = fixtures::context();
    let witness = verified::verify_owned(scalar_raw(&[s(7), s(2), s(7)]), &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let expected = [
        "diagnostic occurrences",
        "diagnostic lookup",
        "diagnostic headers",
        "diagnostic message",
        "diagnostic message",
    ];
    for fail_after in 0..=expected.len() {
        let mut accounting = Accounting {
            fail_after: Some(fail_after),
            ..Accounting::default()
        };
        let result = Diagnostics::new_accounted(
            &plan,
            ENTRY,
            &sources,
            false,
            MAX_DIAGNOSTIC_BYTES,
            plan::MAX_PLAN_BYTES,
            &mut accounting,
        );
        if fail_after < expected.len() {
            let error = result.err().unwrap();
            assert_denial(&error, "allocation");
            assert_eq!(
                accounting.metrics.failed_allocation,
                Some(expected[fail_after])
            );
            assert_eq!(accounting.metrics.allocation_attempts, fail_after + 1);
        } else {
            let diagnostics = result.unwrap();
            assert_eq!(diagnostics.messages.len(), 2);
            assert_eq!(accounting.metrics.failed_allocation, None);
            assert_eq!(accounting.metrics.allocation_attempts, expected.len());
        }
    }
}

#[test]
fn array_native_resource_old_record_and_scalar_caps_preserve_ordinary_output() {
    for build in [
        fixtures::empty_record as super::Fixture,
        fixtures::owned_relay as super::Fixture,
        fixtures::shared_read as super::Fixture,
    ] {
        let (sources, raw, _) = build();
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let expected = native_module(&witness, Some(ENTRY), &sources).unwrap();
        let mut accounting = Accounting::default();
        let module = native_module_accounted(
            &witness,
            Some(ENTRY),
            &sources,
            plan::MAX_FUEL,
            Limits::DEFAULT,
            &mut accounting,
        )
        .unwrap();
        assert_eq!(module, expected);
        assert_eq!(accounting.metrics.count_bytes, module.len());
        assert_eq!(accounting.metrics.render_bytes, module.len());
        assert!(accounting.metrics.admission_scratch_peak > 0);
        eprintln!("Unit2D old-record phase ledger: {:?}", accounting.metrics);
        assert!(accounting.metrics.metadata_peak <= 26_620_032);
        assert_eq!(
            accounting.metrics.emitter_transient_bound,
            EMITTER_TRANSIENT_BYTES
        );
        assert!(
            accounting.metrics.metadata_peak
                >= accounting.metrics.plan_bytes + accounting.metrics.admission_scratch_peak
        );
        let metadata = accounting
            .metrics
            .metadata_admitted_bytes
            .max(accounting.metrics.metadata_peak);
        let limits = Limits {
            metadata_bytes: metadata,
            ir_bytes: module.len(),
            ..Limits::DEFAULT
        };
        assert_eq!(
            native_module_limits(&witness, Some(ENTRY), &sources, plan::MAX_FUEL, limits).unwrap(),
            module
        );
        assert_denial(
            &native_module_limits(
                &witness,
                Some(ENTRY),
                &sources,
                plan::MAX_FUEL,
                Limits {
                    metadata_bytes: metadata - 1,
                    ..limits
                },
            )
            .unwrap_err(),
            "metadata bytes",
        );
        // The next new reservation would be LLVM, but count denial wins.
        let mut denied = Accounting {
            fail_after: Some(accounting.metrics.allocation_attempts - 1),
            ..Accounting::default()
        };
        let error = native_module_accounted(
            &witness,
            Some(ENTRY),
            &sources,
            plan::MAX_FUEL,
            Limits {
                ir_bytes: module.len() - 1,
                ..limits
            },
            &mut denied,
        )
        .unwrap_err();
        assert_denial(&error, "LLVM bytes");
        assert_eq!(denied.metrics.failed_allocation, None);
        assert_eq!(denied.metrics.render_bytes, 0);
    }
    let (sources, s) = fixtures::context();
    let witness = verified::verify_owned(scalar_raw(&[s(3), s(1)]), &sources).unwrap();
    let mut accounting = Accounting::default();
    let expected = native_module_accounted(
        &witness,
        Some(ENTRY),
        &sources,
        plan::MAX_FUEL,
        Limits::DEFAULT,
        &mut accounting,
    )
    .unwrap();
    assert_eq!(accounting.metrics.unique, 2);
    assert_eq!(
        native_module_limits(
            &witness,
            Some(ENTRY),
            &sources,
            plan::MAX_FUEL,
            Limits {
                metadata_bytes: usize::MAX,
                diagnostic_bytes: usize::MAX,
                ir_bytes: usize::MAX,
                ..Limits::DEFAULT
            }
        )
        .unwrap(),
        expected
    );
}

#[test]
fn array_native_resource_entry_and_plan_denials_precede_new_allocations() {
    let (sources, raw, _) = fixtures::owned_relay();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    for (entry, code, text) in [
        (None, "E0700", "declared"),
        (Some(hir::DefId(999)), "E0500", "identity"),
        (Some(hir::DefId(1)), "E0700", "no parameters"),
    ] {
        let mut accounting = Accounting {
            fail_after: Some(0),
            ..Accounting::default()
        };
        let error = plan::fail_allocation_after(0, || {
            native_module_accounted(
                &witness,
                entry,
                &sources,
                0,
                Limits {
                    metadata_bytes: 0,
                    diagnostic_bytes: 0,
                    ir_bytes: 0,
                    ..Limits::DEFAULT
                },
                &mut accounting,
            )
        })
        .unwrap_err();
        assert_eq!(error.code, code);
        assert!(error.message.contains(text));
        assert_eq!(accounting.metrics.allocation_attempts, 0);
        assert_eq!(accounting.metrics.inventory_rows, 0);
        assert_eq!(accounting.metrics.count_bytes, 0);
    }
    let mut accounting = Accounting {
        fail_after: Some(0),
        ..Accounting::default()
    };
    let error = plan::fail_allocation_after(0, || {
        native_module_accounted(
            &witness,
            Some(ENTRY),
            &sources,
            0,
            Limits::DEFAULT,
            &mut accounting,
        )
    })
    .unwrap_err();
    assert_denial(&error, "owned allocation");
    assert_eq!(accounting.metrics.allocation_attempts, 0);
    assert_eq!(accounting.metrics.inventory_rows, 0);
}

#[test]
fn array_native_resource_guarded_record_allocations_include_each_message_and_llvm() {
    let (sources, mut raw, _) = fixtures::owned_relay();
    append_cycle(&mut raw);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let diagnostics = Diagnostics::new(&plan, ENTRY, &sources, true, MAX_DIAGNOSTIC_BYTES).unwrap();
    // The fixture's guarded sites use s0..s12; the appended cycle reuses s0.
    // Thirteen unique messages, three table reservations, then one LLVM String.
    assert_eq!(diagnostics.messages.len(), 13);
    let count = 3 + 13 + 1;
    for failure in 0..=count {
        let mut accounting = Accounting {
            fail_after: Some(failure),
            ..Accounting::default()
        };
        let result = native_module_accounted(
            &witness,
            Some(ENTRY),
            &sources,
            7,
            Limits::DEFAULT,
            &mut accounting,
        );
        if failure == count {
            result.unwrap();
            assert_eq!(accounting.metrics.allocation_attempts, count);
            assert_eq!(accounting.metrics.failed_allocation, None);
        } else {
            assert_denial(&result.unwrap_err(), "allocation");
            let phase = match failure {
                0 => "diagnostic occurrences",
                1 => "diagnostic lookup",
                2 => "diagnostic headers",
                last if last == count - 1 => "LLVM",
                _ => "diagnostic message",
            };
            assert_eq!(accounting.metrics.failed_allocation, Some(phase));
            assert_eq!(accounting.metrics.allocation_attempts, failure + 1);
        }
    }
}

// Checkpoint 2: these tests enter arrays only through the closed verifier probe.
fn array_core(origin: Span, length: usize, access: bool) -> RawOwnedProgram {
    let aggregate = AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, length).unwrap());
    let mut function = fixtures::function(0, ValueTy::Scalar(hir::Ty::I32), origin);
    function.locals = (0..5)
        .map(|_| fixtures::scalar(hir::Ty::I32, origin))
        .collect();
    function.owners.push(OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
        kind: OwnerKind::Local { mutable: true },
        span: origin,
    });
    let mut statements = vec![
        fixtures::assign(0, Rvalue::I32(7), origin),
        fixtures::assign(1, Rvalue::I32(0), origin),
    ];
    if access {
        statements.push(fixtures::assign(
            2,
            Rvalue::CheckedI32 {
                op: hir::ArithmeticOp::Add,
                left: fixtures::operand(0, origin),
                right: fixtures::operand(0, origin),
                operator_span: origin,
            },
            origin,
        ));
    }
    statements.extend([
        fixtures::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), origin),
        fixtures::instruction(
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(0),
                elements: vec![fixtures::operand(0, origin); length],
            },
            origin,
        ),
    ]);
    if access {
        statements.extend([
            fixtures::instruction(
                OwnedInstruction::ReadIndex {
                    destination: LocalId(3),
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    index: fixtures::operand(1, origin),
                },
                origin,
            ),
            fixtures::instruction(
                OwnedInstruction::WriteIndex {
                    base: AccessBase::Owner(OwnerPlaceId(0)),
                    index: fixtures::operand(1, origin),
                    value: fixtures::operand(0, origin),
                },
                origin,
            ),
        ]);
    }
    statements.extend([
        fixtures::instruction(
            OwnedInstruction::ArrayLength {
                destination: LocalId(4),
                base: AccessBase::Owner(OwnerPlaceId(0)),
            },
            origin,
        ),
        fixtures::instruction(OwnedInstruction::Discard(OwnerPlaceId(0)), origin),
    ]);
    function.blocks.push(block(
        statements,
        OwnedTerminatorKind::ReturnScalar(fixtures::operand(4, origin)),
        origin,
    ));
    RawOwnedProgram {
        builtins: BuiltinOrigins::None,
        enums: vec![],
        records: vec![],
        functions: vec![function],
    }
}

fn array_relay(length: usize) -> (SourceMap, RawOwnedProgram, Span) {
    // Reuse only the inherited raw call/owner mechanics, never its Schedule.
    let (sources, mut raw, _) = fixtures::owned_relay();
    raw.records.clear();
    let aggregate = AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, length).unwrap());
    let mut bounds_origin = None;
    for function in &mut raw.functions {
        if matches!(function.result, ValueTy::Owned(_)) {
            function.result = ValueTy::Owned(aggregate);
        }
        for owner in &mut function.owners {
            owner.aggregate = AggregateSlot::try_from_aggregate(aggregate).unwrap();
        }
        for block in &mut function.blocks {
            for statement in &mut block.statements {
                match &statement.kind {
                    OwnedInstruction::Construct {
                        destination,
                        fields,
                    } => {
                        statement.kind = OwnedInstruction::ConstructArray {
                            destination: *destination,
                            elements: vec![fields[0].1; length],
                        };
                    }
                    OwnedInstruction::ReadField {
                        destination, base, ..
                    } => {
                        bounds_origin = Some(statement.span);
                        statement.kind = OwnedInstruction::ReadIndex {
                            destination: *destination,
                            base: *base,
                            index: fixtures::operand(2, statement.span),
                        };
                    }
                    _ => {}
                }
            }
        }
    }
    let origin = raw.functions[0].span;
    raw.functions[0]
        .locals
        .push(fixtures::scalar(hir::Ty::I32, origin));
    raw.functions[0].blocks[0]
        .statements
        .insert(1, fixtures::assign(2, Rvalue::I32(0), origin));
    (sources, raw, bounds_origin.unwrap())
}

fn scalar_store_suffix(raw: &mut RawOwnedProgram, count: usize) {
    let function = &mut raw.functions[0];
    let origin = function.span;
    assert!(function.places.is_empty());
    function.places.push(PlaceDecl {
        ty: hir::Ty::I32,
        span: origin,
    });
    let place = Place {
        id: PlaceId(0),
        span: origin,
    };
    function.blocks[0].statements.insert(
        1,
        fixtures::instruction(
            OwnedInstruction::Scalar(Statement::Initialize {
                place,
                value: fixtures::operand(0, origin),
                span: origin,
            }),
            origin,
        ),
    );
    let statements = &mut function.blocks.last_mut().unwrap().statements;
    statements.extend((0..count).map(|_| {
        fixtures::instruction(
            OwnedInstruction::Scalar(Statement::Store {
                place,
                value: fixtures::operand(0, origin),
                operator_span: origin,
                span: origin,
            }),
            origin,
        )
    }));
}

fn observe_array(
    raw: RawOwnedProgram,
    sources: &SourceMap,
    rendering: &SourceMap,
    control: NativeControl,
) -> NativeObservation {
    verified::probe_array_native(
        raw,
        sources,
        budget::Limits::DEFAULT,
        Some(ENTRY),
        rendering,
        control,
    )
    .expect("resource fixtures must pass the authoritative verifier")
}

fn assert_embedded_message(module: &str, id: usize, expected: &str) {
    let mut escaped = String::new();
    for byte in expected.bytes() {
        write!(&mut escaped, "\\{byte:02X}").unwrap();
    }
    assert!(module.contains(&format!(
        "@__oxid_owned_error_{id} = private unnamed_addr constant [{} x i8] c\"{escaped}\"",
        expected.len()
    )));
}

fn assert_complete_emission(observation: &NativeObservation, expected_cells: usize) {
    let module = observation.result.as_ref().unwrap();
    let metrics = &observation.metrics;
    assert_eq!(metrics.row_sizes, [64, 40, 24, 48]);
    assert_eq!(metrics.count_bytes, module.len());
    assert_eq!(metrics.render_bytes, module.len());
    assert_eq!(metrics.transfer_cells, expected_cells);
    assert_eq!(
        metrics.count_expansions,
        expected_cells + metrics.message_bytes
    );
    assert_eq!(metrics.render_expansions, metrics.count_expansions);
    assert_eq!(
        metrics.count_expansion_kinds,
        metrics.render_expansion_kinds
    );
    assert_eq!(
        metrics.count_expansion_kinds[0] + metrics.count_expansion_kinds[1],
        expected_cells
    );
    assert_eq!(metrics.count_expansion_kinds[2], metrics.message_bytes);
    assert_eq!(
        metrics.render_ordinary_visits,
        metrics.count_ordinary_visits
    );
    assert_eq!(
        metrics.render_predecessor_visits,
        metrics.count_predecessor_visits
    );
    assert!(metrics.count_expansions <= metrics.count_bytes);
    assert_eq!(metrics.message_count_bytes, metrics.message_render_bytes);
    assert_eq!(metrics.message_bytes, metrics.message_render_bytes);
    assert_eq!(metrics.source_bytes, metrics.source_prefix_bound);
    assert!(metrics.metadata_peak <= 26_620_032);
    assert!(metrics.metadata_peak >= metrics.plan_bytes + metrics.admission_scratch_peak);
    assert!(metrics.emitter_transient_bound > 0);
}

#[test]
fn array_native_resource_three_failure_kinds_collide_without_aliasing() {
    let (sources, s) = fixtures::context();
    let origin = s(13);
    for guarded in [false, true] {
        let mut raw = array_core(origin, 3, true);
        if guarded {
            append_cycle(&mut raw);
        }
        let observed = observe_array(raw, &sources, &sources, NativeControl::default());
        assert_complete_emission(&observed, 3);
        let (occurrences, kinds): (usize, &[FailureKind]) = if guarded {
            (
                16,
                &[
                    FailureKind::Fuel,
                    FailureKind::Overflow,
                    FailureKind::Bounds,
                ],
            )
        } else {
            (3, &[FailureKind::Overflow, FailureKind::Bounds])
        };
        assert_eq!(observed.metrics.occurrences, occurrences);
        assert_eq!(observed.metrics.unique, kinds.len());
        assert_eq!(observed.metrics.occurrence_bytes, 64 * occurrences);
        assert_eq!(observed.metrics.lookup_bytes, 40 * kinds.len());
        assert_eq!(observed.metrics.message_header_bytes, 24 * kinds.len());
        assert_eq!(observed.metrics.source_bytes, origin.start);
        assert_eq!(observed.metrics.prefix_preflight_rows, kinds.len());
        assert_eq!(observed.metrics.coordinate_rows, kinds.len());
        for (id, &kind) in kinds.iter().enumerate() {
            assert_embedded_message(
                observed.result.as_ref().unwrap(),
                id,
                &ordinary(kind, origin, &sources),
            );
        }
    }
}

#[test]
fn array_native_resource_length_has_no_bounds_inventory_even_for_empty_arrays() {
    let (sources, s) = fixtures::context();
    for length in [0, 1, 4, 1024] {
        let observed = observe_array(
            array_core(s(3), length, false),
            &sources,
            &sources,
            NativeControl::default(),
        );
        assert_complete_emission(&observed, length.max(1));
        assert_eq!(
            (observed.metrics.occurrences, observed.metrics.unique),
            (0, 0)
        );
        assert_eq!(observed.metrics.message_bytes, 0);
        assert_eq!(observed.metrics.source_bytes, 0);
        assert!(!observed
            .result
            .as_ref()
            .unwrap()
            .contains("@__oxid_owned_error_"));
    }
}

#[test]
fn array_native_resource_array_rendering_uses_the_supplied_map() {
    let mut verification = SourceMap::new();
    let file = verification.add("verification.ox".into(), "xxxxxxxx".into());
    let origin = span(file, 4, 6);
    let mut alternate = SourceMap::new();
    alternate.add("alternate\t\n.ox".into(), "é\r\n中x".into());
    let mut shortened = SourceMap::new();
    shortened.add("short.ox".into(), "x".into());
    let mut non_boundary = SourceMap::new();
    non_boundary.add("boundary.ox".into(), "xx中xxx".into());
    for (rendering, prefix) in [
        (alternate, 0),
        (SourceMap::new(), 0),
        (shortened, 0),
        (non_boundary, 0),
    ] {
        // Alternate's end6 lies within 中; the same full key remains originless.
        let observed = observe_array(
            array_core(origin, 2, true),
            &verification,
            &rendering,
            NativeControl::default(),
        );
        assert_complete_emission(&observed, 2);
        assert_eq!(
            (observed.metrics.occurrences, observed.metrics.unique),
            (3, 2)
        );
        assert_eq!(observed.metrics.source_bytes, prefix);
        for (id, kind) in [FailureKind::Overflow, FailureKind::Bounds]
            .into_iter()
            .enumerate()
        {
            assert_embedded_message(
                observed.result.as_ref().unwrap(),
                id,
                &ordinary(kind, origin, &rendering),
            );
        }
    }
    let mut valid_alternate = SourceMap::new();
    valid_alternate.add("valid-alternate\t.ox".into(), "é\r\nZQxx".into());
    let observed = observe_array(
        array_core(origin, 2, true),
        &verification,
        &valid_alternate,
        NativeControl::default(),
    );
    assert_eq!(observed.metrics.source_bytes, 4);
    assert_eq!(observed.metrics.source_scalars, 3);
    for (id, kind) in [FailureKind::Overflow, FailureKind::Bounds]
        .into_iter()
        .enumerate()
    {
        let expected = ordinary(kind, origin, &valid_alternate);
        assert!(expected.ends_with(":2:1\n"));
        assert_embedded_message(observed.result.as_ref().unwrap(), id, &expected);
    }
}

#[test]
fn array_native_resource_relay_accounts_all_four_transfer_sites() {
    for length in [0, 3, 64] {
        let (sources, raw, origin) = array_relay(length);
        let observed = observe_array(raw, &sources, &sources, NativeControl::default());
        // Construct, staged owned argument, incoming parameter, returned result.
        assert_complete_emission(&observed, 4 * length.max(1));
        assert_eq!(
            (observed.metrics.occurrences, observed.metrics.unique),
            (1, 1)
        );
        assert_eq!(
            observed.metrics.message_bytes,
            ordinary(FailureKind::Bounds, origin, &sources).len()
        );
        assert_eq!(observed.metrics.allocation_attempts, 5);
    }
}

#[test]
fn array_native_resource_relay_each_new_allocation_fails_before_success() {
    let phases = [
        Some("diagnostic occurrences"),
        Some("diagnostic lookup"),
        Some("diagnostic headers"),
        Some("diagnostic message"),
        Some("LLVM"),
        None,
    ];
    for (failure, phase) in phases.into_iter().enumerate() {
        let (sources, raw, _) = array_relay(3);
        let observed = observe_array(
            raw,
            &sources,
            &sources,
            NativeControl {
                fail_after: Some(failure),
                ..NativeControl::default()
            },
        );
        if phase.is_none() {
            assert_complete_emission(&observed, 12);
            assert_eq!(observed.metrics.allocation_attempts, 5);
            assert_eq!(observed.metrics.failed_allocation, None);
        } else {
            assert_denial(observed.result.as_ref().unwrap_err(), "allocation");
            assert_eq!(observed.metrics.failed_allocation, phase);
            assert_eq!(observed.metrics.allocation_attempts, failure + 1);
            assert_eq!(observed.metrics.render_bytes, 0);
        }
    }
}

#[test]
fn array_native_resource_array_caps_are_inclusive_and_keep_phase_priority() {
    let (sources, raw, origin) = array_relay(3);
    let accepted = observe_array(raw, &sources, &sources, NativeControl::default());
    assert_complete_emission(&accepted, 12);
    let bytes = accepted.result.as_ref().unwrap().len();
    let text = ordinary(FailureKind::Bounds, origin, &sources).len();
    let metadata = accepted
        .metrics
        .metadata_peak
        .max(accepted.metrics.metadata_admitted_bytes);
    let exact = Limits {
        metadata_bytes: metadata,
        diagnostic_bytes: text,
        ir_bytes: bytes,
        ..Limits::DEFAULT
    };
    let (_, raw, _) = array_relay(3);
    let observed = observe_array(
        raw,
        &sources,
        &sources,
        NativeControl {
            limits: exact,
            ..NativeControl::default()
        },
    );
    assert_eq!(observed.result.unwrap(), *accepted.result.as_ref().unwrap());
    for (limits, fail_after, marker, attempts) in [
        (
            Limits {
                metadata_bytes: 0,
                ..exact
            },
            0,
            "metadata bytes",
            0,
        ),
        (
            Limits {
                diagnostic_bytes: text - 1,
                ..exact
            },
            3,
            "diagnostic bytes",
            3,
        ),
        (
            Limits {
                ir_bytes: bytes - 1,
                ..exact
            },
            4,
            "LLVM bytes",
            4,
        ),
    ] {
        let (_, raw, _) = array_relay(3);
        let observed = observe_array(
            raw,
            &sources,
            &sources,
            NativeControl {
                limits,
                fail_after: Some(fail_after),
                ..NativeControl::default()
            },
        );
        assert_denial(observed.result.as_ref().unwrap_err(), marker);
        assert_eq!(observed.metrics.allocation_attempts, attempts);
        assert_eq!(observed.metrics.failed_allocation, None);
        assert_eq!(observed.metrics.render_bytes, 0);
    }
    let (_, raw, _) = array_relay(3);
    let observed = observe_array(
        raw,
        &sources,
        &sources,
        NativeControl {
            limits: Limits {
                metadata_bytes: metadata - 1,
                ..exact
            },
            ..NativeControl::default()
        },
    );
    assert_denial(observed.result.as_ref().unwrap_err(), "metadata bytes");
    assert_eq!(observed.metrics.render_bytes, 0);
}

#[test]
fn array_native_resource_malformed_and_entry_priorities_are_closed_by_verification() {
    let (sources, s) = fixtures::context();
    let origin = s(5);
    let mut malformed = array_core(origin, 3, true);
    let statement = malformed.functions[0].blocks[0]
        .statements
        .iter_mut()
        .find(|statement| matches!(statement.kind, OwnedInstruction::ReadIndex { .. }))
        .unwrap();
    let OwnedInstruction::ReadIndex { destination, .. } = &mut statement.kind else {
        unreachable!()
    };
    *destination = LocalId(999);
    let error = plan::fail_allocation_after(0, || {
        verified::probe_array_native(
            malformed,
            &sources,
            budget::Limits::DEFAULT,
            None,
            &SourceMap::new(),
            NativeControl {
                fail_after: Some(0),
                limits: Limits {
                    metadata_bytes: 0,
                    diagnostic_bytes: 0,
                    ir_bytes: 0,
                    ..Limits::DEFAULT
                },
                ..NativeControl::default()
            },
        )
    })
    .err()
    .unwrap();
    assert_eq!(error.kind, OwnedFailureKind::Malformed(Malformed::Id));
    assert_eq!(error.primary.get(), Some(origin));
    // Production validates the same array program before its consumer checks.
    let witness = verified::verify_owned(array_core(origin, 3, true), &sources).unwrap();
    let entry_error = native::native_module(&witness, None, &sources).unwrap_err();
    assert_eq!(entry_error.code, "E0700");
    for (entry, code, marker) in [
        (None, "E0700", "declared"),
        (Some(hir::DefId(999)), "E0500", "identity"),
    ] {
        let observed = plan::fail_allocation_after(0, || {
            verified::probe_array_native(
                array_core(origin, 3, true),
                &sources,
                budget::Limits::DEFAULT,
                entry,
                &sources,
                NativeControl {
                    fail_after: Some(0),
                    limits: Limits {
                        metadata_bytes: 0,
                        diagnostic_bytes: 0,
                        ir_bytes: 0,
                        ..Limits::DEFAULT
                    },
                    ..NativeControl::default()
                },
            )
        })
        .unwrap();
        let error = observed.result.unwrap_err();
        assert_eq!(error.code, code);
        assert!(error.message.contains(marker));
        assert_eq!(observed.metrics.allocation_attempts, 0);
        assert_eq!(observed.metrics.inventory_rows, 0);
        assert_eq!(observed.metrics.count_bytes, 0);
    }
}

#[test]
fn array_native_resource_expansion_stops_inside_constructor_and_transfer_before_suffix() {
    let mut constructor_cut = false;
    let mut transfer_cut = false;
    for maximum in [2048, 4096, 8192, 12288, 16384, 20480, 24576, 28672, 32768] {
        let mut prefix = None;
        for suffix in [1, 4096] {
            let (sources, mut raw, origin) = array_relay(64);
            scalar_store_suffix(&mut raw, suffix);
            let diagnostic_bytes = ordinary(FailureKind::Bounds, origin, &sources).len();
            let observed = observe_array(
                raw,
                &sources,
                &sources,
                NativeControl {
                    limits: Limits {
                        ir_bytes: maximum,
                        ..Limits::DEFAULT
                    },
                    fail_after: Some(4),
                    ..NativeControl::default()
                },
            );
            let kinds = observed.metrics.count_expansion_kinds;
            let in_constructor = (1..64).contains(&kinds[0]) && kinds[1] == 0;
            let in_transfer = kinds[0] == 64 && (1..64).contains(&kinds[1]);
            // Later caps can reach the intentionally different raw suffixes.
            // Compare only cutoffs proven to precede them, and require both
            // constructor and transfer interior boundaries in this fixed set.
            if suffix == 1 && !in_constructor && !in_transfer {
                break;
            }
            assert_denial(observed.result.as_ref().unwrap_err(), "LLVM bytes");
            assert_eq!(observed.metrics.transfer_cells, 4 * 64);
            assert_eq!(observed.metrics.render_bytes, 0);
            assert_eq!(observed.metrics.render_expansions, 0);
            assert_eq!(observed.metrics.failed_allocation, None);
            assert_eq!(observed.metrics.allocation_attempts, 4);
            assert!(observed.metrics.count_bytes <= maximum);
            assert!(observed.metrics.count_expansions <= maximum + 1);
            assert_eq!(kinds[2], diagnostic_bytes);
            assert_eq!(
                observed.metrics.count_expansions,
                kinds.iter().sum::<usize>()
            );
            constructor_cut |= in_constructor;
            transfer_cut |= in_transfer;
            let actual = (
                observed.metrics.count_bytes,
                observed.metrics.count_expansions,
                kinds,
                observed.metrics.count_ordinary_visits,
                observed.metrics.count_predecessor_visits,
            );
            if let Some(expected) = prefix {
                assert_eq!(actual, expected, "cap={maximum}, suffix={suffix}");
            } else {
                prefix = Some(actual);
            }
        }
    }
    assert!(
        constructor_cut,
        "fixed caps must exercise an interior constructor cutoff"
    );
    assert!(
        transfer_cut,
        "fixed caps must exercise an interior transfer cutoff"
    );
}

#[test]
fn array_native_resource_escaping_stops_before_functions_and_oversized_suffix() {
    let mut rendering = SourceMap::new();
    rendering.add("\n".repeat(1024), "x\n".repeat(4096));
    let mut prefix = None;
    for suffix in [1, 4096] {
        let (sources, mut raw, origin) = array_relay(64);
        scalar_store_suffix(&mut raw, suffix);
        let expected_message = ordinary(FailureKind::Bounds, origin, &rendering);
        let observed = observe_array(
            raw,
            &sources,
            &rendering,
            NativeControl {
                limits: Limits {
                    ir_bytes: 1024,
                    ..Limits::DEFAULT
                },
                fail_after: Some(4),
                ..NativeControl::default()
            },
        );
        assert_denial(observed.result.as_ref().unwrap_err(), "LLVM bytes");
        assert_eq!(observed.metrics.message_bytes, expected_message.len());
        assert!(observed.metrics.count_expansions > 0);
        assert!(observed.metrics.count_expansions < expected_message.len());
        assert_eq!(
            observed.metrics.count_expansion_kinds,
            [0, 0, observed.metrics.count_expansions]
        );
        assert_eq!(observed.metrics.render_bytes, 0);
        assert_eq!(observed.metrics.render_expansions, 0);
        assert_eq!(observed.metrics.failed_allocation, None);
        assert_eq!(observed.metrics.allocation_attempts, 4);
        let actual = (
            observed.metrics.count_bytes,
            observed.metrics.count_expansions,
            observed.metrics.count_ordinary_visits,
        );
        if let Some(expected) = prefix {
            assert_eq!(actual, expected);
        } else {
            prefix = Some(actual);
        }
    }
}

#[test]
fn array_native_resource_partial_allocation_peaks_preserve_every_live_inventory() {
    let (sources, s) = fixtures::context();
    let spans: Vec<_> = (0..9).map(s).collect();
    let witness = verified::verify_owned(scalar_raw(&spans), &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    for (failure, previous_peak, expected) in [
        (0, 0, 232),
        (1, 0, 808),
        (2, 0, 1168),
        (1, 4096, 4096),
        (2, 4096, 4096),
    ] {
        let mut accounting = Accounting {
            fail_after: Some(failure),
            ..Accounting::default()
        };
        accounting.metrics.metadata_peak = previous_peak;
        let error = Diagnostics::new_accounted(
            &plan,
            ENTRY,
            &sources,
            false,
            MAX_DIAGNOSTIC_BYTES,
            plan::MAX_PLAN_BYTES,
            &mut accounting,
        )
        .err()
        .unwrap();
        assert_denial(&error, "allocation");
        assert_eq!(accounting.metrics.metadata_peak, expected);
        assert_eq!(accounting.metrics.occurrence_bytes, 9 * 64);
    }
}
