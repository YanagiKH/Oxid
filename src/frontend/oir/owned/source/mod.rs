//! Owned-source producer. The raw verifier alone certifies ownership.
pub(super) mod association;
mod budget;
mod builtin_lower;
pub(super) mod hir;
mod hir_budget;
pub(super) mod lower;
mod program;
pub(in crate::frontend::oir) mod resolve;
mod resolver_storage;
#[cfg(test)]
mod tests;
mod type_storage;
pub(in crate::frontend::oir) mod typeck;
pub(in crate::frontend::oir) use program::{check_enum_source, check_typed, SourceProgram};
mod diagnostic;

/// Test subprocesses reuse the authoritative denial presentation. This exposes
/// no source producer, owner, proof or execution capability.
#[cfg(test)]
pub(super) fn raw_verification_diagnostic(
    error: &super::OwnedFailure,
    sources: &super::SourceMap,
) -> Box<super::Diagnostic> {
    diagnostic::verify(error, sources)
}

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

#[cfg(test)]
mod builtin_source_tests;
#[cfg(test)]
mod enum_native_source_tests;

#[cfg(test)]
mod output_source_tests;

#[cfg(test)]
pub(super) mod u8_tests;

mod u8_resources;
