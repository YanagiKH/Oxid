//! Raw admission checks and a subprocess-only reference-input entry point.
use super::builtin_input_fixtures::{self as fixture, Observation};
use super::consumer_fixtures as f;
use super::enum_consumer_fixtures as e;
use super::*;

#[test]
fn builtin_input_raw_programs_require_the_complete_verifier_and_plan() {
    for (capacity, observation) in fixture::CAPACITIES
        .into_iter()
        .map(|capacity| (capacity, Observation::Status))
        .chain([(3, Observation::Checksum)])
    {
        let (sources, raw, entry) = fixture::program(capacity, observation);
        assert_eq!(
            builtins::check(&raw).unwrap(),
            builtins::BuiltinIds {
                enumeration: Some(EnumId(0)),
                function: Some(hir::DefId(1)),
            }
        );
        let witness = verify_owned(raw, &sources).unwrap();
        assert_eq!(witness.builtin_function(), Some(hir::DefId(1)));
        let plan = plan::ExecutionPlan::build(&witness).unwrap();
        assert_eq!(plan.owner_width(entry, OwnerPlaceId(0)), capacity.max(1));
        assert_eq!(plan.owner_width(entry, OwnerPlaceId(1)), 2);
        assert_eq!(plan.owner_width(entry, OwnerPlaceId(2)), 2);
        assert_eq!(plan.function(entry).call(CallSiteId(0)).borrowed_count(), 1);
        assert_eq!(plan.input_scratch_range(entry), None);
        assert_eq!(plan.input_scratch_range(hir::DefId(1)), Some(8..1032));
    }
}

#[test]
fn builtin_input_scratch_is_exactly_1024_physical_bytes_without_logical_cells() {
    let (sources, raw, entry) = fixture::program(3, Observation::Status);
    let builtin = verify_owned(raw, &sources).unwrap();
    let (sources, raw, _) = fixture::ordinary_control(3, Observation::Status);
    let ordinary = verify_owned(raw, &sources).unwrap();
    let builtin_plan = plan::ExecutionPlan::build(&builtin).unwrap();
    let ordinary_plan = plan::ExecutionPlan::build(&ordinary).unwrap();
    assert_eq!(
        builtin_plan.function(entry).usage(),
        ordinary_plan.function(entry).usage()
    );
    let before = ordinary_plan.function(hir::DefId(1)).usage();
    let after = builtin_plan.function(hir::DefId(1)).usage();
    assert_eq!(before.owner_cells, 2);
    assert_eq!(before.payload_bytes, 8);
    assert_eq!(before.expanded_cells, 16);
    assert_eq!(before.activation_fuel_cells(), 14);
    let mut expected = before;
    expected.payload_bytes += 1024;
    expected.reference_bytes += 1024;
    expected.native_bytes += 1024;
    assert_eq!(after, expected);
    assert_eq!(
        after.activation_fuel_cells(),
        before.activation_fuel_cells()
    );
    assert_eq!(builtin.usage().owner_cells, ordinary.usage().owner_cells);
    assert_eq!(
        builtin.usage().owner_layout_bytes,
        ordinary.usage().owner_layout_bytes
    );
    assert_eq!(ordinary_plan.input_scratch_range(hir::DefId(1)), None);
}

#[test]
fn builtin_input_ordinary_control_exercises_dispatch_checksum_and_cleanup() {
    // No ReadStdin exists in these ordinary programs, so running them inside the
    // test process cannot consume or alter the test runner's standard input.
    for (capacity, observation, expected) in [
        (3, Observation::Status, fixture::FULL),
        (3, Observation::Checksum, 3 * fixture::SENTINEL),
        (0, Observation::Checksum, 0),
    ] {
        let (sources, raw, entry) = fixture::ordinary_control(capacity, observation);
        let witness = verify_owned(raw, &sources).unwrap();
        assert_eq!(
            execute::run(&witness, Some(entry)),
            Ok(Scalar::I32(expected))
        );
    }
}

#[test]
fn builtin_input_enum_only_claim_keeps_ordinary_enum_validation_and_no_scratch() {
    let (sources, s) = f::context();
    let (mut raw, schedule) = e::mixed_case(
        &[Some(hir::Ty::I32), None, None],
        &[0, 1, 2],
        0,
        hir::Ty::I32,
        s(0),
    );
    raw.builtins = BuiltinOrigins::ReadStatus;
    let witness = verify_owned(raw, &sources).unwrap();
    assert_eq!(witness.builtin_enumeration(), Some(EnumId(0)));
    assert_eq!(witness.builtin_function(), None);
    let plan = plan::ExecutionPlan::build(&witness).unwrap();
    assert_eq!(plan.input_scratch_range(schedule.entry), None);
    assert_eq!(plan.function(schedule.entry).usage().payload_bytes, 8);
    assert_eq!(
        execute::run(&witness, Some(schedule.entry)),
        Ok(schedule.result)
    );

    let (mut invalid, _) = e::mixed_case(
        &[Some(hir::Ty::I32), None, None],
        &[0, 1, 2],
        0,
        hir::Ty::I32,
        s(0),
    );
    invalid.builtins = BuiltinOrigins::ReadStatus;
    invalid.functions[0].matches[0].arms[2].variant = e::variant(1);
    // The canonical enum descriptor is intact; the ordinary match proof must
    // independently reject the duplicated arm.
    assert!(builtins::check(&invalid).is_ok());
    assert!(verify_owned(invalid, &sources).is_err());
}

#[test]
fn builtin_input_source_opcode_and_noncanonical_builtin_shapes_are_denied() {
    type Mutation = (&'static str, fn(&mut RawOwnedProgram));
    let mutations: &[Mutation] = &[
        ("input in source function", |raw| {
            let span = raw.functions[0].span;
            raw.functions[0].blocks[0].statements.insert(
                0,
                f::instruction(
                    OwnedInstruction::ReadStdin {
                        buffer: ReferenceParamId(0),
                        destination: OwnerPlaceId(1),
                    },
                    span,
                ),
            );
        }),
        ("input in unreachable source block", |raw| {
            let span = raw.functions[0].span;
            let block = BlockId(raw.functions[0].blocks.len());
            raw.functions[0].blocks.push(e::block(
                vec![f::instruction(
                    OwnedInstruction::ReadStdin {
                        buffer: ReferenceParamId(0),
                        destination: OwnerPlaceId(1),
                    },
                    span,
                )],
                OwnedTerminatorKind::Goto(block),
                span,
            ));
        }),
        ("missing builtin claim", |raw| {
            raw.builtins = BuiltinOrigins::None
        }),
        ("enum-only claim with input opcode", |raw| {
            raw.builtins = BuiltinOrigins::ReadStatus;
        }),
        ("shared builtin parameter", |raw| {
            raw.functions[1].references[0].kind = BorrowKind::Shared;
        }),
        ("wrong input reference", |raw| {
            raw.functions[1].blocks[0].statements[1].kind = OwnedInstruction::ReadStdin {
                buffer: ReferenceParamId(1),
                destination: OwnerPlaceId(0),
            };
        }),
        ("duplicate input operation", |raw| {
            let operation = raw.functions[1].blocks[0].statements[1].clone();
            raw.functions[1].blocks[0].statements.push(operation);
        }),
        ("wrong enum payload", |raw| {
            raw.enums[0].variants[0].payload =
                Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::Bool)));
        }),
    ];
    for &(label, mutate) in mutations {
        let (sources, mut raw, _) = fixture::program(3, Observation::Status);
        mutate(&mut raw);
        let failure = verify_owned(raw, &sources).expect_err(label);
        assert!(
            matches!(failure.kind, OwnedFailureKind::Malformed(_)),
            "{label}: {failure:?}"
        );
    }
}

#[test]
fn builtin_input_call_still_requires_exclusive_complete_available_array() {
    type Mutation = (&'static str, fn(&mut RawOwnedFunction));
    let mutations: &[Mutation] = &[
        ("shared loan", |main| {
            main.loans[0].kind = BorrowKind::Shared
        }),
        ("immutable buffer", |main| {
            main.owners[0].kind = OwnerKind::Local { mutable: false };
        }),
        ("wrong slice element type", |main| {
            main.loans[0].referent =
                BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::Bool)).unwrap();
        }),
        ("missing borrow preparation", |main| {
            main.blocks[0].statements.pop();
        }),
        ("wrong call target", |main| {
            main.calls[0].target = hir::DefId(0)
        }),
        ("uninitialized buffer", |main| {
            main.blocks[0]
                .statements
                .retain(|row| !matches!(row.kind, OwnedInstruction::ConstructArray { .. }));
        }),
        ("invalid projection", |main| {
            main.loans[0].projection = vec![FieldId {
                record: RecordId(0),
                index: 0,
            }];
        }),
    ];
    for &(label, mutate) in mutations {
        let (sources, mut raw, _) = fixture::program(3, Observation::Status);
        mutate(&mut raw.functions[0]);
        // The trailing builtin remains canonical; descriptor acceptance alone
        // cannot excuse malformed source call/borrow/ownership state.
        assert!(builtins::check(&raw).is_ok(), "{label}");
        assert!(verify_owned(raw, &sources).is_err(), "{label}");
    }
}

#[test]
fn builtin_input_raw_activation_does_not_admit_std_source_imports() {
    use crate::frontend::{lexer, parser};
    for item in ["read_stdin", "ReadStatus"] {
        let mut sources = SourceMap::new();
        let file = sources.add(
            "unavailable-std-import.ox".into(),
            format!("use std::io::{item}; fn main()->i32{{return 0;}}"),
        );
        let source = sources.get(file);
        let errors = parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::ProjectCandidate,
        )
        .unwrap_err();
        assert!(errors.iter().all(|error| error.code != "E0500"));
        assert!(errors.iter().any(|error| error.stage == "parse"));
    }
}

/// Launch this exact test in a fresh process with --exact --nocapture and
/// --test-threads=1. The parent owns stdin; this child never changes fd flags.
/// Without the selector it is inert during the ordinary test suite.
#[test]
fn builtin_input_subprocess_child() {
    let Ok(capacity) = std::env::var("OXID_RAW_STDIN_CAPACITY") else {
        return;
    };
    let capacity: usize = capacity.parse().expect("known fixture capacity");
    assert!(fixture::CAPACITIES.contains(&capacity));
    let observation = match std::env::var("OXID_RAW_STDIN_MODE").as_deref() {
        Ok("status") => Observation::Status,
        Ok("checksum") => Observation::Checksum,
        _ => panic!("unknown raw stdin fixture observation"),
    };
    let fuel = match std::env::var("OXID_RAW_STDIN_FUEL") {
        Ok(value) => {
            let fuel = value.parse::<usize>().expect("bounded input fixture fuel");
            assert!(fuel <= plan::MAX_FUEL);
            fuel
        }
        Err(std::env::VarError::NotPresent) => plan::MAX_FUEL,
        Err(error) => panic!("invalid input fixture fuel: {error}"),
    };
    let (sources, raw, entry) = fixture::program(capacity, observation);
    let witness = verify_owned(raw, &sources).unwrap();
    if let Some(directory) = std::env::var_os("OXID_RAW_STDIN_NATIVE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        assert!(directory.is_dir(), "parent must reserve artifact directory");
        let module = native::native_module_with_fuel(&witness, entry, &sources, fuel).unwrap();
        drop(witness);
        std::fs::write(directory.join("program.ll"), &module).unwrap();
        let output = directory.join("program");
        crate::frontend::native::compile(&module, output.to_str().unwrap()).unwrap();
        println!("OXID_RAW_STDIN_NATIVE_READY=1");
        return;
    }
    let observed = execute::run_array_observed(
        &witness,
        Some(entry),
        execute::Limits {
            fuel,
            ..execute::Limits::default()
        },
        execute::ObservationControl::default(),
    );
    assert!(!observed.truncated, "input observation must be complete");
    println!("OXID_RAW_STDIN_REMAINING_FUEL={}", observed.remaining_fuel);
    if let Some(snapshot) = observed.storage.iter().rev().find(|snapshot| {
        snapshot.kind == execute::StorageObservationKind::Failure
            && snapshot.key.frame == 0
            && snapshot.key.owner == 0
    }) {
        print!("OXID_RAW_STDIN_BUFFER=");
        for byte in &snapshot.bytes {
            print!("{byte:02x}");
        }
        println!();
    }
    match observed.result {
        Ok(Scalar::I32(value)) => println!("OXID_RAW_STDIN_RESULT={value}"),
        Err(error @ execute::OwnedRunFailure::Scalar(RunFailure::Fuel(_))) => {
            println!("OXID_RAW_STDIN_FAILURE=fuel");
            eprint!("{}", error.diagnostic(&sources).render_human(&sources));
        }
        other => panic!("unexpected raw stdin outcome: {other:?}"),
    }
}
