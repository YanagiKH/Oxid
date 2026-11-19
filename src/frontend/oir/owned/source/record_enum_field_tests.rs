//! Independent source identities and cleanup for tagged record fields.
use super::super::*;
use super::{association, budget, lower, record_pattern_tests::with_typed, resolve};
use crate::frontend::{
    build_config::ExecutionPolicy, declaration_index::WorkMeter, project::budget::Allocator,
};

const RESOURCES: &str = "use alloc::bytes::ByteBuf;use alloc::bytes::new;use alloc::bytes::len;use alloc::bytes::try_push;enum Option<T>{None,Some(T)}struct Pair{first:Option<ByteBuf>,second:Option<ByteBuf>}fn make(n:u8)->ByteBuf{let mut value=new();try_push(&mut value,n);return value;}fn main()->i32{let mut pair=Pair{first:Option::<ByteBuf>::Some(make(7u8)),second:Option::<ByteBuf>::Some(make(9u8))};pair=Pair{first:Option::<ByteBuf>::Some(make(42u8)),second:Option::<ByteBuf>::None};let Pair{second:rest,first}=pair;match first{Option::None=>{return 1;},Option::Some(value)=>{return len(&value)+41;},}}";

#[test]
fn record_enum_fields_require_exact_source_nominals_even_with_identical_unused_layouts() {
    with_typed(
        "enum Left{V(i32)}enum Right{V(i32)}struct State{value:Left}fn main()->i32{return 42;}",
        |typed, sources| {
            let raw = lower::lower(typed).unwrap();
            let valid = verified::verify_owned(raw, sources).unwrap();
            assert!(valid
                .declarations()
                .has_enum_record_nesting(AggregateTy::Record(RecordId(0)))
                .unwrap());
            let mut substituted = lower::lower(typed).unwrap();
            substituted.records[0].fields[0].ty =
                ParameterTy::Value(ValueTy::Owned(AggregateTy::Enum(EnumId(1))));
            // Storage shape alone deliberately accepts the other declared nominal.
            verified::verify_owned(substituted, sources).unwrap();
            let mut substituted = lower::lower(typed).unwrap();
            substituted.records[0].fields[0].ty =
                ParameterTy::Value(ValueTy::Owned(AggregateTy::Enum(EnumId(1))));
            assert!(association::associate(substituted, typed).is_err());
            let mut invalid = lower::lower(typed).unwrap();
            invalid.records[0].fields[0].ty =
                ParameterTy::Value(ValueTy::Owned(AggregateTy::Enum(EnumId(usize::MAX))));
            assert!(verified::verify_owned(invalid, sources).is_err());
            let mut cyclic = lower::lower(typed).unwrap();
            cyclic.enums[0].variants[0].payload = Some(ParameterTy::Value(ValueTy::Owned(
                AggregateTy::Record(RecordId(0)),
            )));
            assert!(verified::verify_owned(cyclic, sources).is_err());
        },
    );
}

#[test]
fn record_enum_fields_release_active_resources_at_every_fuel_boundary() {
    let checked = super::resource_fixtures::checked_enum_project(RESOURCES);
    for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        let mut completed = false;
        for fuel in 0..1600 {
            let (result, ledger) = byte_buffer::testing::capture(Default::default(), || {
                execute::run_result(
                    &checked.witness,
                    Some(checked.entry),
                    execute::Limits {
                        fuel,
                        ..Default::default()
                    },
                    policy,
                )
            });
            assert_eq!(
                ledger.after_cleanup.live_allocations, 0,
                "{policy:?} fuel={fuel}"
            );
            assert_eq!(ledger.after_cleanup.live_bytes, 0);
            assert_eq!(
                ledger.after_cleanup.allocations,
                ledger.after_cleanup.releases
            );
            match result {
                Ok(value) => {
                    assert_eq!(value.to_string(), "42");
                    assert_eq!(ledger.before_cleanup.live_allocations, 0);
                    assert_eq!(ledger.after_cleanup.allocations, 3);
                    completed = true;
                    break;
                }
                Err(error) => assert_eq!(error.diagnostic(checked.project.sources()).code, "E0601"),
            }
        }
        assert!(completed);
    }
}

#[test]
fn record_enum_fields_drop_resources_after_partial_construction_and_later_traps() {
    let declarations = RESOURCES.split("fn main").next().unwrap();
    for body in [
        "let pair=Pair{first:Option::<ByteBuf>::Some(make(7u8)),second:Option::<ByteBuf>::Some(fail())};return 0;",
        "let pair=Pair{first:Option::<ByteBuf>::Some(make(7u8)),second:Option::<ByteBuf>::Some(make(9u8))};let zero=0;return 1/zero;",
    ] {
        let text = format!("{declarations}fn fail()->ByteBuf{{let zero=0;let n=1/zero;return new();}}fn main()->i32{{{body}}}");
        let checked = super::resource_fixtures::checked_enum_project(&text);
        for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
            let (result, ledger) = byte_buffer::testing::capture(Default::default(), || execute::run_result(&checked.witness, Some(checked.entry), Default::default(), policy));
            assert_eq!(result.unwrap_err().diagnostic(checked.project.sources()).code, "E0607");
            assert!(ledger.after_cleanup.allocations > 0);
            assert_eq!(ledger.after_cleanup.live_allocations, 0);
            assert_eq!(ledger.after_cleanup.allocations, ledger.after_cleanup.releases);
        }
    }
}

#[test]
fn record_enum_fields_reclaim_paid_source_and_raw_storage_after_allocation_failures() {
    with_typed(RESOURCES, |typed, sources| {
        let index = typed.index();
        let mut baseline = Allocator::default();
        drop(resolve::type_enum_source(index, &WorkMeter::default(), &mut baseline).unwrap());
        for failure in 1..=baseline.attempts {
            let mut allocator = Allocator {
                fail_at: Some(failure),
                ..Default::default()
            };
            allocator.observer_trace_bound(512).unwrap();
            let (refused, (_, live, _)) = super::reviewer_source::integration_measured(|| {
                match resolve::type_enum_source(index, &WorkMeter::default(), &mut allocator) {
                    Ok(typed) => {
                        drop(typed);
                        false
                    }
                    Err(errors) => errors.iter().any(|error| error.code == "E0400"),
                }
            });
            assert!(refused, "reservation {failure}");
            assert_eq!(live, 0, "reservation {failure}");
            assert!(!allocator.observer_trace_overflow);
        }
        let mut completed = false;
        for failure in 0..400 {
            let (result, (_, live, _)) = super::reviewer_source::integration_measured(|| {
                budget::fail_allocation_after(failure, || lower::lower(typed))
            });
            match result {
                Err(error) => {
                    assert_eq!(
                        error.kind,
                        OwnedFailureKind::Resource("injected source allocation failure")
                    );
                    // The returned failure contains no owned payload.
                    assert_eq!(live, 0);
                }
                Ok(raw) => {
                    verified::verify_owned(raw, sources).unwrap();
                    completed = true;
                    break;
                }
            }
        }
        assert!(completed);
    });
}
