//! Raw Batch adapter. Expected costs/states come from a separate frozen Python schedule.
//! No production cost, transfer, overlap or CFG helpers calculate expectations here.
use super::{consumer_fixtures::*, *};
use std::collections::BTreeMap;
struct Origins {
    spans: BTreeMap<&'static str, Span>,
}
impl Origins {
    fn at(&self, label: &str) -> Span {
        self.spans[label]
    }
}
fn ass(o: &Origins, label: &str, destination: usize, value: Rvalue) -> OwnedStatement {
    assign(destination, value, o.at(label))
}
fn ins(o: &Origins, label: &str, kind: OwnedInstruction) -> OwnedStatement {
    instruction(kind, o.at(label))
}
fn op(o: &Origins, label: &str, local: usize) -> Operand {
    operand(local, o.at(label))
}
fn bb(
    o: &Origins,
    label: &str,
    statements: Vec<OwnedStatement>,
    kind: OwnedTerminatorKind,
) -> OwnedBlock {
    OwnedBlock {
        merge: None,
        span: o.at(label),
        statements,
        terminator: end(kind, o.at(label)),
    }
}
fn field(index: usize) -> FieldId {
    FieldId {
        record: RecordId(0),
        index,
    }
}
fn read(
    o: &Origins,
    label: &str,
    destination: usize,
    base: AccessBase,
    index: usize,
) -> OwnedStatement {
    ins(
        o,
        label,
        OwnedInstruction::ReadField {
            destination: LocalId(destination),
            base,
            field: field(index),
        },
    )
}
fn write(o: &Origins, label: &str, base: AccessBase, index: usize, value: usize) -> OwnedStatement {
    ins(
        o,
        label,
        OwnedInstruction::WriteField {
            base,
            field: field(index),
            value: op(o, label, value),
        },
    )
}
fn arithmetic(
    o: &Origins,
    label: &str,
    destination: usize,
    operator: hir::ArithmeticOp,
    left: usize,
    right: usize,
) -> OwnedStatement {
    ass(
        o,
        label,
        destination,
        Rvalue::CheckedI32 {
            op: operator,
            left: op(o, label, left),
            right: op(o, label, right),
            operator_span: o.at(label),
        },
    )
}
fn compare(
    o: &Origins,
    label: &str,
    destination: usize,
    operator: hir::ComparisonOp,
    left: usize,
    right: usize,
) -> OwnedStatement {
    ass(
        o,
        label,
        destination,
        Rvalue::CompareScalar {
            op: operator,
            left: op(o, label, left),
            right: op(o, label, right),
            operator_span: o.at(label),
        },
    )
}
fn open(o: &Origins, label: &str, call: usize) -> OwnedStatement {
    ins(o, label, OwnedInstruction::OpenCall(CallSiteId(call)))
}
fn borrow(o: &Origins, label: &str, call: usize, loan: usize) -> OwnedStatement {
    ins(
        o,
        label,
        OwnedInstruction::PrepareBorrow {
            call: CallSiteId(call),
            argument: 0,
            loan: LoanId(loan),
        },
    )
}
fn prep(o: &Origins, label: &str, call: usize, source: usize) -> OwnedStatement {
    ins(
        o,
        label,
        OwnedInstruction::PrepareOwned {
            call: CallSiteId(call),
            argument: 0,
            source: OwnerPlaceId(source),
        },
    )
}
fn invoke(call: usize, continuation: usize) -> OwnedTerminatorKind {
    OwnedTerminatorKind::Invoke {
        call: CallSiteId(call),
        continuation: BlockId(continuation),
    }
}
fn go(target: usize) -> OwnedTerminatorKind {
    OwnedTerminatorKind::Goto(BlockId(target))
}
fn reference_function(
    o: &Origins,
    id: usize,
    label: &str,
    result: hir::Ty,
    kind: BorrowKind,
    types: &[hir::Ty],
    scalar_parameter: bool,
) -> RawOwnedFunction {
    let span = o.at(label);
    let mut f = function(id, ValueTy::Scalar(result), span);
    f.references = vec![ReferenceDecl {
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        kind,
        position: 0,
        span,
    }];
    f.parameters = vec![ParameterBinding::Reference(ReferenceParamId(0))];
    f.locals = types.iter().map(|&ty| scalar(ty, span)).collect();
    if scalar_parameter {
        f.parameters.push(ParameterBinding::Scalar(LocalId(0)));
        f.locals[0].kind = LocalKind::Parameter;
    }
    f
}
pub(super) fn batch() -> (SourceMap, RawOwnedProgram, Schedule) {
    let mut text = String::new();
    let mut spans = BTreeMap::new();
    for &(label, _) in EXPECTED {
        if !spans.contains_key(label) {
            let start = text.len();
            text.push_str(label);
            let finish = text.len();
            text.push('\n');
            spans.insert(label, (start, finish));
        }
    }
    let mut sources = SourceMap::new();
    let file = sources.add("independent-batch-raw.ox".into(), text);
    let o = Origins {
        spans: spans
            .into_iter()
            .map(|(label, (start, end))| (label, Span { file, start, end }))
            .collect(),
    };
    use hir::Ty::{Bool, Unit, I32};
    let root = AccessBase::Owner(OwnerPlaceId(0));
    let parameter = AccessBase::Parameter(ReferenceParamId(0));
    let mut main = function(0, ValueTy::Scalar(I32), o.at("main.entry"));
    main.locals = [
        I32, Bool, Bool, I32, I32, Bool, I32, I32, I32, I32, Bool, Unit, I32, I32, Unit, Bool,
        Bool, I32,
    ]
    .iter()
    .map(|&ty| scalar(ty, main.span))
    .collect();
    main.places = vec![PlaceDecl {
        ty: I32,
        span: o.at("main.initialize_attempt"),
    }];
    main.owners = vec![
        owner(OwnerKind::Local { mutable: true }, main.span),
        owner(
            OwnerKind::StagedArgument {
                call: CallSiteId(3),
                argument: 0,
            },
            main.span,
        ),
        owner(
            OwnerKind::CallResult {
                call: CallSiteId(3),
            },
            main.span,
        ),
        owner(OwnerKind::Local { mutable: false }, main.span),
        owner(
            OwnerKind::StagedArgument {
                call: CallSiteId(4),
                argument: 0,
            },
            main.span,
        ),
    ];
    main.calls = vec![
        CallDecl {
            target: hir::DefId(1),
            arguments: vec![ArgumentSlot::Borrow(LoanId(0))],
            result: CallResult::Scalar(LocalId(11)),
            parent: None,
            span: o.at("main.open_retry"),
        },
        CallDecl {
            target: hir::DefId(3),
            arguments: vec![ArgumentSlot::Borrow(LoanId(1)), ArgumentSlot::Scalar],
            result: CallResult::Scalar(LocalId(14)),
            parent: None,
            span: o.at("main.open_dispatch"),
        },
        CallDecl {
            target: hir::DefId(4),
            arguments: vec![ArgumentSlot::Borrow(LoanId(2))],
            result: CallResult::Scalar(LocalId(15)),
            parent: None,
            span: o.at("main.open_done"),
        },
        CallDecl {
            target: hir::DefId(5),
            arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(1))],
            result: CallResult::Owned(OwnerPlaceId(2)),
            parent: None,
            span: o.at("main.open_relay"),
        },
        CallDecl {
            target: hir::DefId(6),
            arguments: vec![ArgumentSlot::Owned(OwnerPlaceId(4))],
            result: CallResult::Scalar(LocalId(17)),
            parent: None,
            span: o.at("main.open_finish"),
        },
    ];
    main.loans = [
        ("main.prepare_retry_borrow", BorrowKind::Exclusive),
        ("main.prepare_dispatch_borrow", BorrowKind::Exclusive),
        ("main.prepare_done_borrow", BorrowKind::Shared),
    ]
    .iter()
    .enumerate()
    .map(|(i, &(label, kind))| LoanDecl {
        projection: Vec::new(),
        call: CallSiteId(i),
        argument: 0,
        authority: root,
        kind,
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        span: o.at(label),
    })
    .collect();
    main.blocks.push(bb(
        &o,
        "main.goto_outer",
        vec![
            ass(&o, "main.zero", 0, Rvalue::I32(0)),
            ass(&o, "main.true", 1, Rvalue::Bool(true)),
            ass(&o, "main.one", 7, Rvalue::I32(1)),
            ass(&o, "main.three", 4, Rvalue::I32(3)),
            ass(&o, "main.two", 9, Rvalue::I32(2)),
            ins(
                &o,
                "main.live_state",
                OwnedInstruction::StorageLive(OwnerPlaceId(0)),
            ),
            ins(
                &o,
                "main.construct_state",
                OwnedInstruction::Construct {
                    destination: OwnerPlaceId(0),
                    fields: vec![
                        (field(0), op(&o, "main.construct_state", 0)),
                        (field(1), op(&o, "main.construct_state", 0)),
                        (field(2), op(&o, "main.construct_state", 0)),
                        (field(3), op(&o, "main.construct_state", 1)),
                    ],
                },
            ),
        ],
        go(1),
    ));
    main.blocks.push(bb(
        &o,
        "main.branch_outer",
        vec![read(&o, "main.read_active", 2, root, 3)],
        OwnedTerminatorKind::Branch {
            condition: op(&o, "main.branch_outer", 2),
            then_block: BlockId(2),
            else_block: BlockId(13),
        },
    ));
    main.blocks.push(bb(
        &o,
        "main.goto_inner",
        vec![ins(
            &o,
            "main.initialize_attempt",
            OwnedInstruction::Scalar(Statement::Initialize {
                place: Place {
                    id: PlaceId(0),
                    span: o.at("main.initialize_attempt"),
                },
                value: op(&o, "main.initialize_attempt", 0),
                span: o.at("main.initialize_attempt"),
            }),
        )],
        go(3),
    ));
    main.blocks.push(bb(
        &o,
        "main.branch_inner",
        vec![
            ass(
                &o,
                "main.load_attempt_test",
                3,
                Rvalue::Load(Place {
                    id: PlaceId(0),
                    span: o.at("main.load_attempt_test"),
                }),
            ),
            compare(&o, "main.compare_inner", 5, hir::ComparisonOp::Less, 3, 4),
        ],
        OwnedTerminatorKind::Branch {
            condition: op(&o, "main.branch_inner", 5),
            then_block: BlockId(4),
            else_block: BlockId(9),
        },
    ));
    main.blocks.push(bb(
        &o,
        "main.branch_retry",
        vec![
            ass(
                &o,
                "main.load_attempt_increment",
                6,
                Rvalue::Load(Place {
                    id: PlaceId(0),
                    span: o.at("main.load_attempt_increment"),
                }),
            ),
            arithmetic(
                &o,
                "main.increment_attempt",
                8,
                hir::ArithmeticOp::Add,
                6,
                7,
            ),
            ins(
                &o,
                "main.store_attempt",
                OwnedInstruction::Scalar(Statement::Store {
                    place: Place {
                        id: PlaceId(0),
                        span: o.at("main.store_attempt"),
                    },
                    value: op(&o, "main.store_attempt", 8),
                    operator_span: o.at("main.store_attempt"),
                    span: o.at("main.store_attempt"),
                }),
            ),
            compare(&o, "main.compare_retry", 10, hir::ComparisonOp::Less, 8, 9),
        ],
        OwnedTerminatorKind::Branch {
            condition: op(&o, "main.branch_retry", 10),
            then_block: BlockId(5),
            else_block: BlockId(7),
        },
    ));
    main.blocks.push(bb(
        &o,
        "main.invoke_retry",
        vec![
            open(&o, "main.open_retry", 0),
            borrow(&o, "main.prepare_retry_borrow", 0, 0),
        ],
        invoke(0, 6),
    ));
    main.blocks
        .push(bb(&o, "main.continue_inner", vec![], go(3)));
    main.blocks.push(bb(
        &o,
        "main.invoke_dispatch",
        vec![
            read(&o, "main.read_completed", 12, root, 0),
            arithmetic(&o, "main.make_job", 13, hir::ArithmeticOp::Add, 12, 7),
            open(&o, "main.open_dispatch", 1),
            borrow(&o, "main.prepare_dispatch_borrow", 1, 1),
            ins(
                &o,
                "main.prepare_dispatch_job",
                OwnedInstruction::PrepareScalar {
                    call: CallSiteId(1),
                    argument: 1,
                    value: op(&o, "main.prepare_dispatch_job", 13),
                },
            ),
        ],
        invoke(1, 8),
    ));
    main.blocks.push(bb(&o, "main.break_inner", vec![], go(9)));
    main.blocks.push(bb(
        &o,
        "main.invoke_done",
        vec![
            open(&o, "main.open_done", 2),
            borrow(&o, "main.prepare_done_borrow", 2, 2),
        ],
        invoke(2, 10),
    ));
    main.blocks.push(bb(
        &o,
        "main.branch_done",
        vec![],
        OwnedTerminatorKind::Branch {
            condition: op(&o, "main.branch_done", 15),
            then_block: BlockId(12),
            else_block: BlockId(11),
        },
    ));
    main.blocks
        .push(bb(&o, "main.continue_outer", vec![], go(1)));
    main.blocks.push(bb(
        &o,
        "main.break_outer",
        vec![
            ass(&o, "main.false", 16, Rvalue::Bool(false)),
            write(&o, "main.write_inactive", root, 3, 16),
        ],
        go(13),
    ));
    main.blocks.push(bb(
        &o,
        "main.invoke_relay",
        vec![
            open(&o, "main.open_relay", 3),
            prep(&o, "main.prepare_relay_owned", 3, 0),
        ],
        invoke(3, 14),
    ));
    main.blocks.push(bb(
        &o,
        "main.invoke_finish",
        vec![
            ins(
                &o,
                "main.live_completed",
                OwnedInstruction::StorageLive(OwnerPlaceId(3)),
            ),
            ins(
                &o,
                "main.move_result",
                OwnedInstruction::MoveInitialize {
                    destination: OwnerPlaceId(3),
                    source: OwnerPlaceId(2),
                },
            ),
            ins(
                &o,
                "main.end_relay_result",
                OwnedInstruction::StorageEnd(OwnerPlaceId(2)),
            ),
            open(&o, "main.open_finish", 4),
            prep(&o, "main.prepare_finish_owned", 4, 3),
        ],
        invoke(4, 15),
    ));
    main.blocks.push(bb(
        &o,
        "main.return",
        vec![
            ins(
                &o,
                "main.end_state",
                OwnedInstruction::StorageEnd(OwnerPlaceId(0)),
            ),
            ins(
                &o,
                "main.end_completed",
                OwnedInstruction::StorageEnd(OwnerPlaceId(3)),
            ),
        ],
        OwnedTerminatorKind::ReturnScalar(op(&o, "main.return", 17)),
    ));

    let mut retry = reference_function(
        &o,
        1,
        "retry.return",
        Unit,
        BorrowKind::Exclusive,
        &[I32, I32, I32, Unit],
        false,
    );
    retry.blocks.push(bb(
        &o,
        "retry.return",
        vec![
            read(&o, "retry.read_retries", 0, parameter, 1),
            ass(&o, "retry.one", 1, Rvalue::I32(1)),
            arithmetic(&o, "retry.add", 2, hir::ArithmeticOp::Add, 0, 1),
            write(&o, "retry.write_retries", parameter, 1, 2),
            ass(&o, "retry.unit", 3, Rvalue::Unit),
        ],
        OwnedTerminatorKind::ReturnScalar(op(&o, "retry.return", 3)),
    ));
    let mut commit = reference_function(
        &o,
        2,
        "commit.return",
        Unit,
        BorrowKind::Exclusive,
        &[I32, I32, I32, I32, I32, I32, I32, I32, Unit],
        true,
    );
    commit.blocks.push(bb(
        &o,
        "commit.return",
        vec![
            read(&o, "commit.read_completed", 1, parameter, 0),
            ass(&o, "commit.one", 2, Rvalue::I32(1)),
            arithmetic(&o, "commit.add_completed", 3, hir::ArithmeticOp::Add, 1, 2),
            write(&o, "commit.write_completed", parameter, 0, 3),
            read(&o, "commit.read_checksum", 4, parameter, 2),
            ass(&o, "commit.ten", 5, Rvalue::I32(10)),
            arithmetic(&o, "commit.multiply", 6, hir::ArithmeticOp::Multiply, 0, 5),
            arithmetic(&o, "commit.add_checksum", 7, hir::ArithmeticOp::Add, 4, 6),
            write(&o, "commit.write_checksum", parameter, 2, 7),
            ass(&o, "commit.unit", 8, Rvalue::Unit),
        ],
        OwnedTerminatorKind::ReturnScalar(op(&o, "commit.return", 8)),
    ));
    let mut dispatch = reference_function(
        &o,
        3,
        "dispatch.return",
        Unit,
        BorrowKind::Exclusive,
        &[I32, Unit],
        true,
    );
    dispatch.calls = vec![CallDecl {
        target: hir::DefId(2),
        arguments: vec![ArgumentSlot::Borrow(LoanId(0)), ArgumentSlot::Scalar],
        result: CallResult::Scalar(LocalId(1)),
        parent: None,
        span: o.at("dispatch.open_commit"),
    }];
    dispatch.loans = vec![LoanDecl {
        projection: Vec::new(),
        call: CallSiteId(0),
        argument: 0,
        authority: parameter,
        kind: BorrowKind::Exclusive,
        referent: BorrowedSlot::check(BorrowedTy::Exact(AggregateTy::Record(RecordId(0)))).unwrap(),
        span: o.at("dispatch.prepare_exclusive_child"),
    }];
    dispatch.blocks.push(bb(
        &o,
        "dispatch.invoke_commit",
        vec![
            open(&o, "dispatch.open_commit", 0),
            borrow(&o, "dispatch.prepare_exclusive_child", 0, 0),
            ins(
                &o,
                "dispatch.prepare_job",
                OwnedInstruction::PrepareScalar {
                    call: CallSiteId(0),
                    argument: 1,
                    value: op(&o, "dispatch.prepare_job", 0),
                },
            ),
        ],
        invoke(0, 1),
    ));
    dispatch.blocks.push(bb(
        &o,
        "dispatch.return",
        vec![],
        OwnedTerminatorKind::ReturnScalar(op(&o, "dispatch.return", 1)),
    ));
    let mut done = reference_function(
        &o,
        4,
        "done.return",
        Bool,
        BorrowKind::Shared,
        &[I32, I32, Bool],
        false,
    );
    done.blocks.push(bb(
        &o,
        "done.return",
        vec![
            read(&o, "done.read_completed", 0, parameter, 0),
            ass(&o, "done.six", 1, Rvalue::I32(6)),
            compare(&o, "done.compare", 2, hir::ComparisonOp::GreaterEqual, 0, 1),
        ],
        OwnedTerminatorKind::ReturnScalar(op(&o, "done.return", 2)),
    ));
    let mut relay = function(
        5,
        ValueTy::Owned(AggregateTy::Record(RecordId(0))),
        o.at("relay.return_owned"),
    );
    relay.parameters = vec![ParameterBinding::Owned(OwnerPlaceId(0))];
    relay.owners = vec![owner(OwnerKind::Parameter { position: 0 }, relay.span)];
    relay.blocks.push(bb(
        &o,
        "relay.return_owned",
        vec![],
        OwnedTerminatorKind::ReturnOwned(OwnerPlaceId(0)),
    ));
    let mut finish = function(6, ValueTy::Scalar(I32), o.at("finish.return"));
    finish.parameters = vec![ParameterBinding::Owned(OwnerPlaceId(0))];
    finish.owners = vec![owner(OwnerKind::Parameter { position: 0 }, finish.span)];
    finish.locals = (0..7).map(|_| scalar(I32, finish.span)).collect();
    finish.blocks.push(bb(
        &o,
        "finish.return",
        vec![
            read(&o, "finish.checksum", 0, root, 2),
            read(&o, "finish.completed", 1, root, 0),
            ass(&o, "finish.hundred", 2, Rvalue::I32(100)),
            arithmetic(&o, "finish.multiply", 3, hir::ArithmeticOp::Multiply, 1, 2),
            arithmetic(&o, "finish.subtotal", 4, hir::ArithmeticOp::Add, 0, 3),
            read(&o, "finish.retries", 5, root, 1),
            arithmetic(&o, "finish.total", 6, hir::ArithmeticOp::Add, 4, 5),
        ],
        OwnedTerminatorKind::ReturnScalar(op(&o, "finish.return", 6)),
    ));
    let schedule = Schedule {
        result: Scalar::I32(816),
        entry: hir::DefId(0),
        events: EXPECTED
            .iter()
            .map(|&(label, cost)| (o.at(label), cost))
            .collect(),
    };
    (
        sources,
        RawOwnedProgram {
            records: vec![record(&[I32, I32, I32, Bool], main.span)],
            functions: vec![main, retry, commit, dispatch, done, relay, finish],
        },
        schedule,
    )
}

// Frozen independent event schedule; no production cost helper is imported.
const EXPECTED: &[(&str, usize)] = &[
    ("main.entry", 112),
    ("main.zero", 1),
    ("main.true", 1),
    ("main.one", 1),
    ("main.three", 1),
    ("main.two", 1),
    ("main.live_state", 1),
    ("main.construct_state", 5),
    ("main.goto_outer", 1),
    ("main.read_active", 1),
    ("main.branch_outer", 1),
    ("main.initialize_attempt", 1),
    ("main.goto_inner", 1),
    ("main.load_attempt_test", 1),
    ("main.compare_inner", 1),
    ("main.branch_inner", 1),
    ("main.load_attempt_increment", 1),
    ("main.increment_attempt", 1),
    ("main.store_attempt", 1),
    ("main.compare_retry", 1),
    ("main.branch_retry", 1),
    ("main.open_retry", 1),
    ("main.prepare_retry_borrow", 1),
    ("main.invoke_retry", 14),
    ("retry.read_retries", 1),
    ("retry.one", 1),
    ("retry.add", 1),
    ("retry.write_retries", 1),
    ("retry.unit", 1),
    ("retry.return", 2),
    ("main.continue_inner", 1),
    ("main.load_attempt_test", 1),
    ("main.compare_inner", 1),
    ("main.branch_inner", 1),
    ("main.load_attempt_increment", 1),
    ("main.increment_attempt", 1),
    ("main.store_attempt", 1),
    ("main.compare_retry", 1),
    ("main.branch_retry", 1),
    ("main.read_completed", 1),
    ("main.make_job", 1),
    ("main.open_dispatch", 1),
    ("main.prepare_dispatch_borrow", 1),
    ("main.prepare_dispatch_job", 1),
    ("main.invoke_dispatch", 29),
    ("dispatch.open_commit", 1),
    ("dispatch.prepare_exclusive_child", 1),
    ("dispatch.prepare_job", 1),
    ("dispatch.invoke_commit", 20),
    ("commit.read_completed", 1),
    ("commit.one", 1),
    ("commit.add_completed", 1),
    ("commit.write_completed", 1),
    ("commit.read_checksum", 1),
    ("commit.ten", 1),
    ("commit.multiply", 1),
    ("commit.add_checksum", 1),
    ("commit.write_checksum", 1),
    ("commit.unit", 1),
    ("commit.return", 2),
    ("dispatch.return", 4),
    ("main.break_inner", 1),
    ("main.open_done", 1),
    ("main.prepare_done_borrow", 1),
    ("main.invoke_done", 13),
    ("done.read_completed", 1),
    ("done.six", 1),
    ("done.compare", 1),
    ("done.return", 2),
    ("main.branch_done", 1),
    ("main.continue_outer", 1),
    ("main.read_active", 1),
    ("main.branch_outer", 1),
    ("main.initialize_attempt", 1),
    ("main.goto_inner", 1),
    ("main.load_attempt_test", 1),
    ("main.compare_inner", 1),
    ("main.branch_inner", 1),
    ("main.load_attempt_increment", 1),
    ("main.increment_attempt", 1),
    ("main.store_attempt", 1),
    ("main.compare_retry", 1),
    ("main.branch_retry", 1),
    ("main.open_retry", 1),
    ("main.prepare_retry_borrow", 1),
    ("main.invoke_retry", 14),
    ("retry.read_retries", 1),
    ("retry.one", 1),
    ("retry.add", 1),
    ("retry.write_retries", 1),
    ("retry.unit", 1),
    ("retry.return", 2),
    ("main.continue_inner", 1),
    ("main.load_attempt_test", 1),
    ("main.compare_inner", 1),
    ("main.branch_inner", 1),
    ("main.load_attempt_increment", 1),
    ("main.increment_attempt", 1),
    ("main.store_attempt", 1),
    ("main.compare_retry", 1),
    ("main.branch_retry", 1),
    ("main.read_completed", 1),
    ("main.make_job", 1),
    ("main.open_dispatch", 1),
    ("main.prepare_dispatch_borrow", 1),
    ("main.prepare_dispatch_job", 1),
    ("main.invoke_dispatch", 29),
    ("dispatch.open_commit", 1),
    ("dispatch.prepare_exclusive_child", 1),
    ("dispatch.prepare_job", 1),
    ("dispatch.invoke_commit", 20),
    ("commit.read_completed", 1),
    ("commit.one", 1),
    ("commit.add_completed", 1),
    ("commit.write_completed", 1),
    ("commit.read_checksum", 1),
    ("commit.ten", 1),
    ("commit.multiply", 1),
    ("commit.add_checksum", 1),
    ("commit.write_checksum", 1),
    ("commit.unit", 1),
    ("commit.return", 2),
    ("dispatch.return", 4),
    ("main.break_inner", 1),
    ("main.open_done", 1),
    ("main.prepare_done_borrow", 1),
    ("main.invoke_done", 13),
    ("done.read_completed", 1),
    ("done.six", 1),
    ("done.compare", 1),
    ("done.return", 2),
    ("main.branch_done", 1),
    ("main.continue_outer", 1),
    ("main.read_active", 1),
    ("main.branch_outer", 1),
    ("main.initialize_attempt", 1),
    ("main.goto_inner", 1),
    ("main.load_attempt_test", 1),
    ("main.compare_inner", 1),
    ("main.branch_inner", 1),
    ("main.load_attempt_increment", 1),
    ("main.increment_attempt", 1),
    ("main.store_attempt", 1),
    ("main.compare_retry", 1),
    ("main.branch_retry", 1),
    ("main.open_retry", 1),
    ("main.prepare_retry_borrow", 1),
    ("main.invoke_retry", 14),
    ("retry.read_retries", 1),
    ("retry.one", 1),
    ("retry.add", 1),
    ("retry.write_retries", 1),
    ("retry.unit", 1),
    ("retry.return", 2),
    ("main.continue_inner", 1),
    ("main.load_attempt_test", 1),
    ("main.compare_inner", 1),
    ("main.branch_inner", 1),
    ("main.load_attempt_increment", 1),
    ("main.increment_attempt", 1),
    ("main.store_attempt", 1),
    ("main.compare_retry", 1),
    ("main.branch_retry", 1),
    ("main.read_completed", 1),
    ("main.make_job", 1),
    ("main.open_dispatch", 1),
    ("main.prepare_dispatch_borrow", 1),
    ("main.prepare_dispatch_job", 1),
    ("main.invoke_dispatch", 29),
    ("dispatch.open_commit", 1),
    ("dispatch.prepare_exclusive_child", 1),
    ("dispatch.prepare_job", 1),
    ("dispatch.invoke_commit", 20),
    ("commit.read_completed", 1),
    ("commit.one", 1),
    ("commit.add_completed", 1),
    ("commit.write_completed", 1),
    ("commit.read_checksum", 1),
    ("commit.ten", 1),
    ("commit.multiply", 1),
    ("commit.add_checksum", 1),
    ("commit.write_checksum", 1),
    ("commit.unit", 1),
    ("commit.return", 2),
    ("dispatch.return", 4),
    ("main.break_inner", 1),
    ("main.open_done", 1),
    ("main.prepare_done_borrow", 1),
    ("main.invoke_done", 13),
    ("done.read_completed", 1),
    ("done.six", 1),
    ("done.compare", 1),
    ("done.return", 2),
    ("main.branch_done", 1),
    ("main.continue_outer", 1),
    ("main.read_active", 1),
    ("main.branch_outer", 1),
    ("main.initialize_attempt", 1),
    ("main.goto_inner", 1),
    ("main.load_attempt_test", 1),
    ("main.compare_inner", 1),
    ("main.branch_inner", 1),
    ("main.load_attempt_increment", 1),
    ("main.increment_attempt", 1),
    ("main.store_attempt", 1),
    ("main.compare_retry", 1),
    ("main.branch_retry", 1),
    ("main.open_retry", 1),
    ("main.prepare_retry_borrow", 1),
    ("main.invoke_retry", 14),
    ("retry.read_retries", 1),
    ("retry.one", 1),
    ("retry.add", 1),
    ("retry.write_retries", 1),
    ("retry.unit", 1),
    ("retry.return", 2),
    ("main.continue_inner", 1),
    ("main.load_attempt_test", 1),
    ("main.compare_inner", 1),
    ("main.branch_inner", 1),
    ("main.load_attempt_increment", 1),
    ("main.increment_attempt", 1),
    ("main.store_attempt", 1),
    ("main.compare_retry", 1),
    ("main.branch_retry", 1),
    ("main.read_completed", 1),
    ("main.make_job", 1),
    ("main.open_dispatch", 1),
    ("main.prepare_dispatch_borrow", 1),
    ("main.prepare_dispatch_job", 1),
    ("main.invoke_dispatch", 29),
    ("dispatch.open_commit", 1),
    ("dispatch.prepare_exclusive_child", 1),
    ("dispatch.prepare_job", 1),
    ("dispatch.invoke_commit", 20),
    ("commit.read_completed", 1),
    ("commit.one", 1),
    ("commit.add_completed", 1),
    ("commit.write_completed", 1),
    ("commit.read_checksum", 1),
    ("commit.ten", 1),
    ("commit.multiply", 1),
    ("commit.add_checksum", 1),
    ("commit.write_checksum", 1),
    ("commit.unit", 1),
    ("commit.return", 2),
    ("dispatch.return", 4),
    ("main.break_inner", 1),
    ("main.open_done", 1),
    ("main.prepare_done_borrow", 1),
    ("main.invoke_done", 13),
    ("done.read_completed", 1),
    ("done.six", 1),
    ("done.compare", 1),
    ("done.return", 2),
    ("main.branch_done", 1),
    ("main.continue_outer", 1),
    ("main.read_active", 1),
    ("main.branch_outer", 1),
    ("main.initialize_attempt", 1),
    ("main.goto_inner", 1),
    ("main.load_attempt_test", 1),
    ("main.compare_inner", 1),
    ("main.branch_inner", 1),
    ("main.load_attempt_increment", 1),
    ("main.increment_attempt", 1),
    ("main.store_attempt", 1),
    ("main.compare_retry", 1),
    ("main.branch_retry", 1),
    ("main.open_retry", 1),
    ("main.prepare_retry_borrow", 1),
    ("main.invoke_retry", 14),
    ("retry.read_retries", 1),
    ("retry.one", 1),
    ("retry.add", 1),
    ("retry.write_retries", 1),
    ("retry.unit", 1),
    ("retry.return", 2),
    ("main.continue_inner", 1),
    ("main.load_attempt_test", 1),
    ("main.compare_inner", 1),
    ("main.branch_inner", 1),
    ("main.load_attempt_increment", 1),
    ("main.increment_attempt", 1),
    ("main.store_attempt", 1),
    ("main.compare_retry", 1),
    ("main.branch_retry", 1),
    ("main.read_completed", 1),
    ("main.make_job", 1),
    ("main.open_dispatch", 1),
    ("main.prepare_dispatch_borrow", 1),
    ("main.prepare_dispatch_job", 1),
    ("main.invoke_dispatch", 29),
    ("dispatch.open_commit", 1),
    ("dispatch.prepare_exclusive_child", 1),
    ("dispatch.prepare_job", 1),
    ("dispatch.invoke_commit", 20),
    ("commit.read_completed", 1),
    ("commit.one", 1),
    ("commit.add_completed", 1),
    ("commit.write_completed", 1),
    ("commit.read_checksum", 1),
    ("commit.ten", 1),
    ("commit.multiply", 1),
    ("commit.add_checksum", 1),
    ("commit.write_checksum", 1),
    ("commit.unit", 1),
    ("commit.return", 2),
    ("dispatch.return", 4),
    ("main.break_inner", 1),
    ("main.open_done", 1),
    ("main.prepare_done_borrow", 1),
    ("main.invoke_done", 13),
    ("done.read_completed", 1),
    ("done.six", 1),
    ("done.compare", 1),
    ("done.return", 2),
    ("main.branch_done", 1),
    ("main.continue_outer", 1),
    ("main.read_active", 1),
    ("main.branch_outer", 1),
    ("main.initialize_attempt", 1),
    ("main.goto_inner", 1),
    ("main.load_attempt_test", 1),
    ("main.compare_inner", 1),
    ("main.branch_inner", 1),
    ("main.load_attempt_increment", 1),
    ("main.increment_attempt", 1),
    ("main.store_attempt", 1),
    ("main.compare_retry", 1),
    ("main.branch_retry", 1),
    ("main.open_retry", 1),
    ("main.prepare_retry_borrow", 1),
    ("main.invoke_retry", 14),
    ("retry.read_retries", 1),
    ("retry.one", 1),
    ("retry.add", 1),
    ("retry.write_retries", 1),
    ("retry.unit", 1),
    ("retry.return", 2),
    ("main.continue_inner", 1),
    ("main.load_attempt_test", 1),
    ("main.compare_inner", 1),
    ("main.branch_inner", 1),
    ("main.load_attempt_increment", 1),
    ("main.increment_attempt", 1),
    ("main.store_attempt", 1),
    ("main.compare_retry", 1),
    ("main.branch_retry", 1),
    ("main.read_completed", 1),
    ("main.make_job", 1),
    ("main.open_dispatch", 1),
    ("main.prepare_dispatch_borrow", 1),
    ("main.prepare_dispatch_job", 1),
    ("main.invoke_dispatch", 29),
    ("dispatch.open_commit", 1),
    ("dispatch.prepare_exclusive_child", 1),
    ("dispatch.prepare_job", 1),
    ("dispatch.invoke_commit", 20),
    ("commit.read_completed", 1),
    ("commit.one", 1),
    ("commit.add_completed", 1),
    ("commit.write_completed", 1),
    ("commit.read_checksum", 1),
    ("commit.ten", 1),
    ("commit.multiply", 1),
    ("commit.add_checksum", 1),
    ("commit.write_checksum", 1),
    ("commit.unit", 1),
    ("commit.return", 2),
    ("dispatch.return", 4),
    ("main.break_inner", 1),
    ("main.open_done", 1),
    ("main.prepare_done_borrow", 1),
    ("main.invoke_done", 13),
    ("done.read_completed", 1),
    ("done.six", 1),
    ("done.compare", 1),
    ("done.return", 2),
    ("main.branch_done", 1),
    ("main.false", 1),
    ("main.write_inactive", 1),
    ("main.break_outer", 1),
    ("main.open_relay", 2),
    ("main.prepare_relay_owned", 5),
    ("main.invoke_relay", 14),
    ("relay.return_owned", 9),
    ("main.live_completed", 1),
    ("main.move_result", 5),
    ("main.end_relay_result", 5),
    ("main.open_finish", 2),
    ("main.prepare_finish_owned", 5),
    ("main.invoke_finish", 21),
    ("finish.checksum", 1),
    ("finish.completed", 1),
    ("finish.hundred", 1),
    ("finish.multiply", 1),
    ("finish.subtotal", 1),
    ("finish.retries", 1),
    ("finish.total", 1),
    ("finish.return", 5),
    ("main.end_state", 5),
    ("main.end_completed", 5),
    ("main.return", 29),
];

/// Independent failure variation: constructor checksum MAX, then the first
/// commit writes completed=1 before MAX+10 overflows. Root+literal add two fuel.
pub(super) fn overflow_after_write() -> (SourceMap, RawOwnedProgram, Span, Vec<(Span, usize)>) {
    let (mut sources, mut raw, schedule) = batch();
    let text = "independent initial checksum MAX";
    let file = sources.add("independent-batch-overflow.ox".into(), text.into());
    let origin = Span {
        file,
        start: 0,
        end: text.len(),
    };
    let main = &mut raw.functions[0];
    main.locals.push(scalar(hir::Ty::I32, origin));
    let construct = main.blocks[0]
        .statements
        .iter()
        .position(|i| matches!(i.kind, OwnedInstruction::Construct { .. }))
        .unwrap();
    main.blocks[0]
        .statements
        .insert(construct, assign(18, Rvalue::I32(i32::MAX), origin));
    let OwnedInstruction::Construct { fields, .. } =
        &mut main.blocks[0].statements[construct + 1].kind
    else {
        unreachable!()
    };
    fields
        .iter_mut()
        .find(|(field, _)| field.index == 2)
        .unwrap()
        .1 = operand(18, origin);
    let construct_origin = main.blocks[0].statements[construct + 1].span;
    let failure_origin = raw.functions[2].blocks[0].statements[7].span;
    let mut charges = vec![];
    for (i, (span, cost)) in schedule.events.into_iter().enumerate() {
        if span == construct_origin {
            charges.push((origin, 1));
        }
        charges.push((span, cost + usize::from(i == 0)));
        if span == failure_origin {
            break;
        }
    }
    assert_eq!(charges.iter().map(|(_, c)| c).sum::<usize>(), 235);
    (sources, raw, failure_origin, charges)
}
