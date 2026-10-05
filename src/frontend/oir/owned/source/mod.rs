//! Owned-source producer. The raw verifier alone certifies ownership.
mod association;
mod budget;
pub(super) mod hir;
mod hir_budget;
pub(super) mod lower;
mod program;
pub(in crate::frontend::oir) mod resolve;
#[cfg(test)]
mod tests;
pub(in crate::frontend::oir) mod typeck;
pub(in crate::frontend::oir) use program::{check_typed, SourceProgram};
mod diagnostic;

#[cfg(test)]
mod diagnostic_tests;

#[cfg(test)]
mod budget_tests;
#[cfg(test)]
mod lower_tests;

#[cfg(test)]
mod reviewer_heldout;

#[cfg(test)]
pub(super) mod reviewer_source;

#[cfg(test)]
pub(in crate::frontend::oir::owned) mod resource_fixtures;

#[cfg(test)]
mod array_types_tests;

#[cfg(test)]
mod array_type_controls;

#[cfg(test)]
mod array_pipeline;

#[cfg(test)]
mod array_consumer_tests;

#[cfg(test)]
mod slice_raw_tests;
#[cfg(test)]
mod slice_tests;

#[cfg(test)]
mod projected_slice_raw_tests;
