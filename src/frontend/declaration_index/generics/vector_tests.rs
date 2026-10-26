//! Scalar-vector specialization, generic definition and source authority controls.
use super::*;
use crate::frontend::project::{ProjectLimits, ProjectSources};
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
            "oxid-vector-generics-{}-{}",
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

fn origin() -> Span {
    Span {
        file: SourceFileId(0),
        start: 0,
        end: 1,
    }
}

fn empty_plan() -> Plan {
    Plan {
        instances: Vec::new(),
        ordinary: Vec::new(),
        template_count: 0,
        nominals: Nominals::default(),
        vector_functions: Vec::new(),
    }
}

fn source_plan(
    text: &str,
    inspect: impl FnOnce(&Plan, &DeclarationIndex<'_>, &WorkMeter),
) -> Result<(), Vec<Diagnostic>> {
    let fixture = Fixture::new(text);
    let project = fixture.project();
    let mut allocator = Allocator::default();
    let owner = SourceOwner::project(&project);
    let work = WorkMeter::default();
    let facts = collect_originals(owner, IndexLimits::default(), &work, &mut allocator)
        .map_err(|error| vec![*error])?;
    let index = facts.finish(&work, &mut allocator)?;
    let plan = index
        .generic_plan()
        .expect("vector source must use the generic plan");
    inspect(plan, &index, &work);
    Ok(())
}

#[test]
fn scalar_elements_exclude_affine_values_and_unbounded_parameters() {
    let at = origin();
    for ty in [
        Ty::Unit,
        Ty::Bool,
        Ty::I8,
        Ty::I32,
        Ty::I128,
        Ty::U128,
        Ty::F64,
        Ty::Char,
    ] {
        let element = Element::check(Value::Concrete(ValueTy::Scalar(ty)), at).unwrap();
        assert_eq!(element, Element::Concrete(ty));
        assert_eq!(element.scalar(at).unwrap(), ty);
        assert_eq!(element.value(), Value::Concrete(ValueTy::Scalar(ty)));
    }
    let opaque = Element::check(
        Value::Parameter {
            position: 3,
            copy: true,
        },
        at,
    )
    .unwrap();
    assert_eq!(opaque, Element::CopyParameter(3));
    assert_eq!(
        opaque.value(),
        Value::Parameter {
            position: 3,
            copy: true
        }
    );
    assert!(opaque.scalar(at).is_err());
    for value in [
        Value::Parameter {
            position: 3,
            copy: false,
        },
        Value::Concrete(ValueTy::Owned(AggregateTy::ByteBuf)),
        Value::Concrete(ValueTy::Owned(AggregateTy::File)),
        Value::Vector(Element::Concrete(Ty::I32)),
        Value::Record(0),
        Value::Enumeration(0),
    ] {
        assert_eq!(Element::check(value, at).unwrap_err().code, "E0300");
    }
    assert!(!Nominals::copy(Value::Vector(Element::CopyParameter(3))));
    assert!(!Nominals::copy(Value::Vector(Element::Concrete(Ty::Unit))));
}

#[test]
fn abstract_signatures_materialize_the_nine_independent_concrete_signatures() {
    let element = Element::Concrete(Ty::I64);
    let vector = ValueTy::Owned(AggregateTy::Vector(Ty::I64));
    let shared = ParameterTy::Reference {
        referent: BorrowedTy::Exact(AggregateTy::Vector(Ty::I64)),
        kind: BorrowKind::Shared,
    };
    let exclusive = ParameterTy::Reference {
        referent: BorrowedTy::Exact(AggregateTy::Vector(Ty::I64)),
        kind: BorrowKind::Exclusive,
    };
    let slice = ParameterTy::Reference {
        referent: BorrowedTy::ScalarSlice(Ty::I64),
        kind: BorrowKind::Shared,
    };
    let count = ParameterTy::Value(ValueTy::Scalar(Ty::I32));
    let value = ParameterTy::Value(ValueTy::Scalar(Ty::I64));
    let grow = ValueTy::Owned(AggregateTy::Enum(EnumId(91)));
    let expected: [(vector::Operation, &[ParameterTy], ValueTy); 9] = [
        (vector::Operation::New, &[], vector),
        (vector::Operation::Len, &[shared], ValueTy::Scalar(Ty::I32)),
        (
            vector::Operation::Capacity,
            &[shared],
            ValueTy::Scalar(Ty::I32),
        ),
        (vector::Operation::TryReserve, &[exclusive, count], grow),
        (vector::Operation::TryPush, &[exclusive, value], grow),
        (vector::Operation::TryExtend, &[exclusive, slice], grow),
        (
            vector::Operation::TryResize,
            &[exclusive, count, value],
            grow,
        ),
        (
            vector::Operation::Truncate,
            &[exclusive, count],
            ValueTy::Scalar(Ty::Unit),
        ),
        (vector::Operation::Take, &[exclusive], vector),
    ];
    let nominals = Nominals::default();
    for (operation, parameters, result) in expected {
        let (actual, returned) = vector_signature(operation, element, EnumId(91));
        let actual = actual[..operation.parameter_count()]
            .iter()
            .map(|ty| nominals.parameter(*ty, origin()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(actual.as_slice(), parameters, "{operation:?}");
        assert_eq!(
            nominals
                .value(returned.value(origin()).unwrap(), origin())
                .unwrap(),
            result
        );
        let catalog = vector::Function::new(operation, Ty::I64)
            .unwrap()
            .signature(EnumId(91));
        assert_eq!(catalog.parameters(), parameters);
        assert_eq!(catalog.result, result);
    }
}

#[test]
fn only_requested_concrete_keys_are_paid_and_repeated_keys_keep_their_order() {
    let mut plan = empty_plan();
    let mut storage = Storage {
        bytes: size_of::<Plan>(),
        ..Default::default()
    };
    let mut allocator = Allocator::default();
    let work = WorkMeter::default();
    let push = vector::Function::new(vector::Operation::TryPush, Ty::U8).unwrap();
    let new = vector::Function::new(vector::Operation::New, Ty::I128).unwrap();
    assert_eq!(
        plan.register_vector(push, &mut storage, &mut allocator, &work, origin())
            .unwrap(),
        0
    );
    assert_eq!(
        plan.register_vector(new, &mut storage, &mut allocator, &work, origin())
            .unwrap(),
        1
    );
    let before = storage.bytes;
    let used = work.used();
    assert_eq!(
        plan.register_vector(push, &mut storage, &mut allocator, &work, origin())
            .unwrap(),
        0
    );
    assert_eq!(plan.vector_functions(), &[push, new]);
    assert_eq!(plan.vector_count(), 2);
    assert_eq!(plan.vector_function(1), Some(new));
    assert_eq!(plan.vector_function_rank(push), Some(0));
    assert_eq!(
        plan.vector_function_rank(vector::Function::new(vector::Operation::Take, Ty::U8).unwrap()),
        None
    );
    assert_eq!(storage.bytes, before);
    assert_eq!(storage.constructor_scratch, 0);
    assert!(work.used() > used);
    assert!(storage.live_after(0).unwrap() <= MAX_STORAGE);
    assert_eq!(plan.len(), 0);
}

#[test]
fn vector_storage_work_and_allocation_refusals_leave_live_frames_balanced() {
    let key = vector::Function::new(vector::Operation::New, Ty::Unit).unwrap();
    let mut plan = empty_plan();
    let mut storage = Storage {
        bytes: MAX_STORAGE - VECTOR_FRAME_BYTES,
        ..Default::default()
    };
    let mut allocator = Allocator::default();
    assert_eq!(
        plan.register_vector(
            key,
            &mut storage,
            &mut allocator,
            &WorkMeter::default(),
            origin()
        )
        .unwrap_err()
        .code,
        "E0400"
    );
    assert_eq!(allocator.attempts, 0);
    assert_eq!(storage.constructor_scratch, 0);
    assert!(plan.vector_functions().is_empty());
    for (fail_at, limit) in [(None, 0), (Some(1), u64::MAX)] {
        let mut plan = empty_plan();
        let mut storage = Storage {
            bytes: size_of::<Plan>(),
            ..Default::default()
        };
        let mut allocator = Allocator {
            fail_at,
            ..Default::default()
        };
        assert_eq!(
            plan.register_vector(
                key,
                &mut storage,
                &mut allocator,
                &WorkMeter::new(limit),
                origin()
            )
            .unwrap_err()
            .code,
            "E0400"
        );
        assert_eq!(storage.constructor_scratch, 0);
        assert!(plan.vector_functions().is_empty());
        assert_eq!(allocator.attempts, usize::from(fail_at.is_some()));
    }
    assert_eq!(vector_id(5, 3, 0, origin()).unwrap(), DefId(8));
    assert_eq!(vector_id(5, 3, 1, origin()).unwrap(), DefId(9));
    assert_eq!(
        vector_id(usize::MAX, 1, 0, origin()).unwrap_err().code,
        "E0400"
    );
    assert_eq!(
        vector_id(0, usize::MAX, 1, origin()).unwrap_err().code,
        "E0400"
    );
}

#[test]
fn vector_calls_freeze_after_late_user_instances_and_after_the_fixed_suffix() {
    let source = "use alloc::vec::new;use alloc::vec::len;use alloc::bytes::new as byte_new;fn identity<T>(value:T)->T{return value;}fn main()->i32{let values=new::<i32>();byte_new();identity(true);return identity(len(&values));}";
    source_plan(source, |plan, index, work| {
        assert_eq!(plan.len(), 3);
        assert_eq!(index.builtin_set().extra_functions(), 1);
        assert_eq!(
            plan.vector_functions(),
            &[
                vector::Function::new(vector::Operation::New, Ty::I32).unwrap(),
                vector::Function::new(vector::Operation::Len, Ty::I32).unwrap(),
            ]
        );
        let mut vectors = 0;
        let mut fixed = 0;
        for (caller, instance) in plan.instances.iter().enumerate() {
            for call in &instance.calls {
                if let Some(key) = call.vector {
                    let rank = plan.vector_function_rank(key).unwrap();
                    assert_eq!(call.target, DefId(4 + rank));
                    assert_eq!(
                        index
                            .vector_template(
                                call.template,
                                &WorkMeter::default(),
                                index.sources().eof()
                            )
                            .unwrap(),
                        Some(key.operation)
                    );
                    assert_eq!(
                        index
                            .instantiated_call(DefId(caller), call.expression, call.template)
                            .unwrap(),
                        call.target
                    );
                    assert!(index
                        .instantiated_call(DefId(caller), call.expression, DefId(usize::MAX))
                        .is_err());
                    vectors += 1;
                } else if call.template.0 >= plan.template_count {
                    assert_eq!(call.target, DefId(3));
                    fixed += 1;
                }
            }
        }
        assert_eq!((vectors, fixed), (2, 1));
        index.validate_generic_source(work).unwrap();
    })
    .unwrap();
}

#[test]
fn unused_generic_vector_definitions_validate_without_creating_specializations() {
    source_plan(
        "use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::try_resize;struct Holder<T:Copy>{values:Vec<T>}enum Packet<T:Copy>{Data(Vec<T>)}fn unused<T:Copy>()->Vec<T>{return new::<T>();}fn inspect<T:Copy>(items:&[T])->T{return items[0];}fn main()->i32{return 0;}",
        |plan, _, _| {
            assert_eq!(plan.len(), 1);
            assert!(plan.vector_functions().is_empty());
        },
    ).unwrap();
    for source in [
        "use alloc::vec::Vec;struct Holder<T>{values:Vec<T>}fn main()->i32{return 0;}",
        "use alloc::vec::Vec;enum Packet<T>{Data(Vec<T>)}fn main()->i32{return 0;}",
        "fn unused<T>(items:&[T])->(){return;}fn main()->i32{return 0;}",
        "use alloc::vec::Vec;fn unused<T>(values:Vec<T>)->(){return;}fn main()->i32{return 0;}",
    ] {
        let errors =
            source_plan(source, |_, _, _| panic!("invalid definition accepted")).unwrap_err();
        assert_eq!(errors[0].code, "E0300", "{source}: {errors:?}");
    }
}

#[test]
fn generic_range_forwarding_matches_the_root_in_unused_and_instantiated_definitions() {
    const READER: &str = "fn read<T:Copy>(p:&[T])->i32{return p.len();}";
    const VECTOR_IMPORTS: &str = "use alloc::vec::Vec;use alloc::vec::new;";
    const HOLDER: &str = "struct Holder<T:Copy>{values:Vec<T>}";
    for (name, imports, declaration, parameter, invalid, valid, invocation) in [
        (
            "shared slice root",
            "",
            "",
            "&[T]",
            "&value[0..1]",
            "&*value[0..1]",
            "let values=[1,2];return window(&values);",
        ),
        (
            "owned vector root",
            VECTOR_IMPORTS,
            "",
            "Vec<T>",
            "&*value[0..0]",
            "&value[0..0]",
            "let values=new::<i32>();return window(values);",
        ),
        (
            "owned generic record root",
            VECTOR_IMPORTS,
            HOLDER,
            "Holder<T>",
            "&*value.values[0..0]",
            "&value.values[0..0]",
            "let holder=Holder{values:new::<i32>()};return window(holder);",
        ),
        (
            "borrowed generic record root",
            VECTOR_IMPORTS,
            HOLDER,
            "&Holder<T>",
            "&value.values[0..0]",
            "&*value.values[0..0]",
            "let holder=Holder{values:new::<i32>()};return window(&holder);",
        ),
    ] {
        for instantiated in [false, true] {
            let main = if instantiated {
                invocation
            } else {
                "return 0;"
            };
            for (borrow, accepted) in [(invalid, false), (valid, true)] {
                let text = format!(
                    "{imports}{declaration}{READER}fn window<T:Copy>(value:{parameter})->i32{{return read({borrow});}}fn main()->i32{{{main}}}"
                );
                let result = source_plan(&text, |plan, index, work| {
                    assert!(accepted, "invalid {name} reached a frozen plan: {text}");
                    assert_eq!(plan.len(), if instantiated { 3 } else { 1 });
                    if !instantiated {
                        assert!(plan.vector_functions().is_empty());
                    }
                    index.validate_generic_source(work).unwrap();
                });
                if accepted {
                    result.unwrap_or_else(|errors| {
                        panic!("valid {name}, instantiated={instantiated}: {errors:?}; {text}")
                    });
                } else {
                    let errors =
                        result.expect_err("invalid forwarding must be checked without an instance");
                    assert_eq!(errors.len(), 1, "{name}: {errors:?}");
                    assert_eq!(
                        (errors[0].code, errors[0].stage, errors[0].message.as_str()),
                        (
                            "E0300",
                            "generics",
                            "borrow requires an owned value or forwarded reference"
                        ),
                        "{name}, instantiated={instantiated}: {errors:?}; {text}",
                    );
                    let start = text.find(borrow).unwrap();
                    assert_eq!(
                        errors[0].primary.map(|at| (at.start, at.end)),
                        Some((start, start + borrow.len())),
                        "{name}, instantiated={instantiated}: {errors:?}",
                    );
                }
            }
        }
    }
}

#[test]
fn vector_arguments_infer_generic_record_enum_and_function_parameters() {
    source_plan(
        "use alloc::vec::Vec;use alloc::vec::new;struct Holder<T:Copy>{values:Vec<T>}enum Packet<T:Copy>{Data(Vec<T>)}fn wrap<T:Copy>(values:Vec<T>)->Holder<T>{return Holder{values:values};}fn pack<T:Copy>(values:Vec<T>)->Packet<T>{return Packet::Data(values);}fn main()->i32{let holder=wrap(new::<i128>());let packet=pack(new::<u8>());return 0;}",
        |plan, _, _| {
            assert_eq!(plan.len(), 3);
            assert!(plan.nominals.emitted_records.iter().any(|id| {
                plan.nominals.records[*id].fields == [Value::Vector(Element::Concrete(Ty::I128))]
            }));
            assert!((0..plan.nominals.enum_count()).any(|id| {
                plan.nominals.payload(VariantId { enumeration: EnumId(id), index: 0 }, origin()).unwrap()
                    == Some(ValueTy::Owned(AggregateTy::Vector(Ty::U8)))
            }));
        },
    ).unwrap();
}

#[test]
fn generic_slice_indexing_and_forwarded_vector_borrows_keep_the_same_element() {
    source_plan(
        "use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::try_push;use alloc::vec::try_extend;use alloc::vec::len;fn append<T:Copy>(values:&mut Vec<T>,items:&[T])->(){try_push(&mut *values,items[0]);try_extend(&mut *values,&*items);return;}fn main()->i32{let mut values=new::<i32>();let items=[7];append(&mut values,&items);return len(&values);}",
        |plan, _, _| {
            assert_eq!(plan.len(), 2);
            for operation in [vector::Operation::New, vector::Operation::Len, vector::Operation::TryPush, vector::Operation::TryExtend] {
                assert!(plan.vector_functions().contains(&vector::Function::new(operation, Ty::I32).unwrap()));
            }
            assert_eq!(plan.vector_count(), 4);
        },
    ).unwrap();
}

#[test]
fn generic_vector_growth_returns_the_existing_byte_buffer_status_identity() {
    source_plan(
        "use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::try_push;use alloc::bytes::GrowStatus;fn push<T:Copy>(values:&mut Vec<T>,value:T)->GrowStatus{return try_push(&mut *values,value);}fn main()->i32{let mut values=new::<i32>();push(&mut values,7);return 0;}",
        |plan, index, _| {
            let grow = index.experimental_enum_id(experimental::Enumeration::Grow).unwrap();
            let mut checked = false;
            for instance in &plan.instances {
                let (key, module) = index.template_function(instance.definition).unwrap();
                let ast = index.sources().ast(module).unwrap();
                let function = &ast.functions[key.index];
                if index.sources().text(function.name).unwrap() == "push" {
                    let ty = instance.types.iter().find(|(span, _)| *span == function.result.span).unwrap().1;
                    assert_eq!(plan.nominals.parameter(ty, function.result.span).unwrap(),
                        ParameterTy::Value(ValueTy::Owned(AggregateTy::Enum(grow))));
                    checked = true;
                }
            }
            assert!(checked);
            assert_eq!(plan.vector_count(), 2);
        },
    ).unwrap();
}

#[test]
fn all_nine_vector_operations_use_only_the_requested_unit_specializations() {
    source_plan(
        "use alloc::vec::new;use alloc::vec::len;use alloc::vec::capacity;use alloc::vec::try_reserve;use alloc::vec::try_push;use alloc::vec::try_extend;use alloc::vec::try_resize;use alloc::vec::truncate;use alloc::vec::take;fn main()->i32{let mut values=new::<()>();len(&values);capacity(&values);try_reserve(&mut values,4);try_push(&mut values,());let items:[();0]=[];try_extend(&mut values,&items);try_resize(&mut values,2,());truncate(&mut values,0);let taken=take(&mut values);return len(&taken);}",
        |plan, index, _| {
            assert_eq!(plan.vector_count(), 9);
            for key in plan.vector_functions() {
                assert_eq!(key.element, Ty::Unit);
            }
            let grow = index.experimental_enum_id(experimental::Enumeration::Grow).unwrap();
            for key in plan.vector_functions() {
                if key.operation.requires_grow_status() {
                    assert_eq!(key.signature(grow).result, ValueTy::Owned(AggregateTy::Enum(grow)));
                }
            }
        },
    ).unwrap();
}

#[test]
fn vectors_stay_affine_and_owned_or_mismatched_elements_are_rejected() {
    for source in [
        "use alloc::vec::Vec;fn unused<T:Copy>(values:Vec<T>)->(){let first=values;let second=values;return;}fn main()->i32{return 0;}",
        "use alloc::vec::Vec;use alloc::vec::new;fn main()->i32{let values=new::<Vec<i32>>();return 0;}",
        "use alloc::vec::new;fn main()->i32{let values=new::<[i32;1]>();return 0;}",
        "use alloc::vec::new;fn main()->i32{let values=new();return 0;}",
        "use alloc::vec::new;use alloc::vec::try_push;fn main()->i32{let mut values=new::<u8>();try_push(&mut values,1);return 0;}",
        "use alloc::vec::new;use alloc::vec::try_push;fn main()->i32{let mut values=new::<i32>();try_push::<bool>(&mut values,true);return 0;}",
        "use alloc::vec::new;fn needs_copy<T:Copy>(value:T)->(){return;}fn main()->i32{needs_copy(new::<i32>());return 0;}",
        "use alloc::vec::new;fn read<T:Copy>(items:&[T],value:T)->(){return;}fn main()->i32{let values=new::<i32>();read(&values,true);return 0;}",
        "use alloc::bytes::new as byte_new;use alloc::vec::len;fn main()->i32{let values=byte_new();return len(&values);}",
    ] {
        let errors = source_plan(source, |_, _, _| panic!("invalid vector source accepted")).unwrap_err();
        assert_eq!(errors[0].code, "E0300", "{source}: {errors:?}");
    }
}
