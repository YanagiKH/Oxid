//! Source/signature/association controls only; no runtime or native qualification.
use super::super::*;
use super::{association, budget, hir_budget, lower, resolve};
use crate::frontend::{
    builtin_catalog::vector,
    declaration_index::{self as index, IndexLimits, SourceOwner, WorkMeter},
    project::{budget::Allocator, ProjectLimits, ProjectSources},
    source::SourceMap,
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
            "oxid-vector-source-{}-{}",
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

fn with_typed(text: &str, inspect: impl FnOnce(&super::typeck::TypedOwnedProgram<'_>, &SourceMap)) {
    let fixture = Fixture::new(text);
    let project = fixture.project();
    let mut allocator = Allocator::default();
    let owner = SourceOwner::project(&project);
    let work = WorkMeter::default();
    let index = index::collect_originals(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
    let typed = resolve::type_enum_source(&index, &work, &mut allocator).unwrap();
    inspect(&typed, project.sources());
}

const SOURCE: &str = "use alloc::vec::new;use alloc::vec::len;use alloc::vec::capacity;fn identity<T>(value:T)->T{return value;}fn main()->i32{let values=new::<i32>();identity(true);return identity(len(&values)+capacity(&values));}";

fn assert_association_denied(raw: RawOwnedProgram, typed: &super::typeck::TypedOwnedProgram<'_>) {
    assert_eq!(
        association::associate(raw, typed)
            .err()
            .expect("source mutation accepted")
            .code,
        "E0500"
    );
}

fn source_errors(text: &str) -> Vec<Diagnostic> {
    let fixture = Fixture::new(text);
    let project = fixture.project();
    let mut allocator = Allocator::default();
    let owner = SourceOwner::project(&project);
    let work = WorkMeter::default();
    let facts =
        index::collect_originals(owner, IndexLimits::default(), &work, &mut allocator).unwrap();
    let index = match facts.finish(&work, &mut allocator) {
        Ok(index) => index,
        Err(errors) => return errors,
    };
    resolve::type_enum_source(&index, &work, &mut allocator).expect_err("invalid source accepted")
}

#[test]
fn only_actual_keys_add_signatures_and_canonical_suffix_rows() {
    with_typed(SOURCE, |typed, sources| {
        let index = typed.index();
        let plan = hir_budget::preflight_current_hir(index, typed.work())
            .unwrap()
            .unwrap();
        assert_eq!(plan.counts.functions, 3); // main, identity<bool>, identity<i32>
        assert_eq!(plan.counts.signatures, 6); // U=3, B=0, S=3
        assert_eq!(typed.functions().len(), 3);
        assert_eq!(typed.signatures().len(), 6);
        let expected = [
            vector::Function::new(vector::Operation::New, hir::Ty::I32).unwrap(),
            vector::Function::new(vector::Operation::Len, hir::Ty::I32).unwrap(),
            vector::Function::new(vector::Operation::Capacity, hir::Ty::I32).unwrap(),
        ];
        assert_eq!(index.vector_functions(), &expected);
        let raw = lower::lower(typed).unwrap();
        assert_eq!(raw.vector_functions, expected);
        assert_eq!(raw.functions.len(), 6);
        assert_eq!(raw.vector_functions.capacity(), 3);
        for (rank, key) in expected.into_iter().enumerate() {
            let id = index.vector_function_id(key).unwrap();
            assert_eq!(id, hir::DefId(3 + rank));
            let signature = &typed.signatures()[id.0];
            let canonical = index.vector_signature(key).unwrap();
            assert_eq!(signature.params, canonical.parameters());
            assert_eq!(signature.result, canonical.result);
            assert_eq!(signature.span, index.vector_anchor(key.item()).unwrap());
            assert_eq!(raw.functions[id.0].span, signature.span);
        }
        assert_eq!(
            super::super::budget::preflight(&raw, super::super::budget::Limits::DEFAULT).unwrap(),
            budget::preflight(typed, budget::Limits::DEFAULT)
                .unwrap()
                .analysis
        );
        assert!(association::associate(raw, typed).is_ok());
        let _ = sources;
    });
}

#[test]
fn unused_vector_templates_do_not_allocate_a_concrete_suffix() {
    with_typed(
        "use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::len;use alloc::vec::capacity;use alloc::vec::try_reserve;use alloc::vec::try_push;use alloc::vec::try_extend;use alloc::vec::try_resize;use alloc::vec::truncate;use alloc::vec::take;fn main()->i32{return 0;}",
        |typed, _| {
            assert_eq!(typed.index().builtin_set().vector_templates(), 9);
            assert!(typed.index().vector_functions().is_empty());
            assert_eq!(typed.functions().len(), 1);
            assert_eq!(typed.signatures().len(), 1);
            let raw = lower::lower(typed).unwrap();
            assert!(raw.vector_functions.is_empty());
            assert_eq!(raw.vector_functions.capacity(), 0);
            assert_eq!(raw.functions.len(), 1);
            assert!(association::associate(raw, typed).is_ok());
        },
    );
}

#[test]
fn executable_suffix_uses_discovery_rank_instead_of_catalog_template_rank() {
    with_typed(
        "use alloc::vec::new;use alloc::vec::len;use alloc::vec::capacity;fn main()->i32{let values=new::<i32>();capacity(&values);return len(&values);}",
        |typed, _| {
            let expected = [
                vector::Function::new(vector::Operation::New, hir::Ty::I32).unwrap(),
                vector::Function::new(vector::Operation::Capacity, hir::Ty::I32).unwrap(),
                vector::Function::new(vector::Operation::Len, hir::Ty::I32).unwrap(),
            ];
            assert_eq!(typed.index().vector_functions(), &expected);
            let raw = lower::lower(typed).unwrap();
            for (rank, key) in expected.into_iter().enumerate() {
                assert_eq!(typed.index().vector_function_id(key).unwrap(), hir::DefId(1 + rank));
            }
            assert!(association::associate(raw, typed).is_ok());
        },
    );
}

#[test]
fn same_signature_key_swaps_and_call_swaps_do_not_gain_source_authority() {
    with_typed(SOURCE, |typed, _| {
        let index = typed.index();
        let len = vector::Function::new(vector::Operation::Len, hir::Ty::I32).unwrap();
        let capacity = vector::Function::new(vector::Operation::Capacity, hir::Ty::I32).unwrap();
        let len_id = index.vector_function_id(len).unwrap();
        let capacity_id = index.vector_function_id(capacity).unwrap();
        // These two signatures really are identical, including the shared family anchor.
        assert_eq!(
            index.vector_signature(len).unwrap(),
            index.vector_signature(capacity).unwrap()
        );
        let mut raw = lower::lower(typed).unwrap();
        raw.vector_functions.swap(1, 2);
        for (id, key) in [(len_id, capacity), (capacity_id, len)] {
            for statement in &mut raw.functions[id.0].blocks[0].statements {
                if let OwnedInstruction::Vector { operation, .. } = &mut statement.kind {
                    *operation = key;
                }
            }
        }
        // Coherent raw descriptors remain inert and may pass standalone checking.
        builtins::check_candidate(&raw).unwrap();
        assert_association_denied(raw, typed);
        let mut raw = lower::lower(typed).unwrap();
        let call = raw.functions[0]
            .calls
            .iter_mut()
            .find(|call| call.target == capacity_id)
            .unwrap();
        call.target = len_id;
        builtins::check_candidate(&raw).unwrap();
        assert_association_denied(raw, typed);
    });
}

#[test]
fn vector_signature_and_body_suffix_follows_the_existing_fixed_builtins() {
    with_typed(
        "use alloc::bytes::new as byte_new;use alloc::vec::new as vector_new;use alloc::vec::len;fn main()->i32{let bytes=byte_new();let values=vector_new::<i32>();return len(&values);}",
        |typed, _| {
            assert_eq!(typed.index().source_function_count(), 1);
            assert_eq!(typed.index().builtin_set().extra_functions(), 1);
            assert_eq!(typed.index().vector_functions().len(), 2);
            assert_eq!(typed.signatures().len(), 4);
            let raw = lower::lower(typed).unwrap();
            let ids = builtins::check_candidate(&raw).unwrap();
            assert_eq!(ids.experimental_function(crate::frontend::builtin_catalog::experimental::Function::New), Some(hir::DefId(1)));
            assert_eq!(ids.vector_function_at(0), Some(hir::DefId(2)));
            assert_eq!(ids.vector_function_at(1), Some(hir::DefId(3)));
            assert_eq!(raw.functions.len(), 4);
            assert!(association::associate(raw, typed).is_ok());
        },
    );
}

#[test]
fn imported_but_unrequested_keys_and_duplicate_claims_are_rejected() {
    with_typed(
        "use alloc::vec::new;use alloc::vec::len;use alloc::vec::capacity;fn main()->i32{let values=new::<i32>();return len(&values);}",
        |typed, _| {
            let cap = vector::Function::new(vector::Operation::Capacity, hir::Ty::I32).unwrap();
            assert!(!typed.index().vector_functions().contains(&cap));
            let mut raw = lower::lower(typed).unwrap();
            raw.vector_functions[1] = cap;
            for statement in &mut raw.functions[2].blocks[0].statements {
                if let OwnedInstruction::Vector { operation, .. } = &mut statement.kind {
                    *operation = cap;
                }
            }
            builtins::check_candidate(&raw).unwrap();
            assert_association_denied(raw, typed);
            let mut raw = lower::lower(typed).unwrap();
            raw.vector_functions[1] = raw.vector_functions[0];
            assert_association_denied(raw, typed);
            let mut raw = lower::lower(typed).unwrap();
            raw.vector_functions.push(cap);
            assert_association_denied(raw, typed);
            let mut raw = lower::lower(typed).unwrap();
            raw.vector_functions.pop();
            assert_association_denied(raw, typed);
            let mut raw = lower::lower(typed).unwrap();
            raw.vector_functions.try_reserve_exact(1).unwrap();
            assert!(raw.vector_functions.capacity() > raw.vector_functions.len());
            assert_association_denied(raw, typed);
        },
    );
}

#[test]
fn template_domain_ids_and_altered_anchors_fail_source_association() {
    with_typed(SOURCE, |typed, _| {
        let capacity = vector::Function::new(vector::Operation::Capacity, hir::Ty::I32).unwrap();
        let id = typed.index().vector_function_id(capacity).unwrap();
        assert_eq!(id, hir::DefId(5));
        let mut raw = lower::lower(typed).unwrap();
        // N=2, B=0, admitted template rank(capacity)=2 => template ID4.
        // U=3 makes that numeric ID the same-signature executable len, not capacity.
        raw.functions[0]
            .calls
            .iter_mut()
            .find(|call| call.target == id)
            .unwrap()
            .target = hir::DefId(4);
        assert_association_denied(raw, typed);
        let mut raw = lower::lower(typed).unwrap();
        let anchor = raw.functions[0].span;
        let function = &mut raw.functions[id.0];
        function.span = anchor;
        for local in &mut function.locals {
            local.span = anchor;
        }
        for owner in &mut function.owners {
            owner.span = anchor;
        }
        for reference in &mut function.references {
            reference.span = anchor;
        }
        for block in &mut function.blocks {
            block.span = anchor;
            for statement in &mut block.statements {
                statement.span = anchor;
            }
            if let Some(end) = &mut block.terminator {
                end.span = anchor;
                if let OwnedTerminatorKind::ReturnScalar(value) = &mut end.kind {
                    value.span = anchor;
                }
            }
        }
        builtins::check_candidate(&raw).unwrap();
        assert_association_denied(raw, typed);
    });
}

#[test]
fn vector_source_allocation_and_raw_byte_boundaries_stay_fallible() {
    with_typed(SOURCE, |typed, _| {
        let expected = budget::preflight(typed, budget::Limits::DEFAULT).unwrap();
        assert!(budget::preflight(
            typed,
            budget::Limits {
                raw_bytes: expected.raw_bytes - 1
            }
        )
        .is_err());
        assert_eq!(
            budget::preflight(
                typed,
                budget::Limits {
                    raw_bytes: expected.raw_bytes
                }
            )
            .unwrap(),
            expected
        );
        let mut denied = 0;
        loop {
            match budget::fail_allocation_after(denied, || lower::lower(typed)) {
                Err(error) => {
                    assert_eq!(
                        error.kind,
                        OwnedFailureKind::Resource("injected source allocation failure")
                    );
                    denied += 1;
                    assert!(denied < 300);
                }
                Ok(raw) => {
                    assert_eq!(raw.vector_functions, typed.index().vector_functions());
                    assert_eq!(raw.vector_functions.capacity(), raw.vector_functions.len());
                    break;
                }
            }
        }
        assert!(denied > 0);
    });
}

#[test]
fn all_seventeen_scalar_vector_views_and_forwarded_ranges_preserve_element_identity() {
    for spelling in [
        "()", "bool", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64",
        "u128", "usize", "f32", "f64", "char",
    ] {
        let text = format!("use alloc::vec::new;fn whole(items:&[{spelling}])->(){{return;}}fn exclusive(items:&mut [{spelling}])->(){{return;}}fn forward(items:&[{spelling}])->(){{whole(&*items[0..0]);return;}}fn main()->i32{{let mut values=new::<{spelling}>();whole(&values);whole(&values[0..0]);exclusive(&mut values[0..0]);forward(&values);return 0;}}");
        with_typed(&text, |typed, _| {
            let raw = lower::lower(typed).unwrap();
            let element = typed.index().vector_functions()[0].element();
            let mut ranges = 0;
            for function in &raw.functions[..typed.index().source_function_count()] {
                for loan in &function.loans {
                    assert_eq!(loan.referent(), BorrowedTy::ScalarSlice(element));
                }
                ranges += function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.statements)
                    .filter(|statement| {
                        matches!(statement.kind, OwnedInstruction::PrepareBorrowRange { .. })
                    })
                    .count();
            }
            assert_eq!(ranges, 3);
            assert!(association::associate(raw, typed).is_ok());
        });
    }
}

#[test]
fn more_than_the_old_fixed_catalog_count_still_creates_only_requested_signatures() {
    let mut text = String::from("use alloc::vec::new;use alloc::vec::len;fn main()->i32{");
    for (position, spelling) in [
        "()", "bool", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64",
        "u128", "usize", "f32", "f64", "char",
    ]
    .into_iter()
    .enumerate()
    {
        text.push_str(&format!("let values{position}=new::<{spelling}>();"));
        if position < 3 {
            text.push_str(&format!("len(&values{position});"));
        }
    }
    text.push_str("return 0;}");
    with_typed(&text, |typed, _| {
        let plan = hir_budget::preflight_current_hir(typed.index(), typed.work())
            .unwrap()
            .unwrap();
        assert_eq!((plan.counts.functions, plan.counts.signatures), (1, 21));
        assert_eq!(typed.index().vector_functions().len(), 20);
        let raw = lower::lower(typed).unwrap();
        assert_eq!(raw.vector_functions.len(), 20);
        assert_eq!(raw.functions.len(), 21);
        assert!(association::associate(raw, typed).is_ok());
    });
}

#[test]
fn vector_free_fixed_array_and_forwarded_slice_ranges_are_legal_for_all_scalar_elements() {
    for spelling in [
        "()", "bool", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64",
        "u128", "usize", "f32", "f64", "char",
    ] {
        let text = format!("fn whole(items:&[{spelling}])->(){{return;}}fn forward(items:&[{spelling}])->(){{whole(&*items[0..0]);return;}}fn main()->i32{{let items:[{spelling};0]=[];whole(&items[0..0]);forward(&items);return 0;}}");
        with_typed(&text, |typed, _| {
            assert!(typed.index().has_element_ranges());
            assert_eq!(typed.index().builtin_set(), BuiltinOrigins::None);
            assert!(typed.index().vector_functions().is_empty());
            assert!(typed.source_storage_bytes().is_some());
            let raw = lower::lower(typed).unwrap();
            assert!(raw.vector_functions.is_empty());
            assert_eq!(raw.vector_functions.capacity(), 0);
            assert!(association::associate(raw, typed).is_ok());
        });
    }
}

#[test]
fn projected_fixed_array_ranges_preserve_typed_elements_without_vector_imports() {
    let text = "struct Holder{items:[i128;0]}fn whole(items:&[i128])->(){return;}fn forward(holder:&Holder)->(){whole(&*holder.items[0..0]);return;}fn main()->i32{let items:[i128;0]=[];let holder=Holder{items:items};whole(&holder.items[0..0]);forward(&holder);return 0;}";
    with_typed(text, |typed, _| {
        assert_eq!(typed.index().builtin_set(), BuiltinOrigins::None);
        let raw = lower::lower(typed).unwrap();
        assert!(raw
            .functions
            .iter()
            .flat_map(|function| &function.loans)
            .any(|loan| !loan.projection.is_empty()
                && loan.referent() == BorrowedTy::ScalarSlice(hir::Ty::I128)));
        assert!(association::associate(raw, typed).is_ok());
    });
}

#[test]
fn projected_wide_vector_indexing_and_ranges_keep_checked_record_paths() {
    let text = "use alloc::vec::Vec;use alloc::vec::new;struct Holder{values:Vec<f64>}fn use_slice(items:&[f64])->(){return;}fn set(holder:&mut Holder,value:f64)->(){holder.values[0]=value;return;}fn length(holder:&Holder)->i32{return holder.values.len();}fn read(holder:&Holder)->f64{return holder.values[0];}fn forward(holder:&Holder)->(){use_slice(&*holder.values);use_slice(&*holder.values[0..0]);return;}fn main()->i32{let mut holder=Holder{values:new::<f64>()};use_slice(&holder.values);use_slice(&holder.values[0..0]);forward(&holder);return 0;}";
    with_typed(text, |typed, _| {
        let raw = lower::lower(typed).unwrap();
        assert!(raw.functions[..typed.index().source_function_count()]
            .iter()
            .flat_map(|function| &function.loans)
            .any(|loan| !loan.projection.is_empty()
                && loan.referent() == BorrowedTy::ScalarSlice(hir::Ty::F64)));
        assert!(association::associate(raw, typed).is_ok());
    });
}

#[test]
fn source_lookup_work_refuses_exhaustion_and_overflow_before_lookup() {
    let span = Span {
        file: crate::frontend::source::SourceFileId(0),
        start: 0,
        end: 1,
    };
    let exact = WorkMeter::new(6);
    super::vector_source::lookup_work(&exact, 2, 2, span).unwrap();
    assert_eq!(exact.used(), 6);
    assert_eq!(
        super::vector_source::lookup_work(&WorkMeter::new(5), 2, 2, span)
            .unwrap_err()
            .code,
        "E0400"
    );
    assert_eq!(
        super::vector_source::lookup_work(&WorkMeter::default(), usize::MAX, 1, span)
            .unwrap_err()
            .code,
        "E0400"
    );
}

#[test]
fn typed_ranges_keep_exact_element_and_i32_bound_requirements() {
    for text in [
        "fn whole(items:&[i32])->(){return;}fn main()->i32{let items:[f64;0]=[];whole(&items[0..0]);return 0;}",
        "fn whole(items:&[f64])->(){return;}fn main()->i32{let items:[f64;0]=[];whole(&items[0..true]);return 0;}",
        "fn whole(items:&[i32])->(){return;}fn main()->i32{let item=1;whole(&item[0..0]);return 0;}",
        "use alloc::bytes::new;fn whole(items:&[f64])->(){return;}fn main()->i32{let items=new();whole(&items[0..0]);return 0;}",
    ] {
        let errors = source_errors(text);
        assert_eq!(errors[0].code, "E0300", "{text}: {errors:?}");
    }
}
