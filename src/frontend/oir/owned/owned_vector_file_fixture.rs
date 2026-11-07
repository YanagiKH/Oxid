//! The shared caller obtains each File through its real canonical create call
//! and a complete ordinary OpenResult match before moving it into the record.
use super::*;
use crate::frontend::builtin_catalog::experimental as file;

fn file_value(caller: &mut Caller, path: &[u8]) -> OwnerPlaceId {
    let bytes = caller.owner(
        AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::U8, path.len()).unwrap()),
        OwnerKind::Local { mutable: false },
    );
    let elements = path
        .iter()
        .map(|byte| caller.byte_constant(*byte))
        .collect();
    caller.statement(OwnedInstruction::ConstructArray {
        destination: bytes,
        elements,
    });
    let signature = file::Function::Create.signature(|_| EnumId(1));
    let created = match caller.call_signature(
        hir::DefId(1),
        signature.parameters(),
        signature.result,
        &[Input::Borrow(bytes)],
    ) {
        CallResult::Owned(owner) => owner,
        CallResult::Scalar(_) => panic!("file create test returned a scalar"),
    };
    caller.statement(OwnedInstruction::StorageEnd(bytes));
    let subject = caller.owner(
        AggregateTy::Enum(EnumId(1)),
        OwnerKind::Local { mutable: false },
    );
    caller.statement(OwnedInstruction::MoveInitialize {
        destination: subject,
        source: created,
    });
    let failure = caller.constant(-19);
    let binding = caller.scalar_slot(hir::Ty::I32);
    caller.function.locals[binding.0].kind = LocalKind::Binding;
    // A guarded owned binding starts at ConsumeOwnedVariant, without a
    // separate StorageLive, and remains immutable until moved to staging.
    let opened = OwnerPlaceId(caller.function.owners.len());
    caller.function.owners.push(OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::File).unwrap(),
        kind: OwnerKind::Local { mutable: false },
        span: caller.span,
    });
    let first = caller.function.blocks.len();
    let match_id = MatchId(caller.function.matches.len());
    caller.function.matches.push(MatchDecl {
        source: subject,
        span: caller.span,
        arms: (0..4)
            .map(|arm| MatchArm {
                variant: VariantId {
                    enumeration: EnumId(1),
                    index: arm,
                },
                dispatch: BlockId(first + arm),
                entry: BlockId(first + 4 + arm),
            })
            .collect(),
    });
    for arm in 0..4 {
        caller.finish_block(OwnedTerminatorKind::MatchDispatch { match_id, arm });
    }
    caller.statement(OwnedInstruction::ConsumeOwnedVariant {
        match_id,
        arm: 0,
        destination: opened,
    });
    caller.statement(OwnedInstruction::StorageEnd(subject));
    let payload = caller.owner(AggregateTy::File, OwnerKind::Temporary);
    caller.statement(OwnedInstruction::MoveInitialize {
        destination: payload,
        source: opened,
    });
    caller.statement(OwnedInstruction::StorageEnd(opened));
    caller.finish_block(OwnedTerminatorKind::Goto(BlockId(first + 8)));
    for arm in 1..4 {
        caller.statement(OwnedInstruction::ConsumeVariant {
            match_id,
            arm,
            destination: (arm == 1).then_some(binding),
        });
        caller.statement(OwnedInstruction::StorageEnd(subject));
        caller.finish_block(OwnedTerminatorKind::ReturnScalar(failure));
    }
    payload
}

pub(in crate::frontend::oir::owned) fn raw(
    paths: [&[u8]; 2],
) -> (
    SourceMap,
    RawOwnedProgram,
    [Function; 8],
    OwnedVectorImportClaims,
    i32,
) {
    raw_with_schedule(paths, Schedule::TruncateThenTake)
}

fn raw_with_schedule(
    paths: [&[u8]; 2],
    schedule: Schedule,
) -> (
    SourceMap,
    RawOwnedProgram,
    [Function; 8],
    OwnedVectorImportClaims,
    i32,
) {
    assert!(paths
        .iter()
        .all(|path| !path.is_empty() && !path.contains(&0)));
    let (sources, mut raw, keys, imports) = fixture(None);
    let span = raw.records[0].span;
    raw.records[0].fields.push(RawFieldDecl {
        id: FieldId {
            record: RecordId(0),
            index: 1,
        },
        ty: ParameterTy::Value(ValueTy::Owned(AggregateTy::File)),
        span,
    });
    raw.builtins = raw
        .builtins
        .admit(BuiltinItem::Experimental(file::Item::Create));
    raw.enums.push(RawEnumDecl {
        id: EnumId(1),
        span,
        variants: (0..file::Enumeration::Open.members().len())
            .map(|index| RawVariantDecl {
                id: VariantId {
                    enumeration: EnumId(1),
                    index,
                },
                payload: file::Enumeration::Open
                    .value_payload(index)
                    .map(ParameterTy::Value),
                span,
            })
            .collect(),
    });
    let mut caller = Caller::new(span, keys);
    caller.owned_start = 3;
    let main = all_operations_main(
        caller,
        |caller, value, ordinal| {
            let payload = file_value(caller, paths[ordinal]);
            let owner = caller.owner(AggregateTy::Record(RecordId(0)), OwnerKind::Temporary);
            caller.statement(OwnedInstruction::ConstructComposite {
                destination: owner,
                fields: vec![
                    (
                        FieldId {
                            record: RecordId(0),
                            index: 0,
                        },
                        FieldInitializer::Scalar(value),
                    ),
                    (
                        FieldId {
                            record: RecordId(0),
                            index: 1,
                        },
                        FieldInitializer::Owned(payload),
                    ),
                ],
            });
            owner
        },
        schedule,
    );
    for function in &mut raw.functions {
        function.id.0 += 2;
    }
    let signature = file::Function::Create.signature(|_| EnumId(1));
    raw.functions.insert(
        0,
        function(
            1,
            signature.parameters().to_vec(),
            signature.result,
            span,
            |destination| OwnedInstruction::Experimental {
                operation: file::Function::Create,
                destination,
            },
        ),
    );
    raw.functions.insert(0, main);
    (sources, raw, keys, imports, 58)
}

pub(in crate::frontend::oir::owned) fn files(
    paths: [&[u8]; 2],
) -> (SourceMap, VerifiedOwnedVectorProgram, [Function; 8], i32) {
    sealed_with_schedule(paths, Schedule::TruncateThenTake)
}

pub(in crate::frontend::oir::owned) fn files_with_live_take(
    paths: [&[u8]; 2],
) -> (SourceMap, VerifiedOwnedVectorProgram, [Function; 8], i32) {
    sealed_with_schedule(paths, Schedule::TakeThenTruncate)
}

fn sealed_with_schedule(
    paths: [&[u8]; 2],
    schedule: Schedule,
) -> (SourceMap, VerifiedOwnedVectorProgram, [Function; 8], i32) {
    let (sources, raw, keys, imports, expected) = match schedule {
        Schedule::TruncateThenTake => raw(paths),
        Schedule::TakeThenTruncate => raw_with_schedule(paths, schedule),
    };
    let wrapper = prepare(&sources, raw, &keys, imports);
    let witness = VerifiedOwnedVectorProgram::verify(
        wrapper,
        &sources,
        budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    (sources, witness, keys, expected)
}

struct Paths {
    directory: std::path::PathBuf,
    files: [std::path::PathBuf; 2],
}
impl Paths {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "oxid-owned-vector-files-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(directory.is_absolute());
        std::fs::create_dir(&directory).unwrap();
        let directory = std::fs::canonicalize(directory).unwrap();
        Self {
            files: [directory.join("popped"), directory.join("truncated")],
            directory,
        }
    }
    fn bytes(&self) -> [&[u8]; 2] {
        self.files
            .each_ref()
            .map(|path| path.as_os_str().as_encoded_bytes())
    }
    #[cfg(all(
        any(target_os = "linux", target_os = "macos"),
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    fn assert_released(&self) {
        #[cfg(target_os = "linux")]
        for descriptor in std::fs::read_dir("/proc/self/fd").unwrap() {
            if let Ok(target) = std::fs::read_link(descriptor.unwrap().path()) {
                assert!(
                    !self.files.contains(&target),
                    "execution retained an OS descriptor for {}",
                    target.display()
                );
            }
        }
        for path in &self.files {
            assert!(path.is_file());
            std::fs::write(path, b"reopened after owned release").unwrap();
        }
    }
}
impl Drop for Paths {
    fn drop(&mut self) {
        for path in &self.files {
            let _ = std::fs::remove_file(path);
        }
        let _ = std::fs::remove_dir(&self.directory);
    }
}

#[test]
fn owned_vector_witness_file_caller_requires_the_real_opened_arm_twice() {
    let paths = Paths::new();
    for live_take in [false, true] {
        let (_, witness, keys, expected) = if live_take {
            files_with_live_take(paths.bytes())
        } else {
            files(paths.bytes())
        };
        assert_eq!(expected, 58);
        let element = witness.elements().element(RecordId(0)).unwrap();
        assert_eq!(element.size(), 24);
        assert_eq!(element.identity_capacity(), 1);
        let main = &witness.functions()[0];
        assert_eq!(main.matches.len(), 2);
        assert!(main
            .matches
            .iter()
            .all(|descriptor| descriptor.arms.len() == 4));
        assert_eq!(
            main.blocks
                .iter()
                .flat_map(|block| &block.statements)
                .filter(|statement| matches!(
                    statement.kind,
                    OwnedInstruction::ConsumeOwnedVariant { arm: 0, .. }
                ))
                .count(),
            2
        );
        for key in keys {
            let target = witness
                .functions()
                .iter()
                .find(|function| witness.owned_vector_operation(function.id) == Some(key))
                .unwrap()
                .id;
            assert!(main.calls.iter().any(|call| call.target == target));
        }
    }
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn owned_vector_vm_file_caller_releases_both_real_files_before_terminal_cleanup() {
    let paths = Paths::new();
    let (_, witness, _, expected) = files(paths.bytes());
    check_vm_files(&witness, &paths, expected);
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn owned_vector_vm_live_file_take_retains_child_through_ordinary_move_and_truncate() {
    let paths = Paths::new();
    let (_, witness, _, expected) = files_with_live_take(paths.bytes());
    check_vm_files(&witness, &paths, expected);
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn check_vm_files(witness: &VerifiedOwnedVectorProgram, paths: &Paths, expected: i32) {
    use crate::frontend::build_config::ExecutionPolicy;
    use crate::frontend::oir::owned::{byte_buffer::testing, execute};
    let ((result, remaining), observation) = testing::capture(testing::Control::default(), || {
        execute::observe_witness(
            witness,
            Some(hir::DefId(0)),
            execute::Limits::default(),
            ExecutionPolicy::Bounded,
        )
    });
    assert_eq!(result, Ok(Scalar::I32(expected)));
    assert_eq!(observation.before_cleanup.live_allocations, 0);
    assert_eq!(observation.before_cleanup.allocations, 3);
    assert_eq!(observation.before_cleanup.releases, 3);
    assert_eq!(observation.before_cleanup.peak_bytes, 352);
    assert_eq!(observation.before_cleanup, observation.after_cleanup);
    paths.assert_released();
    let exact = execute::Limits::default().fuel - remaining;
    for (fuel, succeeds) in [(exact, true), (exact - 1, false)] {
        let (result, _) = execute::observe_witness(
            witness,
            Some(hir::DefId(0)),
            execute::Limits {
                fuel,
                ..execute::Limits::default()
            },
            ExecutionPolicy::Bounded,
        );
        if succeeds {
            assert_eq!(result, Ok(Scalar::I32(expected)));
        } else {
            assert!(matches!(
                result,
                Err(execute::OwnedRunFailure::Scalar(RunFailure::Fuel(_)))
            ));
        }
    }
    let (result, remaining) = execute::observe_witness(
        witness,
        Some(hir::DefId(0)),
        execute::Limits {
            fuel: 0,
            ..execute::Limits::default()
        },
        ExecutionPolicy::Hosted,
    );
    assert_eq!(result, Ok(Scalar::I32(expected)));
    assert_eq!(remaining, crate::frontend::build_config::HOSTED_FUEL);
    paths.assert_released();
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn owned_vector_vm_file_error_arm_releases_the_preexisting_vector() {
    use crate::frontend::build_config::ExecutionPolicy;
    use crate::frontend::oir::owned::{byte_buffer::testing, execute};
    let paths = Paths::new();
    let missing = paths.files[0].join("missing-parent").join("file");
    let (_, witness, _, _) = files([
        missing.as_os_str().as_encoded_bytes(),
        paths.files[1].as_os_str().as_encoded_bytes(),
    ]);
    let ((result, _), observation) = testing::capture(testing::Control::default(), || {
        execute::observe_witness(
            &witness,
            Some(hir::DefId(0)),
            execute::Limits::default(),
            ExecutionPolicy::Bounded,
        )
    });
    assert_eq!(result, Ok(Scalar::I32(-19)));
    assert_eq!(observation.before_cleanup.live_allocations, 0);
    assert_eq!(observation.before_cleanup.allocations, 2);
    assert_eq!(observation.before_cleanup.releases, 2);
    assert_eq!(observation.before_cleanup.peak_bytes, 352);
    assert_eq!(observation.before_cleanup, observation.after_cleanup);
    assert!(paths.files.iter().all(|path| !path.exists()));
}
