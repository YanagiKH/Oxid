//! Optional source locations and scalar variables for verified LLVM emitters.
//!
//! Only their private source markers are interpreted. This pass adds metadata;
//! it cannot select instructions, declarations, an entry point, or an ABI.
use super::{
    ast,
    build_config::NativeSourceNames,
    diagnostic::Diagnostic,
    hir::Ty,
    project::ProjectSources,
    source::{SourceFileId, Span},
};
use std::fmt::{self, Write};

const MAX_DEBUG_IR_BYTES: usize = 128 * 1024 * 1024;
const FUNCTION: &str = "; oxid.debug.function ";
const LOCATION: &str = "; oxid.debug.location ";
const VARIABLE: &str = "; oxid.debug.variable ";

// Keep source names, LLVM operand types and DWARF encodings independent of
// enum discriminants and of the wider physical slots in the owned emitter.
const TYPES: &[(Ty, &str, &str, usize, &str)] = &[
    (Ty::Bool, "bool", "i1", 8, "DW_ATE_boolean"),
    (Ty::I8, "i8", "i8", 8, "DW_ATE_signed"),
    (Ty::U8, "u8", "i8", 8, "DW_ATE_unsigned"),
    (Ty::I16, "i16", "i16", 16, "DW_ATE_signed"),
    (Ty::U16, "u16", "i16", 16, "DW_ATE_unsigned"),
    (Ty::I32, "i32", "i32", 32, "DW_ATE_signed"),
    (Ty::U32, "u32", "i32", 32, "DW_ATE_unsigned"),
    (Ty::I64, "i64", "i64", 64, "DW_ATE_signed"),
    (Ty::U64, "u64", "i64", 64, "DW_ATE_unsigned"),
    (Ty::Isize, "isize", "i64", 64, "DW_ATE_signed"),
    (Ty::Usize, "usize", "i64", 64, "DW_ATE_unsigned"),
    (Ty::I128, "i128", "i128", 128, "DW_ATE_signed"),
    (Ty::U128, "u128", "i128", 128, "DW_ATE_unsigned"),
    (Ty::F32, "f32", "float", 32, "DW_ATE_float"),
    (Ty::F64, "f64", "double", 64, "DW_ATE_float"),
    (Ty::Char, "char", "i32", 32, "DW_ATE_UTF"),
];

pub(super) enum VariableValue<'a> {
    Address(&'a str, usize),
}

/// These private markers identify admitted source bindings. Their owning
/// emitter must admit and initialize the described storage. Unit and temporary
/// values are not source variables.
pub(super) fn variable(
    out: &mut impl Write,
    span: Span,
    ty: Ty,
    argument: usize,
    value: VariableValue<'_>,
) {
    let Some(ty) = TYPES.iter().position(|entry| entry.0 == ty) else {
        return;
    };
    let VariableValue::Address(operand, offset) = value;
    let kind = "address";
    writeln!(
        out,
        "{VARIABLE}{} {} {} {ty} {argument} {offset} {kind} {operand}",
        span.file.0, span.start, span.end
    )
    .unwrap();
}

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

fn source_function(project: &ProjectSources, span: Span) -> Option<&ast::Function> {
    project
        .try_file_ast(span.file)?
        .functions
        .iter()
        .find(|f| f.name == span)
}

struct LexicalScope {
    span: Span,
    id: usize,
    parent: Option<usize>,
}

struct BindingScope {
    name: Span,
    id: usize,
}

struct Scopes<'a> {
    function: usize,
    declaration: &'a ast::Function,
    blocks: Vec<LexicalScope>,
    bindings: Vec<BindingScope>,
}

impl<'a> Scopes<'a> {
    fn new(function: &'a ast::Function, id: usize, next: &mut usize) -> Result<Self, fmt::Error> {
        // Two fallible inventories share one checked scratch budget. The parser
        // limits nesting; lookup climbs ancestors, not preceding siblings.
        let count = function.blocks.len();
        let bindings = function
            .blocks
            .iter()
            .flat_map(|block| &block.body)
            .try_fold(0usize, |count, statement| {
                let extra = match &statement.kind {
                    ast::StmtKind::Let { .. } | ast::StmtKind::ForRange { .. } => 1,
                    ast::StmtKind::DestructureRecord { fields, .. } => fields.len(),
                    ast::StmtKind::Match { arms, .. } => {
                        arms.iter().filter(|arm| arm.binding.is_some()).count()
                    }
                    _ => 0,
                };
                count.checked_add(extra).ok_or(fmt::Error)
            })?;
        let scratch = count
            .checked_mul(std::mem::size_of::<LexicalScope>())
            .and_then(|size| {
                bindings
                    .checked_mul(std::mem::size_of::<BindingScope>())
                    .and_then(|extra| size.checked_add(extra))
            })
            .ok_or(fmt::Error)?;
        if scratch > MAX_DEBUG_IR_BYTES {
            return Err(fmt::Error);
        }
        let mut blocks = Vec::new();
        blocks.try_reserve_exact(count).map_err(|_| fmt::Error)?;
        for block in &function.blocks {
            blocks.push(LexicalScope {
                span: block.span,
                id: 0,
                parent: None,
            });
        }
        blocks.sort_unstable_by_key(|block| (block.span.start, std::cmp::Reverse(block.span.end)));
        let mut previous: Option<usize> = None;
        for index in 0..blocks.len() {
            while let Some(parent) = previous {
                if blocks[parent].span.start <= blocks[index].span.start
                    && blocks[index].span.end <= blocks[parent].span.end
                {
                    break;
                }
                previous = blocks[parent].parent;
            }
            blocks[index].parent = previous;
            blocks[index].id = *next;
            *next = next.checked_add(1).ok_or(fmt::Error)?;
            previous = Some(index);
        }
        let mut result = Self {
            function: id,
            declaration: function,
            blocks,
            bindings: Vec::new(),
        };
        result
            .bindings
            .try_reserve_exact(bindings)
            .map_err(|_| fmt::Error)?;
        for block in &function.blocks {
            let id = result.block(block.span)?;
            for statement in &block.body {
                match &statement.kind {
                    ast::StmtKind::Let { name, .. } => {
                        result.bindings.push(BindingScope { name: *name, id })
                    }
                    ast::StmtKind::DestructureRecord { fields, .. } => {
                        for field in fields {
                            result.bindings.push(BindingScope {
                                name: field.binding,
                                id,
                            });
                        }
                    }
                    ast::StmtKind::ForRange { name, body, .. } => {
                        let id = result.block(function.blocks[body.0].span)?;
                        result.bindings.push(BindingScope { name: *name, id });
                    }
                    ast::StmtKind::Match { arms, .. } => {
                        for arm in arms {
                            if let Some(name) = arm.binding {
                                let id = result.block(function.blocks[arm.body.0].span)?;
                                result.bindings.push(BindingScope { name, id });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if result.bindings.len() != bindings {
            return Err(fmt::Error);
        }
        result
            .bindings
            .sort_unstable_by_key(|binding| binding.name.start);
        Ok(result)
    }

    fn block(&self, span: Span) -> Result<usize, fmt::Error> {
        self.blocks
            .binary_search_by_key(&(span.start, std::cmp::Reverse(span.end)), |block| {
                (block.span.start, std::cmp::Reverse(block.span.end))
            })
            .map(|index| self.blocks[index].id)
            .map_err(|_| fmt::Error)
    }

    fn binding(&self, span: Span, argument: usize) -> Result<Option<usize>, fmt::Error> {
        if argument != 0 {
            return if self
                .declaration
                .params
                .get(argument - 1)
                .is_some_and(|parameter| parameter.name == span)
            {
                Ok(Some(self.function))
            } else {
                Err(fmt::Error)
            };
        }
        // Synthetic loop cursors and other implementation slots can also be
        // places. Exact AST declaration spans prevent exposing them as locals.
        Ok(self
            .bindings
            .binary_search_by_key(&span.start, |binding| binding.name.start)
            .ok()
            .filter(|&index| self.bindings[index].name == span)
            .map(|index| self.bindings[index].id))
    }

    fn at(&self, span: Span) -> usize {
        let mut index = self
            .blocks
            .partition_point(|block| block.span.start <= span.start)
            .checked_sub(1);
        while let Some(i) = index {
            let block = &self.blocks[i];
            if block.span.file == span.file && span.start < block.span.end {
                return block.id;
            }
            index = block.parent;
        }
        self.function
    }

    fn render(&self, project: &ProjectSources, out: &mut Output) -> fmt::Result {
        for block in &self.blocks {
            let parent = block
                .parent
                .map_or(self.function, |index| self.blocks[index].id);
            writeln!(
                out,
                "!{} = distinct !DILexicalBlock(scope: !{parent}, file: !{}, line: {}, column: 0)",
                block.id,
                5 + block.span.file.0,
                project
                    .sources()
                    .get(block.span.file)
                    .line_number(block.span.start)
            )?;
        }
        Ok(())
    }
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
    let first_type = 5usize
        .checked_add(sources.files().len())
        .ok_or(fmt::Error)?;
    let first_id = first_type.checked_add(TYPES.len()).ok_or(fmt::Error)?;
    for metadata in [false, true] {
        let mut next = first_id;
        let mut scope = None;
        let mut scopes = None;
        let mut location_id = None;
        let mut pending = false;
        let mut switch = false;
        for line in module.lines() {
            if let Some(text) = line.strip_prefix(FUNCTION) {
                let span = span(text, project)?;
                scope = None;
                scopes = None;
                location_id = None;
                pending = false;
                // Runtime builtins have import spans, rather than source
                // function declarations. Do not invent source names for them.
                if let Some(function) = source_function(project, span) {
                    let name = sources.text(function.name);
                    let id = next;
                    next = next.checked_add(1).ok_or(fmt::Error)?;
                    scope = Some(id);
                    let lexical = Scopes::new(function, id, &mut next)?;
                    pending = true;
                    if metadata {
                        let row = sources.get(span.file).line_number(span.start);
                        write!(out, "!{id} = distinct !DISubprogram(name: ")?;
                        string(out, name)?;
                        writeln!(out, ", scope: !{}, file: !{}, line: {row}, type: !3, scopeLine: {row}, spFlags: DISPFlagLocalToUnit | DISPFlagDefinition, unit: !0)", 5 + span.file.0, 5 + span.file.0)?;
                        lexical.render(project, out)?;
                    }
                    scopes = Some(lexical);
                }
                continue;
            }
            if let Some(text) = line.strip_prefix(LOCATION) {
                let span = span(text, project)?;
                if let Some(lexical) = &scopes {
                    let scope = lexical.at(span);
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
            if let Some(text) = line.strip_prefix(VARIABLE) {
                let Some(lexical) = &scopes else { continue };
                let mut words = text.split_whitespace();
                let mut number = || {
                    words
                        .next()
                        .ok_or(fmt::Error)?
                        .parse::<usize>()
                        .map_err(|_| fmt::Error)
                };
                let variable_span = Span {
                    file: SourceFileId(number()?),
                    start: number()?,
                    end: number()?,
                };
                if !sources.is_valid_span(variable_span) {
                    return Err(fmt::Error);
                }
                let ty = number()?;
                let argument = number()?;
                let offset = number()?;
                let kind = words.next().ok_or(fmt::Error)?;
                let operand = words.next().ok_or(fmt::Error)?;
                if words.next().is_some()
                    || !operand.starts_with('%')
                    || !operand[1..]
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                {
                    return Err(fmt::Error);
                }
                let ty_info = TYPES.get(ty).ok_or(fmt::Error)?;
                let Some(scope) = lexical.binding(variable_span, argument)? else {
                    continue;
                };
                let variable = next;
                let position = next.checked_add(1).ok_or(fmt::Error)?;
                next = next.checked_add(2).ok_or(fmt::Error)?;
                if metadata {
                    write!(out, "!{variable} = !DILocalVariable(name: ")?;
                    string(out, sources.text(variable_span))?;
                    if argument != 0 {
                        write!(out, ", arg: {argument}")?;
                    }
                    writeln!(
                        out,
                        ", scope: !{scope}, file: !{}, line: {}, type: !{})",
                        5 + variable_span.file.0,
                        sources
                            .get(variable_span.file)
                            .line_number(variable_span.start),
                        first_type + ty
                    )?;
                    writeln!(
                        out,
                        "!{position} = !DILocation(line: {}, column: 0, scope: !{scope})",
                        sources
                            .get(variable_span.file)
                            .line_number(variable_span.start)
                    )?;
                } else {
                    let (intrinsic, operand_ty) = match kind {
                        "address" => ("declare", "ptr"),
                        "value" if offset == 0 => ("value", ty_info.2),
                        _ => return Err(fmt::Error),
                    };
                    write!(out, "  call void @llvm.dbg.{intrinsic}(metadata {operand_ty} {operand}, metadata !{variable}, metadata !DIExpression(")?;
                    if offset != 0 {
                        write!(out, "DW_OP_plus_uconst, {offset}")?;
                    }
                    writeln!(out, ")), !dbg !{position}")?;
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
                scopes = None;
                location_id = None;
            }
        }
        if scope.is_some() || pending || switch {
            return Err(fmt::Error);
        }
        if !metadata {
            out.write_str("\ndeclare void @llvm.dbg.value(metadata, metadata, metadata)\ndeclare void @llvm.dbg.declare(metadata, metadata, metadata)\n")?;
            out.write_str("\n!llvm.dbg.cu = !{!0}\n!llvm.module.flags = !{!1, !2}\n!0 = distinct !DICompileUnit(language: DW_LANG_Rust, file: !5, producer: ")?;
            string(out, "Oxid")?;
            writeln!(
                out,
                ", isOptimized: {optimized}, runtimeVersion: 0, emissionKind: FullDebug, nameTableKind: None)"
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
            for (index, (_, name, _, bits, encoding)) in TYPES.iter().enumerate() {
                writeln!(
                    out,
                    "!{} = !DIBasicType(name: \"{name}\", size: {bits}, encoding: {encoding})",
                    first_type + index
                )?;
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
