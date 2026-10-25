//! Optional source line tables for the two verified LLVM emitters.
//!
//! Only their private source markers are interpreted. This pass adds metadata;
//! it cannot select instructions, declarations, an entry point, or an ABI.
use super::{
    build_config::NativeSourceNames,
    diagnostic::Diagnostic,
    project::ProjectSources,
    source::{SourceFileId, Span},
};
use std::fmt::{self, Write};

const MAX_DEBUG_IR_BYTES: usize = 128 * 1024 * 1024;
const FUNCTION: &str = "; oxid.debug.function ";
const LOCATION: &str = "; oxid.debug.location ";

pub(super) fn function(out: &mut impl Write, span: Span) {
    writeln!(out, "{FUNCTION}{} {} {}", span.file.0, span.start, span.end).unwrap();
}

pub(super) fn location(out: &mut impl Write, span: Span) {
    writeln!(out, "{LOCATION}{} {} {}", span.file.0, span.start, span.end).unwrap();
}

fn failure(message: &str) -> Box<Diagnostic> {
    Diagnostic::new("E0700", "native-debug-info", message, None)
}

struct Output {
    length: usize,
    maximum: usize,
    text: Option<String>,
    exceeded: bool,
}

impl Write for Output {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(next) = self
            .length
            .checked_add(value.len())
            .filter(|n| *n <= self.maximum)
        else {
            self.exceeded = true;
            return Err(fmt::Error);
        };
        if self
            .text
            .as_ref()
            .is_some_and(|text| next > text.capacity())
        {
            self.exceeded = true;
            return Err(fmt::Error);
        }
        self.length = next;
        if let Some(text) = &mut self.text {
            text.push_str(value);
        }
        Ok(())
    }
}

fn string(out: &mut Output, value: &str) -> fmt::Result {
    out.write_char('"')?;
    for byte in value.bytes() {
        match byte {
            b' '..=b'~' if byte != b'"' && byte != b'\\' => out.write_char(char::from(byte))?,
            _ => write!(out, "\\{byte:02X}")?,
        }
    }
    out.write_char('"')
}

fn span(text: &str, project: &ProjectSources) -> Result<Span, fmt::Error> {
    let mut words = text.split_whitespace();
    let mut number = || {
        words
            .next()
            .ok_or(fmt::Error)?
            .parse()
            .map_err(|_| fmt::Error)
    };
    let span = Span {
        file: SourceFileId(number()?),
        start: number()?,
        end: number()?,
    };
    if words.next().is_some() || !project.sources().is_valid_span(span) {
        return Err(fmt::Error);
    }
    Ok(span)
}

fn source_name(project: &ProjectSources, span: Span) -> Option<&str> {
    let declaration = project
        .try_file_ast(span.file)?
        .functions
        .iter()
        .find(|f| f.name == span)?;
    Some(project.sources().text(declaration.name))
}

// Emit the code and metadata in separate linear walks, without retaining a
// per-instruction table or rescanning source line prefixes for columns.
fn render(
    module: &str,
    project: &ProjectSources,
    names: Option<&NativeSourceNames<'_>>,
    optimized: bool,
    out: &mut Output,
) -> fmt::Result {
    let sources = project.sources();
    let first_id = 5usize
        .checked_add(sources.files().len())
        .ok_or(fmt::Error)?;
    for metadata in [false, true] {
        let mut next = first_id;
        let mut scope = None;
        let mut location_id = None;
        let mut pending = false;
        let mut switch = false;
        for line in module.lines() {
            if let Some(text) = line.strip_prefix(FUNCTION) {
                let span = span(text, project)?;
                scope = None;
                location_id = None;
                pending = false;
                // Runtime builtins have import spans, rather than source
                // function declarations. Do not invent source names for them.
                if let Some(name) = source_name(project, span) {
                    let id = next;
                    next = next.checked_add(1).ok_or(fmt::Error)?;
                    scope = Some(id);
                    pending = true;
                    if metadata {
                        let row = sources.get(span.file).line_number(span.start);
                        write!(out, "!{id} = distinct !DISubprogram(name: ")?;
                        string(out, name)?;
                        writeln!(out, ", scope: !{}, file: !{}, line: {row}, type: !3, scopeLine: {row}, spFlags: DISPFlagLocalToUnit | DISPFlagDefinition, unit: !0)", 5 + span.file.0, 5 + span.file.0)?;
                    }
                }
                continue;
            }
            if let Some(text) = line.strip_prefix(LOCATION) {
                let span = span(text, project)?;
                if let Some(scope) = scope {
                    let id = next;
                    next = next.checked_add(1).ok_or(fmt::Error)?;
                    location_id = Some(id);
                    if metadata {
                        writeln!(
                            out,
                            "!{id} = !DILocation(line: {}, column: 0, scope: !{scope})",
                            sources.get(span.file).line_number(span.start)
                        )?;
                    }
                }
                continue;
            }
            if pending && line.starts_with("define ") {
                let header = line.strip_suffix('{').ok_or(fmt::Error)?.trim_end();
                if !metadata {
                    writeln!(out, "{header} !dbg !{} {{", scope.ok_or(fmt::Error)?)?;
                }
                pending = false;
                continue;
            }
            let trimmed = line.trim();
            let mut attach = false;
            if scope.is_some() && location_id.is_some() {
                if switch {
                    if trimmed == "]" {
                        switch = false;
                        attach = true;
                    }
                } else if trimmed.starts_with("switch ") && trimmed.ends_with('[') {
                    switch = true;
                } else if line.starts_with("  ") && !trimmed.is_empty() && !trimmed.starts_with(';')
                {
                    attach = true;
                }
            }
            if !metadata {
                out.write_str(line)?;
                if attach {
                    write!(out, ", !dbg !{}", location_id.ok_or(fmt::Error)?)?;
                }
                out.write_char('\n')?;
            }
            if line == "}" {
                if switch || pending {
                    return Err(fmt::Error);
                }
                scope = None;
                location_id = None;
            }
        }
        if scope.is_some() || pending || switch {
            return Err(fmt::Error);
        }
        if !metadata {
            out.write_str("\n!llvm.dbg.cu = !{!0}\n!llvm.module.flags = !{!1, !2}\n!0 = distinct !DICompileUnit(language: DW_LANG_Rust, file: !5, producer: ")?;
            string(out, "Oxid")?;
            writeln!(
                out,
                ", isOptimized: {optimized}, runtimeVersion: 0, emissionKind: FullDebug)"
            )?;
            out.write_str("!1 = !{i32 2, !\"Dwarf Version\", i32 5}\n!2 = !{i32 2, !\"Debug Info Version\", i32 3}\n!3 = !DISubroutineType(types: !4)\n!4 = !{}\n")?;
            for (index, file) in sources.files().iter().enumerate() {
                let path = names
                    .and_then(|n| n.path(SourceFileId(index)))
                    .unwrap_or_else(|| file.path());
                write!(out, "!{} = !DIFile(filename: ", 5 + index)?;
                string(out, path)?;
                out.write_str(", directory: \".\")\n")?;
            }
        }
    }
    Ok(())
}

pub(super) fn add(
    module: &str,
    project: &ProjectSources,
    names: Option<&NativeSourceNames<'_>>,
    optimized: bool,
) -> Result<String, Box<Diagnostic>> {
    if project.sources().files().is_empty() {
        return Err(failure("debug information requires source files"));
    }
    if let Some(names) = names {
        names.validate(project.sources())?;
    }
    let mut count = Output {
        length: 0,
        maximum: MAX_DEBUG_IR_BYTES,
        text: None,
        exceeded: false,
    };
    render(module, project, names, optimized, &mut count)
        .map_err(|_| failure("native debug information is invalid or exceeds its byte limit"))?;
    let mut text = String::new();
    text.try_reserve_exact(count.length)
        .map_err(|_| failure("cannot allocate native debug information"))?;
    let mut output = Output {
        length: 0,
        maximum: count.length,
        text: Some(text),
        exceeded: false,
    };
    render(module, project, names, optimized, &mut output)
        .map_err(|_| failure("native debug information changed during emission"))?;
    if output.exceeded || output.length != count.length {
        return Err(failure("native debug byte count mismatch"));
    }
    Ok(output.text.expect("allocated output"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_denies_capacity_and_integer_overflow_before_growth() {
        let mut output = Output {
            length: 0,
            maximum: 4,
            text: Some(String::with_capacity(4)),
            exceeded: false,
        };
        output.write_str("abc").unwrap();
        let capacity = output.text.as_ref().unwrap().capacity();
        assert!(output.write_str("de").is_err());
        assert_eq!(output.length, 3);
        assert_eq!(output.text.as_ref().unwrap(), "abc");
        assert_eq!(output.text.as_ref().unwrap().capacity(), capacity);
        let mut count = Output {
            length: usize::MAX,
            maximum: usize::MAX,
            text: None,
            exceeded: false,
        };
        assert!(count.write_str("x").is_err());
        assert_eq!(count.length, usize::MAX);
    }

    #[test]
    fn metadata_strings_escape_quotes_backslashes_controls_and_utf8_bytes() {
        let mut output = Output {
            length: 0,
            maximum: 128,
            text: Some(String::with_capacity(128)),
            exceeded: false,
        };
        string(&mut output, "a\"\\\n雪").unwrap();
        assert_eq!(output.text.unwrap(), "\"a\\22\\5C\\0A\\E9\\9B\\AA\"");
    }
}
