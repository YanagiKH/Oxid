use super::super::consumer_fixtures as fixtures;
use super::*;
use crate::frontend::oir::owned::source::resource_fixtures as source;
use plan::native_storage::inventory_tests::{item_boundary, width_boundary};

fn emit_source(text: &str, policy: NativeEntryPolicy, limits: Limits) -> NativeObservation {
    let case = source::checked(text);
    let mut accounting = Accounting::default();
    let result = native_module_policy_accounted(
        &case.witness,
        Some(case.entry),
        &case.sources,
        plan::MAX_FUEL,
        limits,
        policy,
        &mut accounting,
    );
    NativeObservation {
        result,
        metrics: accounting.metrics,
    }
}

fn functions(count: usize) -> String {
    let mut text = String::from("fn main()->i32{return 7;}");
    for index in 1..count {
        text.push_str(&format!("fn f{index}()->i32{{return 0;}}"));
    }
    text
}

fn assert_denial(result: Result<String, Box<Diagnostic>>, name: &str, cap: usize) {
    let error = result.unwrap_err();
    assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
    assert_eq!(
        error.message,
        format!("native owned {name} limit exceeded ({cap})")
    );
}

#[test]
fn hosted_size_caps_keep_all_other_budgets_and_clamp_requests() {
    for policy in [
        NativeEntryPolicy::Result,
        NativeEntryPolicy::TypedResult,
        NativeEntryPolicy::Process,
        NativeEntryPolicy::HostedResult,
        NativeEntryPolicy::HostedTypedResult,
        NativeEntryPolicy::HostedProcess,
    ] {
        let limits = Limits::for_policy(policy);
        let d = Limits::DEFAULT;
        assert_eq!(
            (
                limits.parameters,
                limits.function_slots,
                limits.depth,
                limits.cost,
                limits.bytes,
                limits.live_bytes,
                limits.diagnostic_bytes,
                limits.metadata_bytes,
                limits.ir_bytes
            ),
            (
                d.parameters,
                d.function_slots,
                d.depth,
                d.cost,
                d.bytes,
                d.live_bytes,
                d.diagnostic_bytes,
                d.metadata_bytes,
                d.ir_bytes
            )
        );
        let expected = if policy.hosted() {
            (1024, 32768, 16384, 65536, 65536)
        } else {
            (256, 8192, 4096, 8192, 8192)
        };
        assert_eq!(
            (
                limits.functions,
                limits.scalar_slots,
                limits.blocks,
                limits.inventory_items,
                limits.owner_width
            ),
            expected
        );
        let requested = Limits {
            functions: usize::MAX,
            scalar_slots: usize::MAX,
            blocks: usize::MAX,
            inventory_items: usize::MAX,
            owner_width: usize::MAX,
            ..limits
        }
        .bounded_for(policy);
        assert_eq!(
            (
                requested.functions,
                requested.scalar_slots,
                requested.blocks,
                requested.inventory_items,
                requested.owner_width
            ),
            expected
        );
        let small = Limits {
            inventory_items: 3,
            owner_width: 2,
            ..limits
        }
        .bounded_for(policy);
        assert_eq!((small.inventory_items, small.owner_width), (3, 2));
    }
}

#[test]
fn hosted_and_bounded_function_boundaries_include_unused_verified_functions() {
    for policy in [
        NativeEntryPolicy::TypedResult,
        NativeEntryPolicy::HostedTypedResult,
    ] {
        let limits = Limits::for_policy(policy);
        let accepted = emit_source(&functions(limits.functions), policy, limits);
        assert!(accepted.result.is_ok(), "{:?}", accepted.result);
        assert_eq!(accepted.metrics.count_bytes, accepted.metrics.render_bytes);
        assert!(accepted.metrics.metadata_peak <= limits.metadata_bytes);
        let denied = emit_source(&functions(limits.functions + 1), policy, limits);
        assert_denial(denied.result, "function count", limits.functions);
        assert_eq!(
            (denied.metrics.count_bytes, denied.metrics.render_bytes),
            (0, 0)
        );
    }
}

#[test]
fn hosted_admits_the_first_inventory_successor_with_full_storage_and_emission() {
    let (sources, raw) = item_boundary(true, false, false);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let execution = ExecutionPlan::build(&witness).unwrap();
    let census = plan::native_storage::NativeInventories::checked(&execution).unwrap();
    assert_eq!(census.items(), 8193);
    for policy in [NativeEntryPolicy::Result, NativeEntryPolicy::HostedResult] {
        let mut accounting = Accounting::default();
        let result = native_module_policy_accounted(
            &witness,
            Some(hir::DefId(0)),
            &sources,
            plan::MAX_FUEL,
            Limits::for_policy(policy),
            policy,
            &mut accounting,
        );
        if policy.hosted() {
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(
                accounting.metrics.count_bytes,
                accounting.metrics.render_bytes
            );
            assert!(accounting.metrics.metadata_peak <= Limits::DEFAULT.metadata_bytes);
        } else {
            assert_denial(result, "aggregate compiler inventory items", 8192);
        }
    }
}

#[test]
fn hosted_owner_width_successor_keeps_native_byte_limits() {
    let (sources, raw) = width_boundary(true);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    assert_eq!(
        plan::native_storage::NativeInventories::checked(&plan)
            .unwrap()
            .owner_width(),
        8193
    );
    let limits = Limits::for_policy(NativeEntryPolicy::HostedResult);
    let error = admit_policy_accounted(
        &plan,
        Limits {
            owner_width: 8192,
            ..limits
        },
        NativeEntryPolicy::HostedResult,
        &mut Accounting::default(),
    )
    .unwrap_err();
    assert!(error
        .message
        .contains("owner width cells limit exceeded (8192)"));
    admit_policy_accounted(
        &plan,
        limits,
        NativeEntryPolicy::HostedResult,
        &mut Accounting::default(),
    )
    .unwrap();
    let error = admit_policy_accounted(
        &plan,
        Limits { bytes: 1, ..limits },
        NativeEntryPolicy::HostedResult,
        &mut Accounting::default(),
    )
    .unwrap_err();
    assert!(error
        .message
        .contains("aggregate storage bytes limit exceeded (1)"));
}

#[test]
fn hosted_size_selection_still_enforces_metadata_and_output_limits() {
    let policy = NativeEntryPolicy::HostedTypedResult;
    let limits = Limits::for_policy(policy);
    for (restricted, marker) in [
        (
            Limits {
                metadata_bytes: 0,
                ..limits
            },
            "metadata bytes",
        ),
        (
            Limits {
                ir_bytes: 0,
                ..limits
            },
            "LLVM bytes",
        ),
        (
            Limits {
                function_slots: 0,
                ..limits
            },
            "scalar slots per function",
        ),
        (
            Limits {
                scalar_slots: 0,
                ..limits
            },
            "aggregate scalar slots",
        ),
        (
            Limits {
                blocks: 0,
                ..limits
            },
            "aggregate blocks",
        ),
    ] {
        let observed = emit_source("fn main()->i32{return 0;}", policy, restricted);
        let error = observed.result.unwrap_err();
        assert_eq!(error.code, "E0700");
        assert!(error.message.contains(marker), "{}", error.message);
        assert_eq!(observed.metrics.render_bytes, 0);
    }
}

fn append_owner(function: &mut RawOwnedFunction, aggregate: AggregateTy) {
    let span = function.span;
    let owner = OwnerPlaceId(function.owners.len());
    function.owners.push(OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
        kind: OwnerKind::Local { mutable: false },
        span,
    });
    let construct = match aggregate {
        AggregateTy::Record(_) => OwnedInstruction::Construct {
            destination: owner,
            fields: vec![],
        },
        AggregateTy::FixedArray(array) => OwnedInstruction::ConstructArray {
            destination: owner,
            elements: vec![fixtures::operand(0, span); array.length()],
        },
        _ => unreachable!(),
    };
    function.blocks[0].statements.extend([
        fixtures::instruction(OwnedInstruction::StorageLive(owner), span),
        fixtures::instruction(construct, span),
        fixtures::instruction(OwnedInstruction::Discard(owner), span),
        fixtures::instruction(OwnedInstruction::StorageEnd(owner), span),
    ]);
}

fn admit_program(sources: &SourceMap, raw: RawOwnedProgram) -> Result<Vec<Bound>, Box<Diagnostic>> {
    let witness = verified::verify_owned(raw, sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    admit_policy_accounted(
        &plan,
        Limits::for_policy(NativeEntryPolicy::HostedResult),
        NativeEntryPolicy::HostedResult,
        &mut Accounting::default(),
    )
}

fn scalar_function(template: &RawOwnedFunction, id: usize, count: usize) -> RawOwnedFunction {
    let mut function = template.clone();
    function.id = hir::DefId(id);
    function.locals = vec![fixtures::scalar(hir::Ty::I32, function.span); count];
    function.owners.clear();
    function.blocks[0].statements = (0..count)
        .map(|i| fixtures::assign(i, Rvalue::I32(0), function.span))
        .collect();
    function
}

#[test]
fn hosted_inventory_65536_and_scalar_slots_32768_are_inclusive() {
    for extra in [false, true] {
        let (sources, mut raw) = item_boundary(false, false, false);
        let template = raw.functions[0].clone();
        raw.functions = (0..512)
            .map(|id| {
                let mut function = scalar_function(&template, id, 64);
                let referent =
                    BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                        .unwrap();
                function.parameters = (0..64)
                    .map(|position| ParameterBinding::Reference(ReferenceParamId(position)))
                    .collect();
                function.references = (0..64)
                    .map(|position| ReferenceDecl {
                        referent,
                        kind: BorrowKind::Shared,
                        position,
                        span: function.span,
                    })
                    .collect();
                function
            })
            .collect();
        if extra {
            let span = template.span;
            let mut function =
                fixtures::function(512, ValueTy::Owned(AggregateTy::Record(RecordId(0))), span);
            function.parameters = vec![ParameterBinding::Owned(OwnerPlaceId(0))];
            function.owners = vec![fixtures::owner(OwnerKind::Parameter { position: 0 }, span)];
            function.blocks = vec![OwnedBlock {
                merge: None,
                span,
                statements: vec![],
                terminator: fixtures::end(OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)), span),
            }];
            raw.functions.push(function);
        }
        // Admission-only raw boundary: I=512*(64 scalar+64 reference)+extra;
        // S=32768, W=extra, 512+extra functions/blocks. Entry validation is separate.
        let result = admit_program(&sources, raw);
        if extra {
            assert!(result
                .unwrap_err()
                .message
                .contains("compiler inventory items limit exceeded (65536)"));
        } else {
            result.unwrap();
        }

        let (sources, mut raw) = item_boundary(false, false, false);
        let template = raw.functions[0].clone();
        raw.functions = (0..128)
            .map(|id| scalar_function(&template, id, 256))
            .collect();
        if extra {
            raw.functions.push(scalar_function(&template, 128, 1));
        }
        let result = admit_program(&sources, raw);
        if extra {
            assert!(result
                .unwrap_err()
                .message
                .contains("aggregate scalar slots limit exceeded (32768)"));
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn hosted_owner_width_65536_is_inclusive_and_never_bypasses_storage_bytes() {
    for extra in [false, true] {
        let (sources, mut raw) = width_boundary(false);
        let function = &mut raw.functions[0];
        for _ in 8..64 {
            append_owner(
                function,
                AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, 1024).unwrap()),
            );
        }
        if extra {
            append_owner(
                function,
                AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, 0).unwrap()),
            );
        }
        let result = admit_program(&sources, raw);
        if extra {
            assert!(result
                .unwrap_err()
                .message
                .contains("owner width cells limit exceeded (65536)"));
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn hosted_block_count_16384_is_inclusive() {
    for extra in [false, true] {
        let (sources, mut raw) = item_boundary(false, false, false);
        let mut function = scalar_function(&raw.functions[0], 0, 1);
        let initial = function.blocks[0].statements.clone();
        let returned = function.blocks[0].terminator.clone();
        let count = 16384 + usize::from(extra);
        function.blocks = (0..count)
            .map(|id| OwnedBlock {
                merge: None,
                span: function.span,
                statements: if id == 0 { initial.clone() } else { vec![] },
                terminator: if id + 1 == count {
                    returned.clone()
                } else {
                    fixtures::end(OwnedTerminatorKind::Goto(BlockId(id + 1)), function.span)
                },
            })
            .collect();
        raw.functions = vec![function];
        let result = admit_program(&sources, raw);
        if extra {
            assert!(result
                .unwrap_err()
                .message
                .contains("aggregate blocks limit exceeded (16384)"));
        } else {
            result.unwrap();
        }
    }
}
