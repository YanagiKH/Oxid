//! Present verifier denials without granting source metadata semantic authority.
use super::super::{flow::*, *};
use crate::frontend::owned_diagnostic;

pub(super) fn lower(error: &OwnedFailure, sources: &SourceMap) -> Box<Diagnostic> {
    present(error, sources, "oir-owned-lower", None)
}

pub(super) fn verify(error: &OwnedFailure, sources: &SourceMap) -> Box<Diagnostic> {
    let source_code = error
        .context
        .as_ref()
        .and_then(|context| classify(error.kind, context.facts()));
    present(error, sources, "oir-owned-verify", source_code)
}

fn named(owner: OwnerSubject) -> bool {
    matches!(
        owner.class,
        OwnerKind::Local { .. } | OwnerKind::Parameter { .. }
    )
}

fn mutable(owner: OwnerSubject) -> bool {
    matches!(owner.class, OwnerKind::Local { mutable: true })
}

fn named_subject(subject: DeniedSubject) -> bool {
    match subject {
        DeniedSubject::Owner(owner) => named(owner),
        DeniedSubject::Reference { .. } => true,
    }
}

fn writable(subject: DeniedSubject) -> bool {
    match subject {
        DeniedSubject::Owner(owner) => mutable(owner),
        DeniedSubject::Reference { granted, .. } => granted == BorrowKind::Exclusive,
    }
}

/// This deliberately lists complete source-expressible operation/role pairs.
/// Cleanup, staging and other compiler-created accesses never become user errors.
fn classify(kind: OwnedFailureKind, facts: DenialFacts) -> Option<&'static str> {
    use DeniedOperation as Op;
    use DeniedRole as Role;
    let named_move = facts.operation == Op::MoveInitialize
        && facts.role == Role::SourceConsume
        && matches!((facts.subject, facts.counterpart),
            (DeniedSubject::Owner(source), Some(destination))
                if named(source) && destination.class == OwnerKind::Temporary
                    && source.aggregate() == destination.aggregate() && source.id != destination.id)
        && facts.requested_borrow.is_none();
    let field = matches!(facts.operation, Op::ReadField | Op::WriteField)
        && facts.role == Role::FieldBase
        && facts.counterpart.is_none()
        && facts.requested_borrow.is_none();
    let array = matches!(
        facts.operation,
        Op::ReadIndex | Op::WriteIndex | Op::ArrayLength
    ) && facts.role == Role::ArrayBase
        && facts.counterpart.is_none()
        && facts.requested_borrow.is_none()
        && matches!(facts.subject.aggregate(), AggregateTy::FixedArray(_))
        && named_subject(facts.subject);
    let borrow = facts.operation == Op::PrepareBorrow
        && facts.role == Role::BorrowAuthority
        && facts.counterpart.is_none()
        && facts.requested_borrow.is_some();
    match kind {
        OwnedFailureKind::Ownership(Violation::Unavailable)
            if facts.state == ObservedState::Moved
                && (named_move
                    || ((field || array || borrow)
                        && matches!(facts.subject, DeniedSubject::Owner(owner) if named(owner)))) =>
        {
            Some("E0310")
        }
        OwnedFailureKind::Ownership(Violation::LoanConflict)
            if facts.state == ObservedState::NotObserved
                && (named_move
                    || ((field || array)
                        && named_subject(facts.subject)
                        && (matches!(
                            facts.operation,
                            Op::ReadField | Op::ReadIndex | Op::ArrayLength
                        ) || writable(facts.subject)))
                    || (facts.operation == Op::Replace
                        && facts.role == Role::ReplacementDestination
                        && facts.requested_borrow.is_none()
                        && matches!((facts.subject, facts.counterpart),
                            (DeniedSubject::Owner(destination), Some(source))
                                if mutable(destination) && source.aggregate() == destination.aggregate()
                                    && source.id != destination.id))
                    || (borrow
                        && named_subject(facts.subject)
                        && (facts.requested_borrow == Some(BorrowKind::Shared)
                            || writable(facts.subject)))) =>
        {
            Some("E0311")
        }
        OwnedFailureKind::Ownership(Violation::Permission)
            if facts.state == ObservedState::NotObserved
                && matches!(
                    facts.subject,
                    DeniedSubject::Reference {
                        granted: BorrowKind::Shared,
                        ..
                    }
                )
                && ((field && facts.operation == Op::WriteField)
                    || (array && facts.operation == Op::WriteIndex)
                    || (borrow && facts.requested_borrow == Some(BorrowKind::Exclusive))) =>
        {
            Some("E0313")
        }
        _ => None,
    }
}

fn subject_declaration(facts: DenialFacts) -> Span {
    match facts.subject {
        DeniedSubject::Owner(owner) => owner.declaration,
        DeniedSubject::Reference { declaration, .. } => declaration,
    }
}

fn label(
    error: Box<Diagnostic>,
    sources: &SourceMap,
    span: Option<Span>,
    message: &'static str,
) -> Box<Diagnostic> {
    match span.filter(|span| sources.is_valid_span(*span)) {
        Some(span)
            if !error
                .secondary
                .iter()
                .any(|(existing, text)| *existing == span && text == message) =>
        {
            owned_diagnostic::secondary(error, span, format_args!("{message}"))
        }
        _ => error,
    }
}

fn present(
    failure: &OwnedFailure,
    sources: &SourceMap,
    internal_stage: &'static str,
    source_code: Option<&'static str>,
) -> Box<Diagnostic> {
    let primary = failure
        .primary
        .get()
        .filter(|span| sources.is_valid_span(*span));
    let (code, stage, message) = match failure.kind {
        OwnedFailureKind::Resource(_) => ("E0400", internal_stage, "owned resource limit exceeded"),
        _ => match source_code {
            Some("E0310") => (
                "E0310",
                "ownership",
                "owned value is not available on every path",
            ),
            Some("E0311") => (
                "E0311",
                "ownership",
                "access conflicts with an active borrow",
            ),
            Some("E0313") => (
                "E0313",
                "ownership",
                "shared reference does not permit exclusive access",
            ),
            _ => (
                "E0500",
                internal_stage,
                "internal compiler error: owned invariant violation",
            ),
        },
    };
    let mut error = owned_diagnostic::diagnostic(code, stage, format_args!("{message}"), primary);
    if let OwnedFailureKind::Resource(name) = failure.kind {
        return owned_diagnostic::note(
            error,
            format_args!("resource: {}", owned_diagnostic::name(name)),
        );
    }
    let cause_message = match source_code {
        Some("E0310") => "value moved here",
        Some("E0311") => "borrow acquired here",
        _ => "related ownership operation",
    };
    error = label(error, sources, failure.related.get(), cause_message);
    let declaration = failure
        .context
        .as_ref()
        .map(|context| subject_declaration(context.facts()))
        .or_else(|| failure.declaration.get());
    label(error, sources, declaration, "value declared here")
}
