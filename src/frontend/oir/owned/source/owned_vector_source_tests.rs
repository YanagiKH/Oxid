//! Private paid source typing and boundary controls. No raw/consumer execution.
use super::*;
use crate::frontend::{
    declaration_index::{self as index, IndexLimits, SourceOwner},
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
            "oxid-owned-vector-paid-source-{}-{}",
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
        if std::thread::panicking() || std::env::var_os("OXID_KEEP_TEST_PROJECTS").is_some() {
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}
const IMPORTS: &str = "use alloc::bytes::ByteBuf;use alloc::bytes::new as bytes_new;use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::len;use alloc::vec::capacity;use alloc::vec::try_reserve;use alloc::vec::push_reserved;use alloc::vec::pop_last;use alloc::vec::truncate;use alloc::vec::take;struct Entry{bytes:ByteBuf}";
fn workflow() -> String {
    format!("{IMPORTS}fn identity<T>(value:T)->T{{return value;}}fn make()->Entry{{return Entry{{bytes:bytes_new()}};}}fn inspect(items:&Vec<Entry>)->i32{{return len(&*items);}}fn main()->i32{{let mut values=new::<Entry>();try_reserve(&mut values,2);push_reserved(&mut values,identity(make()));let entry=pop_last(&mut values);push_reserved(&mut values,entry);let before=capacity(&values);let mut moved=take(&mut values);truncate(&mut moved,0);let scalars=new::<i32>();identity(true);return len(&moved)+len(&scalars)+before;}}")
}
fn candidate(project: &ProjectSources) -> DeclarationIndex<'_> {
    index::collect_owned_vector_candidate(
        SourceOwner::project(project),
        IndexLimits::default(),
        8 * 1024 * 1024,
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
    .unwrap()
}

#[test]
fn owned_vector_paid_source_all_eight_calls_type_without_public_source_authority() {
    let fixture = Fixture::new(&workflow());
    let project = fixture.project();
    let index = candidate(&project);
    let work = WorkMeter::default();
    let typed =
        resolve::type_owned_vector_source(&index, &work, &mut Allocator::default()).unwrap();
    assert_eq!(
        typed.admission(),
        resolve::SourceAdmission::OwnedVectorPipeline
    );
    assert!(!typed.admission().executable());
    assert!(typed.admission().allows_lowering());
    assert!(typed.admission().allows_paid_source(&index));
    assert!(!resolve::SourceAdmission::Executable.allows_paid_source(&index));
    assert_eq!(index.owned_vector_functions().len(), 8);
    assert_eq!(index.vector_functions().len(), 2);
    assert_eq!(typed.signatures().len(), index.function_count());
    assert_eq!(typed.functions().len(), index.source_function_count());
    typed.validate_function_signatures().unwrap();
    for (rank, key) in index.owned_vector_functions().iter().copied().enumerate() {
        let selected = signature(&index, &work, rank, key).unwrap();
        let actual = &typed.signatures()[selected.id.0];
        assert_eq!(actual.params, selected.signature.parameters());
        assert_eq!(actual.result, selected.signature.result);
        assert_eq!(actual.span, selected.anchor);
        assert_eq!(
            selected.id.0,
            index.source_function_count()
                + index.builtin_set().extra_functions()
                + index.vector_functions().len()
                + rank
        );
    }
    assert!(index.require_current_source_pipeline().is_err());
    assert!(super::super::program::check_typed(&typed).is_err());
    let mut ordinary_allocator = Allocator::default();
    assert!(index::collect_originals(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut ordinary_allocator
    )
    .is_err());
    assert_eq!(ordinary_allocator.attempts, 0);
}

#[test]
fn owned_vector_paid_source_requires_exact_candidate_before_replay_or_typing() {
    let fixture = Fixture::new("fn main()->i32{return 0;}");
    let project = fixture.project();
    let work = WorkMeter::default();
    let ordinary = index::collect_originals(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &work,
        &mut Allocator::default(),
    )
    .unwrap()
    .finish(&work, &mut Allocator::default())
    .unwrap();
    assert!(!ordinary.is_owned_vector_candidate_pipeline());
    assert!(!resolve::SourceAdmission::OwnedVectorPipeline.allows_paid_source(&ordinary));
    let mut allocator = Allocator::default();
    let errors =
        resolve::type_owned_vector_source(&ordinary, &WorkMeter::default(), &mut allocator)
            .unwrap_err();
    assert_eq!(errors[0].code, "E0500");
    assert_eq!(allocator.attempts, 0);
    assert!(
        super::super::hir_budget::preflight_owned_vector_hir(&ordinary, &WorkMeter::default())
            .is_err()
    );
}

#[test]
fn owned_vector_paid_source_preserves_canonical_aliases_and_zero_key_types() {
    let imports = IMPORTS
        .replace(
            "use alloc::vec::push_reserved;",
            "use alloc::vec::push_reserved as store;",
        )
        .replace(
            "use alloc::vec::pop_last;",
            "use alloc::vec::pop_last as remove;",
        );
    for source in [
        format!("{imports}fn main()->i32{{let mut values=new::<Entry>();try_reserve(&mut values,1);store(&mut values,Entry{{bytes:bytes_new()}});let item=remove(&mut values);return len(&values);}}"),
        format!("{IMPORTS}fn inspect(values:&Vec<Entry>)->(){{return;}}fn main()->i32{{return 0;}}"),
    ] {
        let fixture = Fixture::new(&source);
        let project = fixture.project();
        let index = candidate(&project);
        let work = WorkMeter::default();
        let typed = resolve::type_owned_vector_source(&index, &work, &mut Allocator::default()).unwrap();
        assert_eq!(typed.signatures().len(), index.function_count());
        assert!(!typed.admission().executable());
        assert!(typed.source_storage_bytes().is_some());
        assert!(super::super::program::check_typed(&typed).is_err());
    }
}

#[test]
fn owned_vector_paid_source_hir_allowance_and_work_are_exact_at_the_boundary() {
    let fixture = Fixture::new(&workflow());
    let project = fixture.project();
    let index = candidate(&project);
    let plan = super::super::hir_budget::preflight_owned_vector_hir(&index, &WorkMeter::default())
        .unwrap()
        .unwrap();
    assert_eq!(plan.counts.signatures, index.function_count());
    assert!(plan.fixed >= carrier_bytes());
    assert!(plan.total < super::super::hir_budget::MAX_HIR_BYTES);
    assert_eq!(
        super::super::hir_budget::preflight_owned_vector_hir_with_limit(
            &index,
            &WorkMeter::default(),
            plan.total
        )
        .unwrap()
        .unwrap(),
        plan
    );
    let error = super::super::hir_budget::preflight_owned_vector_hir_with_limit(
        &index,
        &WorkMeter::default(),
        plan.total - 1,
    )
    .unwrap_err();
    assert_eq!(error.code, "E0400");

    let work = WorkMeter::default();
    resolve::type_owned_vector_source(&index, &work, &mut Allocator::default()).unwrap();
    let used = work.used();
    let exact = WorkMeter::new(used);
    resolve::type_owned_vector_source(&index, &exact, &mut Allocator::default()).unwrap();
    assert_eq!(exact.used(), used);
    assert!(resolve::type_owned_vector_source(
        &index,
        &WorkMeter::new(used - 1),
        &mut Allocator::default()
    )
    .is_err());
    let mut allocator = Allocator::default();
    assert!(resolve::type_owned_vector_source(&index, &WorkMeter::new(0), &mut allocator).is_err());
    assert_eq!(allocator.attempts, 0);
}

#[test]
fn owned_vector_paid_source_resolver_and_typed_reservations_remain_fallible() {
    let fixture = Fixture::new(&workflow());
    let project = fixture.project();
    let index = candidate(&project);
    let mut baseline = Allocator::default();
    resolve::type_owned_vector_source(&index, &WorkMeter::default(), &mut baseline).unwrap();
    let resolver = baseline
        .trace
        .iter()
        .position(|event| event.kind == "paid HIR signatures")
        .expect("missing paid signature reservation");
    let typed = baseline
        .trace
        .iter()
        .rposition(|event| event.kind.starts_with("paid typed"))
        .expect("missing paid typed reservation");
    for ordinal in [resolver + 1, typed + 1] {
        let mut allocator = Allocator {
            fail_at: Some(ordinal),
            ..Default::default()
        };
        let errors =
            resolve::type_owned_vector_source(&index, &WorkMeter::default(), &mut allocator)
                .unwrap_err();
        assert!(errors.iter().any(|error| error.code == "E0400"));
        assert!(allocator.trace.iter().any(|event| !event.success));
    }
}

#[test]
fn owned_vector_paid_source_rejects_wrong_key_rank_and_missing_owned_key() {
    let fixture = Fixture::new(&workflow());
    let project = fixture.project();
    let index = candidate(&project);
    let keys = index.owned_vector_functions();
    assert!(signature(&index, &WorkMeter::default(), 1, keys[0]).is_err());
    let foreign = owned_vector::Function::record(keys[0].operation(), RecordId(4095)).unwrap();
    assert!(signature(&index, &WorkMeter::default(), 0, foreign).is_err());
}
