//! Owned-source producer. The raw verifier alone certifies ownership.
mod budget;
pub(super) mod hir;
pub(super) mod lower;
mod program;
pub(super) mod resolve;
#[cfg(test)]
mod tests;
pub(super) mod typeck;
pub(in crate::frontend::oir) use program::{check_source, SourceProgram};
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
