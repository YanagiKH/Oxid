//! Bounded syntax-only formatting. Only whitespace gaps may change.
use super::{
    ast::{Argument, BorrowPlace, ExprKind, Program},
    diagnostic::Diagnostic,
    lexer::{self, Kind, Token},
    parser::{self, SourceMode},
    project::budget::Allocator,
    source::{SourceFile, SourceMap, MAX_SOURCE_BYTES},
};

const MAX_DELIMITERS: usize = 128;
const MAX_WORK_BYTES: usize = 8 * 1024 * 1024;
const TIGHT_AFTER: u8 = 1;
const PATH_COLON: u8 = 2;

#[derive(Clone, Copy)]
struct Limits {
    delimiters: usize,
    work_bytes: usize,
}
impl Limits {
    const DEFAULT: Self = Self {
        delimiters: MAX_DELIMITERS,
        work_bytes: MAX_WORK_BYTES,
    };
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FormatSyntax {
    #[allow(dead_code)] // Historical policy remains available to internal controls.
    Closed,
    #[allow(dead_code)]
    Enabled,
    #[cfg(test)]
    EnumCandidate,
}
impl FormatSyntax {
    fn candidate(self) -> bool {
        match self {
            Self::Closed => false,
            Self::Enabled => true,
            #[cfg(test)]
            Self::EnumCandidate => true,
        }
    }
}

/// Stack-only qualification facts; never a source or executable witness.
#[derive(Debug, Default)]
pub(super) struct EnumFormatMetrics {
    #[cfg(test)]
    pub input_source_heap: usize,
    #[cfg(test)]
    pub first_ast_heap: usize,
    #[cfg(test)]
    pub first_token_heap: usize,
    #[cfg(test)]
    pub first_parse_peak_bound: usize,
    #[cfg(test)]
    pub roles_heap: usize,
    #[cfg(test)]
    pub output_heap: usize,
    #[cfg(test)]
    pub source_owner_heap: usize,
    #[cfg(test)]
    pub second_ast_heap: usize,
    #[cfg(test)]
    pub second_token_heap: usize,
    #[cfg(test)]
    pub second_parse_peak_bound: usize,
    #[cfg(test)]
    pub emit_live_heap: usize,
    #[cfg(test)]
    pub reparse_live_heap: usize,
    #[cfg(test)]
    pub phase_peak_heap_bound: usize,
    #[cfg(test)]
    pub parse_calls: usize,
}
#[derive(Default)]
struct ParseMetrics {
    #[cfg(test)]
    ast_heap: usize,
    #[cfg(test)]
    tokens: usize,
    #[cfg(test)]
    peak_bound: usize,
}

/// No file access, project discovery, type checking or execution. Diagnostics
/// refer only to `source`; candidate validation failures have no source span.
pub(super) fn format_source(source: &SourceFile) -> Result<String, Vec<Diagnostic>> {
    format_with_allocator(source, &mut Allocator::default())
}

fn format_with_allocator(
    source: &SourceFile,
    allocator: &mut Allocator,
) -> Result<String, Vec<Diagnostic>> {
    format_with_limits(source, allocator, Limits::DEFAULT)
}

fn format_with_limits(
    source: &SourceFile,
    allocator: &mut Allocator,
    limits: Limits,
) -> Result<String, Vec<Diagnostic>> {
    format_with_syntax(source, allocator, limits, FormatSyntax::Enabled).0
}

#[cfg(test)]
pub(super) fn format_enum_candidate_observed(
    source: &SourceFile,
    allocator: &mut Allocator,
) -> (Result<String, Vec<Diagnostic>>, EnumFormatMetrics) {
    format_with_syntax(
        source,
        allocator,
        Limits::DEFAULT,
        FormatSyntax::EnumCandidate,
    )
}

fn format_with_syntax(
    source: &SourceFile,
    allocator: &mut Allocator,
    limits: Limits,
    syntax: FormatSyntax,
) -> (Result<String, Vec<Diagnostic>>, EnumFormatMetrics) {
    let mut metrics = EnumFormatMetrics::default();
    let result = format_with_syntax_inner(source, allocator, limits, syntax, &mut metrics);
    (result, metrics)
}

fn format_with_syntax_inner(
    source: &SourceFile,
    allocator: &mut Allocator,
    limits: Limits,
    syntax: FormatSyntax,
    metrics: &mut EnumFormatMetrics,
) -> Result<String, Vec<Diagnostic>> {
    #[cfg(not(test))]
    let _ = &metrics;
    if source.text().len() > MAX_SOURCE_BYTES {
        return Err(resource("source byte limit exceeded"));
    }
    #[cfg(test)]
    if syntax.candidate() {
        metrics.input_source_heap = source
            .heap_capacity_bytes()
            .ok_or_else(|| resource("formatter capacity count overflow"))?;
        metrics.parse_calls += 1;
    }
    let (program, first_parse) = parse_with_syntax(source, allocator, syntax)?;
    #[cfg(not(test))]
    let _ = first_parse;
    #[cfg(test)]
    if syntax.candidate() {
        metrics.first_ast_heap = first_parse.ast_heap;
        metrics.first_token_heap = first_parse.tokens;
        metrics.first_parse_peak_bound = first_parse.peak_bound;
        metrics.phase_peak_heap_bound = first_parse.peak_bound;
    }
    let roles = roles_with_syntax(source, &program, allocator, limits.work_bytes, syntax)?;
    #[cfg(test)]
    if syntax.candidate() {
        metrics.roles_heap = roles.capacity();
        metrics
            .first_ast_heap
            .checked_add(metrics.roles_heap)
            .ok_or_else(|| resource("formatter capacity count overflow"))?;
    }
    let mut length = 0usize;
    layout(source, &program, &roles, limits.delimiters, |part| {
        length = output_length(length, part.len())?;
        Ok(())
    })?;
    #[cfg(test)]
    if syntax.candidate() {
        metrics
            .first_ast_heap
            .checked_add(metrics.roles_heap)
            .and_then(|n| n.checked_add(length))
            .ok_or_else(|| resource("formatter capacity count overflow"))?;
    }
    let mut output = String::new();
    allocator
        .string(&mut output, length, "formatted source")
        .map_err(|_| resource("formatted source allocation failed"))?;
    if syntax.candidate() && output.capacity() != length {
        return Err(resource("formatted source capacity exceeded admission"));
    }
    #[cfg(test)]
    if syntax.candidate() {
        metrics.output_heap = output.capacity();
        metrics.emit_live_heap = metrics.first_ast_heap + metrics.roles_heap + metrics.output_heap;
        metrics.phase_peak_heap_bound = metrics.phase_peak_heap_bound.max(metrics.emit_live_heap);
    }
    layout(source, &program, &roles, limits.delimiters, |part| {
        if syntax.candidate()
            && output
                .len()
                .checked_add(part.len())
                .is_none_or(|next| next > length || next > output.capacity())
        {
            return Err(invariant("formatter output exceeded its counted capacity"));
        }
        output.push_str(part);
        Ok(())
    })?;
    debug_assert_eq!(output.len(), length);
    drop(roles);

    // Move, rather than clone, the candidate into its immutable source owner.
    let mut candidates = SourceMap::new();
    let id = match syntax {
        FormatSyntax::Closed => candidates.try_add(String::new(), output, allocator),
        _ => {
            let lines = output
                .bytes()
                .filter(|&byte| byte == b'\n')
                .count()
                .checked_add(1)
                .ok_or_else(|| resource("formatter capacity count overflow"))?;
            let owner_target = lines
                .checked_mul(std::mem::size_of::<usize>())
                .and_then(|n| n.checked_add(std::mem::size_of::<SourceFile>()))
                .and_then(|n| n.checked_add(output.capacity()))
                .ok_or_else(|| resource("formatter capacity count overflow"))?;
            #[cfg(test)]
            metrics
                .first_ast_heap
                .checked_add(owner_target)
                .ok_or_else(|| resource("formatter capacity count overflow"))?;
            #[cfg(not(test))]
            let _ = owner_target;
            let id = candidates.try_add_format_candidate(output, allocator);
            #[cfg(test)]
            if id.is_ok() && candidates.heap_capacity_bytes() != Some(owner_target) {
                return Err(resource(
                    "formatted source owner capacity exceeded admission",
                ));
            }
            id
        }
    }
    .map_err(|_| resource("formatted source allocation failed"))?;
    #[cfg(test)]
    if syntax.candidate() {
        metrics.source_owner_heap = candidates
            .heap_capacity_bytes()
            .ok_or_else(|| resource("formatter capacity count overflow"))?;
        metrics.parse_calls += 1;
    }
    let candidate = candidates.get(id);
    let (parsed, second_parse) =
        parse_with_syntax(candidate, allocator, syntax).map_err(|errors| {
            if errors.iter().any(|error| error.code == "E0400") {
                resource("formatted source exceeds lexer or parser resource limits")
            } else {
                invariant("formatted source no longer parses")
            }
        })?;
    #[cfg(not(test))]
    let _ = second_parse;
    #[cfg(test)]
    if syntax.candidate() {
        metrics.second_ast_heap = second_parse.ast_heap;
        metrics.second_token_heap = second_parse.tokens;
        metrics.second_parse_peak_bound = second_parse.peak_bound;
        let before = metrics
            .first_ast_heap
            .checked_add(metrics.source_owner_heap)
            .ok_or_else(|| resource("formatter capacity count overflow"))?;
        metrics.reparse_live_heap = before
            .checked_add(second_parse.ast_heap)
            .ok_or_else(|| resource("formatter capacity count overflow"))?;
        metrics.phase_peak_heap_bound = metrics.phase_peak_heap_bound.max(
            before
                .checked_add(second_parse.peak_bound)
                .ok_or_else(|| resource("formatter capacity count overflow"))?,
        );
    }
    if !same_projection(source, &program.tokens, candidate, &parsed.tokens) {
        return Err(invariant(
            "formatted source changed tokens, comments or line breaks",
        ));
    }
    Ok(candidates.into_single_text())
}

fn parse(source: &SourceFile, allocator: &mut Allocator) -> Result<Program, Vec<Diagnostic>> {
    let tokens = lexer::lex_with_allocator(source, lexer::MAX_TOKENS, allocator)
        .map_err(|error| vec![*error.diagnostic()])?;
    parser::parse_counted_with_arrays(
        source,
        tokens,
        SourceMode::ProjectCandidate,
        parser::MAX_NODES,
        allocator,
        parser::ArraySyntaxPolicy::Enabled,
    )
    .map(|(program, _)| program)
}

fn parse_with_syntax(
    source: &SourceFile,
    allocator: &mut Allocator,
    syntax: FormatSyntax,
) -> Result<(Program, ParseMetrics), Vec<Diagnostic>> {
    match syntax {
        FormatSyntax::Closed => {
            parse(source, allocator).map(|program| (program, ParseMetrics::default()))
        }
        _ => {
            let tokens = lexer::lex_with_allocator(source, lexer::MAX_TOKENS, allocator)
                .map_err(|error| vec![*error.diagnostic()])?;
            let token_bytes = tokens
                .capacity()
                .checked_mul(std::mem::size_of::<Token>())
                .ok_or_else(|| resource("formatter capacity count overflow"))?;
            let mut storage = Default::default();
            let (program, _) = match syntax {
                FormatSyntax::Enabled => parser::parse_typed_counted(
                    source,
                    tokens,
                    SourceMode::ProjectCandidate,
                    parser::MAX_NODES,
                    allocator,
                    &mut storage,
                ),
                #[cfg(test)]
                FormatSyntax::EnumCandidate => parser::parse_enum_candidate_counted(
                    source,
                    tokens,
                    SourceMode::ProjectCandidate,
                    parser::MAX_NODES,
                    allocator,
                    &mut storage,
                ),
                FormatSyntax::Closed => return Err(invariant("invalid formatter syntax policy")),
            }?;
            let ast_heap = storage
                .retained_capacity
                .checked_add(token_bytes)
                .ok_or_else(|| resource("formatter capacity count overflow"))?;
            let peak_bound = storage
                .peak_capacity_bound
                .checked_add(token_bytes)
                .ok_or_else(|| resource("formatter capacity count overflow"))?;
            #[cfg(not(test))]
            let _ = (ast_heap, peak_bound);
            Ok((
                program,
                ParseMetrics {
                    #[cfg(test)]
                    ast_heap,
                    #[cfg(test)]
                    tokens: token_bytes,
                    #[cfg(test)]
                    peak_bound,
                },
            ))
        }
    }
}

fn output_length(current: usize, additional: usize) -> Result<usize, Vec<Diagnostic>> {
    current
        .checked_add(additional)
        .filter(|&length| length <= MAX_SOURCE_BYTES)
        .ok_or_else(|| resource("formatted source byte limit exceeded"))
}

fn resource(message: &str) -> Vec<Diagnostic> {
    vec![*Diagnostic::new("E0400", "format", message, None)]
}

fn invariant(message: &str) -> Vec<Diagnostic> {
    vec![*Diagnostic::new("E0500", "format", message, None)]
}

/// One byte per input byte is the only formatter work table (at most 1 MiB,
/// below the 8 MiB work budget). AST-origin byte indices avoid per-node searches
/// of the token tape. Paths are disjoint syntactic occurrences, so all marked
/// path ranges together require at most one source-length scan.
fn roles_with_syntax(
    source: &SourceFile,
    program: &Program,
    allocator: &mut Allocator,
    work_limit: usize,
    syntax: FormatSyntax,
) -> Result<Vec<u8>, Vec<Diagnostic>> {
    if !program.belongs_to(source) {
        return Err(invariant("formatter source association mismatch"));
    }
    let length = source.text().len();
    if length > work_limit.min(MAX_WORK_BYTES) {
        return Err(resource("formatter work table limit exceeded"));
    }
    let mut roles = Vec::new();
    let reservation = if syntax.candidate() {
        allocator.vector_exact(&mut roles, length, "formatter token roles")
    } else {
        allocator.vector(&mut roles, length, "formatter token roles")
    };
    reservation.map_err(|_| resource("formatter work table allocation failed"))?;
    if syntax.candidate() && roles.capacity() != length {
        return Err(resource("formatter work table capacity exceeded admission"));
    }
    roles.resize(length, 0);
    if syntax.candidate() {
        // Preserve the unchanged two-token arrow; do not skip raw trivia.
        for pair in program.tokens.windows(2) {
            if pair[0].kind == Kind::Equal
                && pair[1].kind == Kind::Greater
                && pair[0].span.end == pair[1].span.start
            {
                roles[pair[0].span.start] |= TIGHT_AFTER;
            }
        }
    }
    for expression in &program.expressions {
        match &expression.kind {
            ExprKind::Negate { .. } | ExprKind::Number { negative: true, .. } => {
                roles[expression.span.start] |= TIGHT_AFTER;
            }
            ExprKind::IndexRead { base, .. } => {
                // A projected array receiver ends at its final field token.
                let end = program
                    .tokens
                    .partition_point(|token| token.span.end <= base.end);
                let receiver = program
                    .tokens
                    .get(end.saturating_sub(1))
                    .ok_or_else(|| invariant("missing indexed receiver"))?;
                roles[receiver.span.start] |= TIGHT_AFTER;
            }
            ExprKind::Call { args, .. }
            | ExprKind::QualifiedValue {
                args: Some(args), ..
            } => {
                for argument in args {
                    if let Argument::Borrow {
                        place: BorrowPlace::ForwardedParameter { star_span, .. },
                        ..
                    } = argument
                    {
                        roles[star_span.start] |= TIGHT_AFTER;
                    }
                }
            }
            _ => {}
        }
    }
    for path in &program.paths {
        for (byte, role) in source.text().as_bytes()[path.span.start..path.span.end]
            .iter()
            .zip(&mut roles[path.span.start..path.span.end])
        {
            if *byte == b':' {
                *role |= PATH_COLON;
            }
        }
    }
    Ok(roles)
}

fn protected(source: &SourceFile, token: &Token) -> bool {
    token.kind != Kind::Eof
        && (token.kind != Kind::Trivia
            || source.text_at(token.span).starts_with("//")
            || source.text_at(token.span).starts_with("/*"))
}

fn newlines(text: &str) -> usize {
    text.bytes().filter(|&byte| byte == b'\n').count()
}

enum Delimiter {
    Open(u8),
    Close(u8),
}

// Brackets retain their existing lexer kind. Recognize only the exact admitted
// spelling, never unrelated Unsupported tokens or punctuation inside comments.
fn delimiter(source: &SourceFile, token: &Token) -> Option<Delimiter> {
    match token.kind {
        Kind::LParen => Some(Delimiter::Open(b'(')),
        Kind::RParen => Some(Delimiter::Close(b'(')),
        Kind::LBrace => Some(Delimiter::Open(b'{')),
        Kind::RBrace => Some(Delimiter::Close(b'{')),
        Kind::Unsupported => match source.text_at(token.span) {
            "[" => Some(Delimiter::Open(b'[')),
            "]" => Some(Delimiter::Close(b'[')),
            _ => None,
        },
        _ => None,
    }
}

/// Count and emit using the same fixed-depth walk. The counting pass checks the
/// complete output size before allocating or constructing any output bytes.
fn layout(
    source: &SourceFile,
    program: &Program,
    roles: &[u8],
    delimiter_limit: usize,
    mut emit: impl FnMut(&str) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    let mut stack = [0; MAX_DELIMITERS];
    let mut depth = 0usize;
    let mut previous: Option<&Token> = None;
    let spaces = [b' '; MAX_DELIMITERS * 4];
    let spaces = std::str::from_utf8(&spaces).expect("ASCII spaces");
    for (index, token) in program.tokens.iter().enumerate() {
        if !protected(source, token) {
            continue;
        }
        let breaks = previous.map_or(0, |previous| {
            newlines(&source.text()[previous.span.end..token.span.start])
        });
        if previous.is_none() || breaks > 0 {
            for _ in 0..breaks {
                emit("\n")?;
            }
            let closers = leading_closers(source, &program.tokens[index..]);
            let indent = depth
                .checked_sub(closers)
                .ok_or_else(|| invariant("formatter delimiter underflow"))?;
            emit(&spaces[..indent * 4])?;
        } else if let Some(previous) = previous {
            if horizontal_space(source, previous, token, roles) {
                emit(" ")?;
            }
        }
        emit(source.text_at(token.span))?;
        match delimiter(source, token) {
            Some(Delimiter::Open(kind)) => {
                if depth >= delimiter_limit.min(MAX_DELIMITERS) {
                    return Err(resource("formatter delimiter limit exceeded"));
                }
                stack[depth] = kind;
                depth += 1;
            }
            Some(Delimiter::Close(kind)) => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| invariant("formatter delimiter underflow"))?;
                if stack[depth] != kind {
                    return Err(invariant("formatter delimiter mismatch"));
                }
            }
            None => {}
        }
        previous = Some(token);
    }
    if depth != 0 {
        return Err(invariant("formatter unclosed delimiter"));
    }
    if previous.is_some() {
        emit("\n")?;
    }
    Ok(())
}

/// Only called at the start of a line, so no closer prefix is scanned twice.
/// A comment, non-closer or editable newline terminates the prefix.
fn leading_closers(source: &SourceFile, tokens: &[Token]) -> usize {
    let mut count = 0;
    for token in tokens {
        if matches!(delimiter(source, token), Some(Delimiter::Close(_))) {
            count += 1;
        } else if token.kind != Kind::Trivia
            || protected(source, token)
            || newlines(source.text_at(token.span)) != 0
        {
            break;
        }
    }
    count
}

fn horizontal_space(source: &SourceFile, left: &Token, right: &Token, roles: &[u8]) -> bool {
    use Kind::*;
    if left.kind == Trivia || right.kind == Trivia {
        return true;
    }
    if matches!(right.kind, Comma | Semi | RParen)
        || left.kind == LParen
        || matches!(delimiter(source, left), Some(Delimiter::Open(b'[')))
        || matches!(delimiter(source, right), Some(Delimiter::Close(b'[')))
        || (right.kind == LParen && left.kind == Ident)
        || left.kind == Dot
        || right.kind == Dot
        || (left.kind == Colon && roles[left.span.start] & PATH_COLON != 0)
        || (right.kind == Colon && roles[right.span.start] & PATH_COLON != 0)
        || right.kind == Colon
        || matches!(left.kind, Ampersand | Not)
        || roles[left.span.start] & TIGHT_AFTER != 0
        || (left.kind == LBrace && right.kind == RBrace)
    {
        return false;
    }
    true
}

/// A single interleaved projection protects comment anchors as well as exact
/// token spelling. Compare interior LF gaps separately; boundary gaps normalize.
fn same_projection(
    left: &SourceFile,
    left_tokens: &[Token],
    right: &SourceFile,
    right_tokens: &[Token],
) -> bool {
    let mut left_atoms = left_tokens.iter().filter(|token| protected(left, token));
    let mut right_atoms = right_tokens.iter().filter(|token| protected(right, token));
    let mut previous = None;
    loop {
        match (left_atoms.next(), right_atoms.next()) {
            (None, None) => return true,
            (Some(a), Some(b)) => {
                if a.kind != b.kind || left.text_at(a.span) != right.text_at(b.span) {
                    return false;
                }
                if let Some((a_end, b_end)) = previous {
                    if newlines(&left.text()[a_end..a.span.start])
                        != newlines(&right.text()[b_end..b.span.start])
                    {
                        return false;
                    }
                }
                previous = Some((a.span.end, b.span.end));
            }
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn source(text: &str) -> SourceMap {
        let mut sources = SourceMap::new();
        sources.add("format.ox".into(), text.into());
        sources
    }

    pub(super) fn formatted(text: &str) -> Result<String, Vec<Diagnostic>> {
        let sources = source(text);
        format_source(sources.get(super::super::source::SourceFileId(0)))
    }

    #[test]
    fn compact_format_is_syntax_only_and_idempotent() {
        let input = "mod absent;fn f(x:& mut Pair)->i32{sink(& mut * x);return (0007)- - 0002*03;}";
        let expected = "mod absent; fn f(x: &mut Pair) -> i32 { sink(&mut *x); return (0007) - -0002 * 03; }\n";
        assert_eq!(formatted(input).unwrap(), expected);
        assert_eq!(formatted(expected).unwrap(), expected);
    }

    #[test]
    fn fixed_arrays_format_without_semantic_checks() {
        let input = "mod absent;fn f(a:& [i32;02],b:& mut [bool;1])->[();0]{let mut x:[i32;2]=[01,- 2,];x [ 0 ]=x [1];sink(&x,&mut *b);return []; }";
        let expected = "mod absent; fn f(a: &[i32; 02], b: &mut [bool; 1]) -> [(); 0] { let mut x: [i32; 2] = [01, -2,]; x[0] = x[1]; sink(&x, &mut *b); return []; }\n";
        assert_eq!(formatted(input).unwrap(), expected);
        assert_eq!(formatted(expected).unwrap(), expected);
    }

    #[test]
    fn comments_newlines_and_leading_closers_are_preserved() {
        let input =
            "\nfn f()->(){\r\nsink(Pair{\nx:1\n});\n/* }\r\n \t keep */return;// tail  \r\n}\n\n";
        let expected = "fn f() -> () {\n    sink(Pair {\n            x: 1\n    });\n    /* }\r\n \t keep */ return; // tail  \r\n}\n";
        assert_eq!(formatted(input).unwrap(), expected);
        assert_eq!(formatted(expected).unwrap(), expected);
    }

    #[test]
    fn projection_rejects_token_comment_anchor_and_newline_changes() {
        let a = source("fn f()->(){/*a*/return;}\n");
        let a = a.get(super::super::source::SourceFileId(0));
        let a_tokens = lexer::lex(a).unwrap();
        for changed in [
            "fn g()->(){/*a*/return;}\n",
            "fn f()->(){return;/*a*/}\n",
            "fn f()->(){/*b*/return;}\n",
            "fn f()->(){/*a*/\nreturn;}\n",
        ] {
            let b = source(changed);
            let b = b.get(super::super::source::SourceFileId(0));
            assert!(
                !same_projection(a, &a_tokens, b, &lexer::lex(b).unwrap()),
                "{changed}"
            );
        }
    }

    #[test]
    fn invalid_source_rejects_whole_file() {
        for input in [
            "fn bad()->(){return} fn good()->(){return;}",
            "/* open",
            "use crate: :bad;",
        ] {
            assert!(formatted(input).is_err());
        }
    }

    #[test]
    fn builtin_current_formatter_rejects_unknown_and_nonimport_std_paths() {
        for input in [
            "use std::io::missing; fn main()->(){return;}",
            "use std::fs::ReadStatus; fn main()->(){return;}",
            "use std::io::*; fn main()->(){return;}",
            "use std::io::{ReadStatus,read_stdin}; fn main()->(){return;}",
            "fn f()->std::io::ReadStatus{return;}",
            "fn main()->(){std::io::read_stdin();}",
        ] {
            assert!(formatted(input).is_err(), "{input}");
        }
    }
}

#[cfg(test)]
#[path = "format/resource_tests.rs"]
mod resource_tests;

#[cfg(test)]
#[path = "format/ast_tests.rs"]
mod ast_tests;

#[cfg(test)]
#[path = "format/enum_candidate_tests.rs"]
mod enum_candidate_tests;

// Named production parse/owner controls, measured separately from test-only
// heap observations. Each reserve still uses its existing checked count.
#[allow(dead_code)]
struct ProductionFormatControls {
    syntax: FormatSyntax,
    parse_storage: parser::SyntaxStorage,
    parse_metrics: ParseMetrics,
    token_bytes: usize,
    ast_heap: usize,
    peak_bound: usize,
    owner_lines: usize,
    owner_target: usize,
    owner_return: Result<super::source::SourceFileId, super::project::budget::ReserveFailure>,
}
#[allow(dead_code)]
const fn production_format_control_bytes() -> usize {
    std::mem::size_of::<ProductionFormatControls>()
}
#[test]
fn bounded_enum_production_formatter_policy_layout() {
    println!(
        "ENUM_PRODUCTION_FORMAT_LAYOUT policy={} controls={} parse_metrics={} format_metrics={}",
        std::mem::size_of::<FormatSyntax>(),
        production_format_control_bytes(),
        std::mem::size_of::<ParseMetrics>(),
        std::mem::size_of::<EnumFormatMetrics>()
    );
    assert_eq!(std::mem::size_of::<FormatSyntax>(), 1);
}
// Append to frontend/format.rs, or keep this module in a test-only included file.
#[cfg(test)]
mod lexer_reservation_caller_tests {
    use super::*;
    use crate::frontend::source::{SourceFileId, Span};
    use std::mem::size_of;

    // Source-derived, before executing the candidate:
    // Input: four independently retained block-comment trivia, then EOF.
    // Candidate: four comments, three space trivia, one newline trivia, then EOF.
    // Neither parser enters its item loop, so there are no parser reservations.
    const INPUT: &str = "/**//**//**//**/";
    const OUTPUT: &str = "/**/ /**/ /**/ /**/\n";

    fn source() -> SourceMap {
        let mut sources = SourceMap::new();
        // Make first-parse origins distinguishable from candidate file 0.
        sources.add("unrelated.ox".into(), String::new());
        sources.add("retained-format.ox".into(), INPUT.into());
        sources
    }

    fn schedule(syntax: FormatSyntax) -> [(&'static str, usize, usize); 9] {
        let (line_kind, file_kind) = match syntax {
            FormatSyntax::Closed => ("line starts", "source files"),
            _ => ("formatted source line starts", "formatted source files"),
        };
        [
            ("lexer token tape", 4, size_of::<Token>()),
            ("lexer token tape", 8, size_of::<Token>()),
            ("formatter token roles", INPUT.len(), 1),
            ("formatted source", OUTPUT.len(), 1),
            (line_kind, 2, size_of::<usize>()),
            (file_kind, 1, size_of::<SourceFile>()),
            ("lexer token tape", 4, size_of::<Token>()),
            ("lexer token tape", 8, size_of::<Token>()),
            ("lexer token tape", 16, size_of::<Token>()),
        ]
    }

    fn assert_prefix(
        allocator: &Allocator,
        syntax: FormatSyntax,
        count: usize,
        last_success: bool,
    ) {
        let expected = schedule(syntax);
        assert_eq!(allocator.attempts, count);
        assert_eq!(allocator.trace.len(), count);
        assert!(!allocator.observer_trace_overflow);
        for (index, (event, &(kind, length, width))) in
            allocator.trace.iter().zip(&expected).enumerate()
        {
            assert_eq!(
                (event.kind, event.length, event.element_bytes),
                (kind, length, width)
            );
            assert_eq!(event.success, index + 1 != count || last_success);
        }
    }

    fn assert_error(errors: &[Diagnostic], stage: &str, message: &str, primary: Option<Span>) {
        assert_eq!(errors.len(), 1, "{errors:?}");
        let error = &errors[0];
        assert_eq!(error.code, "E0400");
        assert_eq!(error.stage, stage);
        assert_eq!(error.message, message);
        assert_eq!(error.primary, primary);
        assert!(error.secondary.is_empty());
        assert!(error.notes.is_empty());
    }

    #[test]
    fn lexer_caller_formatter_all_policies_obey_independent_phase_schedule() {
        for syntax in [
            FormatSyntax::Closed,
            FormatSyntax::Enabled,
            FormatSyntax::EnumCandidate,
        ] {
            let sources = source();
            let file = sources.get(SourceFileId(1));
            let identity = file.identity();
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(9).unwrap();
            let (result, metrics) =
                format_with_syntax(file, &mut allocator, Limits::DEFAULT, syntax);
            assert_eq!(result.unwrap(), OUTPUT);
            assert_prefix(&allocator, syntax, 9, true);
            assert_eq!(file.text(), INPUT);
            assert_eq!(file.path(), "retained-format.ox");
            assert_eq!(file.identity(), identity);
            if syntax.candidate() {
                assert_eq!(metrics.parse_calls, 2);
                assert_eq!(metrics.first_token_heap, 8 * size_of::<Token>());
                assert_eq!(metrics.second_token_heap, 16 * size_of::<Token>());
                assert_eq!(metrics.roles_heap, INPUT.len());
                assert_eq!(metrics.output_heap, OUTPUT.len());
            }
        }
    }

    #[test]
    fn lexer_caller_formatter_first_parse_initial_and_eof_failure_keep_lex_origin() {
        // Fixed predicted request ordinals, not learned from a successful trace.
        for syntax in [
            FormatSyntax::Closed,
            FormatSyntax::Enabled,
            FormatSyntax::EnumCandidate,
        ] {
            for (ordinal, start, end) in [(1, 0, 4), (2, 16, 16)] {
                let sources = source();
                let file = sources.get(SourceFileId(1));
                let identity = file.identity();
                let mut allocator = Allocator {
                    fail_at: Some(ordinal),
                    ..Allocator::default()
                };
                allocator.observer_trace_bound(9).unwrap();
                let (result, metrics) =
                    format_with_syntax(file, &mut allocator, Limits::DEFAULT, syntax);
                assert_error(
                    &result.unwrap_err(),
                    "lex",
                    "token storage allocation failed",
                    Some(file.span(start, end)),
                );
                assert_prefix(&allocator, syntax, ordinal, false);
                // No subsequent work/emit/source-owner request follows the lexer refusal.
                assert!(allocator
                    .trace
                    .iter()
                    .all(|event| event.kind == "lexer token tape"));
                assert_eq!(metrics.first_ast_heap, 0);
                assert_eq!(metrics.first_token_heap, 0);
                assert_eq!(metrics.roles_heap, 0);
                assert_eq!(metrics.output_heap, 0);
                assert_eq!(metrics.source_owner_heap, 0);
                assert_eq!(metrics.second_ast_heap, 0);
                assert_eq!(metrics.second_token_heap, 0);
                if syntax.candidate() {
                    // Counts parse attempts, not proof of downstream parser entry.
                    assert_eq!(metrics.parse_calls, 1);
                }
                assert_eq!(file.text(), INPUT);
                assert_eq!(file.path(), "retained-format.ox");
                assert_eq!(file.identity(), identity);
            }
        }
    }

    #[test]
    fn lexer_caller_formatter_candidate_initial_growth_and_eof_failure_keep_adapter() {
        // Source-owner requests finish at six; candidate requests are seven,
        // eight (third comment at 10..14), and nine (EOF at 20..20).
        for syntax in [
            FormatSyntax::Closed,
            FormatSyntax::Enabled,
            FormatSyntax::EnumCandidate,
        ] {
            for ordinal in [7, 8, 9] {
                let sources = source();
                let file = sources.get(SourceFileId(1));
                let identity = file.identity();
                let mut allocator = Allocator {
                    fail_at: Some(ordinal),
                    ..Allocator::default()
                };
                allocator.observer_trace_bound(9).unwrap();
                let (result, metrics) =
                    format_with_syntax(file, &mut allocator, Limits::DEFAULT, syntax);
                assert_error(
                    &result.unwrap_err(),
                    "format",
                    "formatted source exceeds lexer or parser resource limits",
                    None,
                );
                assert_prefix(&allocator, syntax, ordinal, false);
                assert_eq!(metrics.second_ast_heap, 0);
                assert_eq!(metrics.second_token_heap, 0);
                if syntax.candidate() {
                    // Earlier first parse and candidate construction remain accounted.
                    assert_eq!(metrics.parse_calls, 2);
                    assert_eq!(metrics.first_token_heap, 8 * size_of::<Token>());
                    assert_eq!(metrics.roles_heap, INPUT.len());
                    assert_eq!(metrics.output_heap, OUTPUT.len());
                    assert_eq!(
                        metrics.source_owner_heap,
                        OUTPUT.len() + 2 * size_of::<usize>() + size_of::<SourceFile>()
                    );
                }
                assert_eq!(file.text(), INPUT);
                assert_eq!(file.path(), "retained-format.ox");
                assert_eq!(file.identity(), identity);
            }
        }
    }
    // Append INSIDE format::lexer_reservation_caller_tests before its closing brace.
    mod actual_null_callers {
        use super::*;
        use crate::frontend::project::budget::{real_null_observer as null, ReserveEvent};
        use null::growth::{self, GrowthTarget};
        use std::alloc::Layout;
        use std::mem::{align_of, align_of_val, offset_of, size_of_val};

        #[derive(Clone, Copy, Debug)]
        struct Facts {
            failed: bool,
            diagnostic: bool,
            metrics: bool,
            source: bool,
            trace: bool,
        }
        fn action(
            source: &SourceFile,
            syntax: FormatSyntax,
            ordinal: usize,
        ) -> impl for<'a> FnOnce(&'a mut Allocator) -> Facts + '_ {
            move |allocator| {
                let identity = source.identity();
                let (result, metrics) =
                    format_with_syntax(source, allocator, Limits::DEFAULT, syntax);
                let first = ordinal <= 2;
                let diagnostic = result.as_ref().err().is_some_and(|errors| {
                    let Some(error) = errors.first() else {
                        return false;
                    };
                    errors.len() == 1
                        && error.code == "E0400"
                        && error.secondary.is_empty()
                        && error.notes.is_empty()
                        && if first {
                            error.stage == "lex"
                                && error.message == "token storage allocation failed"
                                && error.primary
                                    == Some(if ordinal == 1 {
                                        source.span(0, 4)
                                    } else {
                                        source.span(16, 16)
                                    })
                        } else {
                            error.stage == "format"
                                && error.message
                                    == "formatted source exceeds lexer or parser resource limits"
                                && error.primary.is_none()
                        }
                });
                let expected = schedule(syntax);
                let trace = allocator.attempts == ordinal
                    && allocator.trace.len() == ordinal
                    && !allocator.observer_trace_overflow
                    && allocator.trace.iter().zip(&expected).enumerate().all(
                        |(index, (event, &(kind, length, width)))| {
                            (event.kind, event.length, event.element_bytes) == (kind, length, width)
                                && event.success == (index + 1 != ordinal)
                        },
                    );
                let metrics_ok = metrics.second_ast_heap == 0
                    && metrics.second_token_heap == 0
                    && if first {
                        metrics.first_ast_heap == 0
                            && metrics.first_token_heap == 0
                            && metrics.roles_heap == 0
                            && metrics.output_heap == 0
                            && metrics.source_owner_heap == 0
                            && (!syntax.candidate() || metrics.parse_calls == 1)
                    } else {
                        !syntax.candidate()
                            || (metrics.parse_calls == 2
                                && metrics.first_token_heap == 8 * size_of::<Token>()
                                && metrics.roles_heap == INPUT.len()
                                && metrics.output_heap == OUTPUT.len()
                                && metrics.source_owner_heap
                                    == OUTPUT.len()
                                        + 2 * size_of::<usize>()
                                        + size_of::<SourceFile>())
                    };
                let facts = Facts {
                    failed: result.is_err(),
                    diagnostic,
                    metrics: metrics_ok,
                    source: source.text() == INPUT
                        && source.path() == "retained-format.ox"
                        && source.identity() == identity,
                    trace,
                };
                // Errors, formatter output/AST owners and the failed lexical tape
                // are destroyed before these fixed primitive facts escape selection.
                drop(result);
                facts
            }
        }
        fn target(attempt: usize, old: usize, new: usize) -> GrowthTarget {
            GrowthTarget {
                attempt,
                kind: "lexer token tape",
                old_len: old,
                old_capacity: old,
                additional: new - old,
                new_slots: new,
                element_bytes: size_of::<Token>(),
                element_align: align_of::<Token>(),
                old_layout: Layout::array::<Token>(old).unwrap(),
                new_layout: Layout::array::<Token>(new).unwrap(),
                operation: null::Operation::Realloc,
            }
        }
        #[test]
        fn lexer_caller_actual_null_formatter_all_policies_and_five_sites() {
            for syntax in [
                FormatSyntax::Closed,
                FormatSyntax::Enabled,
                FormatSyntax::EnumCandidate,
            ] {
                // Frozen independently: initial/EOF first parse, initial/third
                // comment/EOF reparse. No successful trace feeds selection.
                for (ordinal, old, new) in [(1, 0, 4), (2, 4, 8), (7, 0, 4), (8, 4, 8), (9, 8, 16)]
                {
                    let sources = source();
                    let file = sources.get(SourceFileId(1));
                    let mut allocator = Allocator::default();
                    allocator.observer_trace_bound(9).unwrap();
                    let capacity = allocator.trace.capacity();
                    let action = action(file, syntax, ordinal);
                    let facts = if old == 0 {
                        let target = null::Target {
                            attempt: ordinal,
                            kind: "lexer token tape",
                            slots: new,
                            element_bytes: size_of::<Token>(),
                            layout: Layout::array::<Token>(new).unwrap(),
                        };
                        let (facts, report) =
                            null::with_selected(&mut allocator, target, action).unwrap();
                        assert_eq!(report.target, target);
                        assert!(report.selected && report.matched && report.fired);
                        assert_eq!(report.rejection, None);
                        assert_eq!(
                            report.actual,
                            Some(null::GlobalEvent {
                                operation: null::Operation::Alloc,
                                layout: target.layout,
                                new_size: None
                            })
                        );
                        facts
                    } else {
                        let target = target(ordinal, old, new);
                        let (facts, report) =
                            growth::with_selected_growth(&mut allocator, target, action).unwrap();
                        assert_eq!(report.target, target);
                        assert!(report.selected && report.matched && report.fired);
                        assert_eq!(report.rejection, None);
                        assert_eq!(
                            report.actual,
                            Some(growth::GrowthEvent {
                                operation: null::Operation::Realloc,
                                layout: target.old_layout,
                                new_size: Some(target.new_layout.size()),
                                old_address_matches: true
                            })
                        );
                        assert_eq!(report.reserve_failed, Some(true));
                        assert!(
                            report.owner_unchanged
                                && report.address_unchanged
                                && report.length_unchanged
                                && report.capacity_unchanged
                        );
                        assert_eq!(report.drop_count, 1);
                        assert_eq!(
                            report.drop_event,
                            Some(growth::GrowthDropEvent {
                                layout: target.old_layout,
                                old_address_matches: true,
                                after_reserve_return: true
                            })
                        );
                        assert!(report.trace_preserved);
                        facts
                    };
                    assert!(
                        facts.failed
                            && facts.diagnostic
                            && facts.metrics
                            && facts.source
                            && facts.trace,
                        "{facts:?}"
                    );
                    assert_eq!(allocator.trace.capacity(), capacity);
                    assert_eq!(allocator.attempts, ordinal);
                    assert_eq!(allocator.trace.len(), ordinal);
                }
            }
        }
        #[allow(dead_code)]
        struct FormatCarriers<'a> {
            sources: SourceMap,
            source: &'a SourceFile,
            allocator: Allocator,
            syntax: FormatSyntax,
            ordinal: usize,
            source_identity: u64,
            callee: (Result<String, Vec<Diagnostic>>, EnumFormatMetrics),
            result: Result<String, Vec<Diagnostic>>,
            metrics: EnumFormatMetrics,
            diagnostic_borrow: &'a [Diagnostic],
            expected: [(&'static str, usize, usize); 9],
            first: bool,
            diagnostic: bool,
            trace: bool,
            metrics_ok: bool,
            facts: Facts,
            trace_capacity: usize,
            trace_requested: usize,
            trace_retained: usize,
        }
        #[test]
        fn lexer_caller_formatter_null_layout_measurement_only() {
            // Same actual closure/output type; no action invocation or selection.
            let sources = source();
            let file = sources.get(SourceFileId(1));
            let action = action(file, FormatSyntax::EnumCandidate, 9);
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(9).unwrap();
            println!("caller-null-bank formatter carriers={} align={} closure={} facts={} fresh-selection={} growth-selection={} trace-requested={} trace-retained={} retained-input-heap={} metrics={} observer-fixed={}",
            size_of::<FormatCarriers<'_>>(), align_of::<FormatCarriers<'_>>(), size_of_val(&action), size_of::<Facts>(),
            null::selection_carriers_bytes(&action), growth::selection_carriers_bytes(&action),
            9 * size_of::<ReserveEvent>(), allocator.trace.capacity() * size_of::<ReserveEvent>(),
            sources.heap_capacity_bytes().unwrap(),
            size_of::<EnumFormatMetrics>(), growth::fixed_carriers_bytes::<Token>());
            macro_rules! fields {
            ($ty:ty; $($field:ident),+ $(,)?) => {
                $(println!("caller-null-offset {}.{}={}", stringify!($ty), stringify!($field), offset_of!($ty, $field));)+
            };
        }
            fields!(FormatCarriers<'_>; sources, source, allocator, syntax, ordinal, source_identity,
            callee, result, metrics, diagnostic_borrow, expected, first, diagnostic, trace, metrics_ok,
            facts, trace_capacity, trace_requested, trace_retained);
            fields!(Facts; failed, diagnostic, metrics, source, trace);
            let fresh = null::selection_carriers_bytes(&action);
            let growth = growth::selection_carriers_bytes(&action);
            let exclusive = size_of::<FormatCarriers<'_>>()
                .checked_add(fresh.max(growth))
                .and_then(|v| v.checked_add(growth::fixed_carriers_bytes::<Token>()))
                .and_then(|v| v.checked_add(lexer::reservation_scratch_bytes()))
                .and_then(|v| v.checked_add(lexer::reservation_observer_bytes()))
                .unwrap();
            println!("caller-null-sum formatter exclusive-controller={} conservative-both-transports={} closure-align={} facts-align={}",
            exclusive, exclusive.checked_add(fresh.min(growth)).unwrap(), align_of_val(&action), align_of::<Facts>());
            assert_eq!(allocator.attempts, 0);
            assert!(allocator.trace.is_empty());
        }
    }
}
