//! Private Unit2 complete resolution/type schedule, deliberately ending before
//! lowering, ownership verification, execution or native production.
#![allow(dead_code)] // Complete private type qualification; no public activation.
use super::{owned, *};
use crate::frontend::{
    declaration_index::{self as index, IndexLimits, SourceOwner, WorkMeter},
    project::{budget::Allocator, ProjectSources, SyntaxFlavor},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum ProjectRoute {
    Scalar,
    Owned,
}

/// A structural type-check result. It is not a CheckedSourceProgram or an OIR
/// witness and provides no conversion, execution or native consumer methods.
#[derive(Debug)]
pub(in crate::frontend) struct CheckedProjectTypes {
    route: ProjectRoute,
    functions: usize,
    records: usize,
    root_main: Option<hir::DefId>,
}
impl CheckedProjectTypes {
    pub fn route(&self) -> ProjectRoute {
        self.route
    }
    pub fn functions(&self) -> usize {
        self.functions
    }
    pub fn records(&self) -> usize {
        self.records
    }
    pub fn root_main(&self) -> Option<hir::DefId> {
        self.root_main
    }
}

#[allow(dead_code)] // Private candidate driver; public loader is still closed.
pub(in crate::frontend) fn check_project_candidate(
    project: &ProjectSources,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<CheckedProjectTypes, Vec<Diagnostic>> {
    let sources = SourceOwner::project(project);
    let route = if sources.owned(work).map_err(|e| vec![*e])? {
        ProjectRoute::Owned
    } else {
        ProjectRoute::Scalar
    };
    #[cfg(test)]
    work.observe(crate::frontend::declaration_index::Observation::Route {
        owned: route == ProjectRoute::Owned,
    });
    let facts = index::collect_originals(sources, limits, work, allocator).map_err(|e| {
        work.record_error(&e);
        vec![*e]
    })?;
    // Only original scalar syntax interleaves conflicts with signatures.
    let original_signatures =
        if route == ProjectRoute::Scalar && sources.flavor() == SyntaxFlavor::OriginalSingleFile {
            Some(hir::original_signatures(&facts, work)?)
        } else {
            None
        };
    let frozen = facts.finish(work, allocator)?;
    match route {
        ProjectRoute::Scalar => {
            let resolved = match original_signatures {
                Some(signatures) => hir::resolve_bodies(&frozen, work, signatures)?,
                None => hir::resolve_project(&frozen, work)?,
            };
            work.phase("type");
            let _typed = typeck::check(resolved).inspect_err(|errors| {
                for error in errors {
                    work.record_error(error)
                }
            })?;
        }
        ProjectRoute::Owned => {
            let resolved = owned::source::resolve::resolve_project(&frozen, work)?;
            let _typed = owned::source::typeck::check(resolved)?;
        }
    }
    Ok(CheckedProjectTypes {
        route,
        functions: frozen.function_count(),
        records: frozen.record_count(),
        root_main: frozen.root_original_main(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{declaration_index::Observation, lexer, parser};
    #[test]
    fn observed_legacy_wrappers_delegate_real_signature_caps() {
        for count in [99usize, 100, 101] {
            let text = (0..count)
                .map(|i| format!("fn f{i}()->Missing{{return;}}\n"))
                .collect::<String>();
            let mut map = SourceMap::new();
            let id = map.add("legacy.ox".into(), text);
            let source = map.get(id);
            let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
            for scalar in [true, false] {
                let work = WorkMeter::default();
                work.enable_observation();
                let mut allocator = Allocator::default();
                let errors = if scalar {
                    hir::resolve_observed(source, &ast, &work, &mut allocator).unwrap_err()
                } else {
                    owned::source::resolve::resolve_observed(source, &ast, &work, &mut allocator)
                        .unwrap_err()
                };
                assert_eq!(errors.len(), count.min(100));
                assert_eq!(
                    work.observations
                        .borrow()
                        .iter()
                        .filter(|event| matches!(event, Observation::SignatureStart { .. }))
                        .count(),
                    count.min(100)
                );
            }
        }
    }
}
