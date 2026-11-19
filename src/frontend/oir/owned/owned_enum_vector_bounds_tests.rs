use super::*;
use std::mem::{align_of, size_of};

#[test]
fn owned_enum_vector_ineligible_elements_remain_distinct_from_malformed_roots() {
    use crate::frontend::oir::owned_types::{
        FieldId, ParameterTy, RawEnumDecl, RawFieldDecl, RawRecordDecl, RawVariantDecl, VariantId,
    };
    use crate::frontend::{hir, source::SourceMap};

    let mut sources = SourceMap::new();
    let file = sources.add("element-refusals.ox".into(), "element refusals".into());
    let span = sources.get(file).span(0, 7);
    let records: Vec<_> = [
        (
            false,
            vec![ValueTy::Owned(AggregateTy::OwnedEnumVector(EnumId(0)))],
        ),
        (true, vec![ValueTy::Scalar(hir::Ty::I32)]),
        (false, vec![ValueTy::Scalar(hir::Ty::I32)]),
        (false, vec![]),
    ]
    .into_iter()
    .enumerate()
    .map(|(record, (is_copy, types))| RawRecordDecl {
        id: RecordId(record),
        is_copy,
        span,
        fields: types
            .into_iter()
            .enumerate()
            .map(|(index, ty)| RawFieldDecl {
                id: FieldId {
                    record: RecordId(record),
                    index,
                },
                ty: ParameterTy::Value(ty),
                span,
            })
            .collect(),
    })
    .collect();
    let enums: Vec<_> = [
        ValueTy::Scalar(hir::Ty::I32),
        ValueTy::Owned(AggregateTy::Record(RecordId(0))),
    ]
    .into_iter()
    .enumerate()
    .map(|(enumeration, payload)| RawEnumDecl {
        id: EnumId(enumeration),
        span,
        variants: vec![RawVariantDecl {
            id: VariantId {
                enumeration: EnumId(enumeration),
                index: 0,
            },
            payload: Some(ParameterTy::Value(payload)),
            span,
        }],
    })
    .collect();
    let declarations =
        Declarations::check_combined_owned_vectors(&records, &enums, &sources).unwrap();
    let request = CheckedElements::requested_bytes(&declarations).unwrap();
    let mut checked = CheckedElements::check(&declarations, request).unwrap();
    for root in [
        OwnedVectorRoot::Record(RecordId(0)),
        OwnedVectorRoot::Enum(EnumId(1)),
    ] {
        assert!(matches!(
            checked.element_root(root),
            Err(ShapeFailure::UnsupportedElement)
        ));
    }
    for root in [
        OwnedVectorRoot::Record(RecordId(1)),
        OwnedVectorRoot::Record(RecordId(4)),
        OwnedVectorRoot::Enum(EnumId(2)),
        OwnedVectorRoot::Enum(EnumId(4096)),
    ] {
        assert!(matches!(
            checked.element_root(root),
            Err(ShapeFailure::Type)
        ));
    }
    assert!(checked
        .element_root(OwnedVectorRoot::Record(RecordId(2)))
        .is_ok());
    let empty = checked
        .element_root(OwnedVectorRoot::Record(RecordId(3)))
        .unwrap();
    assert_eq!(empty.bytes, 1);
    checked.bounds[2] = Bounds {
        visits: 0,
        resources: 1,
    };
    assert!(matches!(
        checked.element_root(OwnedVectorRoot::Record(RecordId(2))),
        Err(ShapeFailure::Type)
    ));
    assert!(matches!(
        element_root(&declarations, &[], OwnedVectorRoot::Enum(EnumId(0))),
        Err(ShapeFailure::Type)
    ));
}

#[test]
fn owned_enum_vector_root_lookup_uses_real_return_types_and_independent_physical_roles() {
    let _: for<'a> fn(&'a Declarations, &[Bounds], OwnedVectorRoot) -> Result<Element<'a>> =
        element_root;
    let _: for<'a> fn(&'a ElementQueryCarriers) -> &'a Result<Element<'static>> =
        |bank| &bank.returned;
    let _: for<'a> fn(&'a ElementQueryCarriers) -> &'a Result<Element<'static>> =
        |bank| &bank.caller_return;
    assert_eq!(
        (size_of::<OwnedVectorRoot>(), align_of::<OwnedVectorRoot>()),
        (16, 8)
    );
    assert_eq!(
        (
            size_of::<OwnedVectorRootSlot>(),
            align_of::<OwnedVectorRootSlot>()
        ),
        (4, 4)
    );
    assert_eq!(
        (
            size_of::<Option<OwnedVectorRootSlot>>(),
            align_of::<Option<OwnedVectorRootSlot>>()
        ),
        (8, 4)
    );
    assert_eq!(
        (size_of::<Element<'_>>(), align_of::<Element<'_>>()),
        (40, 8)
    );
    assert_eq!(
        (
            size_of::<Result<Element<'_>>>(),
            align_of::<Result<Element<'_>>>()
        ),
        (40, 8)
    );
    assert_eq!(size_of::<super::super::super::DeclarationError>(), 32);
    assert_eq!(
        size_of::<
            std::result::Result<super::super::super::Layout, super::super::super::DeclarationError>,
        >(),
        32
    );
    // repr(C) role inventory, independent of the production bank's size/fee:
    // two argument borrows24, root argument/caller32, aggregate16, optional
    // compact return8, selected4+alignment4, ordinal8+Result16, validation32,
    // layout16+Result32, selected/table Option borrows16, bytes8, bool1+
    // alignment7, constructed40 and two actual returned transports80.
    let oracle = 24 + 32 + 16 + 8 + 4 + 4 + 8 + 16 + 32 + 16 + 32 + 16 + 8 + 1 + 7 + 40 + 80;
    assert_eq!(oracle, 344);
    assert_eq!(size_of::<ElementQueryCarriers>(), oracle);
    assert_eq!(element_control_bytes(), oracle);
    assert_eq!(std::mem::offset_of!(ElementQueryCarriers, returned), 264);
    assert_eq!(
        std::mem::offset_of!(ElementQueryCarriers, caller_return),
        304
    );
}
