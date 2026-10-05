//! Disconnected accounting controls; test-owned HIR parts are not source witnesses.
use super::*;
use crate::frontend::{
    declaration_index::{collect_enum_candidate, IndexLimits, SourceOwner},
    lexer, parser,
    source::{SourceFileId, SourceMap, SourceView},
};
use std::mem::align_of;

fn at() -> Span {
    let mut map = SourceMap::new();
    let file = map.add("inventory.ox".into(), "x".into());
    map.get(file).span(0, 1)
}
fn scalar_plan() -> HirPlan {
    let mut map = SourceMap::new();
    map.add(
        "inventory-plan.ox".into(),
        "enum Unused{V} fn main()->i32{return 0;}".into(),
    );
    let source = map.get(SourceFileId(0));
    let ast = parser::parse_enum_candidate_counted(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut Allocator::default(),
        &mut Default::default(),
    )
    .unwrap()
    .0;
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&map)).unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_enum_candidate(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
    super::super::hir_budget::preflight_enum_hir(&index, &WorkMeter::default())
        .unwrap()
        .unwrap()
}
fn paid_scalar_parts(plan: &HirPlan, allocator: &mut Allocator) -> (PaidStorage, ResolvedParts) {
    let at = at();
    let mut paid = PaidStorage::new(plan.counts);
    let records = paid.reserve(allocator, Kind::Records, 0, at).unwrap();
    let mut signatures = paid.reserve(allocator, Kind::Signatures, 1, at).unwrap();
    let params = paid.reserve(allocator, Kind::Parameters, 0, at).unwrap();
    signatures.push(Signature {
        params,
        result: ValueTy::Scalar(Ty::I32),
        span: at,
    });
    let mut functions = paid.reserve(allocator, Kind::Functions, 1, at).unwrap();
    let bindings = paid.reserve(allocator, Kind::Bindings, 0, at).unwrap();
    let mut expressions = paid.reserve(allocator, Kind::Expressions, 1, at).unwrap();
    expressions.push(Expr {
        kind: ExprKind::I32(0),
        span: at,
    });
    let mut blocks = paid.reserve(allocator, Kind::Blocks, 1, at).unwrap();
    let mut body = paid.reserve(allocator, Kind::Statements, 1, at).unwrap();
    body.push(Stmt {
        kind: StmtKind::Return(Some(ExprId(0))),
        span: at,
    });
    blocks.push(BodyBlock {
        body,
        span: at,
        end: at,
    });
    functions.push(Function {
        id: DefId(0),
        bindings,
        expressions,
        body: BodyBlockId(0),
        blocks,
        end: at,
    });
    let scope = PaidScope {
        names: paid.reserve(allocator, Kind::Names, 0, at).unwrap(),
        exits: paid.reserve(allocator, Kind::Exits, 0, at).unwrap(),
        marks: paid.reserve(allocator, Kind::Marks, 1, at).unwrap(),
    };
    let loops = paid.reserve(allocator, Kind::Loops, 1, at).unwrap();
    let frames = paid.reserve(allocator, Kind::Frames, 12, at).unwrap();
    paid.observe_scratch(&scope, &loops, &frames, at).unwrap();
    (paid, (records, signatures, functions))
}
fn row_binding(at: Span) -> Binding {
    Binding {
        mutable: false,
        span: at,
        annotation: None,
        scope: BodyBlockId(0),
        parameter_position: None,
    }
}
fn row_field(at: Span, index: usize) -> Field {
    Field {
        id: FieldId {
            record: RecordId(0),
            index,
        },
        ty: ValueTy::Scalar(Ty::I32),
        name_span: at,
        span: at,
    }
}

#[test]
fn c3a_inventory_reads_every_actual_nested_vector_length_and_capacity_without_allocating() {
    let at = at();
    let mut fields = Vec::with_capacity(3);
    fields.extend([row_field(at, 0), row_field(at, 1)]);
    let mut records = Vec::with_capacity(2);
    records.push(Record {
        id: RecordId(0),
        name_span: at,
        span: at,
        fields,
        end: at,
    });
    let mut params = Vec::with_capacity(3);
    params.extend([ParameterTy::Value(ValueTy::Scalar(Ty::I32)); 2]);
    let mut signatures = Vec::with_capacity(2);
    signatures.push(Signature {
        params,
        result: ValueTy::Scalar(Ty::I32),
        span: at,
    });
    let mut bindings = Vec::with_capacity(4);
    bindings.extend([row_binding(at), row_binding(at)]);
    let mut args = Vec::with_capacity(4);
    args.extend([Argument::Value(ExprId(0)), Argument::Value(ExprId(0))]);
    let mut initializers = Vec::with_capacity(5);
    initializers.push(FieldInit {
        field: FieldId {
            record: RecordId(0),
            index: 0,
        },
        value: ExprId(0),
        span: at,
    });
    let mut elements = Vec::with_capacity(6);
    elements.extend([ExprId(0), ExprId(0)]);
    let mut expressions = Vec::with_capacity(5);
    expressions.extend([
        Expr {
            kind: ExprKind::Call {
                target: DefId(0),
                args,
            },
            span: at,
        },
        Expr {
            kind: ExprKind::StructLiteral {
                record: RecordId(0),
                fields: initializers,
            },
            span: at,
        },
        Expr {
            kind: ExprKind::ArrayLiteral { elements },
            span: at,
        },
    ]);
    let mut body = Vec::with_capacity(4);
    body.extend([
        Stmt {
            kind: StmtKind::Return(None),
            span: at,
        },
        Stmt {
            kind: StmtKind::Expr(ExprId(0)),
            span: at,
        },
    ]);
    let mut blocks = Vec::with_capacity(3);
    blocks.extend([
        BodyBlock {
            body,
            span: at,
            end: at,
        },
        BodyBlock {
            body: Vec::with_capacity(2),
            span: at,
            end: at,
        },
    ]);
    let mut functions = Vec::with_capacity(2);
    functions.push(Function {
        id: DefId(0),
        bindings,
        expressions,
        body: BodyBlockId(0),
        blocks,
        end: at,
    });
    let parts = (records, signatures, functions);
    let work = WorkMeter::default();
    let (result, stats) =
        super::super::reviewer_source::integration_measured(|| inventory_parts(&parts, &work, at));
    let inventory = result.unwrap();
    assert_eq!(stats, (0, 0, 0));
    assert_eq!(inventory.counts, [1, 2, 1, 2, 1, 2, 3, 2, 2, 2, 1, 2]);
    assert_eq!(
        inventory.capacities,
        [2, 3, 2, 3, 2, 4, 5, 3, 6, 4, 5, 6, 0, 0, 0, 0, 0]
    );
    assert!(work.used() > 0);
}

#[test]
fn c3a_inventory_reconciliation_uses_real_reserves_and_returns_fixed_payload_totals() {
    let plan = scalar_plan();
    let at = at();
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(13).unwrap();
    let (result, (_, live, peak)) = super::super::reviewer_source::integration_measured(|| {
        let (paid, parts) = paid_scalar_parts(&plan, &mut allocator);
        let work = WorkMeter::default();
        let inventory = inventory_parts(&parts, &work, at).unwrap();
        let result = paid.reconcile(&plan, inventory, allocator.attempts, &work, at);
        drop(parts);
        result
    });
    let observation = result.unwrap();
    assert_eq!(live, 0);
    assert!(peak > 0);
    assert_eq!(
        observation.retained_counts,
        [0, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 0]
    );
    assert_eq!(
        observation.capacities,
        [0, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 12]
    );
    assert_eq!(
        (
            observation.retained_bytes,
            observation.scratch_capacity_bytes
        ),
        (480, 304)
    );
    assert_eq!(observation.reservation_attempts, 13);
    assert!(
        observation.plan.total > observation.retained_bytes + observation.scratch_capacity_bytes
    );
}

#[test]
fn c3a_inventory_defensively_rejects_synthetic_enum_retention_in_every_type_context() {
    use crate::frontend::oir::owned_types::EnumId;
    let plan = scalar_plan();
    let at = at();
    let enumeration = AggregateTy::Enum(EnumId(0));
    for mutant in 0..7 {
        let (_, mut parts) = paid_scalar_parts(&plan, &mut Allocator::default());
        match mutant {
            0 => {
                parts.2[0].expressions[0].kind = ExprKind::ConstructEnum {
                    variant: VariantId {
                        enumeration: EnumId(0),
                        index: 0,
                    },
                    payload: None,
                }
            }
            1 => {
                parts.2[0].blocks[0].body[0].kind = StmtKind::Match {
                    scrutinee: BindingId(0),
                    arms: Vec::new(),
                }
            }
            2 => parts.1[0].result = ValueTy::Owned(enumeration),
            3 => parts.1[0]
                .params
                .push(ParameterTy::Value(ValueTy::Owned(enumeration))),
            4 => parts.1[0].params.push(ParameterTy::Reference {
                referent: BorrowedTy::Exact(enumeration),
                kind: BorrowKind::Shared,
            }),
            5 => {
                let mut binding = row_binding(at);
                binding.annotation = Some(ValueTy::Owned(enumeration));
                parts.2[0].bindings.push(binding);
            }
            _ => {
                let mut field = row_field(at, 0);
                field.ty = ValueTy::Owned(enumeration);
                parts.0.push(Record {
                    id: RecordId(0),
                    name_span: at,
                    span: at,
                    fields: vec![field],
                    end: at,
                });
            }
        }
        let error = inventory_parts(&parts, &WorkMeter::default(), at).unwrap_err();
        assert_eq!(error.code, "E0500", "synthetic enum mutant {mutant}");
    }
}

#[test]
fn c3a_inventory_reconciliation_rejects_unpaid_actual_growth_counts_and_scratch() {
    let plan = scalar_plan();
    let at = at();
    for mutant in 0..5 {
        let (mut paid, mut parts) = paid_scalar_parts(&plan, &mut Allocator::default());
        if mutant == 0 {
            // Real extra capacity in a private test vector, never an updated
            // expected count or a retroactive quota increase.
            parts.2[0].expressions.try_reserve_exact(1).unwrap();
            assert_eq!(parts.2[0].expressions.capacity(), 2);
        }
        let mut inventory = inventory_parts(&parts, &WorkMeter::default(), at).unwrap();
        match mutant {
            0 => (),
            1 => inventory.counts[Kind::Expressions as usize] = 2,
            2 => inventory.capacities[Kind::Names as usize] = 1,
            3 => paid.scratch[Kind::Frames as usize - RETAINED_KINDS] = 0,
            _ => paid.remaining[Kind::Expressions as usize] = 1,
        }
        assert!(paid
            .reconcile(&plan, inventory, 13, &WorkMeter::default(), at)
            .is_err());
    }
}

#[test]
fn c3a_inventory_work_exhaustion_and_checked_totals_drop_errors_without_heap_retention() {
    let plan = scalar_plan();
    let at = at();
    let (paid, parts) = paid_scalar_parts(&plan, &mut Allocator::default());
    // Minimal fixture: 3 top vectors; signature row+params vector; function
    // row+3 vectors; expression row; block row+body vector+statement row = 13.
    for limit in [0, 1, 12, 13, 14] {
        let (_, (_, live, _)) = super::super::reviewer_source::integration_measured(|| {
            let work = WorkMeter::new(limit);
            let result = inventory_parts(&parts, &work, at);
            assert_eq!(result.is_ok(), limit >= 13);
            assert_eq!(work.used(), limit.min(13));
            drop(result);
        });
        assert_eq!(live, 0);
    }
    for limit in [0, 16, 17, 18] {
        let inventory = inventory_parts(&parts, &WorkMeter::default(), at).unwrap();
        let (_, (_, live, _)) = super::super::reviewer_source::integration_measured(|| {
            let work = WorkMeter::new(limit);
            let result = paid.reconcile(&plan, inventory, 13, &work, at);
            assert_eq!(result.is_ok(), limit >= 17);
            assert_eq!(work.used(), limit.min(17));
            drop(result);
        });
        assert_eq!(live, 0);
    }
    let mut overflow = ResolverInventory::default();
    overflow.counts[Kind::Expressions as usize] = usize::MAX;
    let one = vec![Expr {
        kind: ExprKind::I32(0),
        span: at,
    }];
    assert_eq!(
        overflow
            .vector(Kind::Expressions, &one, &WorkMeter::default(), at)
            .unwrap_err()
            .code,
        "E0400"
    );
    let mut wrong_width = ResolverInventory::default();
    assert_eq!(
        wrong_width
            .vector(Kind::Expressions, &vec![0u8], &WorkMeter::default(), at)
            .unwrap_err()
            .code,
        "E0500"
    );
    // Arithmetic-only synthetic quota, impossible through source preflight.
    let mut plan = plan;
    plan.counts = HirCounts {
        expressions: usize::MAX,
        ..HirCounts::default()
    };
    let mut paid = PaidStorage::new(plan.counts);
    paid.remaining[Kind::Expressions as usize] = 0;
    paid.reserved[Kind::Expressions as usize] = usize::MAX;
    let mut inventory = ResolverInventory::default();
    inventory.capacities[Kind::Expressions as usize] = usize::MAX;
    assert_eq!(
        paid.reconcile(&plan, inventory, 0, &WorkMeter::default(), at)
            .unwrap_err()
            .code,
        "E0400"
    );
}

#[test]
fn c3a_inventory_complete_accumulator_and_return_carriers_are_measured() {
    macro_rules! layout { ($($ty:ty),* $(,)?) => { $(
        println!("C3A_INVENTORY_LAYOUT {} bytes={} align={}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());
    )* }; }
    layout!(ResolverInventory, Result<ResolverInventory, Box<Diagnostic>>,
        ResolverStorageObservation, Result<ResolverStorageObservation, Box<Diagnostic>>,
        InventoryInputs, InventorySliceLoop<Record>, InventoryWalkCarriers, InventoryCarriers);
    let members = 3 * size_of::<ResolverInventory>()
        + size_of::<Result<ResolverInventory, Box<Diagnostic>>>()
        + size_of::<Result<ResolverStorageObservation, Box<Diagnostic>>>()
        + size_of::<ResolverStorageObservation>()
        + size_of::<InventoryInputs>()
        + size_of::<InventoryWalkCarriers>()
        + size_of::<[usize; KINDS]>()
        + size_of::<[usize; 2]>()
        + 3 * size_of::<Result<usize, Box<Diagnostic>>>()
        + 3 * size_of::<Result<(), Box<Diagnostic>>>();
    assert!(
        size_of::<InventoryInputs>()
            >= size_of::<&HirPlan>()
                + size_of::<&ResolvedParts>()
                + size_of::<&Vec<Record>>()
                + size_of::<&Vec<Signature>>()
                + size_of::<&Vec<Function>>()
                + size_of::<&Vec<Expr>>()
                + size_of::<&mut ResolverInventory>()
                + size_of::<&PaidStorage>()
                + 2 * size_of::<&WorkMeter>()
                + 2 * size_of::<Span>()
                + size_of::<Kind>()
    );
    assert!(inventory_carrier_bytes() >= members);
    assert_eq!(
        fixed_carrier_bytes(),
        size_of::<FixedCarriers>() + inventory_carrier_bytes()
    );
}

#[test]
fn c3a_inventory_typed_loop_cursors_and_next_returns_are_separate_from_vector_borrows() {
    macro_rules! loop_size {
        ($ty:ty) => {{
            let members = size_of::<std::slice::Iter<'_, $ty>>()
                + size_of::<Option<&$ty>>()
                + size_of::<&$ty>();
            assert!(size_of::<InventorySliceLoop<$ty>>() >= members);
            println!(
                "C3A_INVENTORY_LOOP {} iterator={} next={} current={} envelope={}",
                stringify!($ty),
                size_of::<std::slice::Iter<'_, $ty>>(),
                size_of::<Option<&$ty>>(),
                size_of::<&$ty>(),
                size_of::<InventorySliceLoop<$ty>>()
            );
            size_of::<InventorySliceLoop<$ty>>()
        }};
    }
    let members = loop_size!(Record)
        + loop_size!(Field)
        + loop_size!(Signature)
        + loop_size!(ParameterTy)
        + loop_size!(Function)
        + loop_size!(Binding)
        + loop_size!(Expr)
        + loop_size!(BodyBlock)
        + loop_size!(Stmt)
        + size_of::<std::ops::Range<usize>>()
        + size_of::<Option<usize>>()
        + size_of::<usize>();
    assert!(size_of::<InventoryWalkCarriers>() >= members);
    println!(
        "C3A_INVENTORY_WALK envelope={} inputs={} total={}",
        size_of::<InventoryWalkCarriers>(),
        size_of::<InventoryInputs>(),
        inventory_carrier_bytes()
    );
}
