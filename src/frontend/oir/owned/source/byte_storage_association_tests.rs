//! Explicit relocation inventory for RFC0031 source-authority tightening.
//! These conservative transport roles are not stack/RSS measurements.
use super::*;
use crate::frontend::{declaration_index::DeclarationIndex, source::SourceView};
use std::mem::{align_of, size_of};

type Typed = super::super::typeck::TypedOwnedProgram<'static>;
type DiagnosticBox = Box<Diagnostic>;
type DiagnosticVec = Vec<Diagnostic>;
type OwnedSource = AssociatedOwned<'static>;

// Common roles coexist conservatively across the caller/callee boundary. The
// raw value argument below is additionally charged in the predecessor, even
// though a compiler might elide its move. No such elision is assumed here.
#[allow(dead_code)]
struct SharedRoles {
    program_typed: &'static Typed,
    program_index: &'static DeclarationIndex<'static>,
    program_map: SourceView<'static>,
    program_sources: &'static SourceMap,
    lower_argument: &'static Typed,
    lower_return: Result<RawOwnedProgram, OwnedFailure>,
    raw_caller: RawOwnedProgram,
    lower_mapping_source: &'static SourceMap,
    lower_mapping_error: OwnedFailure,
    lower_diagnostic_source: &'static SourceMap,
    lower_diagnostic_error: &'static OwnedFailure,
    lower_diagnostic_box: DiagnosticBox,
    association_typed: &'static Typed,
    association_index: &'static DeclarationIndex<'static>,
    association_map: SourceView<'static>,
    association_sources: &'static SourceMap,
    provenance_return: Result<(), DiagnosticBox>,
    association_check_return: Result<BindUsage, DiagnosticBox>,
    associated_construction: OwnedSource,
    associated_return: Result<OwnedSource, DiagnosticBox>,
    normalized_associated_return: Result<OwnedSource, DiagnosticVec>,
    associated_caller: OwnedSource,
    associated_error_box: DiagnosticBox,
    associated_error_payload: Diagnostic,
    associated_error_vec: DiagnosticVec,
}
#[allow(dead_code)]
struct PredecessorRoles {
    shared: SharedRoles,
    normalized_lower: Result<RawOwnedProgram, DiagnosticVec>,
    lower_error_payload: Diagnostic,
    lower_error_vec: DiagnosticVec,
    raw_association_argument: RawOwnedProgram,
}
#[allow(dead_code)]
struct CurrentRoles {
    shared: SharedRoles,
    normalized_lower: Result<RawOwnedProgram, DiagnosticBox>,
}

#[test]
fn byte_storage_source_authority_relocated_carriers_are_dominated() {
    fn report<T>(name: &str) {
        println!(
            "BYTE_STORAGE_AUTH_ROLE {name} size={} align={}",
            size_of::<T>(),
            align_of::<T>()
        );
    }
    report::<RawOwnedProgram>("raw_owner");
    report::<OwnedFailure>("owned_failure");
    report::<Result<RawOwnedProgram, OwnedFailure>>("lower_return");
    report::<Result<RawOwnedProgram, DiagnosticVec>>("old_normalized_lower");
    report::<Result<RawOwnedProgram, DiagnosticBox>>("new_normalized_lower");
    report::<(&SourceMap, &OwnedFailure)>("mapping_borrow_arguments");
    report::<DiagnosticBox>("diagnostic_box");
    report::<DiagnosticVec>("diagnostic_vector");
    report::<OwnedSource>("associated_owner");
    report::<Result<OwnedSource, DiagnosticBox>>("associated_box_return");
    report::<Result<OwnedSource, DiagnosticVec>>("associated_vector_return");
    report::<SharedRoles>("shared_complete_roles");
    report::<PredecessorRoles>("old_complete_relocation_envelope");
    report::<CurrentRoles>("new_complete_relocation_envelope");
    assert!(size_of::<CurrentRoles>() <= size_of::<PredecessorRoles>());
    assert_eq!(align_of::<CurrentRoles>(), align_of::<PredecessorRoles>());
    // No retained rows, tracker, vectors, SourceMap or typed-owner copies were
    // added. The raw error Vec normalization moved to the already-existing
    // outer AssociatedOwned error normalization; source errors stay identical.
}
