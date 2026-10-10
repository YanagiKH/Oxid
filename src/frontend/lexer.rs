use super::diagnostic::Diagnostic;
use super::project::budget::{Allocator, ReserveFailure};
use super::source::{SourceFile, Span};

pub const MAX_TOKENS: usize = 100_000;
pub const MAX_TOKEN_BYTES: usize = 65_536;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Trivia,
    Ident,
    Number,
    String,
    Fn,
    Struct,
    Mod,
    Use,
    Pub,
    Ampersand,
    Dot,
    Let,
    Mut,
    Return,
    Break,
    Continue,
    If,
    While,
    Else,
    True,
    False,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Colon,
    Comma,
    Semi,
    Equal,
    EqualEqual,
    NotEqual,
    Not,
    AndAnd,
    OrOr,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Arrow,
    Minus,
    Plus,
    Star,
    Slash,
    Percent,
    Unsupported,
    Invalid,
    Eof,
}
#[derive(Clone, Copy, Debug)]
pub struct Token {
    pub kind: Kind,
    pub span: Span,
}

/// Retains every byte, including trivia and unsupported literal spelling.
#[cfg(test)]
pub fn lex(source: &SourceFile) -> Result<Vec<Token>, Box<Diagnostic>> {
    lex_with_limit(source, MAX_TOKENS)
}

/// Compatibility entry for dependency-light, immutable observer controllers.
/// It delegates to the same fallible core as every accounted production route.
#[allow(dead_code)]
pub(super) fn lex_with_limit(
    source: &SourceFile,
    limit: usize,
) -> Result<Vec<Token>, Box<Diagnostic>> {
    lex_with_allocator(source, limit, &mut Allocator::default()).map_err(Failure::diagnostic)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FailureReason {
    UnterminatedBlockComment,
    UnterminatedString,
    TokenLimit,
    StorageAllocation,
    StorageLimit,
}

/// Allocation-free core failure. Legacy diagnostic adaptation is best-effort;
/// String, Box, diagnostic Vec and rendering allocations remain outside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Failure {
    reason: FailureReason,
    span: Span,
}

impl Failure {
    pub(super) fn is_storage(self) -> bool {
        matches!(
            self.reason,
            FailureReason::StorageAllocation | FailureReason::StorageLimit
        )
    }

    pub(super) fn diagnostic(self) -> Box<Diagnostic> {
        let (code, message) = match self.reason {
            FailureReason::UnterminatedBlockComment => ("E0100", "unterminated block comment"),
            FailureReason::UnterminatedString => ("E0100", "unterminated string literal"),
            FailureReason::TokenLimit => ("E0400", "token resource limit exceeded"),
            FailureReason::StorageAllocation => ("E0400", "token storage allocation failed"),
            FailureReason::StorageLimit => ("E0400", "token storage resource limit exceeded"),
        };
        Diagnostic::new(code, "lex", message, Some(self.span))
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GrowthEvent {
    previous_requested: usize,
    actual_old_capacity: usize,
    new_requested: usize,
    actual_returned_capacity: usize,
    success: bool,
}

/// Fixed, stack-only qualification storage. No shared allocator trace changes.
#[derive(Default)]
struct ReservationObserver {
    /// Qualification-only lowering of requested payload admission.
    #[cfg(test)]
    requested_byte_limit: Option<usize>,
    #[cfg(test)]
    events: [Option<GrowthEvent>; 16],
    #[cfg(test)]
    length: usize,
    #[cfg(test)]
    overflow: bool,
}

impl ReservationObserver {
    fn record(
        &mut self,
        previous: usize,
        old: usize,
        requested: usize,
        returned: usize,
        success: bool,
    ) {
        #[cfg(test)]
        if let Some(slot) = self.events.get_mut(self.length) {
            *slot = Some(GrowthEvent {
                previous_requested: previous,
                actual_old_capacity: old,
                new_requested: requested,
                actual_returned_capacity: returned,
                success,
            });
            self.length += 1;
        } else {
            self.overflow = true;
        }
        #[cfg(not(test))]
        let _ = (previous, old, requested, returned, success);
    }
}

/// Named conservative helper/core/adapter carriers, not compiler stack or RSS.
/// Keep all earlier backing/diagnostic charges; this allowance is additive.
pub(super) fn reservation_scratch_bytes() -> usize {
    use std::mem::size_of;
    3 * size_of::<Failure>()
        + 3 * size_of::<Result<Vec<Token>, Failure>>()
        + 3 * size_of::<Token>()
        + 3 * size_of::<Vec<Token>>()
        + 32 * size_of::<usize>()
        + 8 * size_of::<&str>()
        + size_of::<Allocator>()
}

#[cfg(test)]
#[allow(dead_code)]
struct ReservationObserverCarriers<'a> {
    sink: ReservationObserver,
    active: &'a mut ReservationObserver,
    previous_requested: usize,
    actual_old_capacity: usize,
    new_requested: usize,
    actual_returned_capacity: usize,
    success: bool,
    checked_slot: Option<&'a mut Option<GrowthEvent>>,
    pending: GrowthEvent,
}

#[cfg(test)]
pub(super) fn reservation_observer_bytes() -> usize {
    // Named sink/local/checked-slot carriers, conservatively simultaneous.
    // The same sink is borrowed throughout: no whole-array return copy exists.
    std::mem::size_of::<ReservationObserverCarriers<'_>>()
}

fn request_ceiling(bytes: usize, limit: usize) -> Option<usize> {
    bytes
        .min(limit.min(MAX_TOKENS))
        .checked_add(1)?
        .max(4)
        .checked_next_power_of_two()
}

fn next_target(capacity: usize, ceiling: usize) -> Option<usize> {
    let mut target = 4usize;
    while target <= capacity {
        target = target.checked_mul(2)?;
    }
    (target <= ceiling).then_some(target)
}

fn append(
    tokens: &mut Vec<Token>,
    token: Token,
    ceiling: Option<usize>,
    previous_requested: &mut usize,
    allocator: &mut Allocator,
    observer: &mut ReservationObserver,
) -> Result<(), Failure> {
    let resource = Failure {
        reason: FailureReason::StorageLimit,
        span: token.span,
    };
    if tokens.len() == tokens.capacity() {
        let old = tokens.capacity();
        let target = next_target(old, ceiling.ok_or(resource)?).ok_or(resource)?;
        let additional = target.checked_sub(tokens.len()).ok_or(resource)?;
        #[cfg(test)]
        if observer.requested_byte_limit.is_some_and(|limit| {
            target
                .checked_mul(std::mem::size_of::<Token>())
                .is_none_or(|bytes| bytes > limit)
        }) {
            return Err(resource);
        }
        let result = allocator.vector_exact(tokens, additional, "lexer token tape");
        // The reserve guard is already released: instrumentation never allocates
        // or writes while a selected global-null operation is armed.
        observer.record(
            *previous_requested,
            old,
            target,
            tokens.capacity(),
            result.is_ok(),
        );
        *previous_requested = target;
        result.map_err(|error| Failure {
            reason: match error {
                ReserveFailure::Overflow => FailureReason::StorageLimit,
                ReserveFailure::Allocation => FailureReason::StorageAllocation,
            },
            span: token.span,
        })?;
    }
    // No ordinary over-return refusal: spare returned capacity remains usable.
    if tokens.len() >= tokens.capacity() {
        return Err(resource);
    }
    tokens.push(token);
    Ok(())
}

pub(super) fn lex_with_allocator(
    source: &SourceFile,
    limit: usize,
    allocator: &mut Allocator,
) -> Result<Vec<Token>, Failure> {
    lex_core(
        source,
        limit,
        allocator,
        &mut ReservationObserver::default(),
    )
}

fn lex_core(
    source: &SourceFile,
    limit: usize,
    allocator: &mut Allocator,
    observer: &mut ReservationObserver,
) -> Result<Vec<Token>, Failure> {
    let limit = limit.min(MAX_TOKENS);
    let text = source.text();
    let bytes = text.as_bytes();
    let ceiling = request_ceiling(bytes.len(), limit);
    let mut tokens = Vec::new();
    let mut previous_requested = 0;
    let mut cursor = 0;
    while cursor < bytes.len() {
        let start = cursor;
        let ch = text[cursor..].chars().next().unwrap();
        cursor += ch.len_utf8();
        let kind = match ch {
            c if c.is_whitespace() => {
                while cursor < bytes.len() {
                    let next = text[cursor..].chars().next().unwrap();
                    if !next.is_whitespace() {
                        break;
                    }
                    cursor += next.len_utf8();
                }
                Kind::Trivia
            }
            '/' if bytes.get(cursor) == Some(&b'/') => {
                while cursor < bytes.len() && bytes[cursor] != b'\n' {
                    cursor += 1;
                }
                Kind::Trivia
            }
            '/' if bytes.get(cursor) == Some(&b'*') => {
                cursor += 1;
                let mut closed = false;
                while cursor + 1 < bytes.len() {
                    if &bytes[cursor..cursor + 2] == b"*/" {
                        cursor += 2;
                        closed = true;
                        break;
                    }
                    cursor += 1;
                }
                if !closed {
                    return Err(Failure {
                        reason: FailureReason::UnterminatedBlockComment,
                        span: source.span(start, text.len()),
                    });
                }
                Kind::Trivia
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                while cursor < bytes.len()
                    && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'_')
                {
                    cursor += 1;
                }
                match &text[start..cursor] {
                    "fn" => Kind::Fn,
                    "struct" => Kind::Struct,
                    "mod" => Kind::Mod,
                    "use" => Kind::Use,
                    "pub" => Kind::Pub,
                    "let" => Kind::Let,
                    "mut" => Kind::Mut,
                    "return" => Kind::Return,
                    "break" => Kind::Break,
                    "continue" => Kind::Continue,
                    "if" => Kind::If,
                    "while" => Kind::While,
                    "else" => Kind::Else,
                    "true" => Kind::True,
                    "false" => Kind::False,
                    "import" | "macro" | "macro_rules" | "const" | "for" | "loop" | "match"
                    | "async" | "await" | "move" | "ref" | "unsafe" | "extern" | "enum"
                    | "trait" | "impl" | "type" | "null" | "and" | "or" => Kind::Unsupported,
                    _ => Kind::Ident,
                }
            }
            c if c.is_numeric() => {
                // Preserve the whole candidate, including unsupported Unicode,
                // suffix/radix/float spelling. Parsing accepts ASCII digits only.
                while cursor < bytes.len() {
                    let next = text[cursor..].chars().next().unwrap();
                    if !next.is_alphanumeric() && !matches!(next, '_' | '.') {
                        break;
                    }
                    cursor += next.len_utf8();
                }
                Kind::Number
            }
            '"' => {
                let mut closed = false;
                while cursor < bytes.len() {
                    let next = text[cursor..].chars().next().unwrap();
                    cursor += next.len_utf8();
                    if next == '"' {
                        closed = true;
                        break;
                    }
                    if next == '\\' && cursor < bytes.len() {
                        cursor += text[cursor..].chars().next().unwrap().len_utf8();
                    }
                }
                if !closed {
                    return Err(Failure {
                        reason: FailureReason::UnterminatedString,
                        span: source.span(start, cursor),
                    });
                }
                Kind::String
            }
            '(' => Kind::LParen,
            ')' => Kind::RParen,
            '{' => Kind::LBrace,
            '}' => Kind::RBrace,
            ':' => Kind::Colon,
            ',' => Kind::Comma,
            ';' => Kind::Semi,
            '=' if bytes.get(cursor) == Some(&b'=') => {
                cursor += 1;
                Kind::EqualEqual
            }
            '=' => Kind::Equal,
            '!' if bytes.get(cursor) == Some(&b'=') => {
                cursor += 1;
                Kind::NotEqual
            }
            '!' => Kind::Not,
            '&' if bytes.get(cursor) == Some(&b'&') => {
                cursor += 1;
                Kind::AndAnd
            }
            '&' => Kind::Ampersand,
            '.' => Kind::Dot,
            '|' if bytes.get(cursor) == Some(&b'|') => {
                cursor += 1;
                Kind::OrOr
            }
            '<' if bytes.get(cursor) == Some(&b'=') => {
                cursor += 1;
                Kind::LessEqual
            }
            '>' if bytes.get(cursor) == Some(&b'=') => {
                cursor += 1;
                Kind::GreaterEqual
            }
            '<' => Kind::Less,
            '>' => Kind::Greater,
            '-' if bytes.get(cursor) == Some(&b'>') => {
                cursor += 1;
                Kind::Arrow
            }
            '-' => Kind::Minus,
            '+' => Kind::Plus,
            '*' => Kind::Star,
            '/' => Kind::Slash,
            '%' => Kind::Percent,
            '|' | '[' | ']' | '#' | '\'' => Kind::Unsupported,
            _ => Kind::Invalid,
        };
        let span = source.span(start, cursor);
        if cursor - start > MAX_TOKEN_BYTES || tokens.len() >= limit {
            return Err(Failure {
                reason: FailureReason::TokenLimit,
                span,
            });
        }
        append(
            &mut tokens,
            Token { kind, span },
            ceiling,
            &mut previous_requested,
            allocator,
            observer,
        )?;
    }
    append(
        &mut tokens,
        Token {
            kind: Kind::Eof,
            span: source.span(cursor, cursor),
        },
        ceiling,
        &mut previous_requested,
        allocator,
        observer,
    )?;
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::source::{SourceFileId, SourceMap};

    fn source(text: &str) -> SourceMap {
        let mut sources = SourceMap::new();
        sources.add("lexer-reservation.ox".into(), text.into());
        sources
    }

    #[test]
    fn lexer_reservation_qualified_schedule_and_each_capacity_failure() {
        // Independent contract: append indices 0,4,8,...,65536. Include EOF
        // growth at every boundary and all 16 maximal-tape requests.
        for count in [0, 1, 3, 4, 5, 7, 8, 15, 16, 31, 32, 65535, 65536, 100000] {
            let sources = source(&";".repeat(count));
            let file = sources.get(SourceFileId(0));
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(16).unwrap();
            let mut observer = ReservationObserver::default();
            let tape = lex_core(file, count, &mut allocator, &mut observer).unwrap();
            let expected: Vec<_> = [
                0, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
            ]
            .into_iter()
            .filter(|index| *index <= count)
            .collect();
            assert_eq!(allocator.attempts, expected.len());
            assert_eq!(observer.length, expected.len());
            assert!(!observer.overflow && !allocator.observer_trace_overflow);
            assert_eq!(tape.len(), count + 1);
            for (index, append_index) in expected.iter().copied().enumerate() {
                let event = observer.events[index].unwrap();
                let target = if index == 0 { 4 } else { 4 << index };
                assert_eq!(event.previous_requested, append_index);
                assert_eq!(event.actual_old_capacity, append_index);
                assert_eq!(event.new_requested, target);
                assert_eq!(
                    event.actual_returned_capacity, target,
                    "exact-capacity host qualification"
                );
                assert!(event.success);
                assert_eq!(allocator.trace[index].kind, "lexer token tape");
                assert_eq!(allocator.trace[index].length, target);
                let mut failed = Allocator {
                    fail_at: Some(index + 1),
                    ..Allocator::default()
                };
                failed.observer_trace_bound(16).unwrap();
                let mut failed_observer = ReservationObserver::default();
                let error = lex_core(file, count, &mut failed, &mut failed_observer).unwrap_err();
                assert_eq!(error.reason, FailureReason::StorageAllocation);
                assert_eq!(
                    error.span,
                    file.span(append_index, (append_index + 1).min(count))
                );
                assert_eq!(failed.attempts, index + 1);
                assert_eq!(failed_observer.length, index + 1);
                assert!(!failed_observer.events[index].unwrap().success);
                assert_eq!(
                    failed_observer.events[index]
                        .unwrap()
                        .actual_returned_capacity,
                    append_index
                );
                assert!(!failed_observer.overflow && !failed.observer_trace_overflow);
                let diagnostic = error.diagnostic();
                assert_eq!(
                    (
                        diagnostic.code,
                        diagnostic.stage,
                        diagnostic.message.as_str()
                    ),
                    ("E0400", "lex", "token storage allocation failed")
                );
                assert_eq!(diagnostic.primary, Some(error.span));
                assert!(diagnostic.secondary.is_empty() && diagnostic.notes.is_empty());
            }
        }
    }

    #[test]
    fn lexer_reservation_admission_exact_one_short_and_non_growing_pushes() {
        let sources = source(";;;");
        let file = sources.get(SourceFileId(0));
        let exact = 4 * std::mem::size_of::<Token>();
        for budget in [exact - 1, exact] {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(1).unwrap();
            let mut observer = ReservationObserver {
                requested_byte_limit: Some(budget),
                ..Default::default()
            };
            let result = lex_core(file, 3, &mut allocator, &mut observer);
            if budget == exact {
                assert_eq!(result.unwrap().len(), 4);
                assert_eq!(allocator.attempts, 1);
            } else {
                assert_eq!(
                    result.unwrap_err(),
                    Failure {
                        reason: FailureReason::StorageLimit,
                        span: file.span(0, 1)
                    }
                );
                assert_eq!(allocator.attempts, 0);
            }
        }
        let mut allocator = Allocator {
            fail_at: Some(2),
            ..Default::default()
        };
        let tape = lex_with_allocator(file, 3, &mut allocator).unwrap();
        assert_eq!(tape.len(), 4);
        assert_eq!(
            allocator.attempts, 1,
            "three non-growing pushes consume no ordinal"
        );
    }

    #[test]
    fn lexer_reservation_checked_counter_failure_is_inline_resource() {
        let sources = source("");
        let file = sources.get(SourceFileId(0));
        let mut allocator = Allocator {
            attempts: usize::MAX,
            ..Default::default()
        };
        let failure = lex_with_allocator(file, 0, &mut allocator).unwrap_err();
        assert_eq!(
            failure,
            Failure {
                reason: FailureReason::StorageLimit,
                span: file.span(0, 0)
            }
        );
        assert_eq!(allocator.attempts, usize::MAX);
        assert!(allocator.trace.is_empty());
        assert_eq!(
            failure.diagnostic().message,
            "token storage resource limit exceeded"
        );
    }

    #[test]
    fn lexer_reservation_models_over_return_without_capacity_rejection() {
        assert_eq!(request_ceiling(usize::MAX, usize::MAX), Some(131072));
        for (capacity, expected) in [
            (0, Some(4)),
            (4, Some(8)),
            (6, Some(8)),
            (9, Some(16)),
            (20, Some(32)),
            (32, None),
            (40, None),
        ] {
            assert_eq!(next_target(capacity, 32), expected);
        }
        assert_eq!(next_target(usize::MAX, 131072), None);
        // An over-return above W remains usable while spare capacity exists.
        let mut tape = Vec::with_capacity(8);
        let mut previous = 4;
        let mut allocator = Allocator::default();
        let mut observer = ReservationObserver::default();
        let sources = source("");
        append(
            &mut tape,
            Token {
                kind: Kind::Eof,
                span: sources.get(SourceFileId(0)).span(0, 0),
            },
            Some(4),
            &mut previous,
            &mut allocator,
            &mut observer,
        )
        .unwrap();
        assert_eq!(allocator.attempts, 0);
        assert_eq!(tape.capacity(), 8);
    }

    #[test]
    fn lexer_reservation_recognition_precedes_current_candidate_admission() {
        for (text, reason) in [
            ("\"unfinished", FailureReason::UnterminatedString),
            ("/*unfinished", FailureReason::UnterminatedBlockComment),
        ] {
            let sources = source(text);
            let mut allocator = Allocator {
                fail_at: Some(1),
                ..Default::default()
            };
            let failure =
                lex_with_allocator(sources.get(SourceFileId(0)), 0, &mut allocator).unwrap_err();
            assert_eq!(failure.reason, reason);
            assert_eq!(allocator.attempts, 0);
            let prefixed = source(&format!(";{text}"));
            let failure =
                lex_with_allocator(prefixed.get(SourceFileId(0)), MAX_TOKENS, &mut allocator)
                    .unwrap_err();
            assert_eq!(failure.reason, FailureReason::StorageAllocation);
            assert_eq!(failure.span.start, 0);
            assert_eq!(failure.span.end, 1);
        }
        let sources = source(";");
        let mut allocator = Allocator {
            fail_at: Some(1),
            ..Default::default()
        };
        assert_eq!(
            lex_with_allocator(sources.get(SourceFileId(0)), 0, &mut allocator)
                .unwrap_err()
                .reason,
            FailureReason::TokenLimit
        );
        assert_eq!(allocator.attempts, 0);
    }

    #[test]
    fn lexer_reservation_fixed_receipt_overflow_is_visible() {
        let mut observer = ReservationObserver::default();
        for _ in 0..17 {
            observer.record(0, 0, 4, 4, true);
        }
        assert_eq!(observer.length, 16);
        assert!(observer.overflow);
        println!(
            "LEXER_RESERVATION_LAYOUT failure={} result={} sink={} observer_carriers={} named_scratch={}",
            std::mem::size_of::<Failure>(),
            std::mem::size_of::<Result<Vec<Token>, Failure>>(),
            std::mem::size_of::<ReservationObserver>(),
            reservation_observer_bytes(),
            reservation_scratch_bytes()
        );
    }
    #[test]
    fn token_tape_retains_every_byte_and_exact_numeric_spelling() {
        let text =
            "// 雪\r\n fn f() -> () { /* é */ 900719925474099312345678901234567890; return; } @";
        let mut sources = SourceMap::new();
        let id = sources.add("input.ox".into(), text.into());
        let tokens = lex(sources.get(id)).unwrap();
        let mut end = 0;
        let mut reconstructed = String::new();
        for token in &tokens {
            assert_eq!(token.span.file, SourceFileId(0));
            assert_eq!(token.span.start, end);
            reconstructed.push_str(&text[token.span.start..token.span.end]);
            end = token.span.end;
        }
        assert_eq!(reconstructed, text);
        let number = tokens
            .iter()
            .find(|token| token.kind == Kind::Number)
            .unwrap();
        assert_eq!(
            &text[number.span.start..number.span.end],
            "900719925474099312345678901234567890"
        );
        assert_eq!(tokens.last().unwrap().kind, Kind::Eof);
        assert_eq!(tokens.last().unwrap().span.start, text.len());
        assert!(tokens.iter().any(|token| token.kind == Kind::Invalid));
    }
}
#[cfg(test)]
mod lexer_real_null_tests {
    use super::super::project::budget::real_null_observer::{
        self as fresh, growth, Operation, Target,
    };
    use super::*;
    use std::alloc::Layout;
    use std::mem::{align_of, align_of_val, size_of, size_of_val};

    fn core_action<'s>(
        source: &'s super::super::source::SourceFile,
        limit: usize,
        observer: &'s mut ReservationObserver,
    ) -> impl for<'a> FnOnce(&'a mut Allocator) -> Option<Failure> + 's {
        move |allocator| lex_core(source, limit, allocator, observer).err()
    }

    fn sources(text: &str) -> super::super::source::SourceMap {
        let mut sources = super::super::source::SourceMap::new();
        sources.add("null-lexer.ox".into(), text.into());
        sources
    }
    fn target(attempt: usize, old: usize) -> growth::GrowthTarget {
        growth::GrowthTarget {
            attempt,
            kind: "lexer token tape",
            old_len: old,
            old_capacity: old,
            additional: old,
            new_slots: old * 2,
            element_bytes: size_of::<Token>(),
            element_align: align_of::<Token>(),
            old_layout: Layout::array::<Token>(old).unwrap(),
            new_layout: Layout::array::<Token>(old * 2).unwrap(),
            operation: Operation::Realloc,
        }
    }
    fn accepted(report: growth::GrowthReport, target: growth::GrowthTarget) {
        assert_eq!(report.target, target);
        assert!(report.selected && report.matched && report.fired && report.trace_preserved);
        assert_eq!(report.rejection, None);
        assert_eq!(report.reserve_failed, Some(true));
        assert!(
            report.owner_unchanged
                && report.address_unchanged
                && report.length_unchanged
                && report.capacity_unchanged
        );
        let actual = report.actual.unwrap();
        assert_eq!(actual.operation, Operation::Realloc);
        assert_eq!(actual.layout, target.old_layout);
        assert_eq!(actual.new_size, Some(target.new_layout.size()));
        assert!(actual.old_address_matches);
        let drop = report.drop_event.unwrap();
        assert_eq!(drop.layout, target.old_layout);
        assert!(drop.old_address_matches && drop.after_reserve_return);
        assert_eq!(report.drop_count, 1);
    }

    #[test]
    fn lexer_null_layout_only_never_invokes_selection() {
        let sources = sources(";;;;");
        let source = sources.get(super::super::source::SourceFileId(0));
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(16).unwrap();
        let mut observer = ReservationObserver::default();
        let action = core_action(source, 4, &mut observer);
        let growth_selection = growth::selection_carriers_bytes(&action);
        let fresh_selection = fresh::selection_carriers_bytes(&action);
        let trace = 16usize
            .checked_mul(size_of::<super::super::project::budget::ReserveEvent>())
            .unwrap();
        // Trace payload is a separate heap ledger, not fixed driver storage.
        let driver_terms = [
            reservation_observer_bytes(),
            size_of::<Allocator>(),
            size_of::<Option<Failure>>(),
            size_of::<&super::super::source::SourceFile>(),
            size_of::<super::super::source::SourceMap>(),
            size_of::<Target>(),
            size_of::<fresh::Report>(),
            size_of::<growth::GrowthTarget>(),
            size_of::<growth::GrowthReport>(),
            size_of::<[usize; 16]>(),
            size_of::<[Option<usize>; 3]>(),
        ];
        let driver = driver_terms
            .into_iter()
            .try_fold(0usize, usize::checked_add)
            .unwrap();
        let terms = [
            driver,
            growth_selection.max(fresh_selection),
            growth::fixed_carriers_bytes::<Token>(),
            reservation_scratch_bytes(),
        ];
        let admitted = terms
            .into_iter()
            .try_fold(0usize, usize::checked_add)
            .unwrap();
        let admission = |limit| {
            terms
                .into_iter()
                .try_fold(0usize, usize::checked_add)
                .filter(|&required| required <= limit)
        };
        assert_eq!(admission(admitted), Some(admitted));
        assert_eq!(admission(admitted - 1), None);
        println!("LEXER_NULL_CARRIERS sink={} sink_bank={} allocator={} trace_requested={} trace_payload_separate=true core_result={}/{} closure={}/{} growth_selection={} fresh_selection={} driver={} fixed={} scratch={} exclusive_named_total={} conservative_both={}",
            size_of::<ReservationObserver>(), reservation_observer_bytes(), size_of::<Allocator>(), trace,
            size_of::<Option<Failure>>(), align_of::<Option<Failure>>(), size_of_val(&action), align_of_val(&action),
            growth_selection, fresh_selection, driver, growth::fixed_carriers_bytes::<Token>(),
            reservation_scratch_bytes(), admitted, admitted.checked_add(growth_selection.min(fresh_selection)).unwrap());
        println!(
            "LEXER_NULL_DRIVER_TERMS {driver_terms:?} source_retained={} trace_retained={}",
            sources.heap_capacity_bytes().unwrap(),
            allocator.trace.capacity() * size_of::<super::super::project::budget::ReserveEvent>()
        );
        // Dropping this closure releases only its captured references; it does
        // not invoke the lexical action or install either selector.
        drop(action);
        // Measuring the closure must not execute it, allocate a token tape, arm a
        // null, or mutate its borrowed fixed sink.
        assert_eq!(observer.length, 0);
        assert_eq!(allocator.attempts, 0);
        assert!(allocator.trace.is_empty());
    }

    #[test]
    fn lexer_real_null_initial_alloc_includes_empty_eof_and_public_adaptation() {
        for (text, limit, end) in [("", 0, 0), (";", 1, 1), (";;;;", 4, 1)] {
            let sources = sources(text);
            let source = sources.get(super::super::source::SourceFileId(0));
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(16).unwrap();
            let mut observer = ReservationObserver::default();
            let target = Target {
                attempt: 1,
                kind: "lexer token tape",
                slots: 4,
                element_bytes: size_of::<Token>(),
                layout: Layout::array::<Token>(4).unwrap(),
            };
            let (failure, report) = fresh::with_selected(
                &mut allocator,
                target,
                core_action(source, limit, &mut observer),
            )
            .unwrap();
            assert!(report.selected && report.matched && report.fired);
            assert_eq!(report.rejection, None);
            assert_eq!(report.actual.unwrap().operation, Operation::Alloc);
            let failure = failure.unwrap();
            assert_eq!(failure.reason, FailureReason::StorageAllocation);
            assert_eq!(failure.span, source.span(0, end));
            assert_eq!(allocator.attempts, 1);
            assert_eq!(allocator.trace.len(), 1);
            assert!(!allocator.trace[0].success && !allocator.observer_trace_overflow);
            assert_eq!(observer.length, 1);
            assert_eq!(observer.events[0].unwrap().actual_returned_capacity, 0);
            assert!(!observer.overflow);
            // Public conversion is deliberately after selective failure release.
            let diagnostic = failure.diagnostic();
            assert_eq!(
                (
                    diagnostic.code,
                    diagnostic.stage,
                    diagnostic.message.as_str()
                ),
                ("E0400", "lex", "token storage allocation failed")
            );
            assert_eq!(diagnostic.primary, Some(source.span(0, end)));
            assert!(diagnostic.secondary.is_empty() && diagnostic.notes.is_empty());
        }
    }

    #[test]
    fn lexer_real_null_each_realloc_site_and_eof_drop_old_tape_once() {
        // Independent baseline append boundaries: no successful-run receipt is
        // used to choose these absolute allocator ordinals or target layouts.
        for count in [4, 5, 8, 100_000] {
            let sources = sources(&";".repeat(count));
            let source = sources.get(super::super::source::SourceFileId(0));
            for (index, old) in [
                4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
            ]
            .into_iter()
            .enumerate()
            .filter(|(_, old)| *old <= count)
            {
                let mut allocator = Allocator::default();
                allocator.observer_trace_bound(16).unwrap();
                let mut observer = ReservationObserver::default();
                let selected = target(index + 2, old);
                let (failure, report) = growth::with_selected_growth(
                    &mut allocator,
                    selected,
                    core_action(source, count, &mut observer),
                )
                .unwrap();
                accepted(report, selected);
                let failure = failure.unwrap();
                assert_eq!(failure.reason, FailureReason::StorageAllocation);
                assert_eq!(failure.span, source.span(old, (old + 1).min(count)));
                assert_eq!(allocator.attempts, selected.attempt);
                assert_eq!(allocator.trace.len(), selected.attempt);
                assert!(allocator.trace[..selected.attempt - 1]
                    .iter()
                    .all(|row| row.success));
                assert!(!allocator.trace.last().unwrap().success);
                assert!(!allocator.observer_trace_overflow);
                assert_eq!(observer.length, selected.attempt);
                assert!(!observer.overflow);
                let event = observer.events[selected.attempt - 1].unwrap();
                assert_eq!(event.previous_requested, old);
                assert_eq!(event.actual_old_capacity, old);
                assert_eq!(event.new_requested, old * 2);
                assert_eq!(event.actual_returned_capacity, old);
                assert!(!event.success);
            }
        }
    }
}
