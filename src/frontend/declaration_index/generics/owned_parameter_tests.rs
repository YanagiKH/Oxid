use super::*;
use crate::frontend::builtin_catalog::owned_vector::parameter_cases as cases;
use crate::frontend::project::{ProjectLimits, ProjectSources};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new(body: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "oxid-owned-parameter-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), body).unwrap();
        Self(root)
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
        if !std::thread::panicking() && std::env::var_os("OXID_KEEP_TEST_PROJECTS").is_none() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn candidate<'s>(
    project: &'s ProjectSources,
    metadata: usize,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<DeclarationIndex<'s>, Vec<Diagnostic>> {
    crate::frontend::declaration_index::sealed::collect_owned_vector_candidate(
        SourceOwner::project(project),
        IndexLimits::default(),
        metadata,
        work,
        allocator,
    )
}
const IMPORTS: &str = "use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::len;use alloc::vec::try_reserve;use alloc::vec::push_reserved;use alloc::vec::pop_last;use alloc::vec::take;use alloc::bytes::ByteBuf;";

#[test]
fn borrowed_owned_parameter_unused_source_has_no_executable_identity() {
    let fixture = Fixture::new(&format!("{}fn main()->i32{{return 0;}}", cases::LIBRARY));
    let project = fixture.project();
    let index = candidate(
        &project,
        MAX_STORAGE,
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    assert!(index.owned_vector_source_needed());
    assert!(index.owned_vector_functions().is_empty());
    assert!(index.vector_functions().is_empty());
    assert_eq!(index.source_function_count(), 1);
    assert!(index.require_current_source_pipeline().is_err());
    index
        .validate_owned_vector_candidate(&WorkMeter::default(), &mut Allocator::default())
        .unwrap();
    let at = SourceOwner::project(&project).eof();
    let nominals = &index.generic_plan().unwrap().nominals;
    let opaque = Element::OwnedParameter(0);
    assert_eq!(
        opaque.value(),
        Value::Parameter {
            position: 0,
            copy: false
        }
    );
    assert!(!nominals.concrete(Value::Vector(opaque)));
    assert!(nominals.value(Value::Vector(opaque), at).is_err());
    assert!(nominals
        .parameter(
            Type::Reference {
                value: Value::Vector(opaque),
                kind: BorrowKind::Exclusive
            },
            at
        )
        .is_err());
    assert!(Element::checked(opaque.value(), nominals, at).is_err());
    assert!(index
        .candidate_borrowed_element(opaque.value(), nominals, &WorkMeter::default(), at)
        .is_err());
    assert!(opaque.borrowed(at).is_err());
    let forged = owned::Function::record(owned::Operation::PushReserved, RecordId(0)).unwrap();
    assert!(index
        .owned_vector_function_id(forged, &WorkMeter::default())
        .is_err());
    assert!(index
        .owned_vector_signature(forged, &WorkMeter::default())
        .is_err());
}

#[test]
fn borrowed_owned_parameter_unused_definitions_still_prove_affine_operations() {
    for declaration in [
        "fn bad<T>(v:&mut Vec<T>,x:T)->(){push_reserved(&mut *v,x);push_reserved(&mut *v,x);return;}",
        "fn bad<T>(v:&mut Vec<T>,x:T,flag:bool)->(){if flag{push_reserved(&mut *v,x);}push_reserved(&mut *v,x);return;}",
        "fn bad<T>(v:&mut Vec<T>)->(){push_reserved(&mut *v,pop_last(&mut *v));return;}",
        "fn bad<T>(v:&mut Vec<T>)->(){let alias=&mut *v;push_reserved(&mut *v,pop_last(&mut *alias));return;}",
        "fn bad<T,U>(v:&mut Vec<T>,x:U)->(){push_reserved(&mut *v,x);return;}",
        "fn bad<T>(v:&mut Vec<T>)->T{return v[0];}",
        "fn count<T:Copy>(v:&[T])->i32{return v.len();}fn bad<T>(v:&mut Vec<T>)->i32{return count(&*v[0..0]);}",
        "fn bad<T>(v:&Vec<T>,x:T)->(){push_reserved(&mut *v,x);return;}",
        "fn bad<T>(v:&mut Vec<T>,x:T)->(){while true{push_reserved(&mut *v,x);}return;}",
        "fn bad<T>()->(){let v=new::<T>();return;}",
        "fn bad<T>(v:&[T])->(){return;}",
        "struct Packet<T>{value:T}fn bad<T>(v:&mut Vec<Packet<T>>)->(){return;}",
        "fn bad<T>(v:&mut Vec<Vec<T>>)->(){return;}",
        "use alloc::vec::try_push;fn bad<T>(v:&mut Vec<T>,x:T)->(){try_push(&mut *v,x);return;}",
    ] {
        let fixture = Fixture::new(&format!("{IMPORTS}{declaration}fn main()->i32{{return 0;}}"));
        let project = fixture.project();
        let errors = candidate(&project, MAX_STORAGE, &WorkMeter::default(), &mut Allocator::default()).unwrap_err();
        assert!(errors.iter().any(|error| error.code == "E0300"), "{declaration}: {errors:?}");
        assert!(!errors.iter().any(|error| error.code == "E0500"), "{declaration}: {errors:?}");
    }
}

#[test]
fn function_owned_vector_parameters_and_results_remain_abstract_until_substitution() {
    for declaration in [
        "fn pass<T>(v:Vec<T>)->Vec<T>{return v;}",
        "fn detach<T>(v:&mut Vec<T>)->Vec<T>{return take(&mut *v);}",
        "fn discard<T>(v:Vec<T>)->(){return;}",
    ] {
        let fixture = Fixture::new(&format!(
            "{IMPORTS}{declaration}fn main()->i32{{return 0;}}"
        ));
        let project = fixture.project();
        let index = candidate(
            &project,
            MAX_STORAGE,
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        assert!(index.owned_vector_source_needed());
        assert!(index.owned_vector_functions().is_empty());
        assert!(index.vector_functions().is_empty());
        assert_eq!(index.source_function_count(), 1);
        index
            .validate_owned_vector_candidate(&WorkMeter::default(), &mut Allocator::default())
            .unwrap();
        let at = SourceOwner::project(&project).eof();
        let nominals = &index.generic_plan().unwrap().nominals;
        let opaque = Value::Vector(Element::OwnedParameter(0));
        assert!(!nominals.copy(opaque));
        assert!(!nominals.concrete(opaque));
        assert!(nominals.parameter(Type::Value(opaque), at).is_err());
        assert!(nominals.value(opaque, at).is_err());
    }
}

#[test]
fn function_owned_vector_values_reject_duplicate_moves_loans_and_unsupported_positions() {
    for declaration in [
        "fn bad<T>(v:Vec<T>)->Vec<T>{let moved=v;return v;}",
        "fn pass<T>(v:Vec<T>)->Vec<T>{return v;}fn bad<T>(v:Vec<T>)->Vec<T>{pass(v);return v;}",
        "fn bad<T>(v:Vec<T>,flag:bool)->Vec<T>{if flag{let moved=v;}return v;}",
        "fn bad<T>(v:Vec<T>)->Vec<T>{let alias=&v;return v;}",
        "fn bad<T>(v:&Vec<T>)->Vec<T>{return take(&*v);}",
        "fn bad<T>(v:Vec<T>)->T{return v[0];}",
        "fn bad<T>(v:Vec<T>)->(){let alias=&v;let moved=v;return;}",
        "fn bad<T>(v:Vec<T>)->Vec<Vec<T>>{return v;}",
        "struct Holder<T>{values:Vec<T>}fn bad<T>(v:Vec<T>)->(){return;}",
        "enum Holder<T>{Values(Vec<T>)}fn bad<T>(v:Vec<T>)->(){return;}",
        "fn bad<T>(v:Vec<T>)->Vec<T>{return new::<T>();}",
    ] {
        let fixture = Fixture::new(&format!(
            "{IMPORTS}{declaration}fn main()->i32{{return 0;}}"
        ));
        let project = fixture.project();
        let errors = candidate(
            &project,
            MAX_STORAGE,
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == "E0300"),
            "{declaration}: {errors:?}"
        );
        assert!(
            !errors.iter().any(|error| error.code == "E0500"),
            "{declaration}: {errors:?}"
        );
    }
}

#[test]
fn function_owned_vector_concrete_roots_preserve_scalar_copy_and_owned_key_domains() {
    let fixture = Fixture::new(&format!("mod owned_vec;{}", cases::OWNERSHIP));
    fs::write(fixture.0.join("owned_vec.ox"), cases::LIBRARY).unwrap();
    let project = fixture.project();
    let mut index = candidate(
        &project,
        MAX_STORAGE,
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    assert!(index
        .owned_vector_functions()
        .iter()
        .any(|key| matches!(key.element(), OwnedVectorRoot::Record(_))));
    assert!(index
        .owned_vector_functions()
        .iter()
        .any(|key| matches!(key.element(), OwnedVectorRoot::Enum(_))));
    assert!(index
        .vector_functions()
        .iter()
        .any(|key| key.record_id().is_some()));
    assert!(index
        .vector_functions()
        .iter()
        .any(|key| key.record_id().is_none() && key.element() == Ty::I32));
    for instance in &index.generic_plan().unwrap().instances {
        assert!(instance.arguments.iter().all(|value| index
            .generic_plan()
            .unwrap()
            .nominals
            .concrete(*value)));
        for (_, ty) in &instance.types {
            assert!(!matches!(
                ty,
                Type::Value(Value::Vector(Element::OwnedParameter(_)))
            ));
        }
    }
    index
        .validate_owned_vector_candidate(&WorkMeter::default(), &mut Allocator::default())
        .unwrap();
    index.alter_owned_candidate_for_test(|owner| {
        let slot = owner
            .ordinary
            .instances
            .iter_mut()
            .flat_map(|instance| instance.types.iter_mut())
            .find(|(_, ty)| matches!(ty, Type::Value(Value::Vector(_))))
            .unwrap();
        slot.1 = Type::Value(Value::Vector(Element::OwnedParameter(0)));
    });
    assert!(index
        .validate_owned_vector_candidate(&WorkMeter::default(), &mut Allocator::default())
        .is_err());
}

#[test]
fn mixed_owned_candidate_non_vector_signatures_query_each_outer_path_once() {
    let source = format!("{IMPORTS}struct Entry{{bytes:ByteBuf}}struct Packet<T>{{value:T}}enum Envelope<T>{{Value(T)}}fn pass<T>(v:Packet<T>)->Packet<T>{{return v;}}fn echo<T>(v:Envelope<T>)->Envelope<T>{{return v;}}fn main()->i32{{let values=new::<Entry>();let packet=pass(Packet::<i32>{{value:42}});let envelope=echo(Envelope::<i32>::Value(packet.value));match envelope{{Envelope::Value(value)=>{{return value;}},}}}}");
    let fixture = Fixture::new(&source);
    let project = fixture.project();
    let work = WorkMeter::default();
    work.enable_observation();
    let index = candidate(&project, MAX_STORAGE, &work, &mut Allocator::default()).unwrap();
    assert!(index.owned_vector_source_needed());
    let module = ModuleId(0);
    let program = index.sources().ast(module).unwrap();
    let events = work.events.borrow();
    let observations = work.observations.borrow();
    for name in ["pass", "echo"] {
        let function = program
            .functions
            .iter()
            .find(|function| index.sources().text(function.name).unwrap() == name)
            .unwrap();
        for syntax in [function.params[0].ty, function.result] {
            let TypeSyntaxKind::Applied(id) = syntax.kind else {
                panic!("expected applied nominal");
            };
            let application = program.generic_application(id).unwrap();
            let path = index
                .sources()
                .path_span(ItemPathRef {
                    file: syntax.span.file,
                    path: application.path,
                })
                .unwrap();
            let admissions = events
                .iter()
                .filter(|event| {
                    event.origin == syntax.span
                        && event.operation == "owned vector function value admission"
                })
                .count();
            assert!(admissions > 0, "{name}: {syntax:?}");
            let queries = events
                .iter()
                .filter(|event| {
                    event.origin == path
                        && matches!(event.operation, "query nominal type" | "query record type")
                })
                .count();
            assert_eq!(queries, admissions, "{name}: {syntax:?}");
            let bases = events
                .iter()
                .filter(|event| {
                    event.origin == syntax.span
                        && event.operation == "generic parameter base resolution"
                })
                .count();
            assert_eq!(bases, admissions, "{name}: {syntax:?}");
            let targets = observations.iter().filter(|observation| matches!(observation,
                Observation::Target { operation: "value-type", origin, .. } if *origin == path
            )).count();
            assert_eq!(targets, admissions, "{name}: {syntax:?}");
        }
    }
}

#[test]
fn borrowed_owned_parameter_concrete_calls_use_existing_roots_and_replay_all_facts() {
    let fixture = Fixture::new(cases::EXAMPLE);
    fs::write(fixture.0.join("owned_vec.ox"), cases::LIBRARY).unwrap();
    // The checked source is the real example, with its dependency represented
    // by an ordinary flat module for this private parsed-source fixture.
    let source = format!("mod owned_vec;{}", cases::EXAMPLE);
    fs::write(fixture.0.join("main.ox"), source).unwrap();
    let project = fixture.project();
    for change in 0..4 {
        let mut index = candidate(
            &project,
            MAX_STORAGE,
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        assert!(index
            .owned_vector_functions()
            .iter()
            .any(|key| matches!(key.element(), OwnedVectorRoot::Record(_))));
        assert!(index
            .owned_vector_functions()
            .iter()
            .any(|key| matches!(key.element(), OwnedVectorRoot::Enum(_))));
        for key in index.owned_vector_functions() {
            assert!(index
                .owned_vector_signature(*key, &WorkMeter::default())
                .is_ok());
        }
        index
            .validate_owned_vector_candidate(&WorkMeter::default(), &mut Allocator::default())
            .unwrap();
        index.alter_owned_candidate_for_test(|owner| match change {
            0 => owner.source_needed = false,
            1 => {
                owner.keys[0] =
                    owned::Function::enumeration(owner.keys[0].operation(), EnumId(4095)).unwrap()
            }
            2 => {
                owner.ordinary.instances[0].types[0].1 = Type::Reference {
                    value: Value::Vector(Element::OwnedParameter(0)),
                    kind: BorrowKind::Exclusive,
                }
            }
            3 => {
                owner.calls[0].key =
                    owned::Function::record(owned::Operation::PopLast, RecordId(4095)).unwrap()
            }
            _ => unreachable!(),
        });
        assert!(index
            .validate_owned_vector_candidate(&WorkMeter::default(), &mut Allocator::default())
            .is_err());
    }
}

#[test]
fn borrowed_owned_parameter_concrete_copy_and_ineligible_roots_cannot_use_owned_operations() {
    for (declarations, element, value) in [
        ("", "i32", "42"),
        ("#[derive(Copy)]struct C{value:i32}", "C", "C{value:42}"),
        (
            "use alloc::bytes::new as bytes_new;",
            "ByteBuf",
            "bytes_new()",
        ),
        ("struct Entry{value:i32}", "Vec<Entry>", "new::<Entry>()"),
    ] {
        let fixture = Fixture::new(&format!("{}use alloc::vec::new;use alloc::bytes::ByteBuf;{declarations}fn main()->i32{{let mut values=new::<{element}>();let result=try_push(&mut values,{value});return 0;}}", cases::LIBRARY));
        let project = fixture.project();
        assert!(
            candidate(
                &project,
                MAX_STORAGE,
                &WorkMeter::default(),
                &mut Allocator::default()
            )
            .is_err(),
            "{element}"
        );
    }
}

#[test]
fn borrowed_owned_parameter_work_storage_and_allocator_refusals_remain_exact() {
    let fixture = Fixture::new(&format!("{}fn main()->i32{{return 0;}}", cases::LIBRARY));
    let project = fixture.project();
    let work = WorkMeter::default();
    let mut baseline = Allocator::default();
    let index = candidate(&project, MAX_STORAGE, &work, &mut baseline).unwrap();
    candidate(
        &project,
        MAX_STORAGE,
        &WorkMeter::new(work.used()),
        &mut Allocator::default(),
    )
    .unwrap();
    assert!(candidate(
        &project,
        MAX_STORAGE,
        &WorkMeter::new(work.used() - 1),
        &mut Allocator::default()
    )
    .is_err());
    let mut low = factory_bytes();
    let mut high = MAX_STORAGE;
    while low < high {
        let middle = low + (high - low) / 2;
        if candidate(
            &project,
            middle,
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .is_ok()
        {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    candidate(
        &project,
        low,
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    assert!(candidate(
        &project,
        low - 1,
        &WorkMeter::default(),
        &mut Allocator::default()
    )
    .is_err());
    for (position, event) in baseline.trace.iter().enumerate() {
        if event.kind.starts_with("owned vector candidate") {
            let mut allocator = Allocator {
                fail_at: Some(position + 1),
                ..Default::default()
            };
            assert!(
                candidate(&project, MAX_STORAGE, &WorkMeter::default(), &mut allocator).is_err()
            );
            assert!(allocator.trace.iter().any(|event| !event.success));
        }
    }
    let replay = WorkMeter::default();
    index
        .validate_owned_vector_candidate(&replay, &mut Allocator::default())
        .unwrap();
    index
        .validate_owned_vector_candidate(&WorkMeter::new(replay.used()), &mut Allocator::default())
        .unwrap();
    assert!(index
        .validate_owned_vector_candidate(
            &WorkMeter::new(replay.used() - 1),
            &mut Allocator::default()
        )
        .is_err());
}
