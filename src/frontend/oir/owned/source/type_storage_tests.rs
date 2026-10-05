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
        room(&values, 2, at).unwrap();
        values.push(value);
    }
    assert!(room(&values, 2, at).is_err());
    assert_eq!(values.pop(), Some(value));
    room(&values, 2, at).unwrap();
    values.push(value);
    assert_eq!(
        (values.len(), values.capacity(), allocator.attempts),
        (2, 2, 1)
    );
    assert!(room(&Vec::<ValueTy>::with_capacity(3), 2, at).is_err());
    assert!(room(&Vec::<ValueTy>::with_capacity(1), 2, at).is_err());
    assert!(room(&Vec::<ValueTy>::new(), 0, at).is_err());
    let ticket = Capacity::new::<ValueTy>(2, 2 * size_of::<ValueTy>(), at).unwrap();
    ticket.check_observed(2, at).unwrap();
    assert!(ticket.check_observed(3, at).is_err());
}

#[test]
fn c3_t0_append_reads_real_capacity_and_rejects_empty_vector_with_nonzero_claim() {
    let at = origin();
    let values = Vec::<ValueTy>::new();
    assert_eq!(values.capacity(), 0);
    // The old slice-plus-capacity signature accepted (empty, 1, 1), after which
    // a push could allocate. The helper now owns the actual capacity read.
    assert_eq!(room(&values, 1, at).unwrap_err().code, "E0400");
    assert_eq!((values.len(), values.capacity()), (0, 0));
    let exact = Vec::<ValueTy>::with_capacity(1);
    room(&exact, 1, at).unwrap();
    assert!(room(&exact, 0, at).is_err());
    assert!(room(&exact, 2, at).is_err());
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
        RoomCarriers<Option<ParameterTy>>, RoomCarriers<Option<ValueTy>>, RoomCarriers<Option<FlowSummary>>,
        RoomCarriers<Option<Projection>>, RoomCarriers<bool>, RoomCarriers<ParameterTy>,
        RoomCarriers<FlowSummary>, RoomCarriers<ValueTy>,
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
    assert!(
        size_of::<RoomCarriers<ValueTy>>()
            >= size_of::<&Vec<ValueTy>>()
                + 2 * size_of::<usize>()
                + size_of::<Span>()
                + size_of::<Result<(), Box<Diagnostic>>>()
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

fn plain_function(ordinal: usize) -> Function {
    let at = origin();
    Function {
        id: DefId(ordinal),
        bindings: Vec::new(),
        expressions: vec![Expr {
            kind: ExprKind::Unit,
            span: at,
        }],
        body: BodyBlockId(0),
        blocks: vec![BodyBlock {
            body: vec![Stmt {
                kind: StmtKind::Return(Some(ExprId(0))),
                span: at,
            }],
            span: at,
            end: at,
        }],
        end: at,
    }
}
fn plain_signature() -> Signature {
    Signature {
        params: Vec::new(),
        result: ValueTy::Scalar(Ty::Unit),
        span: origin(),
    }
}
/// Synthetic counter allowance only, not an admitted real source plan.
fn source_bounds(counts: TypeCounts) -> HirPlan {
    HirPlan {
        counts: super::super::hir_budget::HirCounts {
            functions: counts.functions,
            bindings: counts.bindings,
            expressions: counts.expressions,
            blocks: counts.blocks,
            statements: counts.statements,
            calls: counts.calls,
            call_arguments: counts.call_arguments,
            borrow_arguments: counts.borrow_arguments,
            record_literals: counts.record_literals,
            max_record_fields: counts.presence_slots,
            type_frames: counts.type_frames,
            ..Default::default()
        },
        resolved: 0,
        typed: 0,
        staging: 0,
        resolver_scratch: 0,
        typeck_scratch: 0,
        fixed: 0,
        lower_fixed: 0,
        total: 0,
    }
}
fn plain_counts(functions: usize) -> TypeCounts {
    TypeCounts {
        functions,
        expressions: functions,
        blocks: functions,
        statements: functions,
        type_frames: 11 * functions,
        ..TypeCounts::default()
    }
}

#[test]
fn c3_t0_hir_preparation_is_metered_heap_free_and_partitions_bodies_only_once() {
    let at = origin();
    let records = [];
    let signatures = [plain_signature(), plain_signature()];
    let functions = [plain_function(0), plain_function(1)];
    let source = source_bounds(plain_counts(2));
    let work = WorkMeter::default();
    let (result, stats) = super::super::reviewer_source::integration_measured(|| {
        prepare(&records, &signatures, &functions, &source, &work, at)
    });
    let mut plan = result.unwrap();
    assert_eq!(stats, (0, 0, 0));
    assert_eq!(plan.counts, plain_counts(2));
    // Setup1 + signatures2 + functions*(outer1 + local1 + E1+B1+S1)
    // + reconciliation header1 + fourteen kind checks = 28.
    assert_eq!(work.used(), 28);
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(3).unwrap();
    let bodies = plan.reserve_bodies(&mut allocator, at).unwrap();
    assert_eq!(bodies.capacity(), 2);
    for ordinal in 0..2 {
        let (result, stats) =
            super::super::reviewer_source::integration_measured(|| plan.partition_next(&work, at));
        let mut local = result.unwrap();
        assert_eq!(stats, (0, 0, 0));
        assert_eq!(local.ordinal, ordinal);
        assert_eq!(local.counts, plain_counts(1));
        assert_eq!(
            (
                local.storage.slots[Kind::Bodies as usize],
                local.storage.requests[Kind::Bodies as usize]
            ),
            (0, 0)
        );
        assert!(local
            .storage
            .reserve::<TypedBody>(&mut allocator, Kind::Bodies, 0, 0, at)
            .is_err());
        let expressions = local
            .storage
            .none::<ValueTy>(&mut allocator, Kind::ExpressionStage, 1, at)
            .unwrap();
        assert_eq!(expressions.len(), 1);
        assert!(local
            .storage
            .none::<ValueTy>(&mut allocator, Kind::ExpressionStage, 0, at)
            .is_err());
    }
    assert_eq!(plan.storage.slots, [0; KINDS]);
    assert_eq!(plan.storage.requests, [0; KINDS]);
    assert_eq!(plan.next_function, 2);
    assert!(plan.partition_next(&work, at).is_err());
    assert!(plan.reserve_bodies(&mut allocator, at).is_err());
    assert_eq!(allocator.attempts, 3);
}

#[test]
fn c3_t0_empty_program_keeps_one_empty_bodies_request_and_no_function_rights() {
    let at = origin();
    let source = source_bounds(TypeCounts::default());
    let work = WorkMeter::default();
    let mut plan = prepare(&[], &[], &[], &source, &work, at).unwrap();
    assert_eq!(work.used(), 16);
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(1).unwrap();
    assert!(plan.reserve_bodies(&mut allocator, at).unwrap().is_empty());
    assert!(plan.reserve_bodies(&mut allocator, at).is_err());
    assert!(plan.partition_next(&work, at).is_err());
    assert_eq!(allocator.attempts, 1);
    assert_eq!(plan.storage.slots, [0; KINDS]);
    assert_eq!(plan.storage.requests, [0; KINDS]);
}

#[test]
fn c3_t0_hir_counts_full_presence_borrow_arguments_and_canonical_depth() {
    let at = origin();
    let records = [Record {
        id: RecordId(0),
        name_span: at,
        span: at,
        fields: (0..3)
            .map(|index| Field {
                id: FieldId {
                    record: RecordId(0),
                    index,
                },
                ty: ValueTy::Scalar(Ty::I32),
                name_span: at,
                span: at,
            })
            .collect(),
        end: at,
    }];
    let mut function = plain_function(0);
    function.expressions.push(Expr {
        kind: ExprKind::StructLiteral {
            record: RecordId(0),
            fields: Vec::new(),
        },
        span: at,
    });
    function.expressions.push(Expr {
        kind: ExprKind::Call {
            target: DefId(0),
            args: vec![
                Argument::Value(ExprId(0)),
                Argument::Borrow {
                    kind: BorrowKind::Shared,
                    place: BorrowPlace::Owner(BindingId(0)),
                    span: at,
                    name_span: at,
                    star_span: None,
                },
            ],
        },
        span: at,
    });
    function.expressions.push(Expr {
        kind: ExprKind::Call {
            target: DefId(0),
            args: Vec::new(),
        },
        span: at,
    });
    function.blocks[0].body.insert(
        0,
        Stmt {
            kind: StmtKind::If {
                condition: ExprId(0),
                then_block: BodyBlockId(1),
                else_block: Some(BodyBlockId(3)),
            },
            span: at,
        },
    );
    function.blocks.extend([
        BodyBlock {
            body: vec![Stmt {
                kind: StmtKind::While {
                    loop_id: LoopId(2),
                    condition: ExprId(0),
                    body: BodyBlockId(2),
                },
                span: at,
            }],
            span: at,
            end: at,
        },
        BodyBlock {
            body: Vec::new(),
            span: at,
            end: at,
        },
        BodyBlock {
            body: Vec::new(),
            span: at,
            end: at,
        },
    ]);
    let work = WorkMeter::default();
    let (result, stats) = super::super::reviewer_source::integration_measured(|| {
        count_function(&records, &function, &work)
    });
    assert_eq!(stats, (0, 0, 0));
    assert_eq!(
        result.unwrap(),
        TypeCounts {
            functions: 1,
            expressions: 4,
            blocks: 4,
            statements: 3,
            calls: 2,
            call_arguments: 2,
            borrow_arguments: 1,
            record_literals: 1,
            presence_slots: 3,
            type_frames: 17,
            ..TypeCounts::default()
        }
    );
    assert_eq!(work.used(), 14); // function1 + E4 + A2 + B4 + S3
}

#[test]
fn c3_t0_hir_depth_certificate_rejects_shared_cyclic_reordered_and_detached_blocks() {
    let at = origin();
    for mutant in 0..4 {
        let mut function = plain_function(0);
        function.blocks.extend([
            BodyBlock {
                body: Vec::new(),
                span: at,
                end: at,
            },
            BodyBlock {
                body: Vec::new(),
                span: at,
                end: at,
            },
        ]);
        function.blocks[0].body.insert(
            0,
            Stmt {
                kind: StmtKind::If {
                    condition: ExprId(0),
                    then_block: BodyBlockId(1),
                    else_block: Some(BodyBlockId(2)),
                },
                span: at,
            },
        );
        match mutant {
            0 => {
                if let StmtKind::If { else_block, .. } = &mut function.blocks[0].body[0].kind {
                    *else_block = Some(BodyBlockId(1));
                }
            }
            1 => {
                if let StmtKind::If { then_block, .. } = &mut function.blocks[0].body[0].kind {
                    *then_block = BodyBlockId(0);
                }
            }
            2 => {
                if let StmtKind::If {
                    then_block,
                    else_block,
                    ..
                } = &mut function.blocks[0].body[0].kind
                {
                    *then_block = BodyBlockId(2);
                    *else_block = Some(BodyBlockId(1));
                }
            }
            3 => {
                function.blocks[0].body.remove(0);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            count_function(&[], &function, &WorkMeter::default())
                .err()
                .unwrap()
                .code,
            "E0500"
        );
    }
}

#[test]
fn c3_t0_hir_source_reconciliation_and_count_arithmetic_fail_closed() {
    let at = origin();
    let functions = [plain_function(0)];
    let signatures = [plain_signature()];
    let mut source = source_bounds(plain_counts(1));
    source.counts.expressions = 0;
    assert_eq!(
        prepare(
            &[],
            &signatures,
            &functions,
            &source,
            &WorkMeter::default(),
            at
        )
        .err()
        .unwrap()
        .code,
        "E0400"
    );
    source.counts.expressions = 1;
    source.counts.record_literals = usize::MAX;
    source.counts.max_record_fields = 2;
    assert_eq!(
        prepare(
            &[],
            &signatures,
            &functions,
            &source,
            &WorkMeter::default(),
            at
        )
        .err()
        .unwrap()
        .code,
        "E0400"
    );
    let counts = TypeCounts {
        functions: usize::MAX,
        ..TypeCounts::default()
    };
    assert!(counts.combined(&plain_counts(1), at).is_err());
    assert_eq!(counts.functions, usize::MAX);
    let source = source_bounds(plain_counts(1));
    assert_eq!(
        prepare(&[], &[], &functions, &source, &WorkMeter::default(), at)
            .err()
            .unwrap()
            .code,
        "E0500"
    );
}

#[test]
fn c3_t0_partition_work_and_late_quota_failures_preserve_all_global_rights() {
    let at = origin();
    let signatures = [plain_signature()];
    let functions = [plain_function(0)];
    let source = source_bounds(plain_counts(1));
    let mut plan = prepare(
        &[],
        &signatures,
        &functions,
        &source,
        &WorkMeter::default(),
        at,
    )
    .unwrap();
    let before = (
        plan.storage.slots,
        plan.storage.requests,
        plan.next_function,
    );
    // Partition is header1 + local(function1+E1+B1+S1) + fourteen kinds.
    assert!(plan.partition_next(&WorkMeter::new(18), at).is_err());
    assert_eq!(
        (
            plan.storage.slots,
            plan.storage.requests,
            plan.next_function
        ),
        before
    );
    // Last kind subtraction fails after all earlier candidate values exist.
    plan.storage.requests[Kind::ExpressionFinal as usize] = 0;
    let before = (
        plan.storage.slots,
        plan.storage.requests,
        plan.next_function,
    );
    assert!(plan.partition_next(&WorkMeter::default(), at).is_err());
    assert_eq!(
        (
            plan.storage.slots,
            plan.storage.requests,
            plan.next_function
        ),
        before
    );
    plan.storage.requests[Kind::ExpressionFinal as usize] = 1;
    assert_eq!(
        plan.partition_next(&WorkMeter::new(19), at)
            .unwrap()
            .ordinal,
        0
    );
}

#[test]
fn c3_t0_hir_count_and_partition_carriers_are_measured_before_integration() {
    macro_rules! layout {
        ($($ty:ty),* $(,)?) => { $(println!("T0_COUNT_LAYOUT {} size={} align={}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());)* };
    }
    layout!(TypePlan<'static>, FunctionQuota, Result<TypePlan<'static>, Box<Diagnostic>>, Result<FunctionQuota, Box<Diagnostic>>,
        CountReturnCarriers, CountGuardCarriers, BodyCountCarriers, BodyChildBranches, FunctionCountCarriers,
        ReconcileCarriers, PreparationCarriers, PartitionCarriers, BodiesReserveCarriers,
        BlockCursor, Option<BlockCursor>, [Option<BlockCursor>; MAX_BLOCK_NESTING],
        EnumeratedLoop<Record>, SliceLoop<Field>, SliceLoop<Signature>, SliceLoop<ParameterTy>,
        EnumeratedLoop<Function>, SliceLoop<Binding>, SliceLoop<Expr>, SliceLoop<Argument>);
    assert!(
        size_of::<BodyCountCarriers>()
            >= size_of::<[Option<BlockCursor>; MAX_BLOCK_NESTING]>()
                + size_of::<BlockCursor>()
                + size_of::<Option<BlockCursor>>()
    );
    assert!(
        size_of::<PartitionCarriers>()
            >= 2 * size_of::<FunctionQuota>() + size_of::<Result<FunctionQuota, Box<Diagnostic>>>()
    );
    assert_eq!(
        size_of::<BodyChildBranches>(),
        2 * size_of::<&BodyBlockId>() + size_of::<&Option<BodyBlockId>>()
    );
    println!(
        "T0_COUNT_ENVELOPES count={} preparation={} partition={}",
        count_carrier_bytes(),
        preparation_carrier_bytes(),
        partition_carrier_bytes()
    );
}

fn nested_function(depth: usize, with_elses: bool) -> Function {
    fn append(blocks: &mut Vec<BodyBlock>, depth: usize, with_elses: bool) -> BodyBlockId {
        let at = origin();
        let id = BodyBlockId(blocks.len());
        blocks.push(BodyBlock {
            body: Vec::new(),
            span: at,
            end: at,
        });
        if depth > 1 {
            let child = append(blocks, depth - 1, with_elses);
            let kind = if with_elses {
                let otherwise = BodyBlockId(blocks.len());
                blocks.push(BodyBlock {
                    body: Vec::new(),
                    span: at,
                    end: at,
                });
                StmtKind::If {
                    condition: ExprId(0),
                    then_block: child,
                    else_block: Some(otherwise),
                }
            } else {
                StmtKind::While {
                    loop_id: LoopId(child.0),
                    condition: ExprId(0),
                    body: child,
                }
            };
            blocks[id.0].body.push(Stmt { kind, span: at });
        }
        id
    }
    let at = origin();
    let mut function = plain_function(0);
    function.expressions = vec![
        Expr {
            kind: ExprKind::Bool(true),
            span: at,
        },
        Expr {
            kind: ExprKind::Unit,
            span: at,
        },
    ];
    function.blocks.clear();
    function.body = append(&mut function.blocks, depth, with_elses);
    function.blocks[0].body.push(Stmt {
        kind: StmtKind::Return(Some(ExprId(1))),
        span: at,
    });
    function
}

/// Independent schedule replay, not a second count/depth implementation: retain
/// each join and pending else exactly as the ordinary check_body LIFO schedule.
/// It tracks occupied TypeFrame slots, not type semantics or machine stack.
fn ordinary_frame_peak(function: &Function) -> usize {
    enum Pending {
        Block(BodyBlockId, usize),
        Join(BodyBlockId, usize),
    }
    let mut frames = vec![Pending::Block(function.body, 0)];
    let mut maximum = frames.len();
    while let Some(frame) = frames.pop() {
        let (block, position) = match frame {
            Pending::Block(block, position) | Pending::Join(block, position) => (block, position),
        };
        let Some(statement) = function.blocks[block.0].body.get(position) else {
            continue;
        };
        match &statement.kind {
            StmtKind::If {
                then_block,
                else_block,
                ..
            } => {
                frames.push(Pending::Join(block, position + 1));
                if let Some(otherwise) = else_block {
                    frames.push(Pending::Block(*otherwise, 0));
                }
                frames.push(Pending::Block(*then_block, 0));
            }
            StmtKind::While { body, .. } => {
                frames.push(Pending::Join(block, position + 1));
                frames.push(Pending::Block(*body, 0));
            }
            StmtKind::Match { .. } => panic!("the ordinary schedule cannot price Match"),
            _ => frames.push(Pending::Block(block, position + 1)),
        }
        maximum = maximum.max(frames.len());
    }
    maximum
}

#[test]
fn c3_t0_canonical_depth_exact_boundary_and_actual_ordinary_schedule_bound() {
    for depth in [1, 2, 3, 8, MAX_BLOCK_NESTING - 1, MAX_BLOCK_NESTING] {
        for with_elses in [false, true] {
            let function = nested_function(depth, with_elses);
            let work = WorkMeter::default();
            let (result, stats) = super::super::reviewer_source::integration_measured(|| {
                count_function(&[], &function, &work)
            });
            let counts = result.unwrap();
            assert_eq!(stats, (0, 0, 0));
            assert_eq!(counts.functions, 1);
            assert_eq!(counts.expressions, 2);
            assert_eq!(counts.statements, depth);
            assert_eq!(
                counts.blocks,
                if with_elses { 2 * depth - 1 } else { depth }
            );
            assert_eq!(counts.type_frames, 3 * depth + 8);
            let peak = ordinary_frame_peak(&function);
            assert_eq!(peak, if with_elses { 2 * depth - 1 } else { depth });
            assert!(peak <= counts.type_frames);
        }
    }
    for with_elses in [false, true] {
        let too_deep = nested_function(MAX_BLOCK_NESTING + 1, with_elses);
        assert_eq!(
            count_function(&[], &too_deep, &WorkMeter::default())
                .err()
                .unwrap()
                .code,
            "E0400"
        );
    }
}

#[test]
fn c3_t0_schedule_covers_sibling_continuations_and_absent_else_without_new_frames() {
    let at = origin();
    let mut function = plain_function(0);
    function.blocks[0].body.insert(
        0,
        Stmt {
            kind: StmtKind::If {
                condition: ExprId(0),
                then_block: BodyBlockId(1),
                else_block: None,
            },
            span: at,
        },
    );
    function.blocks[0].body.insert(
        1,
        Stmt {
            kind: StmtKind::While {
                loop_id: LoopId(2),
                condition: ExprId(0),
                body: BodyBlockId(2),
            },
            span: at,
        },
    );
    function.blocks.extend([
        BodyBlock {
            body: Vec::new(),
            span: at,
            end: at,
        },
        BodyBlock {
            body: vec![Stmt {
                kind: StmtKind::If {
                    condition: ExprId(0),
                    then_block: BodyBlockId(3),
                    else_block: Some(BodyBlockId(4)),
                },
                span: at,
            }],
            span: at,
            end: at,
        },
        BodyBlock {
            body: Vec::new(),
            span: at,
            end: at,
        },
        BodyBlock {
            body: Vec::new(),
            span: at,
            end: at,
        },
    ]);
    let counts = count_function(&[], &function, &WorkMeter::default()).unwrap();
    assert_eq!(
        (counts.blocks, counts.statements, counts.type_frames),
        (5, 4, 17)
    );
    assert_eq!(ordinary_frame_peak(&function), 4);
}

fn rich_parts() -> (Vec<Record>, Vec<Signature>, Vec<Function>, HirPlan) {
    let at = origin();
    let records = vec![Record {
        id: RecordId(0),
        name_span: at,
        span: at,
        fields: (0..3)
            .map(|index| Field {
                id: FieldId {
                    record: RecordId(0),
                    index,
                },
                ty: ValueTy::Scalar(Ty::I32),
                name_span: at,
                span: at,
            })
            .collect(),
        end: at,
    }];
    let mut signature = plain_signature();
    signature
        .params
        .push(ParameterTy::Value(ValueTy::Scalar(Ty::I32)));
    let mut function = plain_function(0);
    function.bindings.push(Binding {
        mutable: false,
        span: at,
        annotation: None,
        scope: BodyBlockId(0),
        parameter_position: Some(0),
    });
    function.expressions.push(Expr {
        kind: ExprKind::StructLiteral {
            record: RecordId(0),
            fields: Vec::new(),
        },
        span: at,
    });
    function.expressions.push(Expr {
        kind: ExprKind::Call {
            target: DefId(0),
            args: vec![
                Argument::Value(ExprId(0)),
                Argument::Borrow {
                    kind: BorrowKind::Shared,
                    place: BorrowPlace::Owner(BindingId(0)),
                    span: at,
                    name_span: at,
                    star_span: None,
                },
            ],
        },
        span: at,
    });
    function.expressions.push(Expr {
        kind: ExprKind::Call {
            target: DefId(0),
            args: Vec::new(),
        },
        span: at,
    });
    function.blocks[0].body.insert(
        0,
        Stmt {
            kind: StmtKind::If {
                condition: ExprId(0),
                then_block: BodyBlockId(1),
                else_block: Some(BodyBlockId(3)),
            },
            span: at,
        },
    );
    function.blocks.extend([
        BodyBlock {
            body: vec![Stmt {
                kind: StmtKind::While {
                    loop_id: LoopId(2),
                    condition: ExprId(0),
                    body: BodyBlockId(2),
                },
                span: at,
            }],
            span: at,
            end: at,
        },
        BodyBlock {
            body: Vec::new(),
            span: at,
            end: at,
        },
        BodyBlock {
            body: Vec::new(),
            span: at,
            end: at,
        },
    ]);
    let mut source = source_bounds(TypeCounts {
        functions: 1,
        bindings: 1,
        expressions: 4,
        blocks: 4,
        statements: 3,
        calls: 2,
        call_arguments: 2,
        borrow_arguments: 1,
        record_literals: 1,
        presence_slots: 3,
        type_frames: 17,
    });
    source.counts.records = 1;
    source.counts.record_fields = 3;
    source.counts.parameters = 1;
    (records, vec![signature], vec![function], source)
}

#[test]
fn c3_t0_enum_values_references_annotations_constructors_and_match_remain_denied() {
    use crate::frontend::oir::owned_types::EnumId;
    let at = origin();
    let enumeration = AggregateTy::Enum(EnumId(0));
    for mutant in 0..7 {
        let (mut records, mut signatures, mut functions, source) = rich_parts();
        match mutant {
            0 => records[0].fields[0].ty = ValueTy::Owned(enumeration),
            1 => signatures[0].result = ValueTy::Owned(enumeration),
            2 => signatures[0].params[0] = ParameterTy::Value(ValueTy::Owned(enumeration)),
            3 => {
                signatures[0].params[0] = ParameterTy::Reference {
                    referent: BorrowedTy::Exact(enumeration),
                    kind: BorrowKind::Shared,
                }
            }
            4 => functions[0].bindings[0].annotation = Some(ValueTy::Owned(enumeration)),
            5 => {
                functions[0].expressions[0].kind = ExprKind::ConstructEnum {
                    variant: VariantId {
                        enumeration: EnumId(0),
                        index: 0,
                    },
                    payload: None,
                }
            }
            6 => {
                functions[0].blocks[0].body[0].kind = StmtKind::Match {
                    scrutinee: BindingId(0),
                    arms: Vec::new(),
                }
            }
            _ => unreachable!(),
        }
        assert_eq!(
            prepare(
                &records,
                &signatures,
                &functions,
                &source,
                &WorkMeter::default(),
                at
            )
            .err()
            .unwrap()
            .code,
            "E0500",
            "mutant {mutant}"
        );
    }
}

#[test]
fn c3_t0_wrong_nominal_function_root_and_source_table_identities_are_denied() {
    let at = origin();
    for mutant in 0..10 {
        let (mut records, mut signatures, mut functions, mut source) = rich_parts();
        let expected = if matches!(mutant, 6 | 7) {
            "E0400"
        } else {
            "E0500"
        };
        match mutant {
            0 => records[0].id = RecordId(1),
            1 => {
                if let ExprKind::StructLiteral { record, .. } =
                    &mut functions[0].expressions[1].kind
                {
                    *record = RecordId(1);
                }
            }
            2 => functions[0].id = DefId(1),
            3 => functions[0].body = BodyBlockId(1),
            4 => {
                signatures.clear();
            }
            5 => source.counts.records = 2,
            6 => source.counts.record_fields = 2,
            7 => source.total = MAX_HIR_BYTES + 1,
            8 => source.counts.functions = 0,
            9 => functions[0].blocks.clear(),
            _ => unreachable!(),
        }
        assert_eq!(
            prepare(
                &records,
                &signatures,
                &functions,
                &source,
                &WorkMeter::default(),
                at
            )
            .err()
            .unwrap()
            .code,
            expected,
            "mutant {mutant}"
        );
    }
    // Direct lookup also checks identity, independently of prepare's table walk.
    let (mut records, _, functions, _) = rich_parts();
    records[0].id = RecordId(1);
    assert_eq!(
        count_function(&records, &functions[0], &WorkMeter::default())
            .err()
            .unwrap()
            .code,
        "E0500"
    );
}

#[test]
fn c3_t0_every_source_slot_kind_and_zero_slot_call_literal_requests_are_bounded() {
    let at = origin();
    let counts = one();
    for kind in 0..KINDS {
        let mut source = source_bounds(counts);
        match kind {
            0 => source.counts.functions = 0,
            1 | 11 => source.counts.bindings = 0,
            2 | 4 | 13 => source.counts.expressions = 0,
            3 | 5 | 12 => source.counts.blocks = 0,
            6 => source.counts.statements = 0,
            7 => source.counts.borrow_arguments = 0,
            8 => source.counts.type_frames = 0,
            9 => source.counts.call_arguments = 0,
            10 => source.counts.max_record_fields = 0,
            _ => unreachable!(),
        }
        assert_eq!(
            reconcile(&counts, &source, &WorkMeter::default(), at)
                .err()
                .unwrap()
                .code,
            "E0400",
            "slot kind {kind}"
        );
    }
    // Empty calls/records have zero payload slots but still one request each.
    for literal in [false, true] {
        let counts = TypeCounts {
            calls: usize::from(!literal),
            record_literals: usize::from(literal),
            ..TypeCounts::default()
        };
        let source = source_bounds(TypeCounts::default());
        assert_eq!(counts.slots(), [0; KINDS]);
        assert_eq!(
            reconcile(&counts, &source, &WorkMeter::default(), at)
                .err()
                .unwrap()
                .code,
            "E0400"
        );
    }
}

#[test]
fn c3_t0_each_function_local_slot_and_request_subtraction_is_atomic() {
    let at = origin();
    for kind in 1..KINDS {
        for request in [false, true] {
            let (records, signatures, functions, source) = rich_parts();
            let mut plan = prepare(
                &records,
                &signatures,
                &functions,
                &source,
                &WorkMeter::default(),
                at,
            )
            .unwrap();
            if request {
                plan.storage.requests[kind] -= 1;
            } else {
                plan.storage.slots[kind] -= 1;
            }
            let before = (
                plan.storage.slots,
                plan.storage.requests,
                plan.next_function,
            );
            assert!(
                plan.partition_next(&WorkMeter::default(), at).is_err(),
                "kind {kind}, request {request}"
            );
            assert_eq!(
                (
                    plan.storage.slots,
                    plan.storage.requests,
                    plan.next_function
                ),
                before
            );
        }
    }
    let (records, signatures, functions, source) = rich_parts();
    let mut plan = prepare(
        &records,
        &signatures,
        &functions,
        &source,
        &WorkMeter::default(),
        at,
    )
    .unwrap();
    // Consuming Bodies before partition is legitimate and must not consume any
    // function-local right. No function gets the top-level request back.
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(1).unwrap();
    plan.reserve_bodies(&mut allocator, at).unwrap();
    let local = plan.partition_next(&WorkMeter::default(), at).unwrap();
    assert_eq!(
        local.storage.slots,
        [0, 1, 4, 4, 4, 4, 3, 1, 17, 2, 3, 1, 4, 4]
    );
    assert_eq!(
        local.storage.requests,
        [0, 1, 1, 1, 1, 1, 4, 1, 1, 2, 1, 1, 1, 1]
    );
    assert_eq!(plan.storage.slots, [0; KINDS]);
    assert_eq!(plan.storage.requests, [0; KINDS]);
}

#[test]
fn c3_t0_every_preparation_work_boundary_cleans_up_and_success_allocates_nothing() {
    let at = origin();
    let (records, signatures, functions, source) = rich_parts();
    let work = WorkMeter::default();
    prepare(&records, &signatures, &functions, &source, &work, at).unwrap();
    // header1 + record1+fields3 + signature1+parameter1 + outer-function1
    // + local(function1+binding1+E4+A2+B4+S3) + reconcile1+14 = 38.
    assert_eq!(work.used(), 38);
    for limit in 0..=38 {
        let work = WorkMeter::new(limit);
        let (success, stats) = super::super::reviewer_source::integration_measured(|| {
            let result = prepare(&records, &signatures, &functions, &source, &work, at);
            let success = result.is_ok();
            drop(result);
            success
        });
        assert_eq!(success, limit == 38);
        assert_eq!(stats.1, 0);
        if success {
            assert_eq!(stats, (0, 0, 0));
        }
    }
}

#[test]
fn c3_t0_each_count_sum_overflow_leaves_both_scalar_inputs_unchanged() {
    let at = origin();
    for field in 0..11 {
        let mut left = TypeCounts::default();
        match field {
            0 => left.functions = usize::MAX,
            1 => left.bindings = usize::MAX,
            2 => left.expressions = usize::MAX,
            3 => left.blocks = usize::MAX,
            4 => left.statements = usize::MAX,
            5 => left.borrow_arguments = usize::MAX,
            6 => left.type_frames = usize::MAX,
            7 => left.call_arguments = usize::MAX,
            8 => left.calls = usize::MAX,
            9 => left.presence_slots = usize::MAX,
            10 => left.record_literals = usize::MAX,
            _ => unreachable!(),
        }
        let right = one();
        let before = left;
        assert_eq!(
            left.combined(&right, at).err().unwrap().code,
            "E0400",
            "field {field}"
        );
        assert_eq!(left, before);
        assert_eq!(right, one());
    }
}
