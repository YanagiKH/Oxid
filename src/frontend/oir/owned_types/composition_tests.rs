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
