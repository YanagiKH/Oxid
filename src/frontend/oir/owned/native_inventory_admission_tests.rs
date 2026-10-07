//! Production admission controls for the explicit inventory successor.
//! Predecessor X denials are retained as historical data, not a hidden policy.
use super::super::consumer_fixtures as fixtures;
use super::*;
use plan::native_storage::inventory_tests::{item_boundary, width_boundary};

fn current(
    plan: &ExecutionPlan<'_>,
    limits: Limits,
    caps: (usize, usize),
) -> Result<Vec<Bound>, Box<Diagnostic>> {
    let limits = Limits {
        inventory_items: limits.inventory_items.min(caps.0),
        owner_width: limits.owner_width.min(caps.1),
        ..limits
    }
    .bounded();
    admit_policy_accounted(
        plan,
        limits,
        NativeEntryPolicy::Result,
        &mut Accounting::default(),
    )
}

fn denied(result: Result<Vec<Bound>, Box<Diagnostic>>, name: &str, maximum: usize) {
    let error = result.err().unwrap();
    assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
    assert_eq!(
        error.message,
        format!("native owned {name} limit exceeded ({maximum})")
    );
}

#[test]
fn native_inventory_shared_earlier_gates_precede_new_policy() {
    let (sources, raw, _) = fixtures::shared_read();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    for (limits, name, maximum) in [
        (
            Limits {
                functions: 1,
                ..Limits::DEFAULT
            },
            "function count",
            1,
        ),
        (
            Limits {
                parameters: 0,
                ..Limits::DEFAULT
            },
            "parameter count",
            0,
        ),
        (
            Limits {
                function_slots: 0,
                ..Limits::DEFAULT
            },
            "scalar slots per function",
            0,
        ),
        (
            Limits {
                scalar_slots: 0,
                ..Limits::DEFAULT
            },
            "aggregate scalar slots",
            0,
        ),
        (
            Limits {
                blocks: 0,
                ..Limits::DEFAULT
            },
            "aggregate blocks",
            0,
        ),
    ] {
        // I/W zero would also fail; the retained earlier gate must win.
        denied(current(&plan, limits, (0, 0)), name, maximum);
    }
    let (sources, raw, _) = fixtures::empty_record();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    denied(
        current(
            &plan,
            Limits {
                function_slots: 1,
                ..Limits::DEFAULT
            },
            (0, 0),
        ),
        "scalar and owner slots per function",
        1,
    );
}

#[test]
fn native_inventory_shared_former_x_reaches_exact_byte_gates() {
    let (sources, raw) = width_boundary(false);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    assert!(
        plan.functions()
            .iter()
            .map(|f| f.usage().expanded_cells)
            .sum::<usize>()
            > 8192
    );
    let exact = 8 + 8192 * 4; // one scalar and eight 1024-i32 owners
    current(
        &plan,
        Limits {
            bytes: exact,
            live_bytes: exact,
            ..Limits::DEFAULT
        },
        (8192, 8192),
    )
    .unwrap();
    denied(
        current(
            &plan,
            Limits {
                bytes: exact - 1,
                ..Limits::DEFAULT
            },
            (8192, 8192),
        ),
        "aggregate storage bytes",
        exact - 1,
    );
    denied(
        current(
            &plan,
            Limits {
                live_bytes: exact - 1,
                ..Limits::DEFAULT
            },
            (8192, 8192),
        ),
        "live storage bytes",
        exact - 1,
    );
    denied(
        current(
            &plan,
            Limits {
                inventory_items: 8,
                ..Limits::DEFAULT
            },
            (8192, 8192),
        ),
        "aggregate compiler inventory items",
        8,
    );
    denied(
        current(
            &plan,
            Limits {
                owner_width: 8191,
                ..Limits::DEFAULT
            },
            (8192, 8192),
        ),
        "aggregate owner width cells",
        8191,
    );
}

fn invoke_block(call: usize, target: usize, span: Span) -> OwnedBlock {
    OwnedBlock {
        merge: None,
        span,
        statements: vec![fixtures::instruction(
            OwnedInstruction::OpenCall(CallSiteId(call)),
            span,
        )],
        terminator: fixtures::end(
            OwnedTerminatorKind::Invoke {
                call: CallSiteId(call),
                continuation: BlockId(target),
            },
            span,
        ),
    }
}
fn return_block(local: usize, span: Span) -> OwnedBlock {
    OwnedBlock {
        merge: None,
        span,
        statements: vec![],
        terminator: fixtures::end(
            OwnedTerminatorKind::ReturnScalar(fixtures::operand(local, span)),
            span,
        ),
    }
}
fn call_decl(target: usize, local: usize, span: Span) -> CallDecl {
    CallDecl {
        target: hir::DefId(target),
        parent: None,
        arguments: vec![],
        result: CallResult::Scalar(LocalId(local)),
        span,
    }
}
fn leaf(id: usize, span: Span) -> RawOwnedFunction {
    let mut f = fixtures::function(id, ValueTy::Scalar(hir::Ty::I32), span);
    f.locals.push(fixtures::scalar(hir::Ty::I32, span));
    let mut b = return_block(0, span);
    b.statements.push(fixtures::assign(0, Rvalue::I32(0), span));
    f.blocks.push(b);
    f
}
fn single_call(id: usize, target: usize, span: Span) -> RawOwnedFunction {
    let mut f = fixtures::function(id, ValueTy::Scalar(hir::Ty::I32), span);
    f.locals.push(fixtures::scalar(hir::Ty::I32, span));
    f.calls.push(call_decl(target, 0, span));
    f.blocks
        .extend([invoke_block(0, 1, span), return_block(0, span)]);
    f
}

#[test]
fn native_inventory_shared_former_x_keeps_unused_graph_depth_and_recursion() {
    for recursive in [false, true] {
        let (sources, mut raw) = width_boundary(false);
        let span = raw.functions[0].span;
        if recursive {
            raw.functions
                .extend([single_call(1, 2, span), single_call(2, 1, span)]);
        } else {
            for id in 1..33 {
                raw.functions.push(single_call(id, id + 1, span));
            }
            raw.functions.push(leaf(33, span));
        }
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let plan = ExecutionPlan::build(&witness).unwrap();
        native_inventory_fits(&plan);
        assert!(
            plan.functions()
                .iter()
                .map(|f| f.usage().expanded_cells)
                .sum::<usize>()
                > 8192
        );
        let result = current(&plan, Limits::DEFAULT, (8192, 8192));
        if recursive {
            let error = result.err().unwrap();
            assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
            assert_eq!(error.message, "native preview does not support recursive call graphs, including unused functions and unchosen branches");
        } else {
            denied(result, "call depth", 32);
        }
    }
}
fn native_inventory_fits(plan: &ExecutionPlan<'_>) {
    let counts = plan::native_storage::NativeInventories::checked(plan).unwrap();
    assert!(counts.items() <= 8192 && counts.owner_width() == 8192);
}

#[test]
fn native_inventory_shared_former_x_keeps_default_static_cost_gate() {
    let (sources, mut raw) = width_boundary(false);
    let span = raw.functions[0].span;
    raw.functions.push(leaf(1, span));
    // An unused, acyclic doubling DAG has linear compiler inventory. Each
    // callee's bound is reused twice by the unchanged static-cost recurrence.
    // C_0=3, C_n=11+2*C_(n-1); C_13=114677 exceeds 100000 at depth 14.
    for id in 2..15 {
        let mut f = fixtures::function(id, ValueTy::Scalar(hir::Ty::I32), span);
        f.locals = vec![fixtures::scalar(hir::Ty::I32, span); 2];
        f.calls = vec![call_decl(id - 1, 0, span), call_decl(id - 1, 1, span)];
        f.blocks = vec![
            invoke_block(0, 1, span),
            invoke_block(1, 2, span),
            return_block(0, span),
        ];
        raw.functions.push(f);
    }
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    native_inventory_fits(&plan);
    assert!(
        plan.functions()
            .iter()
            .map(|f| f.usage().expanded_cells)
            .sum::<usize>()
            > 8192
    );
    denied(
        current(&plan, Limits::DEFAULT, (8192, 8192)),
        "reference fuel upper bound",
        100000,
    );
}

#[test]
fn native_inventory_shared_mixed_endpoint_retains_bound_layout_and_current_wrapper() {
    let (sources, raw) = item_boundary(false, true, false);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let bounds = current(&plan, Limits::DEFAULT, (8192, 8192)).unwrap();
    assert_eq!(bounds.len(), 32);
    for (f, bound) in witness.functions().iter().zip(&bounds) {
        assert_eq!(bound.cells, plan.function(f.id).usage().expanded_cells);
        assert_eq!(bound.bytes, 255 * 8 + 256 * 4);
        assert_eq!(bound.depth, 1);
    }
    assert!(!native_module(&witness, Some(hir::DefId(0)), &sources)
        .unwrap()
        .is_empty());
    assert_eq!(size_of::<Limits>(), 112);
    assert_eq!(size_of::<Bound>(), 48);
    assert_eq!(size_of::<NativeControl>(), 136);
}

#[test]
fn native_inventory_shared_fixed_roles_include_actual_caller() {
    let (sources, raw, _) = fixtures::owned_relay();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    // Named live caller roles at the I/W insertion point, measured as a tuple
    // so their alignment is not omitted. The old per-function loop has ended;
    // Bound/ready and diagnostic/emission carriers do not yet coexist here.
    // Vec payloads stay in the existing capacity ledger. This tuple is a
    // conservative fixed-role model, not the compiler's machine-stack layout.
    type Caller<'p, 'w> = (
        Limits,
        NativeEntryPolicy,
        &'p ExecutionPlan<'w>,
        &'p mut Accounting,
        &'w [RawOwnedFunction],
        Span,
        Vec<Vec<usize>>,
        Vec<usize>,
        usize,
        [usize; 3],
    );
    let caller = size_of::<Caller<'_, '_>>();
    println!(
        "native inventory caller={caller} limits={} policy={} bound={} control={}",
        size_of::<Limits>(),
        size_of::<NativeEntryPolicy>(),
        size_of::<Bound>(),
        size_of::<NativeControl>()
    );
    for (phase, counters) in
        plan::native_storage::inventory_tests::inventory_fixed_counter_phases(&plan)
    {
        let combined = caller + counters;
        println!("native inventory caller coexistence {phase}: caller={caller} counters={counters} total={combined}");
        assert!(
            combined <= plan::native_storage::FIXED_CARRIER_ALLOWANCE,
            "{phase}"
        );
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the native inventory gate"]
fn native_inventory_shared_source_reuses_helper_beyond_historical_x() {
    use super::super::{execute, source::resource_fixtures as source};
    use super::tests::Scratch;

    // Ordinary source owns the declarations and every borrow/call. The unused
    // source loop selects the production guarded ABI without changing main.
    let parameters = (0..32)
        .map(|i| format!("p{i}:&T"))
        .collect::<Vec<_>>()
        .join(",");
    let arguments = ["&x"; 32].join(",");
    let text = format!(
        "struct T{{value:i32}}\nfn sink({parameters})->i32{{return p0.value;}}\nfn main()->i32{{let x=T{{value:7}};{}return 7;}}\nfn guard()->(){{while false{{}}return;}}\n",
        format!("sink({arguments});").repeat(20),
    );
    let case = source::checked(&text);
    let plan = ExecutionPlan::build(&case.witness).unwrap();
    assert_eq!(case.witness.functions().len(), 3);
    assert_eq!(case.entry, hir::DefId(1));

    // Source census, independent of native admission: sink has one field-read
    // scalar and 32 parameters; main has initializer/return scalars and 20
    // call-result scalars, two owners, and 32 loans/arguments per call. guard
    // adds one bool and one unit scalar. W counts the two one-field owners.
    // X=S+A+W+4O+10R+14L+2C remains reference/fuel data, not native policy.
    for (id, s, a, o, r, l, c, w, x, bytes) in [
        (0, 1, 0, 0, 32, 0, 0, 0, 321, 264),
        (1, 22, 640, 2, 0, 640, 20, 2, 9672, 10424),
        (2, 2, 0, 0, 0, 0, 0, 0, 2, 16),
    ] {
        let usage = plan.function(hir::DefId(id)).usage();
        assert_eq!(
            (
                usage.scalar_slots,
                usage.arguments,
                usage.owners,
                usage.references,
                usage.loans,
                usage.calls,
                usage.owner_cells,
                usage.expanded_cells,
                usage.native_bytes,
            ),
            (s, a, o, r, l, c, w, x, bytes),
        );
    }
    let inventories = plan::native_storage::NativeInventories::checked(&plan).unwrap();
    assert_eq!((inventories.items(), inventories.owner_width()), (1359, 2));
    assert_eq!(
        plan.functions()
            .iter()
            .map(|f| f.usage().expanded_cells)
            .sum::<usize>(),
        9995,
    );
    let bounds = admit(&plan, Limits::DEFAULT).unwrap();
    assert_eq!(
        (bounds[case.entry.0].cells, bounds[case.entry.0].bytes),
        (9993, 10688)
    );
    assert!(bounds[2].cyclic);

    // Find the reference's first success without consulting native cost bounds
    // or replacing the production wrapper. Every failed probe must be fuel.
    let reference = |fuel| {
        execute::run_limits(
            &case.witness,
            Some(case.entry),
            execute::Limits {
                fuel,
                ..execute::Limits::default()
            },
        )
    };
    assert_eq!(reference(plan::MAX_FUEL), Ok(Scalar::I32(7)));
    let (mut low, mut first) = (0, plan::MAX_FUEL);
    while low < first {
        let middle = low + (first - low) / 2;
        match reference(middle) {
            Ok(value) => {
                assert_eq!(value, Scalar::I32(7));
                first = middle;
            }
            Err(execute::OwnedRunFailure::Scalar(RunFailure::Fuel(_))) => low = middle + 1,
            other => panic!("unexpected reference fuel probe at {middle}: {other:?}"),
        }
    }
    assert!(first > 0 && first < plan::MAX_FUEL);
    let evidence = std::env::var_os("OXID_OWNED_NATIVE_EVIDENCE").map(std::path::PathBuf::from);
    if let Some(dir) = &evidence {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("shared-record-32x20.ox"), &text).unwrap();
        std::fs::write(
            dir.join("shared-record-32x20-inventory.json"),
            format!("{{\"S\":25,\"A\":640,\"O\":2,\"R\":32,\"L\":640,\"C\":20,\"I\":1359,\"W\":2,\"X\":9995,\"aggregate_native_bytes\":10712,\"live_native_bytes\":10696,\"first_success_fuel\":{first}}}\n"),
        )
        .unwrap();
    }
    let scratch = Scratch::new();
    for fuel in [first - 1, first, first + 1] {
        let (status, stdout, stderr) = match reference(fuel) {
            Ok(value) => {
                assert!(fuel >= first);
                assert_eq!(value, Scalar::I32(7));
                (0, b"7\n".to_vec(), Vec::new())
            }
            Err(error @ execute::OwnedRunFailure::Scalar(RunFailure::Fuel(_))) => {
                assert_eq!(fuel, first - 1);
                (
                    1,
                    Vec::new(),
                    error
                        .diagnostic(&case.sources)
                        .render_human(&case.sources)
                        .into_bytes(),
                )
            }
            other => panic!("unexpected reference boundary at {fuel}: {other:?}"),
        };
        let module =
            native_module_with_fuel(&case.witness, case.entry, &case.sources, fuel).unwrap();
        assert!(module.contains("%fuel = alloca"));
        let name = format!("shared-record-32x20-fuel-{fuel}");
        let binary = scratch.compile(&module, &name);
        let actual = scratch.run(&binary, &[]);
        if let Some(dir) = &evidence {
            for (suffix, contents) in [
                ("stdout", actual.stdout.as_slice()),
                ("stderr", actual.stderr.as_slice()),
                ("reference.stdout", stdout.as_slice()),
                ("reference.stderr", stderr.as_slice()),
            ] {
                std::fs::write(dir.join(format!("{name}.{suffix}")), contents).unwrap();
            }
            std::fs::write(
                dir.join(format!("{name}.status")),
                format!("native={:?}\nreference={status}\n", actual.status.code()),
            )
            .unwrap();
        }
        assert_eq!(actual.status.code(), Some(status), "fuel={fuel}");
        assert_eq!(actual.stdout, stdout, "fuel={fuel}");
        assert_eq!(actual.stderr, stderr, "fuel={fuel}");
    }
    eprintln!("native inventory ordinary source: I1359 W2 historical X9995; first reference success {first}; three production source-free ELF fuel boundaries match");
}
