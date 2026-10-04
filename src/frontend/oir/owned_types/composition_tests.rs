use super::tests::{record, source};
use super::*;

fn nested(id: usize, children: &[usize], span: Span) -> RawRecordDecl {
    let mut declaration = record(id, &vec![hir::Ty::Unit; children.len()], span);
    for (field, child) in declaration.fields.iter_mut().zip(children) {
        field.ty = ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(*child))));
    }
    declaration
}

#[test]
fn forward_composition_preserves_nominal_order_padding_and_recursive_width() {
    let (sources, span) = source();
    let mut outer = record(
        0,
        &[hir::Ty::Bool, hir::Ty::Unit, hir::Ty::Unit, hir::Ty::Bool],
        span,
    );
    outer.fields[1].ty = ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(1))));
    outer.fields[2].ty = ParameterTy::Value(ValueTy::Owned(AggregateTy::FixedArray(
        FixedArrayTy::check(hir::Ty::I32, 3).unwrap(),
    )));
    let declarations =
        Declarations::check(&[outer, record(1, &[hir::Ty::I32], span)], &sources).unwrap();
    let outer = declarations.record(RecordId(0)).unwrap();
    assert_eq!(outer.layout(), Layout { size: 24, align: 4 });
    assert_eq!(
        declarations
            .fields(RecordId(0))
            .unwrap()
            .iter()
            .map(FieldDecl::offset)
            .collect::<Vec<_>>(),
        [0, 4, 8, 20]
    );
    assert_eq!(
        declarations
            .aggregate_width(AggregateTy::Record(RecordId(0)))
            .unwrap(),
        6
    );
    assert_eq!(declarations.usage.layout_bytes, 28);
    assert_eq!(
        declarations.fields(RecordId(0)).unwrap()[1].value_ty(),
        ValueTy::Owned(AggregateTy::Record(RecordId(1)))
    );
}

#[test]
fn direct_mutual_and_disconnected_cycles_reject() {
    let (sources, span) = source();
    for raw in [
        vec![nested(0, &[0], span)],
        vec![nested(0, &[1], span), nested(1, &[0], span)],
        vec![
            record(0, &[], span),
            nested(1, &[2], span),
            nested(2, &[1], span),
        ],
    ] {
        assert!(matches!(
            Declarations::check(&raw, &sources),
            Err(DeclarationError::ContainmentCycle(_))
        ));
    }
    assert!(matches!(
        Declarations::check(&[nested(0, &[usize::MAX], span)], &sources),
        Err(DeclarationError::InvalidRecordId(_))
    ));
}

#[test]
fn containment_depth_exact_limit_and_one_over_both_declaration_orders() {
    let (sources, span) = source();
    for reverse in [false, true] {
        for length in [64usize, 65] {
            let raw: Vec<_> = (0..length)
                .map(|id| {
                    let child = if reverse {
                        id.checked_sub(1)
                    } else if id + 1 < length {
                        Some(id + 1)
                    } else {
                        None
                    };
                    match child {
                        Some(child) => nested(id, &[child], span),
                        None => record(id, &[hir::Ty::I32], span),
                    }
                })
                .collect();
            let result = Declarations::check(&raw, &sources);
            if length == 64 {
                assert!(result.is_ok());
            } else {
                assert!(matches!(
                    result,
                    Err(DeclarationError::ResourceLimit("record containment depth"))
                ));
            }
        }
    }
}

#[test]
fn empty_children_and_zero_arrays_have_positive_storage_and_work() {
    let (sources, span) = source();
    let mut root = nested(0, &[1, 1], span);
    root.fields[1].ty = ParameterTy::Value(ValueTy::Owned(AggregateTy::FixedArray(
        FixedArrayTy::check(hir::Ty::I32, 0).unwrap(),
    )));
    let d = Declarations::check(&[root, record(1, &[], span)], &sources).unwrap();
    assert_eq!(
        d.record(RecordId(0)).unwrap().layout(),
        Layout { size: 8, align: 4 }
    );
    assert_eq!(
        d.aggregate_width(AggregateTy::Record(RecordId(0))).unwrap(),
        2
    );
}

#[test]
fn compact_exponential_dag_is_rejected_before_transitive_expansion() {
    let (sources, span) = source();
    let raw: Vec<_> = (0..32)
        .map(|id| {
            if id == 31 {
                record(id, &[hir::Ty::Bool], span)
            } else {
                nested(id, &[id + 1, id + 1], span)
            }
        })
        .collect();
    assert!(matches!(
        Declarations::check(&raw, &sources),
        Err(DeclarationError::ResourceLimit("record expanded width"))
    ));
}

#[test]
fn composed_layout_sum_boundary_is_independently_enforced() {
    let (sources, span) = source();
    let raw = [nested(0, &[1, 1], span), record(1, &[hir::Ty::I32], span)];
    assert!(Declarations::check_with_limits(
        &raw,
        &sources,
        Limits {
            layout_bytes: 12,
            ..Limits::DEFAULT
        }
    )
    .is_ok());
    assert!(matches!(
        Declarations::check_with_limits(
            &raw,
            &sources,
            Limits {
                layout_bytes: 11,
                ..Limits::DEFAULT
            }
        ),
        Err(DeclarationError::ResourceLimit("declaration layout bytes"))
    ));
}

#[test]
fn projection_rederives_each_nominal_hop_and_never_accepts_scalar_or_array_hops() {
    let (sources, span) = source();
    let mut root = nested(0, &[1, 1], span);
    root.fields[1].ty = ParameterTy::Value(ValueTy::Owned(AggregateTy::FixedArray(
        FixedArrayTy::check(hir::Ty::I32, 3).unwrap(),
    )));
    let d = Declarations::check(
        &[root, record(1, &[hir::Ty::Bool, hir::Ty::I32], span)],
        &sources,
    )
    .unwrap();
    let root = AggregateTy::Record(RecordId(0));
    let f = |r, index| FieldId {
        record: RecordId(r),
        index,
    };
    assert_eq!(
        d.projection(root, &[f(0, 0), f(1, 1)]).unwrap(),
        (ValueTy::Scalar(hir::Ty::I32), 4)
    );
    assert_eq!(d.projection(root, &[f(0, 1)]).unwrap().1, 8);
    for path in [
        vec![],
        vec![f(1, 0)],
        vec![f(0, 0), f(0, 1)],
        vec![f(0, 0), f(1, 1), f(1, 0)],
        vec![f(0, 1), f(1, 0)],
        vec![f(0, usize::MAX)],
        vec![f(0, 0); 65],
    ] {
        assert!(d.projection(root, &path).is_err(), "{path:?}");
    }
}

#[test]
fn lazy_leaves_have_recursive_width_and_exact_offsets_including_empty_sentinels() {
    let (sources, span) = source();
    let mut root = nested(0, &[1, 2, 2], span);
    root.fields[2].ty = ParameterTy::Value(ValueTy::Owned(AggregateTy::FixedArray(
        FixedArrayTy::check(hir::Ty::I32, 0).unwrap(),
    )));
    let d = Declarations::check(
        &[
            root,
            record(1, &[hir::Ty::Bool, hir::Ty::I32], span),
            record(2, &[], span),
        ],
        &sources,
    )
    .unwrap();
    let ty = AggregateTy::Record(RecordId(0));
    let leaves: Vec<_> = d.leaves(ty).unwrap().map(|l| (l.offset, l.ty)).collect();
    assert_eq!(
        leaves,
        [
            (0, hir::Ty::Bool),
            (4, hir::Ty::I32),
            (8, hir::Ty::Unit),
            (12, hir::Ty::I32)
        ]
    );
    assert_eq!(leaves.len(), d.aggregate_width(ty).unwrap());
    assert_eq!(d.aggregate_layout(ty).unwrap().size(), 16);
}

#[test]
fn lazy_leaves_support_exact_depth_limit_without_host_recursion() {
    let (sources, span) = source();
    let raw: Vec<_> = (0..64)
        .map(|id| {
            if id == 63 {
                record(id, &[hir::Ty::I32], span)
            } else {
                nested(id, &[id + 1], span)
            }
        })
        .collect();
    let d = Declarations::check(&raw, &sources).unwrap();
    assert_eq!(
        d.leaves(AggregateTy::Record(RecordId(0)))
            .unwrap()
            .map(|l| (l.offset, l.ty))
            .collect::<Vec<_>>(),
        [(0, hir::Ty::I32)]
    );
}

#[test]
fn source_inventory_and_checked_facade_agree_without_executable_authority() {
    let (sources, span) = source();
    let raw = [
        nested(0, &[1, 1], span),
        record(1, &[hir::Ty::Bool, hir::Ty::I32], span),
    ];
    let types = || {
        raw.iter()
            .map(|r| r.fields.iter().map(|f| value_field(f).unwrap()))
    };
    assert_eq!(
        admit_value_layouts(types()).unwrap(),
        Declarations::check(&raw, &sources)
            .unwrap()
            .usage
            .layout_bytes
    );
    assert!(matches!(
        admit_value_layouts(
            [vec![ValueTy::Owned(AggregateTy::Record(RecordId(0)))]]
                .into_iter()
                .map(Vec::into_iter)
        ),
        Err(DeclarationError::ContainmentCycle(RecordId(0)))
    ));
}
