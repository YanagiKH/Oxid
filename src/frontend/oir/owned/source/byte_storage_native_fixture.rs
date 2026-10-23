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

pub(in crate::frontend::oir::owned) const BYTE_RECORD_ENUM: &str = "struct R{byte:u8,earlier:i32,later:bool}enum E{Value(R),Empty}fn main()->i32{let n=255;let r=R{byte:n.to_u8_checked(),earlier:42,later:true};let e=E::Value(r);match e{E::Value(out)=>{let b=out.byte;return b.to_i32();},E::Empty=>{return 0;},}}";

pub(in crate::frontend::oir::owned) fn byte_record_enum(
) -> (SourceMap, verified::VerifiedOwnedProgram, hir::DefId) {
    use crate::frontend::declaration_index::{self, IndexLimits, SourceOwner, WorkMeter};
    use crate::frontend::project::budget::Allocator;
    let (sources, ast) = byte_storage_tests::parsed(BYTE_RECORD_ENUM);
    let source = sources.get(SourceFileId(0));
    let owner = SourceOwner::original(
        source,
        &ast,
        crate::frontend::source::SourceView::Map(&sources),
    )
    .unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let facts =
        declaration_index::collect_originals(owner, IndexLimits::default(), &work, &mut allocator)
            .unwrap();
    let index = facts.finish(&work, &mut allocator).unwrap();
    let typed = resolve::type_enum_source(&index, &work, &mut allocator).unwrap();
    let entry = typed.entry().unwrap();
    let witness =
        verified::verify_associated(association::lower_and_associate(&typed).unwrap()).unwrap();
    drop(typed);
    drop(index);
    (sources, witness, entry)
}
