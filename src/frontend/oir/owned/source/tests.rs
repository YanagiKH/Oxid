use super::{hir::*, resolve, typeck};
use crate::frontend::{diagnostic::Diagnostic, lexer, parser, source::SourceMap};

#[test]
fn unit3a_unchanged_hir_enclosing_layouts() {
    use std::mem::size_of;
    macro_rules! sizes { ($($ty:ty),* $(,)?) => { $(println!("layout {} {}", stringify!($ty), size_of::<$ty>());)* }; }
    sizes!(
        Expr,
        ExprKind,
        Stmt,
        StmtKind,
        Binding,
        Signature,
        Function,
        Option<Expr>,
        Option<Stmt>,
        Option<Binding>,
        Vec<Expr>,
        Vec<Stmt>,
        Vec<Binding>
    );
}

#[test]
fn public_array_ast_enters_owned_resolution_with_normal_errors() {
    for (text, expected) in [
        ("fn f(a:[i32;1])->(){}", None),
        ("fn f(a:&mut [i32;1])->(){}", None),
        ("fn f()->[();0]{}", None),
        ("fn f()->(){let a:[bool;0]=true;}", None),
        ("fn f()->(){[1];}", None),
        ("fn f()->(){a[0];}", Some("E0200")),
        ("fn f()->(){a.len();}", Some("E0200")),
        ("fn f()->(){a[0]=1;}", Some("E0200")),
    ] {
        let mut map = SourceMap::new();
        let file = map.add("public.ox".into(), text.into());
        let file = map.get(file);
        let (ast, _) = parser::parse_counted_with_arrays(
            file,
            lexer::lex(file).unwrap(),
            parser::SourceMode::OwnedCandidate,
            parser::MAX_NODES,
            &mut crate::frontend::project::budget::Allocator::default(),
            parser::ArraySyntaxPolicy::Enabled,
        )
        .unwrap();
        match expected {
            None => assert!(resolve::resolve(file, &ast).is_ok(), "{text}"),
            Some(code) => {
                let errors = resolve::resolve(file, &ast).unwrap_err();
                assert_eq!(
                    (errors[0].code, errors[0].stage),
                    (code, "resolve"),
                    "{text}"
                );
                assert!(file.try_text(errors[0].primary.unwrap()).is_some());
            }
        }
    }
}

#[test]
fn unit3a_long_nonstarter_uses_bounded_actual_parser_allocations() {
    // Prepare only immutable source bytes outside measurement. Lexing and the
    // complete parser error path run inside the existing real allocator observer,
    // so no pre-measurement token allocation is freed to hide a temporary String.
    let token = format!("\"{}\"", "雪".repeat(21_000));
    for candidate in [true, false] {
        let mut map = SourceMap::new();
        let text = if candidate {
            format!("fn f()->(){{[1,{token}];}}")
        } else {
            format!("fn f()->(){{{token};}}")
        };
        let file = map.add("allocation.ox".into(), text);
        let file = map.get(file);
        let (errors, (calls, live, peak)) = super::reviewer_source::integration_measured(|| {
            let tokens = lexer::lex(file).unwrap();
            let result = if candidate {
                parser::parse_counted_with_arrays(
                    file,
                    tokens,
                    parser::SourceMode::OwnedCandidate,
                    parser::MAX_NODES,
                    &mut crate::frontend::project::budget::Allocator::default(),
                    parser::ArraySyntaxPolicy::Candidate,
                )
                .map(|(program, _)| program)
            } else {
                parser::parse_with_mode(file, tokens, parser::SourceMode::OwnedCandidate)
            };
            result.unwrap_err()
        });
        assert_eq!((errors[0].code, errors[0].stage), ("E0101", "parse"));
        assert_eq!(file.text_at(errors[0].primary.unwrap()), token);
        assert!(calls > 0 && live > 0 && peak >= live);
        if candidate {
            assert!(
                errors[0].message.len() <= crate::frontend::owned_diagnostic::MAX_MESSAGE_BYTES
            );
            assert!(
                peak < 16 * 1024,
                "candidate diagnostic copied source-sized text: {peak}"
            );
        } else {
            assert_eq!(
                errors[0].message,
                format!("unsupported typed-preview construct `{token}`")
            );
            assert!(
                peak >= token.len() as isize,
                "public control must detect a source-sized diagnostic"
            );
        }
        println!(
            "nonstarter candidate={candidate} allocation_calls={calls} retained={live} peak={peak}"
        );
    }
}

fn errors(text: &str) -> Vec<Diagnostic> {
    let mut sources = SourceMap::new();
    let id = sources.add("owned.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    match resolve::resolve(source, &ast) {
        Err(errors) => errors,
        Ok(resolved) => typeck::check(resolved).unwrap_err(),
    }
}
fn accepts(text: &str) {
    let mut sources = SourceMap::new();
    let id = sources.add("owned.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve(source, &ast).unwrap()).unwrap();
    for view in typed.functions() {
        for index in 0..view.hir().expressions.len() {
            view.expression_ty(ExprId(index));
        }
        for index in 0..view.hir().bindings.len() {
            view.binding_ty(BindingId(index));
        }
        for index in 0..view.hir().blocks.len() {
            view.block_flow(BodyBlockId(index));
        }
    }
}
#[test]
fn nominal_records_forward_signatures_and_independent_namespaces() {
    for text in [
        "fn relay(x: Cell) -> Cell { return x; } struct Cell { value: i32 } fn main() -> Cell { let Cell = Cell { value: 2 }; return relay(Cell); }",
        "struct Empty {} fn Empty() -> Empty { return Empty {}; } fn main() -> () { Empty(); return; }",
        "struct Cell { u: (), b: bool, n: i32 } fn main() -> () { let mut a: Cell = Cell { n: -2147483648, b: true, u: () }; let b = a; a = b; a.n = 2147483647; return; }",
    ] { accepts(text); }
}
#[test]
fn names_fields_types_and_scope_errors_have_stable_families() {
    for (text, code) in [
        ("struct C {} struct C {}", "E0201"),
        ("struct bool {}", "E0202"),
        ("struct i32 {}", "E0202"),
        ("struct C { a: i32, a: bool }", "E0201"),
        ("struct C { a: D } struct D {}", "E0202"),
        ("struct C { a: Unknown }", "E0202"),
        ("fn f(x: &bool) -> () { return; }", "E0202"),
        ("fn f(x: Unknown) -> () { return; }", "E0202"),
        (
            "struct C {} fn f() -> () { let x: Unknown = 0; return; }",
            "E0202",
        ),
        ("struct C {} fn f() -> () { unknown(); return; }", "E0200"),
        ("struct C {} fn f() -> () { let x = x; return; }", "E0200"),
        ("struct C {} fn f() -> () { let f = 1; return; }", "E0201"),
        (
            "struct C {} fn f(p: i32) -> () { if true { let p = 2; } return; }",
            "E0201",
        ),
        (
            "struct C {} fn f() -> () { if true { let x = 1; } x; return; }",
            "E0200",
        ),
        ("struct C {} fn f() -> () { break; return; }", "E0204"),
        ("struct C {} fn f() -> () { continue; return; }", "E0204"),
        ("struct C {} fn f() -> () { D {}; return; }", "E0202"),
        (
            "struct C { a: i32 } fn f() -> () { C { x: 0 }; return; }",
            "E0200",
        ),
        (
            "struct C { a: i32 } fn f() -> () { C { a: 0, a: 1 }; return; }",
            "E0201",
        ),
        (
            "struct C { a: i32 } fn f() -> () { C {}; return; }",
            "E0300",
        ),
        (
            "struct C { a: i32 } fn f() -> () { C { a: false }; return; }",
            "E0300",
        ),
        (
            "struct C {} struct D {} fn f(x: C) -> D { return x; }",
            "E0300",
        ),
        ("struct C {} fn f(x: C) -> bool { return x == x; }", "E0300"),
        ("struct C {} fn f(x: C) -> i32 { return x + 1; }", "E0300"),
        (
            "struct C {} fn f(x: C) -> () { x.missing; return; }",
            "E0305",
        ),
        (
            "struct C {} fn f(x: i32) -> () { x.field; return; }",
            "E0305",
        ),
        ("struct C {} fn f() -> () { f(1); return; }", "E0301"),
        ("struct C {} fn f() -> () { 2147483648; return; }", "E0203"),
    ] {
        assert_eq!(errors(text)[0].code, code, "{text}");
    }
    accepts("struct C {} fn f() -> () { if true { let x = 1; } else { let x = 2; } let x = 3; return; }");
}
#[test]
fn all_bare_reference_value_contexts_are_rejected_at_the_name() {
    for use_site in [
        "p;",
        "(p);",
        "let x = p;",
        "let x = (p);",
        "take(p);",
        "return p;",
    ] {
        let text = format!("struct C {{}} fn take(x: C) -> () {{ return; }} fn f(p: &C) -> () {{ {use_site} return; }}");
        let error = &errors(&text)[0];
        assert_eq!(error.code, "E0312", "{use_site}");
        let span = error.primary.unwrap();
        assert_eq!(&text[span.start..span.end], "p");
    }
}
#[test]
fn reference_permissions_remain_authoritative_raw_facts() {
    // All of these typecheck. Loan conflicts, movedness, writes/reborrows from
    // shared authority belong exclusively to the later raw verifier.
    for text in [
        "struct C { x: i32 } fn f(p: &C) -> () { p.x = 1; return; }",
        "struct C {} fn step(p: &mut C) -> () { return; } fn f(p: &C) -> () { step(&mut *p); return; }",
        "struct C {} fn two(a: &mut C, b: &mut C) -> () { return; } fn f() -> () { let mut x = C {}; two(&mut x, &mut x); return; }",
        "struct C {} fn f() -> () { let mut x = C {}; x; x; x = C {}; return; }",
        "struct C { x: i32 } fn f(p: &mut C) -> () { p.x = 2; return; }",
    ] { accepts(text); }
}
#[test]
fn borrow_categories_nominality_and_binding_mutability_are_checked() {
    for (body, code) in [
        ("let x = C {}; write(&mut x);", "E0304"),
        ("let x = C {}; x.x = 1;", "E0304"),
        ("let x = C {}; x = C {};", "E0304"),
        ("let x = 1; read(&x);", "E0300"),
        ("let x = C {}; read(&*x);", "E0312"),
        ("let x = D {}; read(&x);", "E0300"),
        ("let mut x = C {}; read(&mut x);", "E0300"),
        ("let x = C {}; write(&x);", "E0300"),
    ] {
        let text = format!("struct C {{ x: i32 }} struct D {{}} fn read(p: &C) -> () {{ return; }} fn write(p: &mut C) -> () {{ return; }} fn f() -> () {{ {body} return; }}");
        // Nonempty C literals need a field to reach the borrowing check.
        let text = text.replace("C {}", "C { x: 0 }");
        assert_eq!(errors(&text)[0].code, code, "{body}");
    }
    for borrow in ["&p", "&mut p"] {
        assert_eq!(errors(&format!("struct C {{}} fn read(q: &C) -> () {{ return; }} fn f(p: &C) -> () {{ read({borrow}); return; }}"))[0].code, "E0312");
    }
}
#[test]
fn every_body_branch_and_lazy_operand_is_typed_and_returns_are_explicit() {
    for (text, code) in [
        ("struct C {} fn unused() -> () { let x: bool = 1; return; } fn main() -> () { return; }", "E0300"),
        ("struct C {} fn f() -> () { if true { return; } else { let x: bool = 1; return; } }", "E0300"),
        ("struct C {} fn f() -> () { while false { let x: bool = 1; } return; }", "E0300"),
        ("struct C {} fn f() -> bool { return false && 1; }", "E0300"),
        ("struct C {} fn f() -> bool { return true || 1; }", "E0300"),
        ("struct C {} fn f() -> () {}", "E0302"),
        ("struct C {} fn f() -> () { while true { return; } }", "E0302"),
        ("struct C {} fn f() -> () { return; true; }", "E0303"),
        ("struct C {} fn f() -> () { while true { break; true; } return; }", "E0303"),
        ("struct C {} fn f() -> () { while true { continue; true; } return; }", "E0303"),
    ] { assert_eq!(errors(text)[0].code, code, "{text}"); }
    accepts("struct C {} fn f(c: bool) -> () { while c { if c { break; } else { continue; } } if c { return; } else { return; } }");
}
#[test]
fn source_diagnostic_accumulation_stops_at_one_hundred() {
    for count in [100, 101] {
        let text = "struct C { a: i32 } ".to_owned()
            + &(0..count)
                .map(|i| format!("fn f{i}() -> () {{ C {{}}; return; }} "))
                .collect::<String>();
        let diagnostics = errors(&text);
        assert_eq!(diagnostics.len(), 100);
        assert!(diagnostics
            .iter()
            .all(|e| e.code == "E0300" && e.message.len() <= 1024));
    }
}
#[test]
fn source_amplification_is_bounded_before_the_101st_format() {
    use std::fmt::Write;
    let mut source = String::from("struct T {\n");
    for index in 0..1024 {
        if index != 0 {
            source.push_str(",\n");
        }
        write!(source, "f{index:04}_{}: i32", "x".repeat(894)).unwrap();
    }
    source.push_str("\n}\n");
    for index in 0..100 {
        if index != 0 {
            source.push('\n');
        }
        write!(source, "fn f{index}() -> () {{ T {{}}; return; }}").unwrap();
    }
    assert_eq!(source.len(), 932_069);
    for extra in [false, true] {
        if extra {
            source.push_str("\nfn extra() -> () { T {}; return; }");
        }
        let (diagnostics, stats) =
            crate::frontend::owned_diagnostic::measure_formatting(|| errors(&source));
        assert_eq!(diagnostics.len(), 100);
        assert_eq!(stats.allocations, 100);
        assert_eq!(stats.displayed_names, 800);
        assert_eq!(stats.name_bytes, 51_200);
        assert_eq!(stats.copied_bytes, 56_100);
        assert!(diagnostics
            .iter()
            .all(|e| e.message.len() == 561 && e.message.ends_with("; 1016 more omitted")));
        eprintln!("owned source amplification: source={} retained={} message_capacity={} diagnostics_capacity={} formatter={stats:?}", source.len(), diagnostics.iter().map(|d| d.message.len()).sum::<usize>(), diagnostics.iter().map(|d| d.message.capacity()).sum::<usize>(), diagnostics.capacity());
    }
}
#[test]
fn typed_views_retain_identity_order_modes_projections_and_exact_origins() {
    let text = "struct C { a: i32, b: bool } fn main(n: i32, p: &mut C, c: C, u: ()) -> C { let mut x = C { b: true, a: n }; x.a = p.a; take(&mut *p, x); return c; } fn take(p: &mut C, c: C) -> () { return; }";
    let mut sources = SourceMap::new();
    let id = sources.add("雪.ox".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve(source, &ast).unwrap()).unwrap();
    assert_eq!(typed.entry(), Some(DefId(0)));
    assert_eq!(
        typed.records()[0].fields[1].id,
        FieldId {
            record: RecordId(0),
            index: 1
        }
    );
    let view = typed.functions().next().unwrap();
    let f = view.hir();
    assert_eq!(typed.text(view.signature().span), "main");
    assert_eq!(
        f.bindings
            .iter()
            .map(|b| b.parameter_position)
            .collect::<Vec<_>>(),
        [Some(0), Some(1), Some(2), Some(3), None]
    );
    assert_eq!(
        f.bindings
            .iter()
            .map(|b| typed.text(b.span))
            .collect::<Vec<_>>(),
        ["n", "p", "c", "u", "x"]
    );
    assert_eq!(
        view.binding_ty(BindingId(1)),
        ParameterTy::Reference {
            referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(0))),
            kind: BorrowKind::Exclusive
        }
    );
    assert!(f.bindings[4].mutable);
    let literal = f
        .expressions
        .iter()
        .find_map(|e| {
            if let ExprKind::StructLiteral { fields, .. } = &e.kind {
                Some(fields)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        literal.iter().map(|f| f.field.index).collect::<Vec<_>>(),
        [1, 0]
    );
    let projected = f
        .expressions
        .iter()
        .enumerate()
        .find(|(_, e)| matches!(e.kind, ExprKind::FieldRead { .. }))
        .unwrap();
    assert_eq!(
        view.expression_projection(ExprId(projected.0)),
        Some(Projection {
            base: AccessBase::Reference {
                binding: BindingId(1),
                kind: BorrowKind::Exclusive
            },
            field: FieldId {
                record: RecordId(0),
                index: 0
            }
        })
    );
    assert_eq!(
        view.statement_projection(f.body, 1),
        Some(Projection {
            base: AccessBase::Owner(BindingId(4)),
            field: FieldId {
                record: RecordId(0),
                index: 0
            }
        })
    );
    assert!(view.block_flow(f.body).returns_only());
}
#[test]
fn batch_pilot_completes_typing_without_activation() {
    let lf = include_str!("../../../../../rfcs/0014-owned-structs-call-borrows.md")
        .replace("\r\n", "\n");
    let crlf = lf.replace('\n', "\r\n");
    // Exercise both checkout conventions while still typing the actual RFC pilot.
    for document in [&lf, &crlf] {
        let normalized = document.replace("\r\n", "\n");
        let (_, rest) = normalized
            .split_once("```text\nstruct Batch")
            .expect("RFC must contain the Batch pilot text block");
        let (body, _) = rest
            .split_once("\n```")
            .expect("RFC Batch pilot text block must have a closing fence");
        accepts(&format!("struct Batch{body}"));
    }
}
#[test]
fn declaration_count_limits_are_checked_before_contents() {
    for count in [4096, 4097] {
        let text = (0..count)
            .map(|i| format!("struct R{i} {{}} "))
            .collect::<String>();
        if count == 4096 {
            accepts(&text);
        } else {
            assert_eq!(errors(&text)[0].code, "E0400");
        }
    }
    for count in [1024, 1025] {
        let fields = (0..count)
            .map(|i| format!("a{i}: i32"))
            .collect::<Vec<_>>()
            .join(",");
        let text = format!("struct C {{ {fields} }}");
        if count == 1024 {
            accepts(&text);
        } else {
            assert_eq!(errors(&text)[0].code, "E0400");
        }
    }
    // Duplicate identity and unknown field type must not run before count admission.
    let text = format!("struct C {{ {} }}", vec!["same: Unknown"; 1025].join(","));
    assert_eq!(errors(&text)[0].code, "E0400");
}
#[test]
fn branch_terminal_pairs_and_nearest_loop_targets_are_preserved() {
    for first in ["return;", "break;", "continue;"] {
        for second in ["return;", "break;", "continue;"] {
            let text = format!("struct C {{}} fn f(c: bool) -> () {{ while c {{ if c {{ {first} }} else {{ {second} }} true; }} return; }}");
            let error = &errors(&text)[0];
            assert_eq!(error.code, "E0303");
            assert_eq!(
                error.message,
                if first == "return;" && second == "return;" {
                    "statement after terminal return is unavailable in typed-preview"
                } else {
                    "statement after terminal control transfer is unavailable in typed-preview"
                }
            );
            let span = error.primary.unwrap();
            assert_eq!(&text[span.start..span.end], "true;");
        }
    }
    let text =
        "struct C {} fn f() -> () { while true { while true { break; } continue; } return; }";
    let mut sources = SourceMap::new();
    let id = sources.add("x".into(), text.into());
    let source = sources.get(id);
    let ast = parser::parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve(source, &ast).unwrap()).unwrap();
    let view = typed.functions().next().unwrap();
    assert!(matches!(
        view.hir().blocks[2].body[0].kind,
        StmtKind::Break { target: LoopId(2) }
    ));
    assert!(matches!(
        view.hir().blocks[1].body[1].kind,
        StmtKind::Continue { target: LoopId(1) }
    ));
}
