//! Ordinary descriptor/CFG/ownership proofs for the denied output precursor.
//! No test here manufactures an executable witness or performs output.
use super::builtin_output_fixtures as fixture;
use super::consumer_fixtures as f;
use super::enum_consumer_fixtures as e;
use super::*;
use crate::frontend::builtin_catalog::{BuiltinEnum, BuiltinFunction};

fn probe(raw: &RawOwnedProgram, sources: &SourceMap) -> Result<OwnershipUsage, OwnedFailure> {
    verified::probe_output_validation(raw, sources, budget::Limits::DEFAULT)
}

#[test]
fn builtin_output_candidate_checks_all_nine_independent_suffixes_without_admission() {
    use BuiltinOrigins::*;
    // These expectations do not call catalog rank/member/signature helpers.
    for (inventory, enums, functions, visits, output_denied) in [
        (
            None,
            [Option::None, Option::None],
            [Option::None, Option::None],
            0,
            false,
        ),
        (
            ReadStatus,
            [Some(1), Option::None],
            [Option::None, Option::None],
            5,
            false,
        ),
        (
            ReadStdin,
            [Some(1), Option::None],
            [Some(1), Option::None],
            12,
            false,
        ),
        (
            WriteStatus,
            [Option::None, Some(1)],
            [Option::None, Option::None],
            5,
            true,
        ),
        (
            WriteStdout,
            [Option::None, Some(1)],
            [Option::None, Some(1)],
            12,
            true,
        ),
        (
            ReadStatusWriteStatus,
            [Some(1), Some(2)],
            [Option::None, Option::None],
            9,
            true,
        ),
        (
            ReadStatusWriteStdout,
            [Some(1), Some(2)],
            [Option::None, Some(1)],
            16,
            true,
        ),
        (
            ReadStdinWriteStatus,
            [Some(1), Some(2)],
            [Some(1), Option::None],
            16,
            true,
        ),
        (
            ReadStdinWriteStdout,
            [Some(1), Some(2)],
            [Some(1), Some(2)],
            23,
            true,
        ),
    ] {
        let (sources, raw) = fixture::inventory(inventory);
        let ids = builtins::check_candidate(&raw).unwrap();
        assert_eq!(
            ids.enumeration(BuiltinEnum::ReadStatus),
            enums[0].map(EnumId)
        );
        assert_eq!(
            ids.enumeration(BuiltinEnum::WriteStatus),
            enums[1].map(EnumId)
        );
        assert_eq!(
            ids.function(BuiltinFunction::ReadStdin),
            functions[0].map(hir::DefId)
        );
        assert_eq!(
            ids.function(BuiltinFunction::WriteStdout),
            functions[1].map(hir::DefId)
        );
        assert_eq!(builtins::descriptor_visits(inventory), visits);
        probe(&raw, &sources).unwrap();
        assert_eq!(builtins::check(&raw).is_err(), output_denied);
        assert_eq!(verify_owned(raw, &sources).is_err(), output_denied);
    }
}

#[test]
fn builtin_output_descriptor_canonical_mutations_are_denied() {
    type Mutation = (&'static str, fn(&mut RawOwnedProgram));
    let mutations: &[Mutation] = &[
        ("enum ID", |raw| raw.enums[1].id = EnumId(0)),
        ("wrong variant enum", |raw| {
            raw.enums[1].variants[2].id.enumeration = EnumId(0)
        }),
        ("wrong variant ordinal", |raw| {
            raw.enums[1].variants[2].id.index = 0
        }),
        ("missing variant", |raw| {
            raw.enums[1].variants.pop();
        }),
        ("extra variant", |raw| {
            let variant = raw.enums[1].variants[0].clone();
            raw.enums[1].variants.push(variant);
        }),
        ("payload at input member", |raw| {
            raw.enums[1].variants[0].payload = raw.enums[1].variants[2].payload.take();
        }),
        ("payload bool", |raw| {
            raw.enums[1].variants[2].payload =
                Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::Bool)))
        }),
        ("enum member anchor", |raw| {
            raw.enums[1].variants[2].span = raw.enums[0].span
        }),
        ("function ID", |raw| raw.functions[1].id = hir::DefId(0)),
        ("wrong result enum", |raw| {
            raw.functions[1].result = ValueTy::Owned(AggregateTy::Enum(EnumId(0)))
        }),
        ("wrong owner enum", |raw| {
            raw.functions[1].owners[0].aggregate =
                AggregateSlot::try_from_aggregate(AggregateTy::Enum(EnumId(0))).unwrap()
        }),
        ("exclusive reference", |raw| {
            raw.functions[1].references[0].kind = BorrowKind::Exclusive
        }),
        ("wrong slice element", |raw| {
            raw.functions[1].references[0].referent =
                BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::Bool)).unwrap()
        }),
        ("wrong parameter ordinal", |raw| {
            raw.functions[1].references[0].position = 1
        }),
        ("wrong parameter binding", |raw| {
            raw.functions[1].parameters[0] = ParameterBinding::Reference(ReferenceParamId(1))
        }),
        ("extra reference", |raw| {
            let reference = raw.functions[1].references[0].clone();
            raw.functions[1].references.push(reference);
        }),
        ("local owner", |raw| {
            raw.functions[1].owners[0].kind = OwnerKind::Local { mutable: false }
        }),
        ("reference anchor", |raw| {
            raw.functions[1].references[0].span = raw.functions[0].span
        }),
        ("owner anchor", |raw| {
            raw.functions[1].owners[0].span = raw.functions[0].span
        }),
        ("wrong entry", |raw| raw.functions[1].entry = BlockId(1)),
        ("extra local", |raw| {
            let span = raw.functions[1].span;
            raw.functions[1].locals.push(f::scalar(hir::Ty::I32, span));
        }),
        ("block anchor", |raw| {
            raw.functions[1].blocks[0].span = raw.functions[0].span
        }),
        ("statement anchor", |raw| {
            raw.functions[1].blocks[0].statements[1].span = raw.functions[0].span
        }),
        ("statement diagnostic override", |raw| {
            let span = raw.functions[1].span;
            raw.functions[1].blocks[0].statements[1].diagnostic_origins = Some(DiagnosticOrigins {
                primary: span,
                cause: span,
            });
        }),
        ("extra operation", |raw| {
            let op = raw.functions[1].blocks[0].statements[1].clone();
            raw.functions[1].blocks[0].statements.push(op);
        }),
        ("missing live", |raw| {
            raw.functions[1].blocks[0].statements.remove(0);
        }),
        ("wrong reference", |raw| {
            raw.functions[1].blocks[0].statements[1].kind = OwnedInstruction::WriteStdout {
                buffer: ReferenceParamId(1),
                destination: OwnerPlaceId(0),
            }
        }),
        ("wrong destination", |raw| {
            raw.functions[1].blocks[0].statements[1].kind = OwnedInstruction::WriteStdout {
                buffer: ReferenceParamId(0),
                destination: OwnerPlaceId(1),
            }
        }),
        ("input operation", |raw| {
            raw.functions[1].blocks[0].statements[1].kind = OwnedInstruction::ReadStdin {
                buffer: ReferenceParamId(0),
                destination: OwnerPlaceId(0),
            }
        }),
        ("noncanonical return", |raw| {
            raw.functions[1].blocks[0].terminator.as_mut().unwrap().kind =
                OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(1))
        }),
        ("missing terminator", |raw| {
            raw.functions[1].blocks[0].terminator = None
        }),
        ("return anchor", |raw| {
            raw.functions[1].blocks[0].terminator.as_mut().unwrap().span = raw.functions[0].span
        }),
        ("return diagnostic override", |raw| {
            let span = raw.functions[1].span;
            raw.functions[1].blocks[0]
                .terminator
                .as_mut()
                .unwrap()
                .diagnostic_origins = Some(DiagnosticOrigins {
                primary: span,
                cause: span,
            });
        }),
        ("extra block", |raw| {
            let block = raw.functions[1].blocks[0].clone();
            raw.functions[1].blocks.push(block);
        }),
    ];
    for &(label, mutate) in mutations {
        let (sources, mut raw, _) = fixture::program(3);
        mutate(&mut raw);
        assert!(builtins::check_candidate(&raw).is_err(), "{label}");
        assert!(probe(&raw, &sources).is_err(), "{label}");
    }
}

#[test]
fn builtin_output_shape_confines_opcodes_even_in_unreachable_source_rows() {
    for unreachable in [false, true] {
        let (sources, mut raw, _) = fixture::program(3);
        let main = &mut raw.functions[0];
        let operation = f::instruction(
            OwnedInstruction::WriteStdout {
                buffer: ReferenceParamId(0),
                destination: OwnerPlaceId(1),
            },
            main.span,
        );
        if unreachable {
            let id = BlockId(main.blocks.len());
            main.blocks.push(e::block(
                vec![operation],
                OwnedTerminatorKind::Goto(id),
                main.span,
            ));
        } else {
            main.blocks[0].statements.insert(0, operation);
        }
        assert!(builtins::check_candidate(&raw).is_ok());
        assert!(matches!(
            probe(&raw, &sources).unwrap_err().kind,
            OwnedFailureKind::Malformed(Malformed::CanonicalSite)
        ));
    }
    for inventory in [BuiltinOrigins::None, BuiltinOrigins::WriteStatus] {
        let (sources, mut raw, _) = fixture::program(3);
        raw.builtins = inventory;
        assert!(builtins::check_candidate(&raw).is_ok());
        assert!(probe(&raw, &sources).is_err());
    }
}

#[test]
fn builtin_output_suffixes_cannot_swap_families_or_select_source_lookalikes() {
    for mutate in [
        (|raw: &mut RawOwnedProgram| {
            raw.enums.swap(1, 2);
        }) as fn(&mut RawOwnedProgram),
        |raw| {
            raw.functions.swap(1, 2);
        },
        |raw| {
            raw.functions[2].result = ValueTy::Owned(AggregateTy::Enum(EnumId(1)));
        },
        |raw| {
            raw.functions[2].owners[0].aggregate =
                AggregateSlot::try_from_aggregate(AggregateTy::Enum(EnumId(0))).unwrap();
        },
        |raw| {
            raw.enums.pop();
        },
        |raw| {
            raw.functions.pop();
        },
    ] {
        let (sources, mut raw) = fixture::inventory(BuiltinOrigins::ReadStdinWriteStdout);
        mutate(&mut raw);
        assert!(builtins::check_candidate(&raw).is_err());
        assert!(probe(&raw, &sources).is_err());
    }
}

#[test]
fn builtin_output_status_payload_and_matches_use_member_two() {
    let (sources, s) = f::context();
    for constructed in 0..3 {
        for order in [[0, 1, 2], [2, 0, 1], [1, 2, 0]] {
            let (mut raw, _) = e::mixed_case(
                &[None, None, Some(hir::Ty::I32)],
                &order,
                constructed,
                hir::Ty::I32,
                s(0),
            );
            raw.builtins = BuiltinOrigins::WriteStatus;
            builtins::check_candidate(&raw).unwrap();
            probe(&raw, &sources).unwrap();
            raw.functions[0].matches[0].arms[2].variant =
                raw.functions[0].matches[0].arms[0].variant;
            assert!(builtins::check_candidate(&raw).is_ok());
            assert!(probe(&raw, &sources).is_err());
        }
    }
}

#[test]
fn builtin_output_shared_call_still_requires_complete_available_view_and_cfg() {
    for capacity in [0, 1, 3, 1024] {
        let (sources, raw, _) = fixture::program(capacity);
        probe(&raw, &sources).unwrap();
    }
    // Returning ends the activation's owned storage. Explicit discard/end rows
    // are optional after the call has released its loan and closed its region.
    let (sources, mut implicit_cleanup, _) = fixture::program(3);
    implicit_cleanup.functions[0].blocks[1].statements.clear();
    probe(&implicit_cleanup, &sources).unwrap();
    type Mutation = (&'static str, fn(&mut RawOwnedFunction));
    let mutations: &[Mutation] = &[
        ("exclusive loan", |main| {
            main.loans[0].kind = BorrowKind::Exclusive
        }),
        ("wrong element type", |main| {
            main.loans[0].referent =
                BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::Bool)).unwrap()
        }),
        ("missing preparation", |main| {
            main.blocks[0].statements.pop();
        }),
        ("wrong call target", |main| {
            main.calls[0].target = hir::DefId(0)
        }),
        ("uninitialized owner", |main| {
            main.blocks[0]
                .statements
                .retain(|s| !matches!(s.kind, OwnedInstruction::ConstructArray { .. }))
        }),
        ("invalid projection", |main| {
            main.loans[0].projection = vec![FieldId {
                record: RecordId(0),
                index: 0,
            }]
        }),
        ("invalid continuation", |main| {
            main.blocks[0].terminator.as_mut().unwrap().kind = OwnedTerminatorKind::Invoke {
                call: CallSiteId(0),
                continuation: BlockId(2),
            }
        }),
        ("consume returned status twice", |main| {
            let discard = main.blocks[1].statements[0].clone();
            main.blocks[1].statements.insert(1, discard);
        }),
    ];
    for &(label, mutate) in mutations {
        let (sources, mut raw, _) = fixture::program(3);
        mutate(&mut raw.functions[0]);
        assert!(builtins::check_candidate(&raw).is_ok(), "{label}");
        let failure = probe(&raw, &sources).expect_err(label);
        if label == "consume returned status twice" {
            assert_eq!(
                failure.kind,
                OwnedFailureKind::Ownership(Violation::Unavailable)
            );
        }
    }
}

#[test]
fn builtin_output_projection_and_forwarding_retain_ordinary_view_proofs() {
    let (sources, mut projected, _) = fixture::projected_program();
    probe(&projected, &sources).unwrap();
    projected.functions[0].loans[0].projection[0].index = 0;
    assert!(builtins::check_candidate(&projected).is_ok());
    assert!(probe(&projected, &sources).is_err());

    let (sources, mut forwarded, _) = fixture::forwarded_program();
    probe(&forwarded, &sources).unwrap();
    // The ordinary forwarding function is outside the canonical suffix; its
    // parameter/loan mismatch must be denied by ordinary call-shape validation.
    forwarded.functions[1].references[0].kind = BorrowKind::Exclusive;
    assert!(builtins::check_candidate(&forwarded).is_ok());
    assert!(probe(&forwarded, &sources).is_err());
}

#[test]
fn builtin_output_staging_is_1024_physical_bytes_per_function_without_fuel_growth() {
    let (sources, raw, entry) = fixture::program(3);
    let candidate = probe(&raw, &sources).unwrap();
    let output_usage = plan::probe_builtin_frame_usage(&raw, &sources, hir::DefId(1)).unwrap();
    let caller_usage = plan::probe_builtin_frame_usage(&raw, &sources, entry).unwrap();
    let (sources, ordinary, _) = fixture::ordinary_control(3);
    let witness = verify_owned(ordinary, &sources).unwrap();
    let plan = plan::ExecutionPlan::build(&witness).unwrap();
    assert_eq!(caller_usage, plan.function(entry).usage());
    let mut expected = plan.function(hir::DefId(1)).usage();
    assert_eq!(expected.owner_cells, 2);
    assert_eq!(expected.payload_bytes, 8);
    assert_eq!(expected.expanded_cells, 16);
    assert_eq!(expected.activation_fuel_cells(), 14);
    expected.payload_bytes += 1024;
    expected.reference_bytes += 1024;
    expected.native_bytes += 1024;
    assert_eq!(output_usage, expected);
    assert_eq!(plan.input_scratch_range(hir::DefId(1)), None);
    assert_eq!(plan.output_scratch_range(hir::DefId(1)), None);
    assert_eq!(candidate.work, witness.usage().work + 12);
    assert_eq!(candidate.metadata_bytes, witness.usage().metadata_bytes);
    assert_eq!(
        candidate.owner_layout_bytes,
        witness.usage().owner_layout_bytes
    );
    assert_eq!(candidate.owner_cells, witness.usage().owner_cells);

    let (sources, both) = fixture::inventory(BuiltinOrigins::ReadStdinWriteStdout);
    for function in [hir::DefId(1), hir::DefId(2)] {
        let usage = plan::probe_builtin_frame_usage(&both, &sources, function).unwrap();
        assert_eq!(usage.payload_bytes, 1032);
        assert_eq!(usage.reference_bytes, 1144);
        assert_eq!(usage.native_bytes, 1044);
        assert_eq!(usage.activation_fuel_cells(), 14);
    }
    assert_eq!(
        plan::probe_builtin_frame_usage(&both, &sources, hir::DefId(0))
            .unwrap()
            .payload_bytes,
        0
    );
}

#[test]
fn builtin_output_candidate_resource_rejection_precedes_allocation() {
    let (sources, raw, _) = fixture::program(3);
    let required = budget::preflight(&raw, budget::Limits::DEFAULT).unwrap();
    verified::probe_output_validation(
        &raw,
        &sources,
        budget::Limits {
            work: required.work,
            scratch: required.scratch_bytes,
            metadata: required.metadata_bytes,
            ..budget::Limits::DEFAULT
        },
    )
    .unwrap();
    for (name, limits) in [
        (
            "ownership work",
            budget::Limits {
                work: required.work - 1,
                ..budget::Limits::DEFAULT
            },
        ),
        (
            "ownership scratch",
            budget::Limits {
                scratch: required.scratch_bytes - 1,
                ..budget::Limits::DEFAULT
            },
        ),
        (
            "ownership metadata",
            budget::Limits {
                metadata: required.metadata_bytes - 1,
                ..budget::Limits::DEFAULT
            },
        ),
    ] {
        let failure = budget::fail_allocation_after(0, || {
            verified::probe_output_validation(&raw, &sources, limits)
        })
        .unwrap_err();
        assert!(
            matches!(failure.kind, OwnedFailureKind::Resource(actual) if actual == name),
            "{name}: {failure:?}"
        );
    }
}
