use super::super::{consumer_fixtures as fixture, verified::verify_owned, *};
use super::{diagnostic, lower, resolve, typeck};
use crate::frontend::{lexer, parser, source::SourceFileId};

fn raw(text: &str) -> (SourceMap, RawOwnedProgram) {
    let mut sources = SourceMap::new();
    let file = sources.add("owned-diagnostic.ox".into(), text.into());
    let source = sources.get(file);
    let ast = parser::parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::OwnedCandidate,
    )
    .unwrap();
    let typed = typeck::check(resolve::resolve(source, &ast).unwrap()).unwrap();
    let raw = lower::lower(&typed).unwrap();
    (sources, raw)
}

fn denied(text: &str) -> (SourceMap, OwnedFailure, Box<Diagnostic>) {
    let (sources, raw) = raw(text);
    let failure = verify_owned(raw, &sources).unwrap_err();
    let error = diagnostic::verify(&failure, &sources);
    (sources, failure, error)
}

fn text_at(sources: &SourceMap, span: Span) -> &str {
    assert!(sources.is_valid_span(span));
    &sources.get(span.file).text()[span.start..span.end]
}

#[test]
fn source_unavailable_uses_named_access_and_realizable_statement_cause() {
    for (text, primary, cause) in [
        ("struct C {} fn main() -> () { let x = C {}; let y = x; x; return; }", "x", "let y = x;"),
        ("struct C {} fn f(x: C) -> () { let y = x; x; return; }", "x", "let y = x;"),
        ("struct C {} fn f(x: C) -> () { let y = (x); ((x)); return; }", "x", "let y = (x);"),
        ("struct C { n: i32 } fn main() -> () { let x = C { n: 1 }; let y = x; x.n; return; }", "x.n", "let y = x;"),
        ("struct C { n: i32 } fn main() -> () { let mut x = C { n: 1 }; let y = x; x.n = 2; return; }", "x.n", "let y = x;"),
        ("struct C {} fn take(p: &C) -> () { return; } fn main() -> () { let x = C {}; let y = x; take(&x); return; }", "&x", "let y = x;"),
        ("struct C {} fn f(x: C, flag: bool) -> () { if flag { x; } x; return; }", "x", "x;"),
    ] {
        let (sources, failure, error) = denied(text);
        assert_eq!(failure.kind, OwnedFailureKind::Ownership(Violation::Unavailable), "{text}");
        assert_eq!((error.code, error.stage), ("E0310", "ownership"), "{text}");
        assert!(error.message.contains("not available on every path"));
        assert_eq!(text_at(&sources, error.primary.unwrap()), primary);
        assert_eq!(error.secondary.len(), 2);
        assert_eq!(text_at(&sources, error.secondary[0].0), cause);
        assert_eq!(error.secondary[0].1, "value moved here");
        assert_eq!(text_at(&sources, error.secondary[1].0), "x");
    }
}

#[test]
fn source_borrow_conflicts_keep_borrow_cause_before_declaration() {
    for (text, primary, cause) in [
        ("struct C {} fn take(a: &C, b: C) -> () { return; } fn main() -> () { let x = C {}; take(&x, x); return; }", "x", "&x"),
        ("struct C {} fn take(a: &C, b: C) -> () { return; } fn f(x: C) -> () { take(&x, x); return; }", "x", "&x"),
        ("struct C { n: i32 } fn take(a: &mut C, b: i32) -> () { return; } fn main() -> () { let mut x = C { n: 1 }; take(&mut x, x.n); return; }", "x.n", "&mut x"),
        ("struct C {} fn take(a: &C, b: &mut C) -> () { return; } fn main() -> () { let mut x = C {}; take(&x, &mut x); return; }", "&mut x", "&x"),
        ("struct C { n: i32 } fn take(a: &mut C, b: i32) -> () { return; } fn f(p: &mut C) -> () { take(&mut *p, p.n); return; }", "p.n", "&mut *p"),
    ] {
        let (sources, failure, error) = denied(text);
        assert_eq!(failure.kind, OwnedFailureKind::Ownership(Violation::LoanConflict), "{text}");
        assert_eq!((error.code, error.stage), ("E0311", "ownership"), "{text}");
        assert_eq!(text_at(&sources, error.primary.unwrap()), primary);
        assert_eq!(error.secondary.len(), 2);
        assert_eq!(text_at(&sources, error.secondary[0].0), cause);
        assert_eq!(error.secondary[0].1, "borrow acquired here");
        assert_eq!(error.secondary[1].1, "value declared here");
    }
}

#[test]
fn source_shared_reference_permission_is_an_ownership_error() {
    for (text, primary) in [
        ("struct C { n: i32 } fn f(p: &C) -> () { p.n = 2; return; }", "p.n"),
        ("struct C {} fn take(p: &mut C) -> () { return; } fn f(p: &C) -> () { take(&mut *p); return; }", "&mut *p"),
    ] {
        let (sources, failure, error) = denied(text);
        assert_eq!(failure.kind, OwnedFailureKind::Ownership(Violation::Permission));
        assert_eq!((error.code, error.stage), ("E0313", "ownership"));
        assert_eq!(text_at(&sources, error.primary.unwrap()), primary);
        assert_eq!(error.secondary.len(), 1);
        assert_eq!(text_at(&sources, error.secondary[0].0), "p");
    }
}

#[test]
fn source_internal_discard_and_storage_failures_do_not_become_source_errors() {
    for cleanup in [false, true] {
        let (sources, mut raw, _) = fixture::empty_record();
        let body = &mut raw.functions[0].blocks[0].statements;
        let at = body
            .iter()
            .position(|statement| matches!(statement.kind, OwnedInstruction::StorageEnd(_)))
            .unwrap();
        let span = body[at].span;
        body.insert(
            at,
            fixture::instruction(
                if cleanup {
                    OwnedInstruction::StorageEnd(OwnerPlaceId(0))
                } else {
                    OwnedInstruction::Discard(OwnerPlaceId(0))
                },
                span,
            ),
        );
        let failure = verify_owned(raw, &sources).unwrap_err();
        assert!(matches!(failure.kind, OwnedFailureKind::Ownership(_)));
        let error = diagnostic::verify(&failure, &sources);
        assert_eq!((error.code, error.stage), ("E0500", "oir-owned-verify"));
    }
}

#[test]
fn source_adapter_accepts_only_named_to_temporary_moved_normalization() {
    for (source_kind, destination_kind, expected) in [
        (
            OwnerKind::Local { mutable: false },
            OwnerKind::Temporary,
            "E0310",
        ),
        (OwnerKind::Temporary, OwnerKind::Temporary, "E0500"),
        (
            OwnerKind::Local { mutable: false },
            OwnerKind::Local { mutable: false },
            "E0500",
        ),
    ] {
        let (sources, mut raw, _) = fixture::empty_record();
        let function = &mut raw.functions[0];
        function.owners[0].kind = source_kind;
        let span = function.owners[0].span;
        function.owners.push(fixture::owner(destination_kind, span));
        let statements = &mut function.blocks[0].statements;
        let at = statements
            .iter()
            .position(|statement| matches!(statement.kind, OwnedInstruction::StorageEnd(_)))
            .unwrap();
        statements.splice(
            at..at,
            [
                fixture::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), span),
                fixture::instruction(
                    OwnedInstruction::MoveInitialize {
                        destination: OwnerPlaceId(1),
                        source: OwnerPlaceId(0),
                    },
                    span,
                ),
                fixture::instruction(OwnedInstruction::StorageEnd(OwnerPlaceId(1)), span),
            ],
        );
        let failure = verify_owned(raw, &sources).unwrap_err();
        assert_eq!(
            failure.kind,
            OwnedFailureKind::Ownership(Violation::Unavailable)
        );
        let facts = failure.context.unwrap().facts();
        assert_eq!(facts.operation, flow::DeniedOperation::MoveInitialize);
        assert_eq!(facts.role, flow::DeniedRole::SourceConsume);
        assert_eq!(facts.state, flow::ObservedState::Moved);
        assert_eq!(diagnostic::verify(&failure, &sources).code, expected);
    }
}

#[test]
fn source_unavailable_compiler_transfers_remain_internal_errors() {
    for (text, operation) in [
        (
            "struct C {} fn main() -> () { let x = C {}; let y = x; return; }",
            flow::DeniedOperation::MoveInitialize,
        ),
        (
            "struct C {} fn main() -> () { let mut x = C {}; x = C {}; return; }",
            flow::DeniedOperation::Replace,
        ),
        (
            "struct C {} fn take(x: C) -> () { return; } fn main() -> () { take(C {}); return; }",
            flow::DeniedOperation::PrepareOwned,
        ),
        (
            "struct C {} fn main() -> C { return C {}; }",
            flow::DeniedOperation::ReturnOwned,
        ),
    ] {
        let (sources, mut raw) = raw(text);
        let mut changed = false;
        'functions: for function in &mut raw.functions {
            for block in &mut function.blocks {
                for index in 0..block.statements.len() {
                    let source = match block.statements[index].kind {
                        OwnedInstruction::MoveInitialize {
                            source,
                            destination,
                        } if operation == flow::DeniedOperation::MoveInitialize
                            && matches!(function.owners[source.0].kind, OwnerKind::Temporary)
                            && matches!(
                                function.owners[destination.0].kind,
                                OwnerKind::Local { .. }
                            ) =>
                        {
                            Some(source)
                        }
                        OwnedInstruction::Replace { source, .. }
                            if operation == flow::DeniedOperation::Replace =>
                        {
                            Some(source)
                        }
                        OwnedInstruction::PrepareOwned { source, .. }
                            if operation == flow::DeniedOperation::PrepareOwned =>
                        {
                            Some(source)
                        }
                        _ => None,
                    };
                    if let Some(source) = source {
                        let span = block.statements[index].span;
                        block.statements.insert(
                            index,
                            fixture::instruction(OwnedInstruction::Discard(source), span),
                        );
                        changed = true;
                        break 'functions;
                    }
                }
                if operation == flow::DeniedOperation::ReturnOwned {
                    let terminator = block.terminator.as_ref().unwrap();
                    if let OwnedTerminatorKind::ReturnOwned(source) = terminator.kind {
                        block.statements.push(fixture::instruction(
                            OwnedInstruction::Discard(source),
                            terminator.span,
                        ));
                        changed = true;
                        break 'functions;
                    }
                }
            }
        }
        assert!(changed);
        let failure = verify_owned(raw, &sources).unwrap_err();
        assert_eq!(
            failure.kind,
            OwnedFailureKind::Ownership(Violation::Unavailable),
            "{operation:?}"
        );
        let facts = failure.context.unwrap().facts();
        assert_eq!(facts.operation, operation);
        assert_eq!(facts.state, flow::ObservedState::Moved);
        let error = diagnostic::verify(&failure, &sources);
        assert_eq!((error.code, error.stage), ("E0500", "oir-owned-verify"));
    }
}

#[test]
fn source_cleanup_during_a_loan_and_mutability_lies_remain_internal() {
    let text = "struct C {} fn take(p: &C) -> () { return; } fn main() -> () { let x = C {}; take(&x); return; }";
    let (sources, mut raw) = raw(text);
    let function = &mut raw.functions[1];
    let owner = match function.loans[0].authority {
        AccessBase::Owner(owner) => owner,
        _ => panic!("source fixture requires a named owner"),
    };
    for block in &mut function.blocks {
        block.statements.retain(|statement| !matches!(statement.kind, OwnedInstruction::StorageEnd(found) if found == owner));
    }
    let first = &mut function.blocks[0];
    first.statements.push(fixture::instruction(
        OwnedInstruction::StorageEnd(owner),
        first.span,
    ));
    let failure = verify_owned(raw, &sources).unwrap_err();
    assert_eq!(
        failure.kind,
        OwnedFailureKind::Ownership(Violation::LoanConflict)
    );
    let facts = failure.context.unwrap().facts();
    assert_eq!(facts.operation, flow::DeniedOperation::StorageEnd);
    assert_eq!(facts.role, flow::DeniedRole::Storage);
    assert_eq!(diagnostic::verify(&failure, &sources).code, "E0500");

    let (sources, mut raw) = self::raw(
        "struct C { n: i32 } fn main() -> () { let mut x = C { n: 1 }; x.n = 2; return; }",
    );
    let function = &mut raw.functions[0];
    let owner = function
        .owners
        .iter_mut()
        .find(|owner| matches!(owner.kind, OwnerKind::Local { .. }))
        .unwrap();
    owner.kind = OwnerKind::Local { mutable: false };
    let failure = verify_owned(raw, &sources).unwrap_err();
    assert_eq!(
        failure.kind,
        OwnedFailureKind::Ownership(Violation::Permission)
    );
    assert_eq!(diagnostic::verify(&failure, &sources).code, "E0500");
}

#[test]
fn source_adapter_classifies_verified_replace_destination_role() {
    // Add an otherwise well-shaped replacement during a source call's live loan.
    // The verifier, rather than source syntax or producer tags, supplies the role.
    let (sources, mut raw) = raw("struct C {} fn take(p: &C) -> () { return; } fn main() -> () { let mut x = C {}; take(&x); return; }");
    let function = &mut raw.functions[1];
    let destination = match function.loans[0].authority {
        AccessBase::Owner(owner) => owner,
        _ => panic!("expected an owner loan"),
    };
    let source = OwnerPlaceId(function.owners.len());
    let span = function.owners[destination.0].span;
    function
        .owners
        .push(fixture::owner(OwnerKind::Temporary, span));
    function.blocks[0].statements.extend([
        fixture::instruction(OwnedInstruction::StorageLive(source), span),
        fixture::instruction(
            OwnedInstruction::Construct {
                destination: source,
                fields: vec![],
            },
            span,
        ),
        fixture::instruction(
            OwnedInstruction::Replace {
                destination,
                source,
            },
            span,
        ),
        fixture::instruction(OwnedInstruction::StorageEnd(source), span),
    ]);
    let failure = verify_owned(raw, &sources).unwrap_err();
    assert_eq!(
        failure.kind,
        OwnedFailureKind::Ownership(Violation::LoanConflict)
    );
    let facts = failure.context.unwrap().facts();
    assert_eq!(facts.operation, flow::DeniedOperation::Replace);
    assert_eq!(facts.role, flow::DeniedRole::ReplacementDestination);
    assert_eq!(facts.counterpart.unwrap().id, source);
    let error = diagnostic::verify(&failure, &sources);
    assert_eq!((error.code, error.stage), ("E0311", "ownership"));
}

#[test]
fn source_missing_context_and_resource_stages_are_conservative() {
    let (sources, _, _) = fixture::empty_record();
    let span = sources.get(SourceFileId(0)).span(0, 1);
    for violation in [
        Violation::Unavailable,
        Violation::Permission,
        Violation::LoanConflict,
        Violation::Lifetime,
        Violation::Initialization,
        Violation::CallRegion,
        Violation::LoanRegion,
        Violation::ActiveAtExit,
    ] {
        let failure = OwnedFailure::violation(violation, span, Some(span));
        let error = diagnostic::verify(&failure, &sources);
        assert_eq!((error.code, error.stage), ("E0500", "oir-owned-verify"));
    }
    let failure = OwnedFailure::resource("raw output bytes");
    let lower = diagnostic::lower(&failure, &sources);
    let verify = diagnostic::verify(&failure, &sources);
    assert_eq!((lower.code, lower.stage), ("E0400", "oir-owned-lower"));
    assert_eq!((verify.code, verify.stage), ("E0400", "oir-owned-verify"));
}

#[test]
fn source_origins_change_labels_without_reclassifying_the_denied_access() {
    let text = "struct C {} fn main() -> () { let x = C {}; let y = x; x; return; }";
    let (sources, original_raw) = raw(text);
    let original = verify_owned(original_raw, &sources).unwrap_err();
    let (_, mut raw) = raw(text);
    let replacement = sources.get(SourceFileId(0)).span(0, 0);
    for function in &mut raw.functions {
        for block in &mut function.blocks {
            for statement in &mut block.statements {
                statement.diagnostic_origins = Some(DiagnosticOrigins {
                    primary: replacement,
                    cause: replacement,
                });
            }
            block.terminator.as_mut().unwrap().diagnostic_origins = Some(DiagnosticOrigins {
                primary: replacement,
                cause: replacement,
            });
        }
    }
    let changed = verify_owned(raw, &sources).unwrap_err();
    assert_eq!(original.kind, changed.kind);
    assert_eq!(original.context, changed.context);
    let before = diagnostic::verify(&original, &sources);
    let after = diagnostic::verify(&changed, &sources);
    assert_eq!((before.code, before.stage), (after.code, after.stage));
    assert_eq!(after.primary, Some(replacement));
    assert_eq!(after.secondary[0].0, replacement);
}

#[test]
fn source_rendering_revalidates_every_failure_origin_without_changing_code() {
    let text = "struct C {} fn main() -> () { let x = C {}; let y = x; x; return; }";
    let (sources, mut failure, before) = denied(text);
    let invalid = Span {
        file: SourceFileId(usize::MAX - 1),
        start: 0,
        end: 1,
    };
    failure.primary = Origin::from(Some(invalid));
    failure.related = Origin::from(Some(invalid));
    failure.declaration = Origin::from(Some(invalid));
    let after = diagnostic::verify(&failure, &sources);
    assert_eq!((before.code, before.stage), (after.code, after.stage));
    assert_eq!(after.primary, None);
    // The immutable verifier-derived declaration is still valid and preferred.
    assert_eq!(after.secondary.len(), 1);
    assert_eq!(after.secondary[0].1, "value declared here");
    after.render_json(&sources);
    after.render_human(&sources);
    let unrelated_sources = SourceMap::new();
    let absent = diagnostic::verify(&failure, &unrelated_sources);
    assert_eq!((before.code, before.stage), (absent.code, absent.stage));
    assert_eq!(absent.primary, None);
    assert!(absent.secondary.is_empty());
    absent.render_json(&unrelated_sources);
    absent.render_human(&unrelated_sources);
}

#[test]
fn source_diagnostics_retain_bounded_text_for_long_names() {
    let name = "x".repeat(20_000);
    let text = format!(
        "struct C {{}} fn main() -> () {{ let {name} = C {{}}; let y = {name}; {name}; return; }}"
    );
    let (_, _, error) = denied(&text);
    assert_eq!(error.code, "E0310");
    assert!(error.message.len() <= 1024);
    assert!(error.secondary.len() <= 2);
    assert!(error.secondary.iter().all(|(_, text)| text.len() <= 256));
    assert!(error.notes.len() <= 2);
    assert!(error.notes.iter().all(|text| text.len() <= 256));
    assert!(
        error.message.len()
            + error
                .secondary
                .iter()
                .map(|(_, text)| text.len())
                .sum::<usize>()
            + error.notes.iter().map(String::len).sum::<usize>()
            <= 2048
    );
}
