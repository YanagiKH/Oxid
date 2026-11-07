//! Inert nominal/layout facts for the separate owned-record-vector domain.
//! Neither this query nor a compact slot can authorize source or execution.
use super::*;

#[derive(Clone, Copy, Debug)]
pub(in crate::frontend::oir) struct OwnedRecordVectorType<'a> {
    declarations: &'a Declarations,
    record: RecordId,
}

/// The complete synchronous requested query envelope, including caller and
/// callee results. It is additional logical metadata, not a stack/RSS bound.
struct QueryCarriers {
    declarations: [&'static Declarations; 2],
    records: [RecordId; 2],
    selected: &'static RecordDecl,
    selected_result: Result<&'static RecordDecl, DeclarationError>,
    layouts: [Layout; 2],
    constructed: OwnedRecordVectorType<'static>,
    result: Result<OwnedRecordVectorType<'static>, DeclarationError>,
    caller_result: Result<OwnedRecordVectorType<'static>, DeclarationError>,
    checked_ordinal: Result<u32, std::num::TryFromIntError>,
    mapped_ordinal: Result<u32, DeclarationError>,
    metadata_limit: usize,
    bounded_metadata_limit: usize,
    query_request: usize,
    work: &'static mut usize,
    checked_subtraction: Option<usize>,
    remaining: Result<usize, DeclarationError>,
    selected_work: usize,
    copy_exclusion: bool,
    positive_layout: bool,
}

impl<'a> OwnedRecordVectorType<'a> {
    pub(in crate::frontend::oir) const fn requested_query_bytes() -> usize {
        size_of::<QueryCarriers>()
    }

    /// The borrowed facade already proved every record/enum containment graph
    /// finite and denied owned-vector declarations. Three logical visits pay
    /// the nominal lookup, Copy exclusion and checked record-layout query.
    /// This query performs no allocation and traverses no element values.
    pub(in crate::frontend::oir) fn check(
        declarations: &'a Declarations,
        record: RecordId,
        metadata_limit: usize,
        work: &mut usize,
    ) -> Result<Self, DeclarationError> {
        if Self::requested_query_bytes() > metadata_limit.min(Limits::DEFAULT.table_bytes) {
            return Err(DeclarationError::ResourceLimit(
                "owned vector type query bytes",
            ));
        }
        let remaining = work.checked_sub(3).ok_or(DeclarationError::ResourceLimit(
            "owned vector type query work",
        ))?;
        u32::try_from(record.0).map_err(|_| DeclarationError::InvalidRecordId(record))?;
        let selected = declarations.record(record)?;
        if selected.is_copy() || selected.layout().size() == 0 {
            return Err(DeclarationError::TypeMismatch);
        }
        *work = remaining;
        Ok(Self {
            declarations,
            record,
        })
    }

    pub(in crate::frontend::oir) const fn record(self) -> RecordId {
        self.record
    }

    pub(in crate::frontend::oir) const fn aggregate(self) -> AggregateTy {
        AggregateTy::OwnedRecordVector(self.record)
    }

    pub(in crate::frontend::oir) const fn descriptor_layout(self) -> Layout {
        Self::storage_layout()
    }

    pub(in crate::frontend::oir) const fn descriptor_width(self) -> usize {
        Self::storage_width()
    }

    /// Fixed storage facts alone confer no nominal or execution authority.
    pub(in crate::frontend::oir) const fn storage_layout() -> Layout {
        Layout { size: 16, align: 8 }
    }

    pub(in crate::frontend::oir) const fn storage_width() -> usize {
        3
    }

    pub(in crate::frontend::oir) fn belongs_to(self, declarations: &Declarations) -> bool {
        std::ptr::eq(self.declarations, declarations)
    }
}

/// Borrowed nominal query for both disjoint owned-vector root domains. The
/// compact root slot preserves the legacy query carrier's qualified width.
#[derive(Clone, Copy, Debug)]
pub(in crate::frontend::oir) struct OwnedVectorType<'a> {
    declarations: &'a Declarations,
    root: OwnedVectorRootSlot,
}

struct NominalQueryCarriers {
    declarations: [&'static Declarations; 2],
    roots: [OwnedVectorRoot; 3],
    slots: [OwnedVectorRootSlot; 2],
    slot_return: Option<OwnedVectorRootSlot>,
    selected_record: &'static RecordDecl,
    record_return: Result<&'static RecordDecl, DeclarationError>,
    selected_enum: &'static EnumDecl,
    enum_return: Result<&'static EnumDecl, EnumDeclarationError>,
    mapped_enum_return: Result<&'static EnumDecl, DeclarationError>,
    layouts: [Layout; 2],
    constructed: OwnedVectorType<'static>,
    result: Result<OwnedVectorType<'static>, DeclarationError>,
    caller_result: Result<OwnedVectorType<'static>, DeclarationError>,
    metadata_limit: usize,
    query_request: usize,
    work: &'static mut usize,
    remaining: Option<usize>,
    selected_work: usize,
    copy_exclusion: bool,
}

impl<'a> OwnedVectorType<'a> {
    pub(in crate::frontend::oir) const fn requested_query_bytes() -> usize {
        size_of::<NominalQueryCarriers>()
    }
    pub(in crate::frontend::oir) fn check(
        declarations: &'a Declarations,
        root: OwnedVectorRoot,
        metadata_limit: usize,
        work: &mut usize,
    ) -> Result<Self, DeclarationError> {
        if Self::requested_query_bytes() > metadata_limit.min(Limits::DEFAULT.table_bytes) {
            return Err(DeclarationError::ResourceLimit(
                "owned nominal vector query bytes",
            ));
        }
        let remaining = work.checked_sub(3).ok_or(DeclarationError::ResourceLimit(
            "owned nominal vector query work",
        ))?;
        let slot = OwnedVectorRootSlot::check(root).ok_or(DeclarationError::TypeMismatch)?;
        let layout = match root {
            OwnedVectorRoot::Record(id) => {
                let selected = declarations.record(id)?;
                if selected.is_copy() {
                    return Err(DeclarationError::TypeMismatch);
                }
                selected.layout()
            }
            OwnedVectorRoot::Enum(id) => declarations
                .enums()
                .enumeration(id)
                .map_err(DeclarationError::from)?
                .layout(),
        };
        if layout.size() == 0 {
            return Err(DeclarationError::TypeMismatch);
        }
        *work = remaining;
        Ok(Self {
            declarations,
            root: slot,
        })
    }
    pub(in crate::frontend::oir) const fn root(self) -> OwnedVectorRoot {
        self.root.root()
    }
    pub(in crate::frontend::oir) const fn aggregate(self) -> AggregateTy {
        self.root().vector()
    }
    pub(in crate::frontend::oir) const fn descriptor_layout(self) -> Layout {
        Self::storage_layout()
    }
    pub(in crate::frontend::oir) const fn descriptor_width(self) -> usize {
        Self::storage_width()
    }
    pub(in crate::frontend::oir) const fn storage_layout() -> Layout {
        OwnedRecordVectorType::storage_layout()
    }
    pub(in crate::frontend::oir) const fn storage_width() -> usize {
        OwnedRecordVectorType::storage_width()
    }
    pub(in crate::frontend::oir) fn belongs_to(self, declarations: &Declarations) -> bool {
        std::ptr::eq(self.declarations, declarations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked_declarations() -> (SourceMap, Declarations, Span) {
        let mut sources = SourceMap::new();
        let file = sources.add("owned-vector-type.ox".into(), "record types".into());
        let span = sources.get(file).span(0, 6);
        let raw = [false, true]
            .into_iter()
            .enumerate()
            .map(|(id, is_copy)| RawRecordDecl {
                id: RecordId(id),
                is_copy,
                span,
                fields: vec![RawFieldDecl {
                    id: FieldId {
                        record: RecordId(id),
                        index: 0,
                    },
                    ty: ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32)),
                    span,
                }],
            })
            .collect::<Vec<_>>();
        let declarations = Declarations::check(&raw, &sources).unwrap();
        (sources, declarations, span)
    }

    #[test]
    fn owned_vector_type_query_pays_exact_metadata_and_work_without_admitting_execution() {
        let (_, declarations, _) = checked_declarations();
        let request = OwnedRecordVectorType::requested_query_bytes();
        let mut work = 3;
        assert!(matches!(
            OwnedRecordVectorType::check(&declarations, RecordId(0), request - 1, &mut work),
            Err(DeclarationError::ResourceLimit(
                "owned vector type query bytes"
            ))
        ));
        assert_eq!(work, 3);
        work = 2;
        assert!(matches!(
            OwnedRecordVectorType::check(&declarations, RecordId(0), request, &mut work),
            Err(DeclarationError::ResourceLimit(
                "owned vector type query work"
            ))
        ));
        assert_eq!(work, 2);
        work = 3;
        let ty =
            OwnedRecordVectorType::check(&declarations, RecordId(0), request, &mut work).unwrap();
        assert_eq!(work, 0);
        assert_eq!(ty.record(), RecordId(0));
        assert_eq!(ty.descriptor_layout(), Layout { size: 16, align: 8 });
        assert_eq!(ty.descriptor_width(), 3);
        assert!(ty.belongs_to(&declarations));
        let (_, other, _) = checked_declarations();
        assert!(!ty.belongs_to(&other));
        assert_eq!(
            declarations.check_aggregate_type(ty.aggregate()),
            Err(DeclarationError::TypeMismatch)
        );
        assert_eq!(
            declarations.aggregate_layout(ty.aggregate()),
            Err(DeclarationError::TypeMismatch)
        );
        assert_eq!(
            declarations.aggregate_width(ty.aggregate()),
            Err(DeclarationError::TypeMismatch)
        );
        assert!(!declarations.is_copy(ty.aggregate()));
    }

    #[test]
    fn owned_vector_type_compact_tags_are_distinct_and_do_not_grant_slice_authority() {
        for id in [0, 1, 4095, u32::MAX as usize] {
            let aggregate = AggregateTy::OwnedRecordVector(RecordId(id));
            let slot = AggregateSlot::try_from_aggregate(aggregate).unwrap();
            assert_eq!(slot.aggregate(), aggregate);
            assert_ne!(
                slot,
                AggregateSlot::try_from_aggregate(AggregateTy::RecordVector(RecordId(id))).unwrap()
            );
            let borrowed = BorrowedTy::Exact(aggregate);
            assert_eq!(BorrowedSlot::check(borrowed).unwrap().referent(), borrowed);
            assert_eq!(borrowed.element(), None);
            assert!(!BorrowedTy::RecordSlice(RecordId(id)).accepts(borrowed));
            assert!(!BorrowedTy::ScalarSlice(hir::Ty::I32).accepts(borrowed));
        }
        #[cfg(target_pointer_width = "64")]
        {
            let invalid = RecordId(u32::MAX as usize + 1);
            assert!(
                AggregateSlot::try_from_aggregate(AggregateTy::OwnedRecordVector(invalid)).is_err()
            );
            assert!(
                BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::OwnedRecordVector(invalid)))
                    .is_err()
            );
            assert_eq!(size_of::<AggregateTy>(), 16);
            assert_eq!(size_of::<AggregateSlot>(), 8);
            assert_eq!(size_of::<BorrowedSlot>(), 8);
            assert_eq!(size_of::<ParameterTy>(), 24);
            assert_eq!(size_of::<ValueTy>(), 16);
        }
    }

    #[test]
    fn owned_vector_type_old_compact_discriminants_and_widths_are_preserved() {
        let array = FixedArrayTy::check(hir::Ty::I32, 3).unwrap();
        let aggregates = [
            (AggregateTy::Record(RecordId(0)), 0),
            (AggregateTy::Enum(EnumId(0)), 1),
            (AggregateTy::FixedArray(array), 2),
            (AggregateTy::ByteBuf, 3),
            (AggregateTy::File, 4),
            (AggregateTy::Vector(hir::Ty::I32), 5),
            (AggregateTy::RecordVector(RecordId(0)), 6),
            (AggregateTy::OwnedRecordVector(RecordId(0)), 7),
        ];
        for (aggregate, expected) in aggregates {
            let slot = AggregateSlot::try_from_aggregate(aggregate).unwrap();
            // SAFETY: repr(C) data enums begin with their initialized u32
            // discriminant. No payload padding or uninitialized byte is read.
            let tag = unsafe {
                std::ptr::read_unaligned((&slot.0 as *const AggregateSlotRepr).cast::<u32>())
            };
            assert_eq!(tag, expected);
            assert_eq!(slot.aggregate(), aggregate);
        }
        let borrowed = [
            (BorrowedTy::Exact(AggregateTy::Record(RecordId(0))), 0),
            (BorrowedTy::Exact(AggregateTy::FixedArray(array)), 1),
            (BorrowedTy::ScalarSlice(hir::Ty::I32), 2),
            (BorrowedTy::Exact(AggregateTy::ByteBuf), 3),
            (BorrowedTy::Exact(AggregateTy::File), 4),
            (BorrowedTy::Exact(AggregateTy::Vector(hir::Ty::I32)), 5),
            (BorrowedTy::Exact(AggregateTy::RecordVector(RecordId(0))), 6),
            (BorrowedTy::RecordSlice(RecordId(0)), 7),
            (
                BorrowedTy::Exact(AggregateTy::OwnedRecordVector(RecordId(0))),
                8,
            ),
        ];
        for (referent, expected) in borrowed {
            let slot = BorrowedSlot::check(referent).unwrap();
            // SAFETY: Read only the initialized repr(C) u32 discriminant.
            let tag = unsafe {
                std::ptr::read_unaligned((&slot.0 as *const BorrowedSlotRepr).cast::<u32>())
            };
            assert_eq!(tag, expected);
            assert_eq!(slot.referent(), referent);
        }
    }

    #[test]
    fn owned_vector_type_copy_forged_and_nested_declarations_remain_rejected() {
        let (sources, declarations, span) = checked_declarations();
        for record in [RecordId(1), RecordId(2), RecordId(usize::MAX)] {
            let mut work = 3;
            assert!(OwnedRecordVectorType::check(
                &declarations,
                record,
                OwnedRecordVectorType::requested_query_bytes(),
                &mut work,
            )
            .is_err());
            assert_eq!(work, 3);
        }
        let field = RawFieldDecl {
            id: FieldId {
                record: RecordId(0),
                index: 0,
            },
            ty: ParameterTy::Value(ValueTy::Owned(AggregateTy::OwnedRecordVector(RecordId(0)))),
            span,
        };
        assert!(matches!(
            Declarations::check(
                &[RawRecordDecl {
                    id: RecordId(0),
                    is_copy: false,
                    span,
                    fields: vec![field],
                }],
                &sources,
            ),
            Err(DeclarationError::TypeMismatch)
        ));
        let variant = RawVariantDecl {
            id: VariantId {
                enumeration: EnumId(0),
                index: 0,
            },
            payload: Some(ParameterTy::Value(ValueTy::Owned(
                AggregateTy::OwnedRecordVector(RecordId(0)),
            ))),
            span,
        };
        assert!(Declarations::check_combined(
            &[],
            &[RawEnumDecl {
                id: EnumId(0),
                span,
                variants: vec![variant],
            }],
            &sources,
        )
        .is_err());
    }

    #[test]
    fn owned_vector_type_retained_facade_query_observes_actual_record_field_and_cache_capacity() {
        let (_, mut declarations, _) = checked_declarations();
        let before = declarations.retained_storage_bytes().unwrap();
        let capacity = declarations.records.capacity();
        declarations
            .records
            .try_reserve_exact(capacity + 3)
            .unwrap();
        let after_records = declarations.retained_storage_bytes().unwrap();
        assert_eq!(
            after_records - before,
            (declarations.records.capacity() - capacity) * size_of::<RecordDecl>()
        );
        let capacity = declarations.fields.capacity();
        declarations.fields.try_reserve_exact(capacity + 5).unwrap();
        let after_fields = declarations.retained_storage_bytes().unwrap();
        assert_eq!(
            after_fields - after_records,
            (declarations.fields.capacity() - capacity) * size_of::<FieldDecl>()
        );
        let capacity = declarations.storage.resources.capacity();
        declarations
            .storage
            .resources
            .try_reserve_exact(capacity + 7)
            .unwrap();
        let after_cache = declarations.retained_storage_bytes().unwrap();
        assert_eq!(
            after_cache - after_fields,
            (declarations.storage.resources.capacity() - capacity)
                * size_of::<resources::Presence>()
        );
        assert!(checked_declaration_temporary_request(usize::MAX, 1).is_err());
    }
}
