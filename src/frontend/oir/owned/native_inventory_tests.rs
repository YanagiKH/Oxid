//! Phase 2 counting/policy controls only. Large fixtures are raw OIR, not
//! source-admission claims. They traverse the ordinary raw proof and plan; the
//! unchanged production consumer must reject them before counting/rendering IR.
use super::super::super::{
    consumer_fixtures as fixtures, enum_consumer_fixtures as enums, native, reviewer_origins,
    source::resource_fixtures as source,
};
use super::*;

const CAP: usize = 8192;

/// A second declaration census, independent of FrameUsage and native counters.
/// The order is the RFC's S, A, O, R, L, C, including non-scalar argument holes.
fn declaration_census(witness: &verified::VerifiedOwnedProgram) -> [usize; 6] {
    let mut counts = [0; 6];
    for f in witness.functions() {
        counts[0] += f.locals.len() + f.places.len();
        counts[1] += f.calls.iter().map(|c| c.arguments.len()).sum::<usize>();
        counts[2] += f.owners.len();
        counts[3] += f.references.len();
        counts[4] += f.loans.len();
        counts[5] += f.calls.len();
    }
    counts
}

fn block(statements: Vec<OwnedStatement>, span: Span) -> OwnedBlock {
    OwnedBlock {
        merge: None,
        span,
        statements,
        terminator: fixtures::end(
            OwnedTerminatorKind::ReturnScalar(fixtures::operand(0, span)),
            span,
        ),
    }
}

fn scalar_function(id: usize, locals: usize, span: Span) -> RawOwnedFunction {
    assert!(locals > 0);
    let mut f = fixtures::function(id, ValueTy::Scalar(hir::Ty::I32), span);
    f.locals = vec![fixtures::scalar(hir::Ty::I32, span); locals];
    f.blocks.push(block(
        (0..locals)
            .map(|i| fixtures::assign(i, Rvalue::I32(0), span))
            .collect(),
        span,
    ));
    f
}

fn append_owner(f: &mut RawOwnedFunction, length: Option<usize>, span: Span) {
    let owner = OwnerPlaceId(f.owners.len());
    let aggregate = match length {
        Some(n) => AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::I32, n).unwrap()),
        None => AggregateTy::Record(RecordId(0)),
    };
    f.owners.push(OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
        kind: OwnerKind::Local { mutable: false },
        span,
    });
    f.blocks[0].statements.extend([
        fixtures::instruction(OwnedInstruction::StorageLive(owner), span),
        fixtures::instruction(
            match length {
                Some(n) => OwnedInstruction::ConstructArray {
                    destination: owner,
                    elements: vec![fixtures::operand(0, span); n],
                },
                None => OwnedInstruction::Construct {
                    destination: owner,
                    fields: vec![],
                },
            },
            span,
        ),
        fixtures::instruction(OwnedInstruction::Discard(owner), span),
        fixtures::instruction(OwnedInstruction::StorageEnd(owner), span),
    ]);
}

/// 32 * (255 scalar declarations + one owner) = 8192 items. The optional
/// 33rd scalar-only function gives exactly one additional item without crossing
/// the independent 8192-scalar or 256-slot-per-function limits.
pub(in super::super::super) fn item_boundary(
    extra_item: bool,
    arrays: bool,
    extra_width: bool,
) -> (SourceMap, RawOwnedProgram) {
    let (sources, s) = fixtures::context();
    let mut functions = Vec::new();
    for id in 0..32 {
        let mut f = scalar_function(id, 255, s(id));
        append_owner(
            &mut f,
            arrays.then_some(256 + usize::from(extra_width && id == 0)),
            s(id),
        );
        functions.push(f);
    }
    if extra_item {
        functions.push(scalar_function(32, 1, s(32)));
    }
    (
        sources,
        RawOwnedProgram {
            builtins: BuiltinOrigins::None,
            enums: vec![],
            records: vec![fixtures::record(&[], s(0))],
            functions,
        },
    )
}

/// Eight independently bounded arrays contribute 8*1024 width cells. An
/// additional empty array contributes its existing one-cell sentinel, not zero.
pub(in super::super::super) fn width_boundary(extra: bool) -> (SourceMap, RawOwnedProgram) {
    let (sources, s) = fixtures::context();
    let mut f = scalar_function(0, 1, s(0));
    for _ in 0..8 {
        append_owner(&mut f, Some(1024), s(0));
    }
    if extra {
        append_owner(&mut f, Some(0), s(0));
    }
    (
        sources,
        RawOwnedProgram {
            builtins: BuiltinOrigins::None,
            enums: vec![],
            records: vec![],
            functions: vec![f],
        },
    )
}

fn assert_policy_denial(
    plan: &ExecutionPlan<'_>,
    caps: (usize, usize),
    name: &str,
    maximum: usize,
) {
    let error = native::admit_inventory_policy(plan, caps).err().unwrap();
    assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
    assert_eq!(
        error.message,
        format!("native owned {name} limit exceeded ({maximum})")
    );
    assert_eq!(error.primary, Some(plan.witness().functions()[0].span));
}

fn check_boundary(sources: SourceMap, raw: RawOwnedProgram, expected: [usize; 6], width: usize) {
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    assert_eq!(declaration_census(&witness), expected);
    let items = expected.iter().sum::<usize>();
    let inventory = NativeInventories::checked(&plan).unwrap();
    assert_eq!((inventory.items(), inventory.owner_width()), (items, width));
    assert!(witness.functions().len() <= 256);
    assert!(expected[0] <= CAP);
    assert!(witness.functions().iter().all(|f| {
        f.parameters.len() <= 64 && f.locals.len() + f.places.len() + f.owners.len() <= 256
    }));
    assert!(
        witness
            .functions()
            .iter()
            .map(|f| f.blocks.len())
            .sum::<usize>()
            <= 4096
    );
    let storage = NativeStoragePlan::checked(&plan, false).unwrap();
    let explicit_bytes = witness
        .functions()
        .iter()
        .map(|f| storage.function(f.id).native_bytes().unwrap())
        .sum::<usize>();
    // Every owner here is either an i32 array or an empty record in its own
    // function. Both therefore occupy four bytes per independently known W.
    assert_eq!(explicit_bytes, 8 * expected[0] + 4 * width);
    assert!(explicit_bytes + 8 <= 1024 * 1024);

    if items > CAP {
        assert_policy_denial(&plan, (CAP, CAP), "aggregate compiler inventory items", CAP);
    } else if width > CAP {
        assert_policy_denial(&plan, (CAP, CAP), "aggregate owner width cells", CAP);
    } else {
        let admitted = native::admit_inventory_policy(&plan, (CAP, CAP)).unwrap();
        assert_eq!((admitted.items(), admitted.owner_width()), (items, width));
    }
    // Requests above the real ceilings cannot raise either policy cap.
    if items > CAP {
        assert_policy_denial(
            &plan,
            (usize::MAX, usize::MAX),
            "aggregate compiler inventory items",
            CAP,
        );
    } else if width > CAP {
        assert_policy_denial(
            &plan,
            (usize::MAX, usize::MAX),
            "aggregate owner width cells",
            CAP,
        );
    }

    // These controls intentionally exceed old X. No source, execution, or LLVM
    // authority is obtained through the proposed policy helper.
    let old_x = expected[0]
        + expected[1]
        + width
        + 4 * expected[2]
        + 10 * expected[3]
        + 14 * expected[4]
        + 2 * expected[5];
    assert!(old_x > CAP);
    let observed = native::run_array_observed(
        &witness,
        Some(hir::DefId(0)),
        &sources,
        native::NativeControl::default(),
    );
    let error = observed.result.err().unwrap();
    assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
    assert_eq!(
        error.message,
        "native owned aggregate expanded cells limit exceeded (8192)"
    );
    assert_eq!(
        (observed.metrics.count_bytes, observed.metrics.render_bytes),
        (0, 0)
    );
    assert_eq!(observed.metrics.allocation_attempts, 0);
}

#[test]
fn native_inventory_raw_exact_and_one_over_independent_caps() {
    for extra in [false, true] {
        let (sources, raw) = item_boundary(extra, false, false);
        check_boundary(
            sources,
            raw,
            [8160 + usize::from(extra), 0, 32, 0, 0, 0],
            32,
        );
        let (sources, raw) = width_boundary(extra);
        check_boundary(
            sources,
            raw,
            [1, 0, 8 + usize::from(extra), 0, 0, 0],
            CAP + usize::from(extra),
        );
    }
}

#[test]
fn native_inventory_raw_both_exact_and_both_over_keep_i_before_w() {
    for extra in [false, true] {
        let (sources, raw) = item_boundary(extra, true, extra);
        check_boundary(
            sources,
            raw,
            [8160 + usize::from(extra), 0, 32, 0, 0, 0],
            CAP + usize::from(extra),
        );
    }
}

#[test]
fn native_inventory_raw_argument_holes_and_owner_classes_are_items() {
    for (build, counts, width) in [
        (fixtures::empty_record as fn() -> _, [1, 0, 1, 0, 0, 0], 1),
        (fixtures::owned_relay, [2, 1, 4, 0, 0, 1], 4),
        (fixtures::shared_read, [3, 1, 1, 1, 1, 1], 1),
    ] {
        let (sources, raw, _) = build();
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let plan = ExecutionPlan::build(&witness).unwrap();
        assert_eq!(declaration_census(&witness), counts);
        let inventory = NativeInventories::checked(&plan).unwrap();
        assert_eq!(inventory.items(), counts.iter().sum());
        assert_eq!(inventory.owner_width(), width);
        assert_policy_denial(
            &plan,
            (inventory.items() - 1, usize::MAX),
            "aggregate compiler inventory items",
            inventory.items() - 1,
        );
        assert_policy_denial(
            &plan,
            (usize::MAX, width - 1),
            "aggregate owner width cells",
            width - 1,
        );
        native::admit_inventory_policy(&plan, (inventory.items(), width)).unwrap();
    }
}

#[test]
fn native_inventory_source_unused_untaken_and_zero_argument_calls_count() {
    // Real in-memory record-source parsing, resolution, typing and lowering,
    // followed by ordinary raw verification. No filesystem Project loader and
    // no claims about unrelated source AST boundary capacity are involved.
    for (text, counts, width) in [
        (
            "struct Empty{} fn main()->(){return;}",
            [1, 0, 0, 0, 0, 0],
            0,
        ),
        (
            "struct Empty{} fn unused()->(){let a=Empty{};return;} fn main()->(){return;}",
            [2, 0, 2, 0, 0, 0],
            2,
        ),
        (
            "struct Empty{} fn main()->(){if false{let a=Empty{};}return;}",
            [2, 0, 2, 0, 0, 0],
            2,
        ),
        (
            "struct Empty{} fn zero()->(){return;} fn main()->(){zero();return;}",
            [3, 0, 0, 0, 0, 1],
            0,
        ),
        (
            "struct Empty{} fn main()->i32{let mut n=1;n=2;return n;}",
            [4, 0, 0, 0, 0, 0],
            0,
        ),
    ] {
        let case = source::checked(text);
        let plan = ExecutionPlan::build(&case.witness).unwrap();
        assert_eq!(declaration_census(&case.witness), counts, "{text}");
        let inventory = NativeInventories::checked(&plan).unwrap();
        assert_eq!(
            (inventory.items(), inventory.owner_width()),
            (counts.iter().sum(), width),
            "{text}"
        );
    }
}

#[test]
fn native_inventory_source_unused_and_untaken_borrow_calls_count() {
    // read: S1,R1; caller: S3,O2,A1,L1,C1; unused case main: S1.
    // In the untaken case main is the caller and its false literal replaces
    // the otherwise separate main's return slot. Both totals are written from
    // the source templates, not sampled from the execution/inventory counters.
    for (text, unused) in [
        (
            "struct T{value:i32} fn read(p:&T)->i32{return p.value;} fn unused()->(){let x=T{value:7};read(&x);return;} fn main()->(){return;}",
            true,
        ),
        (
            "struct T{value:i32} fn read(p:&T)->i32{return p.value;} fn main()->(){if false{let x=T{value:7};read(&x);}return;}",
            false,
        ),
    ] {
        let case = source::checked(text);
        assert_eq!(declaration_census(&case.witness), [5, 1, 2, 1, 1, 1]);
        let borrower = &case.witness.functions()[1];
        assert_eq!((borrower.calls.len(), borrower.loans.len()), (1, 1));
        assert_eq!(borrower.calls[0].arguments, [ArgumentSlot::Borrow(LoanId(0))]);
        assert_eq!(case.witness.functions()[0].references.len(), 1);
        if unused {
            assert_ne!(borrower.id, case.entry);
            assert!(case.witness.functions()[case.entry.0].calls.is_empty());
        } else {
            assert_eq!(borrower.id, case.entry);
            let entry = &borrower.blocks[borrower.entry.0];
            let OwnedTerminatorKind::Branch { condition, then_block, else_block } =
                &entry.terminator.as_ref().unwrap().kind else { panic!("source if branch") };
            assert!(entry.statements.iter().any(|statement| matches!(
                &statement.kind,
                OwnedInstruction::Scalar(Statement::Assign(Assign {
                    destination, value: Rvalue::Bool(false), ..
                })) if *destination == condition.local
            )));
            assert!(matches!(
                borrower.blocks[then_block.0].terminator.as_ref().unwrap().kind,
                OwnedTerminatorKind::Invoke { call: CallSiteId(0), .. }
            ));
            assert!(!matches!(
                borrower.blocks[else_block.0].terminator.as_ref().unwrap().kind,
                OwnedTerminatorKind::Invoke { .. }
            ));
        }
        let plan = ExecutionPlan::build(&case.witness).unwrap();
        let inventory = NativeInventories::checked(&plan).unwrap();
        assert_eq!((inventory.items(), inventory.owner_width()), (11, 2));
    }
}

#[test]
fn native_inventory_raw_empty_and_enum_widths_keep_existing_sentinels() {
    let (sources, s) = fixtures::context();
    let mut f = scalar_function(0, 1, s(0));
    append_owner(&mut f, None, s(0));
    append_owner(&mut f, Some(0), s(0));
    append_owner(&mut f, Some(3), s(0));
    // A valid uninitialized raw lifetime contributes its declared width too;
    // no source-level uninitialized declaration feature is implied.
    f.owners
        .push(enums::owner(OwnerKind::Local { mutable: false }, s(0)));
    f.blocks[0].statements.extend([
        fixtures::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(3)), s(0)),
        fixtures::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(3)), s(0)),
    ]);
    let raw = RawOwnedProgram {
        builtins: BuiltinOrigins::None,
        enums: vec![enums::enumeration(&[None], s(0))],
        records: vec![fixtures::record(&[], s(0))],
        functions: vec![f],
    };
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let inventory = NativeInventories::checked(&plan).unwrap();
    assert_eq!(declaration_census(&witness), [1, 0, 4, 0, 0, 0]);
    assert_eq!(
        (inventory.items(), inventory.owner_width()),
        (5, 1 + 1 + 3 + 2)
    );
}

#[test]
fn native_inventory_corrupt_totals_and_plan_fields_are_rejected() {
    let (sources, raw, _) = fixtures::owned_relay();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    for (items, owner_width) in [
        (7, 4),
        (9, 4),
        (8, 3),
        (8, 5),
        (usize::MAX, 4),
        (8, usize::MAX),
    ] {
        assert!(NativeInventories { items, owner_width }
            .check(&plan)
            .is_err());
    }
    let mut missing_function = ExecutionPlan::build(&witness).unwrap();
    missing_function.functions.pop();
    assert!(NativeInventories::checked(&missing_function).is_err());
    type Mutation = fn(&mut ExecutionPlan<'_>);
    for mutate in [
        (|p| p.functions[0].usage.scalar_slots = usize::MAX) as Mutation,
        |p| p.functions[0].usage.arguments = usize::MAX,
        |p| p.functions[0].usage.owners = usize::MAX,
        |p| p.functions[0].usage.owner_cells = usize::MAX,
        |p| p.functions[0].usage.references = usize::MAX,
        |p| p.functions[0].usage.loans = usize::MAX,
        |p| p.functions[0].usage.calls = usize::MAX,
    ] {
        let mut plan = ExecutionPlan::build(&witness).unwrap();
        mutate(&mut plan);
        assert_eq!(
            NativeInventories::checked(&plan).err().unwrap(),
            AdmissionFailure::new("owned count overflow", None)
        );
    }
}

#[test]
fn native_inventory_checked_counters_use_no_new_reservation() {
    let (sources, raw, _) = fixtures::shared_read();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let (result, attempts) = reviewer_origins::integration_counted(|| {
        fail_allocation_after(0, || NativeInventories::checked(&plan))
    });
    let inventory = result.unwrap();
    assert_eq!(attempts, 0);
    let (result, attempts) = reviewer_origins::integration_counted(|| inventory.check(&plan));
    result.unwrap();
    assert_eq!(attempts, 0);
    assert_eq!(size_of::<NativeInventories>(), 2 * size_of::<usize>());
    assert!(!std::mem::needs_drop::<NativeInventories>());
}

/// Named fixed carriers at the count/check/policy seam, excluding the admission
/// caller's existing locals. The sibling admission controls add the real caller
/// types at the actual insertion point. Neither side changes the Vec-capacity
/// heap ledger or claims to inventory all machine stack, spills, frames or RSS.
pub(in super::super::super) fn inventory_fixed_counter_phases(
    execution: &ExecutionPlan<'_>,
) -> [(&'static str, usize); 5] {
    let raw = &execution.witness().functions()[0];
    let owner = &raw.owners[0];
    let width_result = execution
        .witness()
        .declarations()
        .aggregate_width(owner.aggregate());
    let checked_result = NativeInventories::checked(execution);
    let mapped_result = native::admit_inventory_policy(execution, (CAP, CAP));
    // Include the produced value and both actual return transports together
    // conservatively, including the caller-retained value through its guards.
    // Thirty-two words cover bounded scalar/reference temporaries. Array and
    // owning-iterator storage are both charged even when the array is moved.
    let handoff = size_of::<NativeInventories>()
        + std::mem::size_of_val(&checked_result)
        + std::mem::size_of_val(&mapped_result)
        + size_of::<&ExecutionPlan<'_>>()
        + size_of::<(usize, usize)>()
        + size_of::<[usize; 32]>();
    let arithmetic = size_of::<Result<usize, AdmissionFailure>>();
    let checker_result = size_of::<Result<(), AdmissionFailure>>();
    let six_counts = size_of::<[usize; 6]>() + size_of::<std::array::IntoIter<usize, 6>>();
    let raw_scan = std::mem::size_of_val(&execution.witness().functions().iter().enumerate())
        + size_of::<&RawOwnedFunction>()
        + size_of::<&NativeInventories>()
        + checker_result;
    [
        (
            "cached usage census",
            handoff
                + std::mem::size_of_val(&execution.functions().iter())
                + size_of::<&FunctionPlan>()
                + size_of::<FrameUsage>()
                + six_counts
                + arithmetic,
        ),
        (
            "raw declaration census",
            handoff + raw_scan + six_counts + arithmetic,
        ),
        (
            "raw argument census",
            handoff
                + raw_scan
                + std::mem::size_of_val(&raw.calls.iter())
                + size_of::<&CallDecl>()
                + arithmetic,
        ),
        (
            "raw owner-width census",
            handoff
                + raw_scan
                + std::mem::size_of_val(&raw.owners.iter())
                + size_of::<&OwnerDecl>()
                + std::mem::size_of_val(&width_result)
                + arithmetic,
        ),
        (
            "policy limit guards",
            handoff
                + size_of::<Result<(), Box<Diagnostic>>>()
                + std::mem::size_of_val(&execution.witness().functions().first())
                + size_of::<Span>(),
        ),
    ]
}

#[test]
fn native_inventory_named_fixed_roles_fit_existing_transient_partition() {
    let (sources, raw, _) = fixtures::owned_relay();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    println!(
        "native inventory fixed values: inventory={} checked_result={} mapped_result={} checker_result={} usage={} six_counts={} count_iterator={} plan_iterator={} raw_iterator={} call_iterator={} owner_iterator={}",
        size_of::<NativeInventories>(),
        size_of::<Result<NativeInventories, AdmissionFailure>>(),
        size_of::<Result<NativeInventories, Box<Diagnostic>>>(),
        size_of::<Result<(), AdmissionFailure>>(),
        size_of::<FrameUsage>(),
        size_of::<[usize; 6]>(),
        size_of::<std::array::IntoIter<usize, 6>>(),
        std::mem::size_of_val(&plan.functions().iter()),
        std::mem::size_of_val(&witness.functions().iter().enumerate()),
        std::mem::size_of_val(&witness.functions()[0].calls.iter()),
        std::mem::size_of_val(&witness.functions()[0].owners.iter()),
    );
    for (phase, bytes) in inventory_fixed_counter_phases(&plan) {
        println!(
            "native inventory fixed {phase}: counters={bytes} allowance={FIXED_CARRIER_ALLOWANCE}"
        );
        assert!(bytes <= FIXED_CARRIER_ALLOWANCE, "{phase}");
    }
    // Usage loops and raw loops are sequential. These facts are dropped after
    // the policy guards, so graph, diagnostic and emission phases do not inherit
    // this new inventory's lifetime or an additional allocation-ledger charge.
}
