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
    let raw = lower::lower(&typed).map_err(|error| vec![*diagnostic::lower(&error, sources)])?;
    let witness = verified::verify_owned(raw, sources)
        .map_err(|error| vec![*diagnostic::verify(&error, sources)])?;
    Ok((SourceProgram { witness }, entry))
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
