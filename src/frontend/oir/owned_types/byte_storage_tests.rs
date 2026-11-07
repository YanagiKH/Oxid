//! RFC0031 structural admission. Expected sizes/widths follow the closed form,
//! and record admission is tested independently of public source resolution.
use super::tests::{record, source};
use super::*;

#[test]
fn byte_storage_every_length_has_unit_stride_and_full_logical_width() {
    let (sources, _) = source();
    let declarations = Declarations::check(&[], &sources).unwrap();
    for n in 0..=1024 {
        let array = FixedArrayTy::check(hir::Ty::U8, n).unwrap();
        let aggregate = AggregateTy::FixedArray(array);
        assert_eq!(array.stride(), 1);
        assert_eq!(
            declarations.aggregate_layout(aggregate),
            Ok(Layout {
                size: n.max(1),
                align: 1
            })
        );
        assert_eq!(declarations.aggregate_width(aggregate), Ok(n.max(1)));
        assert_eq!(
            declarations.check_value_type(ValueTy::Owned(aggregate)),
            Ok(())
        );
        let leaves: Vec<_> = declarations.leaves(aggregate).unwrap().collect();
        assert_eq!(leaves.len(), n.max(1));
    }
    for n in [1025, 65_536, usize::MAX] {
        assert_eq!(
            FixedArrayTy::check(hir::Ty::U8, n),
            Err(DeclarationError::ResourceLimit("fixed array length"))
        );
    }
}

#[test]
fn byte_storage_borrow_matrix_is_directional_and_preserves_element_identity() {
    let (sources, _) = source();
    let declarations = Declarations::check(&[], &sources).unwrap();
    for n in [0, 1, 1024] {
        for element in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::U8, hir::Ty::Unit] {
            let array = AggregateTy::FixedArray(FixedArrayTy::check(element, n).unwrap());
            let exact = BorrowedTy::Exact(array);
            let slice = BorrowedTy::ScalarSlice(hir::Ty::U8);
            assert_eq!(slice.accepts(exact), element == hir::Ty::U8);
            assert!(!exact.accepts(slice));
            assert_eq!(
                declarations.check_borrowed_view(exact, slice).is_ok(),
                element == hir::Ty::U8
            );
            assert_eq!(BorrowedSlot::check(exact).unwrap().referent(), exact);
            assert_eq!(
                AggregateSlot::try_from_aggregate(array)
                    .unwrap()
                    .aggregate(),
                array
            );
        }
    }
    let slice = BorrowedTy::ScalarSlice(hir::Ty::U8);
    assert!(!slice.accepts(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))));
    assert!(!slice.accepts(BorrowedTy::Exact(AggregateTy::Enum(EnumId(0)))));
    assert!(!slice.accepts(BorrowedTy::ScalarSlice(hir::Ty::Bool)));
}

#[test]
fn byte_storage_raw_unused_and_nested_record_fields_keep_bounded_layout_and_output_reservations() {
    let (mut sources, span) = source();
    let imported_file = sources.add("imported.ox".into(), "field".into());
    let imported_span = sources.get(imported_file).span(0, 5);
    for n in [0, 1, 1024] {
        for ty in [
            ValueTy::Scalar(hir::Ty::U8),
            ValueTy::Owned(AggregateTy::FixedArray(
                FixedArrayTy::check(hir::Ty::U8, n).unwrap(),
            )),
        ] {
            let width = if matches!(ty, ValueTy::Scalar(_)) {
                1
            } else {
                n.max(1)
            };
            for (nested, field_span) in [(false, span), (true, span), (true, imported_span)] {
                let mut inner = record(usize::from(nested), &[hir::Ty::Unit], field_span);
                inner.fields[0].ty = ParameterTy::Value(ty);
                let id = inner.fields[0].id;
                assert_eq!(value_field(&inner.fields[0]), Ok(ty));
                let summary = value_summary(ty, &[]).unwrap();
                assert_eq!(
                    summary.layout,
                    Layout {
                        size: width,
                        align: 1
                    }
                );
                assert_eq!(summary.width, width);
                let raw = if nested {
                    let mut outer = record(0, &[hir::Ty::Unit], span);
                    outer.fields[0].ty =
                        ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(1))));
                    vec![outer, inner]
                } else {
                    vec![inner]
                };
                let mut allocator = Allocator::default();
                let checked = Declarations::check_combined_with_limits(
                    &raw,
                    &[],
                    &sources,
                    Limits::DEFAULT,
                    &mut allocator,
                )
                .unwrap();
                // One flat record vector and one flat field vector. Byte fields
                // introduce no enum scratch or per-field output allocation.
                assert_eq!(allocator.attempts, 2);
                for failure in [1, 2, 3] {
                    let mut allocator = Allocator {
                        fail_at: Some(failure),
                        ..Default::default()
                    };
                    let result = Declarations::check_combined_with_limits(
                        &raw,
                        &[],
                        &sources,
                        Limits::DEFAULT,
                        &mut allocator,
                    );
                    assert_eq!(result.is_ok(), failure == 3);
                    assert_eq!(allocator.attempts, failure.min(2));
                }
                let output_bytes = size_of::<EnumDeclarations>()
                    + raw.len() * (size_of::<RecordDecl>() + size_of::<FieldDecl>());
                assert_eq!(checked.usage().table_bytes, output_bytes);
                for limits in [
                    Limits {
                        table_bytes: output_bytes - 1,
                        ..Limits::DEFAULT
                    },
                    Limits {
                        layout_bytes: width * raw.len() - 1,
                        ..Limits::DEFAULT
                    },
                ] {
                    let mut allocator = Allocator {
                        fail_at: Some(1),
                        ..Default::default()
                    };
                    assert!(matches!(
                        Declarations::check_combined_with_limits(
                            &raw,
                            &[],
                            &sources,
                            limits,
                            &mut allocator,
                        ),
                        Err(DeclarationError::ResourceLimit(_))
                    ));
                    assert_eq!(allocator.attempts, 0);
                }
                let root = AggregateTy::Record(RecordId(0));
                assert_eq!(
                    checked.aggregate_layout(root).unwrap(),
                    Layout {
                        size: width,
                        align: 1
                    }
                );
                assert_eq!(checked.aggregate_width(root).unwrap(), width);
                assert_eq!(checked.field(id.record, id).unwrap().span(), field_span);
                let leaves: Vec<_> = checked
                    .leaves(root)
                    .unwrap()
                    .map(|leaf| (leaf.offset, leaf.ty))
                    .collect();
                assert_eq!(
                    leaves,
                    (0..width)
                        .map(|offset| (offset, hir::Ty::U8))
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    admit_value_layouts(raw.iter().map(|record| {
                        record
                            .fields
                            .iter()
                            .map(|field| value_field(field).unwrap())
                    }))
                    .unwrap(),
                    width * raw.len()
                );
            }
        }
    }
}

#[test]
fn byte_storage_previous_record_elements_and_owned_enum_arrays() {
    let (sources, span) = source();
    for element in [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit] {
        for n in [0, 1, 1024] {
            let mut raw = record(0, &[hir::Ty::Unit], span);
            raw.fields[0].ty = ParameterTy::Value(ValueTy::Owned(AggregateTy::FixedArray(
                FixedArrayTy::check(element, n).unwrap(),
            )));
            assert!(Declarations::check(&[raw], &sources).is_ok());
        }
    }
    assert_eq!(
        admit_scalar_layouts(std::iter::once(std::iter::once(hir::Ty::U8))),
        Ok(1)
    );
    for n in [0, 1, 1024] {
        let enumeration = RawEnumDecl {
            id: EnumId(0),
            span,
            variants: vec![RawVariantDecl {
                id: VariantId {
                    enumeration: EnumId(0),
                    index: 0,
                },
                span,
                payload: Some(ParameterTy::Value(ValueTy::Owned(AggregateTy::FixedArray(
                    FixedArrayTy::check(hir::Ty::U8, n).unwrap(),
                )))),
            }],
        };
        // Complete fixed arrays and byte record fields are admitted. Bare scalar
        // u8 enum payloads retain their separate existing exclusion.
        assert!(Declarations::check_combined(&[], &[enumeration], &sources).is_ok());
    }
}

#[test]
fn byte_storage_lane_a_actual_carrier_inventory() {
    use crate::frontend::{ast, declaration_index, diagnostic::Diagnostic};
    use std::mem::{align_of, size_of};
    macro_rules! rows { ($($ty:ty),+ $(,)?) => { $(println!("BYTE-STORAGE-LANE-A {} size={} align={}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());)+ }; }
    rows!(ast::ArrayElementTypeSyntax, ast::ScalarTypeSyntax, ast::FixedArraySyntax,
        ast::TypeSyntaxKind, ast::TypeSyntax, ast::EnumPayloadSyntax, ast::StructField,
        ast::StructDecl, ast::Param, ast::Function, ast::Expr, ast::ExprKind,
        FixedArrayTy, BorrowedTy, BorrowedSlot, AggregateSlot, ValueTy, ParameterTy,
        RawFieldDecl, RawRecordDecl, FieldDecl, RecordDecl, Declarations,
        ContainmentSummary, Layout, ScalarLeaves<'_>, declaration_index::QuerySession<'_, '_>,
        Result<FixedArrayTy, Box<Diagnostic>>, Result<ParameterTy, Box<Diagnostic>>,
        Result<ValueTy, Box<Diagnostic>>, Result<ValueTy, DeclarationError>,
        DeclarationError, Diagnostic, Box<Diagnostic>, std::fmt::Arguments<'_>,
        Result<(ast::FixedArraySyntax, usize), Box<Diagnostic>>,
        Result<(ast::TypeSyntaxKind, usize), Box<Diagnostic>>);
}

#[test]
fn byte_storage_raw_guard_transient_roles_do_not_change_output_table_metric() {
    use std::mem::{align_of, size_of};
    #[allow(dead_code)]
    struct Guard {
        captured: FixedArrayTy,
        element_receiver: FixedArrayTy,
        element_return: hir::Ty,
        compared: hir::Ty,
        equality: bool,
        selected: bool,
    }
    // Explicit new nonrecursive raw guard role deltas, separate from source
    // HIR payment. DeclarationUsage.table_bytes promises requested retained
    // output payload, NOT caller stack or total memory: do not misprice these
    // transient carriers as output rows or silently broaden that metric.
    let members = 2 * size_of::<FixedArrayTy>() + 2 * size_of::<hir::Ty>() + 2;
    assert_eq!(
        size_of::<Guard>(),
        members.div_ceil(align_of::<Guard>()) * align_of::<Guard>()
    );
    let (sources, _) = source();
    let declarations = Declarations::check(&[], &sources).unwrap();
    assert_eq!(
        declarations.usage().table_bytes,
        size_of::<EnumDeclarations>()
    );
    println!("RFC0031 raw fixed guard size={} align={} conservative_two_helpers={} retained_table_delta=0 output_metric_excludes_transient_carriers=true", size_of::<Guard>(), align_of::<Guard>(), 2 * size_of::<Guard>());
    println!("RFC0031 raw inherited caller/error roles field_ref={} value={} field_result={} summary_ref={} summary_result={} error={} optional_error={}", size_of::<&RawFieldDecl>(), size_of::<ValueTy>(), size_of::<Result<ValueTy, DeclarationError>>(), size_of::<&[ContainmentSummary]>(), size_of::<Result<ContainmentSummary, DeclarationError>>(), size_of::<DeclarationError>(), size_of::<Option<DeclarationError>>());
}

#[test]
fn byte_record_mixed_layout_keeps_padding_and_neighbor_offsets() {
    let (sources, span) = source();
    let mut raw = record(
        0,
        &[hir::Ty::U8, hir::Ty::I32, hir::Ty::U8, hir::Ty::Bool],
        span,
    );
    raw.fields[2].ty = ParameterTy::Value(ValueTy::Owned(AggregateTy::FixedArray(
        FixedArrayTy::check(hir::Ty::U8, 3).unwrap(),
    )));
    let checked = Declarations::check(&[raw], &sources).unwrap();
    let root = AggregateTy::Record(RecordId(0));
    assert_eq!(
        checked.aggregate_layout(root).unwrap(),
        Layout { size: 12, align: 4 }
    );
    assert_eq!(checked.aggregate_width(root).unwrap(), 6);
    assert_eq!(
        checked
            .fields(RecordId(0))
            .unwrap()
            .iter()
            .map(FieldDecl::offset)
            .collect::<Vec<_>>(),
        [0, 4, 8, 11]
    );
    assert_eq!(
        checked
            .leaves(root)
            .unwrap()
            .map(|leaf| (leaf.offset, leaf.ty))
            .collect::<Vec<_>>(),
        [
            (0, hir::Ty::U8),
            (4, hir::Ty::I32),
            (8, hir::Ty::U8),
            (9, hir::Ty::U8),
            (10, hir::Ty::U8),
            (11, hir::Ty::Bool),
        ]
    );
}
