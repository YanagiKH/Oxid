//! A documentation convention over the retained token tape, not a lexer/parser.
use super::super::{ast, lexer::Kind, project::ProjectSources, source::Span};
use super::render::{invalid, Text};
use std::io::{self, Write};

fn line_start(text: &str, end: usize) -> usize {
    text[..end].rfind('\n').map_or(0, |offset| offset + 1)
}

/// An exact standalone line-comment token is required. Looking at the line
/// prefix alone would mistake text within a block comment for documentation.
fn doc_line<'a>(text: &'a str, ast: &ast::Program, start: usize, end: usize) -> Option<&'a str> {
    let raw = text.get(start..end)?;
    let line = raw.strip_suffix('\r').unwrap_or(raw);
    let content = line.trim_start_matches([' ', '\t']);
    let prefix = content.strip_prefix("///")?;
    if prefix.starts_with('/') {
        return None;
    }
    let token_start = start + line.len() - content.len();
    let index = ast
        .tokens
        .binary_search_by_key(&token_start, |token| token.span.start)
        .ok()?;
    let token = ast.tokens.get(index)?;
    if token.kind != Kind::Trivia || token.span.end != end {
        return None;
    }
    Some(prefix.strip_prefix(' ').unwrap_or(prefix))
}

pub(super) fn write(
    project: &ProjectSources,
    ast: &ast::Program,
    anchor: Span,
    output: &mut impl Write,
) -> io::Result<()> {
    let source = project.sources().get(anchor.file);
    let text = source.text();
    if project.try_text(anchor).is_none() {
        return Err(invalid());
    }
    let declaration_start = line_start(text, anchor.start);
    if !text[declaration_start..anchor.start]
        .bytes()
        .all(|b| b == b' ' || b == b'\t')
    {
        return Ok(());
    }
    let mut first = declaration_start;
    while first > 0 {
        let end = first - 1; // Exclude LF, retain optional CR for token equality.
        let start = line_start(text, end);
        if doc_line(text, ast, start, end).is_none() {
            break;
        }
        first = start;
    }
    if first == declaration_start {
        return Ok(());
    }
    let mut start = first;
    while start < declaration_start {
        let end = text[start..declaration_start]
            .find('\n')
            .map(|relative| start + relative)
            .ok_or_else(invalid)?;
        let content = doc_line(text, ast, start, end).ok_or_else(invalid)?;
        writeln!(output, "> {}", Text(content))?;
        start = end + 1;
    }
    writeln!(output)
}
