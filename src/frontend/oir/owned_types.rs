//! Private ownership-phase declaration groundwork, not a source/runtime feature.
//!
//! This module deliberately does not change `hir::Ty`, scalar slots, or the
//! `VerifiedProgram` boundary. Only scalar fields have storage here; references
//! describe call parameters, never Rust references or stored language values.
//! Remove this narrowly scoped allowance when ownership verification consumes
//! the table. Keeping the implementation in production checks its interface even
//! while struct/borrow syntax and ownership execution remain disabled.
#![allow(dead_code)]

use super::{hir, SourceMap, Span};
use std::mem::size_of;

/// Compilation-local nominal identity, assigned in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RecordId(pub(super) usize);

/// An ordinal alone is not a field identity: its declaring record is essential.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FieldId {
    pub(super) record: RecordId,
    pub(super) index: usize,
}

/// Function-local storage identity, independent of source binding mutability.
/// Rust `Copy` on these IDs grants no source-language copying permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OwnerPlaceId(pub(super) usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LoanId(pub(super) usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CallSiteId(pub(super) usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ValueTy {
    Scalar(hir::Ty),
    Owned(RecordId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BorrowKind {
    Shared,
    Exclusive,
}

/// References exist only in parameter descriptors, not in `ValueTy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ParameterTy {
    Value(ValueTy),
    Reference { record: RecordId, kind: BorrowKind },
}

#[derive(Debug)]
pub(super) struct RawRecordDecl {
    pub(super) id: RecordId,
    pub(super) span: Span,
    pub(super) fields: Vec<RawFieldDecl>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RawFieldDecl {
    pub(super) id: FieldId,
    // Raw input can express unsupported fields so validation, not a trusted
    // producer, is responsible for excluding nested records and references.
    pub(super) ty: ParameterTy,
    pub(super) span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeclarationError {
    ResourceLimit(&'static str),
    InvalidRecordId(RecordId),
    InvalidFieldId(FieldId),
    InvalidSpan(Span),
    NonScalarField(FieldId),
    RecordMismatch,
    TypeMismatch,
    LayoutOverflow,
    Allocation,
}

/// Private Linux x86_64 storage layout, not a source or FFI ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Layout {
    size: usize,
    align: usize,
}
impl Layout {
    pub(super) fn size(self) -> usize {
        self.size
    }
    pub(super) fn align(self) -> usize {
        self.align
    }
    fn scalar(ty: hir::Ty) -> Self {
        match ty {
            hir::Ty::Bool | hir::Ty::Unit => Self { size: 1, align: 1 },
            hir::Ty::I32 => Self { size: 4, align: 4 },
        }
    }
}

#[derive(Debug)]
pub(super) struct FieldDecl {
    id: FieldId,
    ty: hir::Ty,
    span: Span,
    offset: usize,
}
impl FieldDecl {
    pub(super) fn id(&self) -> FieldId {
        self.id
    }
    pub(super) fn ty(&self) -> hir::Ty {
        self.ty
    }
    pub(super) fn span(&self) -> Span {
        self.span
    }
    pub(super) fn offset(&self) -> usize {
        self.offset
    }
}

#[derive(Debug)]
pub(super) struct RecordDecl {
    id: RecordId,
    span: Span,
    field_start: usize,
    field_end: usize,
    layout: Layout,
}
impl RecordDecl {
    pub(super) fn id(&self) -> RecordId {
        self.id
    }
    pub(super) fn span(&self) -> Span {
        self.span
    }
    pub(super) fn layout(&self) -> Layout {
        self.layout
    }
}

/// Requested output payload, excluding caller-owned raw input, allocator
/// overhead and these constant-size vector headers. `layout_bytes` charges one
/// instance of each declaration, not runtime slots or activation storage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct DeclarationUsage {
    pub(super) records: usize,
    pub(super) fields: usize,
    pub(super) table_bytes: usize,
    pub(super) layout_bytes: usize,
}

/// Unit-1 engineering ceilings only. Runtime storage, expanded operations and
/// ownership analysis will need their own admission gates before source enablement.
///
/// On Linux x86_64, checked records/fields occupy 64/56 bytes, so the current
/// count ceilings imply at most 64*4096 + 56*65536 = 3,932,160 payload bytes.
/// A nonempty record's padded layout is at most four bytes per scalar field;
/// empty records cost one byte. The maximum is therefore
/// 4*65536 + 4096 - ceil(65536/1024) = 266,176 layout bytes, attained by 64 full
/// i32 records plus 4032 empty records. Both byte ceilings are deliberately
/// redundant today and independently enforced/tested with lowered limits.
#[derive(Clone, Copy)]
struct Limits {
    records: usize,
    fields_per_record: usize,
    fields: usize,
    table_bytes: usize,
    layout_bytes: usize,
}
impl Limits {
    const DEFAULT: Self = Self {
        records: 4_096,
        fields_per_record: 1_024,
        fields: 65_536,
        table_bytes: 8 * 1_024 * 1_024,
        layout_bytes: 1_024 * 1_024,
    };

    /// The private boundary-test seam can lower but never raise any ceiling.
    fn bounded(self) -> Self {
        Self {
            records: self.records.min(Self::DEFAULT.records),
            fields_per_record: self.fields_per_record.min(Self::DEFAULT.fields_per_record),
            fields: self.fields.min(Self::DEFAULT.fields),
            table_bytes: self.table_bytes.min(Self::DEFAULT.table_bytes),
            layout_bytes: self.layout_bytes.min(Self::DEFAULT.layout_bytes),
        }
    }
}

/// Only `check` constructs this facade. No raw/mutable table access, consumer
/// methods, or conversion to a verified executable program is provided.
#[derive(Debug)]
pub(super) struct Declarations {
    records: Vec<RecordDecl>,
    fields: Vec<FieldDecl>,
    usage: DeclarationUsage,
}
impl Declarations {
    pub(super) fn check(
        raw: &[RawRecordDecl],
        sources: &SourceMap,
    ) -> Result<Self, DeclarationError> {
        Self::check_with_limits(raw, sources, Limits::DEFAULT)
    }

    fn check_with_limits(
        raw: &[RawRecordDecl],
        sources: &SourceMap,
        limits: Limits,
    ) -> Result<Self, DeclarationError> {
        let limits = limits.bounded();
        // Preflight slice lengths and the entire requested allocation payload
        // before inspecting field contents or allocating any output/scratch.
        let mut usage = preflight(raw, limits)?;
        for (index, record) in raw.iter().enumerate() {
            if record.id != RecordId(index) {
                return Err(DeclarationError::InvalidRecordId(record.id));
            }
            check_span(sources, record.span)?;
            let mut cursor = LayoutCursor::default();
            for (index, field) in record.fields.iter().enumerate() {
                if field.id
                    != (FieldId {
                        record: record.id,
                        index,
                    })
                {
                    return Err(DeclarationError::InvalidFieldId(field.id));
                }
                check_span(sources, field.span)?;
                cursor.push(Layout::scalar(scalar_field(field)?))?;
            }
            let layout = cursor.finish()?;
            usage.layout_bytes = limited_add(
                usage.layout_bytes,
                layout.size,
                limits.layout_bytes,
                "declaration layout bytes",
            )?;
        }

        // All malformed declarations and all size/overflow gates above reject
        // before these two fallible allocations. Flat fields avoid R separate
        // allocations, with O(R + F) persistent storage and O(1) scratch.
        let mut records = Vec::new();
        records
            .try_reserve_exact(usage.records)
            .map_err(|_| DeclarationError::Allocation)?;
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(usage.fields)
            .map_err(|_| DeclarationError::Allocation)?;
        for record in raw {
            let field_start = fields.len();
            let mut cursor = LayoutCursor::default();
            for field in &record.fields {
                let ty = scalar_field(field)?;
                let offset = cursor.push(Layout::scalar(ty))?;
                fields.push(FieldDecl {
                    id: field.id,
                    ty,
                    span: field.span,
                    offset,
                });
            }
            records.push(RecordDecl {
                id: record.id,
                span: record.span,
                field_start,
                field_end: fields.len(),
                layout: cursor.finish()?,
            });
        }
        Ok(Self {
            records,
            fields,
            usage,
        })
    }

    pub(super) fn usage(&self) -> DeclarationUsage {
        self.usage
    }
    pub(super) fn records(&self) -> &[RecordDecl] {
        &self.records
    }
    pub(super) fn record(&self, id: RecordId) -> Result<&RecordDecl, DeclarationError> {
        self.records
            .get(id.0)
            .filter(|record| record.id == id)
            .ok_or(DeclarationError::InvalidRecordId(id))
    }
    pub(super) fn fields(&self, id: RecordId) -> Result<&[FieldDecl], DeclarationError> {
        let record = self.record(id)?;
        self.fields
            .get(record.field_start..record.field_end)
            .ok_or(DeclarationError::InvalidRecordId(id))
    }
    pub(super) fn field(
        &self,
        record: RecordId,
        id: FieldId,
    ) -> Result<&FieldDecl, DeclarationError> {
        let fields = self.fields(record)?;
        if id.record != record {
            return Err(DeclarationError::RecordMismatch);
        }
        fields
            .get(id.index)
            .filter(|field| field.id == id)
            .ok_or(DeclarationError::InvalidFieldId(id))
    }
    pub(super) fn check_value_type(&self, ty: ValueTy) -> Result<(), DeclarationError> {
        match ty {
            ValueTy::Scalar(_) => Ok(()),
            ValueTy::Owned(id) => self.record(id).map(|_| ()),
        }
    }
    pub(super) fn check_parameter_type(&self, ty: ParameterTy) -> Result<(), DeclarationError> {
        match ty {
            ParameterTy::Value(value) => self.check_value_type(value),
            ParameterTy::Reference { record, .. } => self.record(record).map(|_| ()),
        }
    }
    pub(super) fn same_value_type(
        &self,
        actual: ValueTy,
        expected: ValueTy,
    ) -> Result<(), DeclarationError> {
        self.check_value_type(actual)?;
        self.check_value_type(expected)?;
        if actual == expected {
            Ok(())
        } else {
            Err(DeclarationError::TypeMismatch)
        }
    }
}

fn check_span(sources: &SourceMap, span: Span) -> Result<(), DeclarationError> {
    if sources.is_valid_span(span) {
        Ok(())
    } else {
        Err(DeclarationError::InvalidSpan(span))
    }
}

fn scalar_field(field: &RawFieldDecl) -> Result<hir::Ty, DeclarationError> {
    match field.ty {
        ParameterTy::Value(ValueTy::Scalar(ty)) => Ok(ty),
        _ => Err(DeclarationError::NonScalarField(field.id)),
    }
}

fn limited_add(
    total: usize,
    amount: usize,
    limit: usize,
    resource: &'static str,
) -> Result<usize, DeclarationError> {
    total
        .checked_add(amount)
        .filter(|&sum| sum <= limit)
        .ok_or(DeclarationError::ResourceLimit(resource))
}

fn table_bytes(records: usize, fields: usize) -> Result<usize, DeclarationError> {
    let records = records.checked_mul(size_of::<RecordDecl>());
    let fields = fields.checked_mul(size_of::<FieldDecl>());
    records
        .zip(fields)
        .and_then(|(records, fields)| records.checked_add(fields))
        .ok_or(DeclarationError::ResourceLimit("declaration table bytes"))
}

fn preflight(raw: &[RawRecordDecl], limits: Limits) -> Result<DeclarationUsage, DeclarationError> {
    let records = limited_add(0, raw.len(), limits.records, "record declarations")?;
    let mut fields = 0;
    for record in raw {
        if record.fields.len() > limits.fields_per_record {
            return Err(DeclarationError::ResourceLimit("fields per record"));
        }
        fields = limited_add(
            fields,
            record.fields.len(),
            limits.fields,
            "field declarations",
        )?;
    }
    let table_bytes = limited_add(
        0,
        table_bytes(records, fields)?,
        limits.table_bytes,
        "declaration table bytes",
    )?;
    Ok(DeclarationUsage {
        records,
        fields,
        table_bytes,
        layout_bytes: 0,
    })
}

#[derive(Default)]
struct LayoutCursor {
    end: usize,
    align: usize,
}
impl LayoutCursor {
    fn push(&mut self, field: Layout) -> Result<usize, DeclarationError> {
        let offset = align_up(self.end, field.align)?;
        let end = offset
            .checked_add(field.size)
            .ok_or(DeclarationError::LayoutOverflow)?;
        self.end = end;
        self.align = self.align.max(field.align);
        Ok(offset)
    }
    fn finish(self) -> Result<Layout, DeclarationError> {
        // Empty records receive one private byte; they are still nominal owners.
        let align = self.align.max(1);
        Ok(Layout {
            size: align_up(self.end.max(1), align)?,
            align,
        })
    }
}

fn align_up(offset: usize, align: usize) -> Result<usize, DeclarationError> {
    if !align.is_power_of_two() {
        return Err(DeclarationError::LayoutOverflow);
    }
    // Unlike (offset + align - 1) & !(align - 1), an already aligned
    // representable offset never overflows merely while computing padding.
    let padding = (align - offset % align) % align;
    offset
        .checked_add(padding)
        .ok_or(DeclarationError::LayoutOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{lexer, parser, source::SourceFileId};

    fn source() -> (SourceMap, Span) {
        let mut sources = SourceMap::new();
        let file = sources.add("owned-types.ox".into(), "record 雪 fields".into());
        let span = sources.get(file).span(0, 6);
        (sources, span)
    }

    fn record(id: usize, types: &[hir::Ty], span: Span) -> RawRecordDecl {
        RawRecordDecl {
            id: RecordId(id),
            span,
            fields: types
                .iter()
                .enumerate()
                .map(|(index, &ty)| RawFieldDecl {
                    id: FieldId {
                        record: RecordId(id),
                        index,
                    },
                    ty: ParameterTy::Value(ValueTy::Scalar(ty)),
                    span,
                })
                .collect(),
        }
    }

    fn error(raw: &[RawRecordDecl], sources: &SourceMap) -> DeclarationError {
        Declarations::check(raw, sources).unwrap_err()
    }

    #[test]
    fn empty_table_has_no_payload_or_layout_allocation() {
        let (sources, _) = source();
        let table = Declarations::check(&[], &sources).unwrap();
        assert_eq!(table.usage(), DeclarationUsage::default());
        assert!(table.records().is_empty());
        assert_eq!(table.records.capacity(), 0);
        assert_eq!(table.fields.capacity(), 0);
        assert_eq!(
            table.record(RecordId(0)).unwrap_err(),
            DeclarationError::InvalidRecordId(RecordId(0))
        );
    }

    #[test]
    fn scalar_and_empty_layouts_have_explicit_storage() {
        let (sources, span) = source();
        let raw = [
            record(0, &[], span),
            record(1, &[hir::Ty::Bool], span),
            record(2, &[hir::Ty::Unit], span),
            record(3, &[hir::Ty::I32], span),
        ];
        let table = Declarations::check(&raw, &sources).unwrap();
        for (id, expected) in [(0, (1, 1)), (1, (1, 1)), (2, (1, 1)), (3, (4, 4))] {
            let declaration = table.record(RecordId(id)).unwrap();
            assert_eq!(declaration.id(), RecordId(id));
            assert_eq!(declaration.span(), span);
            assert_eq!(
                (declaration.layout().size(), declaration.layout().align()),
                expected
            );
        }
        assert_eq!(table.usage().layout_bytes, 7);
        assert!(table.fields(RecordId(0)).unwrap().is_empty());
    }

    #[test]
    fn declaration_order_controls_offsets_and_tail_padding() {
        let (sources, span) = source();
        use hir::Ty::{Bool, Unit, I32};
        for (types, offsets, size, align) in [
            (vec![Bool, I32, Unit], vec![0, 4, 8], 12, 4),
            (vec![I32, Bool, Unit], vec![0, 4, 5], 8, 4),
            (vec![Unit, Bool, I32], vec![0, 1, 4], 8, 4),
            (vec![Bool, Unit, Bool], vec![0, 1, 2], 3, 1),
            (vec![I32, I32, I32], vec![0, 4, 8], 12, 4),
            (vec![Bool, I32, Bool, I32], vec![0, 4, 8, 12], 16, 4),
        ] {
            let raw = [record(0, &types, span)];
            let table = Declarations::check(&raw, &sources).unwrap();
            assert_eq!(
                table.record(RecordId(0)).unwrap().layout(),
                Layout { size, align }
            );
            for (index, field) in table.fields(RecordId(0)).unwrap().iter().enumerate() {
                assert_eq!(field.offset(), offsets[index]);
                assert_eq!(field.ty(), types[index]);
                assert_eq!(field.span(), span);
                assert_eq!(field.id(), raw[0].fields[index].id);
            }
        }
    }

    #[test]
    fn identical_record_layouts_remain_nominally_distinct() {
        let (sources, span) = source();
        let raw = [
            record(0, &[hir::Ty::I32], span),
            record(1, &[hir::Ty::I32], span),
        ];
        let table = Declarations::check(&raw, &sources).unwrap();
        assert_eq!(
            table.record(RecordId(0)).unwrap().layout(),
            table.record(RecordId(1)).unwrap().layout()
        );
        assert_eq!(
            table.same_value_type(ValueTy::Owned(RecordId(0)), ValueTy::Owned(RecordId(1))),
            Err(DeclarationError::TypeMismatch)
        );
        assert_eq!(
            table.field(RecordId(0), raw[1].fields[0].id).unwrap_err(),
            DeclarationError::RecordMismatch
        );
        assert_ne!(raw[0].fields[0].id, raw[1].fields[0].id);
        assert_eq!(
            table
                .field(RecordId(1), raw[1].fields[0].id)
                .unwrap()
                .offset(),
            0
        );
    }

    #[test]
    fn owner_and_loan_identities_are_separate_from_binding_mutability() {
        let owner = OwnerPlaceId(3);
        let loan = LoanId(3);
        let call = CallSiteId(3);
        // Distinct Rust types share no conversion. No mutable-binding flag or
        // host address participates in an owner's storage identity.
        assert_eq!(owner, OwnerPlaceId(3));
        assert_ne!(owner, OwnerPlaceId(4));
        assert_eq!(loan, LoanId(3));
        assert_ne!(loan, LoanId(4));
        assert_eq!(call, CallSiteId(3));
        assert_ne!(call, CallSiteId(4));
    }

    #[test]
    fn duplicate_out_of_order_and_huge_record_ids_are_rejected() {
        let (sources, span) = source();
        for id in [0, 2, usize::MAX] {
            let raw = [record(0, &[], span), record(id, &[], span)];
            assert_eq!(
                error(&raw, &sources),
                DeclarationError::InvalidRecordId(RecordId(id))
            );
        }
        assert_eq!(
            error(&[record(1, &[], span), record(0, &[], span)], &sources),
            DeclarationError::InvalidRecordId(RecordId(1))
        );
    }

    #[test]
    fn duplicate_wrong_record_and_huge_field_ids_are_rejected() {
        let (sources, span) = source();
        for id in [
            FieldId {
                record: RecordId(0),
                index: 0,
            },
            FieldId {
                record: RecordId(1),
                index: 1,
            },
            FieldId {
                record: RecordId(0),
                index: 2,
            },
            FieldId {
                record: RecordId(0),
                index: usize::MAX,
            },
        ] {
            let mut raw = [record(0, &[hir::Ty::Bool, hir::Ty::Bool], span)];
            raw[0].fields[1].id = id;
            assert_eq!(error(&raw, &sources), DeclarationError::InvalidFieldId(id));
        }
    }

    #[test]
    fn nested_owned_and_borrowed_fields_are_rejected() {
        let (sources, span) = source();
        for ty in [
            ParameterTy::Value(ValueTy::Owned(RecordId(0))),
            ParameterTy::Value(ValueTy::Owned(RecordId(usize::MAX))),
            ParameterTy::Reference {
                record: RecordId(0),
                kind: BorrowKind::Shared,
            },
            ParameterTy::Reference {
                record: RecordId(0),
                kind: BorrowKind::Exclusive,
            },
        ] {
            let mut raw = [record(0, &[hir::Ty::Bool], span)];
            raw[0].fields[0].ty = ty;
            assert_eq!(
                error(&raw, &sources),
                DeclarationError::NonScalarField(raw[0].fields[0].id)
            );
        }
    }

    #[test]
    fn all_declaration_spans_are_checked_including_later_records() {
        let (sources, span) = source();
        for bad in [
            Span {
                file: SourceFileId(usize::MAX),
                ..span
            },
            Span {
                start: 3,
                end: 2,
                ..span
            },
            Span {
                start: 0,
                end: usize::MAX,
                ..span
            },
            Span {
                start: 8,
                end: 9,
                ..span
            }, // Inside the UTF-8 snow character.
        ] {
            let raw = [record(0, &[], span), record(1, &[], bad)];
            assert_eq!(error(&raw, &sources), DeclarationError::InvalidSpan(bad));
            let mut raw = [record(0, &[hir::Ty::I32], span)];
            raw[0].fields[0].span = bad;
            assert_eq!(error(&raw, &sources), DeclarationError::InvalidSpan(bad));
        }
    }

    #[test]
    fn table_owns_checked_data_and_keeps_no_raw_alias() {
        let (sources, span) = source();
        let mut raw = [record(0, &[hir::Ty::Bool], span)];
        let table = Declarations::check(&raw, &sources).unwrap();
        raw[0].fields[0].ty = ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32));
        raw[0].fields.clear();
        assert_eq!(table.fields(RecordId(0)).unwrap()[0].ty(), hir::Ty::Bool);
        assert_eq!(table.record(RecordId(0)).unwrap().layout().size(), 1);
    }

    #[test]
    fn all_lookup_indices_are_checked() {
        let (sources, span) = source();
        let raw = [record(0, &[hir::Ty::I32], span)];
        let table = Declarations::check(&raw, &sources).unwrap();
        for id in [RecordId(1), RecordId(usize::MAX)] {
            assert_eq!(
                table.record(id).unwrap_err(),
                DeclarationError::InvalidRecordId(id)
            );
            assert_eq!(
                table.fields(id).unwrap_err(),
                DeclarationError::InvalidRecordId(id)
            );
        }
        for index in [1, usize::MAX] {
            let id = FieldId {
                record: RecordId(0),
                index,
            };
            assert_eq!(
                table.field(RecordId(0), id).unwrap_err(),
                DeclarationError::InvalidFieldId(id)
            );
        }
    }

    #[test]
    fn descriptors_validate_nominal_targets_and_scalar_mismatches() {
        let (sources, span) = source();
        let table = Declarations::check(&[record(0, &[], span)], &sources).unwrap();
        for ty in [
            ValueTy::Scalar(hir::Ty::Bool),
            ValueTy::Scalar(hir::Ty::I32),
            ValueTy::Scalar(hir::Ty::Unit),
            ValueTy::Owned(RecordId(0)),
        ] {
            assert_eq!(table.check_value_type(ty), Ok(()));
            assert_eq!(table.check_parameter_type(ParameterTy::Value(ty)), Ok(()));
            assert_eq!(table.same_value_type(ty, ty), Ok(()));
        }
        for kind in [BorrowKind::Shared, BorrowKind::Exclusive] {
            assert_eq!(
                table.check_parameter_type(ParameterTy::Reference {
                    record: RecordId(0),
                    kind
                }),
                Ok(())
            );
            assert_eq!(
                table.check_parameter_type(ParameterTy::Reference {
                    record: RecordId(1),
                    kind
                }),
                Err(DeclarationError::InvalidRecordId(RecordId(1)))
            );
        }
        let bad = ValueTy::Owned(RecordId(usize::MAX));
        assert_eq!(
            table.same_value_type(bad, bad),
            Err(DeclarationError::InvalidRecordId(RecordId(usize::MAX)))
        );
        for (actual, expected) in [
            (
                ValueTy::Scalar(hir::Ty::Bool),
                ValueTy::Scalar(hir::Ty::I32),
            ),
            (ValueTy::Scalar(hir::Ty::Unit), ValueTy::Owned(RecordId(0))),
        ] {
            assert_eq!(
                table.same_value_type(actual, expected),
                Err(DeclarationError::TypeMismatch)
            );
        }
        assert_ne!(
            ParameterTy::Reference {
                record: RecordId(0),
                kind: BorrowKind::Shared
            },
            ParameterTy::Reference {
                record: RecordId(0),
                kind: BorrowKind::Exclusive
            }
        );
    }

    #[test]
    fn lowered_record_limit_is_inclusive_and_precedes_invalid_identity() {
        let (sources, span) = source();
        let limits = Limits {
            records: 1,
            ..Limits::DEFAULT
        };
        let mut raw = vec![record(0, &[], span)];
        assert!(Declarations::check_with_limits(&raw, &sources, limits).is_ok());
        raw.push(record(usize::MAX, &[], span));
        assert_eq!(
            Declarations::check_with_limits(&raw, &sources, limits).unwrap_err(),
            DeclarationError::ResourceLimit("record declarations")
        );
    }

    #[test]
    fn field_count_preflight_precedes_all_field_contents() {
        let (sources, span) = source();
        let limits = Limits {
            fields_per_record: 1,
            ..Limits::DEFAULT
        };
        let mut raw = vec![record(0, &[hir::Ty::Bool], span)];
        assert!(Declarations::check_with_limits(&raw, &sources, limits).is_ok());
        raw[0].fields[0].id.index = usize::MAX;
        raw.push(record(1, &[hir::Ty::Bool, hir::Ty::Bool], span));
        assert_eq!(
            Declarations::check_with_limits(&raw, &sources, limits).unwrap_err(),
            DeclarationError::ResourceLimit("fields per record")
        );
    }

    #[test]
    fn total_field_limit_is_inclusive_across_records() {
        let (sources, span) = source();
        let limits = Limits {
            fields: 2,
            ..Limits::DEFAULT
        };
        let mut raw = vec![
            record(0, &[hir::Ty::Bool], span),
            record(1, &[hir::Ty::I32], span),
        ];
        assert!(Declarations::check_with_limits(&raw, &sources, limits).is_ok());
        raw.push(record(2, &[hir::Ty::Unit], span));
        assert_eq!(
            Declarations::check_with_limits(&raw, &sources, limits).unwrap_err(),
            DeclarationError::ResourceLimit("field declarations")
        );
    }

    #[test]
    fn table_payload_limit_charges_both_arrays_and_is_inclusive() {
        let (sources, span) = source();
        let mut raw = [record(0, &[hir::Ty::I32], span)];
        let bytes = size_of::<RecordDecl>() + size_of::<FieldDecl>();
        let limits = Limits {
            table_bytes: bytes,
            ..Limits::DEFAULT
        };
        let table = Declarations::check_with_limits(&raw, &sources, limits).unwrap();
        assert_eq!(table.usage().table_bytes, bytes);
        raw[0].id = RecordId(usize::MAX); // Payload rejection precedes validation.
        let limits = Limits {
            table_bytes: bytes - 1,
            ..limits
        };
        assert_eq!(
            Declarations::check_with_limits(&raw, &sources, limits).unwrap_err(),
            DeclarationError::ResourceLimit("declaration table bytes")
        );
    }

    #[test]
    fn padded_layout_total_and_empty_identity_bytes_are_charged() {
        let (sources, span) = source();
        let raw = [
            record(0, &[hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit], span),
            record(1, &[], span),
        ];
        let limits = Limits {
            layout_bytes: 13,
            ..Limits::DEFAULT
        };
        let table = Declarations::check_with_limits(&raw, &sources, limits).unwrap();
        assert_eq!(table.usage().layout_bytes, 13); // 12 padded bytes plus empty 1.
        let limits = Limits {
            layout_bytes: 12,
            ..limits
        };
        assert_eq!(
            Declarations::check_with_limits(&raw, &sources, limits).unwrap_err(),
            DeclarationError::ResourceLimit("declaration layout bytes")
        );
    }

    #[test]
    fn zero_limits_admit_only_the_empty_table() {
        let (sources, span) = source();
        let limits = Limits {
            records: 0,
            fields: 0,
            fields_per_record: 0,
            table_bytes: 0,
            layout_bytes: 0,
        };
        assert!(Declarations::check_with_limits(&[], &sources, limits).is_ok());
        assert_eq!(
            Declarations::check_with_limits(&[record(0, &[], span)], &sources, limits).unwrap_err(),
            DeclarationError::ResourceLimit("record declarations")
        );
    }

    #[test]
    fn boundary_seam_cannot_raise_production_limits() {
        let limits = Limits {
            records: usize::MAX,
            fields: usize::MAX,
            fields_per_record: usize::MAX,
            table_bytes: usize::MAX,
            layout_bytes: usize::MAX,
        }
        .bounded();
        assert_eq!(limits.records, Limits::DEFAULT.records);
        assert_eq!(limits.fields, Limits::DEFAULT.fields);
        assert_eq!(limits.fields_per_record, Limits::DEFAULT.fields_per_record);
        assert_eq!(limits.table_bytes, Limits::DEFAULT.table_bytes);
        assert_eq!(limits.layout_bytes, Limits::DEFAULT.layout_bytes);
    }

    #[test]
    fn production_record_and_field_boundaries_are_inclusive() {
        let (sources, span) = source();
        let types = vec![hir::Ty::I32; Limits::DEFAULT.fields_per_record];
        let full_records = Limits::DEFAULT.fields / types.len();
        let mut raw: Vec<_> = (0..Limits::DEFAULT.records)
            .map(|id| record(id, if id < full_records { &types } else { &[] }, span))
            .collect();
        let table = Declarations::check(&raw, &sources).unwrap();
        assert_eq!(table.usage().records, Limits::DEFAULT.records);
        assert_eq!(table.usage().fields, Limits::DEFAULT.fields);
        eprintln!(
            "declaration fixture: RecordDecl={} FieldDecl={} RawRecordDecl={} RawFieldDecl={} usage={:?}, capacities=({}, {})",
            size_of::<RecordDecl>(),
            size_of::<FieldDecl>(),
            size_of::<RawRecordDecl>(),
            size_of::<RawFieldDecl>(),
            table.usage(),
            table.records.capacity(),
            table.fields.capacity(),
        );
        assert_eq!(
            table.usage().layout_bytes,
            4 * Limits::DEFAULT.fields + Limits::DEFAULT.records - full_records
        );
        assert_eq!(
            table.usage().table_bytes,
            Limits::DEFAULT.records * size_of::<RecordDecl>()
                + Limits::DEFAULT.fields * size_of::<FieldDecl>()
        );
        raw.push(record(Limits::DEFAULT.records, &[], span));
        assert_eq!(
            error(&raw, &sources),
            DeclarationError::ResourceLimit("record declarations")
        );
        raw.pop();
        raw[full_records] = record(full_records, &[hir::Ty::Unit], span);
        assert_eq!(
            error(&raw, &sources),
            DeclarationError::ResourceLimit("field declarations")
        );
        let extra = raw[1].fields[0];
        raw[0].fields.push(extra);
        assert_eq!(
            error(&raw, &sources),
            DeclarationError::ResourceLimit("fields per record")
        );
    }

    #[test]
    fn count_and_table_arithmetic_reject_usize_overflow() {
        assert_eq!(
            limited_add(usize::MAX, 1, usize::MAX, "count"),
            Err(DeclarationError::ResourceLimit("count"))
        );
        assert_eq!(
            limited_add(usize::MAX - 1, 1, usize::MAX, "count"),
            Ok(usize::MAX)
        );
        for (records, fields) in [
            (usize::MAX, 0),
            (0, usize::MAX),
            (
                usize::MAX / size_of::<RecordDecl>(),
                usize::MAX / size_of::<FieldDecl>(),
            ),
        ] {
            assert_eq!(
                table_bytes(records, fields),
                Err(DeclarationError::ResourceLimit("declaration table bytes"))
            );
        }
    }

    #[test]
    fn layout_padding_size_and_final_rounding_are_checked() {
        assert_eq!(align_up(usize::MAX, 1), Ok(usize::MAX));
        assert_eq!(align_up(usize::MAX - 3, 4), Ok(usize::MAX - 3));
        assert_eq!(
            align_up(usize::MAX, 4),
            Err(DeclarationError::LayoutOverflow)
        );
        for align in [0, 3, usize::MAX] {
            assert_eq!(align_up(0, align), Err(DeclarationError::LayoutOverflow));
        }
        let mut cursor = LayoutCursor {
            end: usize::MAX,
            align: 1,
        };
        assert_eq!(
            cursor.push(Layout::scalar(hir::Ty::Bool)),
            Err(DeclarationError::LayoutOverflow)
        );
        assert_eq!(cursor.end, usize::MAX);
        assert_eq!(cursor.align, 1);
        let cursor = LayoutCursor {
            end: usize::MAX - 2,
            align: 4,
        };
        assert_eq!(cursor.finish(), Err(DeclarationError::LayoutOverflow));
    }

    #[test]
    fn existing_scalar_type_display_equality_and_values_are_unchanged() {
        use crate::frontend::oir::Scalar;
        let types = [hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit];
        for (index, expected) in ["bool", "i32", "()"].iter().enumerate() {
            assert_eq!(types[index].to_string(), *expected);
            for (other_index, other) in types.iter().enumerate() {
                assert_eq!(types[index] == *other, index == other_index);
            }
        }
        for (value, ty, display, json) in [
            (
                Scalar::Bool(true),
                hir::Ty::Bool,
                "true",
                "{\"type\":\"bool\",\"value\":true}",
            ),
            (
                Scalar::Bool(false),
                hir::Ty::Bool,
                "false",
                "{\"type\":\"bool\",\"value\":false}",
            ),
            (
                Scalar::I32(i32::MIN),
                hir::Ty::I32,
                "-2147483648",
                "{\"type\":\"i32\",\"value\":-2147483648}",
            ),
            (
                Scalar::I32(i32::MAX),
                hir::Ty::I32,
                "2147483647",
                "{\"type\":\"i32\",\"value\":2147483647}",
            ),
            (Scalar::Unit, hir::Ty::Unit, "()", "{\"type\":\"unit\"}"),
        ] {
            assert_eq!(value.ty(), ty);
            assert_eq!(value.to_string(), display);
            assert_eq!(value.json(), json);
            assert_eq!(value, value);
        }
        assert_ne!(Scalar::Bool(false), Scalar::Unit);
        assert_ne!(Scalar::I32(0), Scalar::Bool(false));
    }

    #[test]
    fn original_scalar_budget_formulas_and_inclusive_boundaries_are_unchanged() {
        use crate::frontend::oir::{Budget, FailureKind, MAX_ASSIGNMENTS, MAX_BLOCKS, MAX_LOCALS};
        assert_eq!(MAX_LOCALS, parser::MAX_NODES);
        assert_eq!(MAX_ASSIGNMENTS, parser::MAX_NODES);
        assert_eq!(MAX_BLOCKS, 3 * parser::MAX_NODES);
        for limit in [MAX_LOCALS, MAX_ASSIGNMENTS, MAX_BLOCKS] {
            let mut count = 0;
            Budget::add(&mut count, limit, limit, "scalar", "test", None).unwrap();
            assert_eq!(count, limit);
            assert_eq!(
                Budget::add(&mut count, 1, limit, "scalar", "test", None)
                    .unwrap_err()
                    .kind,
                FailureKind::ResourceLimit("scalar")
            );
        }
    }

    #[test]
    fn production_source_still_rejects_records_and_call_borrows() {
        for text in [
            "struct Counter { value: i32 } fn main() -> i32 { return 0; }",
            "fn read(x: &Counter) -> i32 { return 0; }",
            "fn step(x: &mut Counter) -> () { return; }",
            "fn main() -> () { let x = Counter { value: 0 }; return; }",
            "fn main() -> () { let x = 0; read(&x); return; }",
        ] {
            let mut sources = SourceMap::new();
            let file = sources.add("disabled.ox".into(), text.into());
            let file = sources.get(file);
            let tokens = lexer::lex(file).unwrap();
            assert!(
                parser::parse(file, tokens).is_err(),
                "unexpected source acceptance: {text}"
            );
        }
    }
}
