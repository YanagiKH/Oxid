//! Mutation checks against the independent authenticated vector signature suffix.
use super::*;
use crate::frontend::{
    builtin_catalog::vector,
    project::{ProjectLimits, ProjectSources},
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new(text: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "oxid-vector-signatures-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("main.ox"), text).unwrap();
        Self(path)
    }
    fn project(&self) -> ProjectSources {
        ProjectSources::load_typed(
            self.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn vector_signature_identity_rejects_element_result_arity_and_anchor_mutations() {
    let text = "use alloc::vec::new;use alloc::vec::len;fn main()->i32{let values=new::<i32>();return len(&values);}";
    let fixture = Fixture::new(text);
    let project = fixture.project();
    let mut allocator = Allocator::default();
    let owner = SourceOwner::project(&project);
    let work = WorkMeter::default();
    let index = index::collect_originals(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
    let plan = super::super::hir_budget::preflight_current_hir(&index, &work)
        .unwrap()
        .unwrap();
    let mut paid = PaidStorage::new(plan.counts);
    let (records, signatures, functions) =
        resolve_index_impl(&index, &work, &mut allocator, Some(&mut paid)).unwrap();
    let mut program = ResolvedOwnedProgram {
        projection_bytes: std::cell::Cell::new(plan.total),
        admission: SourceAdmission::Executable,
        sources: SourceView::Map(project.sources()),
        index: IndexOwner::Borrowed(&index),
        work: MeterOwner::Borrowed(&work),
        records,
        signatures,
        functions,
        entry: index.root_original_main(),
    };
    program.validate_function_signatures().unwrap();
    let new = index
        .vector_function_id(vector::Function::new(vector::Operation::New, Ty::I32).unwrap())
        .unwrap();
    let len = index
        .vector_function_id(vector::Function::new(vector::Operation::Len, Ty::I32).unwrap())
        .unwrap();
    let original = program.signatures[new.0].result;
    program.signatures[new.0].result = ValueTy::Owned(AggregateTy::ByteBuf);
    assert_eq!(
        program.validate_function_signatures().unwrap_err().code,
        "E0500"
    );
    program.signatures[new.0].result = original;
    let original = program.signatures[len.0].params[0];
    program.signatures[len.0].params[0] = ParameterTy::Reference {
        referent: BorrowedTy::Exact(AggregateTy::Vector(Ty::U32)),
        kind: BorrowKind::Shared,
    };
    assert_eq!(
        program.validate_function_signatures().unwrap_err().code,
        "E0500"
    );
    program.signatures[len.0].params[0] = original;
    let original = program.signatures[len.0].result;
    program.signatures[len.0].result = ValueTy::Scalar(Ty::U32);
    assert_eq!(
        program.validate_function_signatures().unwrap_err().code,
        "E0500"
    );
    program.signatures[len.0].result = original;
    let original = program.signatures[len.0].span;
    program.signatures[len.0].span = index.sources().ast(ModuleId(0)).unwrap().functions[0].name;
    assert_eq!(
        program.validate_function_signatures().unwrap_err().code,
        "E0500"
    );
    program.signatures[len.0].span = original;
    let parameter = program.signatures[len.0].params.pop().unwrap();
    assert_eq!(
        program.validate_function_signatures().unwrap_err().code,
        "E0500"
    );
    program.signatures[len.0].params.push(parameter);
    program.validate_function_signatures().unwrap();
}
