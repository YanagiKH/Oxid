//! Native output admission/emission controls through ordinary verification.
//! These tests emit LLVM only; external process proofs exercise OS outcomes.
use super::super::builtin_input_fixtures::{self as input_fixture, Observation};
use super::super::builtin_output_fixtures as fixture;
use super::*;

fn process_observed(
    witness: &VerifiedOwnedProgram,
    entry: Option<hir::DefId>,
    sources: &SourceMap,
    fuel: usize,
    limits: Limits,
) -> NativeObservation {
    let mut accounting = Accounting::default();
    let result = native_module_policy_accounted(
        witness,
        entry,
        sources,
        fuel,
        limits,
        NativeEntryPolicy::Process,
        &mut accounting,
    );
    NativeObservation {
        result,
        metrics: accounting.metrics,
    }
}

#[test]
fn builtin_output_native_default_policy_checks_complete_inventory_before_allocation() {
    for (inventory, has_output_function) in [
        (BuiltinOrigins::None, false),
        (BuiltinOrigins::ReadStatus, false),
        (BuiltinOrigins::ReadStdin, false),
        (BuiltinOrigins::WriteStatus, false),
        (BuiltinOrigins::WriteStdout, true),
        (BuiltinOrigins::ReadStatusWriteStatus, false),
        (BuiltinOrigins::ReadStatusWriteStdout, true),
        (BuiltinOrigins::ReadStdinWriteStatus, false),
        (BuiltinOrigins::ReadStdinWriteStdout, true),
    ] {
        let (sources, raw) = fixture::inventory(inventory);
        let witness = verified::verify_owned(raw, &sources).unwrap();
        if has_output_function {
            // The ordinary main does not call the output function. Inventory
            // policy still denies before entry resolution and all allocations.
            for entry in [Some(hir::DefId(0)), None, Some(hir::DefId(usize::MAX))] {
                let mut accounting = Accounting {
                    fail_after: Some(0),
                    ..Accounting::default()
                };
                let error = native_module_accounted(
                    &witness,
                    entry,
                    &sources,
                    0,
                    Limits::DEFAULT,
                    &mut accounting,
                )
                .unwrap_err();
                assert_eq!(error.code, "E0700");
                assert_eq!(error.message, "write_stdout requires process entry");
                assert_eq!(accounting.metrics.allocation_attempts, 0);
                assert_eq!(accounting.metrics.plan_bytes, 0);
                assert_eq!(accounting.metrics.count_bytes, 0);
            }
        } else {
            let module = native_module(&witness, Some(hir::DefId(0)), &sources).unwrap();
            assert!(module.contains("%status = call i32 @__oxid_print_i32(i32 %value)"));
            assert!(!module.contains("__oxid_write_stdout_byte"));
            assert!(!module.contains("__oxid_process_setup"));
            assert!(!module.contains("__oxid_process_failure"));
        }
        let module =
            native_process_module_with_fuel(&witness, hir::DefId(0), &sources, 1000).unwrap();
        assert_eq!(
            module.contains("declare i32 @__oxid_write_stdout_byte(ptr)"),
            has_output_function
        );
        assert!(module.contains("%setup = call i32 @__oxid_process_setup()"));
        assert!(!module.contains("call i32 @__oxid_print_i32"));
    }
}

#[test]
fn builtin_output_native_process_entry_rejects_invalid_signatures_before_allocation() {
    let (sources, raw, _) = input_fixture::program(3, Observation::Status);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    for (entry, expected) in [
        (
            None,
            "native compile requires a declared zero-argument main",
        ),
        (
            Some(hir::DefId(usize::MAX)),
            "internal compiler error: invalid native entry identity",
        ),
        (Some(hir::DefId(1)), "native main must have no parameters"),
    ] {
        let observed = process_observed(&witness, entry, &sources, 0, Limits::DEFAULT);
        assert_eq!(observed.result.unwrap_err().message, expected);
        assert_eq!(observed.metrics.allocation_attempts, 0);
        assert_eq!(observed.metrics.plan_bytes, 0);
    }
    for (scalar, value) in [
        (hir::Ty::Bool, Rvalue::Bool(true)),
        (hir::Ty::Unit, Rvalue::Unit),
    ] {
        let (sources, mut raw) = fixture::inventory(BuiltinOrigins::None);
        raw.functions[0].result = ValueTy::Scalar(scalar);
        raw.functions[0].locals[0].ty = scalar;
        let OwnedInstruction::Scalar(Statement::Assign(assign)) =
            &mut raw.functions[0].blocks[0].statements[0].kind
        else {
            unreachable!("fixture assignment")
        };
        assign.value = value;
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let observed =
            process_observed(&witness, Some(hir::DefId(0)), &sources, 0, Limits::DEFAULT);
        assert_eq!(
            observed.result.unwrap_err().message,
            "process main must return i32"
        );
        assert_eq!(observed.metrics.allocation_attempts, 0);
        assert_eq!(observed.metrics.plan_bytes, 0);
        assert!(native_module(&witness, Some(hir::DefId(0)), &sources).is_ok());
    }
}

#[test]
fn builtin_output_native_physical_storage_has_exact_inclusive_endpoint() {
    let (sources, raw, entry) = fixture::program(3);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    assert_eq!(plan.function(entry).usage().native_bytes, 48);
    let output = plan.function(hir::DefId(1)).usage();
    assert_eq!(output.native_bytes, 1044);
    assert_eq!(output.payload_bytes, 1032);
    assert_eq!(output.expanded_cells, 16);
    assert_eq!(output.activation_fuel_cells(), 14);
    assert_eq!(plan.output_scratch_range(hir::DefId(1)), Some(8..1032));
    assert_eq!(plan.input_scratch_range(hir::DefId(1)), None);
    // Shared source frame, output result/staging/reference frame, and fuel.
    const BYTES: usize = 48 + 1044 + 8;
    assert_eq!(BYTES, 1100);
    let exact = Limits {
        bytes: BYTES,
        live_bytes: BYTES,
        ..Limits::DEFAULT
    };
    let module = process_observed(&witness, Some(entry), &sources, 1000, exact)
        .result
        .unwrap();
    assert!(module.contains("%owners = alloca [1032 x i8], align 4"));
    assert!(module.contains("%output_scratch = getelementptr i8, ptr %owners, i64 8"));
    for (limits, label) in [
        (
            Limits {
                bytes: BYTES - 1,
                ..exact
            },
            "aggregate storage bytes",
        ),
        (
            Limits {
                live_bytes: BYTES - 1,
                ..exact
            },
            "live storage bytes",
        ),
    ] {
        let observed = process_observed(&witness, Some(entry), &sources, 1000, limits);
        let error = observed.result.unwrap_err();
        assert_eq!(error.code, "E0700");
        assert_eq!(
            error.message,
            format!("native owned {label} limit exceeded (1099)")
        );
        assert_eq!(observed.metrics.count_bytes, 0);
    }
}

#[test]
fn builtin_output_native_acyclic_process_charges_runtime_fuel_storage() {
    let (sources, raw) = fixture::inventory(BuiltinOrigins::None);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let entry = hir::DefId(0);
    let plan = ExecutionPlan::build(&witness).unwrap();
    assert_eq!(plan.function(entry).usage().native_bytes, 8);
    let exact = Limits {
        bytes: 16,
        live_bytes: 16,
        ..Limits::DEFAULT
    };
    let module = process_observed(&witness, Some(entry), &sources, 0, exact)
        .result
        .unwrap();
    assert!(module.contains("store i64 0, ptr %fuel, align 8"));
    assert!(module.contains("%root_exhausted = icmp ult i64 %root_remaining, 2"));
    for limits in [
        Limits { bytes: 15, ..exact },
        Limits {
            live_bytes: 15,
            ..exact
        },
    ] {
        assert!(process_observed(&witness, Some(entry), &sources, 0, limits)
            .result
            .is_err());
        assert!(native_module_limits(&witness, Some(entry), &sources, 0, limits).is_ok());
    }
}

#[test]
fn builtin_output_native_stages_whole_shared_view_before_attempts() {
    for capacity in [0, 1, 3, 1024] {
        let (sources, raw, entry) = fixture::program(capacity);
        let witness = verified::verify_owned(raw, &sources).unwrap();
        assert_eq!(
            witness.functions()[1].references[0].kind,
            BorrowKind::Shared
        );
        let module = native_process_module_with_fuel(&witness, entry, &sources, 100_000).unwrap();
        assert!(module.contains("define internal void @__oxid_owned_fn_1(ptr %fuel, ptr %result, ptr %arg0, i32 %arg0_length)"));
        let effect = module
            .split_once("  %f1_b0_i1_capacity =")
            .unwrap()
            .1
            .split_once("f1_b0_i1_output_ok:\n")
            .unwrap()
            .0;
        let validation = effect
            .find("%f1_b0_i1_cell_valid = icmp ule i32 %f1_b0_i1_cell, 255")
            .unwrap();
        let staged = effect.find("f1_b0_i1_staged:\n").unwrap();
        let write_guard = effect
            .find("%f1_b0_i1_write_exhausted = icmp ult i64 %f1_b0_i1_write_remaining, 1")
            .unwrap();
        let attempt = effect.find("call i32 @__oxid_write_stdout_byte").unwrap();
        assert!(validation < staged && staged < write_guard && write_guard < attempt);
        assert!(effect.contains("%f1_b0_i1_capacity_valid = icmp ule i32 %f1_b0_i1_capacity, 1024"));
        assert!(effect.contains("%f1_b0_i1_core_cost = add i64 %f1_b0_i1_capacity64, 4"));
        assert!(effect
            .contains("%f1_b0_i1_all_validated, label %f1_b0_i1_staged, label %f1_b0_i1_validate"));
        assert!(effect
            .contains("%f1_b0_i1_at_capacity, label %f1_b0_i1_complete, label %f1_b0_i1_attempt"));
        assert!(effect.contains("i32 -1, label %f1_b0_i1_retry"));
        assert_eq!(
            effect.matches("call i32 @__oxid_write_stdout_byte").count(),
            1
        );
        assert!(!effect[staged..].contains("ptr %f1_b0_i1_buffer"));
        assert!(!effect.contains("alloca"));
        assert!(!effect.contains("ptr %input_scratch"));
        let complete = effect
            .split_once("f1_b0_i1_complete:\n")
            .unwrap()
            .1
            .split_once("f1_b0_i1_invalid_input:\n")
            .unwrap()
            .0;
        assert!(complete.contains("store i32 0, ptr %o0, align 1"));
        assert!(!complete.contains("getelementptr"));
        let invalid = effect
            .split_once("f1_b0_i1_invalid_input:\n")
            .unwrap()
            .1
            .split_once("f1_b0_i1_io_error:\n")
            .unwrap()
            .0;
        assert!(invalid.contains("store i32 1, ptr %o0, align 1"));
        assert!(!invalid.contains("getelementptr"));
        let error = effect.split_once("f1_b0_i1_io_error:\n").unwrap().1;
        assert!(error.contains("store i32 2, ptr %o0, align 1"));
        assert!(error
            .contains("store i32 %f1_b0_i1_accepted, ptr %f1_b0_i1_io_error_result_active_out"));
    }
}

#[test]
fn builtin_output_native_count_render_and_output_limits_agree() {
    let (sources, raw, entry) = fixture::program(3);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let observed = process_observed(
        &witness,
        Some(entry),
        &sources,
        plan::MAX_FUEL,
        Limits::DEFAULT,
    );
    let module = observed.result.unwrap();
    let metrics = observed.metrics;
    assert_eq!(metrics.transfer_cells, 3 + 4 + 2 * 6);
    assert_eq!(metrics.transfer_inventory_visits, 2 + 1 + 3 + 11 + 3);
    assert_eq!(
        metrics.count_expansion_kinds,
        [7, 12, metrics.message_bytes]
    );
    assert_eq!(
        metrics.count_expansion_kinds,
        metrics.render_expansion_kinds
    );
    assert_eq!(metrics.count_expansions, 19 + metrics.message_bytes);
    assert_eq!(metrics.count_expansions, metrics.render_expansions);
    assert_eq!(metrics.count_bytes, module.len());
    assert_eq!(metrics.render_bytes, module.len());
    assert_eq!(
        metrics.count_ordinary_visits,
        metrics.render_ordinary_visits
    );
    assert_eq!(
        metrics.count_predecessor_visits,
        metrics.render_predecessor_visits
    );
    assert_eq!(
        metrics.count_borrow_projection_visits,
        metrics.render_borrow_projection_visits
    );
    assert_eq!(metrics.message_count_bytes, metrics.message_bytes);
    assert_eq!(metrics.message_render_bytes, metrics.message_bytes);
    assert!(metrics.metadata_peak <= plan::MAX_PLAN_BYTES);
    let exact = Limits {
        diagnostic_bytes: metrics.message_bytes,
        ir_bytes: module.len(),
        ..Limits::DEFAULT
    };
    assert_eq!(
        process_observed(&witness, Some(entry), &sources, plan::MAX_FUEL, exact)
            .result
            .unwrap(),
        module
    );
    for (limits, label) in [
        (
            Limits {
                diagnostic_bytes: metrics.message_bytes - 1,
                ..exact
            },
            "diagnostic bytes",
        ),
        (
            Limits {
                ir_bytes: module.len() - 1,
                ..exact
            },
            "LLVM bytes",
        ),
    ] {
        let error = process_observed(&witness, Some(entry), &sources, plan::MAX_FUEL, limits)
            .result
            .unwrap_err();
        assert_eq!(error.code, "E0700");
        assert!(error.message.contains(label), "{}", error.message);
    }
}

#[test]
fn builtin_output_native_process_setup_precedes_every_guard_and_status_is_exact() {
    for value in [0, 1, 74, 255, -256, 256] {
        let (sources, mut raw) = fixture::inventory(BuiltinOrigins::None);
        let OwnedInstruction::Scalar(Statement::Assign(assign)) =
            &mut raw.functions[0].blocks[0].statements[0].kind
        else {
            unreachable!("fixture assignment")
        };
        assign.value = Rvalue::I32(value);
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let module = native_process_module_with_fuel(&witness, hir::DefId(0), &sources, 0).unwrap();
        let root = module.split_once("define i32 @main() {").unwrap().1;
        let setup = root.find("call i32 @__oxid_process_setup()").unwrap();
        let guard = root.find("%root_remaining = load i64, ptr %fuel").unwrap();
        let activation = root.find("call i32 @__oxid_owned_fn_0(ptr %fuel)").unwrap();
        assert!(setup < guard && guard < activation);
        let setup_error = root
            .split_once("setup_error:\n")
            .unwrap()
            .1
            .split_once("process_ready:\n")
            .unwrap()
            .0;
        assert_eq!(setup_error, "  ret i32 74\n");
        assert!(root.contains("%status_valid = icmp ule i32 %value, 255"));
        assert!(root.contains("process_complete:\n  ret i32 %value\n"));
        assert!(!root.contains("trunc"));
        assert!(!root.contains("call i32 @__oxid_print"));
        assert!(!module.contains("call void @__oxid_overflow"));
        assert!(module.contains("call void @__oxid_process_failure"));
        let plan = ExecutionPlan::build(&witness).unwrap();
        let mut accounting = Accounting::default();
        let diagnostics = Diagnostics::new_policy_accounted(
            &plan,
            hir::DefId(0),
            &sources,
            true,
            NativeEntryPolicy::Process,
            MAX_DIAGNOSTIC_BYTES,
            plan::MAX_PLAN_BYTES,
            &mut accounting,
        )
        .unwrap();
        let expected = FailureKind::ProcessStatus
            .diagnostic(witness.functions()[0].span, &sources)
            .render_human(&sources);
        assert!(diagnostics.messages.contains(&expected));
    }
}

#[test]
fn builtin_output_native_default_input_entry_retains_scalar_rendering() {
    let (sources, raw, entry) = input_fixture::program(3, Observation::Status);
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
    let (sources, raw, _) = input_fixture::program(3, Observation::Status);
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
