//! Raw storage proofs are independent of source construction and SSA values.
use super::*;
use crate::frontend::{lexer, parser};

fn raw(text: &str) -> (SourceMap, Program) {
    let mut sources = SourceMap::new();
    let id = sources.add("mutable-雪.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
    let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
    (sources, lower::lower(&typed).unwrap())
}
fn simple() -> (SourceMap, Program) {
    raw("// 🦀\r\nfn main() -> i32 { let mut x = 1; x = 2; return x; }")
}
fn reject(sources: &SourceMap, program: Program, expected: FailureKind) {
    let error = verify::verify(program, sources).unwrap_err();
    assert_eq!(error.kind, expected);
    assert_eq!(error.stage, "oir-verify");
    assert_eq!(error.diagnostic(sources).code, "E0500");
}
#[test]
fn place_ids_storage_and_ssa_snapshots_are_separate_for_every_scalar() {
    for (ty, a, b, expected) in [
        ("i32", "1", "2", Scalar::I32(1)),
        ("bool", "true", "false", Scalar::Bool(true)),
        ("()", "()", "()", Scalar::Unit),
    ] {
        let text = format!(
            "fn main() -> {ty} {{ let mut x = {a}; let saved = x; x = {b}; return saved; }}"
        );
        let (s, p) = raw(&text);
        let f = &p.functions[0];
        assert_eq!(f.places.len(), 1);
        assert_eq!(
            f.locals
                .iter()
                .filter(|l| l.kind == LocalKind::Binding)
                .count(),
            1
        );
        assert_eq!(
            f.blocks[0]
                .statements
                .iter()
                .filter(|s| matches!(s, Statement::Initialize { .. }))
                .count(),
            1
        );
        assert_eq!(
            f.blocks[0]
                .statements
                .iter()
                .filter(|s| matches!(s, Statement::Store { .. }))
                .count(),
            1
        );
        let v = verify::verify(p, &s).unwrap();
        assert_eq!(v.run(Some(hir::DefId(0))), Ok(expected));
    }
}
#[test]
fn every_place_needs_one_canonical_initialization_and_ssa_stays_unique() {
    let (s, base) = simple();
    let mut p = base.clone();
    p.functions[0].blocks[0].statements.remove(1);
    reject(&s, p, FailureKind::Uninitialized);
    let mut p = base.clone();
    let init = p.functions[0].blocks[0].statements[1].clone();
    p.functions[0].blocks[0].statements.push(init);
    reject(&s, p, FailureKind::AlreadyInitialized);
    let mut p = base.clone();
    let extra = p.functions[0].places[0].clone();
    p.functions[0].places.push(extra);
    reject(&s, p, FailureKind::Uninitialized);
    let mut p = base.clone();
    let duplicate = p.functions[0].blocks[0].statements[0].clone();
    p.functions[0].blocks[0].statements.push(duplicate);
    reject(&s, p, FailureKind::AlreadyInitialized);
    for order in [[0, 3, 2, 1, 4], [0, 2, 3, 1, 4], [0, 4, 1, 2, 3]] {
        let mut p = base.clone();
        let f = &mut p.functions[0];
        f.blocks[0].statements = order
            .iter()
            .map(|i| f.blocks[0].statements[*i].clone())
            .collect();
        reject(&s, p, FailureKind::Uninitialized);
    }
    let mut p = base.clone();
    let f = &mut p.functions[0];
    if let Statement::Initialize { value, .. } = &mut f.blocks[0].statements[1] {
        value.local = LocalId(2);
    }
    reject(&s, p, FailureKind::Uninitialized);
}
#[test]
fn place_access_ids_and_rhs_value_ids_are_independently_checked() {
    for case in 0..6 {
        let (s, mut p) = simple();
        let b = &mut p.functions[0].blocks[0];
        match case {
            0 | 1 => {
                let Statement::Initialize { place, value, .. } = &mut b.statements[1] else {
                    panic!()
                };
                if case == 0 {
                    place.id = PlaceId(usize::MAX);
                } else {
                    value.local = LocalId(usize::MAX);
                }
            }
            2 | 3 => {
                let Statement::Store { place, value, .. } = &mut b.statements[3] else {
                    panic!()
                };
                if case == 2 {
                    place.id = PlaceId(usize::MAX);
                } else {
                    value.local = LocalId(usize::MAX);
                }
            }
            4 => {
                b.statements[4].assignment_mut().value = Rvalue::Load(Place {
                    id: PlaceId(usize::MAX),
                    span: b.span,
                })
            }
            _ => b.statements[4].assignment_mut().destination = LocalId(usize::MAX),
        }
        reject(
            &s,
            p,
            if matches!(case, 0 | 2 | 4) {
                FailureKind::InvalidPlace
            } else {
                FailureKind::InvalidLocal
            },
        );
    }
}
#[test]
fn initialization_store_and_load_enforce_the_fixed_scalar_type() {
    for wrong in [hir::Ty::Bool, hir::Ty::Unit] {
        for case in 0..4 {
            let (s, mut p) = simple();
            let f = &mut p.functions[0];
            match case {
                0 => f.places[0].ty = wrong,
                1 | 2 => {
                    let index = if case == 1 { 0 } else { 2 };
                    let a = f.blocks[0].statements[index].assignment_mut();
                    f.locals[a.destination.0].ty = wrong;
                    a.value = if wrong == hir::Ty::Bool {
                        Rvalue::Bool(true)
                    } else {
                        Rvalue::Unit
                    };
                }
                _ => f.locals[2].ty = wrong,
            }
            reject(&s, p, FailureKind::TypeMismatch);
        }
    }
}
#[test]
fn declaration_target_operator_rhs_and_load_origins_are_all_validated() {
    for case in 0..9 {
        let (s, mut p) = simple();
        let f = &mut p.functions[0];
        let bad = Span {
            file: f.span.file,
            start: 4,
            end: 5,
        }; // inside UTF-8 crab
        match case {
            0 => f.places[0].span = bad,
            1..=3 => {
                let Statement::Initialize { place, value, span } = &mut f.blocks[0].statements[1]
                else {
                    panic!()
                };
                match case {
                    1 => place.span = bad,
                    2 => value.span = bad,
                    _ => *span = bad,
                }
            }
            4..=7 => {
                let Statement::Store {
                    place,
                    value,
                    operator_span,
                    span,
                } = &mut f.blocks[0].statements[3]
                else {
                    panic!()
                };
                match case {
                    4 => place.span = bad,
                    5 => value.span = bad,
                    6 => *operator_span = bad,
                    _ => *span = bad,
                }
            }
            _ => {
                let Rvalue::Load(place) = &mut f.blocks[0].statements[4].assignment_mut().value
                else {
                    panic!()
                };
                place.span = bad;
            }
        }
        reject(&s, p, FailureKind::InvalidSpan);
    }
    let text = "// 🦀\r\nfn main() -> i32 { let /*雪*/ mut x = 1; x /*é*/ = 2; return x; }";
    let (s, p) = raw(text);
    let f = &p.functions[0];
    let slice = |span: Span| &s.get(span.file).text()[span.start..span.end];
    assert_eq!(slice(f.places[0].span), "x");
    let Statement::Initialize { place, value, span } = &f.blocks[0].statements[1] else {
        panic!()
    };
    assert_eq!(
        (slice(place.span), slice(value.span), slice(*span)),
        ("x", "1", "let /*雪*/ mut x = 1;")
    );
    let Statement::Store {
        place,
        value,
        operator_span,
        span,
    } = &f.blocks[0].statements[3]
    else {
        panic!()
    };
    assert_eq!(
        (
            slice(place.span),
            slice(value.span),
            slice(*operator_span),
            slice(*span)
        ),
        ("x", "2", "=", "x /*é*/ = 2;")
    );
    let Rvalue::Load(place) = f.blocks[0].statements[4].assignment().value else {
        panic!()
    };
    assert_eq!(slice(place.span), "x");
    verify::verify(p, &s).unwrap();
}

fn graph(edges: &[Vec<usize>], init: usize, access: usize) -> (SourceMap, Program) {
    let mut sources = SourceMap::new();
    let id = sources.add("graph.ox".into(), "x".into());
    let span = sources.get(id).span(0, 1);
    let value = Operand {
        local: LocalId(0),
        span,
    };
    let place = Place {
        id: PlaceId(0),
        span,
    };
    let mut blocks = Vec::new();
    for (i, successors) in edges.iter().enumerate() {
        let mut statements = vec![];
        if i == init {
            statements.push(Statement::Initialize { place, value, span });
        }
        if i == access {
            statements.push(Statement::Store {
                place,
                value,
                operator_span: span,
                span,
            });
            statements.push(Statement::Assign(Assign {
                destination: LocalId(1),
                value: Rvalue::Load(place),
                span,
            }));
        }
        let kind = match successors.as_slice() {
            [] => TerminatorKind::Return(value),
            [target] => TerminatorKind::Goto {
                target: BlockId(*target),
            },
            [left, right] => TerminatorKind::Branch {
                condition: value,
                then_block: BlockId(*left),
                else_block: BlockId(*right),
            },
            _ => panic!(),
        };
        blocks.push(BasicBlock {
            merge: None,
            span,
            statements,
            terminator: Some(Terminator { kind, span }),
        });
    }
    (
        sources,
        Program {
            functions: vec![Function {
                id: hir::DefId(0),
                span,
                result: hir::Ty::Bool,
                param_count: 1,
                locals: vec![
                    LocalDecl {
                        ty: hir::Ty::Bool,
                        kind: LocalKind::Parameter,
                        span,
                    },
                    LocalDecl {
                        ty: hir::Ty::Bool,
                        kind: LocalKind::Temporary,
                        span,
                    },
                ],
                places: vec![PlaceDecl {
                    ty: hir::Ty::Bool,
                    span,
                }],
                entry: BlockId(0),
                blocks,
            }],
        },
    )
}
fn reachable_without(edges: &[Vec<usize>], removed: Option<usize>) -> Vec<bool> {
    let mut reached = vec![false; edges.len()];
    let mut pending = vec![0];
    while let Some(at) = pending.pop() {
        if Some(at) == removed || reached[at] {
            continue;
        }
        reached[at] = true;
        pending.extend(edges[at].iter().copied());
    }
    reached
}
#[test]
fn place_dominance_matches_independent_path_removal_on_small_dags() {
    let mut checks = 0;
    for n in 1..=6 {
        let pairs: Vec<_> = (0..n)
            .flat_map(|a| (a + 1..n).map(move |b| (a, b)))
            .collect();
        for mask in 0usize..(1 << pairs.len()) {
            let mut edges = vec![vec![]; n];
            for (bit, (a, b)) in pairs.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    edges[*a].push(*b);
                }
            }
            if edges.iter().any(|e| e.len() > 2) || reachable_without(&edges, None).contains(&false)
            {
                continue;
            }
            for init in 0..n {
                let bypass = reachable_without(&edges, Some(init));
                for (access, bypassed) in bypass.iter().enumerate() {
                    let expected = !bypassed;
                    let (s, p) = graph(&edges, init, access);
                    for mode in 0..3 {
                        let mut access_program = p.clone();
                        access_program.functions[0].blocks[access]
                            .statements
                            .retain(|statement| match mode {
                                0 => !matches!(statement, Statement::Assign(_)), // Store alone.
                                1 => !matches!(statement, Statement::Store { .. }), // Load alone.
                                _ => true,                                       // Both in order.
                            });
                        for reverse in [false, true] {
                            let mut p = access_program.clone();
                            if reverse {
                                super::cfg_tests::permute(
                                    &mut p,
                                    &(0..n).rev().collect::<Vec<_>>(),
                                );
                            }
                            let result = verify::verify(p, &s);
                            assert_eq!(result.is_ok(), expected, "edges={edges:?}, init={init}, use={access}, mode={mode}, reverse={reverse}: {result:?}");
                            if let Err(error) = result {
                                assert_eq!(error.kind, FailureKind::Uninitialized);
                            }
                            checks += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checks, 170_490);
}
#[test]
fn slot_and_instruction_caps_include_places_initialization_and_stores() {
    let (s, mut p) = graph(&[vec![]], 0, 0);
    let f = &mut p.functions[0];
    f.locals.resize(MAX_LOCALS - 1, f.locals[1].clone());
    verify::verify(p.clone(), &s).unwrap();
    let extra = p.functions[0].places[0].clone();
    p.functions[0].places.push(extra);
    reject(&s, p, FailureKind::ResourceLimit("locals"));
    let (s, mut p) = graph(&[vec![]], 0, 0);
    let f = &mut p.functions[0];
    let store = f.blocks[0].statements[1].clone();
    f.blocks[0]
        .statements
        .resize(MAX_ASSIGNMENTS, store.clone());
    verify::verify(p.clone(), &s).unwrap();
    p.functions[0].blocks[0].statements.push(store);
    reject(&s, p, FailureKind::ResourceLimit("assignments"));
}

#[test]
fn call_results_are_storable_only_after_the_call_continuation() {
    let (s, base) = raw("fn main() -> i32 { let mut x = 1; x = id(x); return x; } fn id(n: i32) -> i32 { return n; }");
    verify::verify(base.clone(), &s).unwrap();
    let mut p = base.clone();
    let f = &mut p.functions[0];
    let (from, destination, continuation) = f
        .blocks
        .iter()
        .enumerate()
        .find_map(|(i, b)| {
            if let TerminatorKind::Call {
                destination,
                continuation,
                ..
            } = b.terminator.as_ref().unwrap().kind
            {
                Some((i, destination, continuation))
            } else {
                None
            }
        })
        .unwrap();
    let store = f.blocks[continuation.0].statements.remove(0);
    f.blocks[from].statements.push(store);
    reject(&s, p, FailureKind::Uninitialized);
    let mut p = base;
    let f = &mut p.functions[0];
    f.blocks[continuation.0].statements[1]
        .assignment_mut()
        .destination = destination;
    reject(&s, p, FailureKind::AlreadyInitialized);
}

// The raw-ID variant exercises the entry alloca dominance independently of the
// source producer's entry=0 ordering. Explicit invocation requires real LLVM.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run in the native CI job"]
fn raw_mutable_places_use_real_llvm() {
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("oxid-raw-places-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let scratch = Scratch(root);
    let mut artifacts = 0;
    for chosen in [false, true] {
        for (ty, initial, assigned, result) in [
            ("i32", "1", "id(x + 2)", if chosen { "15" } else { "6" }),
            (
                "bool",
                "false",
                "id(x || (1 + 2 == 3))",
                if chosen { "true" } else { "false" },
            ),
            ("()", "()", "id(x)", "()"),
        ] {
            let returned = if ty == "i32" { "x * 3" } else { "x" };
            let source = format!("fn main() -> {ty} {{ let mut x = {initial}; if {chosen} {{ x = {assigned}; }} x = id(x); return {returned}; }} fn id(n: {ty}) -> {ty} {{ let mut own = n; {} return own; }}", if ty == "i32" { "own = own + 1;" } else { "own = n;" });
            let (sources, base) = raw(&source);
            for reversed in [false, true] {
                let mut p = base.clone();
                if reversed {
                    let n = p.functions[0].blocks.len();
                    super::cfg_tests::permute(&mut p, &(0..n).rev().collect::<Vec<_>>());
                    assert_ne!(p.functions[0].entry, BlockId(0));
                }
                let program = verify::verify(p, &sources).unwrap();
                let expected = program.run(Some(hir::DefId(0))).unwrap();
                assert_eq!(expected.to_string(), result);
                let module = program
                    .native_module(Some(hir::DefId(0)), &sources)
                    .unwrap();
                assert!(module.contains("entry:\n  %p0 = alloca"));
                let output = scratch.0.join(format!("artifact-{artifacts}"));
                crate::frontend::native::compile(&module, output.to_str().unwrap()).unwrap();
                let output = std::process::Command::new(output)
                    .env_clear()
                    .env("PATH", scratch.0.join("no-tools"))
                    .output()
                    .unwrap();
                assert_eq!(output.status.code(), Some(0));
                assert!(output.stderr.is_empty());
                assert_eq!(output.stdout, format!("{result}\n").as_bytes());
                artifacts += 1;
            }
        }
    }
    assert_eq!(artifacts, 12);
}
