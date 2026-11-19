use super::*;
use crate::frontend::builtin_catalog::owned_vector::cases;
use crate::frontend::{
    build_config::ExecutionPolicy,
    builtin_catalog::owned_vector::{Function, Operation},
    oir::owned_types::OwnedVectorRoot,
    project::{ProjectLimits, ProjectSources},
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
#[path = "owned_enum_vector_boundary_tests.rs"]
mod boundaries;
struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "oxid-enum-vector-source-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), source).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() && std::env::var_os("OXID_KEEP_TEST_PROJECTS").is_none() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
const SOURCE: &str = r#"
use alloc::bytes::ByteBuf;
use alloc::bytes::new as bytes_new;
use alloc::bytes::try_push as bytes_push;
use alloc::bytes::len as bytes_len;
use alloc::vec::Vec;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::try_reserve;
use alloc::vec::push_reserved;
use alloc::vec::pop_last;
use alloc::vec::take;
use alloc::vec::truncate;
enum Token<T> { Empty, Data(T), Wide(i128) }
struct Record { value: i32 }
fn make() -> Token<ByteBuf> {
    let mut bytes = bytes_new();
    bytes_push(&mut bytes, 42u8);
    return Token::<ByteBuf>::Data(bytes);
}
fn consume(value: Token<ByteBuf>) -> i32 {
    match value {
        Token::Empty => { return -1; },
        Token::Data(bytes) => { return bytes_len(&bytes); },
        Token::Wide(number) => { if number != 170141183460469231731687303715884105727i128 { return -2; } return 41; },
    }
}
fn main() -> i32 {
    let mut values = new::<Token<ByteBuf>>();
    try_reserve(&mut values, 2);
    if capacity(&values) < 2 { return -3; }
    push_reserved(&mut values, make());
    let first = consume(pop_last(&mut values));
    push_reserved(&mut values, Token::<ByteBuf>::Wide(170141183460469231731687303715884105727i128));
    let second = consume(pop_last(&mut values));
    push_reserved(&mut values, make());
    let mut detached = take(&mut values);
    truncate(&mut detached, 0);
    let mut records = new::<Record>();
    try_reserve(&mut records, 1);
    push_reserved(&mut records, Record { value: 3 });
    let record = pop_last(&mut records);
    if record.value != 3 || len(&values) != 0 || len(&detached) != 0 { return -4; }
    return first + second;
}
"#;

#[test]
fn owned_enum_vector_source_mixed_roots_freeze_real_enum_ids_and_execute_active_values() {
    let fixture = Fixture::new(SOURCE);
    let project = ProjectSources::load_typed(
        fixture.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default(),
    )
    .unwrap();
    let index = index::collect_owned_vector_candidate(
        SourceOwner::project(&project),
        IndexLimits::default(),
        8 * 1024 * 1024,
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    let typing_work = WorkMeter::default();
    let typed =
        resolve::type_owned_vector_source(&index, &typing_work, &mut Allocator::default()).unwrap();
    assert!(index
        .owned_vector_functions()
        .iter()
        .any(|key| matches!(key.element(), OwnedVectorRoot::Record(_))));
    assert!(index
        .owned_vector_functions()
        .iter()
        .any(|key| matches!(key.element(), OwnedVectorRoot::Enum(_))));
    for key in index.owned_vector_functions() {
        assert_eq!(
            index
                .owned_vector_signature(*key, &WorkMeter::default())
                .unwrap(),
            key.concrete_signature(if key.operation().requires_grow_status() {
                index
                    .experimental_enum_id(
                        crate::frontend::builtin_catalog::experimental::Enumeration::Grow,
                    )
                    .unwrap()
            } else {
                EnumId(0)
            })
        );
    }
    let associated = super::super::association::lower_and_associate_owned_vector(&typed).unwrap();
    let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
        associated,
        super::super::super::budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        let ((result, _), observed) =
            super::super::super::byte_buffer::testing::capture(Default::default(), || {
                execute::observe_witness(
                    &witness,
                    typed.entry(),
                    execute::Limits::default(),
                    policy,
                )
            });
        assert_eq!(result, Ok(Scalar::I32(42)));
        assert_eq!(observed.before_cleanup.live_allocations, 0);
        assert_eq!(
            observed.before_cleanup.allocations,
            observed.before_cleanup.releases
        );
        assert_eq!(observed.before_cleanup, observed.after_cleanup);
    }
}

#[test]
fn owned_enum_vector_source_forged_key_domain_and_nominal_never_substitute_for_the_source_call() {
    let fixture = Fixture::new(SOURCE);
    let project = ProjectSources::load_typed(
        fixture.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default(),
    )
    .unwrap();
    let index = index::collect_owned_vector_candidate(
        SourceOwner::project(&project),
        IndexLimits::default(),
        8 * 1024 * 1024,
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    let enum_key = *index
        .owned_vector_functions()
        .iter()
        .find(|key| {
            key.operation() == Operation::New && matches!(key.element(), OwnedVectorRoot::Enum(_))
        })
        .unwrap();
    let enum_id = enum_key.element().enumeration().unwrap();
    let record_key = Function::record(Operation::New, RecordId(enum_id.0)).unwrap();
    assert_ne!(
        index
            .owned_vector_function_id(record_key, &WorkMeter::default())
            .unwrap(),
        index
            .owned_vector_function_id(enum_key, &WorkMeter::default())
            .unwrap()
    );
    assert!(index
        .owned_vector_function_id(
            Function::enumeration(Operation::New, EnumId(4095)).unwrap(),
            &WorkMeter::default()
        )
        .is_err());
    assert!(index
        .owned_vector_signature(
            Function::enumeration(Operation::New, EnumId(index.source_enum_count())).unwrap(),
            &WorkMeter::default()
        )
        .is_err());
    let typing_work = WorkMeter::default();
    let typed =
        resolve::type_owned_vector_source(&index, &typing_work, &mut Allocator::default()).unwrap();
    for replacement in [
        record_key,
        Function::enumeration(Operation::New, EnumId(4095)).unwrap(),
    ] {
        let mut raw = lower::lower(&typed).unwrap();
        let mut changed = false;
        for function in &mut raw.functions {
            for block in &mut function.blocks {
                for statement in &mut block.statements {
                    if let OwnedInstruction::OwnedVector { operation, .. } = &mut statement.kind {
                        if *operation == enum_key {
                            *operation = replacement;
                            changed = true;
                        }
                    }
                }
            }
        }
        assert!(changed);
        use super::super::super::owned_vector_descriptors::{
            OwnedVectorImportClaims, RawOwnedVectorProgram,
        };
        use crate::frontend::builtin_catalog::owned_vector::Item;
        let wrapper = RawOwnedVectorProgram::prepare(
            raw,
            OwnedVectorImportClaims::empty()
                .admit(Item::PushReserved)
                .admit(Item::PopLast),
            index.owned_vector_functions(),
            project.sources(),
            super::super::super::budget::Limits::DEFAULT,
            &mut Allocator::default(),
        )
        .unwrap();
        assert!(wrapper
            .check(
                project.sources(),
                super::super::super::budget::Limits::DEFAULT
            )
            .is_err());
    }
}

#[test]
fn builtin_enum_vectors_only_materialize_final_nominals_after_late_generic_applications() {
    let fixture = Fixture::new(&format!("{}{}", cases::API, cases::INTERLEAVED_BUILTINS));
    let project = ProjectSources::load_typed(
        fixture.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default(),
    )
    .unwrap();
    let index = index::collect_owned_vector_candidate(
        SourceOwner::project(&project),
        IndexLimits::default(),
        8 * 1024 * 1024,
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    assert_eq!(index.source_enum_count(), 2);
    let grow = index
        .experimental_enum_id(crate::frontend::builtin_catalog::experimental::Enumeration::Grow)
        .unwrap();
    let read = index
        .builtin_enum_id(crate::frontend::builtin_catalog::BuiltinEnum::ReadStatus)
        .unwrap();
    assert_eq!((read.0, grow.0), (2, 3));
    for (position, key) in index.owned_vector_functions().iter().enumerate() {
        assert!(!index.owned_vector_functions()[..position].contains(key));
    }
    for enumeration in [EnumId(0), EnumId(1), read, grow] {
        assert!(index
            .owned_vector_functions()
            .contains(&Function::enumeration(Operation::New, enumeration).unwrap()));
        assert!(index
            .owned_vector_functions()
            .contains(&Function::enumeration(Operation::PopLast, enumeration).unwrap()));
    }
    let typing_work = WorkMeter::default();
    let typed =
        resolve::type_owned_vector_source(&index, &typing_work, &mut Allocator::default()).unwrap();
    let associated = super::super::association::lower_and_associate_owned_vector(&typed).unwrap();
    let witness = verified::VerifiedOwnedVectorProgram::verify_associated(
        associated,
        super::super::super::budget::Limits::DEFAULT,
        &mut Allocator::default(),
    )
    .unwrap();
    let declarations = witness.declarations();
    for (enumeration, size, alignment, variants) in [
        (EnumId(0), 8, 4, 2),
        (EnumId(1), 32, 16, 2),
        (read, 8, 4, 3),
        (grow, 8, 4, 4),
    ] {
        let layout = declarations
            .aggregate_layout(AggregateTy::Enum(enumeration))
            .unwrap();
        assert_eq!((layout.size(), layout.align()), (size, alignment));
        assert_eq!(
            declarations.enums().variants(enumeration).unwrap().len(),
            variants
        );
        assert_eq!(
            witness
                .elements()
                .element_root(OwnedVectorRoot::Enum(enumeration))
                .unwrap()
                .root(),
            OwnedVectorRoot::Enum(enumeration)
        );
    }
    for policy in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        let ((result, _), observed) =
            super::super::super::byte_buffer::testing::capture(Default::default(), || {
                execute::observe_witness(
                    &witness,
                    typed.entry(),
                    execute::Limits::default(),
                    policy,
                )
            });
        assert_eq!(result, Ok(Scalar::I32(42)));
        assert_eq!(observed.before_cleanup.live_allocations, 0);
        assert_eq!(
            observed.before_cleanup.allocations,
            observed.before_cleanup.releases
        );
        assert_eq!(observed.before_cleanup, observed.after_cleanup);
    }
}
