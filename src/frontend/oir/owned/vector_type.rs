//! Closed checked-type mapping shared by both resource consumers.
//! Only a verified HIR scalar type supplies vector element authority.
use super::byte_buffer::VectorElement;
use crate::frontend::hir;
use crate::frontend::oir::owned_types::{AggregateTy, BorrowedTy, RecordId, ValueTy};

/// The nominal identity is retained separately from scalar layout authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum VectorTy {
    Scalar(hir::Ty),
    Record(RecordId),
}
impl VectorTy {
    pub(super) const fn aggregate(self) -> AggregateTy {
        match self {
            Self::Scalar(ty) => AggregateTy::Vector(ty),
            Self::Record(id) => AggregateTy::RecordVector(id),
        }
    }
    pub(super) const fn slice(self) -> BorrowedTy {
        match self {
            Self::Scalar(ty) => BorrowedTy::ScalarSlice(ty),
            Self::Record(id) => BorrowedTy::RecordSlice(id),
        }
    }
    pub(super) const fn value(self) -> ValueTy {
        match self {
            Self::Scalar(ty) => ValueTy::Scalar(ty),
            Self::Record(id) => ValueTy::Owned(AggregateTy::Record(id)),
        }
    }
    pub(super) const fn from_aggregate(ty: AggregateTy) -> Option<Self> {
        match ty {
            AggregateTy::Vector(ty) => Some(Self::Scalar(ty)),
            AggregateTy::RecordVector(id) => Some(Self::Record(id)),
            AggregateTy::OwnedRecordVector(_) => None,
            _ => None,
        }
    }
    pub(super) const fn from_borrowed(ty: BorrowedTy) -> Option<Self> {
        match ty {
            BorrowedTy::Exact(ty) => Self::from_aggregate(ty),
            BorrowedTy::ScalarSlice(ty) => Some(Self::Scalar(ty)),
            BorrowedTy::RecordSlice(id) => Some(Self::Record(id)),
        }
    }
    pub(super) const fn function(key: crate::frontend::builtin_catalog::vector::Function) -> Self {
        match key.record_id() {
            Some(id) => Self::Record(id),
            None => Self::Scalar(key.element()),
        }
    }
}

pub(super) fn checked<'a>(
    ty: VectorTy,
    declarations: &'a crate::frontend::oir::owned_types::Declarations,
) -> Result<super::byte_buffer::CheckedVectorElement<'a>, super::byte_buffer::Failure> {
    Ok(match ty {
        VectorTy::Scalar(ty) => super::byte_buffer::CheckedVectorElement::Scalar(element(ty)),
        VectorTy::Record(id) => super::byte_buffer::CheckedVectorElement::Record(
            super::byte_buffer::RecordVectorElement::check(declarations, id)?,
        ),
    })
}

pub(super) const fn element(ty: hir::Ty) -> VectorElement {
    match ty {
        hir::Ty::Bool => VectorElement::Bool,
        hir::Ty::Unit => VectorElement::Unit,
        hir::Ty::I8 => VectorElement::I8,
        hir::Ty::U8 => VectorElement::U8,
        hir::Ty::I16 => VectorElement::I16,
        hir::Ty::U16 => VectorElement::U16,
        hir::Ty::I32 => VectorElement::I32,
        hir::Ty::U32 => VectorElement::U32,
        hir::Ty::I64 => VectorElement::I64,
        hir::Ty::U64 => VectorElement::U64,
        hir::Ty::Isize => VectorElement::Isize,
        hir::Ty::Usize => VectorElement::Usize,
        hir::Ty::F32 => VectorElement::F32,
        hir::Ty::F64 => VectorElement::F64,
        hir::Ty::Char => VectorElement::Char,
        hir::Ty::I128 => VectorElement::I128,
        hir::Ty::U128 => VectorElement::U128,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_scalar_types_have_distinct_literal_runtime_tags_and_widths() {
        let cases = [
            (hir::Ty::Bool, 1, 1),
            (hir::Ty::Unit, 2, 1),
            (hir::Ty::I8, 3, 1),
            (hir::Ty::U8, 4, 1),
            (hir::Ty::I16, 5, 2),
            (hir::Ty::U16, 6, 2),
            (hir::Ty::I32, 7, 4),
            (hir::Ty::U32, 8, 4),
            (hir::Ty::I64, 9, 8),
            (hir::Ty::U64, 10, 8),
            (hir::Ty::Isize, 11, 8),
            (hir::Ty::Usize, 12, 8),
            (hir::Ty::F32, 13, 4),
            (hir::Ty::F64, 14, 8),
            (hir::Ty::Char, 15, 4),
            (hir::Ty::I128, 16, 16),
            (hir::Ty::U128, 17, 16),
        ];
        let mut seen = [false; 18];
        for (ty, tag, width) in cases {
            let mapped = element(ty);
            assert_eq!(mapped.tag(), tag);
            assert_eq!(mapped.size(), width);
            assert_eq!(mapped.align(), width);
            assert!(!seen[tag as usize]);
            seen[tag as usize] = true;
        }
        assert!(seen[1..].iter().all(|value| *value));
        assert!(!seen[0]);
    }
}
