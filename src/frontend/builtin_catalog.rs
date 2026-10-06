//! Closed language facts. Catalog identity is neither source admission nor an
//! executable capability; the index and raw validators prove those separately.
#![allow(dead_code)]
use super::{
    hir::Ty,
    oir::owned_types::{AggregateTy, BorrowKind, BorrowedTy, EnumId, ParameterTy, ValueTy},
};

/// Each family is absent, status-only, or function plus its required status.
/// Explicit variants prevent dependency-incomplete inventories from existing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum BuiltinSet {
    #[default]
    None,
    ReadStatus,
    ReadStdin,
    WriteStatus,
    WriteStdout,
    ReadStatusWriteStatus,
    ReadStatusWriteStdout,
    ReadStdinWriteStatus,
    ReadStdinWriteStdout,
}
impl BuiltinSet {
    pub fn union(self, other: Self) -> Self {
        Self::None
            .with_enum(
                BuiltinEnum::ReadStatus,
                self.contains_enum(BuiltinEnum::ReadStatus)
                    || other.contains_enum(BuiltinEnum::ReadStatus),
            )
            .with_function(
                BuiltinFunction::ReadStdin,
                self.contains_function(BuiltinFunction::ReadStdin)
                    || other.contains_function(BuiltinFunction::ReadStdin),
            )
            .with_enum(
                BuiltinEnum::WriteStatus,
                self.contains_enum(BuiltinEnum::WriteStatus)
                    || other.contains_enum(BuiltinEnum::WriteStatus),
            )
            .with_function(
                BuiltinFunction::WriteStdout,
                self.contains_function(BuiltinFunction::WriteStdout)
                    || other.contains_function(BuiltinFunction::WriteStdout),
            )
    }
    fn with_enum(self, item: BuiltinEnum, present: bool) -> Self {
        if !present {
            return self;
        }
        match (self, item) {
            (Self::None, BuiltinEnum::ReadStatus) => Self::ReadStatus,
            (Self::WriteStatus, BuiltinEnum::ReadStatus) => Self::ReadStatusWriteStatus,
            (Self::WriteStdout, BuiltinEnum::ReadStatus) => Self::ReadStatusWriteStdout,
            (Self::None, BuiltinEnum::WriteStatus) => Self::WriteStatus,
            (Self::ReadStatus, BuiltinEnum::WriteStatus) => Self::ReadStatusWriteStatus,
            (Self::ReadStdin, BuiltinEnum::WriteStatus) => Self::ReadStdinWriteStatus,
            _ => self,
        }
    }
    fn with_function(self, item: BuiltinFunction, present: bool) -> Self {
        if !present {
            return self;
        }
        match (self, item) {
            (Self::None | Self::ReadStatus, BuiltinFunction::ReadStdin) => Self::ReadStdin,
            (Self::WriteStatus | Self::ReadStatusWriteStatus, BuiltinFunction::ReadStdin) => {
                Self::ReadStdinWriteStatus
            }
            (Self::WriteStdout | Self::ReadStatusWriteStdout, BuiltinFunction::ReadStdin) => {
                Self::ReadStdinWriteStdout
            }
            (Self::None | Self::WriteStatus, BuiltinFunction::WriteStdout) => Self::WriteStdout,
            (Self::ReadStatus | Self::ReadStatusWriteStatus, BuiltinFunction::WriteStdout) => {
                Self::ReadStatusWriteStdout
            }
            (Self::ReadStdin | Self::ReadStdinWriteStatus, BuiltinFunction::WriteStdout) => {
                Self::ReadStdinWriteStdout
            }
            _ => self,
        }
    }
    pub fn admit(self, item: BuiltinItem) -> Self {
        match item {
            BuiltinItem::Enum(item) => self.with_enum(item, true),
            BuiltinItem::Function(item) => self.with_function(item, true),
        }
    }
    pub fn contains_enum(self, item: BuiltinEnum) -> bool {
        match item {
            BuiltinEnum::ReadStatus => {
                !matches!(self, Self::None | Self::WriteStatus | Self::WriteStdout)
            }
            BuiltinEnum::WriteStatus => self.has_output(),
        }
    }
    pub fn contains_function(self, item: BuiltinFunction) -> bool {
        match item {
            BuiltinFunction::ReadStdin => matches!(
                self,
                Self::ReadStdin | Self::ReadStdinWriteStatus | Self::ReadStdinWriteStdout
            ),
            BuiltinFunction::WriteStdout => self.has_output_function(),
        }
    }
    pub fn extra_enums(self) -> usize {
        usize::from(self.contains_enum(BuiltinEnum::ReadStatus)) + usize::from(self.has_output())
    }
    pub fn extra_functions(self) -> usize {
        usize::from(self.contains_function(BuiltinFunction::ReadStdin))
            + usize::from(self.has_output_function())
    }
    pub fn enum_rank(self, item: BuiltinEnum) -> Option<usize> {
        if !self.contains_enum(item) {
            return None;
        }
        Some(match item {
            BuiltinEnum::ReadStatus => 0,
            BuiltinEnum::WriteStatus => usize::from(self.contains_enum(BuiltinEnum::ReadStatus)),
        })
    }
    pub fn function_rank(self, item: BuiltinFunction) -> Option<usize> {
        if !self.contains_function(item) {
            return None;
        }
        Some(match item {
            BuiltinFunction::ReadStdin => 0,
            BuiltinFunction::WriteStdout => {
                usize::from(self.contains_function(BuiltinFunction::ReadStdin))
            }
        })
    }
    pub fn has_output(self) -> bool {
        !matches!(self, Self::None | Self::ReadStatus | Self::ReadStdin)
    }
    pub fn has_output_function(self) -> bool {
        matches!(
            self,
            Self::WriteStdout | Self::ReadStatusWriteStdout | Self::ReadStdinWriteStdout
        )
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
    WriteStatus,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BuiltinFunction {
    ReadStdin,
    WriteStdout,
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
    pub const ALL: [Self; 2] = [Self::ReadStatus, Self::WriteStatus];
    pub fn name(self) -> &'static str {
        match self {
            Self::ReadStatus => "ReadStatus",
            Self::WriteStatus => "WriteStatus",
        }
    }
    pub fn variant_count(self) -> usize {
        3
    }
    pub fn member_name(self, member: usize) -> Option<&'static str> {
        match self {
            Self::ReadStatus => ["Eof", "Full", "IoError"],
            Self::WriteStatus => ["Complete", "InvalidInput", "IoError"],
        }
        .get(member)
        .copied()
    }
    pub fn member_payload(self, member: usize) -> Option<Ty> {
        (member
            == match self {
                Self::ReadStatus => 0,
                Self::WriteStatus => 2,
            })
        .then_some(Ty::I32)
    }
}
impl BuiltinFunction {
    pub const ALL: [Self; 2] = [Self::ReadStdin, Self::WriteStdout];
    pub fn name(self) -> &'static str {
        match self {
            Self::ReadStdin => "read_stdin",
            Self::WriteStdout => "write_stdout",
        }
    }
    pub fn signature(self, enumeration: EnumId) -> (ParameterTy, ValueTy) {
        (
            ParameterTy::Reference {
                referent: BorrowedTy::ScalarSlice(Ty::I32),
                kind: match self {
                    Self::ReadStdin => BorrowKind::Exclusive,
                    Self::WriteStdout => BorrowKind::Shared,
                },
            },
            ValueTy::Owned(AggregateTy::Enum(enumeration)),
        )
    }
}
