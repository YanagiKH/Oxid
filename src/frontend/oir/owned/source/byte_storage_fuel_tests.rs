//! Independent RFC0031 tariffs. Expected schedules come from source spelling and
//! the published logical tariff, never ExecutionPlan or an observed successor.
//! Nonempty raw fixtures retain only an authentic named-conversion seed; the
//! test-only association seam does not create a public raw-to-source producer.
use super::super::*;
use super::{association, byte_storage_tests::with_raw};
use crate::frontend::source::SourceFileId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Effect {
    Enter(usize),
    Transfer,
    Read(usize, u8),
    Write(usize, u8),
    Len(usize),
    Acquire(BorrowKind),
    Release,
    Return(usize),
}
struct Step {
    span: Span,
    cost: usize,
    effects: Vec<Effect>,
    construction: bool,
}
fn step(span: Span, cost: usize) -> Step {
    Step {
        span,
        cost,
        effects: vec![],
        construction: false,
    }
}
fn effect(span: Span, cost: usize, effects: &[Effect]) -> Step {
    Step {
        span,
        cost,
        effects: effects.to_vec(),
        construction: false,
    }
}
fn construct(span: Span, width: usize) -> Step {
    Step {
        construction: true,
        ..step(span, 1 + width)
    }
}
fn at(text: &str, needle: &str) -> Span {
    let start = text.find(needle).unwrap();
    Span {
        file: SourceFileId(0),
        start,
        end: start + needle.len(),
    }
}
fn inside(text: &str, container: &str, needle: &str) -> Span {
    let outer = at(text, container);
    let start = outer.start + container.find(needle).unwrap();
    Span {
        start,
        end: start + needle.len(),
        ..outer
    }
}
fn seed(text: &str) -> Vec<Step> {
    vec![
        step(at(text, "128"), 1),
        step(at(text, "let x=128;"), 1),
        step(inside(text, "x.to_u8_checked()", "x"), 1),
        step(at(text, "x.to_u8_checked()"), 1),
        step(at(text, "let b=x.to_u8_checked();"), 1),
    ]
}
fn literal_steps(text: &str, statement: &str, literal: &str, n: usize) -> Vec<Step> {
    let span = inside(text, statement, literal);
    // Each source `b` is its own paid receiver snapshot, including repeated names.
    let mut out = (0..n)
        .map(|i| {
            step(
                Span {
                    start: span.start + 1 + 2 * i,
                    end: span.start + 2 + 2 * i,
                    ..span
                },
                1,
            )
        })
        .collect::<Vec<_>>();
    out.push(step(span, 1));
    out.push(construct(span, n.max(1)));
    out
}
fn replay(
    witness: &verified::VerifiedOwnedProgram,
    entry: usize,
    steps: &[Step],
    n: usize,
    label: &str,
) {
    let total = steps.iter().map(|s| s.cost).sum::<usize>();
    replay_budgets(witness, entry, steps, n, label, 0..=total);
}
fn replay_budgets(
    witness: &verified::VerifiedOwnedProgram,
    entry: usize,
    steps: &[Step],
    n: usize,
    label: &str,
    budgets: impl IntoIterator<Item = usize>,
) {
    let total = steps.iter().map(|s| s.cost).sum::<usize>();
    let mut tested = 0;
    for fuel in budgets {
        tested += 1;
        let mut left = fuel;
        let mut paid = vec![];
        let mut effects = vec![];
        let mut constructions = 0;
        let mut expected = Ok(Scalar::I32(n as i32));
        for s in steps {
            if left < s.cost {
                expected = Err(execute::OwnedRunFailure::Scalar(RunFailure::Fuel(s.span)));
                break;
            }
            left -= s.cost;
            paid.push((s.span, s.cost));
            effects.extend_from_slice(&s.effects);
            constructions += usize::from(s.construction);
        }
        let observed = execute::run_array_observed(
            witness,
            Some(hir::DefId(entry)),
            execute::Limits {
                fuel,
                ..Default::default()
            },
            execute::ObservationControl {
                poison_destinations: true,
                ..Default::default()
            },
        );
        assert_eq!(
            observed.result, expected,
            "{label}, N={n}, fuel={fuel}, total={total}"
        );
        assert_eq!(observed.remaining_fuel, left, "{label}, N={n}, fuel={fuel}");
        let charges = observed
            .events
            .iter()
            .filter_map(|e| match e {
                execute::Event::Charge(span, cost) => Some((*span, *cost)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(charges, paid, "{label}, N={n}, fuel={fuel}");
        let actual = observed
            .events
            .iter()
            .filter_map(|e| {
                Some(match e {
                    execute::Event::Charge(..) => return None,
                    execute::Event::Enter(id, _) => Effect::Enter(id.0),
                    execute::Event::Transfer(..) => Effect::Transfer,
                    execute::Event::ReadIndex(_, i, Scalar::U8(b)) => Effect::Read(*i, *b),
                    execute::Event::WriteIndex(_, i, Scalar::U8(b)) => Effect::Write(*i, *b),
                    execute::Event::ArrayLength(_, n) => Effect::Len(*n),
                    execute::Event::Acquire(_, _, kind) => Effect::Acquire(*kind),
                    execute::Event::Release(_) => Effect::Release,
                    execute::Event::Return(id) => Effect::Return(id.0),
                    other => panic!("unexpected {other:?}"),
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual, effects,
            "unpaid/out-of-order effect: {label}, N={n}, fuel={fuel}"
        );
        assert_eq!(
            observed
                .storage
                .iter()
                .filter(|s| s.kind == execute::StorageObservationKind::Construction)
                .count(),
            constructions
        );
        assert!(!observed.truncated);
        for row in observed.storage {
            assert_eq!(row.guards_before, row.guards_after);
            if matches!(
                row.kind,
                execute::StorageObservationKind::Construction
                    | execute::StorageObservationKind::Transfer
                    | execute::StorageObservationKind::IndexWrite
                    | execute::StorageObservationKind::Incoming
            ) {
                assert_eq!(
                    row.bytes,
                    vec![if n == 0 { 0 } else { 128 }; n.max(1)],
                    "{label}, N={n}, fuel={fuel}"
                );
            }
        }
    }
    eprintln!("byte storage independent replay: {label}, N={n}, tested={tested}, total={total}");
}

#[test]
fn byte_storage_source_construct_move_replace_every_fuel_all_widths() {
    for n in [0, 1, 1024] {
        let w = n.max(1);
        let literal = format!("[{}]", vec!["b"; n].join(","));
        let first = format!("let a:[u8;{n}]={literal};");
        let replacement_decl = format!("let r:[u8;{n}]={literal};");
        let replace = "d=r;";
        let text = format!("fn main()->i32{{let x=128;let b=x.to_u8_checked();{first}let mut d=a;{replacement_decl}{replace}return d.len();}}");
        // S=6+2N, O=7, P=7w: root entry 1 + S + P + 4O.
        let mut steps = vec![effect(
            at(&text, "main"),
            35 + 2 * n + 7 * w,
            &[Effect::Enter(0)],
        )];
        steps.extend(seed(&text));
        steps.extend(literal_steps(&text, &first, &literal, n));
        let decl = at(&text, &first);
        steps.extend([
            step(decl, 1),
            effect(decl, 1 + w, &[Effect::Transfer]),
            step(decl, 1 + w),
        ]);
        let moved = inside(&text, "let mut d=a;", "a");
        steps.extend([step(moved, 1), effect(moved, 1 + w, &[Effect::Transfer])]);
        let decl = at(&text, "let mut d=a;");
        steps.extend([
            step(decl, 1),
            effect(decl, 1 + w, &[Effect::Transfer]),
            step(decl, 1 + w),
        ]);
        steps.extend(literal_steps(&text, &replacement_decl, &literal, n));
        let decl = at(&text, &replacement_decl);
        steps.extend([
            step(decl, 1),
            effect(decl, 1 + w, &[Effect::Transfer]),
            step(decl, 1 + w),
        ]);
        let moved = inside(&text, replace, "r");
        steps.extend([step(moved, 1), effect(moved, 1 + w, &[Effect::Transfer])]);
        let replacement = at(&text, replace);
        steps.extend([
            effect(replacement, 1 + 2 * w, &[Effect::Transfer]),
            step(replacement, 1 + w),
        ]);
        steps.push(effect(at(&text, "d.len()"), 1, &[Effect::Len(n)]));
        let end = at(&text, "return d.len();");
        steps.extend([
            step(end, 1 + w),
            step(end, 1 + w),
            step(end, 1 + w),
            effect(end, 1 + 7 * w, &[Effect::Return(0)]),
        ]);
        assert_eq!(
            steps.iter().map(|s| s.cost).sum::<usize>(),
            64 + 4 * n + 30 * w
        );
        with_raw(&text, |_, typed, raw| {
            assert_eq!(
                (raw.functions[0].locals.len(), raw.functions[0].owners.len()),
                (6 + 2 * n, 7)
            );
            // The source route consumes only the trusted typed owner. The raw
            // inventory above is an observation, never the source authority.
            let witness =
                verified::verify_associated(association::lower_and_associate(typed).unwrap())
                    .unwrap();
            replay(&witness, 0, &steps, n, "source moves");
        });
    }
}

#[test]
fn byte_storage_raw_construct_move_replace_access_every_fuel_all_widths() {
    use super::super::consumer_fixtures as f;
    // The authentic conversion seed is kept, but no source array lowering is
    // reused. Three hand-declared owners and one reused byte operand make this
    // raw schedule intentionally different from the source schedule above.
    let text = "fn main()->i32{let x=128;let b=x.to_u8_checked();return 0;}";
    for n in [0, 1, 1024] {
        let w = n.max(1);
        with_raw(text, |_, typed, mut raw| {
            let body = &mut raw.functions[0];
            let span = at(text, "return 0;");
            body.locals.truncate(5);
            body.locals
                .extend([hir::Ty::I32, hir::Ty::I32, hir::Ty::U8].map(|ty| f::scalar(ty, span)));
            body.owners = (0..3)
                .map(|_| OwnerDecl {
                    aggregate: AggregateSlot::try_from_aggregate(AggregateTy::FixedArray(
                        FixedArrayTy::check(hir::Ty::U8, n).unwrap(),
                    ))
                    .unwrap(),
                    kind: OwnerKind::Local { mutable: true },
                    span,
                })
                .collect();
            body.blocks[0].statements.truncate(5);
            let statements = &mut body.blocks[0].statements;
            statements.push(f::assign(5, Rvalue::I32(0), span));
            for kind in [
                OwnedInstruction::StorageLive(OwnerPlaceId(0)),
                OwnedInstruction::ConstructArray {
                    destination: OwnerPlaceId(0),
                    elements: vec![f::operand(4, span); n],
                },
                OwnedInstruction::StorageLive(OwnerPlaceId(1)),
                OwnedInstruction::MoveInitialize {
                    destination: OwnerPlaceId(1),
                    source: OwnerPlaceId(0),
                },
                OwnedInstruction::StorageLive(OwnerPlaceId(2)),
                OwnedInstruction::ConstructArray {
                    destination: OwnerPlaceId(2),
                    elements: vec![f::operand(4, span); n],
                },
                OwnedInstruction::Replace {
                    destination: OwnerPlaceId(1),
                    source: OwnerPlaceId(2),
                },
            ] {
                statements.push(f::instruction(kind, span));
            }
            if n != 0 {
                statements.extend([
                    f::instruction(
                        OwnedInstruction::ReadIndex {
                            destination: LocalId(7),
                            base: AccessBase::Owner(OwnerPlaceId(1)),
                            index: f::operand(5, span),
                        },
                        span,
                    ),
                    f::instruction(
                        OwnedInstruction::WriteIndex {
                            base: AccessBase::Owner(OwnerPlaceId(1)),
                            index: f::operand(5, span),
                            value: f::operand(7, span),
                        },
                        span,
                    ),
                ]);
            }
            statements.push(f::instruction(
                OwnedInstruction::ArrayLength {
                    destination: LocalId(6),
                    base: AccessBase::Owner(OwnerPlaceId(1)),
                },
                span,
            ));
            body.blocks[0].terminator =
                f::end(OwnedTerminatorKind::ReturnScalar(f::operand(6, span)), span);
            // S8 + P3w + 4O3 + entry1; the raw constructor does not
            // manufacture N source receiver-copy instructions.
            let mut steps = vec![effect(at(text, "main"), 21 + 3 * w, &[Effect::Enter(0)])];
            steps.extend(seed(text));
            steps.extend([
                step(span, 1),
                step(span, 1),
                construct(span, w),
                step(span, 1),
                effect(span, 1 + w, &[Effect::Transfer]),
                step(span, 1),
                construct(span, w),
                effect(span, 1 + 2 * w, &[Effect::Transfer]),
            ]);
            if n != 0 {
                steps.extend([
                    effect(span, 1, &[Effect::Read(0, 128)]),
                    effect(span, 1, &[Effect::Write(0, 128)]),
                ]);
            }
            steps.extend([
                effect(span, 1, &[Effect::Len(n)]),
                effect(span, 1 + 3 * w, &[Effect::Return(0)]),
            ]);
            assert_eq!(
                steps.iter().map(|s| s.cost).sum::<usize>(),
                36 + 11 * w + 2 * usize::from(n != 0)
            );
            let witness =
                verified::verify_associated(association::associate(raw, typed).unwrap()).unwrap();
            replay(&witness, 0, &steps, n, "seed-authenticated raw moves");
        });
    }
}

#[test]
fn byte_storage_source_and_raw_borrow_call_reborrow_every_fuel() {
    use super::super::consumer_fixtures as f;
    for n in [0, 1, 1024] {
        let w = n.max(1);
        for (outer, inner) in [
            (BorrowKind::Shared, BorrowKind::Shared),
            (BorrowKind::Exclusive, BorrowKind::Shared),
            (BorrowKind::Exclusive, BorrowKind::Exclusive),
        ] {
            let om = if outer == BorrowKind::Exclusive {
                "mut "
            } else {
                ""
            };
            let im = if inner == BorrowKind::Exclusive {
                "mut "
            } else {
                ""
            };
            let literal = format!("[{}]", vec!["b"; n].join(","));
            let first = format!("let mut a:[u8;{n}]={literal};");
            let child = format!("leaf(&{im}*p)");
            let call = format!("relay(&{om}a)");
            let text = format!("fn leaf(p:&{im}[u8])->i32{{return p.len();}}fn relay(p:&{om}[u8;{n}])->i32{{return {child};}}fn main()->i32{{let x=128;let b=x.to_u8_checked();{first}return {call};}}");
            for raw_route in [false, true] {
                let end = at(&text, &format!("return {call};"));
                let mut steps = vec![effect(
                    at(&text, "main"),
                    if raw_route { 26 + w } else { 30 + n + 2 * w },
                    &[Effect::Enter(2)],
                )];
                steps.extend(seed(&text));
                if raw_route {
                    steps.extend([step(end, 1), construct(end, w)]);
                } else {
                    steps.extend(literal_steps(&text, &first, &literal, n));
                    let decl = at(&text, &first);
                    steps.extend([
                        step(decl, 1),
                        effect(decl, 1 + w, &[Effect::Transfer]),
                        step(decl, 1 + w),
                    ]);
                }
                // Main: A1/L1/C1. Relay: S1/A1/R1/L1/C1 =>24.
                // Leaf: S1/R1=>9. Invoke is1+arity+callee activation;
                // no owned-argument width or pairwise alias debit here.
                steps.extend([
                    step(at(&text, &call), 1),
                    effect(at(&text, &format!("&{om}a")), 1, &[Effect::Acquire(outer)]),
                    effect(at(&text, &call), 26, &[Effect::Enter(1)]),
                    step(at(&text, &child), 1),
                    effect(at(&text, &format!("&{im}*p")), 1, &[Effect::Acquire(inner)]),
                    effect(at(&text, &child), 11, &[Effect::Enter(0)]),
                    effect(at(&text, "p.len()"), 1, &[Effect::Len(n)]),
                    effect(
                        at(&text, "return p.len();"),
                        2,
                        &[Effect::Release, Effect::Return(0)],
                    ),
                    effect(
                        at(&text, &format!("return {child};")),
                        4,
                        &[Effect::Release, Effect::Return(1)],
                    ),
                ]);
                if !raw_route {
                    steps.push(step(end, 1 + w));
                }
                steps.push(effect(
                    end,
                    if raw_route { 3 + w } else { 3 + 2 * w },
                    &[Effect::Return(2)],
                ));
                assert_eq!(
                    steps.iter().map(|s| s.cost).sum::<usize>(),
                    if raw_route {
                        84 + 3 * w
                    } else {
                        92 + 2 * n + 8 * w
                    }
                );
                with_raw(&text, |_, typed, mut raw| {
                    assert_eq!(
                        (
                            raw.functions[0].locals.len(),
                            raw.functions[1].locals.len(),
                            raw.functions[2].locals.len()
                        ),
                        (1, 1, 6 + n)
                    );
                    if raw_route {
                        let main = &mut raw.functions[2];
                        main.locals.truncate(5);
                        main.locals.push(f::scalar(hir::Ty::I32, end));
                        let aggregate = AggregateSlot::try_from_aggregate(AggregateTy::FixedArray(
                            FixedArrayTy::check(hir::Ty::U8, n).unwrap(),
                        ))
                        .unwrap();
                        main.owners = vec![OwnerDecl {
                            aggregate,
                            kind: OwnerKind::Local { mutable: true },
                            span: end,
                        }];
                        main.loans[0].authority = AccessBase::Owner(OwnerPlaceId(0));
                        main.calls[0].result = CallResult::Scalar(LocalId(5));
                        main.blocks[0].statements.truncate(5);
                        main.blocks[0].statements.extend([
                            f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), end),
                            f::instruction(
                                OwnedInstruction::ConstructArray {
                                    destination: OwnerPlaceId(0),
                                    elements: vec![f::operand(4, end); n],
                                },
                                end,
                            ),
                            f::instruction(
                                OwnedInstruction::OpenCall(CallSiteId(0)),
                                at(&text, &call),
                            ),
                            f::instruction(
                                OwnedInstruction::PrepareBorrow {
                                    call: CallSiteId(0),
                                    argument: 0,
                                    loan: LoanId(0),
                                },
                                at(&text, &format!("&{om}a")),
                            ),
                        ]);
                        main.blocks[1].statements.clear();
                        main.blocks[1].terminator =
                            f::end(OwnedTerminatorKind::ReturnScalar(f::operand(5, end)), end);
                    }
                    let associated = if raw_route {
                        association::associate(raw, typed).unwrap()
                    } else {
                        association::lower_and_associate(typed).unwrap()
                    };
                    let witness = verified::verify_associated(associated).unwrap();
                    replay(
                        &witness,
                        2,
                        &steps,
                        n,
                        if raw_route {
                            "seed-authenticated raw reborrow"
                        } else {
                            "source reborrow"
                        },
                    );
                });
            }
        }
    }
}

#[test]
fn byte_storage_owned_call_and_result_independent_fuel_handoffs() {
    use super::super::consumer_fixtures as f;
    for n in [0, 1, 1024] {
        let w = n.max(1);
        let literal = format!("[{}]", vec!["b"; n].join(","));
        let decl = format!("let a:[u8;{n}]={literal};");
        let text = format!("fn relay(a:[u8;{n}])->[u8;{n}]{{return a;}}fn main()->i32{{let x=128;let b=x.to_u8_checked();{decl}let r=relay(a);return r.len();}}");
        let call = at(&text, "relay(a)");
        let argument = Span {
            start: call.end - 2,
            end: call.end - 1,
            ..call
        };
        let child_end = at(&text, "return a;");
        let child_value = inside(&text, "return a;", "a");
        let end = at(&text, "return r.len();");
        for raw_route in [false, true] {
            let mut steps = vec![effect(
                at(&text, "main"),
                if raw_route {
                    22 + 3 * w
                } else {
                    34 + n + 6 * w
                },
                &[Effect::Enter(1)],
            )];
            steps.extend(seed(&text));
            if raw_route {
                steps.extend([step(end, 1), construct(end, w)]);
            } else {
                steps.extend(literal_steps(&text, &decl, &literal, n));
                let decl = at(&text, &decl);
                steps.extend([
                    step(decl, 1),
                    effect(decl, 1 + w, &[Effect::Transfer]),
                    step(decl, 1 + w),
                ]);
            }
            // One owned argument: opening initializes one stage (cost2).
            // The argument snapshot is charged before any callee activation.
            steps.push(step(call, 2));
            if !raw_route {
                steps.extend([
                    step(argument, 1),
                    effect(argument, 1 + w, &[Effect::Transfer]),
                ]);
            }
            steps.push(effect(argument, 1 + w, &[Effect::Transfer]));
            if !raw_route {
                steps.push(step(argument, 1 + w));
            }
            // Invoke copies w cells only after the whole activation charge:
            // raw child O1 =>4+w; source child O2 =>8+2w.
            steps.push(effect(
                call,
                if raw_route { 6 + 2 * w } else { 10 + 3 * w },
                &[Effect::Transfer, Effect::Enter(0)],
            ));
            if !raw_route {
                steps.extend([
                    step(child_value, 1),
                    effect(child_value, 1 + w, &[Effect::Transfer]),
                    step(child_end, 1 + w),
                ]);
            }
            // ReturnOwned adds returned width to retained-owner teardown.
            steps.push(effect(
                child_end,
                if raw_route { 1 + 2 * w } else { 1 + 3 * w },
                &[Effect::Transfer, Effect::Return(0)],
            ));
            if !raw_route {
                let decl = at(&text, "let r=relay(a);");
                steps.extend([
                    step(decl, 1),
                    effect(decl, 1 + w, &[Effect::Transfer]),
                    step(decl, 1 + w),
                ]);
            }
            steps.push(effect(at(&text, "r.len()"), 1, &[Effect::Len(n)]));
            if !raw_route {
                steps.extend([step(end, 1 + w), step(end, 1 + w)]);
            }
            steps.push(effect(
                end,
                if raw_route { 2 + 3 * w } else { 2 + 6 * w },
                &[Effect::Return(1)],
            ));
            let total = steps.iter().map(|s| s.cost).sum::<usize>();
            assert_eq!(
                total,
                if raw_route {
                    42 + 12 * w
                } else {
                    72 + 2 * n + 30 * w
                }
            );
            with_raw(&text, |_, typed, mut raw| {
                assert_eq!(
                    (
                        raw.functions[0].owners.len(),
                        raw.functions[1].owners.len(),
                        raw.functions[1].locals.len()
                    ),
                    (2, 6, 6 + n)
                );
                let associated = if raw_route {
                    let child = &mut raw.functions[0];
                    child.owners.truncate(1);
                    child.blocks[0].statements.clear();
                    child.blocks[0].terminator =
                        f::end(OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)), child_end);
                    let main = &mut raw.functions[1];
                    main.locals.truncate(5);
                    main.locals.push(f::scalar(hir::Ty::I32, end));
                    let aggregate = main.owners[0].aggregate;
                    main.owners = [
                        OwnerKind::Local { mutable: false },
                        OwnerKind::StagedArgument {
                            call: CallSiteId(0),
                            argument: 0,
                        },
                        OwnerKind::CallResult {
                            call: CallSiteId(0),
                        },
                    ]
                    .map(|kind| OwnerDecl {
                        aggregate,
                        kind,
                        span: end,
                    })
                    .to_vec();
                    main.calls[0].arguments[0] = ArgumentSlot::Owned(OwnerPlaceId(1));
                    main.calls[0].result = CallResult::Owned(OwnerPlaceId(2));
                    main.blocks[0].statements.truncate(5);
                    main.blocks[0].statements.extend([
                        f::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), end),
                        f::instruction(
                            OwnedInstruction::ConstructArray {
                                destination: OwnerPlaceId(0),
                                elements: vec![f::operand(4, end); n],
                            },
                            end,
                        ),
                        f::instruction(OwnedInstruction::OpenCall(CallSiteId(0)), call),
                        f::instruction(
                            OwnedInstruction::PrepareOwned {
                                call: CallSiteId(0),
                                argument: 0,
                                source: OwnerPlaceId(0),
                            },
                            argument,
                        ),
                    ]);
                    main.blocks[1].statements = vec![f::instruction(
                        OwnedInstruction::ArrayLength {
                            destination: LocalId(5),
                            base: AccessBase::Owner(OwnerPlaceId(2)),
                        },
                        at(&text, "r.len()"),
                    )];
                    main.blocks[1].terminator =
                        f::end(OwnedTerminatorKind::ReturnScalar(f::operand(5, end)), end);
                    association::associate(raw, typed).unwrap()
                } else {
                    association::lower_and_associate(typed).unwrap()
                };
                let witness = verified::verify_associated(associated).unwrap();
                let label = if raw_route {
                    "raw owned handoff"
                } else {
                    "source owned handoff"
                };
                if n != 1024 {
                    replay(&witness, 1, &steps, n, label);
                } else {
                    // At maximum width, cover zero and exact/one-short at
                    // every independently listed debit, including each source
                    // receiver copy, preparation, invocation, owned return,
                    // result move, cleanup and final return. This is explicitly
                    // a boundary sweep, not an every-integer-budget claim.
                    let mut cuts = vec![0];
                    let mut paid = 0;
                    for step in &steps {
                        paid += step.cost;
                        cuts.extend([paid - 1, paid]);
                    }
                    cuts.sort_unstable();
                    cuts.dedup();
                    replay_budgets(&witness, 1, &steps, n, label, cuts);
                }
            });
        }
    }
}
