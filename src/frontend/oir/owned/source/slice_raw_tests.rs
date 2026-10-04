//! Raw verifier controls for borrowed views; source typing is not authority.
use super::super::*;
use super::{lower, resolve, typeck};
use crate::frontend::{lexer, parser};

fn raw(text: &str) -> (SourceMap, RawOwnedProgram) {
    let mut sources = SourceMap::new();
    let id = sources.add("slice-raw.ox".into(), text.into());
    let file = sources.get(id);
    let (ast, _) = parser::parse_counted_with_arrays(
        file,
        lexer::lex(file).unwrap(),
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut crate::frontend::project::budget::Allocator::default(),
        parser::ArraySyntaxPolicy::Enabled,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve_in_map(file, &ast, &sources).unwrap()).unwrap();
    let raw = lower::lower(&typed).unwrap();
    (sources, raw)
}
const SHARED: &str =
    "fn len(xs:&[i32])->i32{return xs.len();} fn main()->i32{let a=[1,2];return len(&a);}";

#[test]
fn slice_raw_target_view_and_source_authority_are_independently_checked() {
    let (sources, original) = raw(SHARED);
    let witness = verified::verify_owned(original, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(1))).unwrap(),
        Scalar::I32(2)
    );
    for changed in 0..4 {
        let (sources, mut program) = raw(SHARED);
        let other = BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::Bool)).unwrap();
        match changed {
            0 => program.functions[1].loans[0].referent = other,
            1 => program.functions[0].references[0].referent = other,
            2 => {
                // Matching loan and callee still cannot change the owner's element type.
                program.functions[1].loans[0].referent = other;
                program.functions[0].references[0].referent = other;
            }
            _ => {
                let invalid =
                    BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(99))))
                        .unwrap();
                program.functions[1].loans[0].referent = invalid;
                program.functions[0].references[0].referent = invalid;
            }
        }
        assert!(
            verified::verify_owned(program, &sources).is_err(),
            "mutation {changed}"
        );
    }
}

#[test]
fn slice_raw_cannot_narrow_a_slice_authority_back_to_fixed() {
    let text = "fn len(xs:&[i32])->i32{return xs.len();} fn relay(xs:&[i32])->i32{return len(&*xs);} fn main()->i32{let a=[1,2];return relay(&a);}";
    let (sources, mut program) = raw(text);
    let fixed = BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::FixedArray(
        FixedArrayTy::check(hir::Ty::I32, 2).unwrap(),
    )))
    .unwrap();
    program.functions[0].references[0].referent = fixed;
    program.functions[1].loans[0].referent = fixed;
    assert!(verified::verify_owned(program, &sources).is_err());
}

#[test]
fn slice_native_sidecars_are_accounted_without_runtime_handle_growth() {
    let (sources, program) = raw(SHARED);
    let witness = verified::verify_owned(program, &sources).unwrap();
    let slices = plan::ExecutionPlan::build(&witness).unwrap();
    let (exact_sources, exact_program) = raw(&SHARED.replace("&[i32]", "&[i32;2]"));
    let exact_witness = verified::verify_owned(exact_program, &exact_sources).unwrap();
    let exact = plan::ExecutionPlan::build(&exact_witness).unwrap();
    assert_eq!(std::mem::size_of::<storage::ReferenceHandle>(), 64);
    assert_eq!(
        std::mem::size_of::<BorrowedSlot>(),
        std::mem::size_of::<AggregateSlot>()
    );
    for i in 0..2 {
        let dynamic = slices.function(hir::DefId(i)).usage();
        let fixed = exact.function(hir::DefId(i)).usage();
        assert_eq!(dynamic.expanded_cells, fixed.expanded_cells);
        assert_eq!(dynamic.reference_bytes, fixed.reference_bytes);
        assert_eq!(dynamic.native_bytes, fixed.native_bytes + 4);
    }
    assert_eq!(slices.metadata_bytes(), exact.metadata_bytes());
}
