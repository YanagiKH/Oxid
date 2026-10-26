//! Closed builtin identities for scalar `alloc::vec` operations.
//!
//! Imports select templates. The paid generic plan specializes concrete keys,
//! accounts for their storage and lookup work, and authenticates their IDs.
//! GrowStatus shares the existing byte-buffer enumeration identity.

use crate::frontend::{
    hir,
    oir::owned_types::{AggregateTy, BorrowKind, BorrowedTy, EnumId, ParameterTy, ValueTy},
};

/// Nine public operation templates. There is no table of all concrete pairs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::frontend) enum Operation {
    New,
    Len,
    Capacity,
    TryReserve,
    TryPush,
    TryExtend,
    TryResize,
    Truncate,
    Take,
}

impl Operation {
    pub const ALL: [Self; 9] = [
        Self::New,
        Self::Len,
        Self::Capacity,
        Self::TryReserve,
        Self::TryPush,
        Self::TryExtend,
        Self::TryResize,
        Self::Truncate,
        Self::Take,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Len => "len",
            Self::Capacity => "capacity",
            Self::TryReserve => "try_reserve",
            Self::TryPush => "try_push",
            Self::TryExtend => "try_extend",
            Self::TryResize => "try_resize",
            Self::Truncate => "truncate",
            Self::Take => "take",
        }
    }

    pub const fn path(self) -> [&'static str; 3] {
        ["alloc", "vec", self.name()]
    }

    pub const fn parameter_count(self) -> usize {
        match self {
            Self::New => 0,
            Self::Len | Self::Capacity | Self::Take => 1,
            Self::TryReserve | Self::TryPush | Self::TryExtend | Self::Truncate => 2,
            Self::TryResize => 3,
        }
    }

    pub const fn requires_grow_status(self) -> bool {
        matches!(
            self,
            Self::TryReserve | Self::TryPush | Self::TryExtend | Self::TryResize
        )
    }

    pub const fn item(self) -> Item {
        match self {
            Self::New => Item::New,
            Self::Len => Item::Len,
            Self::Capacity => Item::Capacity,
            Self::TryReserve => Item::TryReserve,
            Self::TryPush => Item::TryPush,
            Self::TryExtend => Item::TryExtend,
            Self::TryResize => Item::TryResize,
            Self::Truncate => Item::Truncate,
            Self::Take => Item::Take,
        }
    }
}

/// Import identities only. Integration appends these to the existing catalog
/// without renumbering its original items, and widens its packed inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(in crate::frontend) enum Item {
    Vec,
    New,
    Len,
    Capacity,
    TryReserve,
    TryPush,
    TryExtend,
    TryResize,
    Truncate,
    Take,
}

impl Item {
    pub const ALL: [Self; 10] = [
        Self::Vec,
        Self::New,
        Self::Len,
        Self::Capacity,
        Self::TryReserve,
        Self::TryPush,
        Self::TryExtend,
        Self::TryResize,
        Self::Truncate,
        Self::Take,
    ];

    pub const fn operation(self) -> Option<Operation> {
        Some(match self {
            Self::Vec => return None,
            Self::New => Operation::New,
            Self::Len => Operation::Len,
            Self::Capacity => Operation::Capacity,
            Self::TryReserve => Operation::TryReserve,
            Self::TryPush => Operation::TryPush,
            Self::TryExtend => Operation::TryExtend,
            Self::TryResize => Operation::TryResize,
            Self::Truncate => Operation::Truncate,
            Self::Take => Operation::Take,
        })
    }

    pub const fn name(self) -> &'static str {
        match self.operation() {
            Some(operation) => operation.name(),
            None => "Vec",
        }
    }

    pub const fn path(self) -> [&'static str; 3] {
        ["alloc", "vec", self.name()]
    }
}

/// Admission stays explicit when additional scalar variants are introduced.
pub(in crate::frontend) const fn scalar_admitted(element: hir::Ty) -> bool {
    matches!(
        element,
        hir::Ty::Bool
            | hir::Ty::I32
            | hir::Ty::U8
            | hir::Ty::Unit
            | hir::Ty::I64
            | hir::Ty::I8
            | hir::Ty::I16
            | hir::Ty::U16
            | hir::Ty::U32
            | hir::Ty::U64
            | hir::Ty::Isize
            | hir::Ty::Usize
            | hir::Ty::I128
            | hir::Ty::U128
            | hir::Ty::F32
            | hir::Ty::F64
            | hir::Ty::Char
    )
}

/// A concrete specialization key, independent from import aliases and call IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::frontend) struct Function {
    pub operation: Operation,
    pub element: hir::Ty,
}

impl Function {
    pub const fn new(operation: Operation, element: hir::Ty) -> Option<Self> {
        if scalar_admitted(element) {
            Some(Self { operation, element })
        } else {
            None
        }
    }

    pub const fn item(self) -> Item {
        self.operation.item()
    }

    pub const fn name(self) -> &'static str {
        self.operation.name()
    }

    pub const fn path(self) -> [&'static str; 3] {
        self.operation.path()
    }

    pub const fn parameter_count(self) -> usize {
        self.operation.parameter_count()
    }

    /// The caller supplies the canonical existing GrowStatus identity. No
    /// element-specific status enum is allocated or synthesized.
    pub const fn result(self, grow_status: EnumId) -> ValueTy {
        match self.operation {
            Operation::New | Operation::Take => ValueTy::Owned(AggregateTy::Vector(self.element)),
            Operation::Len | Operation::Capacity => ValueTy::Scalar(hir::Ty::I32),
            Operation::Truncate => ValueTy::Scalar(hir::Ty::Unit),
            Operation::TryReserve
            | Operation::TryPush
            | Operation::TryExtend
            | Operation::TryResize => ValueTy::Owned(AggregateTy::Enum(grow_status)),
        }
    }

    pub const fn signature(self, grow_status: EnumId) -> Signature {
        let unit = ParameterTy::Value(ValueTy::Scalar(hir::Ty::Unit));
        let shared = ParameterTy::Reference {
            referent: BorrowedTy::Exact(AggregateTy::Vector(self.element)),
            kind: BorrowKind::Shared,
        };
        let exclusive = ParameterTy::Reference {
            referent: BorrowedTy::Exact(AggregateTy::Vector(self.element)),
            kind: BorrowKind::Exclusive,
        };
        let count = ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32));
        let value = ParameterTy::Value(ValueTy::Scalar(self.element));
        let elements = ParameterTy::Reference {
            referent: BorrowedTy::ScalarSlice(self.element),
            kind: BorrowKind::Shared,
        };
        let parameters = match self.operation {
            Operation::New => [unit; 3],
            Operation::Len | Operation::Capacity => [shared, unit, unit],
            Operation::Take => [exclusive, unit, unit],
            Operation::TryReserve | Operation::Truncate => [exclusive, count, unit],
            Operation::TryPush => [exclusive, value, unit],
            Operation::TryExtend => [exclusive, elements, unit],
            Operation::TryResize => [exclusive, count, value],
        };
        Signature {
            parameters,
            count: self.parameter_count(),
            result: self.result(grow_status),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_imports_have_explicit_paths_and_arities() {
        assert_eq!(Item::Vec.path(), ["alloc", "vec", "Vec"]);
        assert_eq!(Item::Vec.operation(), None);
        let expected = [
            (Operation::New, "new", 0, false),
            (Operation::Len, "len", 1, false),
            (Operation::Capacity, "capacity", 1, false),
            (Operation::TryReserve, "try_reserve", 2, true),
            (Operation::TryPush, "try_push", 2, true),
            (Operation::TryExtend, "try_extend", 2, true),
            (Operation::TryResize, "try_resize", 3, true),
            (Operation::Truncate, "truncate", 2, false),
            (Operation::Take, "take", 1, false),
        ];
        assert_eq!(Operation::ALL.len(), 9);
        assert_eq!(Item::ALL.len(), 10);
        for (operation, name, parameters, grow) in expected {
            assert_eq!(operation.name(), name);
            assert_eq!(operation.path(), ["alloc", "vec", name]);
            assert_eq!(operation.parameter_count(), parameters);
            assert_eq!(operation.requires_grow_status(), grow);
            assert_eq!(operation.item().operation(), Some(operation));
            assert_eq!(operation.item().path(), ["alloc", "vec", name]);
        }
    }

    #[test]
    fn nine_concrete_signatures_match_independent_vector_and_slice_types() {
        let shared = ParameterTy::Reference {
            referent: BorrowedTy::Exact(AggregateTy::Vector(hir::Ty::I64)),
            kind: BorrowKind::Shared,
        };
        let exclusive = ParameterTy::Reference {
            referent: BorrowedTy::Exact(AggregateTy::Vector(hir::Ty::I64)),
            kind: BorrowKind::Exclusive,
        };
        let elements = ParameterTy::Reference {
            referent: BorrowedTy::ScalarSlice(hir::Ty::I64),
            kind: BorrowKind::Shared,
        };
        let count = ParameterTy::Value(ValueTy::Scalar(hir::Ty::I32));
        let value = ParameterTy::Value(ValueTy::Scalar(hir::Ty::I64));
        let grow = ValueTy::Owned(AggregateTy::Enum(EnumId(23)));
        let vector = ValueTy::Owned(AggregateTy::Vector(hir::Ty::I64));
        let expected: [(Operation, &[ParameterTy], ValueTy); 9] = [
            (Operation::New, &[], vector),
            (Operation::Len, &[shared], ValueTy::Scalar(hir::Ty::I32)),
            (
                Operation::Capacity,
                &[shared],
                ValueTy::Scalar(hir::Ty::I32),
            ),
            (Operation::TryReserve, &[exclusive, count], grow),
            (Operation::TryPush, &[exclusive, value], grow),
            (Operation::TryExtend, &[exclusive, elements], grow),
            (Operation::TryResize, &[exclusive, count, value], grow),
            (
                Operation::Truncate,
                &[exclusive, count],
                ValueTy::Scalar(hir::Ty::Unit),
            ),
            (Operation::Take, &[exclusive], vector),
        ];
        for (operation, parameters, result) in expected {
            let function = Function::new(operation, hir::Ty::I64).unwrap();
            let signature = function.signature(EnumId(23));
            assert_eq!(signature.parameters(), parameters, "{operation:?}");
            assert_eq!(signature.result, result, "{operation:?}");
            assert_eq!(function.result(EnumId(23)), result, "{operation:?}");
        }
    }

    #[test]
    fn scalar_admission_includes_all_seventeen_existing_types_and_unit_values() {
        let scalars = [
            hir::Ty::Bool,
            hir::Ty::I32,
            hir::Ty::U8,
            hir::Ty::Unit,
            hir::Ty::I64,
            hir::Ty::I8,
            hir::Ty::I16,
            hir::Ty::U16,
            hir::Ty::U32,
            hir::Ty::U64,
            hir::Ty::Isize,
            hir::Ty::Usize,
            hir::Ty::I128,
            hir::Ty::U128,
            hir::Ty::F32,
            hir::Ty::F64,
            hir::Ty::Char,
        ];
        assert_eq!(scalars.len(), 17);
        for element in scalars {
            assert!(scalar_admitted(element));
            assert_eq!(
                Function::new(Operation::New, element)
                    .unwrap()
                    .signature(EnumId(23))
                    .result,
                ValueTy::Owned(AggregateTy::Vector(element)),
            );
        }
        let unit_push = Function::new(Operation::TryPush, hir::Ty::Unit)
            .unwrap()
            .signature(EnumId(23));
        assert_eq!(
            unit_push.parameters(),
            &[
                ParameterTy::Reference {
                    referent: BorrowedTy::Exact(AggregateTy::Vector(hir::Ty::Unit)),
                    kind: BorrowKind::Exclusive,
                },
                ParameterTy::Value(ValueTy::Scalar(hir::Ty::Unit)),
            ]
        );
        assert_eq!(
            unit_push.result,
            ValueTy::Owned(AggregateTy::Enum(EnumId(23)))
        );
    }

    #[test]
    fn specialization_keys_preserve_element_type_and_canonical_status_identity() {
        let bytes = Function::new(Operation::TryResize, hir::Ty::U8).unwrap();
        let wide = Function::new(Operation::TryResize, hir::Ty::U128).unwrap();
        assert_ne!(bytes, wide);
        assert_eq!(
            bytes.signature(EnumId(91)).result,
            ValueTy::Owned(AggregateTy::Enum(EnumId(91)))
        );
        assert_eq!(
            wide.signature(EnumId(91)).result,
            ValueTy::Owned(AggregateTy::Enum(EnumId(91)))
        );
        assert_eq!(
            wide.signature(EnumId(91)).parameters()[2],
            ParameterTy::Value(ValueTy::Scalar(hir::Ty::U128))
        );
    }
}
