//! Synthetic T0 helpers only: no resolver/typechecker/owner/seed transfer.
use super::*;
use crate::frontend::source::SourceFileId;
use std::mem::align_of;

fn origin() -> Span {
    Span {
        file: SourceFileId(0),
        start: 0,
        end: 1,
    }
}
fn one() -> TypeCounts {
    TypeCounts {
        functions: 1,
        bindings: 1,
        expressions: 1,
        blocks: 1,
        statements: 1,
        borrow_arguments: 1,
        type_frames: 1,
        call_arguments: 1,
        calls: 1,
        presence_slots: 1,
        record_literals: 1,
    }
}

#[test]
fn c3_t0_all_fourteen_concrete_kinds_have_exact_consumable_slots_and_requests() {
    let at = origin();
    let mut storage = PaidStorage::new(&one());
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(KINDS).unwrap();
    macro_rules! check {
        ($kind:ident, $ty:ty) => {{
            let values = storage
                .reserve::<$ty>(&mut allocator, Kind::$kind, 1, 1, at)
                .unwrap();
            assert_eq!(values.capacity(), 1);
            assert_eq!(values.len(), 0);
            let before = allocator.attempts;
            assert_eq!(
                storage
                    .reserve::<$ty>(&mut allocator, Kind::$kind, 0, 0, at)
                    .err()
                    .unwrap()
                    .code,
                "E0400"
            );
            assert_eq!(allocator.attempts, before);
        }};
    }
    check!(Bodies, TypedBody);
    check!(BindingStage, Option<ParameterTy>);
    check!(ExpressionStage, Option<ValueTy>);
    check!(FlowStage, Option<FlowSummary>);
    check!(ExpressionProjections, Option<Projection>);
    check!(StatementRows, Vec<Option<Projection>>);
    check!(StatementProjections, Option<Projection>);
    check!(BorrowProjections, BorrowProjection);
    check!(TypeFrames, TypeFrame);
    check!(CallActuals, (ParameterTy, Span));
    check!(RecordPresence, bool);
    check!(BindingFinal, ParameterTy);
    check!(FlowFinal, FlowSummary);
    check!(ExpressionFinal, ValueTy);
    assert_eq!(storage.slots, [0; KINDS]);
    assert_eq!(storage.requests, [0; KINDS]);
    assert_eq!(allocator.attempts, KINDS);
    for (event, width) in allocator.trace.iter().zip(WIDTHS) {
        assert_eq!(
            (event.length, event.element_bytes, event.success),
            (1, width, true)
        );
    }
    assert!(!allocator.observer_trace_overflow);
}

#[test]
fn c3_t0_empty_requests_are_bounded_independently_of_unspent_slots() {
    let at = origin();
    let mut storage = PaidStorage::new(&one());
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(KINDS).unwrap();
    macro_rules! check {
        ($kind:ident, $ty:ty) => {{
            let values = storage
                .reserve::<$ty>(&mut allocator, Kind::$kind, 0, 0, at)
                .unwrap();
            assert_eq!(values.capacity(), 0);
            assert_eq!(storage.slots[Kind::$kind as usize], 1);
            assert_eq!(storage.requests[Kind::$kind as usize], 0);
            let before = allocator.attempts;
            assert!(storage
                .reserve::<$ty>(&mut allocator, Kind::$kind, 1, 1, at)
                .is_err());
            assert_eq!(allocator.attempts, before);
        }};
    }
    check!(Bodies, TypedBody);
    check!(BindingStage, Option<ParameterTy>);
    check!(ExpressionStage, Option<ValueTy>);
    check!(FlowStage, Option<FlowSummary>);
    check!(ExpressionProjections, Option<Projection>);
    check!(StatementRows, Vec<Option<Projection>>);
    check!(StatementProjections, Option<Projection>);
    check!(BorrowProjections, BorrowProjection);
    check!(TypeFrames, TypeFrame);
    check!(CallActuals, (ParameterTy, Span));
    check!(RecordPresence, bool);
    check!(BindingFinal, ParameterTy);
    check!(FlowFinal, FlowSummary);
    check!(ExpressionFinal, ValueTy);
    assert_eq!(storage.slots, [1; KINDS]);
    assert_eq!(storage.requests, [0; KINDS]);
    assert_eq!(allocator.attempts, KINDS);
    assert!(allocator
        .trace
        .iter()
        .all(|event| event.length == 0 && event.success));
}

#[test]
fn c3_t0_program_request_multiplicities_are_not_one_quota_per_function_copy() {
    let counts = TypeCounts {
        functions: 2,
        blocks: 5,
        calls: 3,
        record_literals: 4,
        call_arguments: 7,
        presence_slots: 9,
        ..TypeCounts::default()
    };
    let mut storage = PaidStorage::new(&counts);
    assert_eq!(storage.requests, [1, 2, 2, 2, 2, 2, 5, 2, 2, 3, 4, 2, 2, 2]);
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(5).unwrap();
    for _ in 0..5 {
        storage
            .reserve::<Option<Projection>>(
                &mut allocator,
                Kind::StatementProjections,
                0,
                0,
                origin(),
            )
            .unwrap();
    }
    assert!(storage
        .reserve::<Option<Projection>>(&mut allocator, Kind::StatementProjections, 0, 0, origin())
        .is_err());
    assert_eq!(allocator.attempts, 5);
    assert_eq!(storage.requests[Kind::StatementProjections as usize], 0);
    // Another kind's requests cannot replenish the exhausted block rows.
    assert_eq!(storage.requests[Kind::CallActuals as usize], 3);
}

#[test]
fn c3_t0_invalid_local_size_width_and_arithmetic_are_atomic_before_reserve() {
    let at = origin();
    let mut storage = PaidStorage::new(&one());
    let mut allocator = Allocator::default();
    let slots_before = storage.slots;
    let requests_before = storage.requests;
    assert_eq!(
        storage
            .reserve::<ValueTy>(&mut allocator, Kind::ExpressionFinal, 1, 0, at)
            .err()
            .unwrap()
            .code,
        "E0500"
    );
    assert_eq!(
        storage
            .reserve::<u8>(&mut allocator, Kind::ExpressionFinal, 1, 1, at)
            .err()
            .unwrap()
            .code,
        "E0500"
    );
    assert_eq!(
        storage
            .reserve::<ValueTy>(&mut allocator, Kind::ExpressionFinal, 2, 2, at)
            .err()
            .unwrap()
            .code,
        "E0400"
    );
    assert_eq!(
        (storage.slots, storage.requests),
        (slots_before, requests_before)
    );
    assert_eq!(allocator.attempts, 0);

    let mut storage = PaidStorage::new(&TypeCounts {
        expressions: usize::MAX,
        functions: 1,
        ..TypeCounts::default()
    });
    let before = (storage.slots, storage.requests);
    assert_eq!(
        storage
            .reserve::<ValueTy>(
                &mut allocator,
                Kind::ExpressionFinal,
                usize::MAX,
                usize::MAX,
                at
            )
            .err()
            .unwrap()
            .code,
        "E0400"
    );
    assert_eq!((storage.slots, storage.requests), before);
    assert_eq!(allocator.attempts, 0);
    let over_cap = MAX_HIR_BYTES / size_of::<ValueTy>() + 1;
    assert!(storage
        .reserve::<ValueTy>(
            &mut allocator,
            Kind::ExpressionFinal,
            over_cap,
            over_cap,
            at
        )
        .is_err());
    assert_eq!((storage.slots, storage.requests), before);
    assert_eq!(allocator.attempts, 0);
}

#[test]
fn c3_t0_each_concrete_kind_spends_its_rights_before_failed_reservation() {
    let at = origin();
    macro_rules! check {
        ($kind:ident, $ty:ty) => {{
            let mut storage = PaidStorage::new(&one());
            let mut allocator = Allocator {
                fail_at: Some(1),
                ..Allocator::default()
            };
            allocator.observer_trace_bound(1).unwrap();
            assert!(storage
                .reserve::<$ty>(&mut allocator, Kind::$kind, 1, 1, at)
                .is_err());
            assert_eq!(storage.slots[Kind::$kind as usize], 0);
            assert_eq!(storage.requests[Kind::$kind as usize], 0);
            assert!(storage
                .reserve::<$ty>(&mut allocator, Kind::$kind, 0, 0, at)
                .is_err());
            assert_eq!(allocator.attempts, 1);
            assert_eq!(allocator.trace.len(), 1);
            assert!(!allocator.trace[0].success);
        }};
    }
    check!(Bodies, TypedBody);
    check!(BindingStage, Option<ParameterTy>);
    check!(ExpressionStage, Option<ValueTy>);
    check!(FlowStage, Option<FlowSummary>);
    check!(ExpressionProjections, Option<Projection>);
    check!(StatementRows, Vec<Option<Projection>>);
    check!(StatementProjections, Option<Projection>);
    check!(BorrowProjections, BorrowProjection);
    check!(TypeFrames, TypeFrame);
    check!(CallActuals, (ParameterTy, Span));
    check!(RecordPresence, bool);
    check!(BindingFinal, ParameterTy);
    check!(FlowFinal, FlowSummary);
    check!(ExpressionFinal, ValueTy);
}

#[test]
fn c3_t0_none_initializes_noncopy_projection_slots_and_presence_without_growth() {
    let at = origin();
    let counts = TypeCounts {
        expressions: 3,
        statements: 2,
        presence_slots: 4,
        ..one()
    };
    let mut storage = PaidStorage::new(&counts);
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(3).unwrap();
    let projections = storage
        .none::<Projection>(&mut allocator, Kind::ExpressionProjections, 3, at)
        .unwrap();
    let statements = storage
        .none::<Projection>(&mut allocator, Kind::StatementProjections, 2, at)
        .unwrap();
    let presence = storage.presence(&mut allocator, 4, at).unwrap();
    assert_eq!((projections.len(), projections.capacity()), (3, 3));
    assert_eq!((statements.len(), statements.capacity()), (2, 2));
    assert_eq!((presence.len(), presence.capacity()), (4, 4));
    assert!(projections.iter().all(Option::is_none));
    assert!(statements.iter().all(Option::is_none));
    assert_eq!(presence, [false; 4]);
    assert_eq!(allocator.attempts, 3);
    let before = (storage.slots, storage.requests);
    assert!(storage
        .none::<Projection>(&mut allocator, Kind::StatementRows, 0, at)
        .is_err());
    assert_eq!((storage.slots, storage.requests), before);
    assert_eq!(allocator.attempts, 3);
}

#[test]
fn c3_t0_append_checks_exact_capacity_before_push_and_allows_pop_reuse() {
    let at = origin();
    let mut storage = PaidStorage::new(&TypeCounts {
        expressions: 2,
        ..one()
    });
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(1).unwrap();
    let mut values = storage
        .reserve::<ValueTy>(&mut allocator, Kind::ExpressionFinal, 2, 2, at)
        .unwrap();
    let value = ValueTy::Scalar(Ty::I32);
    for _ in 0..2 {
        room(&values, values.capacity(), 2, at).unwrap();
        values.push(value);
    }
    assert!(room(&values, values.capacity(), 2, at).is_err());
    assert_eq!(values.pop(), Some(value));
    room(&values, values.capacity(), 2, at).unwrap();
    values.push(value);
    assert_eq!(
        (values.len(), values.capacity(), allocator.attempts),
        (2, 2, 1)
    );
    assert!(room::<ValueTy>(&[], 3, 2, at).is_err());
    assert!(room::<ValueTy>(&[], 1, 2, at).is_err());
    assert!(room::<ValueTy>(&[], 0, 0, at).is_err());
    let ticket = Capacity::new::<ValueTy>(2, 2 * size_of::<ValueTy>(), at).unwrap();
    ticket.check_observed(2, at).unwrap();
    assert!(ticket.check_observed(3, at).is_err());
}

#[test]
fn c3_t0_copy_finalization_keeps_staging_live_and_does_not_reuse_its_buffer() {
    let at = origin();
    let mut storage = PaidStorage::new(&TypeCounts {
        bindings: 3,
        ..one()
    });
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(2).unwrap();
    let ((stage, values), stats) = super::super::reviewer_source::integration_measured(|| {
        let mut stage = storage
            .none::<ParameterTy>(&mut allocator, Kind::BindingStage, 3, at)
            .unwrap();
        stage[0] = Some(ParameterTy::Value(ValueTy::Scalar(Ty::I32)));
        stage[1] = Some(ParameterTy::Value(ValueTy::Scalar(Ty::Bool)));
        stage[2] = Some(ParameterTy::Value(ValueTy::Scalar(Ty::Unit)));
        let values = storage
            .finalize(&mut allocator, Kind::BindingFinal, &stage, at)
            .unwrap();
        (stage, values)
    });
    let expected = 3 * (size_of::<Option<ParameterTy>>() + size_of::<ParameterTy>());
    assert_eq!(stats, (2, expected as isize, expected as isize));
    assert_eq!(
        (
            stage.len(),
            stage.capacity(),
            values.len(),
            values.capacity()
        ),
        (3, 3, 3, 3)
    );
    assert_ne!(stage.as_ptr() as usize, values.as_ptr() as usize);
    assert_eq!(values[0], stage[0].unwrap());
    assert_eq!(values[1], stage[1].unwrap());
    assert_eq!(values[2], stage[2].unwrap());
    assert_eq!(allocator.attempts, 2);
}

#[test]
fn c3_t0_finalization_missing_slots_fail_closed_and_each_final_kind_is_supported() {
    let at = origin();
    let mut storage = PaidStorage::new(&one());
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(3).unwrap();
    let stage = [Some(ValueTy::Scalar(Ty::Unit))];
    assert_eq!(
        storage
            .finalize(&mut allocator, Kind::ExpressionFinal, &stage, at)
            .unwrap(),
        [ValueTy::Scalar(Ty::Unit)]
    );
    let missing: [Option<ParameterTy>; 1] = [None];
    assert_eq!(
        storage
            .finalize(&mut allocator, Kind::BindingFinal, &missing, at)
            .err()
            .unwrap()
            .code,
        "E0500"
    );
    assert_eq!(storage.slots[Kind::BindingFinal as usize], 0);
    assert_eq!(storage.requests[Kind::BindingFinal as usize], 0);
    let empty: [Option<FlowSummary>; 0] = [];
    assert!(storage
        .finalize(&mut allocator, Kind::FlowFinal, &empty, at)
        .unwrap()
        .is_empty());
    assert_eq!(storage.slots[Kind::FlowFinal as usize], 1);
    assert_eq!(storage.requests[Kind::FlowFinal as usize], 0);
    let before = (storage.slots, storage.requests);
    assert!(storage
        .finalize(&mut allocator, Kind::ExpressionStage, &stage, at)
        .is_err());
    assert_eq!((storage.slots, storage.requests), before);
    assert_eq!(allocator.attempts, 3);
}

#[test]
fn c3_t0_stage_and_final_success_or_either_failure_release_owned_allocations() {
    let at = origin();
    for fail_at in [None, Some(1), Some(2)] {
        let mut storage = PaidStorage::new(&TypeCounts {
            expressions: 2,
            ..one()
        });
        let mut allocator = Allocator {
            fail_at,
            ..Allocator::default()
        };
        allocator.observer_trace_bound(2).unwrap();
        let (_, stats) = super::super::reviewer_source::integration_measured(|| {
            let result = (|| {
                let mut stage =
                    storage.none::<ValueTy>(&mut allocator, Kind::ExpressionStage, 2, at)?;
                stage[0] = Some(ValueTy::Scalar(Ty::I32));
                stage[1] = Some(ValueTy::Scalar(Ty::Unit));
                let values = storage.finalize(&mut allocator, Kind::ExpressionFinal, &stage, at)?;
                assert_eq!(values.len(), 2);
                Ok::<_, Box<Diagnostic>>(())
            })();
            assert_eq!(result.is_ok(), fail_at.is_none());
            drop(result);
        });
        assert_eq!(stats.1, 0);
        assert_eq!(allocator.attempts, fail_at.unwrap_or(2));
        if fail_at.is_none() {
            assert_eq!(stats.0, 2);
            assert_eq!(
                stats.2,
                (2 * (size_of::<Option<ValueTy>>() + size_of::<ValueTy>())) as isize
            );
        }
    }
}

#[test]
fn c3_t0_projection_fields_share_existing_seed_and_charge_before_reserve() {
    let at = origin();
    let width = size_of::<FieldId>();
    let total = Cell::new(MAX_HIR_BYTES - 3 * width);
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(2).unwrap();
    let first = projection_fields(&total, &mut allocator, 1, at).unwrap();
    assert_eq!(first.capacity(), 1);
    assert_eq!(total.get(), MAX_HIR_BYTES - 2 * width);
    let second = projection_fields(&total, &mut allocator, 2, at).unwrap();
    assert_eq!(second.capacity(), 2);
    assert_eq!(total.get(), MAX_HIR_BYTES);
    drop((first, second));
    assert!(projection_fields(&total, &mut allocator, 1, at).is_err());
    assert_eq!(total.get(), MAX_HIR_BYTES);
    assert_eq!(allocator.attempts, 2);
    assert_eq!(allocator.trace[0].kind, "paid typed projection fields");
}

#[test]
fn c3_t0_projection_invalid_length_overflow_and_cap_failure_do_not_mutate() {
    let at = origin();
    let width = size_of::<FieldId>();
    let mut allocator = Allocator::default();
    for (seed, slots) in [
        (0, 0),
        (0, 65),
        (0, usize::MAX),
        (usize::MAX, 1),
        (MAX_HIR_BYTES - width + 1, 1),
    ] {
        let total = Cell::new(seed);
        assert!(projection_fields(&total, &mut allocator, slots, at).is_err());
        assert_eq!(total.get(), seed);
        assert_eq!(allocator.attempts, 0);
    }
    allocator.observer_trace_bound(1).unwrap();
    let total = Cell::new(MAX_HIR_BYTES - 64 * width);
    let path = projection_fields(&total, &mut allocator, 64, at).unwrap();
    assert_eq!(path.capacity(), 64);
    assert_eq!(total.get(), MAX_HIR_BYTES);
    assert_eq!(allocator.attempts, 1);
}

#[test]
fn c3_t0_projection_failed_reserve_keeps_spent_charge_and_all_buffers_drop() {
    let at = origin();
    let width = size_of::<FieldId>();
    for fail_at in [None, Some(1), Some(2)] {
        let total = Cell::new(MAX_HIR_BYTES - 3 * width);
        let mut allocator = Allocator {
            fail_at,
            ..Allocator::default()
        };
        allocator.observer_trace_bound(2).unwrap();
        let (_, stats) = super::super::reviewer_source::integration_measured(|| {
            let result = (|| {
                let first = projection_fields(&total, &mut allocator, 1, at)?;
                let second = projection_fields(&total, &mut allocator, 2, at)?;
                assert_eq!((first.capacity(), second.capacity()), (1, 2));
                Ok::<_, Box<Diagnostic>>(())
            })();
            assert_eq!(result.is_ok(), fail_at.is_none());
            drop(result);
        });
        assert_eq!(stats.1, 0);
        assert_eq!(
            total.get(),
            if fail_at == Some(1) {
                MAX_HIR_BYTES - 2 * width
            } else {
                MAX_HIR_BYTES
            }
        );
        assert_eq!(allocator.attempts, fail_at.unwrap_or(2));
        if fail_at.is_none() {
            assert_eq!(stats, (2, 0, (3 * width) as isize));
        }
    }
}

#[test]
fn c3_t0_actual_helper_carriers_are_measured_without_changing_active_hir_plan() {
    macro_rules! layout {
        ($($ty:ty),* $(,)?) => { $(
            println!("T0_LAYOUT {} size={} align={}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());
        )* };
    }
    layout!(
        Kind, TypeCounts, PaidStorage, Option<PaidStorage>, Result<PaidStorage, Box<Diagnostic>>,
        TypedBody, Projection, Option<Projection>, BorrowProjection, TypeFrame,
        AccountCarriers, ProjectionCarriers,
        ReserveCarriers<TypedBody>, ReserveCarriers<Option<ParameterTy>>, ReserveCarriers<Option<ValueTy>>,
        ReserveCarriers<Option<FlowSummary>>, ReserveCarriers<Option<Projection>>,
        ReserveCarriers<Vec<Option<Projection>>>, ReserveCarriers<BorrowProjection>, ReserveCarriers<TypeFrame>,
        ReserveCarriers<(ParameterTy, Span)>, ReserveCarriers<bool>, ReserveCarriers<ParameterTy>,
        ReserveCarriers<FlowSummary>, ReserveCarriers<ValueTy>,
        FillCarriers<Option<ParameterTy>>, FillCarriers<Option<ValueTy>>, FillCarriers<Option<FlowSummary>>,
        FillCarriers<Option<Projection>>, FillCarriers<bool>,
        FinalizeCarriers<ParameterTy>, FinalizeCarriers<FlowSummary>, FinalizeCarriers<ValueTy>,
        std::slice::Iter<'static, Option<ParameterTy>>, Option<&'static Option<ParameterTy>>,
        Result<ParameterTy, Box<Diagnostic>>, Result<Vec<FieldId>, Box<Diagnostic>>,
    );
    assert_eq!(size_of::<TypeCounts>(), 11 * size_of::<usize>());
    assert_eq!(size_of::<PaidStorage>(), 2 * KINDS * size_of::<usize>());
    assert!(
        account_carrier_bytes() >= 3 * size_of::<PaidStorage>() + 4 * size_of::<[usize; KINDS]>()
    );
    assert!(reserve_carrier_bytes() >= size_of::<ReserveCarriers<TypedBody>>());
    assert!(fill_carrier_bytes() >= size_of::<FillCarriers<Option<Projection>>>());
    assert_eq!(
        finalize_carrier_bytes(),
        size_of::<FinalizeCarriers<ParameterTy>>()
            + size_of::<FinalizeCarriers<FlowSummary>>()
            + size_of::<FinalizeCarriers<ValueTy>>()
    );
    assert_eq!(projection_carrier_bytes(), size_of::<ProjectionCarriers>());
    println!(
        "T0_HELPER_ENVELOPES account={} reserve={} fill={} finalize-sum={} projection={}",
        account_carrier_bytes(),
        reserve_carrier_bytes(),
        fill_carrier_bytes(),
        finalize_carrier_bytes(),
        projection_carrier_bytes()
    );
}
