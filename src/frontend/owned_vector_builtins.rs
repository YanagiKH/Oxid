//! Separate catalog facts for finite owned-record `alloc::vec` operations.
//!
//! These keys do not admit a source type, import or executable operation. The
//! owned sidecar and checked consumers authenticate this domain independently
//! from the original scalar/Copy-vector key and its nine-operation stride.
use crate::frontend::{
    hir,
    oir::owned_types::{
        AggregateTy, BorrowKind, BorrowedTy, EnumId, ParameterTy, RecordId, ValueTy,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(in crate::frontend) enum Operation {
    New = 0,
    Len = 1,
    Capacity = 2,
    TryReserve = 3,
    PushReserved = 4,
    PopLast = 5,
    Truncate = 6,
    Take = 7,
}
impl Operation {
    pub const ALL: [Self; 8] = [
        Self::New,
        Self::Len,
        Self::Capacity,
        Self::TryReserve,
        Self::PushReserved,
        Self::PopLast,
        Self::Truncate,
        Self::Take,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Len => "len",
            Self::Capacity => "capacity",
            Self::TryReserve => "try_reserve",
            Self::PushReserved => "push_reserved",
            Self::PopLast => "pop_last",
            Self::Truncate => "truncate",
            Self::Take => "take",
        }
    }

    pub const fn path(self) -> [&'static str; 3] {
        ["alloc", "vec", self.name()]
    }

    pub const fn requires_grow_status(self) -> bool {
        matches!(self, Self::TryReserve)
    }

    /// Shared spellings select a family only after independent element type
    /// admission. Copy-only operations have no corresponding owned operation.
    pub const fn shared_template(self) -> Option<super::vector::Operation> {
        Some(match self {
            Self::New => super::vector::Operation::New,
            Self::Len => super::vector::Operation::Len,
            Self::Capacity => super::vector::Operation::Capacity,
            Self::TryReserve => super::vector::Operation::TryReserve,
            Self::Truncate => super::vector::Operation::Truncate,
            Self::Take => super::vector::Operation::Take,
            Self::PushReserved | Self::PopLast => return None,
        })
    }

    /// Template shape only. Converting this to a ParameterTy/ValueTy requires
    /// the separate checked owned-record vector type and canonical GrowStatus.
    pub const fn signature(self) -> TemplateSignature {
        use Parameter::{Count, Element, Unused, Vector};
        let shared = Vector(BorrowKind::Shared);
        let exclusive = Vector(BorrowKind::Exclusive);
        let (parameters, count, result) = match self {
            Self::New => ([Unused, Unused], 0, ResultType::Vector),
            Self::Len | Self::Capacity => ([shared, Unused], 1, ResultType::Scalar(hir::Ty::I32)),
            Self::TryReserve => ([exclusive, Count], 2, ResultType::GrowStatus),
            Self::PushReserved => ([exclusive, Element], 2, ResultType::Scalar(hir::Ty::Unit)),
            Self::PopLast => ([exclusive, Unused], 1, ResultType::Element),
            Self::Truncate => ([exclusive, Count], 2, ResultType::Scalar(hir::Ty::Unit)),
            Self::Take => ([exclusive, Unused], 1, ResultType::Vector),
        };
        TemplateSignature {
            parameters,
            count,
            result,
        }
    }
}

/// Only these two imports are additional spellings. The six shared templates
/// retain the existing catalog's canonical import identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(in crate::frontend) enum Item {
    PushReserved,
    PopLast,
}
impl Item {
    pub const ALL: [Self; 2] = [Self::PushReserved, Self::PopLast];
    pub const fn operation(self) -> Operation {
        match self {
            Self::PushReserved => Operation::PushReserved,
            Self::PopLast => Operation::PopLast,
        }
    }
    pub const fn name(self) -> &'static str {
        self.operation().name()
    }
    pub const fn path(self) -> [&'static str; 3] {
        self.operation().path()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum Parameter {
    Unused,
    Vector(BorrowKind),
    Element,
    Count,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum ResultType {
    Vector,
    Element,
    Scalar(hir::Ty),
    GrowStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) struct TemplateSignature {
    parameters: [Parameter; 2],
    count: u8,
    pub result: ResultType,
}
impl TemplateSignature {
    pub fn parameters(&self) -> &[Parameter] {
        &self.parameters[..usize::from(self.count)]
    }
}

/// Independent nominal key: the numeric value is meaningful only in this
/// typed sidecar domain. There is no cast/conversion to the old vector key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::frontend) struct Function([u8; 2]);
const OPERATIONS: u16 = 8;
const RECORDS: usize = 4096;
impl Function {
    pub const fn record(operation: Operation, record: RecordId) -> Option<Self> {
        if record.0 >= RECORDS {
            return None;
        }
        Some(Self(
            ((record.0 as u16) * OPERATIONS + operation as u16).to_le_bytes(),
        ))
    }
    pub const fn operation(self) -> Operation {
        Operation::ALL[(u16::from_le_bytes(self.0) % OPERATIONS) as usize]
    }
    pub const fn record_id(self) -> RecordId {
        RecordId((u16::from_le_bytes(self.0) / OPERATIONS) as usize)
    }
    pub const fn name(self) -> &'static str {
        self.operation().name()
    }
    pub const fn path(self) -> [&'static str; 3] {
        self.operation().path()
    }
    pub const fn signature(self) -> TemplateSignature {
        self.operation().signature()
    }

    /// Concrete inert facts. The independent raw checker must prove the
    /// record, import, status and canonical function identity before use.
    pub const fn concrete_signature(self, grow: EnumId) -> Signature {
        let unit = ParameterTy::Value(ValueTy::Scalar(hir::Ty::Unit));
        let vector = AggregateTy::OwnedRecordVector(self.record_id());
        let shared = ParameterTy::Reference {
            referent: BorrowedTy::Exact(vector),
            kind: BorrowKind::Shared,
        };
        let exclusive = ParameterTy::Reference {
            referent: BorrowedTy::Exact(vector),
            kind: BorrowKind::Exclusive,
        };
        let count = ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32));
        let element = ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(self.record_id())));
        let (parameters, count, result) = match self.operation() {
            Operation::New => ([unit; 2], 0, ValueTy::Owned(vector)),
            Operation::Len | Operation::Capacity => {
                ([shared, unit], 1, ValueTy::Scalar(hir::Ty::I32))
            }
            Operation::TryReserve => (
                [exclusive, count],
                2,
                ValueTy::Owned(AggregateTy::Enum(grow)),
            ),
            Operation::PushReserved => ([exclusive, element], 2, ValueTy::Scalar(hir::Ty::Unit)),
            Operation::PopLast => (
                [exclusive, unit],
                1,
                ValueTy::Owned(AggregateTy::Record(self.record_id())),
            ),
            Operation::Truncate => ([exclusive, count], 2, ValueTy::Scalar(hir::Ty::Unit)),
            Operation::Take => ([exclusive, unit], 1, ValueTy::Owned(vector)),
        };
        Signature {
            parameters,
            count,
            result,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) struct Signature {
    parameters: [ParameterTy; 2],
    count: usize,
    pub result: ValueTy,
}
impl Signature {
    pub fn parameters(&self) -> &[ParameterTy] {
        &self.parameters[..self.count]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_vector_catalog_has_separate_eight_operation_key_domain() {
        assert_eq!(std::mem::size_of::<Function>(), 2);
        assert_eq!(std::mem::align_of::<Function>(), 1);
        for record in [0, 1, 17, 4095] {
            for (operation, ordinal) in [
                (Operation::New, 0),
                (Operation::Len, 1),
                (Operation::Capacity, 2),
                (Operation::TryReserve, 3),
                (Operation::PushReserved, 4),
                (Operation::PopLast, 5),
                (Operation::Truncate, 6),
                (Operation::Take, 7),
            ] {
                let key = Function::record(operation, RecordId(record)).unwrap();
                assert_eq!(u16::from_le_bytes(key.0), record as u16 * 8 + ordinal);
                assert_eq!(key.operation(), operation);
                assert_eq!(key.record_id(), RecordId(record));
            }
        }
        assert!(Function::record(Operation::New, RecordId(4096)).is_none());
        assert!(Function::record(Operation::New, RecordId(usize::MAX)).is_none());
    }

    #[test]
    fn owned_vector_catalog_reserved_push_returns_unit_and_pop_returns_owned_element() {
        let push = Operation::PushReserved.signature();
        assert_eq!(
            push.parameters(),
            &[Parameter::Vector(BorrowKind::Exclusive), Parameter::Element]
        );
        assert_eq!(push.result, ResultType::Scalar(hir::Ty::Unit));
        assert!(!Operation::PushReserved.requires_grow_status());
        let pop = Operation::PopLast.signature();
        assert_eq!(
            pop.parameters(),
            &[Parameter::Vector(BorrowKind::Exclusive)]
        );
        assert_eq!(pop.result, ResultType::Element);
        assert!(!Operation::PopLast.requires_grow_status());
        assert_eq!(
            Operation::TryReserve.signature().result,
            ResultType::GrowStatus
        );
        assert!(Operation::TryReserve.requires_grow_status());
    }

    #[test]
    fn owned_vector_catalog_imports_and_shared_templates_are_explicit() {
        assert_eq!(
            Item::ALL.map(Item::path),
            [
                ["alloc", "vec", "push_reserved"],
                ["alloc", "vec", "pop_last"],
            ]
        );
        assert_eq!(Operation::PushReserved.shared_template(), None);
        assert_eq!(Operation::PopLast.shared_template(), None);
        assert_eq!(
            Operation::New.shared_template(),
            Some(super::super::vector::Operation::New)
        );
        assert_eq!(
            Operation::Take.shared_template(),
            Some(super::super::vector::Operation::Take)
        );
        assert_eq!(
            Operation::Len.signature().parameters(),
            &[Parameter::Vector(BorrowKind::Shared)]
        );
        assert_eq!(
            Operation::Take.signature().parameters(),
            &[Parameter::Vector(BorrowKind::Exclusive)]
        );
    }
}
