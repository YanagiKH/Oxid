//! Closed language facts. Catalog identity is neither source admission nor an
//! executable capability; the index and raw validators prove those separately.
#![allow(dead_code)]
use super::{
    hir::Ty,
    oir::owned_types::{AggregateTy, BorrowKind, BorrowedTy, EnumId, ParameterTy, ValueTy},
};

#[path = "experimental_builtins.rs"]
pub(super) mod experimental;

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
    Extended(ExtendedSet),
}
/// Existing inventories retain their variants. Three byte-aligned payload bytes
/// fit the frozen enclosing raw-program carrier; both fields remain sealed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ExtendedSet {
    legacy: u8,
    experimental: experimental::Inventory,
}
impl BuiltinSet {
    pub fn legacy(self) -> Self {
        match self {
            Self::Extended(value) => [
                Self::None,
                Self::ReadStatus,
                Self::ReadStdin,
                Self::WriteStatus,
                Self::WriteStdout,
                Self::ReadStatusWriteStatus,
                Self::ReadStatusWriteStdout,
                Self::ReadStdinWriteStatus,
                Self::ReadStdinWriteStdout,
            ][value.legacy as usize],
            value => value,
        }
    }
    pub fn experimental(self) -> experimental::Inventory {
        match self {
            Self::Extended(value) => value.experimental,
            _ => experimental::Inventory::default(),
        }
    }
    pub fn with_experimental(self, experimental: experimental::Inventory) -> Self {
        let legacy = match self.legacy() {
            Self::None => 0,
            Self::ReadStatus => 1,
            Self::ReadStdin => 2,
            Self::WriteStatus => 3,
            Self::WriteStdout => 4,
            Self::ReadStatusWriteStatus => 5,
            Self::ReadStatusWriteStdout => 6,
            Self::ReadStdinWriteStatus => 7,
            Self::ReadStdinWriteStdout => 8,
            Self::Extended(_) => unreachable!("unwrapped inventory"),
        };
        if experimental.is_empty() {
            self.legacy()
        } else {
            Self::Extended(ExtendedSet {
                legacy,
                experimental,
            })
        }
    }
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
            .with_experimental(self.experimental().union(other.experimental()))
    }
    fn with_enum(self, item: BuiltinEnum, present: bool) -> Self {
        if !present {
            return self;
        }
        let value = match (self.legacy(), item) {
            (Self::None, BuiltinEnum::ReadStatus) => Self::ReadStatus,
            (Self::WriteStatus, BuiltinEnum::ReadStatus) => Self::ReadStatusWriteStatus,
            (Self::WriteStdout, BuiltinEnum::ReadStatus) => Self::ReadStatusWriteStdout,
            (Self::None, BuiltinEnum::WriteStatus) => Self::WriteStatus,
            (Self::ReadStatus, BuiltinEnum::WriteStatus) => Self::ReadStatusWriteStatus,
            (Self::ReadStdin, BuiltinEnum::WriteStatus) => Self::ReadStdinWriteStatus,
            _ => self.legacy(),
        };
        value.with_experimental(self.experimental())
    }
    fn with_function(self, item: BuiltinFunction, present: bool) -> Self {
        if !present {
            return self;
        }
        let value = match (self.legacy(), item) {
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
            _ => self.legacy(),
        };
        value.with_experimental(self.experimental())
    }
    pub fn admit(self, item: BuiltinItem) -> Self {
        match item {
            BuiltinItem::Enum(item) => self.with_enum(item, true),
            BuiltinItem::Function(item) => self.with_function(item, true),
            BuiltinItem::Experimental(item) => {
                self.with_experimental(self.experimental().admit(item))
            }
        }
    }
    pub fn contains_enum(self, item: BuiltinEnum) -> bool {
        match item {
            BuiltinEnum::ReadStatus => !matches!(
                self.legacy(),
                Self::None | Self::WriteStatus | Self::WriteStdout
            ),
            BuiltinEnum::WriteStatus => self.has_output(),
        }
    }
    pub fn contains_function(self, item: BuiltinFunction) -> bool {
        match item {
            BuiltinFunction::ReadStdin => matches!(
                self.legacy(),
                Self::ReadStdin | Self::ReadStdinWriteStatus | Self::ReadStdinWriteStdout
            ),
            BuiltinFunction::WriteStdout => self.has_output_function(),
        }
    }
    pub fn extra_enums(self) -> usize {
        usize::from(self.contains_enum(BuiltinEnum::ReadStatus))
            + usize::from(self.has_output())
            + self.experimental().enums()
    }
    pub fn extra_functions(self) -> usize {
        usize::from(self.contains_function(BuiltinFunction::ReadStdin))
            + usize::from(self.has_output_function())
            + self.experimental().functions()
    }
    pub fn experimental_enum_rank(self, item: experimental::Enumeration) -> Option<usize> {
        self.experimental()
            .enum_rank(item)
            .map(|rank| self.legacy().extra_enums() + rank)
    }
    pub fn experimental_function_rank(self, item: experimental::Function) -> Option<usize> {
        self.experimental()
            .function_rank(item)
            .map(|rank| self.legacy().extra_functions() + rank)
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
        !matches!(
            self.legacy(),
            Self::None | Self::ReadStatus | Self::ReadStdin
        )
    }
    pub fn has_output_function(self) -> bool {
        matches!(
            self.legacy(),
            Self::WriteStdout | Self::ReadStatusWriteStdout | Self::ReadStdinWriteStdout
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BuiltinItem {
    Enum(BuiltinEnum),
    Function(BuiltinFunction),
    Experimental(experimental::Item),
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
    pub fn variant_count(self) -> usize {
        match self {
            Self::Enum(item) => item.variant_count(),
            Self::Experimental(item) => item.enumeration().map_or(0, |item| item.members().len()),
            Self::Function(_) => 0,
        }
    }
    pub fn member_name(self, index: usize) -> Option<&'static str> {
        match self {
            Self::Enum(item) => item.member_name(index),
            Self::Experimental(item) => item.enumeration()?.members().get(index).copied(),
            Self::Function(_) => None,
        }
    }
    pub fn member_payload(self, index: usize) -> Option<Ty> {
        match self {
            Self::Enum(item) => item.member_payload(index),
            Self::Experimental(item) => item.enumeration()?.payload(index),
            Self::Function(_) => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Enum(item) => item.name(),
            Self::Function(item) => item.name(),
            Self::Experimental(item) => item.name(),
        }
    }
    pub fn path(self) -> [&'static str; 3] {
        match self {
            Self::Experimental(item) => item.path(),
            _ => ["std", "io", self.name()],
        }
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

#[cfg(test)]
mod compact_inventory_tests {
    use super::*;

    #[test]
    fn compact_inventory_preserves_legacy_variants_and_all_extension_bits() {
        assert_eq!(experimental::Item::Take as u8, 14);
        assert_eq!(experimental::Function::Take as u8, 10);
        assert_eq!(std::mem::size_of::<ExtendedSet>(), 3);
        assert_eq!(std::mem::align_of::<ExtendedSet>(), 1);
        assert_eq!(std::mem::size_of::<BuiltinSet>(), 4);
        assert_eq!(std::mem::align_of::<BuiltinSet>(), 1);
        let legacy = [
            BuiltinSet::None,
            BuiltinSet::ReadStatus,
            BuiltinSet::ReadStdin,
            BuiltinSet::WriteStatus,
            BuiltinSet::WriteStdout,
            BuiltinSet::ReadStatusWriteStatus,
            BuiltinSet::ReadStatusWriteStdout,
            BuiltinSet::ReadStdinWriteStatus,
            BuiltinSet::ReadStdinWriteStdout,
        ];
        for original in legacy {
            let mut accumulated = original;
            for item in experimental::Item::ALL {
                let inventory = experimental::Inventory::default().admit(item);
                let extended = original.with_experimental(inventory);
                assert_eq!(extended.legacy(), original);
                assert_eq!(extended.experimental(), inventory);
                assert_eq!(extended.with_experimental(Default::default()), original);
                for kind in BuiltinEnum::ALL {
                    assert_eq!(extended.contains_enum(kind), original.contains_enum(kind));
                }
                for kind in BuiltinFunction::ALL {
                    assert_eq!(
                        extended.contains_function(kind),
                        original.contains_function(kind)
                    );
                }
                accumulated = accumulated.union(extended);
            }
            assert_eq!(accumulated.legacy(), original);
            for item in experimental::Item::ALL {
                assert!(accumulated.experimental().contains(item));
            }
            assert_eq!(accumulated.experimental().enums(), 3);
            assert_eq!(accumulated.experimental().functions(), 11);
        }
    }
}
