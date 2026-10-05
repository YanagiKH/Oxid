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
    if source.text().len() > MAX_SOURCE_BYTES {
        return Err(resource("source byte limit exceeded"));
    }
    let program = parse(source, allocator)?;
    let roles = roles(source, &program, allocator, limits.work_bytes)?;
    let mut length = 0usize;
    layout(source, &program, &roles, limits.delimiters, |part| {
        length = output_length(length, part.len())?;
        Ok(())
    })?;
    let mut output = String::new();
    allocator
        .string(&mut output, length, "formatted source")
        .map_err(|_| resource("formatted source allocation failed"))?;
    layout(source, &program, &roles, limits.delimiters, |part| {
        output.push_str(part);
        Ok(())
    })?;
    debug_assert_eq!(output.len(), length);
    drop(roles);

    // Move, rather than clone, the candidate into its immutable source owner.
    let mut candidates = SourceMap::new();
    let id = candidates
        .try_add(String::new(), output, allocator)
        .map_err(|_| resource("formatted source allocation failed"))?;
    let candidate = candidates.get(id);
    let parsed = parse(candidate, allocator).map_err(|errors| {
        if errors.iter().any(|error| error.code == "E0400") {
            resource("formatted source exceeds lexer or parser resource limits")
        } else {
            invariant("formatted source no longer parses")
        }
    })?;
    if !same_projection(source, &program.tokens, candidate, &parsed.tokens) {
        return Err(invariant(
            "formatted source changed tokens, comments or line breaks",
        ));
    }
    Ok(candidates.into_single_text())
}

fn parse(source: &SourceFile, allocator: &mut Allocator) -> Result<Program, Vec<Diagnostic>> {
    let tokens = lexer::lex_with_limit(source, lexer::MAX_TOKENS).map_err(|error| vec![*error])?;
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
fn roles(
    source: &SourceFile,
    program: &Program,
    allocator: &mut Allocator,
    work_limit: usize,
) -> Result<Vec<u8>, Vec<Diagnostic>> {
    if !program.belongs_to(source) {
        return Err(invariant("formatter source association mismatch"));
    }
    let length = source.text().len();
    if length > work_limit.min(MAX_WORK_BYTES) {
        return Err(resource("formatter work table limit exceeded"));
    }
    let mut roles = Vec::new();
    allocator
        .vector(&mut roles, length, "formatter token roles")
        .map_err(|_| resource("formatter work table allocation failed"))?;
    roles.resize(length, 0);
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
            ExprKind::Call { args, .. } => {
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
}

#[cfg(test)]
#[path = "format/resource_tests.rs"]
mod resource_tests;

#[cfg(test)]
#[path = "format/ast_tests.rs"]
mod ast_tests;
