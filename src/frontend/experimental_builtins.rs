//! Closed facts for the experimental buffer/binary slice. These identities
//! authorize nothing until source association and the ordinary OIR proof pass.
use super::{AggregateTy, BorrowKind, BorrowedTy, EnumId, ParameterTy, Ty, ValueTy};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(in crate::frontend) enum Item {
    ByteBuf,
    GrowStatus,
    ReadSomeStatus,
    WriteSomeStatus,
    New,
    Len,
    Capacity,
    Reserve,
    Push,
    Extend,
    Resize,
    Truncate,
    ReadSome,
    WriteSome,
}
impl Item {
    pub const ALL: [Self; 14] = [
        Self::ByteBuf,
        Self::GrowStatus,
        Self::ReadSomeStatus,
        Self::WriteSomeStatus,
        Self::New,
        Self::Len,
        Self::Capacity,
        Self::Reserve,
        Self::Push,
        Self::Extend,
        Self::Resize,
        Self::Truncate,
        Self::ReadSome,
        Self::WriteSome,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::ByteBuf => "ByteBuf",
            Self::GrowStatus => "GrowStatus",
            Self::ReadSomeStatus => "ReadSomeStatus",
            Self::WriteSomeStatus => "WriteSomeStatus",
            Self::New => "new",
            Self::Len => "len",
            Self::Capacity => "capacity",
            Self::Reserve => "try_reserve",
            Self::Push => "try_push",
            Self::Extend => "try_extend",
            Self::Resize => "try_resize",
            Self::Truncate => "truncate",
            Self::ReadSome => "read_some",
            Self::WriteSome => "write_some",
        }
    }
    pub fn family(self) -> usize {
        match self {
            Self::ReadSomeStatus | Self::ReadSome => 1,
            Self::WriteSomeStatus | Self::WriteSome => 2,
            _ => 0,
        }
    }
    pub fn path(self) -> [&'static str; 3] {
        if self.family() == 0 {
            ["alloc", "bytes", self.name()]
        } else {
            ["std", "io", self.name()]
        }
    }
    pub fn function(self) -> Option<Function> {
        Some(match self {
            Self::New => Function::New,
            Self::Len => Function::Len,
            Self::Capacity => Function::Capacity,
            Self::Reserve => Function::Reserve,
            Self::Push => Function::Push,
            Self::Extend => Function::Extend,
            Self::Resize => Function::Resize,
            Self::Truncate => Function::Truncate,
            Self::ReadSome => Function::ReadSome,
            Self::WriteSome => Function::WriteSome,
            _ => return None,
        })
    }
    pub fn enumeration(self) -> Option<Enumeration> {
        match self {
            Self::GrowStatus => Some(Enumeration::Grow),
            Self::ReadSomeStatus => Some(Enumeration::Read),
            Self::WriteSomeStatus => Some(Enumeration::Write),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum Enumeration {
    Grow,
    Read,
    Write,
}
impl Enumeration {
    pub const ALL: [Self; 3] = [Self::Grow, Self::Read, Self::Write];
    pub fn item(self) -> Item {
        match self {
            Self::Grow => Item::GrowStatus,
            Self::Read => Item::ReadSomeStatus,
            Self::Write => Item::WriteSomeStatus,
        }
    }
    pub fn members(self) -> &'static [&'static str] {
        match self {
            Self::Grow => &["Ok", "CapacityOverflow", "AllocationFailed", "MemoryLimit"],
            Self::Read => &["Data", "Eof", "Error"],
            Self::Write => &["Written", "Error"],
        }
    }
    pub fn payload(self, member: usize) -> Option<Ty> {
        match (self, member) {
            (Self::Read, 0 | 2) | (Self::Write, 0 | 1) => Some(Ty::I32),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum Function {
    New,
    Len,
    Capacity,
    Reserve,
    Push,
    Extend,
    Resize,
    Truncate,
    ReadSome,
    WriteSome,
}
impl Function {
    pub const ALL: [Self; 10] = [
        Self::New,
        Self::Len,
        Self::Capacity,
        Self::Reserve,
        Self::Push,
        Self::Extend,
        Self::Resize,
        Self::Truncate,
        Self::ReadSome,
        Self::WriteSome,
    ];
    pub fn item(self) -> Item {
        match self {
            Self::New => Item::New,
            Self::Len => Item::Len,
            Self::Capacity => Item::Capacity,
            Self::Reserve => Item::Reserve,
            Self::Push => Item::Push,
            Self::Extend => Item::Extend,
            Self::Resize => Item::Resize,
            Self::Truncate => Item::Truncate,
            Self::ReadSome => Item::ReadSome,
            Self::WriteSome => Item::WriteSome,
        }
    }
    pub fn result_enum(self) -> Option<Enumeration> {
        match self {
            Self::Reserve | Self::Push | Self::Extend | Self::Resize => Some(Enumeration::Grow),
            Self::ReadSome => Some(Enumeration::Read),
            Self::WriteSome => Some(Enumeration::Write),
            _ => None,
        }
    }
    /// Fixed canonical parameters, with no allocation or source-provided type.
    pub fn signature(self, mut enumeration: impl FnMut(Enumeration) -> EnumId) -> Signature {
        use Function::*;
        let unit = ParameterTy::Value(ValueTy::Scalar(Ty::Unit));
        let shared = ParameterTy::Reference {
            referent: BorrowedTy::Exact(AggregateTy::ByteBuf),
            kind: BorrowKind::Shared,
        };
        let exclusive = ParameterTy::Reference {
            referent: BorrowedTy::Exact(AggregateTy::ByteBuf),
            kind: BorrowKind::Exclusive,
        };
        let bytes = |kind| ParameterTy::Reference {
            referent: BorrowedTy::ScalarSlice(Ty::U8),
            kind,
        };
        let scalar = |ty| ParameterTy::Value(ValueTy::Scalar(ty));
        let (parameters, count) = match self {
            New => ([unit; 3], 0),
            Len | Capacity => ([shared, unit, unit], 1),
            Reserve | Truncate => ([exclusive, scalar(Ty::I32), unit], 2),
            Push => ([exclusive, scalar(Ty::U8), unit], 2),
            Extend => ([exclusive, bytes(BorrowKind::Shared), unit], 2),
            Resize => ([exclusive, scalar(Ty::I32), scalar(Ty::U8)], 3),
            ReadSome => ([bytes(BorrowKind::Exclusive), unit, unit], 1),
            WriteSome => ([bytes(BorrowKind::Shared), unit, unit], 1),
        };
        let result = match self {
            New => ValueTy::Owned(AggregateTy::ByteBuf),
            Len | Capacity => ValueTy::Scalar(Ty::I32),
            Truncate => ValueTy::Scalar(Ty::Unit),
            _ => ValueTy::Owned(AggregateTy::Enum(enumeration(
                self.result_enum().expect("enum result"),
            ))),
        };
        Signature {
            parameters,
            count,
            result,
        }
    }
}
pub(in crate::frontend) struct Signature {
    parameters: [ParameterTy; 3],
    count: usize,
    pub result: ValueTy,
}
impl Signature {
    pub fn parameters(&self) -> &[ParameterTy] {
        &self.parameters[..self.count]
    }
}

/// Only admit can set bits, and it installs all signature dependencies first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::frontend) struct Inventory(u16);
impl Inventory {
    pub fn contains(self, item: Item) -> bool {
        self.0 & (1 << item as u8) != 0
    }
    pub fn admit(mut self, item: Item) -> Self {
        self.0 |= 1 << item as u8;
        if item.family() == 0 {
            self.0 |= 1 << Item::ByteBuf as u8;
        }
        if let Some(enumeration) = item.function().and_then(Function::result_enum) {
            self.0 |= 1 << enumeration.item() as u8;
        }
        self
    }
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub fn enums(self) -> usize {
        Enumeration::ALL
            .into_iter()
            .filter(|item| self.contains(item.item()))
            .count()
    }
    pub fn functions(self) -> usize {
        Function::ALL
            .into_iter()
            .filter(|item| self.contains(item.item()))
            .count()
    }
    pub fn enum_rank(self, item: Enumeration) -> Option<usize> {
        self.contains(item.item()).then(|| {
            Enumeration::ALL
                .into_iter()
                .take_while(|current| *current != item)
                .filter(|current| self.contains(current.item()))
                .count()
        })
    }
    pub fn function_rank(self, item: Function) -> Option<usize> {
        self.contains(item.item()).then(|| {
            Function::ALL
                .into_iter()
                .take_while(|current| *current != item)
                .filter(|current| self.contains(current.item()))
                .count()
        })
    }
}

/// Supplemental catalog control storage, charged only when this experimental
/// suffix exists. Retained index/raw carriers keep their frozen representations.
/// This conservative envelope includes independent lookup/return/iteration roles
/// across collection, typing, descriptor validation, and association.
#[allow(dead_code)]
struct CompilerCarriers {
    signatures: [Signature; 4],
    signature_results: [Result<Signature, Box<crate::frontend::diagnostic::Diagnostic>>; 4],
    item_iterations: [std::array::IntoIter<Item, 14>; 4],
    enumeration_iterations: [std::array::IntoIter<Enumeration, 3>; 4],
    function_iterations: [std::array::IntoIter<Function, 10>; 4],
    path_iterations: [std::iter::Zip<
        std::array::IntoIter<&'static str, 3>,
        std::slice::Iter<'static, crate::frontend::source::Span>,
    >; 2],
    ranks: [Option<usize>; 8],
    anchors: [crate::frontend::source::Span; 8],
    parameters: [ParameterTy; 6],
    inventory: [Inventory; 8],
}
pub(in crate::frontend) const fn compiler_carrier_bytes() -> usize {
    std::mem::size_of::<CompilerCarriers>()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn experimental_catalog_dependencies_and_signatures() {
        for item in Item::ALL {
            let inventory = Inventory::default().admit(item);
            assert!(inventory.contains(item));
            assert_eq!(inventory.contains(Item::ByteBuf), item.family() == 0);
            assert_eq!(inventory.admit(item), inventory);
            if let Some(function) = item.function() {
                let signature = function.signature(|_| EnumId(7));
                assert_eq!(inventory.functions(), 1);
                assert_eq!(inventory.function_rank(function), Some(0));
                if let Some(enumeration) = function.result_enum() {
                    assert!(inventory.contains(enumeration.item()));
                    assert_eq!(
                        signature.result,
                        ValueTy::Owned(AggregateTy::Enum(EnumId(7)))
                    );
                }
            }
        }
        let all = Item::ALL
            .into_iter()
            .fold(Inventory::default(), Inventory::admit);
        assert_eq!((all.enums(), all.functions()), (3, 10));
        for (index, item) in Function::ALL.into_iter().enumerate() {
            assert_eq!(all.function_rank(item), Some(index));
        }
        assert_eq!(std::mem::size_of::<Inventory>(), 2);
    }
}
