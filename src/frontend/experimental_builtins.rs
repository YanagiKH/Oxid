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
    Take,
    File,
    OpenResult,
    FileStatus,
    OpenRead,
    Create,
    FileReadSome,
    FileWriteSome,
    SyncAll,
    Close,
    ArgStatus,
    ArgCount,
    ArgBytes,
}
impl Item {
    pub const ALL: [Self; 27] = [
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
        Self::Take,
        Self::File,
        Self::OpenResult,
        Self::FileStatus,
        Self::OpenRead,
        Self::Create,
        Self::FileReadSome,
        Self::FileWriteSome,
        Self::SyncAll,
        Self::Close,
        Self::ArgStatus,
        Self::ArgCount,
        Self::ArgBytes,
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
            Self::Take => "take",
            Self::File => "File",
            Self::OpenResult => "OpenResult",
            Self::FileStatus => "FileStatus",
            Self::OpenRead => "open_read",
            Self::Create => "create",
            Self::FileReadSome => "read_some",
            Self::FileWriteSome => "write_some",
            Self::SyncAll => "sync_all",
            Self::Close => "close",
            Self::ArgStatus => "ArgStatus",
            Self::ArgCount => "arg_count",
            Self::ArgBytes => "arg_bytes",
        }
    }
    pub fn family(self) -> usize {
        match self {
            Self::ReadSomeStatus | Self::ReadSome => 1,
            Self::WriteSomeStatus | Self::WriteSome => 2,
            Self::File
            | Self::OpenResult
            | Self::FileStatus
            | Self::OpenRead
            | Self::Create
            | Self::FileReadSome
            | Self::FileWriteSome
            | Self::SyncAll
            | Self::Close => 3,
            Self::ArgStatus | Self::ArgCount | Self::ArgBytes => 4,
            _ => 0,
        }
    }
    pub fn path(self) -> [&'static str; 3] {
        match self.family() {
            0 => ["alloc", "bytes", self.name()],
            3 => ["std", "fs", self.name()],
            4 => ["std", "env", self.name()],
            _ => ["std", "io", self.name()],
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
            Self::Take => Function::Take,
            Self::OpenRead => Function::OpenRead,
            Self::Create => Function::Create,
            Self::FileReadSome => Function::FileReadSome,
            Self::FileWriteSome => Function::FileWriteSome,
            Self::SyncAll => Function::SyncAll,
            Self::Close => Function::Close,
            Self::ArgCount => Function::ArgCount,
            Self::ArgBytes => Function::ArgBytes,

            _ => return None,
        })
    }
    pub fn enumeration(self) -> Option<Enumeration> {
        match self {
            Self::GrowStatus => Some(Enumeration::Grow),
            Self::ReadSomeStatus => Some(Enumeration::Read),
            Self::WriteSomeStatus => Some(Enumeration::Write),
            Self::OpenResult => Some(Enumeration::Open),
            Self::FileStatus => Some(Enumeration::File),
            Self::ArgStatus => Some(Enumeration::Arguments),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum Enumeration {
    Grow,
    Read,
    Write,
    Open,
    File,
    Arguments,
}
impl Enumeration {
    pub const ALL: [Self; 6] = [
        Self::Grow,
        Self::Read,
        Self::Write,
        Self::Open,
        Self::File,
        Self::Arguments,
    ];
    pub fn item(self) -> Item {
        match self {
            Self::Grow => Item::GrowStatus,
            Self::Read => Item::ReadSomeStatus,
            Self::Write => Item::WriteSomeStatus,
            Self::Open => Item::OpenResult,
            Self::File => Item::FileStatus,
            Self::Arguments => Item::ArgStatus,
        }
    }
    pub fn members(self) -> &'static [&'static str] {
        match self {
            Self::Grow => &["Ok", "CapacityOverflow", "AllocationFailed", "MemoryLimit"],
            Self::Read => &["Data", "Eof", "Error"],
            Self::Write => &["Written", "Error"],
            Self::Open => &["Opened", "Error", "MemoryLimit", "AllocationFailed"],
            Self::File => &["Ok", "Error"],
            Self::Arguments => &["Copied", "TooSmall", "Missing", "TooLong"],
        }
    }
    pub fn payload(self, member: usize) -> Option<Ty> {
        match (self, member) {
            (Self::Read, 0 | 2) | (Self::Write, 0 | 1) | (Self::Open | Self::File, 1) => {
                Some(Ty::I32)
            }
            (Self::Arguments, 0 | 1) => Some(Ty::I32),
            _ => None,
        }
    }
    pub fn value_payload(self, member: usize) -> Option<ValueTy> {
        if self == Self::Open && member == 0 {
            Some(ValueTy::Owned(AggregateTy::File))
        } else {
            self.payload(member).map(ValueTy::Scalar)
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
    Take,
    OpenRead,
    Create,
    FileReadSome,
    FileWriteSome,
    SyncAll,
    Close,
    ArgCount,
    ArgBytes,
}
impl Function {
    pub fn is_file(self) -> bool {
        self.item().family() == 3
    }
    pub fn is_arguments(self) -> bool {
        self.item().family() == 4
    }
    pub const ALL: [Self; 19] = [
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
        Self::Take,
        Self::OpenRead,
        Self::Create,
        Self::FileReadSome,
        Self::FileWriteSome,
        Self::SyncAll,
        Self::Close,
        Self::ArgCount,
        Self::ArgBytes,
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
            Self::Take => Item::Take,
            Self::OpenRead => Item::OpenRead,
            Self::Create => Item::Create,
            Self::FileReadSome => Item::FileReadSome,
            Self::FileWriteSome => Item::FileWriteSome,
            Self::SyncAll => Item::SyncAll,
            Self::Close => Item::Close,
            Self::ArgCount => Item::ArgCount,
            Self::ArgBytes => Item::ArgBytes,
        }
    }
    pub fn result_enum(self) -> Option<Enumeration> {
        match self {
            Self::Reserve | Self::Push | Self::Extend | Self::Resize => Some(Enumeration::Grow),
            Self::ReadSome | Self::FileReadSome => Some(Enumeration::Read),
            Self::WriteSome | Self::FileWriteSome => Some(Enumeration::Write),
            Self::OpenRead | Self::Create => Some(Enumeration::Open),
            Self::SyncAll | Self::Close => Some(Enumeration::File),
            Self::ArgBytes => Some(Enumeration::Arguments),
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
        let file = ParameterTy::Reference {
            referent: BorrowedTy::Exact(AggregateTy::File),
            kind: BorrowKind::Exclusive,
        };
        let scalar = |ty| ParameterTy::Value(ValueTy::Scalar(ty));
        let (parameters, count) = match self {
            New | ArgCount => ([unit; 3], 0),
            Take => ([exclusive, unit, unit], 1),
            Len | Capacity => ([shared, unit, unit], 1),
            Reserve | Truncate => ([exclusive, scalar(Ty::I32), unit], 2),
            Push => ([exclusive, scalar(Ty::U8), unit], 2),
            Extend => ([exclusive, bytes(BorrowKind::Shared), unit], 2),
            Resize => ([exclusive, scalar(Ty::I32), scalar(Ty::U8)], 3),
            ReadSome => ([bytes(BorrowKind::Exclusive), unit, unit], 1),
            WriteSome | OpenRead | Create => ([bytes(BorrowKind::Shared), unit, unit], 1),
            FileReadSome => ([file, bytes(BorrowKind::Exclusive), unit], 2),
            FileWriteSome => ([file, bytes(BorrowKind::Shared), unit], 2),
            SyncAll | Close => ([file, unit, unit], 1),
            ArgBytes => ([scalar(Ty::I32), bytes(BorrowKind::Exclusive), unit], 2),
        };
        let result = match self {
            New | Take => ValueTy::Owned(AggregateTy::ByteBuf),
            Len | Capacity | ArgCount => ValueTy::Scalar(Ty::I32),
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
/// The byte-aligned bitset preserves existing item ordinals. Its storage and
/// each actual iterator are included in compiler_carrier_bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub(in crate::frontend) struct Inventory([u8; 4]);
impl Inventory {
    fn bits(self) -> u32 {
        u32::from_le_bytes(self.0)
    }
    // Nine legacy states need four bits; all 27 extension identities fit in
    // the remaining 28. Only dependency-complete inventories reach this path.
    pub(super) fn pack(self, legacy: u8) -> [u8; 4] {
        const {
            assert!(Item::ALL.len() <= 28);
        }
        debug_assert!(legacy < 9);
        ((self.bits() << 4) | u32::from(legacy)).to_le_bytes()
    }
    pub(super) fn unpack(packed: [u8; 4]) -> (Self, u8) {
        let bits = u32::from_le_bytes(packed);
        (Self((bits >> 4).to_le_bytes()), (bits & 15) as u8)
    }
    pub fn contains(self, item: Item) -> bool {
        self.bits() & (1 << item as u8) != 0
    }
    pub fn admit(self, item: Item) -> Self {
        let mut bits = self.bits() | (1 << item as u8);
        if item.family() == 0 {
            bits |= 1 << Item::ByteBuf as u8;
        }
        if let Some(enumeration) = item.function().and_then(Function::result_enum) {
            bits |= 1 << enumeration.item() as u8;
        }
        if item.family() == 3 {
            bits |= 1 << Item::File as u8;
        }
        let bytes = bits.to_le_bytes();
        Self(bytes)
    }
    pub fn union(self, other: Self) -> Self {
        let bytes = (self.bits() | other.bits()).to_le_bytes();
        Self(bytes)
    }
    pub fn is_empty(self) -> bool {
        self.bits() == 0
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
/// suffix exists. Index admission growth is charged separately in its plan.
/// This conservative envelope includes independent lookup/return/iteration roles
/// across collection, typing, descriptor validation, and association.
#[allow(dead_code)]
struct CompilerCarriers {
    signatures: [Signature; 4],
    signature_results: [Result<Signature, Box<crate::frontend::diagnostic::Diagnostic>>; 4],
    item_iterations: [std::array::IntoIter<Item, 27>; 4],
    enumeration_iterations: [std::array::IntoIter<Enumeration, 6>; 4],
    function_iterations: [std::array::IntoIter<Function, 19>; 4],
    path_iterations: [std::iter::Zip<
        std::array::IntoIter<&'static str, 3>,
        std::slice::Iter<'static, crate::frontend::source::Span>,
    >; 2],
    ranks: [Option<usize>; 8],
    anchors: [crate::frontend::source::Span; 9],
    parameters: [ParameterTy; 6],
    file_parameter: ParameterTy,
    bit_conversions: [BitConversionCarriers; 3],
    inventory: [Inventory; 8],
    packed_conversions: [PackedInventoryCarriers; 2],
}
#[allow(dead_code)]
struct PackedInventoryCarriers {
    inventory: Inventory,
    encoded: [u8; 4],
    returned: (Inventory, u8),
    words: [u32; 3],
    legacy: u8,
}
/// Independent complete conversion roles for bits(), admit(), and union().
/// Iterator growth cannot pay for these new u32 and four-byte transports.
#[allow(dead_code)]
struct BitConversionCarriers {
    operands: [u32; 2],
    mask: u32,
    combined: u32,
    returned_bits: u32,
    decoded: [u8; 4],
    encoded: [u8; 4],
    returned_inventory: Inventory,
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
            assert_eq!(inventory.contains(Item::File), item.family() == 3);
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
        assert_eq!((all.enums(), all.functions()), (6, 19));
        for (index, item) in Function::ALL.into_iter().enumerate() {
            assert_eq!(all.function_rank(item), Some(index));
        }
        assert_eq!(std::mem::size_of::<Inventory>(), 4);
        assert_eq!(std::mem::align_of::<Inventory>(), 1);
    }
}
