//! Closed language facts. Catalog identity is neither source admission nor an
//! executable capability; the index and raw validators prove those separately.
#![allow(dead_code)]
use super::{
    hir::Ty,
    oir::owned_types::{AggregateTy, BorrowKind, BorrowedTy, EnumId, ParameterTy, ValueTy},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum BuiltinSet {
    #[default]
    None,
    ReadStatus,
    ReadStdin,
}
impl BuiltinSet {
    pub fn union(self, other: Self) -> Self {
        match (self, other) {
            (Self::ReadStdin, _) | (_, Self::ReadStdin) => Self::ReadStdin,
            (Self::ReadStatus, _) | (_, Self::ReadStatus) => Self::ReadStatus,
            _ => Self::None,
        }
    }
    pub fn admit(self, item: BuiltinItem) -> Self {
        self.union(match item {
            BuiltinItem::Enum(BuiltinEnum::ReadStatus) => Self::ReadStatus,
            BuiltinItem::Function(BuiltinFunction::ReadStdin) => Self::ReadStdin,
        })
    }
    pub fn extra_enums(self) -> usize {
        usize::from(self != Self::None)
    }
    pub fn extra_functions(self) -> usize {
        usize::from(self == Self::ReadStdin)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BuiltinItem {
    Enum(BuiltinEnum),
    Function(BuiltinFunction),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BuiltinEnum {
    ReadStatus,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BuiltinFunction {
    ReadStdin,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeclarationOrigin {
    Source,
    Builtin(BuiltinItem),
}
impl BuiltinItem {
    pub fn name(self) -> &'static str {
        match self {
            Self::Enum(item) => item.name(),
            Self::Function(item) => item.name(),
        }
    }
    pub fn path(self) -> [&'static str; 3] {
        ["std", "io", self.name()]
    }
}
impl BuiltinEnum {
    pub fn name(self) -> &'static str {
        "ReadStatus"
    }
    pub fn variant_count(self) -> usize {
        3
    }
    pub fn member_name(self, member: usize) -> Option<&'static str> {
        ["Eof", "Full", "IoError"].get(member).copied()
    }
    pub fn member_payload(self, member: usize) -> Option<Ty> {
        (member == 0).then_some(Ty::I32)
    }
}
impl BuiltinFunction {
    pub fn name(self) -> &'static str {
        "read_stdin"
    }
    pub fn signature(self, enumeration: EnumId) -> (ParameterTy, ValueTy) {
        (
            ParameterTy::Reference {
                referent: BorrowedTy::ScalarSlice(Ty::I32),
                kind: BorrowKind::Exclusive,
            },
            ValueTy::Owned(AggregateTy::Enum(enumeration)),
        )
    }
}
