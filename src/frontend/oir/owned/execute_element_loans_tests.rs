//! Runtime identity/address refusals exercise real acquired owning cells.
use super::*;
use crate::frontend::{
    declaration_index::{self as index, IndexLimits, SourceOwner, WorkMeter},
    project::{budget::Allocator, ProjectLimits, ProjectSources},
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
const SOURCE:&str="use alloc::bytes::ByteBuf;use alloc::bytes::new as bytes_new;use alloc::vec::new;use alloc::vec::try_reserve;use alloc::vec::push_reserved;struct Entry{value:i32,bytes:ByteBuf}fn main()->i32{let mut values=new::<Entry>();try_reserve(&mut values,1);push_reserved(&mut values,Entry{value:42,bytes:bytes_new()});let item=&mut values[0];item.value=item.value+1;return item.value-1;}";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "oxid-record-element-identity-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), SOURCE).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn with_plan(
    inspect: impl FnOnce(&ExecutionPlan<'_, verified::VerifiedOwnedVectorProgram>, hir::DefId),
) {
    let fixture = Fixture::new();
    let project = ProjectSources::load_typed(
        fixture.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default(),
    )
    .unwrap();
    let work = WorkMeter::default();
    let index = index::collect_owned_vector_candidate(
        SourceOwner::project(&project),
        IndexLimits::default(),
        8 * 1024 * 1024,
        &work,
        &mut Allocator::default(),
    )
    .unwrap();
    let typed = source::resolve::type_owned_vector_source(&index, &work, &mut Allocator::default())
        .unwrap();
    let associated = source::association::associate_element_fixture(&typed).unwrap();
    let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
        associated,
        budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    inspect(&plan, typed.entry().unwrap());
}
fn before_use<'p, 'w>(
    plan: &'p ExecutionPlan<'w, verified::VerifiedOwnedVectorProgram>,
    entry: hir::DefId,
) -> (
    ValueMachine<'p, 'w, Scalar, verified::VerifiedOwnedVectorProgram>,
    usize,
    LoanId,
    Span,
) {
    let initial = 1 + plan.function(entry).usage().activation_fuel_cells();
    for fuel in initial..initial + 8192 {
        let limits = Limits {
            fuel,
            frames: 8,
            ..Default::default()
        };
        let mut machine = ValueMachine {
            plan,
            frames: Vec::new(),
            limits,
            fuel,
            next_activation: 1,
            live_slots: 0,
            live_cells: 0,
            live_bytes: 0,
            header_bytes: limits.frames * size_of::<ValueFrame<Scalar>>()
                + control_bytes::<Scalar, verified::VerifiedOwnedVectorProgram>()
                + shared_adapter_control_bytes::<Scalar, verified::VerifiedOwnedVectorProgram>()
                + plan::element_loan_accounting::query_control_bytes::<
                    verified::VerifiedOwnedVectorProgram,
                >(),
            heap: byte_buffer::Heap::default(),
            events: Vec::new(),
            observer: array_observe::Observer::default(),
        };
        let main = &plan.witness().functions()[entry.0];
        machine
            .activation_preflight(entry, initial, main.span)
            .unwrap();
        machine.frames = plan::reserve(limits.frames).unwrap();
        machine.install(ValueFrame::allocate(plan, entry, 1, None).unwrap());
        let _ = machine.execute();
        let selected = machine
            .frames
            .iter()
            .enumerate()
            .find_map(|(frame, state)| {
                let f = machine.function(state.function);
                state.loans.iter().enumerate().find_map(|(loan, state)| {
                    (state.state == 1 && shape::local_record_element(f, LoanId(loan)).is_some())
                        .then_some((frame, LoanId(loan), f.loans[loan].span))
                })
            });
        if let Some((frame, loan, span)) = selected {
            return (machine, frame, loan, span);
        }
    }
    panic!("no active element loan before its first use");
}
#[test]
fn owning_record_element_runtime_checks_epochs_nominal_captured_ordinal_parent_and_descriptor_before_address(
) {
    with_plan(|plan, entry| {
        for mutation in 0..7 {
            let (mut machine, frame, loan, span) = before_use(plan, entry);
            let mut handle = machine.reference_from_loan(frame, loan, span).unwrap();
            assert_eq!(
                machine.checked_handle_view(handle, span).unwrap(),
                AggregateTy::Record(RecordId(0))
            );
            match mutation {
                0 => handle.root.activation += 1,
                1 => handle.permission.instance += 1,
                2 => {
                    handle.view.offset += plan
                        .witness()
                        .aggregate_layout(AggregateTy::Record(RecordId(0)))
                        .unwrap()
                        .size() as u64;
                    machine.frames[frame].loans[loan.0].view = handle.view;
                }
                3 => {
                    handle.view.aggregate = Some(
                        AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(1)))
                            .unwrap(),
                    );
                    machine.frames[frame].loans[loan.0].view = handle.view;
                }
                4 => {
                    let f = machine.function(machine.frames[frame].function);
                    let operand = shape::local_record_element_index(f, loan).unwrap();
                    machine.frames[frame].slots[operand.local.0] = Some(Scalar::I32(1));
                }
                5 => machine.frames[frame].loans[loan.0].parent.instance = 1,
                6 => {
                    let extent = machine.owner_extent(handle.root, span).unwrap();
                    machine.frames[handle.root.frame as usize].payload[extent].fill(0);
                }
                _ => unreachable!(),
            }
            assert!(
                machine.checked_handle_view(handle, span).is_err(),
                "identity forgery {mutation}"
            );
        }
    });
}

#[test]
fn a_forged_acquisition_operand_is_refused_before_any_runtime_effect() {
    with_plan(|plan, entry| {
        let (mut machine, frame, loan, span) = before_use(plan, entry);
        let function = machine.frames[frame].function;
        let mut operand = plan.function(function).element_index(loan).unwrap();
        operand.local = LocalId(usize::MAX);
        let fuel = machine.fuel;
        let ledger = machine.heap.ledger();
        let loans = machine.frames[frame].loans.clone();
        let owners = machine.frames[frame].owners.clone();
        let error = machine
            .acquire_element(frame, loan, operand, span)
            .unwrap_err();
        assert!(format!("{error:?}").contains("element acquisition plan relation"));
        assert_eq!(machine.fuel, fuel);
        assert_eq!(machine.heap.ledger(), ledger);
        assert_eq!(machine.frames[frame].loans, loans);
        assert_eq!(machine.frames[frame].owners, owners);
    });
}
#[test]
fn owning_record_element_explicit_cell_address_stays_in_the_pinned_outer_node_across_child_growth_and_writeback(
) {
    with_plan(|plan, entry| {
        let (mut machine, frame, loan, span) = before_use(plan, entry);
        let handle = machine.reference_from_loan(frame, loan, span).unwrap();
        let fields = plan.witness().declarations().fields(RecordId(0)).unwrap();
        let scalar = machine
            .resource_place(
                frame,
                AccessBase::Loan(loan),
                handle.root,
                fields[0].offset() as u64,
                4,
                span,
            )
            .unwrap();
        let buffer = machine
            .resource_place(
                frame,
                AccessBase::Loan(loan),
                handle.root,
                fields[1].offset() as u64,
                16,
                span,
            )
            .unwrap();
        let BufferPlace::Cell(ref address) = scalar else {
            panic!("expected checked owning cell address")
        };
        let pointer = address.pointer;
        assert_eq!(address.permission, BorrowKind::Exclusive);
        let mut child = machine
            .heap
            .descriptor(machine.resource_bytes(&buffer))
            .unwrap();
        let mut fuel = usize::MAX;
        assert_eq!(
            machine
                .heap
                .try_extend(&mut child, b"\0\r\n\x1a", &mut fuel),
            Ok(byte_buffer::GrowStatus::Ok)
        );
        machine
            .resource_bytes_mut(&buffer)
            .copy_from_slice(&child.encoding());
        let again = machine
            .resource_place(
                frame,
                AccessBase::Loan(loan),
                handle.root,
                fields[0].offset() as u64,
                4,
                span,
            )
            .unwrap();
        let BufferPlace::Cell(ref address) = again else {
            panic!("expected checked owning cell address")
        };
        assert_eq!(pointer, address.pointer);
        machine
            .resource_bytes_mut(&scalar)
            .copy_from_slice(&43i32.to_le_bytes());
        assert_eq!(machine.resource_bytes(&again), 43i32.to_le_bytes());
        assert_eq!(machine.heap.bytes(&child).unwrap(), b"\0\r\n\x1a");
        // A stale child ID may not produce a ByteBuf payload address. The
        // checked outer-cell address itself remains an unrelated domain.
        let stale = child.encoding();
        machine.heap.release(&mut child).unwrap();
        assert!(machine
            .heap
            .descriptor(machine.resource_bytes(&buffer))
            .is_err());
        assert_eq!(machine.resource_bytes(&buffer), stale);
        machine.resource_bytes_mut(&buffer).fill(0);
    });
}
