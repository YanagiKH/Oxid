//! Checked sum declarations, integrated by the combined declaration facade.
//! Nominal types do not admit stored enum fields, borrows, source lowering or
//! executable consumers. The facade independently recomputes record inventory
//! and completes both preflights before retained allocation.

use super::{
    hir, DeclarationError, DeclarationUsage, Layout, ParameterTy, SourceMap, Span, ValueTy,
};
use crate::frontend::project::budget::{Allocator, ReserveFailure};
use std::mem::size_of;

/// Compilation-local nominal identity, in enum declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) struct EnumId(pub(in crate::frontend) usize);

/// The declaring enum is essential: an ordinal alone is never a variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) struct VariantId {
    pub(in crate::frontend) enumeration: EnumId,
    pub(in crate::frontend) index: usize,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
pub(in crate::frontend::oir) struct RawEnumDecl {
    pub(in crate::frontend::oir) id: EnumId,
    pub(in crate::frontend::oir) span: Span,
    pub(in crate::frontend::oir) variants: Vec<RawVariantDecl>,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::frontend::oir) struct RawVariantDecl {
    pub(in crate::frontend::oir) id: VariantId,
    // This fixed-size raw carrier permits unsupported aggregate/reference types
    // so their rejection belongs to the checker, never a trusted producer.
    // None is nullary; Some(Unit) still requires one explicit unit operand.
    pub(in crate::frontend::oir) payload: Option<ParameterTy>,
    pub(in crate::frontend::oir) span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum EnumDeclarationError {
    Declaration(DeclarationError),
    InvalidEnumId(EnumId),
    InvalidVariantId(VariantId),
    NonScalarPayload(VariantId),
    EnumMismatch,
    PayloadArity(VariantId),
    InvalidTag { enumeration: EnumId, tag: usize },
}

impl From<DeclarationError> for EnumDeclarationError {
    fn from(error: DeclarationError) -> Self {
        Self::Declaration(error)
    }
}

impl From<ReserveFailure> for EnumDeclarationError {
    fn from(error: ReserveFailure) -> Self {
        Self::Declaration(match error {
            ReserveFailure::Allocation => DeclarationError::Allocation,
            ReserveFailure::Overflow => DeclarationError::ResourceLimit("declaration table bytes"),
        })
    }
}

/// Requested enum-only output payload, excluding caller-owned raw input,
/// allocator overhead and constant-size vector headers. Shared admission adds
/// this to checked record usage without reinterpreting records as enum rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::frontend) struct EnumUsage {
    pub(in crate::frontend) enums: usize,
    pub(in crate::frontend) variants: usize,
    pub(in crate::frontend) table_bytes: usize,
    pub(in crate::frontend) layout_bytes: usize,
}

#[derive(Debug)]
pub(in crate::frontend::oir) struct EnumDecl {
    id: EnumId,
    span: Span,
    variant_start: usize,
    variant_end: usize,
}

impl EnumDecl {
    pub(in crate::frontend::oir) fn id(&self) -> EnumId {
        self.id
    }
    pub(in crate::frontend::oir) fn span(&self) -> Span {
        self.span
    }
    pub(in crate::frontend::oir) fn layout(&self) -> Layout {
        Layout { size: 8, align: 4 }
    }
    pub(in crate::frontend::oir) fn width(&self) -> usize {
        2
    }
}

#[derive(Debug)]
pub(in crate::frontend::oir) struct VariantDecl {
    id: VariantId,
    payload: Option<hir::Ty>,
    span: Span,
}

impl VariantDecl {
    pub(in crate::frontend::oir) fn id(&self) -> VariantId {
        self.id
    }
    pub(in crate::frontend::oir) fn span(&self) -> Span {
        self.span
    }
    pub(in crate::frontend::oir) fn payload(&self) -> Option<hir::Ty> {
        self.payload
    }
    pub(in crate::frontend::oir) fn arity(&self) -> usize {
        usize::from(self.payload.is_some())
    }
    /// Internal little-endian u32 tag at offset zero, never a source scalar.
    pub(in crate::frontend::oir) fn tag(&self) -> u32 {
        u32::try_from(self.id.index).expect("checked variant ordinal below 256")
    }
    /// Nullary variants have no payload access, including to inactive bytes.
    pub(in crate::frontend::oir) fn payload_offset(&self) -> Option<usize> {
        self.payload.map(|_| 4)
    }
}

/// Only a successful prepare can create this table. Storage is flat: no
/// per-enum/per-variant nested allocations and no expanded payload inventory.
#[derive(Debug)]
pub(in crate::frontend::oir) struct EnumDeclarations {
    enums: Vec<EnumDecl>,
    variants: Vec<VariantDecl>,
    usage: EnumUsage,
}

impl EnumDeclarations {
    #[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
    pub(in crate::frontend::oir) fn observer_capacity_bytes(&self) -> Option<usize> {
        self.enums.capacity().checked_mul(std::mem::size_of::<EnumDecl>())?
            .checked_add(self.variants.capacity().checked_mul(std::mem::size_of::<VariantDecl>())?)
    }

    pub(in crate::frontend::oir) fn check(
        raw: &[RawEnumDecl],
        sources: &SourceMap,
        existing: DeclarationUsage,
    ) -> Result<Self, EnumDeclarationError> {
        Self::prepare(raw, sources, existing)?.finish_with_allocator(&mut Allocator::default())
    }

    /// Allocation-free full validation, held against an immutable raw slice.
    /// `existing` is a checkpoint integration input, not proof: the combined
    /// facade must compute it independently, never accept producer assertions.
    pub(in crate::frontend::oir) fn prepare<'a>(
        raw: &'a [RawEnumDecl],
        sources: &SourceMap,
        existing: DeclarationUsage,
    ) -> Result<PreparedEnums<'a>, EnumDeclarationError> {
        prepare_with_limits(raw, sources, existing, Limits::DEFAULT)
    }

    pub(in crate::frontend::oir) fn usage(&self) -> EnumUsage {
        self.usage
    }
    pub(in crate::frontend::oir) fn enums(&self) -> &[EnumDecl] {
        &self.enums
    }
    pub(in crate::frontend::oir) fn enumeration(
        &self,
        id: EnumId,
    ) -> Result<&EnumDecl, EnumDeclarationError> {
        self.enums
            .get(id.0)
            .filter(|declaration| declaration.id == id)
            .ok_or(EnumDeclarationError::InvalidEnumId(id))
    }
    pub(in crate::frontend::oir) fn variants(
        &self,
        id: EnumId,
    ) -> Result<&[VariantDecl], EnumDeclarationError> {
        let declaration = self.enumeration(id)?;
        self.variants
            .get(declaration.variant_start..declaration.variant_end)
            .ok_or(EnumDeclarationError::InvalidEnumId(id))
    }
    pub(in crate::frontend::oir) fn variant(
        &self,
        enumeration: EnumId,
        id: VariantId,
    ) -> Result<&VariantDecl, EnumDeclarationError> {
        let variants = self.variants(enumeration)?;
        if id.enumeration != enumeration {
            return Err(EnumDeclarationError::EnumMismatch);
        }
        variants
            .get(id.index)
            .filter(|variant| variant.id == id)
            .ok_or(EnumDeclarationError::InvalidVariantId(id))
    }
    /// Fail closed before consumers inspect any active or inactive payload.
    /// No tag, type or offset from raw producers can override this table.
    pub(in crate::frontend::oir) fn variant_for_tag(
        &self,
        enumeration: EnumId,
        tag: usize,
    ) -> Result<&VariantDecl, EnumDeclarationError> {
        self.variants(enumeration)?
            .get(tag)
            .ok_or(EnumDeclarationError::InvalidTag { enumeration, tag })
    }
    pub(in crate::frontend::oir) fn same_enum_type(
        &self,
        actual: EnumId,
        expected: EnumId,
    ) -> Result<(), EnumDeclarationError> {
        self.enumeration(actual)?;
        self.enumeration(expected)?;
        if actual == expected {
            Ok(())
        } else {
            Err(EnumDeclarationError::EnumMismatch)
        }
    }
    /// Scalar constructor operands and arm bindings have exact presence/type.
    pub(in crate::frontend::oir) fn check_payload(
        &self,
        enumeration: EnumId,
        id: VariantId,
        actual: Option<hir::Ty>,
    ) -> Result<(), EnumDeclarationError> {
        let expected = self.variant(enumeration, id)?.payload();
        if actual.is_some() != expected.is_some() {
            return Err(EnumDeclarationError::PayloadArity(id));
        }
        if actual != expected {
            return Err(DeclarationError::TypeMismatch.into());
        }
        Ok(())
    }
}

/// Unforgeable allocation-free admission token. Retaining the immutable raw
/// borrow prevents changing identities, spans, payloads or lengths after check.
#[derive(Debug)]
pub(in crate::frontend::oir) struct PreparedEnums<'a> {
    raw: &'a [RawEnumDecl],
    usage: EnumUsage,
}

impl PreparedEnums<'_> {
    pub(in crate::frontend::oir) fn usage(&self) -> EnumUsage {
        self.usage
    }

    pub(in crate::frontend::oir) fn finish_with_allocator(
        self,
        allocator: &mut Allocator,
    ) -> Result<EnumDeclarations, EnumDeclarationError> {
        let mut enums = Vec::new();
        let mut variants = Vec::new();
        if !self.raw.is_empty() {
            // Both complete byte counts and all declaration contents were
            // validated by prepare, before either fallible exact reservation.
            allocator.vector_exact(&mut enums, self.usage.enums, "checked enum declarations")?;
            allocator.vector_exact(&mut variants, self.usage.variants, "checked enum variants")?;
        }
        for declaration in self.raw {
            let variant_start = variants.len();
            for variant in &declaration.variants {
                variants.push(VariantDecl {
                    id: variant.id,
                    payload: scalar_payload(variant)?,
                    span: variant.span,
                });
            }
            enums.push(EnumDecl {
                id: declaration.id,
                span: declaration.span,
                variant_start,
                variant_end: variants.len(),
            });
        }
        Ok(EnumDeclarations {
            enums,
            variants,
            usage: self.usage,
        })
    }
}

#[derive(Clone, Copy)]
struct Limits {
    declarations: usize,
    members: usize,
    variants_per_enum: usize,
    table_bytes: usize,
    layout_bytes: usize,
}

impl Limits {
    const DEFAULT: Self = Self {
        declarations: super::Limits::DEFAULT.records,
        members: super::Limits::DEFAULT.fields,
        variants_per_enum: 256,
        table_bytes: super::Limits::DEFAULT.table_bytes,
        layout_bytes: super::Limits::DEFAULT.layout_bytes,
    };

    fn bounded(self) -> Self {
        Self {
            declarations: self.declarations.min(Self::DEFAULT.declarations),
            members: self.members.min(Self::DEFAULT.members),
            variants_per_enum: self.variants_per_enum.min(Self::DEFAULT.variants_per_enum),
            table_bytes: self.table_bytes.min(Self::DEFAULT.table_bytes),
            layout_bytes: self.layout_bytes.min(Self::DEFAULT.layout_bytes),
        }
    }
}

fn scalar_payload(variant: &RawVariantDecl) -> Result<Option<hir::Ty>, EnumDeclarationError> {
    match variant.payload {
        None => Ok(None),
        Some(ParameterTy::Value(ValueTy::Scalar(
            ty @ (hir::Ty::Bool | hir::Ty::I32 | hir::Ty::Unit),
        ))) => Ok(Some(ty)),
        Some(_) => Err(EnumDeclarationError::NonScalarPayload(variant.id)),
    }
}

fn prepare_with_limits<'a>(
    raw: &'a [RawEnumDecl],
    sources: &SourceMap,
    existing: DeclarationUsage,
    limits: Limits,
) -> Result<PreparedEnums<'a>, EnumDeclarationError> {
    let usage = preflight_counts(
        raw.iter().map(|declaration| declaration.variants.len()),
        existing,
        limits,
    )?;
    for (index, declaration) in raw.iter().enumerate() {
        if declaration.id != EnumId(index) {
            return Err(EnumDeclarationError::InvalidEnumId(declaration.id));
        }
        super::check_nominal_span(sources, declaration.span)?;
        for (index, variant) in declaration.variants.iter().enumerate() {
            if variant.id
                != (VariantId {
                    enumeration: declaration.id,
                    index,
                })
            {
                return Err(EnumDeclarationError::InvalidVariantId(variant.id));
            }
            super::check_span(sources, variant.span)?;
            scalar_payload(variant)?;
        }
    }
    Ok(PreparedEnums { raw, usage })
}

/// Allocation-free inventory, not a checked declaration witness. Existing
/// record usage must be independently recomputed by the integrating caller.
/// ExactSizeIterator reports are checked against actual bounded visits.
pub(in crate::frontend) fn admit_enum_counts(
    variant_counts: impl ExactSizeIterator<Item = usize>,
    existing: DeclarationUsage,
) -> Result<EnumUsage, EnumDeclarationError> {
    preflight_counts(variant_counts, existing, Limits::DEFAULT)
}

fn preflight_counts(
    variant_counts: impl ExactSizeIterator<Item = usize>,
    existing: DeclarationUsage,
    limits: Limits,
) -> Result<EnumUsage, EnumDeclarationError> {
    let limits = limits.bounded();
    let expected = variant_counts.len();
    super::limited_add(
        existing.records,
        expected,
        limits.declarations,
        "aggregate declarations",
    )?;
    super::limited_add(existing.fields, 0, limits.members, "aggregate members")?;
    super::limited_add(
        existing.table_bytes,
        0,
        limits.table_bytes,
        "declaration table bytes",
    )?;
    super::limited_add(
        existing.layout_bytes,
        0,
        limits.layout_bytes,
        "declaration layout bytes",
    )?;
    let mut enums = 0;
    let mut variants = 0;
    for count in variant_counts {
        enums = super::limited_add(enums, 1, expected, "enum declarations")?;
        if count == 0 || count > limits.variants_per_enum {
            return Err(DeclarationError::ResourceLimit("variants per enum").into());
        }
        variants = super::limited_add(variants, count, limits.members, "aggregate members")?;
        super::limited_add(
            existing.fields,
            variants,
            limits.members,
            "aggregate members",
        )?;
    }
    if enums != expected {
        return Err(DeclarationError::ResourceLimit("enum declarations").into());
    }
    let table_bytes = enums
        .checked_mul(size_of::<EnumDecl>())
        .zip(variants.checked_mul(size_of::<VariantDecl>()))
        .and_then(|(enums, variants)| enums.checked_add(variants))
        .ok_or(DeclarationError::ResourceLimit("declaration table bytes"))?;
    let layout_bytes = enums
        .checked_mul(8)
        .ok_or(DeclarationError::ResourceLimit("declaration layout bytes"))?;
    super::limited_add(
        existing.table_bytes,
        table_bytes,
        limits.table_bytes,
        "declaration table bytes",
    )?;
    super::limited_add(
        existing.layout_bytes,
        layout_bytes,
        limits.layout_bytes,
        "declaration layout bytes",
    )?;
    Ok(EnumUsage {
        enums,
        variants,
        table_bytes,
        layout_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::oir::owned_types::{
        tests::{record, source},
        AggregateTy, BorrowKind, BorrowedTy, Declarations, FixedArrayTy, RecordId,
    };
    use crate::frontend::source::SourceFileId;

    fn raw_enum(id: usize, payloads: &[Option<hir::Ty>], span: Span) -> RawEnumDecl {
        RawEnumDecl {
            id: EnumId(id),
            span,
            variants: payloads
                .iter()
                .enumerate()
                .map(|(index, payload)| RawVariantDecl {
                    id: VariantId {
                        enumeration: EnumId(id),
                        index,
                    },
                    payload: payload.map(|ty| ParameterTy::Value(ValueTy::Scalar(ty))),
                    span,
                })
                .collect(),
        }
    }

    fn limit(resource: &'static str) -> EnumDeclarationError {
        DeclarationError::ResourceLimit(resource).into()
    }

    fn check(
        raw: &[RawEnumDecl],
        sources: &SourceMap,
    ) -> Result<EnumDeclarations, EnumDeclarationError> {
        EnumDeclarations::check(raw, sources, DeclarationUsage::default())
    }

    #[test]
    fn empty_table_has_no_output_allocation() {
        let (sources, _) = source();
        let prepared =
            EnumDeclarations::prepare(&[], &sources, DeclarationUsage::default()).unwrap();
        assert_eq!(prepared.usage(), EnumUsage::default());
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(0).unwrap();
        let table = prepared.finish_with_allocator(&mut allocator).unwrap();
        assert_eq!(table.usage(), EnumUsage::default());
        assert!(table.enums().is_empty());
        assert_eq!(table.enums.capacity(), 0);
        assert_eq!(table.variants.capacity(), 0);
        assert_eq!(allocator.attempts, 0);
        assert!(!allocator.observer_trace_overflow);
    }

    #[test]
    fn all_payload_shapes_keep_fixed_layout_width_and_declaration_order_tags() {
        let (sources, span) = source();
        let payloads = [
            None,
            Some(hir::Ty::Bool),
            Some(hir::Ty::I32),
            Some(hir::Ty::Unit),
        ];
        let raw = [raw_enum(0, &payloads, span), raw_enum(1, &[None], span)];
        let table = check(&raw, &sources).unwrap();
        for (index, declaration) in table.enums().iter().enumerate() {
            assert_eq!(declaration.id(), EnumId(index));
            assert_eq!(declaration.span(), span);
            assert_eq!(declaration.layout(), Layout { size: 8, align: 4 });
            assert_eq!(declaration.width(), 2);
        }
        assert_eq!(table.usage().layout_bytes, 16);
        for (index, variant) in table.variants(EnumId(0)).unwrap().iter().enumerate() {
            assert_eq!(variant.id(), raw[0].variants[index].id);
            assert_eq!(variant.span(), span);
            assert_eq!(variant.tag(), u32::try_from(index).unwrap());
            assert_eq!(variant.payload(), payloads[index]);
            assert_eq!(variant.arity(), usize::from(index != 0));
            assert_eq!(variant.payload_offset(), (index != 0).then_some(4));
            assert_eq!(
                table.variant_for_tag(EnumId(0), index).unwrap().id(),
                variant.id()
            );
        }
        assert_eq!(table.variants(EnumId(1)).unwrap()[0].tag(), 0);
    }

    #[test]
    fn identical_enums_remain_nominally_distinct_and_invalid_equal_ids_reject() {
        let (sources, span) = source();
        let raw = [
            raw_enum(0, &[Some(hir::Ty::I32)], span),
            raw_enum(1, &[Some(hir::Ty::I32)], span),
        ];
        let table = check(&raw, &sources).unwrap();
        assert_eq!(table.same_enum_type(EnumId(0), EnumId(0)), Ok(()));
        assert_eq!(
            table.same_enum_type(EnumId(0), EnumId(1)),
            Err(EnumDeclarationError::EnumMismatch)
        );
        assert_eq!(
            table.variant(EnumId(0), raw[1].variants[0].id).unwrap_err(),
            EnumDeclarationError::EnumMismatch
        );
        for id in [EnumId(2), EnumId(usize::MAX)] {
            assert_eq!(
                table.same_enum_type(id, id),
                Err(EnumDeclarationError::InvalidEnumId(id))
            );
            assert_eq!(
                table.same_enum_type(EnumId(0), id),
                Err(EnumDeclarationError::InvalidEnumId(id))
            );
            assert_eq!(
                table.enumeration(id).unwrap_err(),
                EnumDeclarationError::InvalidEnumId(id)
            );
            assert_eq!(
                table.variants(id).unwrap_err(),
                EnumDeclarationError::InvalidEnumId(id)
            );
            assert_eq!(
                table.variant_for_tag(id, 0).unwrap_err(),
                EnumDeclarationError::InvalidEnumId(id)
            );
            let variant = VariantId {
                enumeration: id,
                index: 0,
            };
            assert_eq!(
                table.variant(id, variant).unwrap_err(),
                EnumDeclarationError::InvalidEnumId(id)
            );
        }
        for index in [1, usize::MAX] {
            let id = VariantId {
                enumeration: EnumId(0),
                index,
            };
            assert_eq!(
                table.variant(EnumId(0), id).unwrap_err(),
                EnumDeclarationError::InvalidVariantId(id)
            );
            assert_eq!(
                table.check_payload(EnumId(0), id, Some(hir::Ty::I32)),
                Err(EnumDeclarationError::InvalidVariantId(id))
            );
        }
    }

    #[test]
    fn payload_presence_and_exact_scalar_types_are_checked() {
        let (sources, span) = source();
        let payloads = [
            None,
            Some(hir::Ty::Bool),
            Some(hir::Ty::I32),
            Some(hir::Ty::Unit),
        ];
        let raw = [raw_enum(0, &payloads, span)];
        let table = check(&raw, &sources).unwrap();
        for (index, expected) in payloads.iter().copied().enumerate() {
            let id = raw[0].variants[index].id;
            for actual in payloads {
                let result = table.check_payload(EnumId(0), id, actual);
                if actual == expected {
                    assert_eq!(result, Ok(()));
                } else if actual.is_some() != expected.is_some() {
                    assert_eq!(result, Err(EnumDeclarationError::PayloadArity(id)));
                } else {
                    assert_eq!(result, Err(DeclarationError::TypeMismatch.into()));
                }
            }
        }
    }

    #[test]
    fn duplicate_out_of_order_and_huge_enum_ids_reject() {
        let (sources, span) = source();
        for id in [0, 2, usize::MAX] {
            let raw = [raw_enum(0, &[None], span), raw_enum(id, &[None], span)];
            assert_eq!(
                check(&raw, &sources).unwrap_err(),
                EnumDeclarationError::InvalidEnumId(EnumId(id))
            );
        }
        assert_eq!(
            check(
                &[raw_enum(1, &[None], span), raw_enum(0, &[None], span)],
                &sources
            )
            .unwrap_err(),
            EnumDeclarationError::InvalidEnumId(EnumId(1))
        );
    }

    #[test]
    fn duplicate_foreign_out_of_order_and_huge_variant_ids_reject() {
        let (sources, span) = source();
        for id in [
            VariantId {
                enumeration: EnumId(0),
                index: 0,
            },
            VariantId {
                enumeration: EnumId(1),
                index: 1,
            },
            VariantId {
                enumeration: EnumId(usize::MAX),
                index: 1,
            },
            VariantId {
                enumeration: EnumId(0),
                index: 2,
            },
            VariantId {
                enumeration: EnumId(0),
                index: usize::MAX,
            },
        ] {
            let mut raw = [raw_enum(0, &[None, None], span)];
            raw[0].variants[1].id = id;
            assert_eq!(
                check(&raw, &sources).unwrap_err(),
                EnumDeclarationError::InvalidVariantId(id)
            );
        }
    }

    #[test]
    fn aggregate_array_reference_and_slice_payloads_reject_independently() {
        let (sources, span) = source();
        let array = FixedArrayTy::check(hir::Ty::I32, 1).unwrap();
        for ty in [
            ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(0)))),
            ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(RecordId(usize::MAX)))),
            ParameterTy::Value(ValueTy::Owned(AggregateTy::FixedArray(array))),
            ParameterTy::Reference {
                referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(0))),
                kind: BorrowKind::Shared,
            },
            ParameterTy::Reference {
                referent: BorrowedTy::Exact(AggregateTy::FixedArray(array)),
                kind: BorrowKind::Exclusive,
            },
            ParameterTy::Reference {
                referent: BorrowedTy::ScalarSlice(hir::Ty::I32),
                kind: BorrowKind::Shared,
            },
        ] {
            let mut raw = [raw_enum(0, &[None], span), raw_enum(1, &[None], span)];
            raw[1].variants[0].payload = Some(ty);
            assert_eq!(
                check(&raw, &sources).unwrap_err(),
                EnumDeclarationError::NonScalarPayload(raw[1].variants[0].id)
            );
        }
    }

    #[test]
    fn every_enum_and_variant_span_is_validated_including_utf8_boundaries() {
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
                end: usize::MAX,
                ..span
            },
            Span {
                start: 8,
                end: 9,
                ..span
            },
        ] {
            for variant_span in [false, true] {
                let mut raw = [raw_enum(0, &[None], span), raw_enum(1, &[None, None], span)];
                if variant_span {
                    raw[1].variants[1].span = bad;
                } else {
                    raw[1].span = bad;
                }
                assert_eq!(
                    check(&raw, &sources).unwrap_err(),
                    DeclarationError::InvalidSpan(bad).into()
                );
            }
        }
    }

    #[test]
    fn zero_and_257_variants_reject_but_1_and_256_admit() {
        let (sources, span) = source();
        for count in [0, 1, 255, 256, 257] {
            let raw = [raw_enum(0, &vec![None; count], span)];
            let result = check(&raw, &sources);
            if (1..=256).contains(&count) {
                let table = result.unwrap();
                assert_eq!(table.variants(EnumId(0)).unwrap().len(), count);
                assert_eq!(
                    table.variant_for_tag(EnumId(0), count - 1).unwrap().tag(),
                    u32::try_from(count - 1).unwrap()
                );
            } else {
                assert_eq!(result.unwrap_err(), limit("variants per enum"));
            }
        }
    }

    #[test]
    fn invalid_tags_including_u32_and_usize_maxima_fail_closed() {
        let (sources, span) = source();
        let table = check(
            &[raw_enum(0, &vec![Some(hir::Ty::I32); 256], span)],
            &sources,
        )
        .unwrap();
        assert_eq!(table.variant_for_tag(EnumId(0), 255).unwrap().tag(), 255);
        for tag in [256, 257, usize::try_from(u32::MAX).unwrap(), usize::MAX] {
            assert_eq!(
                table.variant_for_tag(EnumId(0), tag).unwrap_err(),
                EnumDeclarationError::InvalidTag {
                    enumeration: EnumId(0),
                    tag
                }
            );
        }
    }

    #[test]
    fn checked_table_owns_flat_rows_without_raw_aliases() {
        let (sources, span) = source();
        let mut raw = [raw_enum(0, &[Some(hir::Ty::Bool)], span)];
        let table = check(&raw, &sources).unwrap();
        raw[0].variants[0].payload = Some(ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32)));
        raw[0].variants.clear();
        assert_eq!(
            table.variants(EnumId(0)).unwrap()[0].payload(),
            Some(hir::Ty::Bool)
        );
        assert_eq!(table.usage().variants, 1);
    }

    #[test]
    fn count_admission_reports_enum_only_exact_row_and_layout_payloads() {
        let (sources, span) = source();
        let records = Declarations::check(&[record(0, &[hir::Ty::I32], span)], &sources).unwrap();
        let usage = admit_enum_counts([1, 256].into_iter(), records.usage()).unwrap();
        assert_eq!(usage.enums, 2);
        assert_eq!(usage.variants, 257);
        assert_eq!(
            usage.table_bytes,
            2 * size_of::<EnumDecl>() + 257 * size_of::<VariantDecl>()
        );
        assert_eq!(usage.layout_bytes, 16);
        let raw = [
            raw_enum(0, &[None], span),
            raw_enum(1, &vec![Some(hir::Ty::I32); 256], span),
        ];
        assert_eq!(
            EnumDeclarations::check(&raw, &sources, records.usage())
                .unwrap()
                .usage(),
            usage
        );
        assert_eq!(
            admit_enum_counts(std::iter::empty(), records.usage()),
            Ok(EnumUsage::default())
        );
    }

    #[test]
    fn shared_4096_declaration_and_65536_member_caps_are_inclusive() {
        let existing = DeclarationUsage {
            records: 4095,
            ..DeclarationUsage::default()
        };
        assert!(admit_enum_counts([1].into_iter(), existing).is_ok());
        assert_eq!(
            admit_enum_counts([1, 1].into_iter(), existing),
            Err(limit("aggregate declarations"))
        );
        let existing = DeclarationUsage {
            fields: 65_535,
            ..DeclarationUsage::default()
        };
        assert!(admit_enum_counts([1].into_iter(), existing).is_ok());
        assert_eq!(
            admit_enum_counts([2].into_iter(), existing),
            Err(limit("aggregate members"))
        );
        for counts in [vec![256; 256], vec![256; 255]] {
            assert!(admit_enum_counts(counts.into_iter(), DeclarationUsage::default()).is_ok());
        }
        assert_eq!(
            admit_enum_counts(vec![256; 257].into_iter(), DeclarationUsage::default()),
            Err(limit("aggregate members"))
        );
    }

    #[test]
    fn existing_usage_overflow_is_rejected_even_with_no_enums() {
        for (existing, resource) in [
            (
                DeclarationUsage {
                    records: usize::MAX,
                    ..DeclarationUsage::default()
                },
                "aggregate declarations",
            ),
            (
                DeclarationUsage {
                    fields: usize::MAX,
                    ..DeclarationUsage::default()
                },
                "aggregate members",
            ),
            (
                DeclarationUsage {
                    table_bytes: usize::MAX,
                    ..DeclarationUsage::default()
                },
                "declaration table bytes",
            ),
            (
                DeclarationUsage {
                    layout_bytes: usize::MAX,
                    ..DeclarationUsage::default()
                },
                "declaration layout bytes",
            ),
        ] {
            assert_eq!(
                admit_enum_counts(std::iter::empty(), existing),
                Err(limit(resource))
            );
        }
    }

    #[test]
    fn full_default_byte_and_layout_caps_include_existing_records() {
        let row_bytes = size_of::<EnumDecl>() + size_of::<VariantDecl>();
        for spare in [0, 1, 2] {
            let existing = DeclarationUsage {
                table_bytes: Limits::DEFAULT.table_bytes - row_bytes + 1 - spare,
                ..DeclarationUsage::default()
            };
            let result = admit_enum_counts([1].into_iter(), existing);
            if spare == 0 {
                assert_eq!(result, Err(limit("declaration table bytes")));
            } else {
                assert!(result.is_ok());
            }
            let existing = DeclarationUsage {
                layout_bytes: Limits::DEFAULT.layout_bytes - 8 + 1 - spare,
                ..DeclarationUsage::default()
            };
            let result = admit_enum_counts([1].into_iter(), existing);
            if spare == 0 {
                assert_eq!(result, Err(limit("declaration layout bytes")));
            } else {
                assert!(result.is_ok());
            }
        }
    }

    #[test]
    fn all_preflight_caps_precede_contents_and_output_allocation() {
        let (sources, span) = source();
        let valid = [raw_enum(0, &[None], span)];
        let malformed = [raw_enum(usize::MAX, &[None], span)];
        let bytes = size_of::<EnumDecl>() + size_of::<VariantDecl>();
        for (limits, resource) in [
            (
                Limits {
                    declarations: 0,
                    ..Limits::DEFAULT
                },
                "aggregate declarations",
            ),
            (
                Limits {
                    members: 0,
                    ..Limits::DEFAULT
                },
                "aggregate members",
            ),
            (
                Limits {
                    variants_per_enum: 0,
                    ..Limits::DEFAULT
                },
                "variants per enum",
            ),
            (
                Limits {
                    table_bytes: bytes - 1,
                    ..Limits::DEFAULT
                },
                "declaration table bytes",
            ),
            (
                Limits {
                    layout_bytes: 7,
                    ..Limits::DEFAULT
                },
                "declaration layout bytes",
            ),
        ] {
            let mut allocator = Allocator::default();
            let result =
                prepare_with_limits(&malformed, &sources, DeclarationUsage::default(), limits)
                    .and_then(|prepared| prepared.finish_with_allocator(&mut allocator));
            assert_eq!(result.unwrap_err(), limit(resource));
            assert_eq!(allocator.attempts, 0);
        }
        let exact = Limits {
            declarations: 1,
            members: 1,
            variants_per_enum: 1,
            table_bytes: bytes,
            layout_bytes: 8,
        };
        assert!(prepare_with_limits(&valid, &sources, DeclarationUsage::default(), exact).is_ok());
        // Even malformed earlier contents cannot preempt a later count failure.
        let raw = [
            raw_enum(usize::MAX, &[None], span),
            raw_enum(1, &vec![None; 257], span),
        ];
        assert_eq!(
            check(&raw, &sources).unwrap_err(),
            limit("variants per enum")
        );
    }

    #[test]
    fn no_private_limit_override_can_raise_existing_ceilings() {
        let unlimited = Limits {
            declarations: usize::MAX,
            members: usize::MAX,
            variants_per_enum: usize::MAX,
            table_bytes: usize::MAX,
            layout_bytes: usize::MAX,
        };
        assert_eq!(
            preflight_counts([257].into_iter(), DeclarationUsage::default(), unlimited),
            Err(limit("variants per enum"))
        );
        assert_eq!(
            preflight_counts(
                std::iter::repeat_n(1, 4097),
                DeclarationUsage::default(),
                unlimited
            ),
            Err(limit("aggregate declarations"))
        );
        assert_eq!(
            preflight_counts(
                std::iter::repeat_n(256, 257),
                DeclarationUsage::default(),
                unlimited
            ),
            Err(limit("aggregate members"))
        );
        let zero = Limits {
            declarations: 0,
            members: 0,
            variants_per_enum: 0,
            table_bytes: 0,
            layout_bytes: 0,
        };
        assert_eq!(
            preflight_counts(std::iter::empty(), DeclarationUsage::default(), zero),
            Ok(EnumUsage::default())
        );
    }

    #[test]
    fn count_admission_rejects_huge_claim_before_visiting_input() {
        struct NeverVisit;
        impl Iterator for NeverVisit {
            type Item = usize;
            fn next(&mut self) -> Option<Self::Item> {
                panic!("preflight must reject before visiting variant counts")
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                (usize::MAX, Some(usize::MAX))
            }
        }
        impl ExactSizeIterator for NeverVisit {}
        assert_eq!(
            admit_enum_counts(NeverVisit, DeclarationUsage::default()),
            Err(limit("aggregate declarations"))
        );
    }

    #[test]
    fn false_exact_size_claims_do_not_authorize_unmetered_traversal() {
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
                    Some(1)
                }
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                (self.reported, Some(self.reported))
            }
        }
        impl ExactSizeIterator for Misreported {}
        for (remaining, reported) in [(1, 0), (2, 1), (1, 2), (usize::MAX, 1)] {
            assert_eq!(
                admit_enum_counts(
                    Misreported {
                        remaining,
                        reported
                    },
                    DeclarationUsage::default()
                ),
                Err(limit("enum declarations"))
            );
        }
    }

    #[test]
    fn every_retained_allocation_has_an_observed_real_failure_control() {
        let (sources, span) = source();
        let raw = [
            raw_enum(0, &[None, Some(hir::Ty::I32)], span),
            raw_enum(1, &[Some(hir::Ty::Bool)], span),
        ];
        for fail_at in [Some(1), Some(2), Some(3), None] {
            let mut allocator = Allocator {
                fail_at,
                ..Allocator::default()
            };
            allocator.observer_trace_bound(2).unwrap();
            let prepared =
                EnumDeclarations::prepare(&raw, &sources, DeclarationUsage::default()).unwrap();
            assert_eq!(allocator.attempts, 0);
            let result = prepared.finish_with_allocator(&mut allocator);
            if matches!(fail_at, Some(1 | 2)) {
                assert_eq!(result.unwrap_err(), DeclarationError::Allocation.into());
                assert_eq!(allocator.attempts, fail_at.unwrap());
                assert!(!allocator.trace.last().unwrap().success);
            } else {
                let table = result.unwrap();
                assert_eq!(allocator.attempts, 2);
                assert_eq!(table.enums.len(), 2);
                assert_eq!(table.variants.len(), 3);
                assert!(allocator.trace.iter().all(|event| event.success));
            }
            for (event, expected) in allocator.trace.iter().zip([
                ("checked enum declarations", 2, size_of::<EnumDecl>()),
                ("checked enum variants", 3, size_of::<VariantDecl>()),
            ]) {
                assert_eq!((event.kind, event.length, event.element_bytes), expected);
            }
            assert!(!allocator.observer_trace_overflow);
        }
    }

    #[test]
    fn declaration_count_does_not_introduce_nested_output_allocations() {
        let (sources, span) = source();
        let raw: Vec<_> = (0..256).map(|id| raw_enum(id, &[None], span)).collect();
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(2).unwrap();
        let table = EnumDeclarations::prepare(&raw, &sources, DeclarationUsage::default())
            .unwrap()
            .finish_with_allocator(&mut allocator)
            .unwrap();
        assert_eq!(allocator.attempts, 2);
        assert_eq!(
            table.usage().table_bytes,
            256 * (size_of::<EnumDecl>() + size_of::<VariantDecl>())
        );
        assert_eq!(
            allocator
                .trace
                .iter()
                .map(|event| event.length * event.element_bytes)
                .sum::<usize>(),
            table.usage().table_bytes
        );
        assert!(!allocator.observer_trace_overflow);
    }

    #[test]
    fn bounded_enum_declaration_carrier_measurements() {
        let measured = [
            size_of::<EnumId>(),
            size_of::<VariantId>(),
            size_of::<RawEnumDecl>(),
            size_of::<RawVariantDecl>(),
            size_of::<EnumDecl>(),
            size_of::<VariantDecl>(),
            size_of::<EnumUsage>(),
            size_of::<PreparedEnums<'_>>(),
            size_of::<EnumDeclarations>(),
        ];
        eprintln!("enum declaration carriers [EnumId, VariantId, RawEnumDecl, RawVariantDecl, EnumDecl, VariantDecl, EnumUsage, PreparedEnums, EnumDeclarations]: {measured:?}");
        if cfg!(target_pointer_width = "64") {
            assert_eq!(measured, [8, 16, 56, 64, 48, 48, 32, 48, 80]);
        }
    }
}
