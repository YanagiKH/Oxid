//! Closed output/process native precursor. No alternate witness or positive
//! emitter seam exists here; ordinary raw admission remains authoritative.
use super::super::builtin_input_fixtures::{self as fixture, Observation};
use super::*;

#[test]
fn builtin_output_native_process_gate_precedes_allocations_and_entry_work() {
    let (sources, raw, entry) = fixture::program(3, Observation::Status);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    for selected_entry in [Some(entry), None, Some(hir::DefId(usize::MAX))] {
        let mut accounting = Accounting {
            fail_after: Some(0),
            ..Accounting::default()
        };
        let error = native_module_policy_accounted(
            &witness,
            selected_entry,
            &sources,
            0,
            Limits::DEFAULT,
            NativeEntryPolicy::Process,
            &mut accounting,
        )
        .unwrap_err();
        assert_eq!(error.code, "E0700");
        assert_eq!(error.stage, "native-admission");
        assert_eq!(error.message, "native process entry is not enabled");
        assert_eq!(accounting.metrics.allocation_attempts, 0);
        assert_eq!(accounting.metrics.plan_bytes, 0);
        assert_eq!(accounting.metrics.count_bytes, 0);
        assert_eq!(accounting.metrics.render_bytes, 0);
    }
    assert_eq!(
        native_process_module_with_fuel(&witness, entry, &sources, 1000)
            .unwrap_err()
            .message,
        "native process entry is not enabled"
    );
}

#[test]
fn builtin_output_native_default_entry_retains_scalar_rendering() {
    let (sources, raw, entry) = fixture::program(3, Observation::Status);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let module = native_module(&witness, Some(entry), &sources).unwrap();
    assert!(module.contains("%status = call i32 @__oxid_print_i32(i32 %value)"));
    assert!(module.contains("declare i32 @__oxid_read_stdin_byte(ptr)"));
    assert!(!module.contains("__oxid_write_stdout_byte"));
    assert!(!module.contains("__oxid_process_setup"));
    assert!(!module.contains("__oxid_process_failure"));
    assert!(!module.contains("output_scratch"));
}

#[test]
fn builtin_output_native_raw_gate_rejects_every_output_inventory() {
    for inventory in [
        BuiltinOrigins::WriteStatus,
        BuiltinOrigins::WriteStdout,
        BuiltinOrigins::ReadStatusWriteStatus,
        BuiltinOrigins::ReadStatusWriteStdout,
        BuiltinOrigins::ReadStdinWriteStatus,
        BuiltinOrigins::ReadStdinWriteStdout,
    ] {
        let (sources, mut raw, _) = fixture::program(3, Observation::Status);
        raw.builtins = inventory;
        // These raw rows deliberately retain the input descriptor. The closed
        // output admission gate must reject before examining any candidate row.
        let error = verified::verify_owned(raw, &sources).unwrap_err();
        assert_eq!(error.kind, OwnedFailureKind::Malformed(Malformed::Binding));
    }
}

#[test]
fn builtin_output_native_enclosing_carriers_preserve_current_sizes() {
    // The entry policy consumes Emission's existing bool padding. These are
    // enclosing production/control carriers, not just the new selector size.
    assert_eq!(size_of::<NativeEntryPolicy>(), 1);
    assert_eq!(size_of::<FailureKind>(), 1);
    assert_eq!(size_of::<Continuation>(), 1);
    assert_eq!(size_of::<Emission>(), 120);
    assert_eq!(size_of::<Diagnostics>(), 48);
    assert_eq!(size_of::<DiagnosticOccurrence>(), 64);
    assert_eq!(size_of::<DiagnosticLookup>(), 40);
    assert_eq!(size_of::<Bound>(), 48);
    assert_eq!(size_of::<Limits>(), 112);
    assert_eq!(size_of::<NativeControl>(), 136);
    assert_eq!(EMITTER_TRANSIENT_BYTES, 32_768);
    assert_eq!(MAX_IR_BYTES, 64 * 1024 * 1024);
    assert_eq!(MAX_NATIVE_BYTES, 1024 * 1024);
}

#[test]
fn builtin_output_native_diagnostics_have_exact_fixed_envelopes() {
    let (sources, raw, _) = fixture::program(3, Observation::Status);
    let span = raw.functions[0].span;
    let capacity = FailureKind::OutputCapacity.diagnostic(span, &sources);
    assert_eq!(capacity.code, "E0500");
    assert_eq!(capacity.stage, "oir-owned-run");
    assert_eq!(capacity.primary, Some(span));
    assert_eq!(capacity.message, OUTPUT_CAPACITY_INVARIANT);
    assert_eq!(capacity.message.len(), 66);
    assert_eq!(capacity.message.capacity(), 66);
    assert_eq!(
        FailureKind::OutputCapacity.transient_bytes(),
        size_of::<Diagnostic>() + 66
    );
    let status = FailureKind::ProcessStatus.diagnostic(span, &sources);
    assert_eq!(status.code, "E0600");
    assert_eq!(status.stage, "oir-run");
    assert_eq!(status.primary, Some(span));
    assert_eq!(
        status.message,
        "process main must return a status in 0..255"
    );
    assert!(status.message.capacity() <= 64);
}
