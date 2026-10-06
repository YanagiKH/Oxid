//! Native admission and emission controls for fully verified raw input programs.
//! These tests emit LLVM only; none can consume the test runner's stdin.
use super::super::builtin_input_fixtures::{self as fixture, Observation};
use super::super::consumer_fixtures as f;
use super::super::enum_consumer_fixtures as e;
use super::*;

#[test]
fn builtin_input_native_physical_bytes_have_an_exact_inclusive_endpoint() {
    let (sources, raw, entry) = fixture::program(3, Observation::Status);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let main = plan.function(entry).usage();
    let input = plan.function(hir::DefId(1)).usage();
    // Main: five scalar/argument slots (40), owners (12 + 8 + 8),
    // one loan pointer (8), and its slice length (4).
    assert_eq!(main.native_bytes, 80);
    // Builtin: result (8), fixed scratch (1024), reference (8), length (4).
    assert_eq!(input.native_bytes, 1044);
    assert_eq!(input.payload_bytes, 1032);
    assert_eq!(input.expanded_cells, 16);
    assert_eq!(input.activation_fuel_cells(), 14);
    assert_eq!(plan.input_scratch_range(hir::DefId(1)), Some(8..1032));
    // Both frames coexist at the call; shared fuel contributes another 8 bytes.
    const BYTES: usize = 80 + 1044 + 8;
    assert_eq!(BYTES, 1132);
    let exact = Limits {
        bytes: BYTES,
        live_bytes: BYTES,
        ..Limits::DEFAULT
    };
    let module = native_module_limits(&witness, Some(entry), &sources, 1000, exact).unwrap();
    assert!(module.contains("%owners = alloca [1032 x i8], align 4"));
    assert!(module.contains("%input_scratch = getelementptr i8, ptr %owners, i64 8"));
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
        let error =
            native_module_limits(&witness, Some(entry), &sources, 1000, limits).unwrap_err();
        assert_eq!(error.code, "E0700");
        assert_eq!(
            error.message,
            format!("native owned {label} limit exceeded (1131)")
        );
    }
}

#[test]
fn builtin_input_native_unused_builtin_guards_do_not_exempt_acyclic_main() {
    for include_input in [true, false] {
        let (sources, mut raw, entry) = fixture::program(3, Observation::Status);
        let span = raw.functions[0].span;
        let mut main = f::function(0, ValueTy::Scalar(hir::Ty::I32), span);
        main.locals = vec![f::scalar(hir::Ty::I32, span)];
        main.blocks = vec![e::block(
            vec![f::assign(0, Rvalue::I32(7), span)],
            OwnedTerminatorKind::ReturnScalar(f::operand(0, span)),
            span,
        )];
        raw.functions[0] = main;
        if !include_input {
            raw.builtins = BuiltinOrigins::None;
            raw.enums.clear();
            raw.functions.truncate(1);
        }
        let witness = verified::verify_owned(raw, &sources).unwrap();
        assert_eq!(
            witness.builtin_function(),
            include_input.then_some(hir::DefId(1))
        );
        // One root step, one local activation cell, assignment, and return: 4.
        // Limits.cost gates static admission; the separate fuel argument only
        // initializes emitted runtime guards. An unused builtin must not waive
        // this ordinary main's static bound even though it forces those guards.
        let error = native_module_limits(
            &witness,
            Some(entry),
            &sources,
            3,
            Limits {
                cost: 3,
                ..Limits::DEFAULT
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "E0700");
        assert_eq!(
            error.message,
            "native owned reference fuel upper bound limit exceeded (3)"
        );
        let module = native_module_limits(
            &witness,
            Some(entry),
            &sources,
            4,
            Limits {
                cost: 4,
                ..Limits::DEFAULT
            },
        )
        .unwrap();
        if include_input {
            assert!(module.contains("define internal i32 @__oxid_owned_fn_0(ptr %fuel)"));
            assert!(module.contains("store i64 4, ptr %fuel, align 8"));
            assert!(module.contains("%f0_b0_g1_remaining = load i64, ptr %fuel"));
            assert!(module.contains("%f0_b0_g2_remaining = load i64, ptr %fuel"));
            assert!(module.contains("declare i32 @__oxid_read_stdin_byte(ptr)"));
        } else {
            assert!(module.contains("define internal i32 @__oxid_owned_fn_0()"));
            assert!(!module.contains("%fuel"));
            assert!(!module.contains("__oxid_read_stdin_byte"));
        }
    }
}

#[test]
fn builtin_input_native_count_render_and_active_store_inventory_agree() {
    let (sources, raw, entry) = fixture::program(3, Observation::Status);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let observed = run_array_observed(&witness, Some(entry), &sources, NativeControl::default());
    let module = observed.result.as_ref().unwrap();
    let metrics = &observed.metrics;
    // Three array cells and four result writes; each of the two enum transfers
    // contributes three switch rows and three tag-directed case bodies.
    assert_eq!(metrics.transfer_cells, 3 + 4 + 2 * 6);
    assert_eq!(metrics.transfer_inventory_visits, 2 + 1 + 8 + 24 + 8);
    assert_eq!(
        metrics.count_expansion_kinds,
        [7, 12, metrics.message_bytes]
    );
    assert_eq!(
        metrics.render_expansion_kinds,
        metrics.count_expansion_kinds
    );
    assert_eq!(metrics.count_expansions, 19 + metrics.message_bytes);
    assert_eq!(metrics.render_expansions, metrics.count_expansions);
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
    // The capacity-independent input result text writes Eof's tag and active
    // payload, Full's tag, and IoError's tag, without touching inactive payloads.
    let results = module
        .split_once("f1_b0_i1_result_eof:\n")
        .unwrap()
        .1
        .split_once("f1_b0_i1_input_ok:\n")
        .unwrap()
        .0;
    assert_eq!(results.matches("  store i32 ").count(), 4);
    assert!(results.contains("store i32 %f1_b0_i1_staged, ptr %f1_b0_i1_eof_result_active_out"));
    assert_eq!(results.matches("getelementptr").count(), 1);
    assert!(!results.contains("load "));

    let exact = Limits {
        diagnostic_bytes: metrics.message_bytes,
        ir_bytes: module.len(),
        ..Limits::DEFAULT
    };
    assert_eq!(
        native_module_limits(&witness, Some(entry), &sources, plan::MAX_FUEL, exact).unwrap(),
        *module
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
        let error = native_module_limits(&witness, Some(entry), &sources, plan::MAX_FUEL, limits)
            .unwrap_err();
        assert_eq!(error.code, "E0700");
        assert!(error.message.contains(label), "{}", error.message);
    }
}

#[test]
fn builtin_input_native_diagnostic_transient_is_charged_only_for_input() {
    for has_input in [true, false] {
        let (sources, raw, entry) = if has_input {
            fixture::program(3, Observation::Status)
        } else {
            fixture::ordinary_control(3, Observation::Status)
        };
        let span = raw.functions[1].span;
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let plan = ExecutionPlan::build(&witness).unwrap();
        let mut accounting = Accounting::default();
        let diagnostics = Diagnostics::new_accounted(
            &plan,
            entry,
            &sources,
            has_input,
            MAX_DIAGNOSTIC_BYTES,
            plan::MAX_PLAN_BYTES,
            &mut accounting,
        )
        .unwrap();
        let metrics = &accounting.metrics;
        // Input adds the root/statement/terminator fuel inventory (33) and one
        // InputCapacity row to the ordinary enum inventory (13 occurrences).
        let (occurrences, unique, message_capacity) =
            if has_input { (47, 24, 65) } else { (13, 6, 64) };
        assert_eq!((metrics.occurrences, metrics.unique), (occurrences, unique));
        assert_eq!(metrics.row_sizes, [64, 40, 24, 48]);
        assert_eq!(
            metrics.metadata_admitted_bytes,
            metrics.plan_bytes
                + 2 * 48
                + occurrences * (64 + 40 + 24)
                + size_of::<Diagnostic>()
                + message_capacity
        );
        assert!(metrics.metadata_peak <= metrics.metadata_admitted_bytes);
        assert_eq!(metrics.message_count_bytes, metrics.message_bytes);
        assert_eq!(metrics.message_render_bytes, metrics.message_bytes);
        assert_eq!(metrics.source_bytes, metrics.source_prefix_bound);
        if has_input {
            let diagnostic = FailureKind::InputCapacity.diagnostic(span, &sources);
            assert_eq!(diagnostic.code, "E0500");
            assert_eq!(diagnostic.stage, "oir-owned-run");
            assert_eq!(diagnostic.primary, Some(span));
            assert_eq!(
                diagnostic.message,
                "internal compiler error: owned execution invariant input capacity"
            );
            assert_eq!(diagnostic.message.len(), 65);
            assert_eq!(diagnostic.message.capacity(), 65);
            assert_eq!(
                metrics.diagnostic_transient_peak,
                size_of::<Diagnostic>() + 65
            );
            assert!(diagnostics
                .messages
                .contains(&diagnostic.render_human(&sources)));
        } else {
            assert!(metrics.diagnostic_transient_peak <= size_of::<Diagnostic>() + 64);
            assert!(diagnostics
                .messages
                .iter()
                .all(|message| !message.contains("input capacity")));
        }
    }
}
