//! Independent raw admission controls: source lowering is not a witness.
use super::super::*;
use super::{lower, resolve, typeck};
use crate::frontend::{lexer, parser};

const PILOT: &str = "struct Batch{tag:i32,samples:[i32;3],tail:bool}fn edit(p:&mut[i32])->i32{p[0]=p[0]+1;return p.len();}fn main()->i32{let mut b=Batch{tag:73,samples:[1,2,3],tail:true};let n=edit(&mut b.samples);if b.tag==73&&b.tail{return n+b.samples[0];}else{return 99;}}";
fn raw(text: &str) -> (SourceMap, RawOwnedProgram) {
    let mut sources = SourceMap::new();
    let id = sources.add("projected-raw.ox".into(), text.into());
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

#[test]
fn projected_slice_raw_rederives_paths_and_rejects_forged_view_types() {
    let (sources, program) = raw(PILOT);
    let witness = verified::verify_owned(program, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(1))),
        Ok(Scalar::I32(5))
    );
    for mutation in 0..9 {
        let (sources, mut program) = raw(PILOT);
        let loan = &mut program.functions[1].loans[0];
        match mutation {
            0 => loan.projection.clear(),
            1 => loan.projection[0].record = RecordId(999),
            2 => loan.projection[0].index = 999,
            3 => loan.projection[0].index = 0, // scalar metadata is not an array view
            4 => loan.projection.push(loan.projection[0]), // cannot walk through an array
            5 => {
                loan.referent = BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::Bool)).unwrap()
            }
            6 => {
                loan.referent = BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::FixedArray(
                    FixedArrayTy::check(hir::Ty::I32, 3).unwrap(),
                )))
                .unwrap()
            }
            7 => loan.projection = vec![loan.projection[0]; 65],
            _ => {
                // Matching forged formal still cannot change the actual field element.
                let other = BorrowedSlot::check(BorrowedTy::ScalarSlice(hir::Ty::Bool)).unwrap();
                loan.referent = other;
                program.functions[0].references[0].referent = other;
            }
        }
        assert!(
            verified::verify_owned(program, &sources).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn projected_slice_paths_and_retained_views_are_charged() {
    use std::mem::size_of;
    assert_eq!(size_of::<storage::BorrowView>(), 16);
    assert_eq!(size_of::<storage::ReferenceHandle>(), 80);
    assert_eq!(size_of::<storage::LoanRuntime>(), 112);
    let (sources, program) = raw(PILOT);
    let usage = budget::preflight(&program, budget::Limits::DEFAULT).unwrap();
    let path_bytes: usize = program
        .functions
        .iter()
        .flat_map(|f| &f.loans)
        .map(|l| l.projection.len() * size_of::<FieldId>())
        .sum();
    assert_eq!(path_bytes, size_of::<FieldId>());
    assert!(usage.metadata_bytes >= path_bytes);
    let witness = verified::verify_owned(program, &sources).unwrap();
    let plan = plan::ExecutionPlan::build(&witness).unwrap();
    for f in witness.functions() {
        let u = plan.function(f.id).usage();
        assert!(u.expanded_cells >= f.references.len() * 10 + f.loans.len() * 14);
        assert!(u.reference_bytes >= f.references.len() * 80 + f.loans.len() * 112);
    }
    eprintln!(
        "projected layouts LoanDecl={} BorrowView={} ReferenceHandle={} LoanRuntime={}",
        size_of::<LoanDecl>(),
        size_of::<storage::BorrowView>(),
        size_of::<storage::ReferenceHandle>(),
        size_of::<storage::LoanRuntime>()
    );
}
