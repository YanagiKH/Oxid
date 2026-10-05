//! Reviewer-authored held-out ownership-native qualification tests.
use super::super::{consumer_fixtures as fx, execute};
use super::tests::{argv_fuel_harness, assert_result, block, scalar_output, Scratch};
use super::*;

fn mixed_parameter_fixture(
    values: [i32; 4],
    result_field: usize,
    guarded: bool,
) -> (SourceMap, VerifiedOwnedProgram, fx::Schedule) {
    use fx::*;
    let (sources, s) = context();
    let fields = [hir::Ty::Bool, hir::Ty::Unit, hir::Ty::I32];
    let field = |i| FieldId {
        record: RecordId(0),
        index: i,
    };
    let mut caller = function(
        0,
        ValueTy::Scalar([hir::Ty::I32, hir::Ty::Bool, hir::Ty::Unit][result_field]),
        s(0),
    );
    let local_types = [
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::I32,
        hir::Ty::Bool,
        hir::Ty::Bool,
        hir::Ty::Unit,
        hir::Ty::I32,
        hir::Ty::Bool,
        hir::Ty::Unit,
        hir::Ty::I32,
        hir::Ty::I32,
    ];
    caller.locals = local_types.into_iter().map(|t| scalar(t, s(0))).collect();
    caller.owners = vec![
        owner(OwnerKind::Local { mutable: false }, s(0)),
        owner(OwnerKind::Local { mutable: false }, s(0)),
        owner(OwnerKind::Local { mutable: false }, s(0)),
        owner(
            OwnerKind::StagedArgument {
                call: CallSiteId(0),
                argument: 0,
            },
            s(0),
        ),
        owner(
            OwnerKind::StagedArgument {
                call: CallSiteId(0),
                argument: 3,
            },
            s(0),
        ),
        owner(
            OwnerKind::CallResult {
                call: CallSiteId(0),
            },
            s(0),
        ),
    ];
    caller.calls = vec![CallDecl {
        target: hir::DefId(1),
        arguments: vec![
            ArgumentSlot::Owned(OwnerPlaceId(3)),
            ArgumentSlot::Borrow(LoanId(1)),
            ArgumentSlot::Scalar,
            ArgumentSlot::Owned(OwnerPlaceId(4)),
            ArgumentSlot::Borrow(LoanId(0)),
            ArgumentSlot::Scalar,
            ArgumentSlot::Scalar,
        ],
        result: CallResult::Owned(OwnerPlaceId(5)),
        parent: None,
        span: s(0),
    }];
    caller.loans = vec![
        LoanDecl {
            projection: Vec::new(),
            call: CallSiteId(0),
            argument: 4,
            authority: AccessBase::Owner(OwnerPlaceId(0)),
            kind: BorrowKind::Shared,
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            span: s(0),
        },
        LoanDecl {
            projection: Vec::new(),
            call: CallSiteId(0),
            argument: 1,
            authority: AccessBase::Owner(OwnerPlaceId(0)),
            kind: BorrowKind::Shared,
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            span: s(0),
        },
    ];
    let mut statements = vec![];
    let mut events = vec![(s(0), 88)];
    let mut next = 1;
    macro_rules! push {
        ($kind:expr,$cost:expr) => {{
            statements.push(fx::instruction($kind, s(next)));
            events.push((s(next), $cost));
            next += 1;
        }};
    }
    for (i, v) in [values[2], values[0], values[1], values[3]]
        .into_iter()
        .enumerate()
    {
        push!(
            OwnedInstruction::Scalar(Statement::Assign(Assign {
                destination: LocalId(i),
                value: Rvalue::I32(v),
                span: s(next)
            })),
            1
        );
    }
    for (i, value) in [
        (4, Rvalue::Bool(false)),
        (5, Rvalue::Bool(true)),
        (6, Rvalue::Unit),
    ] {
        push!(
            OwnedInstruction::Scalar(Statement::Assign(Assign {
                destination: LocalId(i),
                value,
                span: s(next)
            })),
            1
        );
    }
    // The outer span is intentionally different from the first embedded scalar span.
    statements[0].span = s(3000);
    for o in 0..3 {
        push!(OwnedInstruction::StorageLive(OwnerPlaceId(o)), 1);
        // Written order differs from declaration order; unit/bool/i32 storage has padding.
        push!(
            OwnedInstruction::Construct {
                destination: OwnerPlaceId(o),
                fields: vec![
                    (field(2), operand(o, s(next))),
                    (field(0), operand(4, s(next))),
                    (field(1), operand(6, s(next)))
                ]
            },
            4
        );
    }
    push!(OwnedInstruction::OpenCall(CallSiteId(0)), 3);
    push!(
        OwnedInstruction::PrepareOwned {
            call: CallSiteId(0),
            argument: 0,
            source: OwnerPlaceId(1)
        },
        4
    );
    push!(
        OwnedInstruction::PrepareBorrow {
            call: CallSiteId(0),
            argument: 1,
            loan: LoanId(1)
        },
        1
    );
    push!(
        OwnedInstruction::PrepareScalar {
            call: CallSiteId(0),
            argument: 2,
            value: operand(3, s(next))
        },
        1
    );
    push!(
        OwnedInstruction::PrepareOwned {
            call: CallSiteId(0),
            argument: 3,
            source: OwnerPlaceId(2)
        },
        4
    );
    push!(
        OwnedInstruction::PrepareBorrow {
            call: CallSiteId(0),
            argument: 4,
            loan: LoanId(0)
        },
        1
    );
    push!(
        OwnedInstruction::PrepareScalar {
            call: CallSiteId(0),
            argument: 5,
            value: operand(5, s(next))
        },
        1
    );
    push!(
        OwnedInstruction::PrepareScalar {
            call: CallSiteId(0),
            argument: 6,
            value: operand(6, s(next))
        },
        1
    );
    caller.loans[0].span = s(19);
    caller.loans[1].span = s(16);
    let invoke_span = s(next);
    events.push((invoke_span, 63));
    caller.blocks.push(block(
        std::mem::take(&mut statements),
        OwnedTerminatorKind::Invoke {
            call: CallSiteId(0),
            continuation: BlockId(1),
        },
        invoke_span,
    ));
    let mut callee = function(1, ValueTy::Owned(AggregateTy::Record(RecordId(0))), s(100));
    callee.parameters = vec![
        ParameterBinding::Owned(OwnerPlaceId(1)),
        ParameterBinding::Reference(ReferenceParamId(1)),
        ParameterBinding::Scalar(LocalId(0)),
        ParameterBinding::Owned(OwnerPlaceId(0)),
        ParameterBinding::Reference(ReferenceParamId(0)),
        ParameterBinding::Scalar(LocalId(1)),
        ParameterBinding::Scalar(LocalId(2)),
    ];
    callee.locals = (0..11)
        .map(|i| LocalDecl {
            ty: if i == 1 {
                hir::Ty::Bool
            } else if i == 2 {
                hir::Ty::Unit
            } else {
                hir::Ty::I32
            },
            kind: if i < 3 {
                LocalKind::Parameter
            } else {
                LocalKind::Temporary
            },
            span: s(100),
        })
        .collect();
    callee.owners = vec![
        owner(OwnerKind::Parameter { position: 3 }, s(100)),
        owner(OwnerKind::Parameter { position: 0 }, s(100)),
        owner(OwnerKind::Local { mutable: true }, s(100)),
    ];
    callee.references = vec![
        ReferenceDecl {
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            kind: BorrowKind::Shared,
            position: 4,
            span: s(100),
        },
        ReferenceDecl {
            referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0))))
                .unwrap(),
            kind: BorrowKind::Shared,
            position: 1,
            span: s(100),
        },
    ];
    next = 101;
    for (dest, base) in [
        (3, AccessBase::Owner(OwnerPlaceId(1))),
        (4, AccessBase::Owner(OwnerPlaceId(0))),
        (5, AccessBase::Parameter(ReferenceParamId(0))),
        (6, AccessBase::Parameter(ReferenceParamId(1))),
    ] {
        push!(
            OwnedInstruction::ReadField {
                destination: LocalId(dest),
                base,
                field: field(2)
            },
            1
        );
    }
    for (dest, left, right) in [(7, 3, 4), (8, 7, 5), (9, 8, 6), (10, 9, 0)] {
        push!(
            OwnedInstruction::Scalar(Statement::Assign(Assign {
                destination: LocalId(dest),
                value: Rvalue::CheckedI32 {
                    op: hir::ArithmeticOp::Add,
                    left: operand(left, s(next)),
                    right: operand(right, s(next)),
                    operator_span: s(next + 1000)
                },
                span: s(next)
            })),
            1
        );
    }
    push!(OwnedInstruction::StorageLive(OwnerPlaceId(2)), 1);
    push!(
        OwnedInstruction::MoveInitialize {
            destination: OwnerPlaceId(2),
            source: OwnerPlaceId(0)
        },
        4
    );
    for (fi, value) in [(2, 10), (0, 1), (1, 2)] {
        push!(
            OwnedInstruction::WriteField {
                base: AccessBase::Owner(OwnerPlaceId(2)),
                field: field(fi),
                value: operand(value, s(next))
            },
            1
        );
    }
    events.push((s(next), 15));
    callee.blocks.push(block(
        std::mem::take(&mut statements),
        OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(2)),
        s(next),
    ));
    next = 201;
    for (dest, base, fi) in [
        (7, OwnerPlaceId(5), 2),
        (8, OwnerPlaceId(5), 0),
        (9, OwnerPlaceId(5), 1),
        (10, OwnerPlaceId(0), 2),
    ] {
        push!(
            OwnedInstruction::ReadField {
                destination: LocalId(dest),
                base: AccessBase::Owner(base),
                field: field(fi)
            },
            1
        );
    }
    push!(
        OwnedInstruction::Scalar(Statement::Assign(Assign {
            destination: LocalId(11),
            value: Rvalue::CheckedI32 {
                op: hir::ArithmeticOp::Add,
                left: operand(7, s(next)),
                right: operand(10, s(next)),
                operator_span: s(next + 1000)
            },
            span: s(next)
        })),
        1
    );
    for owner in [0, 1, 2, 5] {
        push!(OwnedInstruction::StorageEnd(OwnerPlaceId(owner)), 4);
    }
    events.push((s(next), 22));
    caller.blocks.push(block(
        statements,
        OwnedTerminatorKind::ReturnScalar(operand([11, 8, 9][result_field], s(next))),
        s(next),
    ));
    let mut raw = RawOwnedProgram {
        records: vec![record(&fields, s(0))],
        functions: vec![caller, callee],
    };
    if guarded {
        let mut cycle = function(2, ValueTy::Scalar(hir::Ty::Unit), s(300));
        cycle.locals.push(scalar(hir::Ty::Unit, s(300)));
        cycle.blocks.push(block(
            vec![assign(0, Rvalue::Unit, s(301))],
            OwnedTerminatorKind::Goto(BlockId(0)),
            s(302),
        ));
        raw.functions.push(cycle);
    }
    let expected = values[0] + values[1] + 3 * values[2] + values[3];
    let result = [Scalar::I32(expected), Scalar::Bool(true), Scalar::Unit][result_field];
    let schedule = Schedule {
        result,
        entry: hir::DefId(0),
        events,
    };
    assert_eq!(schedule.fuel(), 263);
    let verified = verified::verify_owned(raw, &sources).unwrap();
    (sources, verified, schedule)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn review_heldout_interleaved_abi_real_llvm() {
    let scratch = Scratch::new();
    let mut cases = 0;
    for (n, values) in [
        [10, 20, 30, 100],
        [-19, 37, -11, 211],
        [-2_000_000_000, 1_000_000_000, 3, 4],
    ]
    .into_iter()
    .enumerate()
    {
        for result_field in 0..3 {
            for guarded in [false, true] {
                let (sources, witness, schedule) =
                    mixed_parameter_fixture(values, result_field, guarded);
                assert_eq!(
                    execute::run(&witness, Some(schedule.entry)),
                    Ok(schedule.result)
                );
                let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
                let name = format!("review-mixed-{n}-{result_field}-{guarded}");
                let binary = scratch.compile(&module, &name);
                assert_result(
                    scratch.run(&binary, &[]),
                    &scalar_output(schedule.result),
                    b"",
                    0,
                );
                cases += 1;
                if guarded && n == 0 && result_field == 0 {
                    let harness = argv_fuel_harness(&module, plan::MAX_FUEL);
                    let binary = scratch.compile(&harness, "review-mixed-all-fuel");
                    for fuel in 0..=263 {
                        let actual = scratch.run(&binary, &[fuel.to_string()]);
                        let reference = execute::run_limits(
                            &witness,
                            Some(schedule.entry),
                            execute::Limits {
                                fuel,
                                ..execute::Limits::default()
                            },
                        );
                        match schedule.failure(fuel) {
                            Some(span) => {
                                let message = RunFailure::Fuel(span)
                                    .diagnostic(&sources)
                                    .render_human(&sources);
                                assert_eq!(
                                    reference
                                        .unwrap_err()
                                        .diagnostic(&sources)
                                        .render_human(&sources),
                                    message
                                );
                                assert_result(actual, b"", message.as_bytes(), 1);
                            }
                            None => {
                                assert_eq!(reference, Ok(schedule.result));
                                assert_result(actual, &scalar_output(schedule.result), b"", 0);
                            }
                        }
                        cases += 1;
                    }
                }
            }
        }
    }
    eprintln!("reviewer held-out ABI: {cases} real-LLVM source-free processes; reverse-index/interleaved owned+shared-alias+i32+bool+unit parameters, reordered mixed fields, three sentinel sets, all263 fuel thresholds");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7; explicitly run the owned native gate"]
fn review_heldout_call_result_merge_real_llvm() {
    use fx::*;
    let scratch = Scratch::new();
    for guarded in [false, true] {
        for condition in [false, true] {
            let (sources, s) = context();
            let mut f = function(0, ValueTy::Scalar(hir::Ty::Bool), s(0));
            f.entry = BlockId(3);
            f.locals = [
                hir::Ty::Bool,
                hir::Ty::I32,
                hir::Ty::I32,
                hir::Ty::I32,
                hir::Ty::Bool,
                hir::Ty::Bool,
                hir::Ty::I32,
                hir::Ty::Bool,
                hir::Ty::Bool,
            ]
            .into_iter()
            .map(|t| scalar(t, s(0)))
            .collect();
            f.calls = vec![CallDecl {
                target: hir::DefId(1),
                arguments: vec![],
                result: CallResult::Scalar(LocalId(5)),
                parent: None,
                span: s(10),
            }];
            let checked = |dest, at| {
                assign(
                    dest,
                    Rvalue::CheckedI32 {
                        op: hir::ArithmeticOp::Add,
                        left: operand(1, s(at)),
                        right: operand(2, s(at)),
                        operator_span: s(at + 100),
                    },
                    s(at),
                )
            };
            f.blocks = vec![
                block(
                    vec![
                        checked(3, 10),
                        assign(4, Rvalue::Bool(true), s(11)),
                        instruction(OwnedInstruction::OpenCall(CallSiteId(0)), s(12)),
                    ],
                    OwnedTerminatorKind::Invoke {
                        call: CallSiteId(0),
                        continuation: BlockId(2),
                    },
                    s(13),
                ),
                block(
                    vec![checked(6, 20), assign(7, Rvalue::Bool(true), s(21))],
                    OwnedTerminatorKind::Goto(BlockId(2)),
                    s(22),
                ),
                block(
                    vec![],
                    OwnedTerminatorKind::ReturnScalar(operand(8, s(31))),
                    s(31),
                ),
                block(
                    vec![
                        assign(0, Rvalue::Bool(condition), s(1)),
                        assign(1, Rvalue::I32(7), s(2)),
                        assign(2, Rvalue::I32(8), s(3)),
                    ],
                    OwnedTerminatorKind::Branch {
                        condition: operand(0, s(4)),
                        then_block: BlockId(0),
                        else_block: BlockId(1),
                    },
                    s(4),
                ),
            ];
            f.blocks[2].merge = Some(BoolMerge {
                destination: LocalId(8),
                incoming: [
                    MergeInput {
                        predecessor: BlockId(1),
                        value: operand(7, s(30)),
                    },
                    MergeInput {
                        predecessor: BlockId(0),
                        value: operand(5, s(30)),
                    },
                ],
                operator_span: s(30),
                span: s(30),
            });
            let mut helper = function(1, ValueTy::Scalar(hir::Ty::Bool), s(40));
            helper.locals.push(scalar(hir::Ty::Bool, s(40)));
            helper.blocks.push(block(
                vec![assign(0, Rvalue::Bool(false), s(41))],
                OwnedTerminatorKind::ReturnScalar(operand(0, s(42))),
                s(42),
            ));
            let mut raw = RawOwnedProgram {
                records: vec![],
                functions: vec![f, helper],
            };
            if guarded {
                let mut cycle = function(2, ValueTy::Scalar(hir::Ty::Unit), s(50));
                cycle.locals.push(scalar(hir::Ty::Unit, s(50)));
                cycle.blocks.push(block(
                    vec![assign(0, Rvalue::Unit, s(51))],
                    OwnedTerminatorKind::Goto(BlockId(0)),
                    s(52),
                ));
                raw.functions.push(cycle);
            }
            let witness = verified::verify_owned(raw, &sources).unwrap();
            let result = Scalar::Bool(!condition);
            let mut events = vec![(s(0), 12), (s(1), 1), (s(2), 1), (s(3), 1), (s(4), 1)];
            events.extend(if condition {
                vec![
                    (s(10), 1),
                    (s(11), 1),
                    (s(12), 1),
                    (s(13), 2),
                    (s(41), 1),
                    (s(42), 1),
                ]
            } else {
                vec![(s(20), 1), (s(21), 1), (s(22), 1)]
            });
            events.extend([(s(30), 1), (s(31), 2)]);
            let schedule = Schedule {
                entry: hir::DefId(0),
                result,
                events,
            };
            assert_eq!(schedule.fuel(), if condition { 26 } else { 22 });
            assert_eq!(execute::run(&witness, Some(schedule.entry)), Ok(result));
            let module = native_module(&witness, Some(schedule.entry), &sources).unwrap();
            let binary =
                scratch.compile(&module, &format!("review-call-merge-{guarded}-{condition}"));
            assert_result(scratch.run(&binary, &[]), &scalar_output(result), b"", 0);
            if guarded {
                let harness = argv_fuel_harness(&module, plan::MAX_FUEL);
                let binary =
                    scratch.compile(&harness, &format!("review-call-merge-fuel-{condition}"));
                for fuel in 0..=schedule.fuel() {
                    let actual = scratch.run(&binary, &[fuel.to_string()]);
                    match schedule.failure(fuel) {
                        Some(span) => assert_result(
                            actual,
                            b"",
                            RunFailure::Fuel(span)
                                .diagnostic(&sources)
                                .render_human(&sources)
                                .as_bytes(),
                            1,
                        ),
                        None => assert_result(actual, &scalar_output(result), b"", 0),
                    }
                }
            }
        }
    }
    eprintln!("reviewer held-out phi: 54 real-LLVM executions across guarded/unguarded, nonzero entry, checked-arithmetic predecessor and call-result normal edge, both arms and exact fuel thresholds");
}

#[test]
fn review_heldout_actual_admission_edges() {
    use fx::*;
    for count in [256, 257] {
        let (sources, s) = context();
        let functions = (0..count)
            .map(|id| {
                let mut f = function(id, ValueTy::Scalar(hir::Ty::Unit), s(0));
                f.locals.push(scalar(hir::Ty::Unit, s(0)));
                f.blocks.push(block(
                    vec![assign(0, Rvalue::Unit, s(1))],
                    OwnedTerminatorKind::ReturnScalar(operand(0, s(2))),
                    s(2),
                ));
                f
            })
            .collect();
        let witness = verified::verify_owned(
            RawOwnedProgram {
                records: vec![],
                functions,
            },
            &sources,
        )
        .unwrap();
        let result = native_module(&witness, Some(hir::DefId(0)), &sources);
        if count == 256 {
            assert!(result.is_ok())
        } else {
            assert!(result.unwrap_err().message.contains("function count"))
        }
    }
    for count in [4096, 4097] {
        let (sources, s) = context();
        let mut f = function(0, ValueTy::Scalar(hir::Ty::Unit), s(0));
        f.locals.push(scalar(hir::Ty::Unit, s(0)));
        f.blocks = (0..count)
            .map(|id| {
                block(
                    if id == 0 {
                        vec![assign(0, Rvalue::Unit, s(1))]
                    } else {
                        vec![]
                    },
                    if id + 1 == count {
                        OwnedTerminatorKind::ReturnScalar(operand(0, s(2)))
                    } else {
                        OwnedTerminatorKind::Goto(BlockId(id + 1))
                    },
                    s(2),
                )
            })
            .collect();
        let witness = verified::verify_owned(
            RawOwnedProgram {
                records: vec![],
                functions: vec![f],
            },
            &sources,
        )
        .unwrap();
        let result = native_module(&witness, Some(hir::DefId(0)), &sources);
        if count == 4096 {
            assert!(result.is_ok())
        } else {
            assert!(result.unwrap_err().message.contains("aggregate blocks"))
        }
    }
    for count in [256, 257] {
        let (sources, s) = context();
        let mut f = function(0, ValueTy::Scalar(hir::Ty::Unit), s(0));
        f.locals = (0..count).map(|_| scalar(hir::Ty::Unit, s(0))).collect();
        f.blocks.push(block(
            (0..count)
                .map(|id| assign(id, Rvalue::Unit, s(1)))
                .collect(),
            OwnedTerminatorKind::ReturnScalar(operand(count - 1, s(2))),
            s(2),
        ));
        let witness = verified::verify_owned(
            RawOwnedProgram {
                records: vec![],
                functions: vec![f],
            },
            &sources,
        )
        .unwrap();
        let result = native_module(&witness, Some(hir::DefId(0)), &sources);
        if count == 256 {
            assert!(result.is_ok())
        } else {
            assert!(result
                .unwrap_err()
                .message
                .contains("scalar slots per function"))
        }
    }
    eprintln!("reviewer admission: inclusive/+1 actual functions256, blocks4096, scalar slots/function256");
}
