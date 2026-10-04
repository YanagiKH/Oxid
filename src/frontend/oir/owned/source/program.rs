//! Source-only construction keeps the owned witness sealed in this child module.
use super::super::{execute, native, verified, *};
use super::{diagnostic, lower, resolve, typeck};
use crate::frontend::{ast, source::SourceFile};

#[derive(Debug)]
pub(in crate::frontend::oir) struct SourceProgram {
    witness: verified::VerifiedOwnedProgram,
}

pub(in crate::frontend::oir) fn check_source(
    source: &SourceFile,
    ast: &ast::Program,
    sources: &SourceMap,
) -> Result<(SourceProgram, Option<hir::DefId>), Vec<Diagnostic>> {
    let resolved = resolve::resolve_in_map(source, ast, sources)?;
    let typed = typeck::check(resolved)?;
    let entry = typed.entry();
    Ok((check_typed(&typed)?, entry))
}

/// The map comes from the checked typed/index owner, never an independent caller.
pub(in crate::frontend::oir) fn check_typed(
    typed: &typeck::TypedOwnedProgram<'_>,
) -> Result<SourceProgram, Vec<Diagnostic>> {
    if !typed.admission().executable() {
        return Err(vec![*crate::frontend::owned_diagnostic::diagnostic(
            "E0500",
            "oir-owned-lower",
            format_args!("internal compiler error: owned invariant violation"),
            Some(typed.index().sources().eof()),
        )]);
    }
    let index = typed.index();
    let crate::frontend::source::SourceView::Map(sources) = index.sources().view() else {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    };
    if typed.entry() != index.root_original_main() {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    let raw = lower::lower(typed).map_err(|error| vec![*diagnostic::lower(&error, sources)])?;
    super::association::check(&raw, index, sources).map_err(|error| vec![*error])?;
    let witness = verified::verify_owned(raw, sources)
        .map_err(|error| vec![*diagnostic::verify(&error, sources)])?;
    Ok(SourceProgram { witness })
}

/// Source-produced arrays can reach the real Reference consumer only through
/// this private test seam. No typed owner, raw program or witness is returned.
#[cfg(test)]
pub(super) fn run_array_source<'s>(
    owner: crate::frontend::declaration_index::SourceOwner<'s>,
    limits: crate::frontend::declaration_index::IndexLimits,
    work: &'s crate::frontend::declaration_index::WorkMeter,
    allocator: &mut crate::frontend::project::budget::Allocator,
) -> Result<Scalar, Vec<Diagnostic>> {
    let typed = typeck::check(resolve::resolve_array_consumer(
        owner, limits, work, allocator,
    )?)?;
    let index = typed.index();
    let crate::frontend::source::SourceView::Map(sources) = index.sources().view() else {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    };
    let entry = index.root_original_main();
    if typed.admission() != resolve::SourceAdmission::ArrayConsumer || typed.entry() != entry {
        return Err(vec![*crate::frontend::oir::source::association::bad()]);
    }
    let raw = lower::lower(&typed).map_err(|error| vec![*diagnostic::lower(&error, sources)])?;
    super::association::check(&raw, index, sources).map_err(|error| vec![*error])?;
    let observation = verified::probe_array_reference(
        raw,
        sources,
        super::super::budget::Limits::DEFAULT,
        entry,
        execute::Limits::default(),
        execute::ObservationControl::default(),
    )
    .map_err(|error| vec![*diagnostic::verify(&error, sources)])?;
    observation
        .result
        .map_err(|error| vec![*error.diagnostic(sources)])
}

impl SourceProgram {
    pub(in crate::frontend::oir) fn function_count(&self) -> usize {
        self.witness.functions().len()
    }

    pub(in crate::frontend::oir) fn run(
        &self,
        entry: Option<hir::DefId>,
        sources: &SourceMap,
    ) -> Result<Scalar, Box<Diagnostic>> {
        execute::run(&self.witness, entry).map_err(|error| error.diagnostic(sources))
    }

    pub(in crate::frontend::oir) fn native_module(
        &self,
        entry: Option<hir::DefId>,
        sources: &SourceMap,
    ) -> Result<String, Box<Diagnostic>> {
        native::native_module(&self.witness, entry, sources)
    }
}

#[cfg(test)]
#[path = "candidate_adapter.rs"]
mod candidate_adapter;
