//! Typed-project facades. Both depths share the source sealing leaf schedule.
#![allow(dead_code)] // Historical qualification adapters retain their private API.
use super::*;
use crate::frontend::{
    declaration_index::{IndexLimits, WorkMeter},
    project::{budget::Allocator, ProjectSources},
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
    pub(in crate::frontend::oir) fn new(
        route: ProjectRoute,
        functions: usize,
        records: usize,
        root_main: Option<hir::DefId>,
    ) -> Self {
        Self {
            route,
            functions,
            records,
            root_main,
        }
    }
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

pub(in crate::frontend) fn check_project_candidate(
    project: &ProjectSources,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<CheckedProjectTypes, Vec<Diagnostic>> {
    super::source::check_project_candidate(project, limits, work, allocator)
}

pub(in crate::frontend) fn check_project_executable<'s>(
    project: &'s ProjectSources,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<CheckedSourceProgram<'s>, Vec<Diagnostic>> {
    super::source::check_project_executable_candidate(project, limits, work, allocator)
}

/// Historical qualification adapter for the same complete source checker.
pub(in crate::frontend) fn check_project_executable_candidate<'s>(
    project: &'s ProjectSources,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<CheckedSourceProgram<'s>, Vec<Diagnostic>> {
    check_project_executable(project, limits, work, allocator)
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

#[cfg(test)]
#[path = "project_execution_tests.rs"]
mod execution_tests;
