//! The public source path produces the call frame; only the failure boundary
//! is perturbed. OS-visible bytes independently identify the file position.
use super::*;
use crate::frontend::oir::owned::source::resource_fixtures::checked_enum_project;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Input(std::path::PathBuf);
impl Input {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "oxid-seek-delivery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, b"ABCDE").unwrap();
        Self(path)
    }
    fn source(&self) -> String {
        let path: String = self
            .0
            .to_str()
            .unwrap()
            .chars()
            .flat_map(char::escape_default)
            .collect();
        format!("use std::fs::open_read;use std::fs::OpenResult;use std::fs::FileStatus;use std::fs::seek_absolute;fn main()->i32{{let path=b\"{path}\";let opened=open_read(&path);match opened{{OpenResult::Opened(value)=>{{let mut file=value;let status=seek_absolute(&mut file,3i64);match status{{FileStatus::Ok=>{{return 42;}},FileStatus::Error(code)=>{{return code;}},}}}},OpenResult::Error(code)=>{{return code;}},OpenResult::MemoryLimit=>{{return 2;}},OpenResult::AllocationFailed=>{{return 3;}},}}}}")
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn before_seek<'p, 'w>(
    plan: &'p ExecutionPlan<'w>,
    entry: hir::DefId,
) -> (ValueMachine<'p, 'w, RuntimeScalar>, CallResult, Span) {
    let initial = 1 + plan.function(entry).usage().activation_fuel_cells();
    for fuel in initial..initial + 4096 {
        let limits = Limits {
            fuel,
            ..Default::default()
        };
        let mut machine = ValueMachine {
            plan,
            frames: vec![],
            limits,
            fuel,
            next_activation: 1,
            live_slots: 0,
            live_cells: 0,
            live_bytes: 0,
            header_bytes: limits.frames * size_of::<ValueFrame<RuntimeScalar>>(),
            heap: byte_buffer::Heap::default(),
            events: vec![],
            observer: array_observe::Observer::default(),
        };
        let function = &plan.witness().functions()[entry.0];
        machine
            .activation_preflight(entry, initial, function.span)
            .unwrap();
        machine.frames = plan::reserve(limits.frames).unwrap();
        machine.install(ValueFrame::allocate(plan, entry, 1, None).unwrap());
        let result = machine.execute();
        assert!(
            matches!(result, Err(OwnedRunFailure::Scalar(RunFailure::Fuel(_)))),
            "{result:?}"
        );
        let frame = machine.frames.last().unwrap();
        if Some(frame.function) == plan.witness().experimental_function(Function::SeekAbsolute) {
            let function = &plan.witness().functions()[frame.function.0];
            if let Some(statement) = function.blocks[frame.block.0].statements.get(frame.next) {
                if let OwnedInstruction::Experimental {
                    operation: Function::SeekAbsolute,
                    destination,
                } = statement.kind
                {
                    return (machine, destination, plan::instruction_span(statement));
                }
            }
        }
    }
    panic!("source call did not reach seek");
}

#[test]
fn file_seek_prepares_complete_return_loan_and_fuel_before_os_position_changes() {
    let input = Input::new();
    let checked = checked_enum_project(&input.source());
    let plan = ExecutionPlan::build(&checked.witness).unwrap();
    assert!(plan.has_file_seek() && plan.uses_wide_scalars());
    for mutation in 0..6 {
        let (mut machine, destination, span) = before_seek(&plan, checked.entry);
        let frame = machine.frames.len() - 1;
        let (_, file) = machine.file_parameter(frame, span).unwrap();
        let delivery = machine
            .prepare_file_delivery(frame, Function::SeekAbsolute, destination, span)
            .unwrap();
        machine.fuel = delivery.cost;
        match mutation {
            0 => machine.fuel -= 1,
            1 => machine.frames[frame].owners[delivery.from.owner as usize].generation = u64::MAX,
            2 => {
                machine.frames[delivery.caller].owners[delivery.to.owner as usize].generation =
                    u64::MAX
            }
            3 => {
                let release = delivery.releases[0].unwrap();
                if release.owner {
                    machine.frames[release.frame].owners[release.index].exclusive_children = 0;
                } else {
                    machine.frames[release.frame].loans[release.index].exclusive_children = 0;
                }
            }
            4 => machine.frames[frame].slots[0] = Some(RuntimeScalar::Narrow(Scalar::I32(3))),
            5 => {} // exactly the prepaid return succeeds, without subsequent fuel
            _ => unreachable!(),
        }
        let before: Vec<_> = machine.frames.iter().map(|f| f.payload.clone()).collect();
        let ledger = machine.heap.ledger();
        let result = machine.file_delivery(frame, Function::SeekAbsolute, destination, span);
        if mutation == 5 {
            result.unwrap();
            assert_eq!(machine.fuel, 0);
            assert_eq!(machine.frames.len(), frame);
        } else {
            assert!(result.is_err(), "mutation {mutation}");
            assert_eq!(
                machine
                    .frames
                    .iter()
                    .map(|f| f.payload.clone())
                    .collect::<Vec<_>>(),
                before
            );
        }
        assert_eq!(machine.heap.ledger(), ledger);
        let mut byte = [0];
        let mut fuel = 1;
        assert_eq!(
            machine
                .heap
                .file_access(&file)
                .unwrap()
                .read(&mut byte, &mut fuel)
                .unwrap(),
            Ok(1)
        );
        assert_eq!(
            byte,
            [if mutation == 5 { b'D' } else { b'A' }],
            "mutation {mutation}"
        );
    }
}
