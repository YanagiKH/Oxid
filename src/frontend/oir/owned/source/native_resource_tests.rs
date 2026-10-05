//! Registered below native::tests: no production visibility is widened.
use super::super::*;
use crate::frontend::oir::owned::source::resource_fixtures as source;

fn denial(error: &Diagnostic, name: &str, maximum: usize, origin: Span) {
    assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
    assert_eq!(
        error.message,
        format!("native owned {name} limit exceeded ({maximum})")
    );
    assert_eq!(error.primary, Some(origin));
}

#[test]
fn source_native_scalar_plus_owner_slots_256_are_inclusive_257_is_denied() {
    for extra in [false, true] {
        let case = source::checked(&source::scalar_owner_slots(extra));
        let plan = ExecutionPlan::build(&case.witness).unwrap();
        let usage = plan.function(case.entry).usage();
        assert_eq!(
            (usage.scalar_slots, usage.owners, usage.expanded_cells),
            (254 + usize::from(extra), 2, 264 + usize::from(extra))
        );
        let result = admit(&plan, Limits::DEFAULT);
        if extra {
            denial(
                &result.unwrap_err(),
                "scalar and owner slots per function",
                256,
                case.name("main"),
            );
        } else {
            result.unwrap();
            assert!(native_module_limits(
                &case.witness,
                Some(case.entry),
                &case.sources,
                plan::MAX_FUEL,
                Limits::DEFAULT
            )
            .is_ok());
        }
    }
}

#[test]
fn source_native_8192_aggregate_and_path_cells_are_inclusive() {
    let case = source::checked(&source::native_cell_chain(false));
    let plan = ExecutionPlan::build(&case.witness).unwrap();
    for (
        id,
        scalar_slots,
        arguments,
        owners,
        owner_cells,
        payload_bytes,
        references,
        loans,
        calls,
        reference_bytes,
        native_bytes,
    ) in std::iter::once((0, 229, 1, 2, 2, 8, 0, 1, 1, 2040, 1856))
        .chain((1..31).map(|id| (id, 229, 1, 0, 0, 0, 1, 1, 1, 2048, 1856)))
        .chain(std::iter::once((31, 246, 0, 0, 0, 0, 1, 0, 0, 2048, 1976)))
    {
        assert_eq!(
            plan.function(hir::DefId(id)).usage(),
            plan::FrameUsage {
                scalar_slots,
                arguments,
                owners,
                owner_cells,
                payload_bytes,
                references,
                loans,
                calls,
                expanded_cells: 256,
                reference_bytes,
                native_bytes
            }
        );
    }
    let bounds = admit(&plan, Limits::DEFAULT).unwrap();
    assert_eq!(
        (
            bounds[0].depth,
            bounds[0].scalar_slots,
            bounds[0].cells,
            bounds[0].bytes
        ),
        (32, 7345, 8192, 59512)
    );
    assert!(!bounds[0].cyclic);
    // Default aggregate8192 necessarily precedes path8193. Isolate the path
    // check at its own lowered seam using the same unmodified source witness.
    denial(
        &admit(
            &plan,
            Limits {
                live_cells: 8191,
                ..Limits::DEFAULT
            },
        )
        .unwrap_err(),
        "live expanded cells",
        8191,
        case.name("main"),
    );

    let over = source::checked(&source::native_cell_chain(true));
    let over_plan = ExecutionPlan::build(&over.witness).unwrap();
    assert_eq!(
        over_plan.function(hir::DefId(31)).usage().expanded_cells,
        257
    );
    assert_eq!(
        over_plan
            .functions()
            .iter()
            .map(|f| f.usage().expanded_cells)
            .sum::<usize>(),
        8193
    );
    denial(
        &admit(&over_plan, Limits::DEFAULT).unwrap_err(),
        "aggregate expanded cells",
        8192,
        over.name("main"),
    );
}

#[test]
fn source_native_owner_classes_have_independent_aggregate_and_path_seams() {
    let case = source::checked(source::OWNER_CLASSES);
    let plan = ExecutionPlan::build(&case.witness).unwrap();
    source::assert_owner_classes(&plan);
    // Physical views: aggregate X10+11+42=63 and native D8+16+56=80.
    // Read now maximizes path cells (42+11=53) and native bytes (56+16=72).
    let limits = Limits {
        cells: 63,
        live_cells: 53,
        bytes: 80,
        live_bytes: 72,
        ..Limits::DEFAULT
    };
    let bounds = admit(&plan, limits).unwrap();
    assert_eq!((bounds[2].cells, bounds[2].bytes), (53, 72));
    assert!(native_module_limits(
        &case.witness,
        Some(case.entry),
        &case.sources,
        plan::MAX_FUEL,
        limits
    )
    .is_ok());
    for (lowered, name, maximum, origin) in [
        (
            Limits {
                cells: 62,
                ..limits
            },
            "aggregate expanded cells",
            62,
            case.name("relay"),
        ),
        (
            Limits {
                live_cells: 52,
                ..limits
            },
            "live expanded cells",
            52,
            case.name("main"),
        ),
        (
            Limits {
                bytes: 79,
                ..limits
            },
            "aggregate storage bytes",
            79,
            case.name("relay"),
        ),
        (
            Limits {
                live_bytes: 71,
                ..limits
            },
            "live storage bytes",
            71,
            case.name("main"),
        ),
    ] {
        denial(&admit(&plan, lowered).unwrap_err(), name, maximum, origin);
    }
    // Dnative<=8X by the same scalar/payload bound; a guarded module adds8.
    // Native byte maxima are therefore masked by aggregate/path8192 cells.
    assert_eq!(8 * MAX_SLOTS + 8, 65_544);
    const { assert!(8 * MAX_SLOTS + 8 < MAX_NATIVE_BYTES) };
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the source resource native gate"]
fn source_native_actual_slot_and_cell_boundaries_use_real_llvm() {
    for (name, text) in [
        ("source-slots256", source::scalar_owner_slots(false)),
        ("source-cells8192", source::native_cell_chain(false)),
    ] {
        let case = source::checked(&text);
        assert_eq!(
            execute::run(&case.witness, Some(case.entry)),
            Ok(Scalar::Unit)
        );
        let module = native_module(&case.witness, Some(case.entry), &case.sources).unwrap();
        let scratch = super::Scratch::new();
        let binary = scratch.compile(&module, name);
        // Compile removes intermediate .ll/.bc files. Each fresh runtime
        // directory contains only its ELF; run clears env and hides all tools.
        assert_eq!(std::fs::read_dir(&scratch.0).unwrap().count(), 1);
        super::assert_result(scratch.run(&binary, &[]), b"()\n", b"", 0);
        println!("{name}: source witness -> LLVM19.1.7 -> source-free ELF; stdout=(), stderr=empty, status=0");
    }
}
