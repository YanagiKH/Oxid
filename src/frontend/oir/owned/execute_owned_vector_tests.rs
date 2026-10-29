use super::*;
use crate::frontend::builtin_catalog::{vector::Item, BuiltinItem};
use crate::frontend::oir::owned::owned_vector_descriptors::{self, OwnedVectorImportClaims};
use crate::frontend::oir::owned::verified::VerifiedOwnedVectorProgram;
use crate::frontend::project::budget::Allocator;

#[test]
fn owned_vector_vm_same_executor_runs_all_eight_and_exact_whole_program_fuel() {
    let (_, witness, _, expected) =
        owned_vector_descriptors::tests::execution_fixture::all_operations();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let mut remaining = 0;
    let result = execute_plan_policy::<Scalar, _>(
        &plan,
        hir::DefId(0),
        Limits::default(),
        EntryPolicy::Result,
        ExecutionPolicy::Bounded,
        None,
        None,
        Some(&mut remaining),
    );
    assert_eq!(result, Ok(Scalar::I32(expected)));
    let exact = Limits::default().fuel - remaining;
    for (fuel, succeeds) in [(exact, true), (exact - 1, false)] {
        let result = execute_plan_policy::<Scalar, _>(
            &plan,
            hir::DefId(0),
            Limits {
                fuel,
                ..Limits::default()
            },
            EntryPolicy::Result,
            ExecutionPolicy::Bounded,
            None,
            None,
            None,
        );
        if succeeds {
            assert_eq!(result, Ok(Scalar::I32(expected)));
        } else {
            assert!(matches!(
                result,
                Err(OwnedRunFailure::Scalar(RunFailure::Fuel(_)))
            ));
        }
    }
    remaining = 0;
    let result = execute_plan_policy::<Scalar, _>(
        &plan,
        hir::DefId(0),
        Limits {
            fuel: 0,
            ..Limits::default()
        },
        EntryPolicy::Result,
        ExecutionPolicy::Hosted,
        None,
        None,
        Some(&mut remaining),
    );
    assert_eq!(result, Ok(Scalar::I32(expected)));
    assert_eq!(remaining, HOSTED_FUEL);
}

#[test]
fn owned_vector_vm_resource_scope_prices_real_transfer_and_zero_fuel_maintenance() {
    let (_, witness, _) = owned_vector_descriptors::tests::verified_fixture();
    let element = witness.elements().element(RecordId(0)).unwrap();
    let mut heap = byte_buffer::Heap::default();
    let mut vector = byte_buffer::OwnedVectorDescriptor::empty();
    let mut setup_fuel = 1_000_000;
    assert_eq!(
        heap.owned_vector_try_reserve(&mut vector, element, 1, &mut setup_fuel),
        Ok(byte_buffer::GrowStatus::Ok)
    );
    let bytes = vector.encoding();
    let aggregate = AggregateTy::OwnedRecordVector(RecordId(0));
    let span = witness.functions()[0].span;
    let before = heap.ledger();
    assert_eq!(before.live_allocations, 1);
    let mut fuel = 6;
    assert!(matches!(
        resources::preflight(
            resources::RuntimeResourceScope::paid(&witness, &mut fuel),
            &heap,
            aggregate,
            &bytes,
            16,
            span,
        ),
        Err(OwnedRunFailure::Scalar(RunFailure::Fuel(_)))
    ));
    assert_eq!(fuel, 6);
    assert_eq!(heap.ledger(), before);
    fuel = 7;
    resources::preflight(
        resources::RuntimeResourceScope::paid(&witness, &mut fuel),
        &heap,
        aggregate,
        &bytes,
        16,
        span,
    )
    .unwrap();
    assert_eq!(fuel, 0);
    fuel = HOSTED_FUEL;
    resources::preflight(
        resources::RuntimeResourceScope::paid(&witness, &mut fuel),
        &heap,
        aggregate,
        &bytes,
        16,
        span,
    )
    .unwrap();
    assert_eq!(fuel, HOSTED_FUEL);
    let mut destination = [0xEF; 16];
    resources::copy(
        resources::RuntimeResourceScope::maintenance(&witness),
        aggregate,
        &bytes,
        &mut destination,
        span,
    )
    .unwrap();
    assert_eq!(destination, bytes);
    fuel = 0;
    resources::release(
        resources::RuntimeResourceScope::maintenance(&witness),
        &mut heap,
        aggregate,
        &bytes,
        span,
    )
    .unwrap();
    assert_eq!(fuel, 0);
    assert_eq!(heap.ledger().live_allocations, 0);
    assert_eq!(heap.ledger().allocations, heap.ledger().releases);
}

fn move_witness() -> (SourceMap, VerifiedOwnedVectorProgram) {
    let (sources, mut raw, _, _) = owned_vector_descriptors::tests::fixture(None);
    let span = raw.records[0].span;
    let aggregate = AggregateTy::OwnedRecordVector(RecordId(0));
    raw.builtins = BuiltinOrigins::None.admit(BuiltinItem::Vector(Item::Vec));
    raw.enums.clear();
    raw.vector_functions.clear();
    let mut function = consumer_fixtures::function(0, ValueTy::Owned(aggregate), span);
    function
        .parameters
        .push(ParameterBinding::Owned(OwnerPlaceId(0)));
    function.owners = vec![
        OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
            kind: OwnerKind::Parameter { position: 0 },
            span,
        },
        OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(aggregate).unwrap(),
            kind: OwnerKind::Local { mutable: false },
            span,
        },
    ];
    function.blocks.push(OwnedBlock {
        span,
        merge: None,
        statements: vec![
            consumer_fixtures::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), span),
            consumer_fixtures::instruction(
                OwnedInstruction::MoveInitialize {
                    destination: OwnerPlaceId(1),
                    source: OwnerPlaceId(0),
                },
                span,
            ),
        ],
        terminator: consumer_fixtures::end(OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(1)), span),
    });
    raw.functions = vec![function];
    let wrapper = owned_vector_descriptors::tests::prepare(
        &sources,
        raw,
        &[],
        OwnedVectorImportClaims::empty(),
    );
    let witness = VerifiedOwnedVectorProgram::verify(
        wrapper,
        &sources,
        budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    (sources, witness)
}

#[test]
fn owned_vector_vm_paid_whole_move_exact_and_one_less_preserve_real_owners() {
    let (_, witness) = move_witness();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let span = witness.functions()[0].span;
    let usage = plan.function(hir::DefId(0)).usage();
    for fuel in [10, 11] {
        let mut heap = byte_buffer::Heap::default();
        let mut vector = byte_buffer::OwnedVectorDescriptor::empty();
        let element = witness.elements().element(RecordId(0)).unwrap();
        let mut setup_fuel = 1_000_000;
        heap.owned_vector_try_reserve(&mut vector, element, 1, &mut setup_fuel)
            .unwrap();
        let encoded = vector.encoding();
        let mut frame = ValueFrame::<Scalar>::allocate(&plan, hir::DefId(0), 1, None).unwrap();
        let offset = plan.function(hir::DefId(0)).owner_offset(OwnerPlaceId(0));
        frame.payload[offset..offset + 16].copy_from_slice(&encoded);
        frame.owners[0] = OwnerRuntime {
            state: AVAILABLE,
            generation: 1,
            shared_children: 0,
            exclusive_children: 0,
        };
        let mut machine = ValueMachine::<Scalar, VerifiedOwnedVectorProgram> {
            plan: &plan,
            frames: vec![frame],
            limits: Limits::default(),
            fuel,
            next_activation: 2,
            live_slots: usage.scalar_slots,
            live_cells: usage.expanded_cells,
            live_bytes: usage.reference_bytes,
            header_bytes: size_of::<byte_buffer::Heap>()
                + resources::control_bytes()
                + resources::owned_control_bytes::<VerifiedOwnedVectorProgram>(),
            heap,
            events: Vec::new(),
            observer: array_observe::Observer::default(),
        };
        machine
            .statement(0, &OwnedInstruction::StorageLive(OwnerPlaceId(1)), span)
            .unwrap();
        let owners = machine.frames[0].owners.clone();
        let bytes = machine.frames[0].payload.clone();
        let ledger = machine.heap.ledger();
        let instruction = OwnedInstruction::MoveInitialize {
            destination: OwnerPlaceId(1),
            source: OwnerPlaceId(0),
        };
        let result = machine
            .charge(plan.statement_cost(hir::DefId(0), &instruction), span)
            .and_then(|_| machine.statement(0, &instruction, span));
        if fuel == 10 {
            assert!(matches!(
                result,
                Err(OwnedRunFailure::Scalar(RunFailure::Fuel(_)))
            ));
            assert_eq!(machine.frames[0].owners, owners);
            assert_eq!(machine.frames[0].payload, bytes);
            assert_eq!(machine.heap.ledger(), ledger);
            assert_eq!(machine.fuel, 6);
        } else {
            result.unwrap();
            assert_eq!(machine.fuel, 0);
            assert_eq!(machine.frames[0].owners[0].state, MOVED);
            assert_eq!(machine.frames[0].owners[1].state, AVAILABLE);
            let destination = machine
                .owner_extent(machine.raw_key(0, OwnerPlaceId(1)), span)
                .unwrap();
            assert_eq!(&machine.frames[0].payload[destination], &encoded);
            machine
                .release_resources(machine.raw_key(0, OwnerPlaceId(1)), span)
                .unwrap();
            assert_eq!(machine.heap.ledger().live_allocations, 0);
        }
        machine.heap.cleanup();
    }
}
