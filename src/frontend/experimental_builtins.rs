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
    SeekAbsolute,
}
impl Item {
    pub const ALL: [Self; 28] = [
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
        Self::SeekAbsolute,
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
            Self::SeekAbsolute => "seek_absolute",
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
            | Self::Close
            | Self::SeekAbsolute => 3,
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
            Self::SeekAbsolute => Function::SeekAbsolute,

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
    SeekAbsolute,
}
impl Function {
    pub fn is_file(self) -> bool {
        self.item().family() == 3
    }
    pub fn is_arguments(self) -> bool {
        self.item().family() == 4
    }
    pub const ALL: [Self; 20] = [
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
        Self::SeekAbsolute,
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
            Self::SeekAbsolute => Item::SeekAbsolute,
        }
    }
    pub fn result_enum(self) -> Option<Enumeration> {
        match self {
            Self::Reserve | Self::Push | Self::Extend | Self::Resize => Some(Enumeration::Grow),
            Self::ReadSome | Self::FileReadSome => Some(Enumeration::Read),
            Self::WriteSome | Self::FileWriteSome => Some(Enumeration::Write),
            Self::OpenRead | Self::Create => Some(Enumeration::Open),
            Self::SyncAll | Self::Close | Self::SeekAbsolute => Some(Enumeration::File),
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
            SeekAbsolute => ([file, scalar(Ty::I64), unit], 2),
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
pub(in crate::frontend) struct Inventory([u8; 8]);
struct VectorItemCursor {
    mask: u16,
    wanted: usize,
    offset: usize,
}
impl VectorItemCursor {
    fn split(&mut self, width: usize) {
        let lower = self.mask & ((1u16 << width) - 1);
        let count = lower.count_ones() as usize;
        if self.wanted >= count {
            self.wanted -= count;
            self.mask >>= width;
            self.offset += width;
        } else {
            self.mask = lower;
        }
    }
}
// The original vector namespace occupies bits 27..36. Extending the fixed
// catalog must not reinterpret those existing packed authorities.
const VECTOR_ITEM_START: usize = 27;
const SEEK_ITEM_BIT: usize = 37;
impl Item {
    const fn inventory_bit(self) -> usize {
        match self {
            Self::SeekAbsolute => SEEK_ITEM_BIT,
            _ => self as usize,
        }
    }
}
impl Inventory {
    fn bits(self) -> u64 {
        u64::from_le_bytes(self.0)
    }
    // Four legacy bits and the two closed item catalogs fit in one u64.
    // Only dependency-complete inventories reach this path.
    pub(super) fn pack(self, legacy: u8) -> [u8; 8] {
        const {
            assert!(Item::ALL.len() + super::vector::Item::ALL.len() <= 60);
            assert!(VECTOR_ITEM_START + super::vector::Item::ALL.len() <= SEEK_ITEM_BIT);
        }
        debug_assert!(legacy < 9);
        ((self.bits() << 4) | u64::from(legacy)).to_le_bytes()
    }
    pub(super) fn unpack(packed: [u8; 8]) -> (Self, u8) {
        let bits = u64::from_le_bytes(packed);
        (Self((bits >> 4).to_le_bytes()), (bits & 15) as u8)
    }
    pub fn contains(self, item: Item) -> bool {
        self.bits() & (1u64 << item.inventory_bit()) != 0
    }
    pub fn contains_vector(self, item: super::vector::Item) -> bool {
        let bit = VECTOR_ITEM_START + item as usize;
        self.bits() & (1u64 << bit) != 0
    }
    pub fn admit_vector(self, item: super::vector::Item) -> Self {
        let mut inventory = self;
        if item
            .operation()
            .is_some_and(|operation| operation.requires_grow_status())
        {
            inventory = inventory.admit(Item::GrowStatus);
        }
        let bit = VECTOR_ITEM_START + item as usize;
        let vector = VECTOR_ITEM_START + super::vector::Item::Vec as usize;
        Self((inventory.bits() | (1u64 << bit) | (1u64 << vector)).to_le_bytes())
    }
    /// Import templates do not create executable functions. Concrete vector
    /// keys are retained and charged by the authenticated generic plan.
    pub fn vector_templates(self) -> usize {
        let start = VECTOR_ITEM_START + super::vector::Item::New as usize;
        ((self.bits() >> start) & 0x1ff).count_ones() as usize
    }
    pub fn vector_template_rank(self, operation: super::vector::Operation) -> Option<usize> {
        let start = VECTOR_ITEM_START + super::vector::Item::New as usize;
        let bit = VECTOR_ITEM_START + operation.item() as usize;
        self.contains_vector(operation.item()).then(|| {
            let preceding = ((1u64 << bit) - 1) & !((1u64 << start) - 1);
            (self.bits() & preceding).count_ones() as usize
        })
    }
    pub fn vector_item_rank(self, item: super::vector::Item) -> Option<usize> {
        let start = VECTOR_ITEM_START;
        let bit = start + item as usize;
        self.contains_vector(item).then(|| {
            let preceding = ((1u64 << bit) - 1) & !((1u64 << start) - 1);
            (self.bits() & preceding).count_ones() as usize
        })
    }
    /// Select an imported item with four fixed-width bit partitions. This
    /// inspects one catalog row, without scanning the other imported rows.
    pub fn vector_item_at(self, rank: usize) -> Option<super::vector::Item> {
        let mask = ((self.bits() >> VECTOR_ITEM_START) & 0x3ff) as u16;
        if rank >= mask.count_ones() as usize {
            return None;
        }
        let mut cursor = VectorItemCursor {
            mask,
            wanted: rank,
            offset: 0,
        };
        cursor.split(8);
        cursor.split(4);
        cursor.split(2);
        cursor.split(1);
        super::vector::Item::ALL.get(cursor.offset).copied()
    }
    pub fn admit(self, item: Item) -> Self {
        let mut bits = self.bits() | (1u64 << item.inventory_bit());
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
    vector_cursor: VectorItemCursor,
    vector_cursor_receiver: &'static mut VectorItemCursor,
    vector_split_width: usize,
    vector_lower_bits: u16,
    vector_count: usize,
    vector_mask: u64,
    selected_item_bit: usize,
    vector_preceding_mask: u64,
    vector_rank_return: Option<usize>,
    vector_selected: Option<super::vector::Item>,
    vector_caller_selected: Option<super::vector::Item>,
    template_operations: std::array::IntoIter<super::vector::Operation, 9>,
    template_next_operation: Option<super::vector::Operation>,
    template_selected: Option<super::vector::Operation>,
    template_selector_return:
        Result<Option<super::vector::Operation>, Box<crate::frontend::diagnostic::Diagnostic>>,
    template_caller_return:
        Result<Option<super::vector::Operation>, Box<crate::frontend::diagnostic::Diagnostic>>,
    template_caller_selected: Option<super::vector::Operation>,
    signatures: [Signature; 4],
    signature_results: [Result<Signature, Box<crate::frontend::diagnostic::Diagnostic>>; 4],
    item_iterations: [std::array::IntoIter<Item, 28>; 4],
    enumeration_iterations: [std::array::IntoIter<Enumeration, 6>; 4],
    function_iterations: [std::array::IntoIter<Function, 20>; 4],
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
    encoded: [u8; 8],
    returned: (Inventory, u8),
    words: [u64; 3],
    legacy: u8,
}
/// Independent complete conversion roles for bits(), admit(), and union().
/// Iterator growth does not account for the independent u64 conversions.
#[allow(dead_code)]
struct BitConversionCarriers {
    operands: [u64; 2],
    mask: u64,
    combined: u64,
    returned_bits: u64,
    decoded: [u8; 8],
    encoded: [u8; 8],
    returned_inventory: Inventory,
}
pub(in crate::frontend) const fn compiler_carrier_bytes() -> usize {
    std::mem::size_of::<CompilerCarriers>()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seek_preserves_old_ordinals_and_vector_inventory_bits() {
        use super::super::vector::Item as VectorItem;
        assert_eq!(Item::ArgBytes as usize, 26);
        assert_eq!(Function::ArgBytes as usize, 18);
        assert_eq!(Item::SeekAbsolute as usize, 27);
        assert_eq!(Function::SeekAbsolute as usize, 19);
        let seek = Inventory::default().admit(Item::SeekAbsolute);
        assert_eq!(seek.bits(), (1 << 37) | (1 << 15) | (1 << 17));
        for item in VectorItem::ALL {
            let old = Inventory::default().admit_vector(item);
            assert_eq!(
                old.bits() & (0x3ff << 27),
                (1 << 27) | (1 << (27 + item as usize))
            );
            assert!(!old.contains(Item::SeekAbsolute));
            assert!(!seek.contains_vector(item));
            let both = old.union(seek);
            assert!(both.contains_vector(item));
            assert!(both.contains(Item::SeekAbsolute));
            for legacy in 0..9 {
                assert_eq!(Inventory::unpack(both.pack(legacy)), (both, legacy));
            }
        }
        let signature = Function::SeekAbsolute.signature(|enumeration| {
            assert_eq!(enumeration, Enumeration::File);
            EnumId(7)
        });
        assert_eq!(
            signature.parameters[0],
            ParameterTy::Reference {
                kind: BorrowKind::Exclusive,
                referent: BorrowedTy::Exact(AggregateTy::File),
            }
        );
        assert_eq!(
            signature.parameters[1],
            ParameterTy::Value(ValueTy::Scalar(Ty::I64))
        );
        assert_eq!(
            signature.result,
            ValueTy::Owned(AggregateTy::Enum(EnumId(7)))
        );
    }
    #[test]
    fn vector_catalog_selection_and_ranks_cover_all_operation_subsets() {
        for mask in 0usize..512 {
            let mut inventory = Inventory::default();
            for (bit, operation) in super::super::vector::Operation::ALL.into_iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    inventory = inventory.admit_vector(operation.item());
                }
            }
            let expected: Vec<_> = super::super::vector::Item::ALL
                .into_iter()
                .filter(|item| inventory.contains_vector(*item))
                .collect();
            assert_eq!(inventory.vector_templates(), mask.count_ones() as usize);
            for (rank, item) in expected.iter().copied().enumerate() {
                assert_eq!(inventory.vector_item_rank(item), Some(rank));
                assert_eq!(inventory.vector_item_at(rank), Some(item));
            }
            assert_eq!(inventory.vector_item_at(expected.len()), None);
            assert_eq!(inventory.vector_item_at(usize::MAX), None);
            let operations: Vec<_> = super::super::vector::Operation::ALL
                .into_iter()
                .filter(|operation| inventory.contains_vector(operation.item()))
                .collect();
            for operation in super::super::vector::Operation::ALL {
                assert_eq!(
                    inventory.vector_template_rank(operation),
                    operations
                        .iter()
                        .position(|candidate| *candidate == operation)
                );
            }
        }
    }
    #[test]
    fn vector_templates_do_not_create_concrete_functions_or_change_status_identity() {
        use super::super::vector::{Item as VectorItem, Operation};
        let empty = Inventory::default();
        let new = empty.admit_vector(VectorItem::New);
        assert!(new.contains_vector(VectorItem::Vec));
        assert!(new.contains_vector(VectorItem::New));
        assert!(!new.contains(Item::ByteBuf));
        assert_eq!(
            (new.enums(), new.functions(), new.vector_templates()),
            (0, 0, 1)
        );
        let push = empty.admit_vector(VectorItem::TryPush);
        assert!(push.contains(Item::GrowStatus));
        assert_eq!(push.enum_rank(Enumeration::Grow), Some(0));
        assert_eq!(
            (push.enums(), push.functions(), push.vector_templates()),
            (1, 0, 1)
        );
        let both = new.union(push);
        assert_eq!(both.vector_template_rank(Operation::New), Some(0));
        assert_eq!(both.vector_template_rank(Operation::TryPush), Some(1));
        assert_eq!(both.vector_template_rank(Operation::TryResize), None);
        for legacy in 0..9 {
            assert_eq!(Inventory::unpack(both.pack(legacy)), (both, legacy));
        }
        let old = empty.admit(Item::New).admit(Item::WriteSome);
        let union = old.union(both);
        assert_eq!(union.functions(), 2);
        assert!(union.contains(Item::New));
        assert!(union.contains(Item::WriteSome));
        assert!(union.contains_vector(VectorItem::New));
        assert!(union.contains_vector(VectorItem::TryPush));
        assert!(!union.contains_vector(VectorItem::Truncate));
        for legacy in 0..9 {
            assert_eq!(Inventory::unpack(union.pack(legacy)), (union, legacy));
        }
    }
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
        assert_eq!((all.enums(), all.functions()), (6, 20));
        for (index, item) in Function::ALL.into_iter().enumerate() {
            assert_eq!(all.function_rank(item), Some(index));
        }
        assert_eq!(std::mem::size_of::<Inventory>(), 8);
        assert_eq!(std::mem::align_of::<Inventory>(), 1);
    }
}
