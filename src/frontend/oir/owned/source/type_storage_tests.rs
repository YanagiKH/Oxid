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
fn c3_t0_actual_helper_carriers_are_measured_without_checker_admission() {
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
fn c3_t0_hir_count_and_partition_carriers_are_measured_before_checker_integration() {
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
        2 * size_of::<&BodyBlockId>()
            + size_of::<&Option<BodyBlockId>>()
            + 2 * size_of::<&Vec<MatchArm>>()
            + size_of::<Option<&MatchArm>>()
            + size_of::<&MatchArm>()
            + size_of::<Option<BodyBlockId>>()
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
fn c3_t0_enum_values_are_counted_but_containment_references_and_empty_match_are_denied() {
    use crate::frontend::oir::owned_types::EnumId;
    let at = origin();
    let enumeration = AggregateTy::Enum(EnumId(0));
    for mutant in 0..6 {
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
            _ => unreachable!(),
        }
        let result = prepare(
            &records,
            &signatures,
            &functions,
            &source,
            &WorkMeter::default(),
            at,
        );
        if matches!(mutant, 0 | 3) {
            assert_eq!(result.err().unwrap().code, "E0500", "mutant {mutant}");
        } else {
            // Resource preparation permits by-value enum storage. It is not
            // semantic typing, nominal identity proof, or owner admission.
            assert!(result.is_ok(), "mutant {mutant}");
        }
    }

    // An empty match cannot certify arm preorder, even though its resource
    // dimensions alone would match this ordinary-statement control.
    let signatures = [plain_signature()];
    let mut functions = [plain_function(0)];
    functions[0].blocks[0].body.insert(
        0,
        Stmt {
            kind: StmtKind::Expr(ExprId(0)),
            span: at,
        },
    );
    let source = source_bounds(TypeCounts {
        statements: 2,
        ..plain_counts(1)
    });
    assert_eq!(functions[0].blocks.len(), 1);
    assert!(prepare(
        &[],
        &signatures,
        &functions,
        &source,
        &WorkMeter::default(),
        at
    )
    .is_ok());
    functions[0].blocks[0].body[0].kind = StmtKind::Match {
        scrutinee: BindingId(0),
        arms: Vec::new(),
    };
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
        "E0500"
    );
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

#[test]
fn c3_t0_control_components_preserve_full_models_and_map_shared_transports_once() {
    assert_eq!(
        size_of::<AccountCarriers>(),
        size_of::<AccountControls>() + size_of::<TypeCounts>() + size_of::<PaidStorage>()
    );
    assert_eq!(
        size_of::<PartitionCarriers>(),
        size_of::<PartitionControls>() + size_of::<FunctionQuota>()
    );
    macro_rules! reserve {
        ($($ty:ty),* $(,)?) => { $(
            assert_eq!(size_of::<PrimitiveTransports<$ty>>(), size_of::<CapacityReturnEnvelope>() + size_of::<VectorReturnEnvelope<$ty>>());
            assert!(size_of::<VectorReturnEnvelope<$ty>>() <= super::super::hir_budget::VECTOR_RETURN_ENVELOPE_BYTES);
            assert_eq!(size_of::<ReserveCarriers<$ty>>(), size_of::<ReserveControls<$ty>>() + size_of::<PrimitiveTransports<$ty>>() + size_of::<Vec<$ty>>());
            println!("T0_CONTROL_LAYOUT reserve<{}>={} primitive<{}>={}", stringify!($ty), size_of::<ReserveControls<$ty>>(), stringify!($ty), size_of::<PrimitiveTransports<$ty>>());
        )* };
    }
    reserve!(
        TypedBody,
        Option<ParameterTy>,
        Option<ValueTy>,
        Option<FlowSummary>,
        Option<Projection>,
        Vec<Option<Projection>>,
        BorrowProjection,
        TypeFrame,
        (ParameterTy, Span),
        bool,
        ParameterTy,
        FlowSummary,
        ValueTy
    );
    macro_rules! fill {
        ($($ty:ty),* $(,)?) => { $(
            assert_eq!(size_of::<FillCarriers<$ty>>(), size_of::<FillControls<$ty>>() + size_of::<Vec<$ty>>());
            println!("T0_CONTROL_LAYOUT fill<{}>={}", stringify!($ty), size_of::<FillControls<$ty>>());
        )* };
    }
    fill!(
        Option<ParameterTy>,
        Option<ValueTy>,
        Option<FlowSummary>,
        Option<Projection>,
        bool
    );
    macro_rules! finalize {
        ($($ty:ty),* $(,)?) => { $(
            assert_eq!(size_of::<FinalizeCarriers<$ty>>(), size_of::<FinalizeControls<$ty>>() + size_of::<Vec<$ty>>());
            println!("T0_CONTROL_LAYOUT finalize<{}>={}", stringify!($ty), size_of::<FinalizeControls<$ty>>());
        )* };
    }
    finalize!(ParameterTy, FlowSummary, ValueTy);
    assert_eq!(
        size_of::<PrimitiveTransports<FieldId>>(),
        size_of::<CapacityReturnEnvelope>() + size_of::<VectorReturnEnvelope<FieldId>>()
    );
    assert!(
        size_of::<VectorReturnEnvelope<FieldId>>()
            <= super::super::hir_budget::VECTOR_RETURN_ENVELOPE_BYTES
    );
    assert_eq!(
        size_of::<ProjectionCarriers>(),
        size_of::<ProjectionControls>()
            + size_of::<PrimitiveTransports<FieldId>>()
            + size_of::<Vec<FieldId>>()
    );
    println!(
        "T0_CONTROL_LAYOUT account={} partition={} projection={} path-primitive={}",
        size_of::<AccountControls>(),
        size_of::<PartitionControls>(),
        size_of::<ProjectionControls>(),
        size_of::<PrimitiveTransports<FieldId>>()
    );
}

#[test]
fn c3_t0_fixed_control_formula_is_an_independent_sum_of_actual_components() {
    let reserve = [
        size_of::<ReserveControls<TypedBody>>(),
        size_of::<ReserveControls<Option<ParameterTy>>>(),
        size_of::<ReserveControls<Option<ValueTy>>>(),
        size_of::<ReserveControls<Option<FlowSummary>>>(),
        size_of::<ReserveControls<Option<Projection>>>(),
        size_of::<ReserveControls<Vec<Option<Projection>>>>(),
        size_of::<ReserveControls<BorrowProjection>>(),
        size_of::<ReserveControls<TypeFrame>>(),
        size_of::<ReserveControls<(ParameterTy, Span)>>(),
        size_of::<ReserveControls<bool>>(),
        size_of::<ReserveControls<ParameterTy>>(),
        size_of::<ReserveControls<FlowSummary>>(),
        size_of::<ReserveControls<ValueTy>>(),
    ]
    .into_iter()
    .max()
    .unwrap();
    let fill = [
        size_of::<FillControls<Option<ParameterTy>>>(),
        size_of::<FillControls<Option<ValueTy>>>(),
        size_of::<FillControls<Option<FlowSummary>>>(),
        size_of::<FillControls<Option<Projection>>>(),
        size_of::<FillControls<bool>>(),
    ]
    .into_iter()
    .max()
    .unwrap();
    let expected = size_of::<AccountControls>()
        + size_of::<CountReturnCarriers>()
        + size_of::<CountGuardCarriers>()
        + size_of::<BodyCountCarriers>()
        + size_of::<FunctionCountCarriers>()
        + size_of::<PreparationCarriers>()
        + size_of::<ReconcileCarriers>()
        + size_of::<PartitionControls>()
        + size_of::<BodiesReserveCarriers>()
        + reserve
        + fill
        + size_of::<FinalizeControls<ParameterTy>>()
        + size_of::<FinalizeControls<FlowSummary>>()
        + size_of::<FinalizeControls<ValueTy>>()
        + size_of::<ProjectionControls>();
    assert_eq!(fixed_control_carrier_bytes(), expected);
    assert_eq!(function_output_carrier_bytes(), size_of::<FunctionQuota>());
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    // Existing T0 controls plus the five newly reachable Match branch roles:
    // two Vec borrows, get/selected arm borrows, and its constructed child Option.
    assert_eq!(
        (expected, function_output_carrier_bytes()),
        (11584 + 48, 320)
    );
    println!(
        "T0_PASSIVE_SURCHARGE fixed={} per-function={}",
        expected,
        function_output_carrier_bytes()
    );
}

// These are isolated storage-helper controls. No resolved owner or paid checker
// context is constructed, and the source observer remains hard denied.
#[test]
fn c3_t1_metered_projection_rejects_before_changing_seed_work_or_allocator() {
    let at = origin();
    let width = size_of::<FieldId>();
    for (seed, slots) in [
        (0, 0),
        (0, 65),
        (0, usize::MAX),
        (usize::MAX, 1),
        (MAX_HIR_BYTES - width + 1, 1),
    ] {
        let total = Cell::new(seed);
        let work = WorkMeter::new(0);
        work.enable_observation();
        let mut allocator = Allocator {
            attempts: 7,
            ..Allocator::default()
        };
        let error =
            projection_fields_metered(&total, &mut allocator, slots, &work, at).unwrap_err();
        assert_eq!(error.code, "E0400");
        assert_eq!(total.get(), seed);
        assert_eq!(work.used(), 0);
        assert!(work.events.borrow().is_empty());
        assert_eq!(allocator.attempts, 7);
    }
}

#[test]
fn c3_t1_metered_projection_precharge_precedes_work_failure_and_reserve() {
    let at = origin();
    let width = size_of::<FieldId>();
    let total = Cell::new(MAX_HIR_BYTES - 2 * width);
    let work = WorkMeter::new(1);
    work.enable_observation();
    let mut allocator = Allocator {
        attempts: 7,
        ..Allocator::default()
    };
    allocator.observer_trace_bound(1).unwrap();
    let (_, stats) = super::super::reviewer_source::integration_measured(|| {
        let error = projection_fields_metered(&total, &mut allocator, 2, &work, at).unwrap_err();
        assert_eq!(error.code, "E0400");
        assert_eq!(error.message, "declaration index work limit exceeded");
        assert_eq!(error.primary, Some(at));
        drop(error);
    });
    assert_eq!(stats.1, 0);
    assert_eq!(total.get(), MAX_HIR_BYTES);
    assert_eq!(work.used(), 0);
    assert!(work.events.borrow().is_empty());
    assert_eq!(allocator.attempts, 7);
    assert!(allocator.trace.is_empty());
}

#[test]
fn c3_t1_metered_projection_exact_old_debit_schedule_and_cap_boundary() {
    let at = origin();
    let width = size_of::<FieldId>();
    for slots in [1, 2, 64] {
        for below in [0, 1] {
            let total = Cell::new(MAX_HIR_BYTES - slots * width - below);
            let work = WorkMeter::new(if slots == 1 { 0 } else { slots as u64 });
            work.enable_observation();
            let mut allocator = Allocator {
                attempts: 7,
                ..Allocator::default()
            };
            allocator.observer_trace_bound(1).unwrap();
            let path = projection_fields_metered(&total, &mut allocator, slots, &work, at).unwrap();
            assert_eq!((path.len(), path.capacity()), (0, slots));
            assert_eq!(total.get(), MAX_HIR_BYTES - below);
            assert_eq!(work.used(), if slots == 1 { 0 } else { slots as u64 });
            let events = work.events.borrow();
            if slots == 1 {
                assert!(events.is_empty());
            } else {
                assert_eq!(events.len(), 1);
                assert_eq!(
                    (events[0].operation, events[0].units, events[0].origin),
                    ("record projection path", slots as u64, at)
                );
            }
            assert_eq!(allocator.attempts, 8);
            assert_eq!(allocator.trace.len(), 1);
            let reserve = &allocator.trace[0];
            assert_eq!(
                (
                    reserve.kind,
                    reserve.length,
                    reserve.element_bytes,
                    reserve.success
                ),
                ("paid typed projection fields", slots, width, true)
            );
            drop(path);
        }
    }
}

#[test]
fn c3_t1_metered_projection_reserve_failure_keeps_spent_work_and_cleans_buffers() {
    let at = origin();
    let width = size_of::<FieldId>();
    for fail_at in [None, Some(8), Some(9)] {
        let total = Cell::new(MAX_HIR_BYTES - 3 * width);
        let work = WorkMeter::new(2);
        let mut allocator = Allocator {
            attempts: 7,
            fail_at,
            ..Allocator::default()
        };
        allocator.observer_trace_bound(2).unwrap();
        let (_, stats) = super::super::reviewer_source::integration_measured(|| {
            let result = (|| {
                let first = projection_fields_metered(&total, &mut allocator, 1, &work, at)?;
                let second = projection_fields_metered(&total, &mut allocator, 2, &work, at)?;
                assert_eq!((first.capacity(), second.capacity()), (1, 2));
                Ok::<_, Box<Diagnostic>>(())
            })();
            assert_eq!(result.is_ok(), fail_at.is_none());
            drop(result);
        });
        assert_eq!(stats.1, 0);
        assert_eq!(
            allocator.attempts - 7,
            if fail_at == Some(8) { 1 } else { 2 }
        );
        assert_eq!(
            total.get(),
            if fail_at == Some(8) {
                MAX_HIR_BYTES - 2 * width
            } else {
                MAX_HIR_BYTES
            }
        );
        assert_eq!(work.used(), if fail_at == Some(8) { 0 } else { 2 });
        if fail_at.is_none() {
            assert_eq!(stats, (2, 0, (3 * width) as isize));
        } else {
            assert!(!allocator.trace.last().unwrap().success);
        }
    }
}

#[test]
fn c3_t1_sample_kind_mappings_are_disjoint_complete_and_noncontiguous() {
    let mut covered = [false; KINDS];
    for (slot, kind) in SCRATCH_KINDS.iter().copied().enumerate() {
        assert!(!covered[kind as usize]);
        covered[kind as usize] = true;
        assert_eq!(scratch_index(kind), Some(slot));
        assert_eq!(retained_index(kind), None);
    }
    for (slot, kind) in RETAINED_KINDS.iter().copied().enumerate() {
        assert!(!covered[kind as usize]);
        covered[kind as usize] = true;
        assert_eq!(retained_index(kind), Some(slot));
        assert_eq!(scratch_index(kind), None);
    }
    assert_eq!(covered, [true; KINDS]);
    assert!(retained_index(Kind::Bodies).is_some());
    assert!(scratch_index(Kind::BindingStage).is_some());
    assert!(retained_index(Kind::ExpressionProjections).is_some());
    assert!(scratch_index(Kind::TypeFrames).is_some());
    assert!(retained_index(Kind::ExpressionFinal).is_some());
}

#[test]
fn c3_t1_empty_actual_vectors_are_sample_events_for_all_fourteen_kinds() {
    let mut observed = TypedObserved::new();
    macro_rules! sample {
        ($kind:ident, $ty:ty) => {{
            let values = Vec::<$ty>::new();
            let (result, measured) = super::super::reviewer_source::integration_measured(|| {
                observed.materialized(Kind::$kind, &values, origin())
            });
            result.unwrap();
            assert_eq!(measured, (0, 0, 0));
        }};
    }
    sample!(Bodies, TypedBody);
    sample!(BindingStage, Option<ParameterTy>);
    sample!(ExpressionStage, Option<ValueTy>);
    sample!(FlowStage, Option<FlowSummary>);
    sample!(ExpressionProjections, Option<Projection>);
    sample!(StatementRows, Vec<Option<Projection>>);
    sample!(StatementProjections, Option<Projection>);
    sample!(BorrowProjections, BorrowProjection);
    sample!(TypeFrames, TypeFrame);
    sample!(CallActuals, (ParameterTy, Span));
    sample!(RecordPresence, bool);
    sample!(BindingFinal, ParameterTy);
    sample!(FlowFinal, FlowSummary);
    sample!(ExpressionFinal, ValueTy);
    assert_eq!(observed.materialized_vectors, [1; KINDS]);
    assert_eq!(observed.materialized_capacity, [0; KINDS]);
    assert_eq!(observed.scratch_endpoints, [0; 6]);
    assert_eq!(
        (observed.path_vectors, observed.path_capacity_fields),
        (0, 0)
    );
}

#[test]
fn c3_t1_samples_read_actual_nonzero_capacities_and_scratch_endpoints() {
    let mut observed = TypedObserved::new();
    let at = origin();
    let bindings = vec![None::<ParameterTy>; 2];
    let expressions = vec![None::<ValueTy>; 3];
    let flows = vec![None::<FlowSummary>; 4];
    let frames = Vec::<TypeFrame>::with_capacity(5);
    let mut actuals = Vec::<(ParameterTy, Span)>::with_capacity(6);
    let presence = vec![false; 7];
    macro_rules! both {
        ($kind:ident, $values:expr) => {{
            observed.materialized(Kind::$kind, &$values, at).unwrap();
            observed
                .scratch_endpoint(Kind::$kind, &$values, at)
                .unwrap();
        }};
    }
    both!(BindingStage, bindings);
    both!(ExpressionStage, expressions);
    both!(FlowStage, flows);
    both!(TypeFrames, frames);
    observed
        .materialized(Kind::CallActuals, &actuals, at)
        .unwrap();
    for _ in 0..6 {
        actuals.push((ParameterTy::Value(ValueTy::Scalar(Ty::Unit)), at));
    }
    observed
        .scratch_endpoint(Kind::CallActuals, &actuals, at)
        .unwrap();
    both!(RecordPresence, presence);
    assert_eq!(observed.scratch_endpoints, [1; 6]);
    assert_eq!(observed.scratch_endpoint_capacity, [2, 3, 4, 5, 6, 7]);
    for (slot, kind) in SCRATCH_KINDS.iter().copied().enumerate() {
        assert_eq!(observed.materialized_vectors[kind as usize], 1);
        assert_eq!(
            observed.materialized_capacity[kind as usize],
            observed.scratch_endpoint_capacity[slot]
        );
    }
    let sparse = Vec::<BorrowProjection>::with_capacity(9);
    observed
        .materialized(Kind::BorrowProjections, &sparse, at)
        .unwrap();
    assert_eq!(
        observed.materialized_capacity[Kind::BorrowProjections as usize],
        sparse.capacity()
    );
    assert_eq!(sparse.len(), 0);
}

type SampleSnapshot = (
    [usize; KINDS],
    [usize; KINDS],
    [usize; 6],
    [usize; 6],
    usize,
    usize,
);
fn sample_snapshot(value: &TypedObserved) -> SampleSnapshot {
    (
        value.materialized_vectors,
        value.materialized_capacity,
        value.scratch_endpoints,
        value.scratch_endpoint_capacity,
        value.path_vectors,
        value.path_capacity_fields,
    )
}

#[test]
fn c3_t1_sample_wrong_width_shape_kind_and_overflows_leave_all_counters_unchanged() {
    let at = origin();
    let mut observed = TypedObserved::new();
    let full = vec![None::<ValueTy>; 1];
    let sparse_stage = Vec::<Option<ValueTy>>::with_capacity(1);
    let filled_actuals = vec![(ParameterTy::Value(ValueTy::Scalar(Ty::Unit)), at)];
    let empty_actuals = Vec::<(ParameterTy, Span)>::with_capacity(1);
    let before = sample_snapshot(&observed);
    assert_eq!(
        observed
            .materialized(Kind::BindingStage, &vec![0u8], at)
            .unwrap_err()
            .code,
        "E0500"
    );
    assert_eq!(
        observed
            .materialized(Kind::ExpressionStage, &sparse_stage, at)
            .unwrap_err()
            .code,
        "E0500"
    );
    assert_eq!(
        observed
            .materialized(Kind::CallActuals, &filled_actuals, at)
            .unwrap_err()
            .code,
        "E0500"
    );
    assert_eq!(
        observed
            .scratch_endpoint(Kind::CallActuals, &empty_actuals, at)
            .unwrap_err()
            .code,
        "E0500"
    );
    assert_eq!(
        observed
            .scratch_endpoint(Kind::ExpressionFinal, &Vec::<ValueTy>::new(), at)
            .unwrap_err()
            .code,
        "E0500"
    );
    assert_eq!(sample_snapshot(&observed), before);
    let k = Kind::ExpressionStage as usize;
    observed.materialized_vectors[k] = usize::MAX;
    let before = sample_snapshot(&observed);
    assert_eq!(
        observed
            .materialized(Kind::ExpressionStage, &full, at)
            .unwrap_err()
            .code,
        "E0400"
    );
    assert_eq!(sample_snapshot(&observed), before);
    observed.materialized_vectors[k] = 0;
    observed.materialized_capacity[k] = usize::MAX;
    let before = sample_snapshot(&observed);
    assert_eq!(
        observed
            .materialized(Kind::ExpressionStage, &full, at)
            .unwrap_err()
            .code,
        "E0400"
    );
    assert_eq!(sample_snapshot(&observed), before);
    observed.scratch_endpoint_capacity[1] = usize::MAX;
    let before = sample_snapshot(&observed);
    assert_eq!(
        observed
            .scratch_endpoint(Kind::ExpressionStage, &full, at)
            .unwrap_err()
            .code,
        "E0400"
    );
    assert_eq!(sample_snapshot(&observed), before);
}

#[test]
fn c3_t1_path_sample_accepts_fresh_empty_reserved_paths_not_initialized_paths() {
    let at = origin();
    let mut observed = TypedObserved::new();
    for capacity in [1, 2, 64] {
        let path = Vec::<FieldId>::with_capacity(capacity);
        let (result, measured) =
            super::super::reviewer_source::integration_measured(|| observed.path(&path, at));
        result.unwrap();
        assert_eq!(measured, (0, 0, 0));
    }
    assert_eq!(
        (observed.path_vectors, observed.path_capacity_fields),
        (3, 67)
    );
    for path in [
        Vec::new(),
        Vec::with_capacity(65),
        vec![FieldId {
            record: RecordId(0),
            index: 0,
        }],
    ] {
        let before = sample_snapshot(&observed);
        assert_eq!(observed.path(&path, at).unwrap_err().code, "E0500");
        assert_eq!(sample_snapshot(&observed), before);
    }
    observed.path_capacity_fields = usize::MAX;
    let before = sample_snapshot(&observed);
    assert_eq!(
        observed.path(&Vec::with_capacity(1), at).unwrap_err().code,
        "E0400"
    );
    assert_eq!(sample_snapshot(&observed), before);
}

#[test]
fn c3_t1_quota_and_plan_completion_are_read_only_metered_rights_checks() {
    let at = origin();
    let functions = [plain_function(0)];
    let signatures = [plain_signature()];
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
    assert!(std::ptr::eq(plan.counts(), &plan.counts));
    let before = (
        plan.storage.slots,
        plan.storage.requests,
        plan.next_function,
    );
    assert_eq!(
        plan.complete(&WorkMeter::default(), at).unwrap_err().code,
        "E0500"
    );
    assert_eq!(
        (
            plan.storage.slots,
            plan.storage.requests,
            plan.next_function
        ),
        before
    );
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(12).unwrap();
    let bodies = plan.reserve_bodies(&mut allocator, at).unwrap();
    let mut quota = plan.partition_next(&WorkMeter::default(), at).unwrap();
    let before = (quota.storage.slots, quota.storage.requests);
    let incomplete_work = WorkMeter::default();
    assert_eq!(
        quota.complete(&incomplete_work, at).unwrap_err().code,
        "E0500"
    );
    // Bodies has no local right; zero-slot BindingStage still owes one request.
    assert_eq!(incomplete_work.used(), 2);
    assert_eq!((quota.storage.slots, quota.storage.requests), before);
    macro_rules! consume {
        ($kind:ident, $ty:ty, $slots:expr) => {
            drop(
                quota
                    .storage
                    .reserve::<$ty>(&mut allocator, Kind::$kind, $slots, $slots, at)
                    .unwrap(),
            );
        };
    }
    consume!(BindingStage, Option<ParameterTy>, 0);
    consume!(ExpressionStage, Option<ValueTy>, 1);
    consume!(FlowStage, Option<FlowSummary>, 1);
    consume!(ExpressionProjections, Option<Projection>, 1);
    consume!(StatementRows, Vec<Option<Projection>>, 1);
    consume!(StatementProjections, Option<Projection>, 1);
    consume!(BorrowProjections, BorrowProjection, 0);
    consume!(TypeFrames, TypeFrame, 11);
    consume!(BindingFinal, ParameterTy, 0);
    consume!(FlowFinal, FlowSummary, 1);
    consume!(ExpressionFinal, ValueTy, 1);
    let before = (quota.storage.slots, quota.storage.requests);
    assert_eq!(before, ([0; KINDS], [0; KINDS]));
    for limit in 0..KINDS as u64 {
        let work = WorkMeter::new(limit);
        assert_eq!(quota.complete(&work, at).unwrap_err().code, "E0400");
        assert_eq!(work.used(), limit);
        assert_eq!((quota.storage.slots, quota.storage.requests), before);
    }
    let work = WorkMeter::new(KINDS as u64);
    let (result, measured) =
        super::super::reviewer_source::integration_measured(|| quota.complete(&work, at));
    result.unwrap();
    assert_eq!(measured, (0, 0, 0));
    assert_eq!(work.used(), KINDS as u64);
    let before_plan = (
        plan.storage.slots,
        plan.storage.requests,
        plan.next_function,
    );
    let work = WorkMeter::new(KINDS as u64);
    assert_eq!(plan.complete(&work, at).unwrap_err().code, "E0400");
    assert_eq!(
        (
            plan.storage.slots,
            plan.storage.requests,
            plan.next_function
        ),
        before_plan
    );
    let work = WorkMeter::new(KINDS as u64 + 1);
    plan.complete(&work, at).unwrap();
    assert_eq!(work.used(), KINDS as u64 + 1);
    assert_eq!(allocator.attempts, 12);
    drop(bodies);
    // Completion proves exhausted rights only; this test never types a body or
    // constructs a source/paid checker context from its synthetic T0 fixtures.
}

#[test]
fn c3_t1_observation_primitive_carriers_keep_t0_shapes_unchanged() {
    macro_rules! layout {
        ($($ty:ty),* $(,)?) => { $(
            println!("C3_T1_OBSERVATION_PRIMITIVE_LAYOUT {} {} {}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());
        )* };
    }
    layout!(
        TypedObserved,
        ObservedConstructionCarriers,
        KindMappingCarriers,
        PathSampleCarriers,
        CompletionCarriers,
        QuotaCompletionCarriers,
        PlanCompletionCarriers,
        CountsAccessCarriers
    );
    macro_rules! samples {
        ($($ty:ty),* $(,)?) => { $(layout!(SampleCarriers<$ty>);)* };
    }
    samples!(
        TypedBody,
        Option<ParameterTy>,
        Option<ValueTy>,
        Option<FlowSummary>,
        Option<Projection>,
        Vec<Option<Projection>>,
        BorrowProjection,
        TypeFrame,
        (ParameterTy, Span),
        bool,
        ParameterTy,
        FlowSummary,
        ValueTy
    );
    layout!(
        EndpointSampleCarriers<Option<ParameterTy>>,
        EndpointSampleCarriers<Option<ValueTy>>,
        EndpointSampleCarriers<Option<FlowSummary>>,
        EndpointSampleCarriers<TypeFrame>,
        EndpointSampleCarriers<(ParameterTy, Span)>,
        EndpointSampleCarriers<bool>
    );
    assert_eq!(
        size_of::<TypedObserved>(),
        (2 * KINDS + 2 * 6 + 2) * size_of::<usize>()
    );
    assert_eq!(size_of::<PaidStorage>(), 2 * KINDS * size_of::<usize>());
    assert_eq!(
        size_of::<FunctionQuota>(),
        size_of::<usize>() + size_of::<TypeCounts>() + size_of::<PaidStorage>()
    );
    assert_eq!(
        size_of::<TypePlan<'_>>(),
        size_of::<TypeCounts>()
            + size_of::<PaidStorage>()
            + size_of::<&[Record]>()
            + size_of::<&[Function]>()
            + size_of::<usize>()
    );
}

#[test]
fn c3_t1_zero_function_plan_still_requires_its_one_empty_bodies_request() {
    let at = origin();
    let source = source_bounds(TypeCounts::default());
    let mut plan = prepare(&[], &[], &[], &source, &WorkMeter::default(), at).unwrap();
    let before = (
        plan.storage.slots,
        plan.storage.requests,
        plan.next_function,
    );
    let work = WorkMeter::default();
    assert_eq!(plan.complete(&work, at).unwrap_err().code, "E0500");
    assert_eq!(work.used(), 2);
    assert_eq!(
        (
            plan.storage.slots,
            plan.storage.requests,
            plan.next_function
        ),
        before
    );
    let mut allocator = Allocator::default();
    let bodies = plan.reserve_bodies(&mut allocator, at).unwrap();
    assert_eq!(
        (bodies.len(), bodies.capacity(), allocator.attempts),
        (0, 0, 1)
    );
    plan.complete(&WorkMeter::new(KINDS as u64 + 1), at)
        .unwrap();
}

#[test]
fn c3_t1_endpoint_and_path_event_count_overflows_are_atomic() {
    let at = origin();
    let mut observed = TypedObserved::new();
    let stage = vec![None::<ValueTy>; 1];
    observed.scratch_endpoints[1] = usize::MAX;
    let before = sample_snapshot(&observed);
    assert_eq!(
        observed
            .scratch_endpoint(Kind::ExpressionStage, &stage, at)
            .unwrap_err()
            .code,
        "E0400"
    );
    assert_eq!(sample_snapshot(&observed), before);
    observed.path_vectors = usize::MAX;
    let path = Vec::<FieldId>::with_capacity(1);
    let before = sample_snapshot(&observed);
    assert_eq!(observed.path(&path, at).unwrap_err().code, "E0400");
    assert_eq!(sample_snapshot(&observed), before);
    // This has a correct filled endpoint shape; only the element width is wrong.
    assert_eq!(
        observed
            .scratch_endpoint(Kind::ExpressionStage, &vec![0u8], at)
            .unwrap_err()
            .code,
        "E0500"
    );
    assert_eq!(sample_snapshot(&observed), before);
}

#[test]
fn c3_t1_non_test_sample_size_surface_uses_actual_generic_components() {
    macro_rules! materialized {
        ($($ty:ty),* $(,)?) => { $(
            assert!(sample_carrier_bytes() >= size_of::<SampleCarriers<$ty>>());
        )* };
    }
    materialized!(
        TypedBody,
        Option<ParameterTy>,
        Option<ValueTy>,
        Option<FlowSummary>,
        Option<Projection>,
        Vec<Option<Projection>>,
        BorrowProjection,
        TypeFrame,
        (ParameterTy, Span),
        bool,
        ParameterTy,
        FlowSummary,
        ValueTy
    );
    macro_rules! endpoint {
        ($($ty:ty),* $(,)?) => { $(
            assert!(endpoint_sample_carrier_bytes() >= size_of::<EndpointSampleCarriers<$ty>>());
            assert_eq!(size_of::<EndpointSampleCarriers<$ty>>(),
                size_of::<SampleCarriers<$ty>>() + size_of::<KindMappingCarriers>());
        )* };
    }
    endpoint!(
        Option<ParameterTy>,
        Option<ValueTy>,
        Option<FlowSummary>,
        TypeFrame,
        (ParameterTy, Span),
        bool
    );
    assert_eq!(
        size_of::<ObservedConstructionCarriers>(),
        2 * size_of::<TypedObserved>()
    );
    assert_eq!(
        sample_sizing_carrier_bytes(),
        size_of::<SampleSizingCarriers>()
    );
    assert_eq!(
        observed_construction_carrier_bytes(),
        size_of::<ObservedConstructionCarriers>()
    );
    assert_eq!(
        counts_access_carrier_bytes(),
        size_of::<CountsAccessCarriers>()
    );
    assert_eq!(path_sample_carrier_bytes(), size_of::<PathSampleCarriers>());
    assert_eq!(completion_carrier_bytes(), size_of::<CompletionCarriers>());
    assert_eq!(
        quota_completion_carrier_bytes(),
        size_of::<QuotaCompletionCarriers>()
    );
    assert_eq!(
        plan_completion_carrier_bytes(),
        size_of::<PlanCompletionCarriers>()
    );
    println!(
        "C3_T1_OBSERVATION_PRIMITIVE_LAYOUT SampleSizingCarriers {} {}",
        size_of::<SampleSizingCarriers>(),
        align_of::<SampleSizingCarriers>()
    );
    println!(
        "C3_T1_OBSERVATION_PRIMITIVE_MAX materialized={} endpoint={}",
        sample_carrier_bytes(),
        endpoint_sample_carrier_bytes()
    );
}

fn empty_inventory_primitive() -> (
    TypedObserved,
    TypedInventory,
    TypeCounts,
    HirPlan,
    Cell<usize>,
) {
    // Actual empty buffer plus scalar T0 bounds only, not a source owner or
    // proof of the fresh-source prerequisites of the later closed observer.
    let bodies = Vec::<TypedBody>::new();
    let mut observed = TypedObserved::new();
    observed
        .materialized(Kind::Bodies, &bodies, origin())
        .unwrap();
    let mut inventory = TypedInventory::new();
    inventory
        .retained(Kind::Bodies, &bodies, &WorkMeter::new(1), origin())
        .unwrap();
    let counts = TypeCounts::default();
    (
        observed,
        inventory,
        counts,
        source_bounds(counts),
        Cell::new(0),
    )
}

#[test]
fn c3_t1_retained_vector_inventory_reads_actual_shapes_after_its_debit() {
    let at = origin();
    let mut inventory = TypedInventory::new();
    let bad_width = vec![0u8];
    let work = WorkMeter::new(0);
    let error = inventory
        .retained(Kind::BindingFinal, &bad_width, &work, at)
        .unwrap_err();
    assert_eq!(error.message, "declaration index work limit exceeded");
    assert_eq!(inventory, TypedInventory::new());
    assert_eq!(
        inventory
            .retained(Kind::BindingFinal, &bad_width, &WorkMeter::new(1), at)
            .unwrap_err()
            .code,
        "E0500"
    );
    assert_eq!(inventory, TypedInventory::new());
    let unfilled_bodies = Vec::<TypedBody>::with_capacity(1);
    assert_eq!(
        inventory
            .retained(Kind::Bodies, &unfilled_bodies, &WorkMeter::new(1), at)
            .unwrap_err()
            .code,
        "E0500"
    );
    let sparse = Vec::<BorrowProjection>::with_capacity(3);
    let work = WorkMeter::new(1);
    let (result, measured) = super::super::reviewer_source::integration_measured(|| {
        inventory.retained(Kind::BorrowProjections, &sparse, &work, at)
    });
    result.unwrap();
    assert_eq!(measured, (0, 0, 0));
    let slot = retained_index(Kind::BorrowProjections).unwrap();
    assert_eq!(
        (
            inventory.retained_vectors[slot],
            inventory.retained_lengths[slot],
            inventory.retained_capacities[slot]
        ),
        (1, 0, sparse.capacity())
    );
    inventory.retained_capacities[slot] = usize::MAX;
    let before = (
        inventory.retained_vectors,
        inventory.retained_lengths,
        inventory.retained_capacities,
    );
    assert_eq!(
        inventory
            .retained(Kind::BorrowProjections, &sparse, &WorkMeter::new(1), at)
            .unwrap_err()
            .code,
        "E0400"
    );
    assert_eq!(
        (
            inventory.retained_vectors,
            inventory.retained_lengths,
            inventory.retained_capacities
        ),
        before
    );
}

#[test]
fn c3_t1_scalar_reconciliation_has_exact_metered_schedule_and_no_heap() {
    let (observed, inventory, counts, source, total) = empty_inventory_primitive();
    for limit in 0..16 {
        let work = WorkMeter::new(limit);
        assert_eq!(
            reconcile_typed_storage(
                &observed,
                &inventory,
                &counts,
                &source,
                &total,
                1,
                &work,
                origin()
            )
            .unwrap_err()
            .code,
            "E0400"
        );
        assert_eq!(work.used(), limit);
    }
    let work = WorkMeter::new(16);
    work.enable_observation();
    // Work trace is separate instrumentation; disable it for the heap check.
    let (result, measured) = super::super::reviewer_source::integration_measured(|| {
        reconcile_typed_storage(
            &observed,
            &inventory,
            &counts,
            &source,
            &total,
            1,
            &WorkMeter::new(16),
            origin(),
        )
    });
    let result = result.unwrap();
    assert_eq!(measured, (0, 0, 0));
    assert_eq!(result.materialized_vectors[Kind::Bodies as usize], 1);
    assert_eq!(result.capacities, [0; KINDS]);
    assert_eq!(result.retained_lengths, [0; 8]);
    assert_eq!(
        (
            result.final_cell,
            result.typed_attempts,
            result.retained_backing_bytes,
            result.staging_backing_bytes,
            result.scratch_backing_bytes
        ),
        (0, 1, 0, 0, 0)
    );
    reconcile_typed_storage(
        &observed,
        &inventory,
        &counts,
        &source,
        &total,
        1,
        &work,
        origin(),
    )
    .unwrap();
    assert_eq!(work.used(), 16);
    let events = work.events.borrow();
    assert_eq!(events.len(), 16);
    assert_eq!(events[0].operation, "typed storage reconciliation");
    assert!(events[1..15]
        .iter()
        .all(|event| event.operation == "typed storage reconciliation kind"));
    assert_eq!(
        events[15].operation,
        "typed storage reconciliation completion"
    );
}

#[test]
fn c3_t1_reconciliation_independently_rejects_scalar_shape_and_final_fact_mismatches() {
    for mutant in 0..7 {
        let (mut observed, mut inventory, counts, source, total) = empty_inventory_primitive();
        match mutant {
            0 => inventory.retained_lengths[0] = 1,
            1 => inventory.retained_vectors[0] = 0,
            2 => observed.materialized_vectors[Kind::Bodies as usize] = 0,
            3 => observed.scratch_endpoints[0] = 1,
            4 => total.set(1),
            5 => inventory.path_vectors = 1,
            _ => inventory.path_capacity_fields = 1,
        }
        assert_eq!(
            reconcile_typed_storage(
                &observed,
                &inventory,
                &counts,
                &source,
                &total,
                1,
                &WorkMeter::new(16),
                origin()
            )
            .unwrap_err()
            .code,
            "E0500"
        );
    }
    let (observed, inventory, counts, source, total) = empty_inventory_primitive();
    // A wrong final attempt count is not read before the last authorized visit.
    let work = WorkMeter::new(15);
    let error = reconcile_typed_storage(
        &observed,
        &inventory,
        &counts,
        &source,
        &total,
        0,
        &work,
        origin(),
    )
    .unwrap_err();
    assert_eq!(error.message, "declaration index work limit exceeded");
    assert_eq!(work.used(), 15);
    assert_eq!(
        reconcile_typed_storage(
            &observed,
            &inventory,
            &counts,
            &source,
            &total,
            0,
            &WorkMeter::new(16),
            origin()
        )
        .unwrap_err()
        .code,
        "E0500"
    );
}

#[test]
fn c3_t1_inventory_and_reconciliation_actual_models_match_layout_forcing() {
    macro_rules! layout {
        ($($ty:ty),* $(,)?) => { $(
            println!("C3_T1_INVENTORY_LAYOUT {} {} {}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());
        )* };
    }
    layout!(TypedInventory, TypeStorageObservation, InventoryConstructionCarriers,
        TypedReconciliationCarriers, Result<TypedInventory, Box<Diagnostic>>,
        Result<TypeStorageObservation, Box<Diagnostic>>);
    macro_rules! sample {
        ($($ty:ty),* $(,)?) => { $(
            layout!(RetainedSampleCarriers<$ty>);
            assert!(retained_sample_carrier_bytes() >= size_of::<RetainedSampleCarriers<$ty>>());
        )* };
    }
    sample!(
        TypedBody,
        Option<Projection>,
        Vec<Option<Projection>>,
        BorrowProjection,
        ParameterTy,
        FlowSummary,
        ValueTy
    );
    assert_eq!(
        inventory_construction_carrier_bytes(),
        size_of::<InventoryConstructionCarriers>()
    );
    assert_eq!(
        typed_reconciliation_carrier_bytes(),
        size_of::<TypedReconciliationCarriers>()
    );
}

// Explicit Stage B role schemas: field names/types and role counts are checked
// independently of total width, which can conceal an omission in padding.
macro_rules! inventory_roles {
    ($model:ty, $count:expr; $( $field:ident : $ty:ty ),+ $(,)?) => {{
        $(let _: for<'a> fn(&'a $model) -> &'a $ty = |model| &model.$field;)+
        let roles = [$(
            (std::mem::offset_of!($model, $field), size_of::<$ty>(), align_of::<$ty>())
        ),+];
        assert_eq!(roles.len(), $count);
        let mut occupied = 0;
        for (i, (offset, bytes, alignment)) in roles.iter().copied().enumerate() {
            assert_eq!(offset % alignment, 0);
            assert!(offset + bytes <= size_of::<$model>());
            occupied += bytes;
            for (j, (other, width, _)) in roles.iter().copied().enumerate() {
                if i != j && bytes != 0 && width != 0 {
                    assert!(offset + bytes <= other || other + width <= offset);
                }
            }
        }
        assert!(occupied <= size_of::<$model>());
        println!("C3_T1_INVENTORY_ROLES {} fields={} typed_bytes={} padding={}",
            stringify!($model), roles.len(), occupied, size_of::<$model>() - occupied);
    }};
}
fn retained_sample_roles<T: 'static>() {
    inventory_roles!(RetainedSampleCarriers<T>, 17;
        inventory: &'static mut TypedInventory, kind: Kind, values: &'static Vec<T>,
        work: &'static WorkMeter, origin: Span, debit: Result<(), Box<Diagnostic>>,
        mapping: KindMappingCarriers, kind_index: usize, len: usize, capacity: usize,
        width: usize, additions: [Result<usize, Box<Diagnostic>>; 3], vectors: usize,
        lengths: usize, capacities: usize, returned: Result<(), Box<Diagnostic>>,
        caller_result: Result<(), Box<Diagnostic>>);
}
#[test]
fn c3_t1_inventory_role_schemas_and_fixed_return_forcing_are_complete() {
    inventory_roles!(TypedInventory, 6;
        retained_vectors: [usize; 8], retained_lengths: [usize; 8],
        retained_capacities: [usize; 8], path_vectors: usize,
        path_length_fields: usize, path_capacity_fields: usize);
    inventory_roles!(TypeStorageObservation, 14;
        materialized_vectors: [usize; KINDS], capacities: [usize; KINDS],
        retained_lengths: [usize; 8], retained_backing_bytes: usize,
        staging_backing_bytes: usize, scratch_backing_bytes: usize,
        path_vectors: usize, path_length_fields: usize, path_capacity_fields: usize,
        materialized_path_bytes: usize, retained_path_bytes: usize,
        precharged_path_bytes: usize, final_cell: usize, typed_attempts: usize);
    inventory_roles!(InventoryConstructionCarriers, 2;
        constructed: TypedInventory, returned: TypedInventory);
    inventory_roles!(TypedReconciliationCarriers, 51;
        observed: &'static TypedObserved, inventory: &'static TypedInventory,
        counts: &'static TypeCounts, source: &'static HirPlan, total: &'static Cell<usize>,
        typed_attempts: usize, work: &'static WorkMeter, origin: Span,
        expected_array_returns: [[usize; KINDS]; 2], expected_arrays: [[usize; KINDS]; 2],
        capacities: [usize; KINDS], retained_bytes: usize, staging_bytes: usize,
        scratch_bytes: usize, vector_events: usize, kinds: std::ops::Range<usize>,
        next: Option<usize>, k: usize, kind: Kind, retained_map_kind: Kind,
        retained_map_return: Option<usize>, retained_map_caller: Option<usize>,
        retained_slot: usize, scratch_mapping: KindMappingCarriers,
        retained_product: Option<usize>, retained_product_result: Result<usize, Box<Diagnostic>>,
        retained_product_bytes: usize, scratch_product: Option<usize>,
        scratch_product_result: Result<usize, Box<Diagnostic>>, scratch_product_bytes: usize,
        backing_add_returns: [Result<usize, Box<Diagnostic>>; 3],
        event_add_return: Result<usize, Box<Diagnostic>>, debit_returns: [Result<(), Box<Diagnostic>>; 3],
        path_products: [Option<usize>; 2], path_product_returns: [Result<usize, Box<Diagnostic>>; 2],
        materialized_path_bytes: usize, retained_path_bytes: usize, final_cell: usize,
        precharge_subtraction: Option<usize>, precharge_return: Result<usize, Box<Diagnostic>>,
        precharged_path_bytes: usize, attempt_return: Result<usize, Box<Diagnostic>>,
        attempts: usize, path_bound_option: Option<usize>, path_bound_return: Result<usize, Box<Diagnostic>>,
        path_bound: usize, path_slot_returns: [Result<usize, Box<Diagnostic>>; 2],
        path_slot_inner: usize, path_slots: usize, constructed: TypeStorageObservation,
        returned: Result<TypeStorageObservation, Box<Diagnostic>>);
    retained_sample_roles::<TypedBody>();
    retained_sample_roles::<Option<Projection>>();
    retained_sample_roles::<Vec<Option<Projection>>>();
    retained_sample_roles::<BorrowProjection>();
    retained_sample_roles::<ParameterTy>();
    retained_sample_roles::<FlowSummary>();
    retained_sample_roles::<ValueTy>();
    assert_eq!(
        inventory_result_carrier_bytes(),
        size_of::<TypedInventory>()
    );
    assert_eq!(
        inventory_return_carrier_bytes(),
        size_of::<Result<TypedInventory, Box<Diagnostic>>>()
    );
    assert_eq!(
        observation_result_carrier_bytes(),
        size_of::<TypeStorageObservation>()
    );
    assert_eq!(
        observation_return_carrier_bytes(),
        size_of::<Result<TypeStorageObservation, Box<Diagnostic>>>()
    );
}

fn inventory_snapshot(
    value: &TypedInventory,
) -> ([usize; 8], [usize; 8], [usize; 8], usize, usize, usize) {
    (
        value.retained_vectors,
        value.retained_lengths,
        value.retained_capacities,
        value.path_vectors,
        value.path_length_fields,
        value.path_capacity_fields,
    )
}
#[test]
fn c3_t1_retained_samples_cover_nonzero_shapes_and_independent_atomic_failures() {
    let at = origin();
    let mut inventory = TypedInventory::new();
    let bindings = vec![ParameterTy::Value(ValueTy::Scalar(Ty::I32)); 2]
        .into_boxed_slice()
        .into_vec();
    let projections = vec![None::<Projection>, None].into_boxed_slice().into_vec();
    let rows = vec![vec![None::<Projection>], Vec::new()]
        .into_boxed_slice()
        .into_vec();
    let bodies = Vec::<TypedBody>::new();
    let sparse = Vec::<BorrowProjection>::with_capacity(3);
    let work = WorkMeter::new(5);
    let (result, measured) = super::super::reviewer_source::integration_measured(|| {
        inventory.retained(Kind::Bodies, &bodies, &work, at)?;
        inventory.retained(Kind::BindingFinal, &bindings, &work, at)?;
        inventory.retained(Kind::ExpressionProjections, &projections, &work, at)?;
        inventory.retained(Kind::StatementRows, &rows, &work, at)?;
        inventory.retained(Kind::BorrowProjections, &sparse, &work, at)
    });
    result.unwrap();
    assert_eq!(measured, (0, 0, 0));
    assert_eq!(inventory.retained_vectors, [1, 1, 1, 0, 1, 1, 0, 0]);
    assert_eq!(inventory.retained_lengths, [0, 2, 2, 0, 0, 2, 0, 0]);
    assert_eq!(
        inventory.retained_capacities,
        [0, 2, 2, 0, sparse.capacity(), 2, 0, 0]
    );
    for kind in [Kind::BindingStage, Kind::CallActuals] {
        // Scratch-only kinds are forbidden even if their width happened to match.
        let before = inventory_snapshot(&inventory);
        let error = inventory
            .retained(kind, &bindings, &WorkMeter::new(0), at)
            .unwrap_err();
        assert_eq!(error.message, "declaration index work limit exceeded");
        assert_eq!(
            inventory
                .retained(kind, &bindings, &WorkMeter::new(1), at)
                .unwrap_err()
                .code,
            "E0500"
        );
        assert_eq!(inventory_snapshot(&inventory), before);
    }
    let slot = retained_index(Kind::BindingFinal).unwrap();
    for counter in 0..3 {
        let mut inventory = TypedInventory::new();
        match counter {
            0 => inventory.retained_vectors[slot] = usize::MAX,
            1 => inventory.retained_lengths[slot] = usize::MAX,
            _ => inventory.retained_capacities[slot] = usize::MAX,
        }
        let before = inventory_snapshot(&inventory);
        let work = WorkMeter::new(1);
        assert_eq!(
            inventory
                .retained(Kind::BindingFinal, &bindings, &work, at)
                .unwrap_err()
                .code,
            "E0400"
        );
        assert_eq!(work.used(), 1);
        assert_eq!(inventory_snapshot(&inventory), before);
        // Invalid shape must win over the same pending arithmetic overflow.
        let unfilled = Vec::<ParameterTy>::with_capacity(1);
        assert_eq!(
            inventory
                .retained(Kind::BindingFinal, &unfilled, &WorkMeter::new(1), at)
                .unwrap_err()
                .code,
            "E0500"
        );
        assert_eq!(inventory_snapshot(&inventory), before);
        let error = inventory
            .retained(Kind::BindingFinal, &unfilled, &WorkMeter::new(0), at)
            .unwrap_err();
        assert_eq!(error.message, "declaration index work limit exceeded");
        assert_eq!(inventory_snapshot(&inventory), before);
    }
}

// Arbitrary scalar primitive facts, explicitly not a matching fresh source/T0
// receipt. Constants below independently spell out the noncontiguous maps.
fn nonzero_inventory_primitive() -> (
    TypedObserved,
    TypedInventory,
    TypeCounts,
    HirPlan,
    Cell<usize>,
) {
    let counts = TypeCounts {
        functions: 2,
        bindings: 3,
        expressions: 5,
        blocks: 7,
        statements: 11,
        borrow_arguments: 13,
        type_frames: 17,
        call_arguments: 19,
        calls: 23,
        presence_slots: 29,
        record_literals: 31,
    };
    let observed = TypedObserved {
        materialized_vectors: [1, 2, 2, 2, 2, 2, 7, 2, 2, 23, 31, 2, 2, 2],
        materialized_capacity: [2, 3, 5, 7, 5, 7, 11, 13, 17, 19, 29, 3, 7, 5],
        scratch_endpoints: [2, 2, 2, 2, 23, 31],
        scratch_endpoint_capacity: [3, 5, 7, 17, 19, 29],
        path_vectors: 3,
        path_capacity_fields: 6,
    };
    let inventory = TypedInventory {
        retained_vectors: [1, 2, 2, 7, 2, 2, 2, 2],
        retained_lengths: [2, 5, 7, 11, 4, 3, 7, 5],
        retained_capacities: [2, 5, 7, 11, 13, 3, 7, 5],
        path_vectors: 3,
        path_length_fields: 6,
        path_capacity_fields: 6,
    };
    let mut source = source_bounds(counts);
    source.typed = 2 * size_of::<TypedBody>()
        + 16 * size_of::<Option<Projection>>()
        + 7 * size_of::<Vec<Option<Projection>>>()
        + 13 * size_of::<BorrowProjection>()
        + 3 * size_of::<ParameterTy>()
        + 7 * size_of::<FlowSummary>()
        + 5 * size_of::<ValueTy>();
    source.staging = 3 * size_of::<Option<ParameterTy>>()
        + 5 * size_of::<Option<ValueTy>>()
        + 7 * size_of::<Option<FlowSummary>>();
    source.typeck_scratch = 17 * size_of::<TypeFrame>()
        + 19 * size_of::<(ParameterTy, Span)>()
        + 29 * size_of::<bool>();
    source.total = 12345;
    let total = Cell::new(12345 + 6 * size_of::<FieldId>());
    (observed, inventory, counts, source, total)
}
#[test]
fn c3_t1_nonzero_reconciliation_has_independent_asymmetric_outputs_and_work() {
    let (observed, inventory, counts, source, total) = nonzero_inventory_primitive();
    // 82 fixed-vector requests plus three paths, independently summed here.
    let attempts = 1 + 10 * 2 + 7 + 23 + 31 + 3;
    assert_eq!(attempts, 85);
    let before = (
        format!("{observed:?}{inventory:?}"),
        counts,
        source,
        total.get(),
    );
    for limit in 0..16 {
        let work = WorkMeter::new(limit);
        let error = reconcile_typed_storage(
            &observed,
            &inventory,
            &counts,
            &source,
            &total,
            attempts,
            &work,
            origin(),
        )
        .unwrap_err();
        assert_eq!(error.message, "declaration index work limit exceeded");
        assert_eq!(work.used(), limit);
        assert_eq!(
            (
                format!("{observed:?}{inventory:?}"),
                counts,
                source,
                total.get()
            ),
            before
        );
    }
    let (result, measured) = super::super::reviewer_source::integration_measured(|| {
        reconcile_typed_storage(
            &observed,
            &inventory,
            &counts,
            &source,
            &total,
            attempts,
            &WorkMeter::new(16),
            origin(),
        )
    });
    let result = result.unwrap();
    assert_eq!(measured, (0, 0, 0));
    assert_eq!(
        result.materialized_vectors,
        [1, 2, 2, 2, 2, 2, 7, 2, 2, 23, 31, 2, 2, 2]
    );
    assert_eq!(
        result.capacities,
        [2, 3, 5, 7, 5, 7, 11, 13, 17, 19, 29, 3, 7, 5]
    );
    assert_eq!(result.retained_lengths, [2, 5, 7, 11, 4, 3, 7, 5]);
    assert_eq!(
        (
            result.retained_backing_bytes,
            result.staging_backing_bytes,
            result.scratch_backing_bytes
        ),
        (source.typed, source.staging, source.typeck_scratch)
    );
    assert_eq!(
        (
            result.path_vectors,
            result.path_length_fields,
            result.path_capacity_fields
        ),
        (3, 6, 6)
    );
    assert_eq!(
        (
            result.materialized_path_bytes,
            result.retained_path_bytes,
            result.precharged_path_bytes
        ),
        (
            6 * size_of::<FieldId>(),
            6 * size_of::<FieldId>(),
            6 * size_of::<FieldId>()
        )
    );
    assert_eq!(
        (result.final_cell, result.typed_attempts),
        (12345 + 6 * size_of::<FieldId>(), 85)
    );
    assert_eq!(
        (
            format!("{observed:?}{inventory:?}"),
            counts,
            source,
            total.get()
        ),
        before
    );
}

fn reject_scalar_inventory(
    facts: &(
        TypedObserved,
        TypedInventory,
        TypeCounts,
        HirPlan,
        Cell<usize>,
    ),
    attempts: usize,
    code: &str,
    visits: u64,
) {
    let (observed, inventory, counts, source, total) = facts;
    let before = (
        format!("{observed:?}{inventory:?}"),
        *counts,
        *source,
        total.get(),
    );
    let work = WorkMeter::new(16);
    let error = reconcile_typed_storage(
        observed,
        inventory,
        counts,
        source,
        total,
        attempts,
        &work,
        origin(),
    )
    .unwrap_err();
    assert_eq!(error.code, code);
    assert_eq!(work.used(), visits);
    assert_eq!(
        (
            format!("{observed:?}{inventory:?}"),
            *counts,
            *source,
            total.get()
        ),
        before
    );
}
#[test]
fn c3_t1_nonzero_reconciliation_checks_every_kind_and_noncontiguous_mapping() {
    for k in 0..14 {
        for capacity in [false, true] {
            let mut facts = nonzero_inventory_primitive();
            if capacity {
                facts.0.materialized_capacity[k] += 1;
            } else {
                facts.0.materialized_vectors[k] += 1;
            }
            reject_scalar_inventory(&facts, 85, "E0500", 2 + k as u64);
        }
    }
    // Independent literal ordinal lists, not calls to the implementation maps.
    for (slot, kind) in [0, 4, 5, 6, 7, 11, 12, 13].into_iter().enumerate() {
        for component in 0..3 {
            let mut facts = nonzero_inventory_primitive();
            match component {
                0 => facts.1.retained_vectors[slot] += 1,
                1 => facts.1.retained_capacities[slot] += 1,
                _ if slot == 4 => facts.1.retained_lengths[slot] = 14,
                _ => facts.1.retained_lengths[slot] -= 1,
            }
            reject_scalar_inventory(&facts, 85, "E0500", 2 + kind);
        }
    }
    for (slot, kind) in [1, 2, 3, 8, 9, 10].into_iter().enumerate() {
        for capacity in [false, true] {
            let mut facts = nonzero_inventory_primitive();
            if capacity {
                facts.0.scratch_endpoint_capacity[slot] += 1;
            } else {
                facts.0.scratch_endpoints[slot] += 1;
            }
            reject_scalar_inventory(&facts, 85, "E0500", 2 + kind);
        }
    }
}
#[test]
fn c3_t1_nonzero_reconciliation_final_guards_follow_the_last_debit() {
    for mutant in 0..15 {
        let mut facts = nonzero_inventory_primitive();
        let (observed, inventory, _, source, total) = &mut facts;
        let mut attempts = 85;
        match mutant {
            0 => source.typed -= 1,
            1 => source.staging -= 1,
            2 => source.typeck_scratch -= 1,
            3 => total.set(source.total - 1),
            4 => {
                source.total = MAX_HIR_BYTES + 1;
                total.set(source.total + 6 * size_of::<FieldId>());
            }
            5 => total.set(total.get() + 1),
            6 => observed.path_vectors += 1,
            7 => observed.path_capacity_fields += 1,
            8 => inventory.path_vectors += 1,
            9 => inventory.path_length_fields -= 1,
            10 => inventory.path_capacity_fields += 1,
            11 => {
                observed.path_vectors = 7;
                inventory.path_vectors = 7;
                attempts = 89;
            } // fields < paths
            12 => {
                observed.path_capacity_fields = 193;
                inventory.path_length_fields = 193;
                inventory.path_capacity_fields = 193;
                total.set(source.total + 193 * size_of::<FieldId>());
            } // fields > 64P
            13 => {
                observed.path_vectors = 21;
                inventory.path_vectors = 21; // only 5+11+4 eligible slots
                observed.path_capacity_fields = 21;
                inventory.path_length_fields = 21;
                inventory.path_capacity_fields = 21;
                total.set(source.total + 21 * size_of::<FieldId>());
                attempts = 103;
            }
            _ => attempts -= 1,
        }
        let work = WorkMeter::new(15);
        let error = reconcile_typed_storage(
            &facts.0,
            &facts.1,
            &facts.2,
            &facts.3,
            &facts.4,
            attempts,
            &work,
            origin(),
        )
        .unwrap_err();
        assert_eq!(error.message, "declaration index work limit exceeded");
        assert_eq!(work.used(), 15);
        reject_scalar_inventory(&facts, attempts, "E0500", 16);
    }
}
#[test]
fn c3_t1_nonzero_reconciliation_checked_arithmetic_fails_without_mutation() {
    // Retained product: the first kind fails before later count mismatches.
    let mut facts = nonzero_inventory_primitive();
    facts.2.functions = usize::MAX;
    facts.0.materialized_capacity[0] = usize::MAX;
    facts.1.retained_capacities[0] = usize::MAX;
    facts.1.retained_lengths[0] = usize::MAX;
    reject_scalar_inventory(&facts, 85, "E0400", 2);

    let mut facts = nonzero_inventory_primitive();
    facts.2.type_frames = usize::MAX;
    facts.0.materialized_capacity[8] = usize::MAX;
    facts.0.scratch_endpoint_capacity[3] = usize::MAX;
    reject_scalar_inventory(&facts, 85, "E0400", 10); // scratch product

    // Retained category addition: every preceding count/shape is consistent.
    let mut facts = nonzero_inventory_primitive();
    let functions = usize::MAX / size_of::<TypedBody>();
    facts.2.functions = functions;
    facts.0.materialized_capacity[0] = functions;
    facts.1.retained_capacities[0] = functions;
    facts.1.retained_lengths[0] = functions;
    for k in [1, 2, 3, 4, 5, 7, 8, 11, 12, 13] {
        facts.0.materialized_vectors[k] = functions;
    }
    for slot in [1, 2, 4, 5, 6, 7] {
        facts.1.retained_vectors[slot] = functions;
    }
    for slot in [0, 1, 2, 3] {
        facts.0.scratch_endpoints[slot] = functions;
    }
    assert!(usize::MAX - functions * size_of::<TypedBody>() < 5 * size_of::<Option<Projection>>());
    reject_scalar_inventory(&facts, 85, "E0400", 6);

    let mut facts = nonzero_inventory_primitive();
    let bindings = usize::MAX / size_of::<Option<ParameterTy>>();
    facts.2.bindings = bindings;
    facts.0.materialized_capacity[1] = bindings;
    facts.0.materialized_capacity[11] = bindings;
    facts.0.scratch_endpoint_capacity[0] = bindings;
    facts.1.retained_capacities[5] = bindings;
    facts.1.retained_lengths[5] = bindings;
    assert!(
        usize::MAX - bindings * size_of::<Option<ParameterTy>>() < 5 * size_of::<Option<ValueTy>>()
    );
    reject_scalar_inventory(&facts, 85, "E0400", 4); // staging addition

    let mut facts = nonzero_inventory_primitive();
    facts.2.presence_slots = usize::MAX;
    facts.0.materialized_capacity[10] = usize::MAX;
    facts.0.scratch_endpoint_capacity[5] = usize::MAX;
    reject_scalar_inventory(&facts, 85, "E0400", 12); // bool width1, scratch addition

    let mut facts = nonzero_inventory_primitive();
    facts.2.calls = usize::MAX;
    facts.0.materialized_vectors[9] = usize::MAX;
    facts.0.scratch_endpoints[4] = usize::MAX;
    reject_scalar_inventory(&facts, 85, "E0400", 11); // materialized event addition

    for mutant in 0..4 {
        let mut facts = nonzero_inventory_primitive();
        let mut attempts = 85;
        match mutant {
            0 => facts.0.path_capacity_fields = usize::MAX,
            1 => facts.1.path_capacity_fields = usize::MAX,
            2 => facts.0.path_vectors = usize::MAX, // vector events + paths
            _ => {
                let paths = usize::MAX / 64 + 1;
                facts.0.path_vectors = paths;
                facts.1.path_vectors = paths;
                attempts = 82 + paths;
            }
        }
        reject_scalar_inventory(
            &facts,
            attempts,
            if mutant == 3 { "E0500" } else { "E0400" },
            16,
        );
    }
    // path_slots adds a subset of retained lengths. Its overflow cannot follow
    // successful positive-width retained products and their checked total;
    // manufacturing that branch would require bypassing an earlier guard.
}

#[test]
fn c3_t0_match_child_branch_roles_are_typed_and_charged() {
    macro_rules! role {
        ($field:ident: $ty:ty) => {{
            let _: for<'a> fn(&'a BodyChildBranches) -> &'a $ty = |model| &model.$field;
            (
                std::mem::offset_of!(BodyChildBranches, $field),
                size_of::<$ty>(),
                align_of::<$ty>(),
            )
        }};
    }
    let roles = [
        role!(while_body: &'static BodyBlockId),
        role!(if_then: &'static BodyBlockId),
        role!(if_else: &'static Option<BodyBlockId>),
        role!(match_guard_arms: &'static Vec<MatchArm>),
        role!(match_child_arms: &'static Vec<MatchArm>),
        role!(match_get_return: Option<&'static MatchArm>),
        role!(match_arm: &'static MatchArm),
        role!(match_branch_return: Option<BodyBlockId>),
    ];
    assert_eq!(roles.len(), 8);
    let mut occupied = 0;
    for (position, &(offset, bytes, alignment)) in roles.iter().enumerate() {
        assert_eq!(offset % alignment, 0);
        assert!(offset + bytes <= size_of::<BodyChildBranches>());
        occupied += bytes;
        for &(other, width, _) in &roles[..position] {
            assert!(offset + bytes <= other || other + width <= offset);
        }
    }
    assert!(occupied <= size_of::<BodyChildBranches>());
    assert_eq!(
        count_carrier_bytes(),
        size_of::<CountReturnCarriers>()
            + size_of::<CountGuardCarriers>()
            + size_of::<BodyCountCarriers>()
            + size_of::<FunctionCountCarriers>()
    );
    println!(
        "ENUM_BODY_COUNT_LAYOUT branches={} occupied={} body={} total={}",
        size_of::<BodyChildBranches>(),
        occupied,
        size_of::<BodyCountCarriers>(),
        count_carrier_bytes()
    );
}
