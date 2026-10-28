//! Independent emitted schema, frame storage and closed native dispatch oracles.
use super::*;
use crate::frontend::{build_config::ExecutionPolicy, builtin_catalog::vector::Operation};
use source::resource_fixtures::checked_enum_project;

const POINT: &str = "use alloc::vec::new;use alloc::vec::try_push;#[derive(Copy)]struct P{valid:bool,number:i32,glyph:char,end:()}fn main()->i32{let p=P{valid:true,number:21,glyph:'界',end:()};let mut v=new::<P>();try_push(&mut v,p);let q=v[0];return p.number+q.number;}";

#[test]
fn record_vector_native_catalog_maps_initialized_leaves_without_owner_padding() {
    let checked = checked_enum_project(POINT);
    let (sources, witness, entry) = (checked.project.sources(), &checked.witness, checked.entry);
    let observed = run_array_observed(witness, Some(entry), sources, NativeControl::default());
    let module = observed.result.unwrap();
    assert_eq!(observed.metrics.count_bytes, module.len());
    assert_eq!(observed.metrics.render_bytes, module.len());
    assert_eq!(
        observed.metrics.count_ordinary_visits,
        observed.metrics.render_ordinary_visits
    );
    assert_eq!(
        observed.metrics.count_expansion_kinds,
        observed.metrics.render_expansion_kinds
    );
    assert!(module.contains("@__oxid_record_vector_schema_count = constant i32 1, align 4"));
    for leaf in [
        "{ i32 0, i32 0, i32 1 }",
        "{ i32 1, i32 4, i32 7 }",
        "{ i32 5, i32 8, i32 15 }",
        "{ i32 9, i32 12, i32 2 }",
    ] {
        assert!(module.contains(leaf), "missing literal leaf {leaf}");
    }
    assert!(module
        .contains("i32 2147483648, i32 10, i32 16, i32 4, ptr @__oxid_record_vector_leaves_0"));
    assert!(module.contains("call i32 @__oxid_record_vector_pack(i32 2147483648"));
    assert!(module.contains("call i32 @__oxid_record_vector_unpack(i32 2147483648"));
    for forbidden in [
        "inbounds",
        "noalias",
        "nonnull",
        "dereferenceable",
        "undef",
        "poison",
    ] {
        assert!(!module.contains(forbidden), "{forbidden}");
    }
}
#[test]
fn record_vector_staging_is_paid_outside_all_owners_with_literal_frame_oracles() {
    let checked = checked_enum_project(POINT);
    let witness = &checked.witness;
    let plan = ExecutionPlan::build(witness).unwrap();
    assert!(plan.uses_record_vectors());
    let f = witness
        .functions()
        .iter()
        .find(|f| {
            witness
                .vector_operation(f.id)
                .is_some_and(|key| key.operation() == Operation::TryPush)
        })
        .unwrap();
    let usage = plan.function(f.id).usage();
    // P uses16 bytes. The inherited scalar/nullary enum ABI keeps an8-byte
    // tag/reserved-payload carrier and2 logical cells even without a payload.
    // Add one dense10-byte record value outside both language owner extents.
    assert_eq!(usage.payload_bytes, 34);
    assert_eq!(plan.function(f.id).owner_offset(OwnerPlaceId(0)), 0);
    assert_eq!(plan.function(f.id).owner_offset(OwnerPlaceId(1)), 16);
    assert_eq!(plan.record_vector_scratch_range(f.id), Some(24..34));
    assert_eq!((usage.owner_cells, usage.expanded_cells), (6, 24));
    assert_eq!(usage.reference_bytes, 34 + 2 * 32 + 80);
    assert_eq!(usage.native_bytes, 8 + 36); // reference pointer + align4(payload34)
    assert_eq!(
        plan.statement_cost(f.id, &f.blocks[0].statements[1].kind),
        528
    ); // 4+131*4
    let storage = NativeStoragePlan::checked(&plan, true).unwrap();
    assert_eq!(storage.function(f.id).native_bytes().unwrap(), 44);
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(std::mem::size_of::<ExecutionPlan<'_>>(), 40);
        // Independent 64-bit C control rows: three16-byte schema lookups,
        // layout48, canonical64, source96, cell72, transfer104, write32, slice48.
        assert_eq!(
            vector::RECORD_RUNTIME_BYTES,
            3 * 16 + 48 + 64 + 96 + 72 + 104 + 32 + 48
        );
    }
}
#[test]
fn scalar_vectors_do_not_acquire_record_catalogs_or_record_only_frame_scratch() {
    let checked=checked_enum_project("use alloc::vec::new;use alloc::vec::try_push;#[derive(Copy)]struct Unused{x:i32}fn main()->i32{let mut v=new::<i32>();try_push(&mut v,42);return v[0];}");
    let (sources, witness, entry) = (checked.project.sources(), &checked.witness, checked.entry);
    let plan = ExecutionPlan::build(witness).unwrap();
    assert!(plan.uses_vectors());
    assert!(!plan.uses_record_vectors());
    for f in witness.functions() {
        assert_eq!(plan.record_vector_scratch_range(f.id), None);
    }
    let module = native_module(witness, Some(entry), sources).unwrap();
    assert!(!module.contains("@__oxid_record_vector_schema_count = constant"));
    assert!(!module.contains("%record_vector_scratch ="));
}
#[test]
fn record_slice_reads_and_writes_have_distinct_nominal_native_helpers() {
    let text="use alloc::vec::new;use alloc::vec::try_push;#[derive(Copy)]struct P{x:i32}fn first<T:Copy>(xs:&[T])->T{return xs[0];}fn set<T:Copy>(xs:&mut[T],p:T)->(){xs[0]=p;return;}fn main()->i32{let p=P{x:20};let mut v=new::<P>();try_push(&mut v,p);let before=first(&v);set(&mut v[0..1],P{x:22});let after=first(&v);return before.x+after.x;}";
    let checked = checked_enum_project(text);
    let (sources, witness, entry) = (checked.project.sources(), &checked.witness, checked.entry);
    for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        let module =
            native_module_with_execution(witness, Some(entry), sources, false, policy, None)
                .unwrap();
        assert!(module.contains("call i32 @__oxid_record_vector_index_slice"));
        assert!(module.contains("call i32 @__oxid_record_vector_write"));
        assert!(module.contains("%slice_lengths = alloca"));
        assert!(module.contains("%record_vector_scratch ="));
    }
}
