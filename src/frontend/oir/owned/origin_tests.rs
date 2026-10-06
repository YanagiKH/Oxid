//! Raw provenance is diagnostic metadata, never ownership authority.
use super::*;

pub(super) fn context() -> (SourceMap, Span) {
    let mut sources = SourceMap::new();
    let file = sources.add("owned-origins.ox".into(), "x".repeat(512));
    (
        sources,
        Span {
            file,
            start: 0,
            end: 1,
        },
    )
}
pub(super) fn at(s: Span, start: usize) -> Span {
    Span {
        start,
        end: start + 1,
        ..s
    }
}
pub(super) fn operand(s: Span) -> Operand {
    Operand {
        local: LocalId(0),
        span: s,
    }
}
pub(super) fn ins(kind: OwnedInstruction, s: Span) -> OwnedStatement {
    OwnedStatement {
        kind,
        span: s,
        diagnostic_origins: None,
    }
}
pub(super) fn term(kind: OwnedTerminatorKind, s: Span) -> Option<OwnedTerminator> {
    Some(OwnedTerminator {
        kind,
        span: s,
        diagnostic_origins: None,
    })
}
pub(super) fn function(id: usize, s: Span) -> RawOwnedFunction {
    RawOwnedFunction {
        id: hir::DefId(id),
        span: s,
        result: ValueTy::Scalar(hir::Ty::Unit),
        parameters: vec![],
        locals: vec![LocalDecl {
            ty: hir::Ty::Unit,
            kind: LocalKind::Temporary,
            span: s,
        }],
        places: vec![],
        owners: vec![],
        references: vec![],
        calls: vec![],
        loans: vec![],
        matches: Vec::new(),
        entry: BlockId(0),
        blocks: vec![OwnedBlock {
            merge: None,
            span: s,
            statements: vec![ins(
                OwnedInstruction::Scalar(Statement::Assign(Assign {
                    destination: LocalId(0),
                    value: Rvalue::Unit,
                    span: s,
                })),
                s,
            )],
            terminator: term(OwnedTerminatorKind::ReturnScalar(operand(s)), s),
        }],
    }
}
pub(super) fn raw(s: Span) -> RawOwnedProgram {
    RawOwnedProgram {
        builtins: BuiltinOrigins::None,
        enums: vec![],
        records: vec![RawRecordDecl {
            id: RecordId(0),
            span: s,
            fields: vec![],
        }],
        functions: vec![function(0, s)],
    }
}
pub(super) fn owner(f: &mut RawOwnedFunction, kind: OwnerKind, s: Span) -> OwnerPlaceId {
    let id = OwnerPlaceId(f.owners.len());
    f.owners.push(OwnerDecl {
        aggregate: AggregateSlot::try_from_aggregate(AggregateTy::Record(RecordId(0))).unwrap(),
        kind,
        span: s,
    });
    if let OwnerKind::Parameter { position } = kind {
        assert_eq!(position, f.parameters.len());
        f.parameters.push(ParameterBinding::Owned(id));
    } else if matches!(kind, OwnerKind::Local { .. } | OwnerKind::Temporary) {
        f.blocks[0]
            .statements
            .push(ins(OwnedInstruction::StorageLive(id), s));
    }
    id
}
pub(super) fn construct(f: &mut RawOwnedFunction, id: OwnerPlaceId, s: Span) {
    f.blocks[0].statements.push(ins(
        OwnedInstruction::Construct {
            destination: id,
            fields: vec![],
        },
        s,
    ));
}
pub(super) fn fail(raw: RawOwnedProgram, sources: &SourceMap) -> OwnedFailure {
    verify_owned(raw, sources).expect_err("invalid input must not obtain a witness")
}
fn origins(s: Span, primary: usize, cause: usize) -> DiagnosticOrigins {
    DiagnosticOrigins {
        primary: at(s, primary),
        cause: at(s, cause),
    }
}

#[test]
fn supplied_origins_validate_both_spans_on_inactive_and_unreachable_operations() {
    let (sources, s) = context();
    for unreachable in [false, true] {
        for terminator in [false, true] {
            for cause in [false, true] {
                let mut p = raw(s);
                let f = &mut p.functions[0];
                if unreachable {
                    f.blocks.push(f.blocks[0].clone());
                }
                let b = f.blocks.last_mut().unwrap();
                let invalid = Span { end: 513, ..s };
                let o = DiagnosticOrigins {
                    primary: if cause { s } else { invalid },
                    cause: if cause { invalid } else { s },
                };
                if terminator {
                    b.terminator.as_mut().unwrap().diagnostic_origins = Some(o);
                } else {
                    b.statements[0].diagnostic_origins = Some(o);
                }
                let e = fail(p, &sources);
                assert_eq!(e.kind, OwnedFailureKind::Malformed(Malformed::Span));
                assert_eq!(e.primary.get(), Some(invalid));
                assert!(e.context.is_none());
            }
        }
    }
}

#[test]
fn origin_only_admission_has_exact_payload_and_work_without_ownership_activity() {
    let (sources, s) = context();
    let zero = budget::Limits {
        owners: 0,
        events: 0,
        work: 0,
        scratch: 0,
        metadata: (std::mem::size_of::<Vec<RawEnumDecl>>()
            + std::mem::size_of::<usize>()
            + std::mem::size_of::<Vec<MatchDecl>>()),
    };
    assert!(verify_with_limits(raw(s), &sources, zero).is_ok());
    let build = || {
        let mut p = raw(s);
        p.functions[0].blocks[0].statements[0].diagnostic_origins = Some(origins(s, 2, 3));
        p
    };
    assert!(!budget::active(&build().functions[0]));
    let u = budget::preflight(&build(), budget::Limits::DEFAULT).unwrap();
    assert_eq!(
        u,
        OwnershipUsage {
            work: 2,
            metadata_bytes: (std::mem::size_of::<Vec<RawEnumDecl>>()
                + std::mem::size_of::<usize>()
                + std::mem::size_of::<Vec<MatchDecl>>())
                + 2 * std::mem::size_of::<Option<DiagnosticOrigins>>(),
            ..OwnershipUsage::default()
        }
    );
    let exact = budget::Limits {
        work: u.work,
        metadata: u.metadata_bytes,
        ..zero
    };
    assert!(verify_with_limits(build(), &sources, exact).is_ok());
    for (limits, name) in [
        (budget::Limits { work: 1, ..exact }, "ownership work"),
        (
            budget::Limits {
                metadata: u.metadata_bytes - 1,
                ..exact
            },
            "ownership metadata",
        ),
    ] {
        let e = budget::fail_allocation_after(0, || verify_with_limits(build(), &sources, limits))
            .unwrap_err();
        assert_eq!(e.kind, OwnedFailureKind::Resource(name));
    }
    let mut missing = build();
    missing.functions[0].blocks[0].terminator = None;
    assert_eq!(
        budget::preflight(&missing, budget::Limits::DEFAULT)
            .unwrap()
            .metadata_bytes,
        u.metadata_bytes
    );
}

#[test]
fn active_metadata_charges_all_embedded_fields_and_some_adds_four_checks() {
    let (sources, s) = context();
    let mut p = raw(s);
    let f = &mut p.functions[0];
    let o = owner(f, OwnerKind::Local { mutable: true }, s);
    construct(f, o, s);
    let none = budget::preflight(&p, budget::Limits::DEFAULT).unwrap();
    // O=1, B=1, I=3, V=1: N=1+2+3+1+1=8; multiplier=36.
    assert_eq!(none.work, 288);
    assert_eq!(
        none.metadata_bytes,
        (std::mem::size_of::<Vec<RawEnumDecl>>()
            + std::mem::size_of::<usize>()
            + std::mem::size_of::<Vec<MatchDecl>>())
            + std::mem::size_of::<shape::OwnerSites>()
            + 4 * std::mem::size_of::<Option<DiagnosticOrigins>>()
    );
    for i in &mut p.functions[0].blocks[0].statements {
        i.diagnostic_origins = Some(origins(s, 2, 3));
    }
    p.functions[0].blocks[0]
        .terminator
        .as_mut()
        .unwrap()
        .diagnostic_origins = Some(origins(s, 4, 5));
    let some = budget::preflight(&p, budget::Limits::DEFAULT).unwrap();
    assert_eq!(some.work, none.work + 16);
    assert_eq!(some.metadata_bytes, none.metadata_bytes);
    assert!(verify_owned(p, &sources).is_ok());
}

#[test]
fn move_failure_primary_and_cause_are_distinct_from_charge_and_none_is_compatible() {
    let (sources, s) = context();
    for supplied in [false, true] {
        let mut p = raw(s);
        let f = &mut p.functions[0];
        let o = owner(f, OwnerKind::Local { mutable: true }, at(s, 1));
        construct(f, o, at(s, 2));
        let a = owner(f, OwnerKind::Temporary, at(s, 3));
        let b = owner(f, OwnerKind::Temporary, at(s, 4));
        let mut first = ins(
            OwnedInstruction::MoveInitialize {
                destination: a,
                source: o,
            },
            at(s, 10),
        );
        let mut second = ins(
            OwnedInstruction::MoveInitialize {
                destination: b,
                source: o,
            },
            at(s, 20),
        );
        if supplied {
            first.diagnostic_origins = Some(origins(s, 100, 110));
            second.diagnostic_origins = Some(origins(s, 200, 210));
        }
        f.blocks[0].statements.extend([first, second]);
        let e = fail(p, &sources);
        assert_eq!(e.kind, OwnedFailureKind::Ownership(Violation::Unavailable));
        assert_eq!(
            e.primary.get(),
            Some(at(s, if supplied { 200 } else { 20 }))
        );
        assert_eq!(
            e.related.get(),
            Some(at(s, if supplied { 110 } else { 10 }))
        );
        assert_eq!(e.declaration.get(), Some(at(s, 1)));
    }
}

#[test]
fn valid_primary_variation_cannot_reorder_denied_operations() {
    let (sources, s) = context();
    for (first, second) in [(300, 100), (10, 400)] {
        let mut p = raw(s);
        let f = &mut p.functions[0];
        let o = owner(f, OwnerKind::Parameter { position: 0 }, at(s, 1));
        f.locals.push(LocalDecl {
            ty: hir::Ty::Bool,
            kind: LocalKind::Temporary,
            span: s,
        });
        f.blocks[0].statements.extend([
            ins(
                OwnedInstruction::Scalar(Statement::Assign(Assign {
                    destination: LocalId(1),
                    value: Rvalue::Bool(true),
                    span: s,
                })),
                s,
            ),
            ins(OwnedInstruction::Discard(o), at(s, 5)),
        ]);
        f.blocks[0].terminator = term(
            OwnedTerminatorKind::Branch {
                condition: Operand {
                    local: LocalId(1),
                    span: s,
                },
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
            s,
        );
        for (charge, primary) in [(20, first), (30, second)] {
            let mut i = ins(OwnedInstruction::Discard(o), at(s, charge));
            i.diagnostic_origins = Some(origins(s, primary, primary + 1));
            f.blocks.push(OwnedBlock {
                span: s,
                merge: None,
                statements: vec![i],
                terminator: term(OwnedTerminatorKind::ReturnScalar(operand(s)), s),
            });
        }
        let e = fail(p, &sources);
        assert_eq!(e.primary.get(), Some(at(s, first)));
        assert_eq!(e.related.get(), Some(at(s, 5)));
        let facts = e.context.unwrap().facts();
        assert_eq!(
            (facts.operation, facts.role, facts.state),
            (
                flow::DeniedOperation::Discard,
                flow::DeniedRole::SourceConsume,
                flow::ObservedState::Moved
            )
        );
    }
}

#[test]
fn cause_reconstruction_uses_only_real_reaching_paths_with_supplied_origins() {
    let (sources, s) = context();
    for terminated_arm in [false, true] {
        let mut p = raw(s);
        let f = &mut p.functions[0];
        let o = owner(f, OwnerKind::Parameter { position: 0 }, at(s, 1));
        f.locals.push(LocalDecl {
            ty: hir::Ty::Bool,
            kind: LocalKind::Temporary,
            span: s,
        });
        f.blocks[0].statements.push(ins(
            OwnedInstruction::Scalar(Statement::Assign(Assign {
                destination: LocalId(1),
                value: Rvalue::Bool(true),
                span: s,
            })),
            s,
        ));
        f.blocks[0].terminator = term(
            OwnedTerminatorKind::Branch {
                condition: Operand {
                    local: LocalId(1),
                    span: s,
                },
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
            s,
        );
        for (charge, cause) in [(80, 150), (90, 50)] {
            let mut i = ins(OwnedInstruction::Discard(o), at(s, charge));
            i.diagnostic_origins = Some(origins(s, charge + 1, cause));
            f.blocks.push(OwnedBlock {
                span: s,
                merge: None,
                statements: vec![i],
                terminator: term(
                    if terminated_arm && charge == 90 {
                        OwnedTerminatorKind::ReturnScalar(operand(s))
                    } else {
                        OwnedTerminatorKind::Goto(BlockId(3))
                    },
                    s,
                ),
            });
        }
        let mut i = ins(OwnedInstruction::Discard(o), at(s, 120));
        i.diagnostic_origins = Some(origins(s, 250, 251));
        f.blocks.push(OwnedBlock {
            span: s,
            merge: None,
            statements: vec![i],
            terminator: term(OwnedTerminatorKind::ReturnScalar(operand(s)), s),
        });
        let e = fail(p, &sources);
        assert_eq!(e.primary.get(), Some(at(s, 250)));
        assert_eq!(
            e.related.get(),
            Some(at(s, if terminated_arm { 150 } else { 50 }))
        );
    }
}

#[test]
fn shape_ticks_before_each_added_span_validation() {
    let (sources, s) = context();
    let mut p = raw(s);
    p.functions[0].blocks[0].statements[0].diagnostic_origins = Some(origins(s, 2, 3));
    let declarations = Declarations::check(&p.records, &sources).unwrap();
    for ceiling in [0, 1] {
        let mut meter = budget::Meter { visits: 0, ceiling };
        let e = shape::check(&p.functions[0], &p, &declarations, &sources, &mut meter)
            .err()
            .unwrap();
        assert_eq!(e.kind, OwnedFailureKind::Resource("ownership work ledger"));
        assert_eq!(meter.visits, ceiling + 1);
    }
    let mut meter = budget::Meter {
        visits: 0,
        ceiling: 2,
    };
    shape::check(&p.functions[0], &p, &declarations, &sources, &mut meter).unwrap();
    assert_eq!(meter.visits, 2);
}

#[test]
fn shared_counts_match_independently_inventoried_raw_without_bypassing_global_caps() {
    let (_, s) = context();
    let mut p = raw(s);
    let f = &mut p.functions[0];
    let o = owner(f, OwnerKind::Local { mutable: true }, s);
    construct(f, o, s);
    f.blocks[0].statements[1].diagnostic_origins = Some(origins(s, 2, 3));
    let counts = budget::FunctionCounts {
        locals: 1,
        owners: 1,
        blocks: 1,
        statements: 3,
        diagnostic_origins: 1,
        ownership_active: true,
        ..budget::FunctionCounts::default()
    };
    let mut program = budget::ProgramCounts::default();
    budget::account_enum_declarations(EnumUsage::default(), budget::Limits::DEFAULT, &mut program)
        .unwrap();
    budget::account_function(counts, budget::Limits::DEFAULT, &mut program).unwrap();
    assert_eq!(
        program.usage(),
        budget::preflight(&p, budget::Limits::DEFAULT).unwrap()
    );
    let mut malformed = counts;
    malformed.locals = MAX_LOCALS;
    assert_eq!(
        budget::account_function(
            malformed,
            budget::Limits::DEFAULT,
            &mut budget::ProgramCounts::default()
        )
        .unwrap_err()
        .kind,
        OwnedFailureKind::Resource("locals")
    );
    malformed = counts;
    malformed.statements = usize::MAX;
    assert!(budget::account_function(
        malformed,
        budget::Limits::DEFAULT,
        &mut budget::ProgramCounts::default()
    )
    .is_err());
    malformed = counts;
    malformed.diagnostic_origins = usize::MAX;
    assert_eq!(
        budget::account_function(
            malformed,
            budget::Limits::DEFAULT,
            &mut budget::ProgramCounts::default()
        )
        .unwrap_err()
        .kind,
        OwnedFailureKind::Resource("count overflow")
    );
}

#[test]
#[ignore = "maximum-block Some-origin work/payload/RSS qualification"]
fn maximum_block_some_origins_retain_linear_scratch_and_exact_work() {
    let (sources, s) = context();
    let mut p = raw(s);
    let f = &mut p.functions[0];
    let o = owner(f, OwnerKind::Local { mutable: true }, s);
    construct(f, o, s);
    for i in &mut f.blocks[0].statements {
        i.diagnostic_origins = Some(origins(s, 2, 3));
    }
    f.blocks[0].terminator.as_mut().unwrap().diagnostic_origins = Some(origins(s, 4, 5));
    let empty = OwnedBlock {
        span: s,
        merge: None,
        statements: vec![],
        terminator: f.blocks[0].terminator.clone(),
    };
    f.blocks.resize(MAX_BLOCKS, empty);
    for b in 0..MAX_BLOCKS - 1 {
        f.blocks[b].terminator.as_mut().unwrap().kind = OwnedTerminatorKind::Goto(BlockId(b + 1));
    }
    let usage = budget::preflight(&p, budget::Limits::DEFAULT).unwrap();
    assert_eq!(usage.work, 33_600_192);
    assert_eq!(
        usage.metadata_bytes,
        16_800_240 + std::mem::size_of::<Vec<MatchDecl>>()
    );
    assert_eq!(usage.scratch_bytes, 9_900_000);
    let w = verify_owned(p, &sources).unwrap();
    assert_eq!(w.usage().work, usage.work);
    println!("maximum Some-origin graph usage: {:?}; block bytes={}, statement bytes={}, origin field bytes={}", w.usage(), std::mem::size_of::<OwnedBlock>(), std::mem::size_of::<OwnedStatement>(), std::mem::size_of::<Option<DiagnosticOrigins>>());
}

#[test]
fn valid_provenance_changes_neither_frozen_fuel_boundaries_nor_native_output() {
    type Builder = fn() -> (SourceMap, RawOwnedProgram, consumer_fixtures::Schedule);
    for build in [
        consumer_fixtures::empty_record as Builder,
        consumer_fixtures::owned_relay,
        consumer_fixtures::shared_read,
    ] {
        let (sources, plain, schedule) = build();
        let (_, mut annotated, _) = build();
        for f in &mut annotated.functions {
            for b in &mut f.blocks {
                for i in &mut b.statements {
                    i.diagnostic_origins = Some(DiagnosticOrigins {
                        primary: f.span,
                        cause: b.span,
                    });
                }
                if let Some(e) = &mut b.terminator {
                    e.diagnostic_origins = Some(DiagnosticOrigins {
                        primary: f.span,
                        cause: b.span,
                    });
                }
            }
        }
        let plain = verify_owned(plain, &sources).unwrap();
        let annotated = verify_owned(annotated, &sources).unwrap();
        for fuel in 0..=schedule.fuel() {
            let limits = execute::Limits {
                fuel,
                ..execute::Limits::default()
            };
            let before = execute::run_limits(&plain, Some(schedule.entry), limits);
            let after = execute::run_limits(&annotated, Some(schedule.entry), limits);
            assert_eq!(before, after, "fuel {fuel}");
            if let Some(span) = schedule.failure(fuel) {
                assert_eq!(
                    after,
                    Err(execute::OwnedRunFailure::Scalar(RunFailure::Fuel(span)))
                );
            } else {
                assert_eq!(after.unwrap(), schedule.result);
            }
        }
        assert_eq!(
            native::native_module(&plain, Some(schedule.entry), &sources).unwrap(),
            native::native_module(&annotated, Some(schedule.entry), &sources).unwrap()
        );
    }
}

#[test]
fn moved_backedge_reconstructs_the_previous_iteration_cause() {
    let (sources, s) = context();
    let mut p = raw(s);
    let f = &mut p.functions[0];
    let o = owner(f, OwnerKind::Parameter { position: 0 }, at(s, 1));
    f.locals.push(LocalDecl {
        ty: hir::Ty::Bool,
        kind: LocalKind::Temporary,
        span: s,
    });
    f.blocks[0].statements.push(ins(
        OwnedInstruction::Scalar(Statement::Assign(Assign {
            destination: LocalId(1),
            value: Rvalue::Bool(true),
            span: s,
        })),
        s,
    ));
    f.blocks[0].terminator = term(
        OwnedTerminatorKind::Branch {
            condition: Operand {
                local: LocalId(1),
                span: s,
            },
            then_block: BlockId(1),
            else_block: BlockId(2),
        },
        s,
    );
    let mut moved = ins(OwnedInstruction::Discard(o), at(s, 10));
    moved.diagnostic_origins = Some(origins(s, 200, 250));
    f.blocks.push(OwnedBlock {
        span: s,
        merge: None,
        statements: vec![moved],
        terminator: term(OwnedTerminatorKind::Goto(BlockId(0)), s),
    });
    f.blocks.push(OwnedBlock {
        span: s,
        merge: None,
        statements: vec![],
        terminator: term(OwnedTerminatorKind::ReturnScalar(operand(s)), s),
    });
    let e = fail(p, &sources);
    assert_eq!(e.kind, OwnedFailureKind::Ownership(Violation::Unavailable));
    assert_eq!(e.primary.get(), Some(at(s, 200)));
    assert_eq!(e.related.get(), Some(at(s, 250)));
}

#[test]
fn diagnostic_context_is_fixed_stack_payload_without_new_analysis_allocations() {
    assert_eq!(std::mem::size_of::<OwnedFailure>(), 240);
    assert_eq!(std::mem::size_of::<flow::DenialContext>(), 136);
    assert_eq!(std::mem::size_of::<flow::DenialFacts>(), 136);
    println!(
        "fixed diagnostic payload: OwnedFailure={} DenialContext={} DenialFacts={}",
        std::mem::size_of::<OwnedFailure>(),
        std::mem::size_of::<flow::DenialContext>(),
        std::mem::size_of::<flow::DenialFacts>()
    );
}
