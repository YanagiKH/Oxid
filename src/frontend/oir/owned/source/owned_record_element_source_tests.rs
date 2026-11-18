//! Parsed local loans over genuine checked, non-Copy record vector cells.
use super::*;
use crate::frontend::{
    build_config::ExecutionPolicy,
    declaration_index::{self as index, IndexLimits, SourceOwner},
    oir::owned::{self as owned, byte_buffer, execute, native, verified},
    project::{ProjectLimits, ProjectSources},
    source::SourceMap,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
#[path = "owned_record_element_cases.rs"]
mod cases;

struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "oxid-record-element-loans-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), format!("{}{source}", cases::IMPORTS)).unwrap();
        Self(root)
    }
    fn project(&self) -> ProjectSources {
        ProjectSources::load_typed(
            self.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() && std::env::var_os("OXID_KEEP_TEST_PROJECTS").is_none() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn with_typed(
    source: &str,
    inspect: impl FnOnce(&typeck::TypedOwnedProgram<'_>, &SourceMap, &Fixture),
) {
    let fixture = Fixture::new(source);
    let project = fixture.project();
    let work = WorkMeter::default();
    let index = index::collect_owned_vector_candidate(
        SourceOwner::project(&project),
        IndexLimits::default(),
        8 * 1024 * 1024,
        &work,
        &mut Allocator::default(),
    )
    .unwrap();
    let typed =
        resolve::type_owned_vector_source(&index, &work, &mut Allocator::default()).unwrap();
    inspect(&typed, project.sources(), &fixture);
}
fn witness(typed: &typeck::TypedOwnedProgram<'_>) -> verified::VerifiedOwnedVectorProgram {
    let associated = owned::source::association::lower_and_associate_owned_vector(typed).unwrap();
    verified::VerifiedOwnedVectorProgram::verify_associated(
        associated,
        owned::budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap()
}
fn run(source: &str, expected: i32) {
    with_typed(source, |typed, _, _| {
        let witness = witness(typed);
        assert!(owned::plan::ExecutionPlan::build(&witness)
            .unwrap()
            .has_local_record_element_loans());
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let ((result, _), seen) = byte_buffer::testing::capture(Default::default(), || {
                execute::observe_witness(
                    &witness,
                    typed.entry(),
                    execute::Limits::default(),
                    policy,
                )
            });
            assert_eq!(
                result,
                Ok(owned::Scalar::I32(expected)),
                "{policy:?}: {source}"
            );
            assert_eq!(seen.after_cleanup.live_bytes, 0);
            assert_eq!(seen.after_cleanup.live_allocations, 0);
            assert_eq!(seen.after_cleanup.allocations, seen.after_cleanup.releases);
            assert_eq!(seen.before_cleanup, seen.after_cleanup);
        }
    });
}
#[test]
fn owning_record_element_local_loans_capture_once_and_reborrow_across_statements_and_helpers() {
    run(cases::SIMPLE, 42);
}
#[test]
fn owning_record_element_local_loans_evaluate_reallocating_index_before_pinning_the_selected_cell()
{
    run(cases::REALLOCATING_INDEX, 42);
}
#[test]
fn owning_record_element_local_loans_keep_concrete_generic_nominal_identity_and_reference_annotations(
) {
    run(cases::GENERIC_RECORD, 42);
}
#[test]
fn owning_record_element_local_loans_allow_whole_vector_shared_queries() {
    run(
        &cases::BASE.replace("__ACTION__", "if len(&values)!=2{return -2;}"),
        42,
    );
}
#[test]
fn owning_record_element_local_loans_mutate_real_nested_buffer_and_copy_vector_children() {
    run(cases::CHILDREN, 42);
}
#[test]
fn owning_record_element_local_loans_release_on_continue_break_and_scope_exit() {
    run(cases::LOOP, 42);
}
#[test]
fn owning_record_element_local_loans_allocate_no_acquisition_storage() {
    with_typed(cases::SIMPLE, |typed, _, _| {
        let witness = witness(typed);
        let ((result, _), seen) = byte_buffer::testing::capture(Default::default(), || {
            execute::observe_witness(
                &witness,
                typed.entry(),
                execute::Limits::default(),
                ExecutionPolicy::Bounded,
            )
        });
        assert_eq!(result, Ok(owned::Scalar::I32(42)));
        // The first outer reserve and the post-scope growth own exactly two
        // allocations. Empty ByteBuf values and every local/helper loan own none.
        assert_eq!(seen.before_cleanup.allocations, 2);
        assert_eq!(seen.before_cleanup.releases, 2);
        assert_eq!(seen.before_cleanup.live_allocations, 0);
    });
}
#[test]
fn owning_record_element_local_loans_cleanup_every_allocation_refusal_and_exact_fuel_edge() {
    // The final reserve is otherwise optional: the original capacity still
    // holds the second value, so ignoring its failure legitimately returns 42.
    // Observe its status here so every injected refusal has an observable exit.
    let source = format!(
        "use alloc::vec::GrowStatus;{}",
        cases::CHILDREN.replace(
            "    try_reserve(&mut values,9);",
            "    let grown=try_reserve(&mut values,9); match grown { GrowStatus::Ok=>{}, GrowStatus::MemoryLimit=>{return -10;}, GrowStatus::AllocationFailed=>{return -11;}, GrowStatus::CapacityOverflow=>{return -12;}, }",
        )
    );
    with_typed(&source, |typed, _, _| {
        let witness = witness(typed);
        let limits = execute::Limits::default();
        let ((success, remaining), baseline) =
            byte_buffer::testing::capture(Default::default(), || {
                execute::observe_witness(&witness, typed.entry(), limits, ExecutionPolicy::Bounded)
            });
        assert_eq!(success, Ok(owned::Scalar::I32(42)));
        let exact = limits.fuel - remaining;
        for fuel in [exact, exact - 1, exact / 2] {
            let ((result, _), seen) = byte_buffer::testing::capture(Default::default(), || {
                execute::observe_witness(
                    &witness,
                    typed.entry(),
                    execute::Limits { fuel, ..limits },
                    ExecutionPolicy::Bounded,
                )
            });
            assert_eq!(result == Ok(owned::Scalar::I32(42)), fuel == exact);
            assert_eq!(seen.after_cleanup.live_allocations, 0);
            assert_eq!(seen.after_cleanup.live_bytes, 0);
            assert_eq!(seen.after_cleanup.allocations, seen.after_cleanup.releases);
        }
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            for skip in 0..baseline.before_cleanup.allocations as usize {
                let control = byte_buffer::testing::Control {
                    fault: Some((byte_buffer::Boundary::Allocate, skip)),
                    ..Default::default()
                };
                let ((result, _), seen) = byte_buffer::testing::capture(control, || {
                    execute::observe_witness(&witness, typed.entry(), limits, policy)
                });
                assert!(seen.fault_applied, "{policy:?}/{skip}");
                assert_ne!(result, Ok(owned::Scalar::I32(42)), "{policy:?}/{skip}");
                assert_eq!(seen.after_cleanup.live_allocations, 0);
                assert_eq!(seen.after_cleanup.live_bytes, 0);
                assert_eq!(seen.after_cleanup.allocations, seen.after_cleanup.releases);
            }
        }
    });
}

#[test]
fn owning_record_element_local_loans_keep_children_after_optional_growth_is_refused() {
    with_typed(cases::CHILDREN, |typed, _, _| {
        let witness = witness(typed);
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            // Outer storage, three child stores, two child growths, then the
            // optional outer growth. Failure leaves the live first cell intact.
            let control = byte_buffer::testing::Control {
                fault: Some((byte_buffer::Boundary::Allocate, 6)),
                ..Default::default()
            };
            let ((result, _), seen) = byte_buffer::testing::capture(control, || {
                execute::observe_witness(
                    &witness,
                    typed.entry(),
                    execute::Limits::default(),
                    policy,
                )
            });
            assert!(seen.fault_applied, "{policy:?}");
            assert_eq!(result, Ok(owned::Scalar::I32(42)), "{policy:?}");
            assert_eq!(seen.after_cleanup.live_allocations, 0);
            assert_eq!(seen.after_cleanup.live_bytes, 0);
            assert_eq!(seen.after_cleanup.allocations, seen.after_cleanup.releases);
        }
    });
}
#[test]
fn owning_record_element_local_loans_check_empty_negative_and_end_indices_before_view_publication()
{
    for (body, index) in [
        (cases::BASE.replace("__ACTION__", ""), "-1"),
        (cases::BASE.replace("__ACTION__", ""), "2"),
        (
            cases::BASE
                .replace("__ACTION__", "")
                .replace(
                    "push_reserved(&mut values,Entry{value:42,bytes:bytes_new()});",
                    "",
                )
                .replace(
                    "push_reserved(&mut values,Entry{value:7,bytes:bytes_new()});",
                    "",
                ),
            "0",
        ),
    ] {
        let source = body.replace(
            "let item=&values[0];",
            &format!("let item=&values[{index}];"),
        );
        with_typed(&source, |typed, _, _| {
            let witness = witness(typed);
            for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
                let ((result, _), seen) = byte_buffer::testing::capture(Default::default(), || {
                    execute::observe_witness(
                        &witness,
                        typed.entry(),
                        execute::Limits::default(),
                        policy,
                    )
                });
                assert!(
                    matches!(result, Err(execute::OwnedRunFailure::Bounds(_))),
                    "{result:?}"
                );
                assert_eq!(seen.after_cleanup.live_allocations, 0);
                assert_eq!(seen.after_cleanup.allocations, seen.after_cleanup.releases);
            }
        });
    }
}
fn rejects(source: &str, reason: &str) {
    let fixture = Fixture::new(source);
    let project = fixture.project();
    let work = WorkMeter::default();
    let index = match index::collect_owned_vector_candidate(
        SourceOwner::project(&project),
        IndexLimits::default(),
        8 * 1024 * 1024,
        &work,
        &mut Allocator::default(),
    ) {
        Ok(index) => index,
        Err(errors) => {
            assert_eq!(errors[0].code, "E0300", "{errors:?}");
            assert!(
                errors[0].message.contains(reason),
                "expected {reason}: {errors:?}"
            );
            return;
        }
    };
    let typed = match resolve::type_owned_vector_source(&index, &work, &mut Allocator::default()) {
        Ok(typed) => typed,
        Err(errors) => {
            assert_eq!(errors[0].code, "E0300", "{errors:?}");
            assert!(
                errors[0].message.contains(reason),
                "expected {reason}: {errors:?}"
            );
            return;
        }
    };
    let associated = owned::source::association::lower_and_associate_owned_vector(&typed).unwrap();
    let error = verified::VerifiedOwnedVectorProgram::verify_associated(
        associated,
        owned::budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap_err();
    assert!(
        matches!(
            error.kind,
            owned::OwnedFailureKind::Ownership(owned::Violation::LoanConflict)
        ),
        "expected lexical conflict, found {error:?}"
    );
}
#[test]
fn owning_record_element_local_loans_pin_the_entire_vector_until_lexical_release() {
    for action in [
        "try_reserve(&mut values,1);",
        "push_reserved(&mut values,Entry{value:8,bytes:bytes_new()});",
        "let removed=pop_last(&mut values);",
        "truncate(&mut values,0);",
        "let moved=take(&mut values);",
        "consume(values);",
        "values=new::<Entry>();",
        "let other=&mut values[1];",
    ] {
        let source = cases::BASE.replace("__ACTION__", action);
        let reason = if action == "consume(values);" {
            "value is borrowed"
        } else if action == "values=new::<Entry>();" {
            "cannot replace a borrowed value"
        } else {
            "conflicting borrows of the same value"
        };
        rejects(&source, reason);
        // A near-identical neighbor executes the same operation after the
        // lexical element loan has ended. Reassignment/transfer then rebuilds
        // the vector only if the action consumed it.
        let neighbor = cases::BASE.replace("__ACTION__", "").replace(
            "    return 42;",
            &format!("    try_reserve(&mut values,1); {action} return 42;"),
        );
        run(&neighbor, 42);
    }
    rejects(
        &cases::BASE
            .replace("let item=&values[0];", "let item=&mut values[0];")
            .replace("__ACTION__", "let other=&values[1];"),
        "conflicting borrows of the same value",
    );
}
#[test]
fn owning_record_element_local_loans_keep_value_move_enum_and_opaque_element_paths_closed() {
    for source in [
        "struct E{bytes:ByteBuf}fn main()->i32{let values=new::<E>();let element=values[0];return 0;}",
        "struct E{bytes:ByteBuf}fn main()->i32{let mut values=new::<E>();values[0]=E{bytes:bytes_new()};return 0;}",
        "enum E{Bytes(ByteBuf)}fn main()->i32{let values=new::<E>();let item=&values[0];return 0;}",
        "fn inspect<T>(values:&Vec<T>)->(){let item=&*values[0];return;}fn main()->i32{return 0;}",
        "struct E{bytes:ByteBuf}fn main()->i32{let values=new::<E>();let item=&values[0];let moved=item;return 0;}",
        "#[derive(Copy)]struct E{value:i32}fn main()->i32{let values=new::<E>();let item=&values[0];return 0;}",
        "fn main()->i32{let values=new::<i32>();let item=&values[0];return 0;}",
    ] {
        let reason=if source.contains("let element=values[0]") || source.contains("values[0]=") { "owned vector elements must be moved with pop_last" }
            else if source.contains("let moved=item") { "a reference cannot be used as an owned value" }
            else { "local element references require a concrete non-Copy record vector" };
        rejects(source,reason);
    }
    run(cases::SIMPLE, 42);
}
#[test]
fn owning_record_element_local_loans_reject_raw_opcode_type_source_index_and_acquisition_forgeries()
{
    with_typed(cases::SIMPLE, |typed, sources, _| {
        for mutation in 0..9 {
            let mut raw = owned::source::lower::lower(typed).unwrap();
            let (function, block, position, loan) = raw
                .functions
                .iter()
                .enumerate()
                .find_map(|(function, row)| {
                    row.blocks.iter().enumerate().find_map(|(block, row)| {
                        row.statements
                            .iter()
                            .enumerate()
                            .find_map(|(position, row)| {
                                if let owned::OwnedInstruction::AcquireLocalElementBorrow {
                                    loan,
                                    ..
                                } = row.kind
                                {
                                    Some((function, block, position, loan))
                                } else {
                                    None
                                }
                            })
                    })
                })
                .unwrap();
            let f = &mut raw.functions[function];
            match mutation {
                0 => {
                    f.blocks[block].statements[position].kind =
                        owned::OwnedInstruction::AcquireLocalBorrow(loan)
                }
                1 => {
                    f.loans[loan.0].region = owned::LoanRegion::Call {
                        call: owned::CallSiteId(0),
                        argument: 0,
                    }
                }
                2 => {
                    f.loans[loan.0].referent = owned::BorrowedSlot::check(owned::BorrowedTy::Exact(
                        owned::AggregateTy::ByteBuf,
                    ))
                    .unwrap()
                }
                3 => {
                    let row = f.blocks[block].statements[position].clone();
                    f.blocks[block].statements.insert(position, row);
                }
                4 => {
                    let owned::OwnedInstruction::AcquireLocalElementBorrow { index, .. } =
                        &mut f.blocks[block].statements[position].kind
                    else {
                        unreachable!()
                    };
                    index.local.0 = usize::MAX;
                }
                5 => {
                    let owned::OwnedInstruction::AcquireLocalElementBorrow { index, .. } =
                        &mut f.blocks[block].statements[position].kind
                    else {
                        unreachable!()
                    };
                    f.locals[index.local.0].ty = crate::frontend::hir::Ty::Bool;
                }
                6 => {
                    let owned::OwnedInstruction::AcquireLocalElementBorrow { index, .. } =
                        f.blocks[block].statements[position].kind
                    else {
                        unreachable!()
                    };
                    f.blocks[block].statements.insert(
                        position,
                        owned::consumer_fixtures::assign(
                            index.local.0,
                            crate::frontend::oir::Rvalue::I32(1),
                            index.span,
                        ),
                    );
                }
                7 => f.loans[loan.0].projection.push(owned::FieldId {
                    record: owned::RecordId(0),
                    index: 0,
                }),
                8 => f.blocks[block].statements[position].span = f.span,
                _ => unreachable!(),
            }
            let imports = owned::owned_vector_descriptors::OwnedVectorImportClaims::empty()
                .admit(crate::frontend::builtin_catalog::owned_vector::Item::PushReserved)
                .admit(crate::frontend::builtin_catalog::owned_vector::Item::PopLast);
            let result = owned::owned_vector_descriptors::RawOwnedVectorProgram::prepare(
                raw,
                imports,
                typed.index().owned_vector_functions(),
                sources,
                owned::budget::Limits::DEFAULT,
                &mut Allocator::default(),
            )
            .and_then(|wrapper| {
                verified::VerifiedOwnedVectorProgram::verify(
                    wrapper,
                    sources,
                    owned::budget::Limits::DEFAULT,
                    &mut Allocator::default(),
                )
            });
            assert!(result.is_err(), "raw forgery {mutation}");
        }
    });
}
#[test]
fn owning_record_element_local_loans_pay_source_and_plan_storage_at_exact_and_one_less_limits() {
    with_typed(cases::SIMPLE, |typed, _, _| {
        let hir = owned::source::hir_budget::preflight_owned_vector_hir(
            typed.index(),
            &WorkMeter::default(),
        )
        .unwrap()
        .unwrap();
        assert!(hir.fixed >= super::local_element_carrier_bytes());
        assert!(
            owned::source::hir_budget::preflight_owned_vector_hir_with_limit(
                typed.index(),
                &WorkMeter::default(),
                hir.total
            )
            .is_ok()
        );
        assert!(
            owned::source::hir_budget::preflight_owned_vector_hir_with_limit(
                typed.index(),
                &WorkMeter::default(),
                hir.total - 1
            )
            .is_err()
        );
        let witness = witness(typed);
        let plan = owned::plan::ExecutionPlan::build(&witness).unwrap();
        let exact = plan.metadata_bytes();
        assert!(owned::plan::ExecutionPlan::build_with_test_limit(&witness, exact).is_ok());
        assert!(owned::plan::ExecutionPlan::build_with_test_limit(&witness, exact - 1).is_err());
    });
}
#[test]
fn owning_record_element_local_loans_emit_real_checked_native_lookup_and_preserve_helper_status_labels(
) {
    with_typed(cases::CHILDREN, |typed, sources, _| {
        let witness = witness(typed);
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let module = native::native_module_with_execution(
                &witness,
                typed.entry(),
                sources,
                false,
                policy,
                None,
            )
            .unwrap();
            assert!(module.contains("call i32 @oxid_owned_vector_borrow_record("));
            assert!(module.contains("_bounds_ok:"));
            assert!(module.contains("_identity_error:"));
            assert!(module.contains("_fuel_error:"));
            assert!(module.contains("@oxid_owned_vector_push_reserved("));
        }
    });
}
#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn owning_record_element_local_loans_stage_file_and_disjoint_fixed_bytes_and_close_once() {
    let fixture = Fixture::new("");
    let path = fs::canonicalize(&fixture.0).unwrap().join("file");
    let source = cases::FILE.replace(
        "__ELEMENT_FILE_PATH__",
        &format!("{:?}", path.to_str().unwrap()),
    );
    run(&source, 42);
    assert_eq!(fs::read(&path).unwrap(), [42, 0, 13, 10, 26, 255]);
    #[cfg(target_os = "linux")]
    for descriptor in fs::read_dir("/proc/self/fd").unwrap() {
        assert_ne!(
            fs::read_link(descriptor.unwrap().path()).ok().as_ref(),
            Some(&path)
        );
    }
}

#[test]
fn owning_record_element_empty_source_record_uses_its_checked_unit_storage_without_zero_stride() {
    run(cases::EMPTY_RECORD, 42);
}

#[test]
fn owning_record_element_execution_fixed_and_live_storage_have_a_real_exact_byte_boundary() {
    with_typed(cases::SIMPLE, |typed, _, _| {
        let witness = witness(typed);
        let limits = execute::Limits::default();
        let attempt = |bytes| {
            let ((result, _), seen) = byte_buffer::testing::capture(Default::default(), || {
                execute::observe_witness(
                    &witness,
                    typed.entry(),
                    execute::Limits { bytes, ..limits },
                    ExecutionPolicy::Bounded,
                )
            });
            assert_eq!(seen.after_cleanup.live_bytes, 0);
            assert_eq!(seen.after_cleanup.live_allocations, 0);
            assert_eq!(seen.after_cleanup.allocations, seen.after_cleanup.releases);
            match result {
                Ok(value) => {
                    assert_eq!(value, owned::Scalar::I32(42));
                    true
                }
                Err(execute::OwnedRunFailure::Resource(_)) => false,
                Err(error) => panic!("unexpected byte admission result: {error:?}"),
            }
        };
        assert!(attempt(limits.bytes));
        let (mut low, mut high) = (0, limits.bytes);
        while low < high {
            let middle = low + (high - low) / 2;
            if attempt(middle) {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        assert!(low > 0);
        assert!(attempt(low));
        assert!(!attempt(low - 1));
    });
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn owning_record_element_fixed_path_address_stays_live_while_a_distinct_file_is_registered() {
    let fixture = Fixture::new("");
    let path = fs::canonicalize(&fixture.0).unwrap().join("cell-path");
    let spelling = path.to_str().unwrap();
    let source=format!("use std::fs::create;use std::fs::OpenResult;use std::fs::close;struct Path{{bytes:[u8;{}]}}fn open(path:&Path)->i32{{let opened=create(&*path.bytes);match opened{{OpenResult::Opened(value)=>{{let mut file=value;close(&mut file);return 42;}},OpenResult::Error(code)=>{{return -1;}},OpenResult::MemoryLimit=>{{return -2;}},OpenResult::AllocationFailed=>{{return -3;}},}}}}fn main()->i32{{let mut values=new::<Path>();try_reserve(&mut values,1);push_reserved(&mut values,Path{{bytes:b{spelling:?}}});let item=&values[0];return open(&*item);}}",spelling.len());
    run(&source, 42);
    assert!(fs::read(path).unwrap().is_empty());
}

#[test]
fn owning_record_element_final_field_empty_end_slices_and_forwarded_reborrows_have_zero_extent() {
    run(cases::EMPTY_END_SLICES, 42);
}
#[test]
fn owning_record_element_empty_end_views_keep_original_bounds_and_order_refusals() {
    for (from, to) in [
        ("item.bytes[4..4]", "item.bytes[5..5]"),
        ("item.bytes[4..4]", "item.bytes[4..3]"),
        (
            "return empty(&*bytes[0..0]);",
            "return empty(&*bytes[0..1]);",
        ),
    ] {
        let source = cases::EMPTY_END_SLICES.replacen(from, to, 1);
        assert_ne!(source, cases::EMPTY_END_SLICES);
        with_typed(&source, |typed, _, _| {
            let witness = witness(typed);
            for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
                let ((result, _), seen) = byte_buffer::testing::capture(Default::default(), || {
                    execute::observe_witness(
                        &witness,
                        typed.entry(),
                        execute::Limits::default(),
                        policy,
                    )
                });
                assert!(
                    matches!(result, Err(execute::OwnedRunFailure::Bounds(_))),
                    "{result:?}"
                );
                assert_eq!(seen.after_cleanup.live_allocations, 0);
                assert_eq!(seen.after_cleanup.allocations, seen.after_cleanup.releases);
            }
        });
    }
}
