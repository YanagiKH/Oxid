//! Test-only native fixture retaining its original source map. This enters the
//! same current source association as SourceProgram; no raw permit is returned.
use super::super::*;
use super::{association, byte_storage_tests, resolve, typeck};
use crate::frontend::source::SourceFileId;

pub(in crate::frontend::oir::owned) fn checked(
    text: &str,
) -> (SourceMap, verified::VerifiedOwnedProgram, hir::DefId) {
    let (sources, ast) = byte_storage_tests::parsed(text);
    let typed = typeck::check(
        resolve::resolve_in_map(sources.get(SourceFileId(0)), &ast, &sources).unwrap(),
    )
    .unwrap();
    let entry = typed.entry().unwrap();
    let associated = association::lower_and_associate(&typed).unwrap();
    let witness = verified::verify_associated(associated).unwrap();
    (sources, witness, entry)
}
