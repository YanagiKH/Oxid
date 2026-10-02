use super::super::*;
use super::{lower, resolve, typeck};
use crate::frontend::{lexer, parser, source::SourceMap};

fn raw(text: &str) -> (SourceMap, RawOwnedProgram) {
    let mut sources = SourceMap::new();
    let id = sources.add("lower.ox".into(), text.into());
    let file = sources.get(id);
    let parsed = parser::parse_with_mode(
        file,
        lexer::lex(file).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve(file, &parsed).unwrap()).unwrap();
    let raw = lower::lower(&typed).unwrap();
    (sources, raw)
}

#[test]
fn source_lowering_normalizes_names_groups_and_cleanup_exactly() {
    let (sources, raw) =
        raw("struct T {} fn main() -> () { let x = T {}; let y = (x); y; return; }");
    let f = &raw.functions[0];
    assert_eq!(f.owners.len(), 5);
    assert_eq!(f.locals.len(), 1);
    assert_eq!(f.blocks.len(), 1);
    let operations: Vec<_> = f.blocks[0]
        .statements
        .iter()
        .map(|s| match s.kind {
            OwnedInstruction::StorageLive(o) => format!("live{}", o.0),
            OwnedInstruction::StorageEnd(o) => format!("end{}", o.0),
            OwnedInstruction::Construct { destination, .. } => format!("new{}", destination.0),
            OwnedInstruction::MoveInitialize {
                destination,
                source,
            } => format!("move{}<-{}", destination.0, source.0),
            OwnedInstruction::Discard(o) => format!("discard{}", o.0),
            OwnedInstruction::Scalar(_) => "unit".into(),
            _ => panic!("unexpected operation"),
        })
        .collect();
    assert_eq!(
        operations,
        [
            "live0", "new0", "live1", "move1<-0", "end0", "live2", "move2<-1", "live3", "move3<-2",
            "end2", "live4", "move4<-3", "discard4", "end4", "unit", "end3", "end1"
        ]
    );
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(0))).unwrap(),
        Scalar::Unit
    );
}

#[test]
fn source_lowering_preserves_interleaved_parameter_positions() {
    let (sources, raw) =
        raw("struct T {} fn f(a: i32, x: T, p: &T, b: bool, y: T) -> () { return; }");
    let f = &raw.functions[0];
    assert!(matches!(
        f.parameters.as_slice(),
        [
            ParameterBinding::Scalar(LocalId(0)),
            ParameterBinding::Owned(OwnerPlaceId(0)),
            ParameterBinding::Reference(ReferenceParamId(0)),
            ParameterBinding::Scalar(LocalId(1)),
            ParameterBinding::Owned(OwnerPlaceId(1))
        ]
    ));
    assert_eq!(f.owners[0].kind, OwnerKind::Parameter { position: 1 });
    assert_eq!(f.owners[1].kind, OwnerKind::Parameter { position: 4 });
    assert_eq!(f.references[0].position, 2);
    assert_eq!(f.locals.len(), 3);
    verified::verify_owned(raw, &sources).unwrap();
}

#[test]
fn source_lowering_keeps_return_result_alive_and_ends_parameter() {
    let (sources, raw) = raw("struct T {} fn relay(x: T) -> T { return x; }");
    let b = &raw.functions[0].blocks[0];
    assert_eq!(b.statements.len(), 3);
    assert!(matches!(
        b.statements[2].kind,
        OwnedInstruction::StorageEnd(OwnerPlaceId(0))
    ));
    assert!(matches!(
        b.terminator.as_ref().unwrap().kind,
        OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(1))
    ));
    verified::verify_owned(raw, &sources).unwrap();
}

#[test]
fn source_lowering_keeps_outer_preparation_through_lazy_argument_cfg() {
    let (sources, raw) = raw("struct T { x: i32 } fn f(p: &T, b: bool) -> bool { return b; } fn main() -> bool { let x = T { x: 0 }; return f(&x, false && (x.x == 0)); }");
    let f = &raw.functions[1];
    assert_eq!(f.blocks.len(), 4);
    assert_eq!(f.calls.len(), 1);
    assert_eq!(f.loans.len(), 1);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(1))).unwrap(),
        Scalar::Bool(false)
    );
}

#[test]
fn source_lowering_excludes_later_bindings_from_early_continue_cleanup() {
    let (sources, raw) = raw("struct T {} fn main() -> i32 { let mut n = 0; while n < 2 { if n == 0 { n = n + 1; continue; } let later = T {}; n = n + 1; } return n; }");
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(0))).unwrap(),
        Scalar::I32(2)
    );
}

#[test]
fn scalar_names_groups_bindings_and_places_keep_the_canonical_snapshots() {
    let (sources, raw) = raw("struct T {} fn f(a: i32) -> i32 { let x = (a); let mut y = x; y = y + 1; return y; } fn main() -> i32 { return f(4); }");
    let f = &raw.functions[0];
    assert_eq!(f.locals.len(), 9);
    assert_eq!(f.places.len(), 1);
    assert_eq!(f.blocks[0].statements.len(), 10);
    assert!(matches!(
        f.blocks[0].statements[0].kind,
        OwnedInstruction::Scalar(Statement::Assign(Assign {
            value: Rvalue::Copy(_),
            ..
        }))
    ));
    assert!(matches!(
        f.blocks[0].statements[1].kind,
        OwnedInstruction::Scalar(Statement::Assign(Assign {
            value: Rvalue::Copy(_),
            ..
        }))
    ));
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(1))).unwrap(),
        Scalar::I32(5)
    );
}

#[test]
fn literal_fields_snapshot_nested_calls_in_written_order() {
    let (sources, raw) = raw("struct T { a: i32, b: i32 } fn next(p: &mut T) -> i32 { p.a = p.a + 1; return p.a; } fn main() -> i32 { let mut state = T { a: 0, b: 0 }; let result = T { b: next(&mut state), a: next(&mut state) }; return result.a * 10 + result.b; }");
    let constructions: Vec<_> = raw.functions[1]
        .blocks
        .iter()
        .flat_map(|b| &b.statements)
        .filter_map(|s| match &s.kind {
            OwnedInstruction::Construct { fields, .. } => {
                Some(fields.iter().map(|(id, _)| id.index).collect::<Vec<_>>())
            }
            _ => None,
        })
        .collect();
    assert_eq!(constructions, [vec![0, 1], vec![1, 0]]);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(1))).unwrap(),
        Scalar::I32(21)
    );
}

#[test]
fn replacement_field_store_and_nested_owned_results_end_at_their_consumers() {
    let (sources, raw) = raw("struct T { value: i32 } fn relay(x: T) -> T { return x; } fn next(p: &mut T) -> i32 { p.value = p.value + 1; return p.value; } fn sum(a: T, b: T) -> i32 { return a.value + b.value; } fn main() -> i32 { let mut x = T { value: 2 }; let y = T { value: 7 }; x = x; x = relay(x); x.value = next(&mut x); return sum(relay(x), relay(y)); }");
    let main = &raw.functions[3];
    let outer = main
        .calls
        .iter()
        .position(|c| c.target == hir::DefId(2))
        .unwrap();
    let children: Vec<_> = main
        .calls
        .iter()
        .filter_map(|c| c.parent)
        .filter(|(parent, _)| parent.0 == outer)
        .map(|(_, argument)| argument)
        .collect();
    assert_eq!(children, [0, 1]);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(3))).unwrap(),
        Scalar::I32(10)
    );
}

#[test]
fn canonical_loop_owner_sites_restart_after_continue_and_fallthrough() {
    let (sources, raw) = raw("struct T {} fn main() -> i32 { let mut n = 0; while n < 3 { let local = T {}; n = n + 1; if n == 1 { continue; } local; } return n; }");
    let f = &raw.functions[0];
    assert_eq!(f.owners.len(), 3);
    for owner in 0..3 {
        assert_eq!(
            f.blocks
                .iter()
                .flat_map(|b| &b.statements)
                .filter(|s| matches!(s.kind, OwnedInstruction::StorageLive(id) if id.0 == owner))
                .count(),
            1
        );
    }
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(0))).unwrap(),
        Scalar::I32(3)
    );
}

#[test]
fn both_owned_return_arms_have_no_join_and_preserve_the_selected_result() {
    let (sources, raw) = raw("struct T { value: i32 } fn choose(x: T, flag: bool) -> T { if flag { return x; } else { return x; } } fn main() -> i32 { let x = choose(T { value: 8 }, false); return x.value; }");
    assert_eq!(raw.functions[0].blocks.len(), 3);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(1))).unwrap(),
        Scalar::I32(8)
    );
}

#[test]
fn wide_literals_and_calls_use_cursor_frames_instead_of_expanding_the_stack() {
    let fields = (0..1024)
        .map(|i| format!("f{i}: i32"))
        .collect::<Vec<_>>()
        .join(",");
    let values = (0..1024)
        .map(|i| format!("f{i}: {i}"))
        .collect::<Vec<_>>()
        .join(",");
    let (sources, program) = raw(&format!("struct Wide {{ {fields} }} fn main() -> i32 {{ let x = Wide {{ {values} }}; return x.f1023; }}"));
    let witness = verified::verify_owned(program, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(0))).unwrap(),
        Scalar::I32(1023)
    );
    let params = (0..256)
        .map(|i| format!("p{i}: &T"))
        .collect::<Vec<_>>()
        .join(",");
    let args = vec!["&x"; 256].join(",");
    let (sources, program) = raw(&format!("struct T {{}} fn many({params}) -> () {{ return; }} fn main() -> () {{ let x = T {{}}; many({args}); return; }}"));
    assert_eq!(program.functions[0].references.len(), 256);
    assert_eq!(program.functions[1].loans.len(), 256);
    let witness = verified::verify_owned(program, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(1))).unwrap(),
        Scalar::Unit
    );
}

#[test]
fn maximum_owned_group_depth_keeps_one_temporary_and_no_group_events() {
    let text = format!(
        "struct T {{}} fn main() -> () {{ let x = {}T {{}}{}; return; }}",
        "(".repeat(63),
        ")".repeat(63)
    );
    let (sources, program) = raw(&text);
    assert_eq!(program.functions[0].owners.len(), 2);
    assert_eq!(program.functions[0].blocks[0].statements.len(), 7);
    verified::verify_owned(program, &sources).unwrap();
}
