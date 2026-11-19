//! Source-backed emission controls; runtime behavior has separate native tests.
use super::*;
use crate::frontend::build_config::ExecutionPolicy;
use source::resource_fixtures::checked_enum_project;

const APPEND: &str = "use alloc::vec::new;use alloc::vec::try_push;fn main()->i32{let mut v=new::<i32>();try_push(&mut v,42);return v[0];}";

#[test]
fn vector_primitives_emit_complete_diagnostics_under_both_execution_policies() {
    let checked = checked_enum_project(APPEND);
    for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        let module = native_module_with_execution(
            &checked.witness,
            Some(checked.entry),
            checked.project.sources(),
            false,
            policy,
            None,
        )
        .unwrap();
        assert!(module.contains("@__oxid_vector_index"));
        assert!(module.contains("call i32 @__oxid_vector_operation"));
        assert!(module.contains("%vector_index_pointer = alloca ptr"));
    }
}

#[test]
fn unused_vector_fields_still_admit_the_composed_resource_helpers() {
    let checked = checked_enum_project(
        "use alloc::vec::Vec;struct Unused{values:Vec<i32>}fn main()->i32{return 0;}",
    );
    let plan = ExecutionPlan::build(&checked.witness).unwrap();
    assert!(plan.uses_vectors());
    assert_eq!(checked.witness.functions().len(), 1);
    let module = native_module_with_execution(
        &checked.witness,
        Some(checked.entry),
        checked.project.sources(),
        false,
        ExecutionPolicy::Hosted,
        None,
    )
    .unwrap();
    assert!(module.contains("declare i32 @__oxid_vector_valid"));
    assert!(module.contains("declare void @__oxid_vector_drop"));
}

#[test]
fn vector_index_scratch_is_reused_and_scalar_ranges_need_no_pointer_slot() {
    for (text, expected) in [
        ("use alloc::vec::new;use alloc::vec::len;fn main()->i32{let v=new::<i32>();return len(&v);}", 0),
        (APPEND, std::mem::size_of::<*mut u8>()),
        ("use alloc::vec::new;use alloc::vec::try_push;fn main()->i32{let mut v=new::<i32>();try_push(&mut v,21);v[0]=42;let first=v[0];let second=v[0];return first+second;}", std::mem::size_of::<*mut u8>()),
        ("fn first(p:&[i32])->i32{return p[0];}fn main()->i32{let values=[1,42];return first(&values[1..2]);}", 0),
    ] {
        let checked = checked_enum_project(text);
        let plan = ExecutionPlan::build(&checked.witness).unwrap();
        let function = &checked.witness.functions()[checked.entry.0];
        assert_eq!(plan::native_vector_layout::frame_bytes(checked.witness.declarations(), function).unwrap(), expected);
        let storage = NativeStoragePlan::checked(&plan, true).unwrap();
        assert_eq!(storage.function(checked.entry).native_bytes().unwrap(), plan.function(checked.entry).usage().native_bytes);
    }
}
