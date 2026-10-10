//! Independent source qualification: expectations come from RFC0014 examples
//! and hand-written source identities, never from the producer's count pass.
use super::super::*;
use super::{budget, diagnostic, lower, resolve, typeck};
use crate::frontend::{lexer, parser, source::SourceFileId};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default)]
struct AllocationStats {
    calls: usize,
    live: isize,
    peak: isize,
}
thread_local! {
    static TRACKING: Cell<Option<AllocationStats>> = const { Cell::new(None) };
}
pub(in crate::frontend::oir::owned) fn account(delta: isize, allocation: bool) {
    let _ = TRACKING.try_with(|cell| {
        if let Some(mut stats) = cell.get() {
            stats.calls += usize::from(allocation);
            stats.live += delta;
            stats.peak = stats.peak.max(stats.live);
            cell.set(Some(stats));
        }
    });
}
fn measured<T>(action: impl FnOnce() -> T) -> (T, AllocationStats) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            TRACKING.with(|cell| cell.set(None));
        }
    }
    TRACKING.with(|cell| {
        assert!(cell.get().is_none());
        cell.set(Some(AllocationStats::default()));
    });
    let _reset = Reset;
    let value = action();
    let stats = TRACKING.with(|cell| cell.get().unwrap());
    (value, stats)
}
fn with_typed<T>(
    text: &str,
    action: impl FnOnce(&typeck::TypedOwnedProgram<'_>, &SourceMap) -> T,
) -> T {
    let mut sources = SourceMap::new();
    let id = sources.add("reviewer-source.ox".into(), text.into());
    let file = sources.get(id);
    let ast = parser::parse_with_mode(
        file,
        lexer::lex(file).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve(file, &ast).unwrap()).unwrap();
    action(&typed, &sources)
}
fn raw(text: &str) -> (SourceMap, RawOwnedProgram) {
    let mut sources = SourceMap::new();
    let id = sources.add("reviewer-source.ox".into(), text.into());
    let file = sources.get(id);
    let ast = parser::parse_with_mode(
        file,
        lexer::lex(file).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve(file, &ast).unwrap()).unwrap();
    let program = lower::lower(&typed).unwrap();
    (sources, program)
}
fn text_at(sources: &SourceMap, span: Span) -> &str {
    assert!(sources.is_valid_span(span));
    &sources.get(span.file).text()[span.start..span.end]
}
fn checked(text: &str) -> (SourceMap, verified::VerifiedOwnedProgram, hir::DefId) {
    let (sources, program) = raw(text);
    let entry = program
        .functions
        .iter()
        .find(|f| text_at(&sources, f.span) == "main")
        .unwrap()
        .id;
    let witness = verified::verify_owned(program, &sources).unwrap();
    (sources, witness, entry)
}
fn result(text: &str) -> Scalar {
    let (_, witness, entry) = checked(text);
    execute::run(&witness, Some(entry)).unwrap()
}

#[test]
fn reviewer_source_preflight_is_actually_allocation_free_and_failures_drop_every_payload() {
    let text = "struct T { value: i32 } fn relay(x: T) -> T { return x; } fn get(p: &T, b: bool) -> i32 { if b { return p.value; } else { return 0; } } fn main() -> i32 { let mut x = relay(T { value: 7 }); x.value = get(&x, true && (false || true)); return x.value; }";
    with_typed(text, |typed, sources| {
        let (usage, stats) =
            measured(|| budget::preflight(typed, budget::Limits::DEFAULT).unwrap());
        assert_eq!((stats.calls, stats.live, stats.peak), (0, 0, 0));
        // Three schema/top-level, 3*(one count-map+8 function vectors+3 maps),
        // eleven block payloads, two argument lists, one literal payload.
        let attempts = 3 + 3 * 12 + 11 + 2 + 1;
        for fail_at in 0..attempts {
            let (failure, stats) = measured(|| {
                budget::fail_allocation_after(fail_at, || lower::lower(typed)).unwrap_err()
            });
            assert_eq!(
                failure.kind,
                OwnedFailureKind::Resource("injected source allocation failure")
            );
            assert_eq!(stats.live, 0, "leak at reservation {fail_at}: {stats:?}");
        }
        let (_, stats) = measured(|| {
            drop(budget::fail_allocation_after(attempts, || lower::lower(typed)).unwrap());
        });
        assert_eq!(stats.live, 0);
        assert!(stats.peak as usize >= usage.raw_bytes - std::mem::size_of::<RawOwnedProgram>());
        let witness = verified::verify_owned(lower::lower(typed).unwrap(), sources).unwrap();
        assert_eq!(
            execute::run(&witness, typed.entry()).unwrap(),
            Scalar::I32(7)
        );
        println!("source reservations={attempts}; raw payload={}; producer scratch={}; measured lowering heap peak={}", usage.raw_bytes, usage.scratch_bytes, stats.peak);
    });
}

#[test]
fn reviewer_source_deep_if_else_and_expression_frames_reach_the_parser_boundary() {
    let mut body = "return 19;".to_owned();
    // Main is block depth one, each wrapping if introduces one additional
    // active then block. Every pending sibling else adds the worst-case frame.
    for _ in 0..63 {
        body = format!("if true {{ {body} }} else {{ return 23; }}");
    }
    assert_eq!(
        result(&format!("struct T {{}} fn main() -> i32 {{ {body} }}")),
        Scalar::I32(19)
    );
    let mut expression = "1".to_owned();
    // A left-associated binary tree accumulates Emit and pending right frames.
    for _ in 0..63 {
        expression.push_str(" + 1");
    }
    assert_eq!(
        result(&format!(
            "struct T {{}} fn main() -> i32 {{ return {expression}; }}"
        )),
        Scalar::I32(64)
    );
}

#[test]
fn reviewer_source_reached_prefix_cleanup_handles_break_continue_return_and_siblings() {
    for flag in [false, true] {
        let text = format!("struct T {{ n: i32 }} fn relay(x: T) -> T {{ return x; }} fn choose(flag: bool) -> T {{ let outside = T {{ n: 40 }}; while true {{ let first = T {{ n: 1 }}; if flag {{ return relay(outside); }} let later = T {{ n: 2 }}; break; }} return outside; }} fn main() -> i32 {{ let selected = choose({flag}); let mut n = 0; while n < 3 {{ if n < 1 {{ n = n + 1; continue; }} let sibling = T {{ n: n }}; n = n + 1; }} return selected.n + n; }}");
        assert_eq!(result(&text), Scalar::I32(43));
    }
    assert_eq!(result("struct T { n: i32 } fn main() -> i32 { let outside = T { n: 5 }; if true { let x = T { n: 1 }; x; } else { let x = T { n: 2 }; x; } return outside.n; }"), Scalar::I32(5));
}

#[test]
fn reviewer_source_nested_lazy_calls_keep_immediate_parent_argument_positions() {
    let text = "struct T { n: i32 } fn read(p: &T) -> i32 { return p.n; } fn same(a: i32, b: i32) -> bool { return a == b; } fn choose(p: &T, flag: bool, n: i32) -> i32 { if flag { return p.n + n; } else { return 0; } } fn main() -> i32 { let x = T { n: 9 }; return choose(&x, true && same(read(&x), read(&x)), read(&x)); }";
    let (sources, raw) = raw(text);
    let calls = &raw.functions[3].calls;
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls
            .iter()
            .map(|c| (c.target.0, c.parent.map(|(p, a)| (p.0, a))))
            .collect::<Vec<_>>(),
        [
            (2, None),
            (1, Some((0, 1))),
            (0, Some((1, 0))),
            (0, Some((1, 1))),
            (0, Some((0, 2)))
        ]
    );
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(3))).unwrap(),
        Scalar::I32(18)
    );
}

#[test]
fn reviewer_source_raw_valid_mutability_nominal_and_origin_lies_are_detected_as_correspondence() {
    let text = "struct A { n: i32 } struct B { n: i32 } fn main() -> i32 { let x = A { n: 6 }; return x.n; }";
    for mutant in 0..3 {
        let (sources, mut raw) = raw(text);
        let f = &mut raw.functions[0];
        let local = f
            .owners
            .iter()
            .position(|o| matches!(o.kind, OwnerKind::Local { .. }))
            .unwrap();
        match mutant {
            0 => f.owners[local].kind = OwnerKind::Local { mutable: true },
            1 => {
                for owner in &mut f.owners {
                    owner.aggregate =
                        AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(1)))
                            .unwrap();
                }
                for block in &mut f.blocks {
                    for statement in &mut block.statements {
                        match &mut statement.kind {
                            OwnedInstruction::Construct { fields, .. } => {
                                for (field, _) in fields {
                                    field.record = RecordId(1);
                                }
                            }
                            OwnedInstruction::ReadField { field, .. } => field.record = RecordId(1),
                            _ => {}
                        }
                    }
                }
            }
            _ => {
                for block in &mut f.blocks {
                    for statement in &mut block.statements {
                        statement.diagnostic_origins = Some(DiagnosticOrigins {
                            primary: f.span,
                            cause: f.span,
                        });
                    }
                }
            }
        }
        let witness =
            verified::verify_owned(raw, &sources).expect("coherent producer lie is valid raw IR");
        assert_eq!(
            execute::run(&witness, Some(hir::DefId(0))).unwrap(),
            Scalar::I32(6)
        );
        let f = &witness.functions()[0];
        let source_match = match mutant {
            0 => f.owners[local].kind == OwnerKind::Local { mutable: false },
            1 => f.owners[local].aggregate() == AggregateTy::Record(RecordId(0)),
            _ => f
                .blocks
                .iter()
                .flat_map(|b| &b.statements)
                .filter(|s| matches!(s.kind, OwnedInstruction::ReadField { .. }))
                .all(|s| text_at(&sources, s.diagnostic_origins.unwrap().primary) == "x.n"),
        };
        assert!(!source_match, "mutant escaped source correspondence check");
    }
}

#[test]
fn reviewer_source_raw_valid_interleaved_parameter_permutation_changes_the_meaning() {
    let text = "struct T { n: i32 } fn pick(a: i32, left: T, p: &T, right: T, b: bool) -> i32 { return left.n * 10 + right.n + a; } fn main() -> i32 { let p = T { n: 0 }; return pick(3, T { n: 2 }, &p, T { n: 7 }, true); }";
    assert_eq!(result(text), Scalar::I32(30));
    let (sources, mut raw) = raw(text);
    let f = &mut raw.functions[0];
    f.parameters.swap(1, 3);
    f.owners[0].kind = OwnerKind::Parameter { position: 3 };
    f.owners[1].kind = OwnerKind::Parameter { position: 1 };
    let witness = verified::verify_owned(raw, &sources)
        .expect("same-typed owned parameters may coherently permute in raw IR");
    let f = &witness.functions()[0];
    let ParameterBinding::Owned(at_position_one) = f.parameters[1] else {
        panic!("owner parameter")
    };
    assert_eq!(text_at(&sources, f.owners[at_position_one.0].span), "right");
    assert_ne!(text_at(&sources, f.owners[at_position_one.0].span), "left");
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(1))).unwrap(),
        Scalar::I32(75)
    );
}

#[test]
fn reviewer_source_raw_valid_reference_mode_and_field_identity_lies_have_independent_controls() {
    let text = "struct T { a: i32, b: i32 } fn read(p: &T) -> i32 { return p.a; } fn main() -> i32 { let mut x = T { b: 8, a: 3 }; return read(&x); }";
    assert_eq!(result(text), Scalar::I32(3));
    for mutant in 0..2 {
        let (sources, mut raw) = raw(text);
        if mutant == 0 {
            raw.functions[0].references[0].kind = BorrowKind::Exclusive;
            raw.functions[1].loans[0].kind = BorrowKind::Exclusive;
        } else {
            for block in &mut raw.functions[0].blocks {
                for statement in &mut block.statements {
                    if let OwnedInstruction::ReadField { field, .. } = &mut statement.kind {
                        field.index = 1;
                    }
                }
            }
        }
        let witness =
            verified::verify_owned(raw, &sources).expect("coherent raw permission/field mutation");
        if mutant == 0 {
            assert_ne!(
                witness.functions()[0].references[0].kind,
                BorrowKind::Shared
            );
            assert_eq!(
                execute::run(&witness, Some(hir::DefId(1))).unwrap(),
                Scalar::I32(3)
            );
        } else {
            assert_eq!(
                execute::run(&witness, Some(hir::DefId(1))).unwrap(),
                Scalar::I32(8)
            );
        }
    }
}

#[test]
fn reviewer_source_raw_valid_snapshot_reread_lie_changes_result_after_mutation() {
    let text="struct T { n: i32 } fn bump(p: &mut T) -> () { p.n = 9; return; } fn main() -> i32 { let mut x = T { n: 2 }; let snapshot = (x.n); bump(&mut x); return snapshot; }";
    assert_eq!(result(text), Scalar::I32(2));
    let (sources, mut raw) = raw(text);
    let f = &mut raw.functions[1];
    let owner = OwnerPlaceId(
        f.owners
            .iter()
            .position(|o| text_at(&sources, o.span) == "x")
            .unwrap(),
    );
    let mut changed = 0;
    for block in &mut f.blocks {
        for statement in &mut block.statements {
            if text_at(&sources, statement.span) == "snapshot" {
                let OwnedInstruction::Scalar(Statement::Assign(Assign { destination, .. })) =
                    statement.kind
                else {
                    panic!("snapshot expression copy")
                };
                statement.kind = OwnedInstruction::ReadField {
                    destination,
                    base: AccessBase::Owner(owner),
                    field: FieldId {
                        record: RecordId(0),
                        index: 0,
                    },
                };
                changed += 1;
            }
        }
    }
    assert_eq!(changed, 1);
    let witness = verified::verify_owned(raw, &sources)
        .expect("late reread has valid raw authority and dominance");
    assert_eq!(
        execute::run(&witness, Some(hir::DefId(1))).unwrap(),
        Scalar::I32(9)
    );
}

#[test]
fn reviewer_source_diagnostic_origins_are_utf8_safe_and_lazy_branches_are_checked() {
    for (text,code,primary) in [
        ("/* 雪 */\r\nstruct T { n: i32 } fn read(p: &T, b: bool) -> bool { return b; } fn bump(p: &mut T) -> bool { return true; } fn main() -> bool { let mut x = T { n: 0 }; return read(&x, false && bump(&mut /*雪*/ x)); }","E0311","&mut /*雪*/ x"),
        ("struct T { n: i32 } fn f(p: &T) -> () { p.n = 1; return; } fn main() -> () { return; }","E0313","p.n"),
        ("struct T { n: i32 } fn take(x: T) -> i32 { return 8; } fn main() -> () { let mut x = T { n: 2 }; x.n = take(x); return; }","E0310","x.n"),
    ] {
        let(sources,raw)=raw(text);
        let failure=verified::verify_owned(raw,&sources).unwrap_err();
        let diagnostic=diagnostic::verify(&failure,&sources);
        assert_eq!((diagnostic.code,diagnostic.stage),(code,"ownership"));
        assert_eq!(text_at(&sources,diagnostic.primary.unwrap()),primary);
        diagnostic.render_json(&sources); diagnostic.render_human(&sources);
    }
    let (sources, mut raw) = raw("/* 雪 */ struct T {} fn main() -> () { return; }");
    let invalid = Span {
        file: SourceFileId(0),
        start: 4,
        end: 5,
    }; // middle of UTF-8 snow
    raw.functions[0].blocks[0].statements[0].diagnostic_origins = Some(DiagnosticOrigins {
        primary: invalid,
        cause: invalid,
    });
    let failure = verified::verify_owned(raw, &sources).unwrap_err();
    assert_eq!(failure.kind, OwnedFailureKind::Malformed(Malformed::Span));
    let diagnostic = diagnostic::verify(&failure, &sources);
    assert_eq!(diagnostic.code, "E0500");
    diagnostic.render_json(&sources);
    diagnostic.render_human(&sources);
}

#[test]
fn reviewer_source_earlier_writes_survive_overflow_and_unpaid_final_store_is_absent() {
    let text="struct T { n: i32 } fn main() -> i32 { let mut x = T { n: 0 }; x.n = 7; x.n = x.n + 2147483647; return x.n; }";
    let (sources, witness, entry) = checked(text);
    let mut events = Vec::new();
    let failure = execute::run_observed(&witness, entry, execute::Limits::default(), &mut events)
        .unwrap_err();
    assert_eq!(failure.diagnostic(&sources).code, "E0604");
    assert_eq!(
        events
            .iter()
            .filter_map(
                |event| if let execute::Event::WriteField(_, _, value) = event {
                    Some(*value)
                } else {
                    None
                }
            )
            .collect::<Vec<_>>(),
        [Scalar::I32(7)]
    );
    let text="struct T { n: i32 } fn main() -> i32 { let mut x = T { n: 0 }; x.n = 7; x.n = 9; return x.n; }";
    let (sources, witness, entry) = checked(text);
    let mut full = Vec::new();
    assert_eq!(
        execute::run_observed(&witness, entry, execute::Limits::default(), &mut full).unwrap(),
        Scalar::I32(9)
    );
    // Hand ledger: S=4, O=2, P=2, X=4+2+4*2=14. Root=15;
    // literal/live/construct/live/move/end=9; two literal/stores=4;
    // read=1, lexical cleanup=2, scalar return=1+P=3: total 34.
    assert_eq!(
        full.iter()
            .filter_map(|event| if let execute::Event::Charge(_, cost) = event {
                Some(*cost)
            } else {
                None
            })
            .collect::<Vec<_>>(),
        [15, 1, 1, 2, 1, 2, 2, 1, 1, 1, 1, 1, 2, 3]
    );
    let mut before_second = 0;
    let mut second_cost = 0;
    for event in &full {
        if let execute::Event::Charge(span, cost) = event {
            if text_at(&sources, *span) == "x.n = 9;" {
                second_cost = *cost;
                break;
            }
            before_second += cost;
        }
    }
    assert_eq!(second_cost, 1);
    assert_eq!(before_second, 27);
    let mut partial = Vec::new();
    let denied = execute::run_observed(
        &witness,
        entry,
        execute::Limits {
            fuel: before_second,
            ..execute::Limits::default()
        },
        &mut partial,
    )
    .unwrap_err();
    assert_eq!(denied.diagnostic(&sources).code, "E0601");
    assert_eq!(
        text_at(&sources, denied.diagnostic(&sources).primary.unwrap()),
        "x.n = 9;"
    );
    assert_eq!(
        partial
            .iter()
            .filter_map(
                |event| if let execute::Event::WriteField(_, _, value) = event {
                    Some(*value)
                } else {
                    None
                }
            )
            .collect::<Vec<_>>(),
        [Scalar::I32(7)]
    );
}

#[test]
fn reviewer_source_physical_payload_includes_fields_operands_and_headers_once() {
    let text="struct T { b: bool, u: (), n: i32 } fn main() -> i32 { let x = T { n: 5, u: (), b: true }; return x.n; }";
    with_typed(text, |typed, sources| {
        use std::mem::size_of;
        let expected = size_of::<RawOwnedProgram>()
            + size_of::<RawRecordDecl>()
            + 3 * size_of::<RawFieldDecl>()
            + size_of::<RawOwnedFunction>()
            + 4 * size_of::<LocalDecl>()
            + 2 * size_of::<OwnerDecl>()
            + size_of::<OwnedBlock>()
            + 10 * size_of::<OwnedStatement>()
            + 3 * size_of::<(FieldId, Operand)>();
        let usage = budget::preflight(typed, budget::Limits::DEFAULT).unwrap();
        assert_eq!(usage.raw_bytes, expected);
        assert_eq!(usage.raw_bytes, 3368 + size_of::<Vec<MatchDecl>>());
        let denied = budget::fail_allocation_after(0, || {
            lower::lower_with_limits(
                typed,
                budget::Limits {
                    raw_bytes: expected - 1,
                },
            )
        })
        .unwrap_err();
        assert_eq!(
            denied.kind,
            OwnedFailureKind::Resource("source raw payload")
        );
        let raw = lower::lower_with_limits(
            typed,
            budget::Limits {
                raw_bytes: expected,
            },
        )
        .unwrap();
        let witness = verified::verify_owned(raw, sources).unwrap();
        assert_eq!(
            execute::run(&witness, typed.entry()).unwrap(),
            Scalar::I32(5)
        );
    });
}

#[test]
#[ignore = "requires qualified LLVM 19.1.7 and separate native evidence directory"]
fn reviewer_source_native_heldout_successes_and_failures() {
    use std::{fs, process::Command};
    let root = std::path::PathBuf::from(
        std::env::var_os("OXID_REVIEWER_SOURCE_NATIVE_EVIDENCE")
            .expect("native evidence directory"),
    );
    fs::create_dir(&root).expect("fresh directory");
    let rich="struct T { a: i32, b: i32 } fn next(p: &mut T) -> i32 { p.a = p.a + 1; return p.a; } fn relay(x: T) -> T { return x; } fn pair(p: &T, n: i32) -> i32 { return p.a * 100 + n; } fn main() -> i32 { let mut state = T { a: 1, b: 0 }; let first = (state.a); let value = T { b: next(&mut state), a: next(&mut state) }; let mut boxed = relay(value); boxed = relay(boxed); boxed.b = next(&mut state); return pair(&boxed, first * 10 + state.a); }";
    let reborrow="struct T { n: i32 } fn two(a: &T, b: &T) -> i32 { return a.n + b.n; } fn forward(p: &mut T) -> i32 { let old = two(&*p, &*p); p.n = p.n + 1; return old; } fn main() -> i32 { let mut x = T { n: 2 }; let old = forward(&mut x); return x.n * 10 + old; }";
    let overflow="struct T { n: i32 } fn main() -> i32 { let mut x = T { n: 0 }; x.n = 7; x.n = x.n + 2147483647; return x.n; } fn unused() -> () { while true {} return; }";
    let stores="struct T { n: i32 } fn main() -> i32 { let mut x = T { n: 0 }; x.n = 7; x.n = 9; return x.n; } fn unused() -> () { while true {} return; }";
    let mut default_store_body = String::new();
    for (name, text, fuel, expected, code) in [
        ("written-order-owned-results", rich, None, Some(314), None),
        (
            "shared-children-exclusive-parent",
            reborrow,
            None,
            Some(34),
            None,
        ),
        ("overflow-after-write", overflow, None, None, Some("E0604")),
        ("stores-default", stores, None, Some(9), None),
        ("unpaid-second-store", stores, Some(27), None, Some("E0601")),
        ("stores-exact-fuel", stores, Some(34), Some(9), None),
    ] {
        let (sources, witness, entry) = checked(text);
        let module = if let Some(fuel) = fuel {
            native::native_module_with_fuel(&witness, entry, &sources, fuel)
        } else {
            native::native_module(&witness, Some(entry), &sources)
        }
        .unwrap();
        if name == "stores-default" {
            default_store_body = module.split("define i32 @main").next().unwrap().to_owned();
        }
        if name == "unpaid-second-store" || name == "stores-exact-fuel" {
            assert_eq!(
                module.split("define i32 @main").next().unwrap(),
                default_store_body,
                "test-only wrapper must retain identical function bodies"
            );
        }
        fs::write(root.join(format!("{name}.ox")), text).unwrap();
        fs::write(root.join(format!("{name}.ll")), &module).unwrap();
        let run = root.join(format!("{name}-run"));
        fs::create_dir(&run).unwrap();
        let executable = run.join("program.elf");
        crate::frontend::native::compile(&module, executable.to_str().unwrap()).unwrap();
        assert_eq!(fs::read_dir(&run).unwrap().count(), 1);
        assert_eq!(&fs::read(&executable).unwrap()[..4], b"\x7fELF");
        let process = Command::new(fs::canonicalize(&executable).unwrap())
            .current_dir(&run)
            .env_clear()
            .env("PATH", "/no-tools")
            .output()
            .unwrap();
        fs::write(root.join(format!("{name}.stdout")), &process.stdout).unwrap();
        fs::write(root.join(format!("{name}.stderr")), &process.stderr).unwrap();
        if let Some(expected) = expected {
            assert_eq!(process.status.code(), Some(0), "{name}");
            assert_eq!(process.stdout, format!("{expected}\n").as_bytes());
            assert!(process.stderr.is_empty());
        } else {
            assert_eq!(process.status.code(), Some(1), "{name}");
            assert!(process.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&process.stderr).contains(code.unwrap()),
                "{name}"
            );
            let mut events = Vec::new();
            let reference = execute::run_observed(
                &witness,
                entry,
                execute::Limits {
                    fuel: fuel.unwrap_or(plan::MAX_FUEL),
                    ..execute::Limits::default()
                },
                &mut events,
            )
            .unwrap_err();
            assert_eq!(
                String::from_utf8_lossy(&process.stderr),
                reference.diagnostic(&sources).render_human(&sources)
            );
        }
        println!(
            "{name}: exit {:?}, stdout {:?}, stderr {:?}",
            process.status.code(),
            String::from_utf8_lossy(&process.stdout),
            String::from_utf8_lossy(&process.stderr)
        );
    }
}

// Independent integration controls only.
pub(in crate::frontend::oir::owned) fn integration_measured<T>(
    action: impl FnOnce() -> T,
) -> (T, (usize, isize, isize)) {
    let (value, stats) = measured(action);
    (value, (stats.calls, stats.live, stats.peak))
}
pub(in crate::frontend::oir::owned) fn integration_enabled() -> bool {
    TRACKING.with(|cell| cell.get().is_some())
}

/// Read-only test sizing; neither exposes tracker state nor starts tracking.
#[cfg(test)]
pub(in crate::frontend::oir::owned) fn integration_tracker_layout(
) -> (usize, usize, usize, usize, usize, usize) {
    (
        std::mem::size_of::<Cell<Option<AllocationStats>>>(),
        std::mem::align_of::<Cell<Option<AllocationStats>>>(),
        std::mem::size_of::<AllocationStats>(),
        std::mem::align_of::<AllocationStats>(),
        std::mem::size_of::<Option<AllocationStats>>(),
        std::mem::align_of::<Option<AllocationStats>>(),
    )
}
