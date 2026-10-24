use super::*;
use crate::frontend::{
    declaration_index::{collect_originals, IndexLimits, SourceOwner, WorkMeter},
    lexer,
    source::{SourceFileId, SourceMap, SourceView},
};

fn parse_generic(file: &SourceFile, allocator: &mut Allocator) -> Result<Program, Vec<Diagnostic>> {
    let tokens =
        lexer::lex_current_with_allocator(file, lexer::MAX_TOKENS, &mut Allocator::default())
            .unwrap();
    parse_typed_counted(
        file,
        tokens,
        SourceMode::ProjectCandidate,
        MAX_NODES,
        allocator,
        &mut Default::default(),
    )
    .map(|(program, _)| program)
}

#[test]
fn nominal_applications_keep_nested_types_and_enum_members_as_separate_origins() {
    let mut map = SourceMap::new();
    let id=map.add("nominal.ox".into(),"struct Pair<T,U>{first:T,second:U}enum Option<T>{None,Some(T)}fn read<T>(pair:&Pair<T,Option<i32>>)->Option<T>{let value:Option<Pair<T,i32>>=Option::<Pair<T,i32>>::None;return Option::<T>::Some(pair.first);}".into());
    let file = map.get(id);
    let mut allocator = Allocator::default();
    let program = parse_generic(file, &mut allocator).unwrap();
    assert!(program.belongs_to(file));
    assert!(program
        .generic_calls()
        .windows(2)
        .all(|p| p[0].callee.start < p[1].callee.start));
    let TypeSyntaxKind::AppliedReference { application, .. } =
        program.functions[0].params[0].ty.kind
    else {
        panic!("expected applied reference")
    };
    let app = program.generic_application(application).unwrap();
    assert_eq!(app.context, GenericArgumentContext::Type);
    assert_eq!(file.text_at(app.callee), "Pair");
    assert_eq!(app.arguments.len(), 2);
    let TypeSyntaxKind::Applied(nested) = app.arguments[1].kind else {
        panic!("expected nested application")
    };
    assert_eq!(
        file.text_at(program.generic_application(nested).unwrap().callee),
        "Option"
    );
    let variants = program
        .expressions
        .iter()
        .filter_map(|expr| {
            if let ExprKind::GenericVariant {
                application,
                variant,
                ..
            } = expr.kind
            {
                Some((application, variant))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(variants.len(), 2);
    assert_eq!(file.text_at(variants[0].1), "None");
    assert_eq!(file.text_at(variants[1].1), "Some");
    for (application, _) in variants {
        assert_eq!(
            program.generic_application(application).unwrap().context,
            GenericArgumentContext::Expression
        );
    }
}

#[test]
fn nominal_application_syntax_refuses_each_allocation_failure_and_excessive_depth() {
    let mut map = SourceMap::new();
    let id=map.add("nominal-alloc.ox".into(),"struct Pair<T>{value:T}fn pass<T>(value:Pair<Pair<T>>)->Pair<T>{let n=Option::<Pair<T>>::Some(value);return value;}".into());
    let file = map.get(id);
    let mut allocator = Allocator::default();
    parse_generic(file, &mut allocator).unwrap();
    for failure in 1..=allocator.attempts {
        let mut allocator = Allocator {
            fail_at: Some(failure),
            ..Default::default()
        };
        assert_eq!(
            parse_generic(file, &mut allocator).unwrap_err()[0].code,
            "E0400",
            "allocation {failure}"
        );
    }
    let nested = format!(
        "{}i32{}",
        "Pair<".repeat(MAX_NESTING + 1),
        ">".repeat(MAX_NESTING + 1)
    );
    let mut map = SourceMap::new();
    let id = map.add(
        "depth.ox".into(),
        format!("fn main(value:{nested})->(){{return;}}"),
    );
    let errors = parse_generic(map.get(id), &mut Allocator::default()).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.code == "E0400" && e.message.contains("nesting")),
        "{errors:?}"
    );
}

#[test]
fn generic_header_origins_and_every_allocation_failure_are_checked() {
    let mut map = SourceMap::new();
    let file = map.add("generic.ox".into(), "fn identity < T /* bound */ : Copy, U, > (value:T,other:U)->T{return value;}fn main()->i32{return identity(42,true);}".into());
    let file = map.get(file);
    let mut allocator = Allocator::default();
    let program = parse_generic(file, &mut allocator).unwrap();
    assert!(program.belongs_to(file));
    let parameters = program.generic_parameters(program.functions[0].name);
    assert_eq!(parameters.len(), 2);
    assert_eq!(file.text_at(parameters[0].span), "T /* bound */ : Copy");
    assert!(allocator
        .trace
        .iter()
        .any(|event| event.kind == "generic syntax owner"));
    for failure in 1..=allocator.attempts {
        let mut allocator = Allocator {
            fail_at: Some(failure),
            ..Default::default()
        };
        let diagnostics = parse_generic(file, &mut allocator).unwrap_err();
        assert_eq!(
            diagnostics[0].code, "E0400",
            "failure {failure}: {diagnostics:?}"
        );
    }
}

#[test]
fn coherent_generic_signature_and_body_forgeries_cannot_construct_an_index() {
    for body in [false, true] {
        let mut map = SourceMap::new();
        let id = map.add(
            "generic.ox".into(),
            "fn identity<T>(value:T)->T{return value;}fn main()->i32{identity(42);return 0;}"
                .into(),
        );
        let file = map.get(id);
        let mut program = parse_generic(file, &mut Allocator::default()).unwrap();
        if body {
            // A well-typed unit-returning function with original valid spans.
            program.expressions[0].kind = ExprKind::Unit;
            program.functions[0].result.kind = TypeSyntaxKind::Unit;
        } else {
            // Coherently erase T from both signature and actual argument.
            program.functions[0].params[0].ty.kind = TypeSyntaxKind::Unit;
            program.functions[0].result.kind = TypeSyntaxKind::Unit;
            let argument = program
                .expressions
                .iter_mut()
                .find(|expression| matches!(expression.kind, ExprKind::Number { .. }))
                .unwrap();
            argument.kind = ExprKind::Unit;
        }
        let owner = SourceOwner::original(file, &program, SourceView::Map(&map)).unwrap();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let facts =
            collect_originals(owner, IndexLimits::default(), &work, &mut allocator).unwrap();
        let errors = facts.finish(&work, &mut allocator).unwrap_err();
        assert_eq!(errors[0].code, "E0500");
    }
}

#[test]
fn malformed_headers_and_parameter_limits_report_errors() {
    for header in ["<>", "<T:", "<T U>", "<T,,U>"] {
        let mut map = SourceMap::new();
        let id = map.add(
            "bad.ox".into(),
            format!("fn f{header}(value:i32)->i32{{return value;}}"),
        );
        assert!(parse_generic(map.get(id), &mut Allocator::default()).is_err());
    }
    let mut map = SourceMap::new();
    let names = (0..=MAX_PARAMS)
        .map(|i| format!("T{i}"))
        .collect::<Vec<_>>()
        .join(",");
    map.add(
        "too-many.ox".into(),
        format!("fn f<{names}>()->(){{return;}}"),
    );
    assert_eq!(
        parse_generic(map.get(SourceFileId(0)), &mut Allocator::default()).unwrap_err()[0].code,
        "E0400"
    );
}

#[test]
fn explicit_type_arguments_have_source_origins_and_fallible_storage() {
    let mut map = SourceMap::new();
    let id=map.add("explicit.ox".into(),"fn answer<T>()->i32{return 42;}fn main()->i32{return answer :: < /* retained */ i32, > ();}".into());
    let file = map.get(id);
    let mut allocator = Allocator::default();
    let program = parse_generic(file, &mut allocator).unwrap();
    assert!(program.belongs_to(file));
    let calls = program.generic_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(file.text_at(calls[0].callee), "answer");
    assert_eq!(file.text_at(calls[0].span), ":: < /* retained */ i32, >");
    assert_eq!(file.text_at(calls[0].arguments[0].span), "i32");
    for failure in 1..=allocator.attempts {
        let mut injected = Allocator {
            fail_at: Some(failure),
            ..Default::default()
        };
        assert_eq!(
            parse_generic(file, &mut injected).unwrap_err()[0].code,
            "E0400",
            "allocation {failure}"
        );
    }
    let owner = SourceOwner::original(file, &program, SourceView::Map(&map)).unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    collect_originals(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
    for failure in 1..=allocator.attempts {
        let mut injected = Allocator {
            fail_at: Some(failure),
            ..Default::default()
        };
        let work = WorkMeter::default();
        let error = match collect_originals(owner, IndexLimits::default(), &work, &mut injected) {
            Ok(facts) => facts.finish(&work, &mut injected).unwrap_err().remove(0),
            Err(error) => *error,
        };
        assert_eq!(error.code, "E0400", "allocation {failure}: {error:?}");
    }
}

#[test]
fn explicit_argument_call_syntax_is_authenticated_before_instantiation() {
    let mut map = SourceMap::new();
    let id=map.add("explicit-forgery.ox".into(),"fn answer<T>()->i32{return 42;}fn other<T>()->i32{return 7;}fn main()->i32{return answer::<i32>();}".into());
    let file = map.get(id);
    let mut program = parse_generic(file, &mut Allocator::default()).unwrap();
    let other = program.functions[1].name;
    let call = program
        .expressions
        .iter_mut()
        .find(|expr| matches!(expr.kind, ExprKind::Call { .. }))
        .unwrap();
    let ExprKind::Call { callee, .. } = &mut call.kind else {
        unreachable!()
    };
    *callee = ItemPath::Unqualified(other);
    let owner = SourceOwner::original(file, &program, SourceView::Map(&map)).unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let errors = collect_originals(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap_err();
    assert_eq!(errors[0].code, "E0500");
}

#[test]
fn generic_index_construction_never_survives_an_injected_allocation_failure() {
    let mut map = SourceMap::new();
    let id = map.add(
        "generic.ox".into(),
        "fn identity<T>(value:T)->T{return value;}fn main()->i32{return identity(42);}".into(),
    );
    let file = map.get(id);
    let program = parse_generic(file, &mut Allocator::default()).unwrap();
    let owner = SourceOwner::original(file, &program, SourceView::Map(&map)).unwrap();
    let mut allocator = Allocator::default();
    let work = WorkMeter::default();
    collect_originals(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
    assert!(allocator
        .trace
        .iter()
        .any(|event| event.kind == "generic instantiation owner"));
    for failure in 1..=allocator.attempts {
        let work = WorkMeter::default();
        let mut allocator = Allocator {
            fail_at: Some(failure),
            ..Default::default()
        };
        let error = match collect_originals(owner, IndexLimits::default(), &work, &mut allocator) {
            Ok(facts) => facts.finish(&work, &mut allocator).unwrap_err().remove(0),
            Err(error) => *error,
        };
        assert_eq!(error.code, "E0400", "allocation {failure}: {error:?}");
    }
}

#[test]
fn generic_metadata_does_not_open_original_adapter_project_syntax() {
    for prefix in [
        "pub fn visible()->(){return;}",
        "mod helper;",
        "use crate::unknown;",
        "pub struct N{x:i32}",
        "struct N{pub x:i32}",
    ] {
        let mut map = SourceMap::new();
        let id=map.add("closed.ox".into(),format!("{prefix}fn identity<T>(value:T)->T{{return value;}}fn main()->i32{{return identity(42);}}"));
        let file = map.get(id);
        let program = parse_generic(file, &mut Allocator::default()).unwrap();
        let owner = SourceOwner::original(file, &program, SourceView::Map(&map)).unwrap();
        assert!(
            collect_originals(
                owner,
                IndexLimits::default(),
                &WorkMeter::default(),
                &mut Allocator::default()
            )
            .is_err(),
            "{prefix}"
        );
    }
}

#[test]
fn nominal_index_allocation_and_work_exhaustion_never_publish_a_partial_plan() {
    let mut map = SourceMap::new();
    let id=map.add("nominal-resource.ox".into(), "struct Cell<T>{value:T}enum Option<T>{None,Some(T)}fn wrap<T>(value:T)->Option<Cell<T>>{return Option::<Cell<T>>::Some(Cell::<T>{value:value});}fn main()->i32{let value=wrap(42);match value{Option::None=>{return 1;},Option::Some(cell)=>{return cell.value;},}}".into());
    let file = map.get(id);
    let program = parse_generic(file, &mut Allocator::default()).unwrap();
    let owner = SourceOwner::original(file, &program, SourceView::Map(&map)).unwrap();
    let mut baseline = Allocator::default();
    let work = WorkMeter::default();
    let index = collect_originals(owner, IndexLimits::default(), &work, &mut baseline)
        .unwrap()
        .finish(&work, &mut baseline)
        .unwrap();
    assert_eq!(index.record_count(), 1);
    assert_eq!(index.source_enum_count(), 1);
    assert_eq!(index.source_function_count(), 2);
    for failure in 1..=baseline.attempts {
        let work = WorkMeter::default();
        let mut allocator = Allocator {
            fail_at: Some(failure),
            ..Default::default()
        };
        let error = match collect_originals(owner, IndexLimits::default(), &work, &mut allocator) {
            Ok(facts) => facts.finish(&work, &mut allocator).unwrap_err().remove(0),
            Err(error) => *error,
        };
        assert_eq!(error.code, "E0400", "allocation {failure}: {error:?}");
        assert_eq!(allocator.attempts, failure);
    }
    let used = work.used();
    let work = WorkMeter::new(used - 1);
    let mut allocator = Allocator::default();
    let error = match collect_originals(owner, IndexLimits::default(), &work, &mut allocator) {
        Ok(facts) => facts.finish(&work, &mut allocator).unwrap_err().remove(0),
        Err(error) => *error,
    };
    assert_eq!(error.code, "E0400");
}

#[test]
fn complete_record_patterns_preserve_binding_origins_and_fallible_storage() {
    let mut map = SourceMap::new();
    let id=map.add("record-pattern.ox".into(),"struct Cell<T>{value:T}fn take<T>(cell:Cell<T>)->T{let Cell{value:inner,}=cell;return inner;}fn main()->i32{return take(Cell::<i32>{value:42});}".into());
    let source = map.get(id);
    let mut allocator = Allocator::default();
    let program = parse_generic(source, &mut allocator).unwrap();
    assert!(program.belongs_to(source));
    let StmtKind::DestructureRecord { record, fields, .. } =
        &program.functions[0].blocks[0].body[0].kind
    else {
        panic!("expected record pattern");
    };
    assert_eq!(
        source.text_at(program.item_path_span(*record).unwrap()),
        "Cell"
    );
    assert_eq!(fields.len(), 1);
    assert_eq!(source.text_at(fields[0].field), "value");
    assert_eq!(source.text_at(fields[0].binding), "inner");
    for failure in 1..=allocator.attempts {
        let mut allocator = Allocator {
            fail_at: Some(failure),
            ..Default::default()
        };
        assert_eq!(
            parse_generic(source, &mut allocator).unwrap_err()[0].code,
            "E0400"
        );
    }
}
