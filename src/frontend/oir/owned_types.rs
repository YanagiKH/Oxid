//! Checked private declaration, aggregate identity and layout queries.
//!
//! Production record source, verification and consumers use this facade.
//! Enum identity/layout is admitted only in declarations; enum source and
//! executable witnesses remain gated until their consumer phase.
//! References describe call parameters, never stored language values.
#![allow(dead_code)]

use super::{hir, SourceMap, Span};
use crate::frontend::project::budget::{Allocator, ReserveFailure};
use std::mem::size_of;

mod enums;
#[allow(unused_imports)]
pub(in crate::frontend) use enums::{admit_enum_counts, EnumId, EnumUsage, VariantId};
#[allow(unused_imports)]
pub(super) use enums::{
    EnumDeclarationError, EnumDeclarations, PreparedEnums, RawEnumDecl, RawVariantDecl,
};

/// Compilation-local nominal identity, assigned in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) struct RecordId(pub(in crate::frontend) usize);

/// Structural fixed-array type. The constructor checks before narrowing;
/// neither a caller-supplied stride/layout nor a nominal ID can create one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) struct FixedArrayTy {
    element: hir::Ty,
    length: u16,
}
impl FixedArrayTy {
    pub(in crate::frontend) fn check(
        element: hir::Ty,
        length: usize,
    ) -> Result<Self, DeclarationError> {
        if !matches!(
            element,
            hir::Ty::Bool | hir::Ty::I32 | hir::Ty::U8 | hir::Ty::Unit
        ) {
            return Err(DeclarationError::TypeMismatch);
        }
        if length > 1024 {
            return Err(DeclarationError::ResourceLimit("fixed array length"));
        }
        Ok(Self {
            element,
            length: u16::try_from(length)
                .map_err(|_| DeclarationError::ResourceLimit("fixed array length"))?,
        })
    }
    pub(in crate::frontend) fn element(self) -> hir::Ty {
        self.element
    }
    pub(in crate::frontend) fn length(self) -> usize {
        usize::from(self.length)
    }
    pub(super) fn stride(self) -> usize {
        Layout::scalar(self.element).size
    }
    fn layout(self) -> Result<Layout, DeclarationError> {
        Self::check(self.element, self.length())?;
        let scalar = Layout::scalar(self.element);
        let bytes = self
            .length()
            .checked_mul(self.stride())
            .ok_or(DeclarationError::LayoutOverflow)?;
        Ok(Layout {
            // Positive private storage, aligned even when there are no elements.
            size: align_up(bytes.max(1), scalar.align)?,
            align: scalar.align,
        })
    }
}

/// Explicit aggregate identity; nominal IDs are never packed or reinterpreted.
/// Checked enum identity does not grant executable enum admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum AggregateTy {
    Record(RecordId),
    Enum(EnumId),
    FixedArray(FixedArrayTy),
}

/// A call-only borrowed view. Unsized slices are never owned value types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum BorrowedTy {
    Exact(AggregateTy),
    ScalarSlice(hir::Ty),
}
impl BorrowedTy {
    pub(in crate::frontend) fn accepts(self, authority: Self) -> bool {
        match (authority, self) {
            (Self::Exact(AggregateTy::Enum(_)), _) | (_, Self::Exact(AggregateTy::Enum(_))) => {
                false
            }
            (Self::Exact(actual), Self::Exact(expected)) => actual == expected,
            (Self::Exact(AggregateTy::FixedArray(array)), Self::ScalarSlice(element)) => {
                array.element() == element
            }
            (Self::ScalarSlice(actual), Self::ScalarSlice(expected)) => actual == expected,
            _ => false,
        }
    }
    pub(in crate::frontend) fn element(self) -> Option<hir::Ty> {
        match self {
            Self::Exact(AggregateTy::FixedArray(array)) => Some(array.element()),
            Self::ScalarSlice(element) => Some(element),
            Self::Exact(AggregateTy::Record(_) | AggregateTy::Enum(_)) => None,
        }
    }
}
/// Compact retained borrowed identity; it cannot be used as an owner descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) struct BorrowedSlot(BorrowedSlotRepr);
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BorrowedSlotRepr {
    Record(u32),
    FixedArray(FixedArrayTy),
    ScalarSlice(hir::Ty),
}
impl BorrowedSlot {
    pub(in crate::frontend) fn check(ty: BorrowedTy) -> Result<Self, DeclarationError> {
        Ok(Self(match ty {
            BorrowedTy::Exact(AggregateTy::Enum(_)) => return Err(DeclarationError::TypeMismatch),
            BorrowedTy::Exact(AggregateTy::Record(id)) => BorrowedSlotRepr::Record(
                u32::try_from(id.0).map_err(|_| DeclarationError::InvalidRecordId(id))?,
            ),
            BorrowedTy::Exact(AggregateTy::FixedArray(array)) => {
                FixedArrayTy::check(array.element(), array.length())?;
                BorrowedSlotRepr::FixedArray(array)
            }
            BorrowedTy::ScalarSlice(
                element @ (hir::Ty::Bool | hir::Ty::I32 | hir::Ty::U8 | hir::Ty::Unit),
            ) => BorrowedSlotRepr::ScalarSlice(element),
        }))
    }
    pub(in crate::frontend) fn referent(self) -> BorrowedTy {
        match self.0 {
            BorrowedSlotRepr::Record(id) => BorrowedTy::Exact(AggregateTy::Record(RecordId(
                usize::try_from(id).expect("qualified ordinal width"),
            ))),
            BorrowedSlotRepr::FixedArray(array) => {
                BorrowedTy::Exact(AggregateTy::FixedArray(array))
            }
            BorrowedSlotRepr::ScalarSlice(element) => BorrowedTy::ScalarSlice(element),
        }
    }
}

/// Compact retained identity, never an ownership or declaration witness.
/// The independent tag cannot reinterpret malformed nominal IDs as arrays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) struct AggregateSlot(AggregateSlotRepr);
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AggregateSlotRepr {
    Record(u32),
    Enum(u32),
    FixedArray(FixedArrayTy),
}
impl AggregateSlot {
    pub(in crate::frontend) fn try_from_aggregate(
        ty: AggregateTy,
    ) -> Result<Self, DeclarationError> {
        Ok(Self(match ty {
            AggregateTy::Record(id) => AggregateSlotRepr::Record(
                u32::try_from(id.0).map_err(|_| DeclarationError::InvalidRecordId(id))?,
            ),
            AggregateTy::Enum(id) => AggregateSlotRepr::Enum(
                u32::try_from(id.0).map_err(|_| DeclarationError::InvalidEnumId(id))?,
            ),
            AggregateTy::FixedArray(array) => {
                FixedArrayTy::check(array.element(), array.length())?;
                AggregateSlotRepr::FixedArray(array)
            }
        }))
    }
    pub(in crate::frontend) fn aggregate(self) -> AggregateTy {
        match self.0 {
            // All qualified hosts are 64-bit; no truncation on supported targets.
            AggregateSlotRepr::Record(id) => AggregateTy::Record(RecordId(
                usize::try_from(id).expect("qualified ordinal width"),
            )),
            AggregateSlotRepr::Enum(id) => AggregateTy::Enum(EnumId(
                usize::try_from(id).expect("qualified ordinal width"),
            )),
            AggregateSlotRepr::FixedArray(array) => AggregateTy::FixedArray(array),
        }
    }
}

/// An ordinal alone is not a field identity: its declaring record is essential.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) struct FieldId {
    pub(in crate::frontend) record: RecordId,
    pub(in crate::frontend) index: usize,
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
pub(in crate::frontend) enum ValueTy {
    Scalar(hir::Ty),
    Owned(AggregateTy),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum BorrowKind {
    Shared,
    Exclusive,
}

/// References exist only in parameter descriptors, not in `ValueTy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum ParameterTy {
    Value(ValueTy),
    Reference {
        referent: BorrowedTy,
        kind: BorrowKind,
    },
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
    // producer, is responsible for excluding enums and references.
    pub(super) ty: ParameterTy,
    pub(super) span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum DeclarationError {
    ResourceLimit(&'static str),
    InvalidRecordId(RecordId),
    InvalidEnumId(EnumId),
    InvalidVariantId(VariantId),
    NonScalarPayload(VariantId),
    InvalidFieldId(FieldId),
    InvalidSpan(Span),
    NonScalarField(FieldId),
    ContainmentCycle(RecordId),
    RecordMismatch,
    TypeMismatch,
    LayoutOverflow,
    Allocation,
}

impl From<EnumDeclarationError> for DeclarationError {
    fn from(error: EnumDeclarationError) -> Self {
        match error {
            EnumDeclarationError::Declaration(error) => error,
            EnumDeclarationError::InvalidEnumId(id) => Self::InvalidEnumId(id),
            EnumDeclarationError::InvalidVariantId(id) => Self::InvalidVariantId(id),
            EnumDeclarationError::NonScalarPayload(id) => Self::NonScalarPayload(id),
            EnumDeclarationError::EnumMismatch
            | EnumDeclarationError::PayloadArity(_)
            | EnumDeclarationError::InvalidTag { .. } => Self::TypeMismatch,
        }
    }
}
impl From<ReserveFailure> for DeclarationError {
    fn from(error: ReserveFailure) -> Self {
        match error {
            ReserveFailure::Allocation => Self::Allocation,
            ReserveFailure::Overflow => Self::ResourceLimit("declaration table bytes"),
        }
    }
}

/// Private Linux x86_64 storage layout, not a source or FFI ABI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
            hir::Ty::Bool | hir::Ty::U8 | hir::Ty::Unit => Self { size: 1, align: 1 },
            hir::Ty::I32 => Self { size: 4, align: 4 },
        }
    }
}

#[derive(Debug)]
pub(super) struct FieldDecl {
    id: FieldId,
    ty: ValueTy,
    span: Span,
    offset: usize,
}
impl FieldDecl {
    pub(super) fn id(&self) -> FieldId {
        self.id
    }
    pub(super) fn value_ty(&self) -> ValueTy {
        self.ty
    }
    // Scalar-only compatibility query. Authoritative instruction shape checks
    // prove this precondition; composed accesses use value_ty/projection/leaves.
    pub(super) fn ty(&self) -> hir::Ty {
        match self.ty {
            ValueTy::Scalar(ty) => ty,
            ValueTy::Owned(_) => unreachable!("verified scalar field query"),
        }
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
    width: usize,
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
/// overhead and the original record vector headers. Checked combined usage
/// additionally charges the retained enum facade, including empty headers.
/// Record/member counts remain record-only; combined table/layout bytes include
/// enums. `layout_bytes` charges one instance of each declaration, not runtime
/// slots or activation storage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::frontend) struct DeclarationUsage {
    pub(super) records: usize,
    pub(super) fields: usize,
    pub(super) table_bytes: usize,
    pub(super) layout_bytes: usize,
}

/// Engineering ceilings remain unchanged for composed declarations. Compact
/// containment DAGs can describe exponential storage, so checked bottom-up
/// layout/width admission precedes output allocation or any leaf expansion.
/// Graph scratch is O(records), with a separately bounded depth-64 work stack.
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

/// Only `check` constructs this immutable declaration facade. Queries do not
/// grant a verified executable witness or expose mutable/raw table storage.
#[derive(Debug)]
pub(super) struct Declarations {
    records: Vec<RecordDecl>,
    fields: Vec<FieldDecl>,
    enums: EnumDeclarations,
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
        Self::check_combined_with_limits(raw, &[], sources, limits, &mut Allocator::default())
    }

    pub(super) fn check_combined(
        records: &[RawRecordDecl],
        enums: &[RawEnumDecl],
        sources: &SourceMap,
    ) -> Result<Self, DeclarationError> {
        Self::check_combined_with_limits(
            records,
            enums,
            sources,
            Limits::DEFAULT,
            &mut Allocator::default(),
        )
    }

    fn check_combined_with_limits(
        raw: &[RawRecordDecl],
        raw_enums: &[RawEnumDecl],
        sources: &SourceMap,
        limits: Limits,
        allocator: &mut Allocator,
    ) -> Result<Self, DeclarationError> {
        let limits = limits.bounded();
        // Independently inventory both raw slices. The added retained enum
        // facade is charged even when both of its vectors are empty.
        let mut usage = preflight(raw, limits)?;
        let prepared_enums = EnumDeclarations::prepare(raw_enums, sources, usage)?;
        let enum_usage = prepared_enums.usage();
        limited_add(
            usage.records,
            enum_usage.enums,
            limits.records,
            "aggregate declarations",
        )?;
        limited_add(
            usage.fields,
            enum_usage.variants,
            limits.fields,
            "aggregate members",
        )?;
        usage.table_bytes = limited_add(
            usage.table_bytes,
            enum_usage.table_bytes,
            limits.table_bytes,
            "declaration table bytes",
        )?;
        usage.layout_bytes = limited_add(
            0,
            enum_usage.layout_bytes,
            limits.layout_bytes,
            "declaration layout bytes",
        )?;
        for (index, record) in raw.iter().enumerate() {
            if record.id != RecordId(index) {
                return Err(DeclarationError::InvalidRecordId(record.id));
            }
            check_nominal_span(sources, record.span)?;
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
                if let ValueTy::Owned(AggregateTy::Record(id)) = value_field(field)? {
                    if id.0 >= raw.len() {
                        return Err(DeclarationError::InvalidRecordId(id));
                    }
                }
            }
        }
        let summaries = containment_summaries(raw, limits)?;
        for summary in &summaries {
            usage.layout_bytes = limited_add(
                usage.layout_bytes,
                summary.layout.size,
                limits.layout_bytes,
                "declaration layout bytes",
            )?;
        }

        // All malformed declarations and all size/overflow gates above reject
        // before these output allocations. Flat fields avoid R separate
        // allocations, with O(R + F) persistent storage and O(R) graph scratch.
        let mut records = Vec::new();
        if usage.records != 0 {
            allocator.vector_exact(&mut records, usage.records, "checked record declarations")?;
        }
        let mut fields = Vec::new();
        if usage.fields != 0 {
            allocator.vector_exact(&mut fields, usage.fields, "checked record fields")?;
        }
        let enums = prepared_enums.finish_with_allocator(allocator)?;
        for record in raw {
            let field_start = fields.len();
            let mut cursor = LayoutCursor::default();
            for field in &record.fields {
                let ty = value_field(field)?;
                let offset = cursor.push(value_summary(ty, &summaries)?.layout)?;
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
                width: summaries[record.id.0].width,
            });
        }
        Ok(Self {
            records,
            fields,
            enums,
            usage,
        })
    }

    /// A field path is a bounded access description, never a new owner. Offsets
    /// are derived only from checked declarations, not accepted from producers.
    pub(super) fn projection(
        &self,
        root: AggregateTy,
        path: &[FieldId],
    ) -> Result<(ValueTy, usize), DeclarationError> {
        if path.is_empty() || path.len() > MAX_CONTAINMENT_DEPTH {
            return Err(DeclarationError::ResourceLimit("record projection depth"));
        }
        self.check_aggregate_type(root)?;
        let (mut ty, mut offset) = (ValueTy::Owned(root), 0usize);
        for &id in path {
            let ValueTy::Owned(AggregateTy::Record(record)) = ty else {
                return Err(DeclarationError::TypeMismatch);
            };
            let field = self.field(record, id)?;
            offset = offset
                .checked_add(field.offset())
                .ok_or(DeclarationError::LayoutOverflow)?;
            ty = field.value_ty();
        }
        Ok((ty, offset))
    }
    /// No retained flattening: a fixed depth-bounded stack expands an already
    /// admitted aggregate lazily. Empty records/arrays yield a storage sentinel.
    pub(super) fn leaves(
        &self,
        aggregate: AggregateTy,
    ) -> Result<ScalarLeaves<'_>, DeclarationError> {
        self.check_aggregate_type(aggregate)?;
        if matches!(aggregate, AggregateTy::Enum(_)) {
            return Err(DeclarationError::TypeMismatch);
        }
        let mut stack = [None; MAX_CONTAINMENT_DEPTH + 1];
        stack[0] = Some((ValueTy::Owned(aggregate), 0, 0));
        Ok(ScalarLeaves {
            declarations: self,
            stack,
            length: 1,
        })
    }

    pub(super) fn usage(&self) -> DeclarationUsage {
        self.usage
    }
    pub(super) fn enums(&self) -> &EnumDeclarations {
        &self.enums
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
    pub(super) fn check_aggregate_type(&self, ty: AggregateTy) -> Result<(), DeclarationError> {
        match ty {
            AggregateTy::Record(id) => self.record(id).map(|_| ()),
            AggregateTy::Enum(id) => self.enums.enumeration(id).map(|_| ()).map_err(Into::into),
            AggregateTy::FixedArray(array) => array.layout().map(|_| ()),
        }
    }
    pub(super) fn aggregate_layout(&self, ty: AggregateTy) -> Result<Layout, DeclarationError> {
        match ty {
            AggregateTy::Record(id) => self.record(id).map(RecordDecl::layout),
            AggregateTy::Enum(id) => self
                .enums
                .enumeration(id)
                .map(|enumeration| enumeration.layout())
                .map_err(Into::into),
            AggregateTy::FixedArray(array) => array.layout(),
        }
    }
    pub(super) fn aggregate_width(&self, ty: AggregateTy) -> Result<usize, DeclarationError> {
        match ty {
            AggregateTy::Record(id) => self.record(id).map(|record| record.width),
            AggregateTy::Enum(id) => self
                .enums
                .enumeration(id)
                .map(|enumeration| enumeration.width())
                .map_err(Into::into),
            AggregateTy::FixedArray(array) => {
                FixedArrayTy::check(array.element(), array.length())?;
                Ok(array.length().max(1))
            }
        }
    }
    pub(super) fn same_aggregate_type(
        &self,
        actual: AggregateTy,
        expected: AggregateTy,
    ) -> Result<(), DeclarationError> {
        // Malformed nominal IDs reject even when both inputs compare equal.
        self.check_aggregate_type(actual)?;
        self.check_aggregate_type(expected)?;
        if actual == expected {
            Ok(())
        } else {
            Err(DeclarationError::TypeMismatch)
        }
    }
    pub(super) fn check_borrowed_type(&self, ty: BorrowedTy) -> Result<(), DeclarationError> {
        match ty {
            BorrowedTy::Exact(AggregateTy::Enum(_)) => Err(DeclarationError::TypeMismatch),
            BorrowedTy::Exact(aggregate) => self.check_aggregate_type(aggregate),
            BorrowedTy::ScalarSlice(hir::Ty::Bool | hir::Ty::I32 | hir::Ty::U8 | hir::Ty::Unit) => {
                Ok(())
            }
        }
    }
    pub(super) fn same_borrowed_type(
        &self,
        actual: BorrowedTy,
        expected: BorrowedTy,
    ) -> Result<(), DeclarationError> {
        self.check_borrowed_type(actual)?;
        self.check_borrowed_type(expected)?;
        if actual == expected {
            Ok(())
        } else {
            Err(DeclarationError::TypeMismatch)
        }
    }
    pub(super) fn check_borrowed_view(
        &self,
        authority: BorrowedTy,
        target: BorrowedTy,
    ) -> Result<(), DeclarationError> {
        self.check_borrowed_type(authority)?;
        self.check_borrowed_type(target)?;
        if target.accepts(authority) {
            Ok(())
        } else {
            Err(DeclarationError::TypeMismatch)
        }
    }
    pub(super) fn check_value_type(&self, ty: ValueTy) -> Result<(), DeclarationError> {
        match ty {
            ValueTy::Scalar(_) => Ok(()),
            ValueTy::Owned(aggregate) => self.check_aggregate_type(aggregate),
        }
    }
    pub(super) fn check_parameter_type(&self, ty: ParameterTy) -> Result<(), DeclarationError> {
        match ty {
            ParameterTy::Value(value) => self.check_value_type(value),
            ParameterTy::Reference { referent, .. } => self.check_borrowed_type(referent),
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

/// Includes initialized empty-value sentinel storage; excludes all padding.
#[derive(Clone, Copy, Debug)]
pub(super) struct ScalarLeaf {
    pub(super) offset: usize,
    pub(super) ty: hir::Ty,
}
pub(super) struct ScalarLeaves<'a> {
    declarations: &'a Declarations,
    stack: [Option<(ValueTy, usize, usize)>; MAX_CONTAINMENT_DEPTH + 1],
    length: usize,
}
impl Iterator for ScalarLeaves<'_> {
    type Item = ScalarLeaf;
    fn next(&mut self) -> Option<Self::Item> {
        while self.length != 0 {
            let position = self.length - 1;
            let (ty, offset, next) = self.stack[position].expect("checked traversal stack");
            match ty {
                ValueTy::Scalar(ty) => {
                    self.length -= 1;
                    return Some(ScalarLeaf { offset, ty });
                }
                ValueTy::Owned(AggregateTy::Enum(_)) => {
                    unreachable!("enum excluded from static leaves")
                }
                ValueTy::Owned(AggregateTy::FixedArray(array)) => {
                    if next < array.length().max(1) {
                        self.stack[position] = Some((ty, offset, next + 1));
                        return Some(ScalarLeaf {
                            offset: offset + next * array.stride(),
                            ty: array.element(),
                        });
                    }
                    self.length -= 1;
                }
                ValueTy::Owned(AggregateTy::Record(record)) => {
                    let fields = self
                        .declarations
                        .fields(record)
                        .expect("checked traversal record");
                    if fields.is_empty() && next == 0 {
                        self.length -= 1;
                        return Some(ScalarLeaf {
                            offset,
                            ty: hir::Ty::Unit,
                        });
                    }
                    if let Some(field) = fields.get(next) {
                        self.stack[position] = Some((ty, offset, next + 1));
                        self.stack[self.length] =
                            Some((field.value_ty(), offset + field.offset(), 0));
                        self.length += 1;
                    } else {
                        self.length -= 1;
                    }
                }
            }
        }
        None
    }
}

fn check_span(sources: &SourceMap, span: Span) -> Result<(), DeclarationError> {
    if sources.is_valid_span(span) {
        Ok(())
    } else {
        Err(DeclarationError::InvalidSpan(span))
    }
}

// A raw nominal declaration cannot manufacture the reserved primitive binding.
fn check_nominal_span(sources: &SourceMap, span: Span) -> Result<(), DeclarationError> {
    check_span(sources, span)?;
    if sources.try_text(span) == Some("u8") {
        return Err(DeclarationError::TypeMismatch);
    }
    Ok(())
}

fn value_field(field: &RawFieldDecl) -> Result<ValueTy, DeclarationError> {
    match field.ty {
        ParameterTy::Value(ValueTy::Scalar(hir::Ty::U8)) => {
            Err(DeclarationError::NonScalarField(field.id))
        }
        ParameterTy::Value(ValueTy::Owned(AggregateTy::FixedArray(array)))
            if array.element() == hir::Ty::U8 =>
        {
            Err(DeclarationError::NonScalarField(field.id))
        }
        ParameterTy::Value(ValueTy::Owned(AggregateTy::Enum(_))) => {
            Err(DeclarationError::NonScalarField(field.id))
        }
        ParameterTy::Value(ty) => Ok(ty),
        _ => Err(DeclarationError::NonScalarField(field.id)),
    }
}

/// Maximum number of records along any by-value containment path.
pub(super) const MAX_CONTAINMENT_DEPTH: usize = 64;

#[derive(Clone, Copy, Default)]
struct ContainmentSummary {
    layout: Layout,
    width: usize,
    depth: usize,
}

fn value_summary(
    ty: ValueTy,
    summaries: &[ContainmentSummary],
) -> Result<ContainmentSummary, DeclarationError> {
    Ok(match ty {
        ValueTy::Scalar(hir::Ty::U8) => return Err(DeclarationError::TypeMismatch),
        ValueTy::Scalar(ty) => ContainmentSummary {
            layout: Layout::scalar(ty),
            width: 1,
            depth: 0,
        },
        ValueTy::Owned(AggregateTy::FixedArray(array)) if array.element() == hir::Ty::U8 => {
            return Err(DeclarationError::TypeMismatch)
        }
        ValueTy::Owned(AggregateTy::FixedArray(array)) => ContainmentSummary {
            layout: array.layout()?,
            width: array.length().max(1),
            depth: 0,
        },
        ValueTy::Owned(AggregateTy::Enum(_)) => return Err(DeclarationError::TypeMismatch),
        ValueTy::Owned(AggregateTy::Record(id)) => *summaries
            .get(id.0)
            .ok_or(DeclarationError::InvalidRecordId(id))?,
    })
}

fn containment_summaries(
    raw: &[RawRecordDecl],
    limits: Limits,
) -> Result<Vec<ContainmentSummary>, DeclarationError> {
    containment_graph(
        raw.len(),
        |id| raw[id].fields.len(),
        |id, field| value_field(&raw[id].fields[field]),
        limits,
    )
}
fn containment_graph(
    record_count: usize,
    field_count: impl Fn(usize) -> usize,
    field_type: impl Fn(usize, usize) -> Result<ValueTy, DeclarationError>,
    limits: Limits,
) -> Result<Vec<ContainmentSummary>, DeclarationError> {
    // Count preflight has already bounded record_count. Allocation is fallible and
    // no traversal or transitive flattening can allocate additional graph nodes.
    let mut summaries = Vec::new();
    summaries
        .try_reserve_exact(record_count)
        .map_err(|_| DeclarationError::Allocation)?;
    summaries.resize(record_count, ContainmentSummary::default());
    let mut state = Vec::new();
    state
        .try_reserve_exact(record_count)
        .map_err(|_| DeclarationError::Allocation)?;
    state.resize(record_count, 0u8);
    let mut stack = Vec::new();
    stack
        .try_reserve_exact(record_count.min(MAX_CONTAINMENT_DEPTH))
        .map_err(|_| DeclarationError::Allocation)?;
    for root in 0..record_count {
        if state[root] == 2 {
            continue;
        }
        state[root] = 1;
        stack.push((root, 0usize));
        while let Some(&(id, next)) = stack.last() {
            if next < field_count(id) {
                stack.last_mut().unwrap().1 += 1;
                if let ValueTy::Owned(AggregateTy::Record(child)) = field_type(id, next)? {
                    match state[child.0] {
                        1 => return Err(DeclarationError::ContainmentCycle(child)),
                        0 => {
                            if stack.len() == MAX_CONTAINMENT_DEPTH {
                                return Err(DeclarationError::ResourceLimit(
                                    "record containment depth",
                                ));
                            }
                            state[child.0] = 1;
                            stack.push((child.0, 0));
                        }
                        _ => {}
                    }
                }
                continue;
            }
            let mut cursor = LayoutCursor::default();
            let (mut width, mut depth) = (0usize, 1usize);
            for field in 0..field_count(id) {
                let child = value_summary(field_type(id, field)?, &summaries)?;
                cursor.push(child.layout)?;
                width = limited_add(
                    width,
                    child.width,
                    limits.layout_bytes,
                    "record expanded width",
                )?;
                depth = depth.max(child.depth + 1);
                if depth > MAX_CONTAINMENT_DEPTH {
                    return Err(DeclarationError::ResourceLimit("record containment depth"));
                }
            }
            let layout = cursor.finish()?;
            if layout.size > limits.layout_bytes {
                return Err(DeclarationError::ResourceLimit("declaration layout bytes"));
            }
            summaries[id] = ContainmentSummary {
                layout,
                width: width.max(1),
                depth,
            };
            state[id] = 2;
            stack.pop();
        }
    }
    Ok(summaries)
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
        .and_then(|bytes| bytes.checked_add(size_of::<EnumDeclarations>()))
        .ok_or(DeclarationError::ResourceLimit("declaration table bytes"))
}

/// Allocation-free admission for source declaration lengths. This is only an
/// inventory: it validates no identities, spans, field types, or layouts and
/// cannot construct the checked declaration facade.
pub(in crate::frontend) fn admit_declaration_counts(
    field_counts: impl ExactSizeIterator<Item = usize>,
) -> Result<DeclarationUsage, DeclarationError> {
    preflight_counts(field_counts, Limits::DEFAULT)
}

/// Count-only source admission for composed field types. Scratch retains a
/// bounded flat input inventory, never expanded leaves or an execution witness.
/// ExactSizeIterator claims are checked against actual visits before use.
pub(super) fn admit_value_layouts<R, F>(records: R) -> Result<usize, DeclarationError>
where
    R: ExactSizeIterator<Item = F> + Clone,
    F: ExactSizeIterator<Item = ValueTy>,
{
    let limits = Limits::DEFAULT;
    let inventory = preflight_counts(records.clone().map(|fields| fields.len()), limits)?;
    let count = inventory.records;
    let mut starts = Vec::new();
    starts
        .try_reserve_exact(count + 1)
        .map_err(|_| DeclarationError::Allocation)?;
    starts.push(0usize);
    let mut values = Vec::new();
    values
        .try_reserve_exact(inventory.fields)
        .map_err(|_| DeclarationError::Allocation)?;
    let mut visited = 0;
    for fields in records {
        visited = limited_add(visited, 1, count, "record declarations")?;
        let length = limited_add(
            0,
            fields.len(),
            limits.fields_per_record,
            "fields per record",
        )?;
        let total = limited_add(values.len(), length, limits.fields, "field declarations")?;
        limited_add(
            0,
            table_bytes(visited, total)?,
            limits.table_bytes,
            "declaration table bytes",
        )?;
        let begin = values.len();
        for ty in fields {
            if values.len() - begin >= length {
                return Err(DeclarationError::ResourceLimit("field declarations"));
            }
            if matches!(ty, ValueTy::Owned(AggregateTy::Enum(_))) {
                return Err(DeclarationError::TypeMismatch);
            }
            if let ValueTy::Owned(AggregateTy::Record(id)) = ty {
                if id.0 >= count {
                    return Err(DeclarationError::InvalidRecordId(id));
                }
            }
            values.push(ty);
        }
        if values.len() - begin != length {
            return Err(DeclarationError::ResourceLimit("field declarations"));
        }
        starts.push(values.len());
    }
    if visited != count {
        return Err(DeclarationError::ResourceLimit("record declarations"));
    }
    let summaries = containment_graph(
        count,
        |id| starts[id + 1] - starts[id],
        |id, field| Ok(values[starts[id] + field]),
        limits,
    )?;
    let mut total = 0;
    for summary in summaries {
        total = limited_add(
            total,
            summary.layout.size,
            limits.layout_bytes,
            "declaration layout bytes",
        )?;
    }
    Ok(total)
}

/// Count scalar layouts without building declarations. The private cursor is
/// shared with full declaration validation, and this result grants no authority.
pub(super) fn admit_scalar_layouts<R, F>(records: R) -> Result<usize, DeclarationError>
where
    R: ExactSizeIterator<Item = F>,
    F: ExactSizeIterator<Item = hir::Ty>,
{
    let limits = Limits::DEFAULT;
    let expected_records = limited_add(0, records.len(), limits.records, "record declarations")?;
    let (mut seen_records, mut fields, mut layouts) = (0, 0, 0);
    for types in records {
        seen_records = limited_add(seen_records, 1, limits.records, "record declarations")?;
        let expected_fields = limited_add(
            0,
            types.len(),
            limits.fields_per_record,
            "fields per record",
        )?;
        let mut cursor = LayoutCursor::default();
        let mut seen_fields = 0;
        for ty in types {
            seen_fields = limited_add(
                seen_fields,
                1,
                limits.fields_per_record,
                "fields per record",
            )?;
            fields = limited_add(fields, 1, limits.fields, "field declarations")?;
            if !matches!(ty, hir::Ty::Bool | hir::Ty::I32 | hir::Ty::Unit) {
                return Err(DeclarationError::TypeMismatch);
            }
            cursor.push(Layout::scalar(ty))?;
        }
        if seen_fields != expected_fields {
            return Err(DeclarationError::ResourceLimit("field declarations"));
        }
        layouts = limited_add(
            layouts,
            cursor.finish()?.size,
            limits.layout_bytes,
            "declaration layout bytes",
        )?;
    }
    if seen_records != expected_records {
        return Err(DeclarationError::ResourceLimit("record declarations"));
    }
    Ok(layouts)
}

fn preflight(raw: &[RawRecordDecl], limits: Limits) -> Result<DeclarationUsage, DeclarationError> {
    preflight_counts(raw.iter().map(|record| record.fields.len()), limits)
}

fn preflight_counts(
    field_counts: impl ExactSizeIterator<Item = usize>,
    limits: Limits,
) -> Result<DeclarationUsage, DeclarationError> {
    let expected_records =
        limited_add(0, field_counts.len(), limits.records, "record declarations")?;
    let mut records = 0;
    let mut fields = 0;
    for count in field_counts {
        records = limited_add(records, 1, limits.records, "record declarations")?;
        if count > limits.fields_per_record {
            return Err(DeclarationError::ResourceLimit("fields per record"));
        }
        fields = limited_add(fields, count, limits.fields, "field declarations")?;
    }
    // ExactSizeIterator is not an authority for understated table payload.
    // Bound actual visits as well, and deny inconsistent producer inventory.
    if records != expected_records {
        return Err(DeclarationError::ResourceLimit("record declarations"));
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

    pub(super) fn source() -> (SourceMap, Span) {
        let mut sources = SourceMap::new();
        let file = sources.add("owned-types.ox".into(), "record 雪 fields".into());
        let span = sources.get(file).span(0, 6);
        (sources, span)
    }

    pub(super) fn record(id: usize, types: &[hir::Ty], span: Span) -> RawRecordDecl {
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
        assert_eq!(
            table.usage(),
            DeclarationUsage {
                table_bytes: size_of::<EnumDeclarations>(),
                ..DeclarationUsage::default()
            }
        );
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
            table.same_value_type(
                ValueTy::Owned(AggregateTy::Record(RecordId(0))),
                ValueTy::Owned(AggregateTy::Record(RecordId(1)))
            ),
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
    fn borrowed_fields_are_rejected() {
        let (sources, span) = source();
        for ty in [
            ParameterTy::Reference {
                referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(0))),
                kind: BorrowKind::Shared,
            },
            ParameterTy::Reference {
                referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(0))),
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
            ValueTy::Owned(AggregateTy::Record(RecordId(0))),
        ] {
            assert_eq!(table.check_value_type(ty), Ok(()));
            assert_eq!(table.check_parameter_type(ParameterTy::Value(ty)), Ok(()));
            assert_eq!(table.same_value_type(ty, ty), Ok(()));
        }
        for kind in [BorrowKind::Shared, BorrowKind::Exclusive] {
            assert_eq!(
                table.check_parameter_type(ParameterTy::Reference {
                    referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(0))),
                    kind
                }),
                Ok(())
            );
            assert_eq!(
                table.check_parameter_type(ParameterTy::Reference {
                    referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(1))),
                    kind
                }),
                Err(DeclarationError::InvalidRecordId(RecordId(1)))
            );
        }
        let bad = ValueTy::Owned(AggregateTy::Record(RecordId(usize::MAX)));
        assert_eq!(
            table.same_value_type(bad, bad),
            Err(DeclarationError::InvalidRecordId(RecordId(usize::MAX)))
        );
        for (actual, expected) in [
            (
                ValueTy::Scalar(hir::Ty::Bool),
                ValueTy::Scalar(hir::Ty::I32),
            ),
            (
                ValueTy::Scalar(hir::Ty::Unit),
                ValueTy::Owned(AggregateTy::Record(RecordId(0))),
            ),
        ] {
            assert_eq!(
                table.same_value_type(actual, expected),
                Err(DeclarationError::TypeMismatch)
            );
        }
        assert_ne!(
            ParameterTy::Reference {
                referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(0))),
                kind: BorrowKind::Shared
            },
            ParameterTy::Reference {
                referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(0))),
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
        let bytes =
            size_of::<EnumDeclarations>() + size_of::<RecordDecl>() + size_of::<FieldDecl>();
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
    fn header_only_limit_admits_only_the_empty_table() {
        let (sources, span) = source();
        let limits = Limits {
            records: 0,
            fields: 0,
            fields_per_record: 0,
            table_bytes: size_of::<EnumDeclarations>(),
            layout_bytes: 0,
        };
        assert!(Declarations::check_with_limits(&[], &sources, limits).is_ok());
        assert_eq!(
            Declarations::check_with_limits(&[record(0, &[], span)], &sources, limits).unwrap_err(),
            DeclarationError::ResourceLimit("record declarations")
        );
    }

    #[test]
    fn count_admission_returns_only_observed_counts_and_table_payload() {
        let usage = admit_declaration_counts([0, 1, 1024].into_iter()).unwrap();
        assert_eq!(usage.records, 3);
        assert_eq!(usage.fields, 1025);
        assert_eq!(
            usage.table_bytes,
            size_of::<EnumDeclarations>()
                + 3 * size_of::<RecordDecl>()
                + 1025 * size_of::<FieldDecl>()
        );
        assert_eq!(usage.layout_bytes, 0);
        assert_eq!(
            admit_declaration_counts(std::iter::empty()).unwrap(),
            DeclarationUsage {
                table_bytes: size_of::<EnumDeclarations>(),
                ..DeclarationUsage::default()
            }
        );
    }

    #[test]
    fn count_only_scalar_layouts_share_padding_and_empty_identity_rules() {
        let fields: &[&[hir::Ty]] = &[&[hir::Ty::Bool, hir::Ty::I32, hir::Ty::Unit], &[]];
        assert_eq!(
            admit_scalar_layouts(fields.iter().map(|fields| fields.iter().copied())),
            Ok(13)
        );
        assert_eq!(
            admit_scalar_layouts(std::iter::empty::<std::iter::Empty<hir::Ty>>()),
            Ok(0)
        );
    }

    #[test]
    fn count_admission_rejects_record_limit_before_visiting_field_counts() {
        struct NeverVisit;
        impl Iterator for NeverVisit {
            type Item = usize;
            fn next(&mut self) -> Option<Self::Item> {
                panic!("record-count admission must precede field counts");
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                let count = Limits::DEFAULT.records + 1;
                (count, Some(count))
            }
        }
        impl ExactSizeIterator for NeverVisit {}
        assert_eq!(
            admit_declaration_counts(NeverVisit),
            Err(DeclarationError::ResourceLimit("record declarations"))
        );
    }

    #[test]
    fn count_admission_rejects_inconsistent_exact_size_iterators() {
        struct Misreported {
            remaining: usize,
            reported: usize,
        }
        impl Iterator for Misreported {
            type Item = usize;
            fn next(&mut self) -> Option<Self::Item> {
                if self.remaining == 0 {
                    None
                } else {
                    self.remaining -= 1;
                    Some(0)
                }
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                (self.reported, Some(self.reported))
            }
        }
        impl ExactSizeIterator for Misreported {}
        for (remaining, reported) in [(1, 0), (2, 1), (1, 2), (4097, 1)] {
            assert_eq!(
                admit_declaration_counts(Misreported {
                    remaining,
                    reported
                }),
                Err(DeclarationError::ResourceLimit("record declarations"))
            );
        }
    }

    #[test]
    fn shared_count_admission_preserves_inclusive_lowered_limits() {
        let exact_bytes =
            size_of::<EnumDeclarations>() + size_of::<RecordDecl>() + 2 * size_of::<FieldDecl>();
        let limits = Limits {
            records: 1,
            fields_per_record: 2,
            fields: 2,
            table_bytes: exact_bytes,
            ..Limits::DEFAULT
        };
        assert_eq!(
            preflight_counts([2].into_iter(), limits)
                .unwrap()
                .table_bytes,
            exact_bytes
        );
        for (changed, expected) in [
            (
                Limits {
                    records: 0,
                    ..limits
                },
                "record declarations",
            ),
            (
                Limits {
                    fields_per_record: 1,
                    ..limits
                },
                "fields per record",
            ),
            (
                Limits {
                    fields: 1,
                    ..limits
                },
                "field declarations",
            ),
            (
                Limits {
                    table_bytes: exact_bytes - 1,
                    ..limits
                },
                "declaration table bytes",
            ),
        ] {
            assert_eq!(
                preflight_counts([2].into_iter(), changed),
                Err(DeclarationError::ResourceLimit(expected))
            );
        }
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
            size_of::<EnumDeclarations>()
                + Limits::DEFAULT.records * size_of::<RecordDecl>()
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
            assert_eq!(value.json().as_deref(), Some(json));
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
    fn explicit_scalar_source_mode_keeps_ownership_syntax_closed() {
        for text in [
            "struct Counter { value: i32 } fn main() -> i32 { return 0; }",
            "fn read(x: &Counter) -> i32 { return 0; }",
            "fn step(x: &mut Counter) -> () { return; }",
            "fn main() -> () { let x = Counter { value: 0 }; return; }",
            "fn main() -> () { let x = 0; read(&x); return; }",
        ] {
            let mut sources = SourceMap::new();
            let file = sources.add("source-mode.ox".into(), text.into());
            let file = sources.get(file);
            let tokens = lexer::lex(file).unwrap();
            assert!(
                parser::parse_with_mode(file, tokens, parser::SourceMode::ScalarOnly).is_err(),
                "unexpected scalar-mode acceptance: {text}"
            );
            let owned = parser::parse(file, lexer::lex(file).unwrap()).unwrap();
            assert!(owned.uses_owned_syntax(file), "missing owned route: {text}");
        }
    }
}

#[cfg(test)]
mod array_tests;

#[cfg(test)]
mod composition_tests;

#[cfg(test)]
mod enum_integration_tests;

#[cfg(test)]
mod u8_tests;

#[cfg(test)]
mod byte_storage_tests;
