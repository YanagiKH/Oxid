//! Test-only producer fault injection after real source lowering.
//!
//! A mutation is called raw-valid only after `verify_owned` returns its sealed
//! witness. The observer serializes the complete immutable witness, including
//! every instruction, and runs the existing consumer. No expected facts are
//! read here; an independent source model compares the emitted observations.
use super::*;
use crate::frontend::oir::owned::source::{lower, resolve, typeck};

const IDENTITY_SOURCE: &str =
    "struct A { n: i32 } struct B { n: i32 } fn main() -> i32 { let x = A { n: 6 }; return x.n; }";
const POSITIONS_SOURCE: &str = "struct T { n: i32 } fn pick(a: i32, left: T, p: &T, right: T, b: bool) -> i32 { return left.n * 10 + right.n + a; } fn main() -> i32 { let p = T { n: 0 }; return pick(3, T { n: 2 }, &p, T { n: 7 }, true); }";
const MODES_SOURCE: &str = "struct T { a: i32, b: i32 } fn read(p: &T) -> i32 { return p.a; } fn main() -> i32 { let mut x = T { b: 8, a: 3 }; return read(&x); }";
const REREAD_SOURCE: &str = "struct T { n: i32 } fn bump(p: &mut T) -> () { p.n = 9; return; } fn main() -> i32 { let mut x = T { n: 2 }; let snapshot = (x.n); bump(&mut x); return snapshot; }";
const NESTED_SOURCE: &str = "struct T { n: i32 } fn bump(p: &mut T) -> i32 { p.n = 9; return 0; } fn pick(a: i32, b: i32) -> i32 { return a; } fn main() -> i32 { let mut x = T { n: 2 }; return pick(x.n, bump(&mut x)); }";
const WEAKENING_SOURCE: &str = "struct T { n: i32 } fn pair(a: &mut T, b: &mut T) -> i32 { return a.n + b.n; } fn main() -> i32 { let mut x = T { n: 4 }; return pair(&mut x, &mut x); }";

#[derive(Clone, Copy, Debug)]
enum Mutation {
    BindingMutability,
    NominalIdentity,
    DiagnosticOrigin,
    ParameterPositions,
    ReferenceMode,
    FieldIdentity,
    SnapshotReread,
    NestedSnapshot,
    BorrowWeakening,
}

const MUTATIONS: [Mutation; 9] = [
    Mutation::BindingMutability,
    Mutation::NominalIdentity,
    Mutation::DiagnosticOrigin,
    Mutation::ParameterPositions,
    Mutation::ReferenceMode,
    Mutation::FieldIdentity,
    Mutation::SnapshotReread,
    Mutation::NestedSnapshot,
    Mutation::BorrowWeakening,
];

impl Mutation {
    fn id(self) -> &'static str {
        match self {
            Self::BindingMutability => "producer_binding_mutability",
            Self::NominalIdentity => "producer_nominal_identity",
            Self::DiagnosticOrigin => "producer_diagnostic_origin",
            Self::ParameterPositions => "producer_parameter_positions",
            Self::ReferenceMode => "producer_reference_mode",
            Self::FieldIdentity => "producer_field_identity",
            Self::SnapshotReread => "producer_snapshot_reread",
            Self::NestedSnapshot => "producer_nested_snapshot",
            Self::BorrowWeakening => "producer_borrow_weakening",
        }
    }

    fn source(self) -> &'static str {
        match self {
            Self::BindingMutability | Self::NominalIdentity | Self::DiagnosticOrigin => {
                IDENTITY_SOURCE
            }
            Self::ParameterPositions => POSITIONS_SOURCE,
            Self::ReferenceMode | Self::FieldIdentity => MODES_SOURCE,
            Self::SnapshotReread => REREAD_SOURCE,
            Self::NestedSnapshot => NESTED_SOURCE,
            Self::BorrowWeakening => WEAKENING_SOURCE,
        }
    }

    fn recipe(self) -> &'static str {
        match self {
            Self::BindingMutability => "Change the immutable local owner's raw mutable flag to true",
            Self::NominalIdentity => "Retag every used A owner and its constructor/read field IDs as same-layout B; preserve both declarations",
            Self::DiagnosticOrigin => "Replace all statement diagnostic origins with the valid enclosing function-name span",
            Self::ParameterPositions => "Swap owned parameter bindings at positions 1 and 3 and update both OwnerKind parameter positions coherently",
            Self::ReferenceMode => "Change the callee reference and matching caller loan from Shared to Exclusive on an already mutable owner",
            Self::FieldIdentity => "Change the callee's ReadField from field index 0 to same-typed field index 1",
            Self::SnapshotReread => "Replace the return expression's scalar Copy of snapshot with a new ReadField of x.n after bump returns",
            Self::NestedSnapshot => "Keep outer OpenCall in place; move ReadField(x.n) and PrepareScalar(outer,0) into bump's continuation, before PrepareScalar(outer,1); change bump.parent from (outer,1) to (outer,0)",
            Self::BorrowWeakening => "Change both callee reference declarations and both matching caller loans from Exclusive to Shared, preserving argument descriptors and all instructions",
        }
    }

    fn apply(self, raw: &mut RawOwnedProgram, sources: &SourceMap) {
        match self {
            Self::BindingMutability => {
                let owner = raw.functions[0]
                    .owners
                    .iter_mut()
                    .find(|owner| matches!(owner.kind, OwnerKind::Local { mutable: false }))
                    .expect("original immutable source binding");
                owner.kind = OwnerKind::Local { mutable: true };
            }
            Self::NominalIdentity => {
                let function = &mut raw.functions[0];
                for owner in &mut function.owners {
                    assert_eq!(owner.aggregate(), AggregateTy::Record(RecordId(0)));
                    owner.aggregate =
                        AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(1)))
                            .unwrap();
                }
                for block in &mut function.blocks {
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
            Self::DiagnosticOrigin => {
                let function = &mut raw.functions[0];
                for block in &mut function.blocks {
                    for statement in &mut block.statements {
                        statement.diagnostic_origins = Some(DiagnosticOrigins {
                            primary: function.span,
                            cause: function.span,
                        });
                    }
                }
            }
            Self::ParameterPositions => {
                let function = &mut raw.functions[0];
                function.parameters.swap(1, 3);
                function.owners[0].kind = OwnerKind::Parameter { position: 3 };
                function.owners[1].kind = OwnerKind::Parameter { position: 1 };
            }
            Self::ReferenceMode => {
                assert_eq!(raw.functions[0].references[0].kind, BorrowKind::Shared);
                assert_eq!(raw.functions[1].loans[0].kind, BorrowKind::Shared);
                raw.functions[0].references[0].kind = BorrowKind::Exclusive;
                raw.functions[1].loans[0].kind = BorrowKind::Exclusive;
            }
            Self::FieldIdentity => {
                let mut changed = 0;
                for block in &mut raw.functions[0].blocks {
                    for statement in &mut block.statements {
                        if let OwnedInstruction::ReadField { field, .. } = &mut statement.kind {
                            assert_eq!(field.index, 0);
                            field.index = 1;
                            changed += 1;
                        }
                    }
                }
                assert_eq!(changed, 1);
            }
            Self::SnapshotReread => {
                let function = &mut raw.functions[1];
                let owner = OwnerPlaceId(
                    function
                        .owners
                        .iter()
                        .position(|owner| text_at(sources, owner.span) == "x")
                        .expect("source x binding"),
                );
                let mut changed = 0;
                for block in &mut function.blocks {
                    for statement in &mut block.statements {
                        if text_at(sources, statement.span) == "snapshot" {
                            let OwnedInstruction::Scalar(Statement::Assign(Assign {
                                destination,
                                ..
                            })) = statement.kind
                            else {
                                panic!("return snapshot must copy its captured value")
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
            }
            Self::NestedSnapshot => move_snapshot_after_nested_call(&mut raw.functions[2], sources),
            Self::BorrowWeakening => {
                assert_eq!(raw.functions[0].references.len(), 2);
                assert_eq!(raw.functions[1].loans.len(), 2);
                for reference in &mut raw.functions[0].references {
                    assert_eq!(reference.kind, BorrowKind::Exclusive);
                    reference.kind = BorrowKind::Shared;
                }
                for loan in &mut raw.functions[1].loans {
                    assert_eq!(loan.kind, BorrowKind::Exclusive);
                    loan.kind = BorrowKind::Shared;
                }
            }
        }
    }
}

fn text_at(sources: &SourceMap, at: Span) -> &str {
    &sources.get(at.file).text()[at.start..at.end]
}

fn move_snapshot_after_nested_call(function: &mut RawOwnedFunction, sources: &SourceMap) {
    let outer = CallSiteId(
        function
            .calls
            .iter()
            .position(|call| call.target == hir::DefId(1))
            .unwrap(),
    );
    let nested = CallSiteId(
        function
            .calls
            .iter()
            .position(|call| call.target == hir::DefId(0))
            .unwrap(),
    );
    assert_eq!(function.calls[nested.0].parent, Some((outer, 1)));
    let before = function
        .blocks
        .iter()
        .position(|block| {
            matches!(block.terminator.as_ref().map(|term| &term.kind),
            Some(OwnedTerminatorKind::Invoke { call, .. }) if *call == nested)
        })
        .expect("nested call invocation");
    let OwnedTerminatorKind::Invoke { continuation, .. } =
        function.blocks[before].terminator.as_ref().unwrap().kind
    else {
        unreachable!()
    };
    let mut delayed = Vec::new();
    function.blocks[before].statements.retain(|statement| {
        let move_after = match statement.kind {
            OwnedInstruction::ReadField { .. } => text_at(sources, statement.span) == "x.n",
            OwnedInstruction::PrepareScalar { call, argument, .. } => {
                call == outer && argument == 0
            }
            _ => false,
        };
        if move_after {
            delayed.push(statement.clone());
        }
        !move_after
    });
    assert_eq!(
        delayed.len(),
        2,
        "move the read and first preparation together"
    );
    assert!(matches!(
        delayed[0].kind,
        OwnedInstruction::ReadField { .. }
    ));
    assert!(
        matches!(delayed[1].kind, OwnedInstruction::PrepareScalar { call, argument: 0, .. } if call == outer)
    );
    function.calls[nested.0].parent = Some((outer, 0));
    function.blocks[continuation.0]
        .statements
        .splice(0..0, delayed);
}

fn owner_key(key: storage::OwnerKey) -> String {
    object([
        ("frame", key.frame.to_string()),
        ("activation", key.activation.to_string()),
        ("owner", key.owner.to_string()),
        ("generation", key.generation.to_string()),
    ])
}

fn loan_key(key: storage::LoanKey) -> String {
    object([
        ("frame", key.frame.to_string()),
        ("activation", key.activation.to_string()),
        ("loan", key.loan.to_string()),
        ("instance", key.instance.to_string()),
    ])
}

fn event(value: &execute::Event) -> String {
    use execute::Event;
    match value {
        Event::EnumTagRead(..) | Event::EnumPayloadRead(..) | Event::EnumBind(..) => {
            unreachable!("enum events are outside this predecessor qualification schema")
        }
        Event::ReadIndex(..) | Event::WriteIndex(..) | Event::ArrayLength(..) => {
            unreachable!("source arrays and production array witnesses remain gated")
        }
        Event::Charge(at, cost) => object([
            ("event", json_string("Charge")),
            ("span", span(*at)),
            ("cost", cost.to_string()),
        ]),
        Event::Enter(function, activation) => object([
            ("event", json_string("Enter")),
            ("function", function.0.to_string()),
            ("activation", activation.to_string()),
        ]),
        Event::WriteField(owner, field, value) => object([
            ("event", json_string("WriteField")),
            ("owner", owner_key(*owner)),
            ("field", field_id(*field)),
            ("value", result(Ok(*value))),
        ]),
        Event::Transfer(from, to) => object([
            ("event", json_string("Transfer")),
            ("from", owner_key(*from)),
            ("to", owner_key(*to)),
        ]),
        Event::Acquire(loan, owner, kind) => object([
            ("event", json_string("Acquire")),
            ("loan", loan_key(*loan)),
            ("owner", owner_key(*owner)),
            ("mode", borrow_kind(*kind)),
        ]),
        Event::Release(loan) => {
            object([("event", json_string("Release")), ("loan", loan_key(*loan))])
        }
        Event::Return(function) => object([
            ("event", json_string("Return")),
            ("function", function.0.to_string()),
        ]),
    }
}

fn observe(program: &SourceProgram, entry: Option<hir::DefId>, sources: &SourceMap) -> String {
    let reference = result(program.run(entry, sources));
    let entry = entry.expect("mutant fixtures have a scalar main entry");
    let mut events = Vec::new();
    let observed = execute::run_observed(
        &program.witness,
        entry,
        execute::Limits::default(),
        &mut events,
    )
    .map_err(|error| error.diagnostic(sources));
    assert_eq!(
        result(observed),
        reference,
        "ordinary and observed sealed consumers agree"
    );
    object([
        ("reference", reference),
        ("raw_view", raw_view(program)),
        ("frames", frames(program, sources)),
        ("events", array(events.iter().map(event))),
        (
            "charge_events",
            array(events.iter().filter_map(|event| {
                if let execute::Event::Charge(at, cost) = event {
                    Some(object([("cost", cost.to_string()), ("span", span(*at))]))
                } else {
                    None
                }
            })),
        ),
        ("runtime_usage", runtime_usage(program, &events)),
    ])
}

fn mutation_receipt(mutation: Mutation, text: String, output: &Path) -> String {
    // Establish the real original check/reference outcome before touching raw IR.
    let original = receipt(mutation.id(), text.clone(), Options::default(), output);
    let mut sources = SourceMap::new();
    let file = sources.add(format!("{}.ox", mutation.id()), text);
    let source = sources.get(file);
    let original_execution = match checked_source(source, &sources) {
        Ok((program, entry)) => observe(&program, entry, &sources),
        Err(_) => "null".into(),
    };
    let tokens = lexer::lex(source).expect("original fixture lexes");
    let ast = parser::parse_with_mode(source, tokens, parser::SourceMode::OwnedCandidate)
        .expect("original fixture parses in owned-candidate mode");
    let resolved = resolve::resolve(source, &ast).expect("original fixture resolves");
    let typed = typeck::check(resolved).expect("original fixture typechecks");
    let entry = typed.entry();
    let mut raw = lower::lower(&typed).expect("original fixture lowers before mutation");
    let original_lowered = array(raw.functions.iter().map(raw_function));
    mutation.apply(&mut raw, &sources);
    // Only this call may construct the witness. Panic rather than mislabel a
    // rejected mutation as a successful raw-valid correspondence counterexample.
    let witness = verified::verify_owned(raw, &sources)
        .unwrap_or_else(|error| panic!("{} failed raw verification: {error:?}", mutation.id()));
    let program = SourceProgram { witness };
    let observed = observe(&program, entry, &sources);
    let mutant = format!(
        "{{\"id\":{},\"check\":{{\"accepted\":true,\"diagnostics\":[]}},{}",
        json_string(mutation.id()),
        &observed[1..]
    );
    object([
        ("id", json_string(mutation.id())),
        ("mutation", json_string(&format!("{mutation:?}"))),
        ("recipe", json_string(mutation.recipe())),
        ("raw_valid", "true".into()),
        (
            "witness",
            json_string("verified::verify_owned returned a sealed witness"),
        ),
        ("original", original),
        ("original_execution", original_execution),
        ("lowered_functions_before_mutation", original_lowered),
        ("mutant", mutant),
    ])
}

#[test]
fn producer_mutations_require_actual_witnesses_and_preserve_original_outcomes() {
    for (mutation, original, changed) in [
        (Mutation::BindingMutability, 6, 6),
        (Mutation::NominalIdentity, 6, 6),
        (Mutation::DiagnosticOrigin, 6, 6),
        (Mutation::ParameterPositions, 30, 75),
        (Mutation::ReferenceMode, 3, 3),
        (Mutation::FieldIdentity, 3, 8),
        (Mutation::SnapshotReread, 2, 9),
        (Mutation::NestedSnapshot, 2, 9),
    ] {
        let receipt = mutation_receipt(mutation, mutation.source().into(), Path::new("."));
        let baseline = receipt
            .split("\"original\":")
            .nth(1)
            .unwrap()
            .split("\"original_execution\":")
            .next()
            .unwrap();
        assert!(baseline.contains("\"accepted\":true"));
        assert!(
            baseline.contains(&format!("\"result\":{original},\"failure\":null")),
            "missing original result: {mutation:?}"
        );
        let mutant = receipt.split("\"mutant\":").nth(1).unwrap();
        assert!(
            mutant.contains(&format!("\"result\":{changed},\"failure\":null")),
            "missing actual mutant result: {mutation:?}"
        );
        assert!(receipt.contains("\"raw_valid\":true"));
        assert!(receipt.contains("\"lowered_functions_before_mutation\":["));
    }
}

#[test]
fn coherent_borrow_weakening_changes_original_e0311_into_verified_acceptance() {
    let receipt = mutation_receipt(
        Mutation::BorrowWeakening,
        WEAKENING_SOURCE.into(),
        Path::new("."),
    );
    let original = receipt
        .split("\"original\":")
        .nth(1)
        .unwrap()
        .split("\"original_execution\":")
        .next()
        .unwrap();
    assert!(original.contains("\"accepted\":false"));
    assert!(original.contains("\"code\":\"E0311\""));
    assert!(!original.contains("\"reference\""));
    let mutant = receipt.split("\"mutant\":").nth(1).unwrap();
    assert!(mutant.contains("\"accepted\":true"));
    assert!(mutant.contains("\"result\":8,\"failure\":null"));
    assert!(mutant.contains("\"mode\":\"shared\""));
    assert!(!mutant.contains("\"mode\":\"exclusive\""));
}

/// Read source bytes only. The caller freezes and hashes source expectations
/// independently before invoking this generator and compares receipts afterward.
#[test]
#[ignore = "producer mutant receipts; requires frozen source and fresh output directories"]
fn emit_producer_mutant_receipts() {
    let input =
        std::env::var_os("OXID_UNIT4B_MUTANT_SOURCE_INPUT_DIR").expect("frozen source directory");
    let output =
        std::env::var_os("OXID_UNIT4B_MUTANT_OUTPUT_DIR").expect("fresh mutant receipt directory");
    let input = Path::new(&input);
    let output = Path::new(&output);
    fs::create_dir(output).expect("fresh output directory must not already exist");
    for mutation in MUTATIONS {
        let id = mutation.id();
        let text = fs::read_to_string(input.join(format!("{id}.ox")))
            .expect("read frozen original source");
        assert_eq!(
            text,
            mutation.source(),
            "fixture bytes changed from the pre-trace draft"
        );
        let receipt = mutation_receipt(mutation, text, output);
        fs::write(output.join(format!("{id}.json")), format!("{receipt}\n"))
            .expect("write full mutant receipt");
    }
}
