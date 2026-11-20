//! Shared termination selectors are paid even when the family is absent.
//! Catalog traversal remains conditional; these are actual borrowed/query and
//! accounting transports, not a second raw program or another plan allocation.
use super::{
    plan::{AdmissionFailure, ExecutionPlan},
    verified::PlanWitness,
    *,
};
use std::mem::size_of;

#[allow(dead_code)]
struct RawSelectors<'a> {
    raw_return: &'a RawOwnedProgram,
    raw_caller: &'a RawOwnedProgram,
    entry_selection: bool,
    metadata_surcharge: usize,
    metadata_addition: Result<usize, OwnedFailure>,
    metadata_caller: Result<usize, OwnedFailure>,
    native_addition: Result<usize, Box<Diagnostic>>,
    native_caller: Result<usize, Box<Diagnostic>>,
    #[cfg(test)]
    counter_storage: std::cell::Cell<[usize; 3]>,
    #[cfg(test)]
    counter_receiver: &'a std::cell::Cell<[usize; 3]>,
    #[cfg(test)]
    counter_read: [usize; 3],
    #[cfg(test)]
    counter_updated: [usize; 3],
    #[cfg(test)]
    counter_return: [usize; 3],
}
pub(super) const fn raw_bytes() -> usize {
    size_of::<RawSelectors<'_>>()
}

#[allow(dead_code)]
struct PlanSelectors<'a, W: PlanWitness> {
    plan_receiver: &'a ExecutionPlan<'a, W>,
    witness_return: &'a W,
    witness_caller: &'a W,
    raw_return: &'a RawOwnedProgram,
    selection_return: bool,
    selection_caller: bool,
    cost_argument: hir::DefId,
    cost_construction: Option<usize>,
    cost_return: Option<usize>,
    cost_caller: Option<usize>,
    metadata_surcharge: usize,
    metadata_addition: Result<usize, AdmissionFailure>,
    metadata_caller: Result<usize, AdmissionFailure>,
}
pub(super) const fn plan_bytes<W: PlanWitness>() -> usize {
    size_of::<PlanSelectors<'_, W>>()
}

#[cfg(test)]
thread_local! {
    static CATALOGS: std::cell::Cell<[usize; 3]> = const { std::cell::Cell::new([0, 0, 0]) };
}
#[cfg(test)]
pub(super) fn reset_catalogs() {
    CATALOGS.with(|count| count.set([0, 0, 0]));
}
#[cfg(test)]
pub(super) fn catalogs() -> [usize; 3] {
    CATALOGS.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(super) fn vm_catalog_row() {
    CATALOGS.with(|count| {
        let mut observed = count.get();
        observed[1] += 1;
        count.set(observed);
    });
}
#[cfg(test)]
pub(super) fn native_catalog_row() {
    CATALOGS.with(|count| {
        let mut observed = count.get();
        observed[2] += 1;
        count.set(observed);
    });
}
#[cfg(test)]
pub(super) fn source_catalog_row() {
    CATALOGS.with(|count| {
        let mut observed = count.get();
        observed[0] += 1;
        count.set(observed);
    });
}
