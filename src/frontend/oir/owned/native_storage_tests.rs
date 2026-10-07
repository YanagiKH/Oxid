use super::super::super::{
    builtin_input_fixtures as input, builtin_output_fixtures as output,
    consumer_fixtures as fixtures, native,
};
use super::*;

#[test]
fn native_storage_fixed_carriers_and_existing_plan_layout() {
    println!("native storage sizes plan={} function={} plan_result={} function_result={} slice_iter={} slice_item={} slice_option={}",
        size_of::<NativeStoragePlan<'_, '_>>(), size_of::<NativeFunctionStorage<'_, '_>>(),
        size_of::<Result<NativeStoragePlan<'_, '_>, AdmissionFailure>>(),
        size_of::<Result<NativeFunctionStorage<'_, '_>, AdmissionFailure>>(),
        size_of::<NativeSliceMappings<'_>>(), size_of::<NativeSliceMapping>(), size_of::<Option<NativeSliceMapping>>());
    assert_eq!(size_of::<NativeStoragePlan<'_, '_>>(), 16);
    assert_eq!(size_of::<NativeFunctionStorage<'_, '_>>(), 48);
    assert_eq!(
        size_of::<Result<NativeStoragePlan<'_, '_>, AdmissionFailure>>(),
        48
    );
    assert_eq!(
        size_of::<Result<NativeFunctionStorage<'_, '_>, AdmissionFailure>>(),
        56
    );
    assert_eq!(size_of::<NativeSliceMappings<'_>>(), 24);
    assert_eq!(size_of::<NativeSliceMapping>(), 32);
    assert_eq!(size_of::<FunctionPlan>(), 184);
    assert_eq!(size_of::<CallPlan>(), 48);
    assert_eq!(size_of::<FrameUsage>(), 88);
}

#[test]
fn native_storage_borrow_binds_same_shaped_distinct_witnesses() {
    let (sources_a, raw_a, _) = fixtures::owned_relay();
    let (sources_b, raw_b, _) = fixtures::owned_relay();
    let witness_a = verified::verify_owned(raw_a, &sources_a).unwrap();
    let witness_b = verified::verify_owned(raw_b, &sources_b).unwrap();
    let a = ExecutionPlan::build(&witness_a).unwrap();
    let b = ExecutionPlan::build(&witness_b).unwrap();
    for (plan, other) in [(&a, &b), (&b, &a)] {
        let storage = NativeStoragePlan::checked(plan, false).unwrap();
        assert!(std::ptr::eq(storage.execution(), plan));
        assert!(!std::ptr::eq(storage.execution(), other));
        for f in plan.witness().functions() {
            let function = storage.function(f.id);
            assert!(std::ptr::eq(function.raw(), f));
            assert!(std::ptr::eq(function.execution(), plan));
            assert!(!std::ptr::eq(
                function.execution().witness(),
                other.witness()
            ));
        }
    }
}

#[test]
fn native_storage_rejects_private_table_and_extent_corruption() {
    type Mutation = fn(&mut ExecutionPlan<'_>);
    for mutate in [
        (|p| {
            p.functions.pop();
        }) as Mutation,
        |p| {
            p.functions
                .iter_mut()
                .find(|f| !f.owner_offsets.is_empty())
                .unwrap()
                .owner_offsets
                .clear();
        },
        |p| {
            p.functions
                .iter_mut()
                .find(|f| !f.owner_offsets.is_empty())
                .unwrap()
                .owner_offsets[0] += 1;
        },
        |p| {
            p.functions
                .iter_mut()
                .find(|f| !f.calls.is_empty())
                .unwrap()
                .calls
                .pop();
        },
        |p| {
            p.functions
                .iter_mut()
                .find(|f| !f.calls.is_empty())
                .unwrap()
                .calls[0]
                .argument_start += 1;
        },
        |p| {
            p.functions
                .iter_mut()
                .find(|f| !f.calls.is_empty())
                .unwrap()
                .calls[0]
                .owned_len = usize::MAX;
        },
        |p| {
            p.functions
                .iter_mut()
                .find(|f| !f.owned_stages.is_empty())
                .unwrap()
                .owned_stages[0] = OwnerPlaceId(usize::MAX);
        },
        |p| {
            p.functions[0].usage.scalar_slots += 1;
        },
        |p| {
            p.functions[0].usage.arguments = usize::MAX;
        },
        |p| {
            p.functions[0].usage.payload_bytes = usize::MAX;
        },
        |p| {
            p.functions[0].usage.native_bytes += 1;
        },
        |p| {
            p.functions[0].usage.owner_cells += 1;
        },
        |p| {
            p.functions[0].usage.references += 1;
        },
        |p| {
            p.functions[0].usage.calls += 1;
        },
        |p| {
            p.functions[0].usage.expanded_cells += 1;
        },
        |p| {
            p.functions[0].usage.reference_bytes += 1;
        },
    ] {
        let (sources, raw, _) = fixtures::owned_relay();
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let mut plan = ExecutionPlan::build(&witness).unwrap();
        mutate(&mut plan);
        assert!(NativeStoragePlan::checked(&plan, false).is_err());
    }
    let (sources, raw, _) = fixtures::shared_read();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let mut plan = ExecutionPlan::build(&witness).unwrap();
    let function = plan
        .functions
        .iter_mut()
        .find(|p| !p.borrowed_loans.is_empty())
        .unwrap();
    function.borrowed_loans[0] = LoanId(usize::MAX);
    assert!(NativeStoragePlan::checked(&plan, false).is_err());
}

#[test]
fn native_storage_rejects_short_arenas_and_builtin_overlap() {
    for output_case in [false, true] {
        let (sources, raw, _) = if output_case {
            output::program(3)
        } else {
            input::program(3, input::Observation::Checksum)
        };
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let plan = ExecutionPlan::build(&witness).unwrap();
        let native = NativeStoragePlan::checked(&plan, true).unwrap();
        let id = if output_case {
            witness.builtin_output_function()
        } else {
            witness.builtin_function()
        }
        .unwrap();
        for arena in 0..4 {
            let mut storage = native.function(id);
            match arena {
                0 => storage.scalar_slots += 1,
                1 => storage.owner_bytes -= 1,
                2 => storage.reference_slots -= 1,
                _ => storage.slice_slots -= 1,
            }
            assert!(storage.check().is_err());
        }
        let mut plan = ExecutionPlan::build(&witness).unwrap();
        plan.functions[id.0].usage.payload_bytes -= 1;
        assert!(NativeStoragePlan::checked(&plan, true).is_err());
    }
}

/// Independent raw declaration census. It neither calls frame_usage nor reads
/// NativeFunctionStorage sizes to generate the expected LLVM text.
fn check_ir_census(sources: SourceMap, raw: RawOwnedProgram, entry: hir::DefId, process: bool) {
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let storage = NativeStoragePlan::checked(&plan, process).unwrap();
    let module = if process {
        native::native_process_module_with_fuel(&witness, entry, &sources, 100_000).unwrap()
    } else {
        native::native_module(&witness, Some(entry), &sources).unwrap()
    };
    let mut allocation_count = 0;
    for f in witness.functions() {
        let function_name = format!("@__oxid_owned_fn_{}(", f.id.0);
        let body = module
            .split("\ndefine internal ")
            .find(|part| {
                part.lines()
                    .next()
                    .is_some_and(|line| line.contains(&function_name))
            })
            .unwrap()
            .split("\n}\n")
            .next()
            .unwrap();
        let native = storage.function(f.id);
        let scalar = f.locals.len()
            + f.places.len()
            + f.calls.iter().map(|c| c.arguments.len()).sum::<usize>();
        let pointers = f.references.len() + f.loans.len();
        let mut end = 0;
        for (i, owner) in f.owners.iter().enumerate() {
            let layout = witness
                .declarations()
                .aggregate_layout(owner.aggregate())
                .unwrap();
            while end % layout.align() != 0 {
                end += 1;
            }
            assert!(body.contains(&format!(
                "%o{i} = getelementptr i8, ptr %owners, i64 {end}\n"
            )));
            assert_eq!(native.owner_offset(OwnerPlaceId(i)), end);
            end += layout.size();
        }
        for (builtin, name) in [
            (witness.builtin_function(), "input"),
            (witness.builtin_output_function(), "output"),
        ] {
            if builtin == Some(f.id) {
                assert!(body.contains(&format!(
                    "%{name}_scratch = getelementptr i8, ptr %owners, i64 {end}\n"
                )));
                end += 1024;
            }
        }
        while end % 4 != 0 {
            end += 1;
        }
        let mut slices = 0;
        for (name, declarations) in [
            (
                "rl",
                f.references
                    .iter()
                    .map(|r| r.referent())
                    .collect::<Vec<_>>(),
            ),
            ("ll", f.loans.iter().map(|l| l.referent()).collect()),
        ] {
            for (i, referent) in declarations.into_iter().enumerate() {
                let marker = format!("%{name}{i} = getelementptr");
                if matches!(referent, BorrowedTy::ScalarSlice(_)) {
                    assert!(body.contains(&format!(
                        "{marker} i8, ptr %slice_lengths, i64 {}\n",
                        slices * 4
                    )));
                    slices += 1;
                } else {
                    assert!(!body.contains(&marker));
                }
            }
        }
        for (name, count, ty, alignment) in [
            ("scalars", scalar, "i64", 8),
            ("owners", end, "i8", 4),
            ("references", pointers, "ptr", 8),
            ("slice_lengths", slices, "i32", 4),
        ] {
            if count != 0 {
                allocation_count += 1;
                assert!(body.contains(&format!(
                    "%{name} = alloca [{count} x {ty}], align {alignment}\n"
                )));
            } else {
                assert!(!body.contains(&format!("%{name} = alloca")));
            }
        }
        for (name, count, arena) in [("s", scalar, "scalars"), ("r", pointers, "references")] {
            for i in 0..count {
                assert!(body.contains(&format!(
                    "%{name}{i} = getelementptr i8, ptr %{arena}, i64 {}\n",
                    i * 8
                )));
            }
        }
        assert_eq!(
            native.native_bytes().unwrap(),
            scalar * 8 + end + pointers * 8 + slices * 4
        );
    }
    let fuel_allocations = module
        .lines()
        .filter(|l| l.trim() == "%fuel = alloca i64, align 8")
        .count();
    assert_eq!(fuel_allocations, usize::from(process));
    assert_eq!(
        module.lines().filter(|l| l.contains(" = alloca ")).count(),
        allocation_count + fuel_allocations
    );
    assert_eq!(storage.wrapper_fuel_bytes(), fuel_allocations * 8);
}

#[test]
fn native_storage_raw_plan_and_ir_census_agree() {
    for build in [
        fixtures::empty_record,
        fixtures::owned_relay,
        fixtures::shared_read,
        fixtures::distinct_owned_results,
    ] {
        let (sources, raw, schedule) = build();
        check_ir_census(sources, raw, schedule.entry, false);
    }
    for capacity in [0, 3] {
        let (sources, raw, entry) = input::program(capacity, input::Observation::Checksum);
        check_ir_census(sources, raw, entry, true);
        let (sources, raw, entry) = output::program(capacity);
        check_ir_census(sources, raw, entry, true);
    }
    for build in [
        input::projected_record_program,
        output::projected_program,
        output::forwarded_program,
    ] {
        let (sources, raw, entry) = build();
        check_ir_census(sources, raw, entry, true);
    }
}

#[test]
fn native_storage_mapping_identity_seed_and_no_new_reservation() {
    let (sources, raw, _) = input::projected_record_program();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let storage = fail_allocation_after(0, || NativeStoragePlan::checked(&plan, true)).unwrap();
    let id = witness.builtin_function().unwrap();
    let function = storage.function(id);
    let mut shifted = function.slice_mappings();
    shifted.pointer = 1;
    assert!(function.check_slice_mappings(shifted).is_err());
    let mut offset = function.slice_mappings();
    offset.length = usize::MAX;
    assert!(function.check_slice_mappings(offset).is_err());
    let wrong_function = storage.function(hir::DefId((id.0 + 1) % witness.functions().len()));
    assert!(function
        .check_slice_mappings(wrong_function.slice_mappings())
        .is_err());
}

#[test]
fn native_storage_named_fixed_roles_fit_existing_transient_partition() {
    let (sources, raw, _) = fixtures::owned_relay();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let execution = ExecutionPlan::build(&witness).unwrap();
    let storage = NativeStoragePlan::checked(&execution, false).unwrap();
    let function = storage.function(hir::DefId(0));
    let raw = function.raw();
    let fp = execution.function(raw.id);
    // Sum even disjoint return/iterator lifetimes conservatively. The 32 scalar
    // words cover the new checker's bounded counters/references/temporary sizes.
    // Inherited formatter internals, diagnostics and machine stack are outside
    // this named fixed-carrier model, as in the original emitter envelope.
    let roles = [
        size_of::<NativeStoragePlan<'_, '_>>(),
        size_of::<NativeFunctionStorage<'_, '_>>(),
        size_of::<Result<NativeStoragePlan<'_, '_>, AdmissionFailure>>(),
        size_of::<Result<NativeFunctionStorage<'_, '_>, AdmissionFailure>>(),
        size_of::<Result<(), AdmissionFailure>>(),
        size_of::<NativeSliceMappings<'_>>(),
        size_of::<NativeSliceMapping>(),
        size_of::<Option<NativeSliceMapping>>(),
        size_of::<[(bool, usize, Option<Range<usize>>); 2]>(),
        size_of::<std::array::IntoIter<(bool, usize, Option<Range<usize>>), 2>>(),
        std::mem::size_of_val(&witness.functions().iter().enumerate()),
        std::mem::size_of_val(&raw.owners.iter().enumerate()),
        std::mem::size_of_val(&raw.calls.iter().zip(&fp.calls)),
        size_of::<std::slice::Iter<'_, ArgumentSlot>>(),
        std::mem::size_of_val(
            &raw.references
                .iter()
                .map(|r| r.referent())
                .chain(raw.loans.iter().map(|l| l.referent())),
        ),
        size_of::<[usize; 32]>(),
    ];
    let total: usize = roles.iter().sum();
    println!("native storage fixed roles {roles:?}; conservative total {total}; allowance {FIXED_CARRIER_ALLOWANCE}");
    assert!(total <= FIXED_CARRIER_ALLOWANCE);
}
