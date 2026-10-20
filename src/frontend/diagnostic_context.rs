//! Optional human source excerpts for ordinary compile diagnostics.
//!
//! This is an additive presentation layer: it never changes `Diagnostic`,
//! its human or JSON formats, or source spans. The original
//! `Diagnostic::render_human` text is emitted first, byte for byte, and the
//! excerpts are appended after it. It is not intended for runtime or bounded
//! native diagnostics, which keep their own streaming cost model.
//!
//! Columns count one display cell per Unicode scalar (matching
//! `SourceFile::location`), except that a tab expands to four cells and an
//! escaped control character uses its escaped width. Full terminal width
//! layout (wide or combining characters) is intentionally out of scope.

use super::diagnostic::Diagnostic;
use super::source::{SourceFile, SourceMap, Span};

/// Displayed cells per source line before the excerpt is truncated.
const MAX_LINE_CELLS: usize = 100;
const ELLIPSIS: &str = "...";
const TAB: &str = "    ";

/// `render_human` output followed by a readable excerpt for each label.
/// Diagnostics without primary or secondary spans render unchanged.
pub(super) fn render_human_context(diagnostic: &Diagnostic, sources: &SourceMap) -> String {
    let mut result = diagnostic.render_human(sources);
    let labels = diagnostic
        .primary
        .map(|span| (span, '^', "primary", None))
        .into_iter()
        .chain(
            diagnostic
                .secondary
                .iter()
                .map(|(span, message)| (*span, '-', "secondary", Some(message.as_str()))),
        );
    for (span, mark, kind, message) in labels {
        if sources.is_valid_span(span) {
            write_excerpt(
                &mut result,
                sources.get(span.file),
                span,
                mark,
                kind,
                message,
            );
        }
    }
    result
}

enum Row {
    Source {
        number: usize,
        text: String,
        marks: String,
    },
    Elided,
}

fn write_excerpt(
    out: &mut String,
    source: &SourceFile,
    span: Span,
    mark: char,
    kind: &str,
    message: Option<&str>,
) {
    let text = source.text();
    let (start_line, column) = source.location(span.start);
    let start_bounds = line_bounds(text, span.start);
    // A non-empty span ending right after a line break ends on that line.
    let end_probe = if span.end > span.start && span.end == line_bounds(text, span.end).start {
        span.end - 1
    } else {
        span.end
    };
    let end_bounds = line_bounds(text, end_probe);
    let mut rows = Vec::new();
    if end_bounds.start == start_bounds.start {
        let (text_line, marks) =
            render_line(text, start_bounds, span.start, span.end, mark, Focus::First);
        rows.push(Row::Source {
            number: start_line,
            text: text_line,
            marks,
        });
    } else {
        let end_line = source.location(end_bounds.start).0;
        let (first, first_marks) = render_line(
            text,
            start_bounds,
            span.start,
            start_bounds.next,
            mark,
            Focus::First,
        );
        rows.push(Row::Source {
            number: start_line,
            text: first,
            marks: first_marks,
        });
        if end_line > start_line + 1 {
            rows.push(Row::Elided);
        }
        let (last, last_marks) = render_line(
            text,
            end_bounds,
            end_bounds.start,
            span.end,
            mark,
            Focus::Last,
        );
        rows.push(Row::Source {
            number: end_line,
            text: last,
            marks: last_marks,
        });
    }
    let width = rows
        .iter()
        .filter_map(|row| match row {
            Row::Source { number, .. } => Some(number.to_string().len()),
            Row::Elided => None,
        })
        .max()
        .unwrap_or(1);
    let pad = " ".repeat(width);

    out.push_str("  = ");
    out.push_str(kind);
    out.push_str(": ");
    push_escaped(out, source.path());
    out.push_str(&format!(":{start_line}:{column}\n"));
    out.push_str(&format!("  {pad} |\n"));
    let last_source = rows
        .iter()
        .rposition(|row| matches!(row, Row::Source { .. }))
        .expect("an excerpt has at least one source row");
    for (index, row) in rows.iter().enumerate() {
        match row {
            Row::Elided => out.push_str(&format!("  {pad} | {ELLIPSIS}\n")),
            Row::Source {
                number,
                text,
                marks,
            } => {
                if text.is_empty() {
                    out.push_str(&format!("  {number:>width$} |\n"));
                } else {
                    out.push_str(&format!("  {number:>width$} | {text}\n"));
                }
                out.push_str(&format!("  {pad} | {marks}"));
                if index == last_source {
                    if let Some(message) = message {
                        out.push(' ');
                        push_escaped(out, message);
                    }
                }
                out.push('\n');
            }
        }
    }
}

#[derive(Clone, Copy)]
struct LineBounds {
    /// First byte of the line.
    start: usize,
    /// End of displayed content, excluding LF and a CR directly before it.
    content_end: usize,
    /// First byte of the next line, or EOF.
    next: usize,
}

fn line_bounds(text: &str, offset: usize) -> LineBounds {
    let start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let (line_end, next) = match text[offset..].find('\n') {
        Some(index) => (offset + index, offset + index + 1),
        None => (text.len(), text.len()),
    };
    let content_end = if next > line_end && text[start..line_end].ends_with('\r') {
        line_end - 1
    } else {
        line_end
    };
    LineBounds {
        start,
        content_end: content_end.max(start),
        next,
    }
}

/// Which mark a truncated line keeps in view: a span's start or its end.
#[derive(Clone, Copy)]
enum Focus {
    First,
    Last,
}

struct Cell {
    display: String,
    width: usize,
    marked: bool,
}

/// Render one line's display text and its mark row for bytes `[from, to)`.
/// A zero-width range gets one mark at its displayed start. A range covering
/// only the line break or EOF gets one mark just after the displayed content.
fn render_line(
    text: &str,
    bounds: LineBounds,
    from: usize,
    to: usize,
    mark: char,
    focus: Focus,
) -> (String, String) {
    let mut cells: Vec<Cell> = text[bounds.start..bounds.content_end]
        .char_indices()
        .map(|(index, ch)| {
            let offset = bounds.start + index;
            let end = offset + ch.len_utf8();
            let marked = if from == to {
                offset == from
            } else {
                offset < to && end > from
            };
            let display = display_char(ch);
            Cell {
                width: display.chars().count(),
                display,
                marked,
            }
        })
        .collect();
    if !cells.iter().any(|cell| cell.marked) {
        cells.push(Cell {
            display: String::new(),
            width: 1,
            marked: true,
        });
    }
    let (first, last, left_cut, right_cut) = window(&cells, focus);
    let mut display = String::new();
    let mut marks = String::new();
    let mut push = |piece: &str, width: usize, marked: bool| {
        display.push_str(piece);
        marks.extend((0..width).map(|cell| {
            if marked && (from != to || cell == 0) {
                mark
            } else {
                ' '
            }
        }));
    };
    if left_cut {
        push(
            ELLIPSIS,
            ELLIPSIS.len(),
            cells[..first].iter().any(|cell| cell.marked),
        );
    }
    for cell in &cells[first..last] {
        push(&cell.display, cell.width, cell.marked);
    }
    if right_cut {
        push(
            ELLIPSIS,
            ELLIPSIS.len(),
            cells[last..].iter().any(|cell| cell.marked),
        );
    }
    (
        display.trim_end_matches(' ').to_owned(),
        marks.trim_end().to_owned(),
    )
}

/// Choose a cell window of at most `MAX_LINE_CELLS`, ellipses included,
/// that keeps the focused mark visible with some surrounding context.
fn window(cells: &[Cell], focus: Focus) -> (usize, usize, bool, bool) {
    let total: usize = cells.iter().map(|cell| cell.width).sum();
    if total <= MAX_LINE_CELLS {
        return (0, cells.len(), false, false);
    }
    let budget = MAX_LINE_CELLS - 2 * ELLIPSIS.len();
    let marked = |cell: &Cell| cell.marked;
    let (focus, lead) = match focus {
        Focus::First => (cells.iter().position(marked), budget / 4),
        Focus::Last => (cells.iter().rposition(marked), budget - budget / 4),
    };
    let focus = focus.expect("a rendered line has at least one mark");
    let before: usize = cells[..focus].iter().map(|cell| cell.width).sum();
    let mut first = 0;
    let mut skipped = 0;
    while first < focus && before - skipped > lead {
        skipped += cells[first].width;
        first += 1;
    }
    let mut last = first;
    let mut used = 0;
    while last < cells.len() && (last <= focus || used + cells[last].width <= budget) {
        used += cells[last].width;
        last += 1;
    }
    (first, last, first > 0, last < cells.len())
}

/// Characters that must never reach a terminal raw: C0/C1 controls and
/// bidirectional formatting controls that can reorder displayed source.
fn is_unsafe(ch: char) -> bool {
    ch.is_control()
        || matches!(ch, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

fn display_char(ch: char) -> String {
    if ch == '\t' {
        TAB.to_owned()
    } else if ch.is_control() {
        ch.escape_default().collect()
    } else if is_unsafe(ch) {
        ch.escape_unicode().collect()
    } else {
        ch.to_string()
    }
}

fn push_escaped(out: &mut String, text: &str) {
    for ch in text.chars() {
        if ch.is_control() {
            out.extend(ch.escape_default());
        } else if is_unsafe(ch) {
            out.extend(ch.escape_unicode());
        } else {
            out.push(ch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(path: &str, text: &str) -> (SourceMap, super::super::source::SourceFileId) {
        let mut sources = SourceMap::new();
        let id = sources.add(path.into(), text.into());
        (sources, id)
    }

    fn primary(
        sources: &SourceMap,
        id: super::super::source::SourceFileId,
        start: usize,
        end: usize,
    ) -> Box<Diagnostic> {
        Diagnostic::new(
            "E1001",
            "resolve",
            "unknown",
            Some(sources.get(id).span(start, end)),
        )
    }

    /// The excerpt after the unchanged `render_human` prefix.
    fn excerpt(diagnostic: &Diagnostic, sources: &SourceMap) -> String {
        let original = diagnostic.render_human(sources);
        let rendered = render_human_context(diagnostic, sources);
        assert!(
            rendered.starts_with(&original),
            "original output must be a prefix"
        );
        rendered[original.len()..].to_owned()
    }

    #[test]
    fn single_line_span_keeps_original_output_then_marks_columns() {
        let (sources, id) = one("main.ox", "let x = foo;\n");
        let diagnostic = primary(&sources, id, 8, 11);
        assert_eq!(
            render_human_context(&diagnostic, &sources),
            concat!(
                "error[E1001] (resolve): unknown\n",
                "  --> main.ox:1:9\n",
                "  = primary: main.ox:1:9\n",
                "    |\n",
                "  1 | let x = foo;\n",
                "    |         ^^^\n",
            )
        );
    }

    #[test]
    fn sourceless_diagnostics_render_exactly_as_before() {
        let mut diagnostic = Diagnostic::new("E0002", "io", "bad\n\u{1b}", None);
        diagnostic.notes.push("note".into());
        let sources = SourceMap::new();
        assert_eq!(
            render_human_context(&diagnostic, &sources),
            diagnostic.render_human(&sources)
        );
    }

    #[test]
    fn multi_line_spans_show_first_and_last_lines_and_elide_the_middle() {
        let (sources, id) = one("m.ox", "fn f() {\n  a\n  b\n}\n");
        assert_eq!(
            excerpt(&primary(&sources, id, 3, 13), &sources),
            concat!(
                "  = primary: m.ox:1:4\n",
                "    |\n",
                "  1 | fn f() {\n",
                "    |    ^^^^^\n",
                "  2 |   a\n",
                "    | ^^^\n",
            )
        );
        assert_eq!(
            excerpt(&primary(&sources, id, 7, 19), &sources),
            concat!(
                "  = primary: m.ox:1:8\n",
                "    |\n",
                "  1 | fn f() {\n",
                "    |        ^\n",
                "    | ...\n",
                "  4 | }\n",
                "    | ^\n",
            )
        );
    }

    #[test]
    fn line_number_gutter_is_aligned_across_rows() {
        let text = "x\n".repeat(9) + "abc\n";
        let (sources, id) = one("g.ox", &text);
        assert_eq!(
            excerpt(&primary(&sources, id, 16, 20), &sources),
            concat!(
                "  = primary: g.ox:9:1\n",
                "     |\n",
                "   9 | x\n",
                "     | ^\n",
                "  10 | abc\n",
                "     | ^^\n",
            )
        );
    }

    #[test]
    fn zero_width_and_eof_spans_get_one_caret() {
        let (sources, id) = one("z.ox", "ab\n");
        assert_eq!(
            excerpt(&primary(&sources, id, 1, 1), &sources),
            "  = primary: z.ox:1:2\n    |\n  1 | ab\n    |  ^\n"
        );
        assert_eq!(
            excerpt(&primary(&sources, id, 2, 2), &sources),
            "  = primary: z.ox:1:3\n    |\n  1 | ab\n    |   ^\n"
        );
        assert_eq!(
            excerpt(&primary(&sources, id, 3, 3), &sources),
            "  = primary: z.ox:2:1\n    |\n  2 |\n    | ^\n"
        );
        let (empty, empty_id) = one("e.ox", "");
        assert_eq!(
            excerpt(&primary(&empty, empty_id, 0, 0), &empty),
            "  = primary: e.ox:1:1\n    |\n  1 |\n    | ^\n"
        );
        let (no_newline, eof_id) = one("n.ox", "ab");
        assert_eq!(
            excerpt(&primary(&no_newline, eof_id, 0, 2), &no_newline),
            "  = primary: n.ox:1:1\n    |\n  1 | ab\n    | ^^\n"
        );
    }

    #[test]
    fn zero_width_spans_at_expanded_characters_get_one_caret() {
        let (sources, id) = one("z.ox", "a\t\u{1b}b\n");
        assert_eq!(
            excerpt(&primary(&sources, id, 1, 1), &sources),
            "  = primary: z.ox:1:2\n    |\n  1 | a    \\u{1b}b\n    |  ^\n"
        );
        assert_eq!(
            excerpt(&primary(&sources, id, 2, 2), &sources),
            "  = primary: z.ox:1:3\n    |\n  1 | a    \\u{1b}b\n    |      ^\n"
        );
    }

    #[test]
    fn utf8_scalars_and_tabs_keep_caret_alignment() {
        let (sources, id) = one("u.ox", "é🦀\tx = 1\n");
        // 'x' starts at byte 2 + 4 + 1 = 7; display "é🦀" + four-cell tab.
        assert_eq!(
            excerpt(&primary(&sources, id, 7, 8), &sources),
            "  = primary: u.ox:1:4\n    |\n  1 | é🦀    x = 1\n    |       ^\n"
        );
        assert_eq!(
            excerpt(&primary(&sources, id, 2, 6), &sources),
            "  = primary: u.ox:1:2\n    |\n  1 | é🦀    x = 1\n    |  ^\n"
        );
        assert_eq!(
            excerpt(&primary(&sources, id, 6, 7), &sources),
            "  = primary: u.ox:1:3\n    |\n  1 | é🦀    x = 1\n    |   ^^^^\n"
        );
    }

    #[test]
    fn crlf_is_not_displayed_and_line_break_spans_mark_end_of_line() {
        let (sources, id) = one("c.ox", "ab\r\ncd\r\n");
        assert_eq!(
            excerpt(&primary(&sources, id, 0, 2), &sources),
            "  = primary: c.ox:1:1\n    |\n  1 | ab\n    | ^^\n"
        );
        assert_eq!(
            excerpt(&primary(&sources, id, 2, 4), &sources),
            "  = primary: c.ox:1:3\n    |\n  1 | ab\n    |   ^\n"
        );
        assert_eq!(
            excerpt(&primary(&sources, id, 1, 5), &sources),
            "  = primary: c.ox:1:2\n    |\n  1 | ab\n    |  ^\n  2 | cd\n    | ^\n"
        );
        assert_eq!(
            excerpt(&primary(&sources, id, 4, 8), &sources),
            "  = primary: c.ox:2:1\n    |\n  2 | cd\n    | ^^\n"
        );
        let (lone_cr, cr_id) = one("r.ox", "a\rb\n");
        assert_eq!(
            excerpt(&primary(&lone_cr, cr_id, 2, 3), &lone_cr),
            "  = primary: r.ox:1:3\n    |\n  1 | a\\rb\n    |    ^\n"
        );
    }

    #[test]
    fn long_lines_are_truncated_with_aligned_carets() {
        let text = format!("{}bad{}\n", "a".repeat(300), "b".repeat(300));
        let (sources, id) = one("l.ox", &text);
        let rendered = excerpt(&primary(&sources, id, 300, 303), &sources);
        let lines: Vec<&str> = rendered.lines().collect();
        assert_eq!(lines[0], "  = primary: l.ox:1:301");
        let source_line = lines[2].strip_prefix("  1 | ").unwrap();
        let marks = lines[3].strip_prefix("    | ").unwrap();
        assert!(source_line.starts_with(ELLIPSIS) && source_line.ends_with(ELLIPSIS));
        assert!(source_line.chars().count() <= MAX_LINE_CELLS);
        let at = source_line.find("bad").unwrap();
        assert_eq!(marks, format!("{}^^^", " ".repeat(at)));

        let tail = format!("{}end\n", "a".repeat(300));
        let (sources, id) = one("t.ox", &tail);
        let rendered = excerpt(&primary(&sources, id, 303, 303), &sources);
        let lines: Vec<&str> = rendered.lines().collect();
        let source_line = lines[2].strip_prefix("  1 | ").unwrap();
        assert!(source_line.starts_with(ELLIPSIS) && source_line.ends_with("end"));
        assert_eq!(
            lines[3].strip_prefix("    | ").unwrap().len(),
            source_line.len() + 1
        );

        let multi = format!("x{}\n{}y\n", "a".repeat(300), "b".repeat(300));
        let (sources, id) = one("w.ox", &multi);
        let rendered = excerpt(&primary(&sources, id, 0, 603), &sources);
        let lines: Vec<&str> = rendered.lines().collect();
        assert!(lines[2]
            .strip_prefix("  1 | x")
            .unwrap()
            .ends_with(ELLIPSIS));
        assert_eq!(
            lines[3].trim_end(),
            format!("    | {}", "^".repeat(MAX_LINE_CELLS - 3))
        );
        let last = lines[4].strip_prefix("  2 | ").unwrap();
        assert!(last.starts_with(ELLIPSIS) && last.ends_with('y'));
        assert_eq!(lines[5], format!("    | {}", "^".repeat(last.len())));
    }

    #[test]
    fn control_and_bidi_characters_are_escaped_not_emitted() {
        // Bytes: a, ESC, "[2Jb", U+202E (6..9), c (9), NUL (10), d (11), LF.
        let (sources, id) = one("x\u{1b}.ox", "a\u{1b}[2Jb\u{202e}c\u{0}d\n");
        let diagnostic = primary(&sources, id, 9, 10)
            .secondary(sources.get(id).span(11, 12), "label\u{7}\u{2066}");
        let rendered = render_human_context(&diagnostic, &sources);
        let excerpt = &rendered[diagnostic.render_human(&sources).len()..];
        assert!(!excerpt.chars().any(|ch| ch != '\n' && is_unsafe(ch)));
        // Escaped widths: "a" 1 + "\\u{1b}" 6 + "[2Jb" 4 + "\\u{202e}" 8 = 19 cells before c,
        // then "c" 1 + "\\u{0}" 5 = 25 cells before d.
        let line = "  1 | a\\u{1b}[2Jb\\u{202e}c\\u{0}d\n";
        assert_eq!(
            excerpt,
            format!(
                "  = primary: x\\u{{1b}}.ox:1:8\n    |\n{line}    | {}^\n\
                 \x20 = secondary: x\\u{{1b}}.ox:1:10\n    |\n{line}    | {}- label\\u{{7}}\\u{{2066}}\n",
                " ".repeat(19),
                " ".repeat(25),
            )
        );
    }

    #[test]
    fn secondary_labels_render_with_and_without_primary() {
        let mut sources = SourceMap::new();
        let main = sources.add("main.ox".into(), "use x;\n".into());
        let other = sources.add("lib.ox".into(), "fn x() {}\n".into());
        let with_primary = Diagnostic::new(
            "E1002",
            "resolve",
            "dup",
            Some(sources.get(main).span(4, 5)),
        )
        .secondary(sources.get(other).span(3, 4), "first defined here");
        assert_eq!(
            render_human_context(&with_primary, &sources),
            concat!(
                "error[E1002] (resolve): dup\n",
                "  --> main.ox:1:5\n",
                "  ::: lib.ox:1:4: first defined here\n",
                "  = primary: main.ox:1:5\n",
                "    |\n",
                "  1 | use x;\n",
                "    |     ^\n",
                "  = secondary: lib.ox:1:4\n",
                "    |\n",
                "  1 | fn x() {}\n",
                "    |    - first defined here\n",
            )
        );
        let without_primary = Diagnostic::new("E1003", "resolve", "cycle", None)
            .secondary(sources.get(other).span(0, 9), "here");
        assert_eq!(
            render_human_context(&without_primary, &sources),
            concat!(
                "error[E1003] (resolve): cycle\n",
                "  ::: lib.ox:1:1: here\n",
                "  = secondary: lib.ox:1:1\n",
                "    |\n",
                "  1 | fn x() {}\n",
                "    | --------- here\n",
            )
        );
    }
}
