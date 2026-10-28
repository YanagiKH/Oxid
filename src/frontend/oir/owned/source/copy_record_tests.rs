//! Checked Copy is explicit nominal authority, independent of source lowering.
use super::super::*;
use super::{association, budget, lower, resolve, typeck};
use crate::frontend::{
    declaration_index::{collect_originals, IndexLimits, SourceOwner, WorkMeter},
    project::{budget::Allocator, ProjectSources},
};

pub(in crate::frontend::oir::owned) const NESTED: &str = "#[derive(Copy)]struct Point{x:i32,y:i32}#[derive(Copy)]struct Holder<T:Copy>{value:T}fn score(p:Point)->i32{return p.x+p.y;}fn from(h:Holder<Point>)->i32{let Holder{value:p}=h;return score(p);}fn identity<T:Copy>(value:T)->T{return value;}fn main()->i32{let p=Point{x:20,y:1};let second=identity(p);let h=Holder::<Point>{value:p};let other=h;return score(p)+score(second)+from(h)+from(other);}";

pub(in crate::frontend::oir::owned) fn with_typed<T>(
    text: &str,
    action: impl FnOnce(&typeck::TypedOwnedProgram<'_>, &SourceMap) -> T,
) -> T {
    let project = ProjectSources::from_u8_index_test_files(&[("main.ox", text)]);
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_originals(SourceOwner::project(&project), IndexLimits::default(),
        &work, &mut allocator).unwrap().finish(&work, &mut allocator).unwrap();
    let typed = if resolve::requires_paid_source(&index) {
        resolve::type_enum_source(&index, &work, &mut allocator).unwrap()
    } else {
        typeck::check(resolve::resolve_project(&index, &work).unwrap()).unwrap()
    };
    action(&typed, project.sources())
}

pub(in crate::frontend::oir::owned) fn checked(text: &str)
    -> (SourceMap, verified::VerifiedOwnedProgram, hir::DefId)
{
    // Retain exactly the parsed map used to establish nominal/source identity.
    let mut sources = SourceMap::new();
    let id = sources.add("checked-copy.ox".into(), text.into());
    let file = sources.get(id);
    let (ast, _) = crate::frontend::parser::parse_typed_counted(file,
        crate::frontend::lexer::lex(file).unwrap(),
        crate::frontend::parser::SourceMode::ProjectCandidate,
        crate::frontend::parser::MAX_NODES, &mut Allocator::default(), &mut Default::default()).unwrap();
    let owner = SourceOwner::original(file, &ast, crate::frontend::source::SourceView::Map(&sources)).unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_originals(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap().finish(&work, &mut allocator).unwrap();
    let typed = if resolve::requires_paid_source(&index) {
        resolve::type_enum_source(&index, &work, &mut allocator).unwrap()
    } else {
        typeck::check(resolve::resolve_project(&index, &work).unwrap()).unwrap()
    };
    let entry = typed.entry().unwrap();
    let witness = verified::verify_associated(association::lower_and_associate(&typed).unwrap()).unwrap();
    drop(typed);
    drop(index);
    (sources, witness, entry)
}

#[test]
fn checked_copy_records_preserve_sources_through_lets_calls_generics_and_patterns() {
    for (source, expected) in [
        (NESTED, 84),
        ("#[derive(Copy)]struct Empty{}fn accept(value:Empty)->i32{return 21;}fn main()->i32{let x=Empty{};let y=x;return accept(x)+accept(y);}", 42),
        ("#[derive(Copy)]struct P{x:i32}fn read(p:&P,copy:P)->i32{return p.x+copy.x;}fn main()->i32{let p=P{x:21};return read(&p,p);}", 42),
        ("#[derive(Copy)]struct P{x:i32}fn main()->i32{let mut a=P{x:20};let b=a;a.x=22;return a.x+b.x;}", 42),
    ] {
        with_typed(source, |typed, map| {
            let raw = lower::lower(typed).unwrap();
            assert!(raw.records.iter().all(|record| record.is_copy));
            assert!(raw.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.statements)
                .any(|s| matches!(s.kind, OwnedInstruction::CopyInitialize { .. })));
            let witness = verified::verify_associated(association::associate(raw, typed).unwrap()).unwrap();
            assert_eq!(execute::run(&witness, typed.entry()).unwrap(), Scalar::I32(expected));
            let checked = super::program::check_typed(typed).unwrap();
            assert_eq!(checked.run(typed.entry(), map).unwrap(), Scalar::I32(expected));
        });
    }
}

fn source_error(text: &str) -> Vec<Diagnostic> {
    let project = ProjectSources::from_u8_index_test_files(&[("main.ox", text)]);
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = match collect_originals(SourceOwner::project(&project), IndexLimits::default(),
        &work, &mut allocator).unwrap().finish(&work, &mut allocator) {
        Ok(index) => index,
        Err(errors) => return errors,
    };
    let typed = if resolve::requires_paid_source(&index) {
        resolve::type_enum_source(&index, &work, &mut allocator)
    } else {
        resolve::resolve_project(&index, &work).and_then(typeck::check)
    };
    match typed {
        Err(errors) => errors,
        Ok(typed) => match super::program::check_typed(&typed) {
            Err(errors) => errors,
            Ok(_) => panic!("expected source refusal: {text}"),
        },
    }
}

#[test]
fn checked_copy_rejects_affine_fields_cycles_and_borrow_conflicts() {
    for source in [
        "struct P{x:i32}#[derive(Copy)]struct Outer{value:P}fn main()->i32{return 0;}",
        "#[derive(Copy)]struct P{value:[i32;1]}fn main()->i32{return 0;}",
        "enum E{None}#[derive(Copy)]struct P{value:E}fn main()->i32{return 0;}",
        "use alloc::bytes::ByteBuf;#[derive(Copy)]struct P{value:ByteBuf}fn main()->i32{return 0;}",
        "use std::fs::File;#[derive(Copy)]struct P{value:File}fn main()->i32{return 0;}",
        "use alloc::vec::Vec;#[derive(Copy)]struct P{value:Vec<i32>}fn main()->i32{return 0;}",
        "#[derive(Copy)]struct P{value:P}fn main()->i32{return 0;}",
        "#[derive(Copy)]struct A{value:B}#[derive(Copy)]struct B{value:A}fn main()->i32{return 0;}",
        "#[derive(Copy)]struct Holder<T>{value:T}fn main()->i32{return 0;}",
        "struct P{x:i32}fn use_value(p:P)->i32{return p.x;}fn main()->i32{let p=P{x:21};return use_value(p)+use_value(p);}",
        "#[derive(Copy)]struct P{x:i32}fn read(p:&mut P,value:P)->i32{return value.x;}fn main()->i32{let mut p=P{x:21};return read(&mut p,p);}",
    ] {
        let errors = source_error(source);
        assert!(!errors.is_empty(), "{source}");
        assert!(errors.iter().all(|e| e.code != "E0500"), "{source}: {errors:?}");
    }
}

#[test]
fn checked_copy_fields_preserve_the_generic_aggregate_extraction_fence() {
    let known = "#[derive(Copy)]struct P{x:i32}#[derive(Copy)]struct H{p:P}fn hidden<T:Copy>(h:H)->P{return h.p;}";
    for main in [
        "fn main()->i32{return 0;}",
        "fn main()->i32{let p=hidden::<i32>(H{p:P{x:42}});return p.x;}",
    ] {
        let source = format!("{known}{main}");
        let errors = source_error(&source);
        assert_eq!(errors[0].code, "E0300", "{source}: {errors:?}");
        assert_eq!(errors[0].message,
            "cannot move an owned record field in a generic function",
            "{source}: {errors:?}");
    }
    for definition in [
        "#[derive(Copy)]struct Holder<T:Copy>{value:T}fn read<T:Copy>(h:Holder<T>)->T{return h.value;}",
        "#[derive(Copy)]struct Holder<T:Copy>{value:i32}fn read<T:Copy>(h:Holder<T>)->i32{return h.value;}",
    ] {
        for (main, expected) in [
            ("fn main()->i32{return 0;}", 0),
            ("fn main()->i32{return read::<i32>(Holder::<i32>{value:42});}", 42),
        ] {
            let source = format!("{definition}{main}");
            with_typed(&source, |typed, _| {
                let raw = lower::lower(typed).unwrap();
                let witness = verified::verify_associated(
                    association::associate(raw, typed).unwrap()).unwrap();
                assert_eq!(execute::run(&witness, typed.entry()).unwrap(),
                    Scalar::I32(expected), "{source}");
            });
        }
    }
    // The opaque Copy bound remains valid, but substituting a record cannot
    // bypass the concrete field extractor's existing scalar-only boundary.
    let errors = source_error("#[derive(Copy)]struct P{x:i32}#[derive(Copy)]struct Holder<T:Copy>{value:T}fn read<T:Copy>(h:Holder<T>)->T{return h.value;}fn main()->i32{let p=read::<P>(Holder::<P>{value:P{x:42}});return p.x;}");
    assert_eq!(errors[0].code, "E0305", "{errors:?}");
    assert_eq!(errors[0].message,
        "aggregate field extraction is unavailable; move or borrow the whole root",
        "{errors:?}");
}

#[test]
fn checked_copy_bound_does_not_widen_vector_or_slice_elements() {
    for body in [
        "use alloc::vec::new;fn main()->i32{let v=new::<P>();return 0;}",
        "use alloc::vec::Vec;use alloc::vec::new;fn make<T:Copy>()->Vec<T>{return new::<T>();}fn main()->i32{let v=make::<P>();return 0;}",
        "fn count<T:Copy>(values:&[T])->i32{return values.len();}fn main()->i32{let a=[1];return count::<P>(&a);}",
    ] {
        let source = format!("#[derive(Copy)]struct P{{x:i32}}{body}");
        let errors = source_error(&source);
        assert_eq!(errors[0].code, "E0300", "{errors:?}");
        assert!(errors[0].message.contains("element"), "{errors:?}");
    }
}

fn first_copy(raw: &mut RawOwnedProgram) -> &mut OwnedInstruction {
    &mut raw.functions.iter_mut().flat_map(|f| &mut f.blocks).flat_map(|b| &mut b.statements)
        .find(|s| matches!(s.kind, OwnedInstruction::CopyInitialize { .. })).unwrap().kind
}

#[test]
fn checked_copy_raw_verifier_rejects_wrong_authority_identity_lifetime_and_destination() {
    let source = "#[derive(Copy)]struct P{x:i32}#[derive(Copy)]struct Q{x:i32}fn main()->i32{let p=P{x:21};let q=Q{x:21};let second=p;return p.x+second.x;}";
    with_typed(source, |typed, map| {
        for mutation in 0..6 {
            let mut raw = lower::lower(typed).unwrap();
            match mutation {
                0 => raw.records[0].is_copy = false,
                1 => {
                    let OwnedInstruction::CopyInitialize { destination, source } = first_copy(&mut raw) else { unreachable!() };
                    *destination = *source;
                }
                2 => {
                    let OwnedInstruction::CopyInitialize { source, .. } = first_copy(&mut raw) else { unreachable!() };
                    source.0 = usize::MAX;
                }
                3 => {
                    let OwnedInstruction::CopyInitialize { destination, source } = first_copy(&mut raw) else { unreachable!() };
                    std::mem::swap(destination, source);
                }
                4 => {
                    let owner = raw.functions[0].owners.iter().position(|owner|
                        owner.aggregate() == AggregateTy::Record(RecordId(1))
                            && matches!(owner.kind, OwnerKind::Local { .. })).unwrap();
                    let OwnedInstruction::CopyInitialize { source, .. } = first_copy(&mut raw) else { unreachable!() };
                    *source = OwnerPlaceId(owner);
                }
                5 => {
                    let block = &mut raw.functions[0].blocks[0];
                    let position = block.statements.iter().position(|statement|
                        matches!(statement.kind, OwnedInstruction::CopyInitialize { .. })).unwrap();
                    let statement = &block.statements[position];
                    let OwnedInstruction::CopyInitialize { source, .. } = statement.kind else { unreachable!() };
                    let consume = super::super::consumer_fixtures::instruction(
                        OwnedInstruction::Discard(source), statement.span);
                    block.statements.insert(position, consume);
                }
                _ => unreachable!(),
            }
            assert!(verified::verify_owned(raw, map).is_err(), "mutation={mutation}");
        }
        // Removing the source mark is rejected even if every copy is changed
        // back into a raw move that would otherwise be a legal raw program.
        let mut raw = lower::lower(typed).unwrap();
        raw.records[0].is_copy = false;
        assert!(association::associate(raw, typed).is_err());
    });
    with_typed("struct P{x:i32}fn main()->i32{let p=P{x:42};return p.x;}", |typed, map| {
        let mut raw = lower::lower(typed).unwrap();
        raw.records[0].is_copy = true;
        // A raw checked declaration can assert its own policy, but cannot
        // acquire the distinct source declaration's authenticated authority.
        assert!(verified::verify_owned(lower::lower(typed).unwrap(), map).is_ok());
        assert!(association::associate(raw, typed).is_err());
    });
}

#[test]
fn checked_copy_lowering_retains_exact_raw_budget_and_refuses_one_short() {
    with_typed(NESTED, |typed, map| {
        let usage = budget::preflight(typed, budget::Limits::DEFAULT).unwrap();
        let raw = lower::lower_with_limits(typed, budget::Limits { raw_bytes: usage.raw_bytes }).unwrap();
        assert_eq!(super::super::budget::preflight(&raw, super::super::budget::Limits::DEFAULT).unwrap(), usage.analysis);
        verified::verify_owned(raw, map).unwrap();
        assert!(lower::lower_with_limits(typed, budget::Limits { raw_bytes: usage.raw_bytes - 1 }).is_err());
    });
}
