//! Packed standard-library operations for termination notifications and time.
use super::{experimental, Ty};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(in crate::frontend) enum Function {
    Enable,
    Requested,
    Wait,
    MonotonicMillis,
    SleepMillis,
    UnixMillis,
}
impl Function {
    pub const SIGNALS: [Self; 3] = [Self::Enable, Self::Requested, Self::Wait];
    pub const ALL: [Self; 6] = [
        Self::Enable,
        Self::Requested,
        Self::Wait,
        Self::MonotonicMillis,
        Self::SleepMillis,
        Self::UnixMillis,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Enable => "enable_termination_notifications",
            Self::Requested => "termination_requested",
            Self::Wait => "wait_for_termination",
            Self::MonotonicMillis => "monotonic_millis",
            Self::SleepMillis => "sleep_millis",
            Self::UnixMillis => "unix_millis",
        }
    }
    pub fn path(self) -> [&'static str; 3] {
        let module = match self {
            Self::MonotonicMillis | Self::SleepMillis | Self::UnixMillis => "time",
            _ => "process",
        };
        ["std", module, self.name()]
    }
    pub fn takes_millis(self) -> bool {
        matches!(self, Self::Wait | Self::SleepMillis)
    }
    pub fn result_ty(self) -> Ty {
        match self {
            Self::MonotonicMillis | Self::UnixMillis => Ty::I64,
            _ => Ty::Bool,
        }
    }
    pub fn operation(self) -> experimental::Function {
        match self {
            Self::Enable => experimental::Function::EnableTermination,
            Self::Requested => experimental::Function::TerminationRequested,
            Self::Wait => experimental::Function::WaitForTermination,
            Self::MonotonicMillis => experimental::Function::MonotonicMillis,
            Self::SleepMillis => experimental::Function::SleepMillis,
            Self::UnixMillis => experimental::Function::UnixMillis,
        }
    }
    pub fn signature(self) -> experimental::Signature {
        self.operation()
            .signature(|_| unreachable!("process functions have no enum dependency"))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::frontend) struct Inventory(u8);
impl Inventory {
    pub(super) const MASK: u8 = (1 << Function::ALL.len()) - 1;
    // Only the enclosing sealed packed inventory can construct these bits.
    pub(super) fn from_bits(bits: u8) -> Self {
        debug_assert!(bits & !Self::MASK == 0);
        Self(bits & Self::MASK)
    }
    pub fn contains(self, item: Function) -> bool {
        self.0 & (1 << item as u8) != 0
    }
    pub fn count(self) -> usize {
        self.0.count_ones() as usize
    }
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub fn has_signals(self) -> bool {
        self.0 & ((1 << Function::SIGNALS.len()) - 1) != 0
    }
    pub fn rank(self, item: Function) -> Option<usize> {
        self.contains(item).then(|| {
            let lower = (1u8 << item as u8) - 1;
            (self.0 & lower).count_ones() as usize
        })
    }
    pub fn at(self, rank: usize) -> Option<Function> {
        Function::ALL
            .into_iter()
            .filter(|item| self.contains(*item))
            .nth(rank)
    }
}

// These getters/classifiers are reached while deciding that the family is
// absent too. Price their actual construction/return/caller roles separately
// from the conditional catalogs. No historical bank is treated as spare space.
#[allow(dead_code)]
struct SharedSelectorCarriers {
    set_process_receiver: super::BuiltinSet,
    experimental_caller: experimental::Inventory,
    packed_process_receiver: experimental::Inventory,
    packed_word_caller: u64,
    masked_bits: u8,
    construction_argument: u8,
    constructed_inventory: Inventory,
    packed_process_return: Inventory,
    set_process_return: Inventory,
    caller_inventory: Inventory,
    empty_receiver: Inventory,
    empty_return: bool,
    count_receiver: Inventory,
    count_ones_return: u32,
    count_return: usize,
    operation_receiver: experimental::Function,
    operation_construction: Option<Function>,
    operation_return: Option<Function>,
    operation_caller: Option<Function>,
    selector_return: bool,
    selector_caller: bool,
    surcharge: usize,
    checked_surcharge_return: Option<usize>,
    surcharge_result: Result<usize, Box<super::super::diagnostic::Diagnostic>>,
    surcharge_caller: Result<usize, Box<super::super::diagnostic::Diagnostic>>,
}
pub(in crate::frontend) const fn shared_selector_bytes() -> usize {
    std::mem::size_of::<SharedSelectorCarriers>()
}

// Separately prepaid new-family iteration, lookup, signature, count, and return
// roles. Historical ALL28/ALL20 declarations and carrier definitions stay intact.
#[allow(dead_code)]
struct CompilerCarriers {
    catalog_inventory: [super::BuiltinSet; 4],
    inventory: [Inventory; 6],
    selected: [Option<Function>; 8],
    endpoint_catalog: std::array::IntoIter<Function, { Function::ALL.len() }>,
    hir_catalog: std::array::IntoIter<Function, { Function::ALL.len() }>,
    resolver_catalog: std::array::IntoIter<Function, { Function::ALL.len() }>,
    signature_check_catalog: std::array::IntoIter<Function, { Function::ALL.len() }>,
    raw_budget_catalog: std::array::IntoIter<Function, { Function::ALL.len() }>,
    raw_lower_catalog: std::array::IntoIter<Function, { Function::ALL.len() }>,
    association_catalog: std::array::IntoIter<Function, { Function::ALL.len() }>,
    descriptor_check_catalog: std::array::IntoIter<Function, { Function::ALL.len() }>,
    source_entry_catalog: std::array::IntoIter<Function, { Function::ALL.len() }>,
    native_entry_catalog: std::array::IntoIter<Function, { Function::ALL.len() }>,
    vm_entry_catalog: std::array::IntoIter<experimental::Function, { Function::ALL.len() }>,
    path_iterator: std::array::IntoIter<&'static str, 3>,
    path_zip: std::iter::Zip<
        std::array::IntoIter<&'static str, 3>,
        std::slice::Iter<'static, super::super::source::Span>,
    >,
    builtin_arguments_and_returns: [super::BuiltinItem; 6],
    builtin_endpoint_return: Result<super::BuiltinItem, Box<super::super::diagnostic::Diagnostic>>,
    process_endpoint_return: Result<Function, Box<super::super::diagnostic::Diagnostic>>,
    signatures: [experimental::Signature; 4],
    canonical_signature_returns:
        [Result<experimental::Signature, Box<super::super::diagnostic::Diagnostic>>; 3],
    ids: [Option<super::super::hir::DefId>; 6],
    counts: [usize; 8],
    anchors: [super::super::source::Span; 6],
    import_ordinals: std::ops::Range<usize>,
    path_words: [&'static str; 3],
    path_comparisons: [std::cmp::Ordering; 3],
    scalar_type: Ty,
}
pub(in crate::frontend) const fn compiler_carrier_bytes() -> usize {
    std::mem::size_of::<CompilerCarriers>()
        + super::super::declaration_index::process_lookup_carrier_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::builtin_catalog::{BuiltinItem, BuiltinSet};
    #[test]
    fn process_family_catalog_and_prepaid_roles_have_compiled_layout_receipts() {
        println!(
            "process family compiler_bank={} item={} operation={} builtin_item={} builtin_set={}",
            compiler_carrier_bytes(),
            std::mem::size_of::<experimental::Item>(),
            std::mem::size_of::<experimental::Function>(),
            std::mem::size_of::<BuiltinItem>(),
            std::mem::size_of::<BuiltinSet>()
        );
        assert_eq!(std::mem::size_of::<experimental::Item>(), 1);
        assert_eq!(std::mem::size_of::<Inventory>(), 1);
    }
    #[test]
    fn process_items_are_a_separate_authority() {
        assert_eq!(experimental::Item::ALL.len(), 28);
        assert_eq!(experimental::Function::ALL.len(), 20);
        let empty = BuiltinSet::None;
        for item in Function::ALL {
            let selected = empty.admit(BuiltinItem::Process(item));
            assert_eq!(selected.process().count(), 1);
            assert_eq!(selected.process().at(0), Some(item));
            assert_eq!(selected.process().at(1), None);
            assert_eq!(selected.process().rank(item), Some(0));
            assert!(!selected.experimental().contains(item.operation().item()));
            assert_eq!(selected.experimental_function_rank(item.operation()), None);
            assert_eq!(
                empty.admit(BuiltinItem::Experimental(item.operation().item())),
                empty
            );
        }
        let both = empty
            .admit(BuiltinItem::Process(Function::Enable))
            .admit(BuiltinItem::Process(Function::Requested));
        assert_eq!(both.process().count(), 2);
        assert_eq!(both.process().at(0), Some(Function::Enable));
        assert_eq!(both.process().at(1), Some(Function::Requested));
        assert_eq!(both.process().at(2), None);
        assert_eq!(both.extra_functions(), 0);
        for old in experimental::Item::ALL {
            let old_set = empty.admit(BuiltinItem::Experimental(old));
            let mixed = old_set.union(both);
            assert_eq!(
                mixed.experimental().contains(old),
                old_set.experimental().contains(old)
            );
            for operation in experimental::Function::ALL {
                assert_eq!(
                    mixed.experimental_function_rank(operation),
                    old_set.experimental_function_rank(operation)
                );
            }
            assert_eq!(old_set.process().count(), 0);
        }
        for old in super::super::vector::Item::ALL {
            let old_set = empty.admit(BuiltinItem::Vector(old));
            assert_eq!(old_set.process().count(), 0);
            let mixed = old_set.union(both);
            for selected in super::super::vector::Item::ALL {
                assert_eq!(
                    old_set.contains_vector(selected),
                    mixed.contains_vector(selected)
                );
            }
        }
        for old in super::super::owned_vector::Item::ALL {
            let item = super::super::vector::Item::from_owned(old);
            let selected = empty.admit(BuiltinItem::Vector(item));
            let mixed = selected.union(both);
            assert_eq!(selected.process().count(), 0);
            assert!(mixed.contains_vector(item));
            assert_eq!(mixed.process().count(), 2);
            assert_eq!(
                selected.experimental().owned_vector_templates(),
                mixed.experimental().owned_vector_templates()
            );
        }
        assert_eq!(both.extra_enums(), 0);
        assert_eq!(both.vector_templates(), 0);
        assert_eq!(both.experimental().owned_vector_templates(), 0);
        assert!(!both.experimental().contains(experimental::Item::ByteBuf));
        assert!(!both.experimental().contains(experimental::Item::File));
    }
}
