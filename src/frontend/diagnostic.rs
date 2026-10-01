//! Human and newline-delimited JSON diagnostics for the typed preview.

use super::source::{SourceMap, Span};

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub code: &'static str,
    pub stage: &'static str,
    pub message: String,
    pub primary: Option<Span>,
    pub secondary: Vec<(Span, String)>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn new(
        code: &'static str,
        stage: &'static str,
        message: impl Into<String>,
        primary: Option<Span>,
    ) -> Box<Self> {
        Box::new(Self {
            code,
            stage,
            message: message.into(),
            primary,
            secondary: Vec::new(),
            notes: Vec::new(),
        })
    }

    pub fn secondary(mut self: Box<Self>, span: Span, message: impl Into<String>) -> Box<Self> {
        self.secondary.push((span, message.into()));
        self
    }

    /// Render plain text with a final newline and no terminal control sequences.
    pub fn render_human(&self, sources: &SourceMap) -> String {
        let mut result = format!(
            "error[{}] ({}): {}\n",
            human_text(self.code),
            human_text(self.stage),
            human_text(&self.message),
        );
        if let Some(span) = self.primary {
            result.push_str(&format!("  --> {}\n", human_location(span, sources)));
        }
        for (span, message) in &self.secondary {
            result.push_str(&format!(
                "  ::: {}: {}\n",
                human_location(*span, sources),
                human_text(message),
            ));
        }
        for note in &self.notes {
            result.push_str(&format!("  = note: {}\n", human_text(note)));
        }
        result
    }

    /// Render one JSON record without a trailing newline. The driver adds it.
    pub fn render_json(&self, sources: &SourceMap) -> String {
        let primary = self
            .primary
            .map_or_else(|| "null".into(), |span| json_span(span, sources));
        let secondary = self
            .secondary
            .iter()
            .map(|(span, message)| {
                format!(
                    "{{\"span\":{},\"message\":{}}}",
                    json_span(*span, sources),
                    json_string(message)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let notes = self
            .notes
            .iter()
            .map(|note| json_string(note))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",\"severity\":\"error\",\"code\":{},\"stage\":{},\"message\":{},\"primary\":{},\"secondary\":[{}],\"notes\":[{}]}}",
            json_string(self.code),
            json_string(self.stage),
            json_string(&self.message),
            primary,
            secondary,
            notes,
        )
    }
}

/// Quote a UTF-8 string as a JSON primitive, escaping all JSON control bytes.
/// Unicode scalar values are preserved; JSON does not require ASCII output.
pub fn json_string(text: &str) -> String {
    use std::fmt::Write;

    let mut result = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\u{8}' => result.push_str("\\b"),
            '\t' => result.push_str("\\t"),
            '\n' => result.push_str("\\n"),
            '\u{c}' => result.push_str("\\f"),
            '\r' => result.push_str("\\r"),
            '\u{0}'..='\u{1f}' => {
                write!(result, "\\u{:04x}", ch as u32).expect("writing to a String cannot fail")
            }
            _ => result.push(ch),
        }
    }
    result.push('"');
    result
}

fn json_span(span: Span, sources: &SourceMap) -> String {
    let source = sources.get(span.file);
    source.span(span.start, span.end);
    let (line, column) = source.location(span.start);
    let (end_line, end_column) = source.location(span.end);
    format!(
        "{{\"file_id\":{},\"path\":{},\"start\":{},\"end\":{},\"line\":{},\"column\":{},\"end_line\":{},\"end_column\":{}}}",
        span.file.0, json_string(source.path()), span.start, span.end, line, column, end_line, end_column,
    )
}

fn human_location(span: Span, sources: &SourceMap) -> String {
    let source = sources.get(span.file);
    source.span(span.start, span.end);
    let (line, column) = source.location(span.start);
    format!("{}:{line}:{column}", human_text(source.path()))
}

fn human_text(text: &str) -> String {
    let mut result = String::new();
    for ch in text.chars() {
        if ch.is_control() {
            result.extend(ch.escape_default());
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_and_labels_preserve_diagnostic_fields() {
        let mut sources = SourceMap::new();
        let id = sources.add("main.ox".into(), "true".into());
        let span = sources.get(id).span(0, 4);
        let diagnostic = Diagnostic::new("E0001", "parse", "expected bool", Some(span))
            .secondary(span, "declared here");
        assert_eq!(diagnostic.code, "E0001");
        assert_eq!(diagnostic.stage, "parse");
        assert_eq!(diagnostic.message, "expected bool");
        assert_eq!(diagnostic.primary, Some(span));
        assert_eq!(diagnostic.secondary, vec![(span, "declared here".into())]);
        assert!(diagnostic.notes.is_empty());
    }

    #[test]
    fn json_envelope_supports_errors_without_a_source() {
        let diagnostic = Diagnostic::new("E0002", "io", "file missing", None);
        assert_eq!(diagnostic.render_json(&SourceMap::default()), concat!(
            "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",",
            "\"severity\":\"error\",\"code\":\"E0002\",\"stage\":\"io\",\"message\":\"file missing\",",
            "\"primary\":null,\"secondary\":[],\"notes\":[]}"
        ));
    }

    #[test]
    fn json_locations_preserve_bytes_unicode_crlf_and_file_identity() {
        let mut sources = SourceMap::new();
        let first = sources.add("a\"\\\n.ox".into(), "é\r\n🦀".into());
        let second = sources.add("other.ox".into(), "x".into());
        let primary = sources.get(first).span(4, 8);
        let label = sources.get(second).span(1, 1);
        let mut diagnostic = Diagnostic::new("E1001", "resolve", "unknown 🦀", Some(primary))
            .secondary(label, "other \"name\"");
        diagnostic.notes.push("note\r\nline".into());
        assert_eq!(diagnostic.render_json(&sources), concat!(
            "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",",
            "\"severity\":\"error\",\"code\":\"E1001\",\"stage\":\"resolve\",\"message\":\"unknown 🦀\",",
            "\"primary\":{\"file_id\":0,\"path\":\"a\\\"\\\\\\n.ox\",\"start\":4,\"end\":8,",
            "\"line\":2,\"column\":1,\"end_line\":2,\"end_column\":2},",
            "\"secondary\":[{\"span\":{\"file_id\":1,\"path\":\"other.ox\",\"start\":1,\"end\":1,",
            "\"line\":1,\"column\":2,\"end_line\":1,\"end_column\":2},\"message\":\"other \\\"name\\\"\"}],",
            "\"notes\":[\"note\\r\\nline\"]}"
        ));
    }

    #[test]
    fn json_string_escapes_every_c0_control_and_quote_backslash() {
        let input = String::from_utf8((0..32).collect()).unwrap();
        assert_eq!(
            json_string(&input),
            concat!(
                "\"\\u0000\\u0001\\u0002\\u0003\\u0004\\u0005\\u0006\\u0007",
                "\\b\\t\\n\\u000b\\f\\r\\u000e\\u000f",
                "\\u0010\\u0011\\u0012\\u0013\\u0014\\u0015\\u0016\\u0017",
                "\\u0018\\u0019\\u001a\\u001b\\u001c\\u001d\\u001e\\u001f\""
            )
        );
        assert_eq!(json_string("\"\\/é🦀"), "\"\\\"\\\\/é🦀\"");
        assert_eq!(json_string(""), "\"\"");
    }

    #[test]
    fn human_render_includes_stage_primary_secondary_and_notes() {
        let mut sources = SourceMap::new();
        let id = sources.add("main.ox".into(), "é\r\ntrue".into());
        let mut diagnostic = Diagnostic::new(
            "E2001",
            "typecheck",
            "expected ()",
            Some(sources.get(id).span(4, 8)),
        )
        .secondary(sources.get(id).span(0, 2), "declared here");
        diagnostic.notes.push("bool is not unit".into());
        assert_eq!(
            diagnostic.render_human(&sources),
            concat!(
                "error[E2001] (typecheck): expected ()\n",
                "  --> main.ox:2:1\n",
                "  ::: main.ox:1:1: declared here\n",
                "  = note: bool is not unit\n"
            )
        );
    }

    #[test]
    fn human_render_supports_sourceless_errors_and_escapes_terminal_controls() {
        let diagnostic = Diagnostic::new("E0002", "io", "bad\n\u{1b}[31m\tpath", None);
        assert_eq!(
            diagnostic.render_human(&SourceMap::new()),
            "error[E0002] (io): bad\\n\\u{1b}[31m\\tpath\n"
        );
    }

    #[test]
    fn json_record_contains_no_raw_newline_or_terminal_controls() {
        let mut diagnostic = Diagnostic::new("E0003", "io", "\n\r\t\u{0}\u{1b}", None);
        diagnostic.notes.push("\u{8}\u{c}".into());
        let result = diagnostic.render_json(&SourceMap::new());
        assert!(result.starts_with('{'));
        assert!(!result.chars().any(|ch| ch <= '\u{1f}'));
    }
}
