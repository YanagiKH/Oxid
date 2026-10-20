//! Deterministic public-API rendering from checked, retained compiler syntax.
use super::super::{
    ast::{self, ArrayElementTypeSyntax, ItemId, ItemPath, PathRoot, TypeSyntax, TypeSyntaxKind},
    builtin_catalog::{experimental, BuiltinEnum, BuiltinItem},
    project::{ModuleId, ProjectSources},
    source::Span,
};
use super::comments;
use std::{
    fmt::{self, Write as _},
    io::{self, Write},
    path::Path,
};

pub(super) fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid checked documentation view",
    )
}

/// Plain text inside generated Markdown. Never interpret comments/metadata as
/// markup, and never send terminal control characters through successful output.
pub(super) struct Text<'a>(pub &'a str);
impl fmt::Display for Text<'_> {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        for character in self.0.chars() {
            if character.is_control() {
                write!(output, "\\[U\\+{:04X}\\]", u32::from(character))?;
            } else {
                if character.is_ascii_punctuation() {
                    output.write_char('\\')?;
                }
                output.write_char(character)?;
            }
        }
        Ok(())
    }
}

fn span_text(project: &ProjectSources, span: Span) -> io::Result<&str> {
    project.try_text(span).ok_or_else(invalid)
}

fn parent(project: &ProjectSources, root: usize, module: usize) -> io::Result<usize> {
    let header = project.modules().get(module).ok_or_else(invalid)?;
    let parent = header.parent.ok_or_else(invalid)?.0;
    if parent < root || parent >= module {
        return Err(invalid());
    }
    Ok(parent)
}

fn visible(project: &ProjectSources, root: usize, mut module: usize) -> io::Result<bool> {
    while module != root {
        let header = project.modules().get(module).ok_or_else(invalid)?;
        let previous = parent(project, root, module)?;
        if header.public.is_none() {
            return Ok(false);
        }
        module = previous;
    }
    Ok(true)
}

fn module_path(
    project: &ProjectSources,
    root: usize,
    module: usize,
    output: &mut impl Write,
) -> io::Result<()> {
    if module == root {
        return output.write_all(b"crate");
    }
    // The unchanged checked loader bounds depth, and the ordinal test prevents
    // a corrupt cycle from recursing even during the output-free preflight.
    module_path(project, root, parent(project, root, module)?, output)?;
    let header = project.modules().get(module).ok_or_else(invalid)?;
    let name = span_text(project, header.declaration.ok_or_else(invalid)?)?;
    write!(output, "::{}", Text(name))
}

fn source_label(project: &ProjectSources, root: usize, module: usize) -> io::Result<&str> {
    let header = project.modules().get(module).ok_or_else(invalid)?;
    if module != root {
        return Ok(&header.relative_path);
    }
    Path::new(project.sources().get(header.file).path())
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(invalid)
}

fn path_segments(ast: &ast::Program, id: ast::PathId) -> io::Result<&[Span]> {
    let path = ast.paths.get(id.0).ok_or_else(invalid)?;
    let end = path
        .segment_start
        .checked_add(usize::from(path.segment_len))
        .ok_or_else(invalid)?;
    ast.path_segments
        .get(path.segment_start..end)
        .filter(|segments| !segments.is_empty())
        .ok_or_else(invalid)
}

fn qualified_path(
    project: &ProjectSources,
    ast: &ast::Program,
    path: ast::PathId,
    output: &mut impl Write,
) -> io::Result<()> {
    for (index, span) in path_segments(ast, path)?.iter().enumerate() {
        if index != 0 {
            output.write_all(b"::")?;
        }
        output.write_all(span_text(project, *span)?.as_bytes())?;
    }
    Ok(())
}

fn item_path(
    project: &ProjectSources,
    ast: &ast::Program,
    path: ItemPath,
    output: &mut impl Write,
) -> io::Result<()> {
    match path {
        ItemPath::Unqualified(span) => output.write_all(span_text(project, span)?.as_bytes()),
        ItemPath::Absolute(id) => qualified_path(project, ast, id, output),
    }
}

fn array_element(element: ArrayElementTypeSyntax) -> &'static str {
    match element {
        ArrayElementTypeSyntax::Bool => "bool",
        ArrayElementTypeSyntax::I32 => "i32",
        ArrayElementTypeSyntax::U8 => "u8",
        ArrayElementTypeSyntax::Unit => "()",
    }
}

fn reference(mutable: bool, output: &mut impl Write) -> io::Result<()> {
    output.write_all(if mutable { b"&mut " } else { b"&" })
}

fn ty(
    project: &ProjectSources,
    ast: &ast::Program,
    ty: TypeSyntax,
    output: &mut impl Write,
) -> io::Result<()> {
    match ty.kind {
        TypeSyntaxKind::Name(path) => item_path(project, ast, path, output),
        TypeSyntaxKind::Unit => output.write_all(b"()"),
        TypeSyntaxKind::Reference { mutable, referent } => {
            reference(mutable, output)?;
            item_path(project, ast, referent, output)
        }
        TypeSyntaxKind::Array(array) => {
            write!(
                output,
                "[{}; {}]",
                array_element(array.element),
                array.length
            )
        }
        TypeSyntaxKind::SliceReference { mutable, element } => {
            reference(mutable, output)?;
            write!(output, "[{}]", array_element(element))
        }
        TypeSyntaxKind::ArrayReference { mutable, array } => {
            reference(mutable, output)?;
            write!(
                output,
                "[{}; {}]",
                array_element(array.element),
                array.length
            )
        }
    }
}

fn primitive(name: &str) -> bool {
    matches!(name, "bool" | "i32" | "u8")
}

fn builtin_type(path: [&str; 3]) -> bool {
    BuiltinEnum::ALL
        .into_iter()
        .any(|item| BuiltinItem::Enum(item).path() == path)
        || experimental::Item::ALL.into_iter().any(|item| {
            (matches!(item, experimental::Item::ByteBuf) || item.enumeration().is_some())
                && item.path() == path
        })
}

/// Classify a successful original import's endpoint using retained declarations.
/// This is presentation only: the ordinary checker has already resolved names,
/// visibility and both namespaces. In particular, a value-only import must not
/// explain a same-spelled local type, scalar, or separate imported type alias.
fn type_import(
    project: &ProjectSources,
    module: usize,
    ast: &ast::Program,
    import: &ast::ImportDecl,
) -> io::Result<bool> {
    let path = ast.paths.get(import.path.0).ok_or_else(invalid)?;
    let segments = path_segments(ast, import.path)?;
    if path.root == PathRoot::Std {
        if segments.len() != 3 {
            return Err(invalid());
        }
        return Ok(builtin_type([
            span_text(project, segments[0])?,
            span_text(project, segments[1])?,
            span_text(project, segments[2])?,
        ]));
    }
    if segments.len() < 2 {
        return Err(invalid());
    }
    let graph = project.graph().ok_or_else(invalid)?;
    let owner = project
        .package_for_module(ModuleId(module))
        .ok_or_else(invalid)?;
    let owner = graph.packages.get(owner.0).ok_or_else(invalid)?;
    let mut target = match path.root {
        PathRoot::Crate => owner.root.0,
        PathRoot::Dependency => {
            let alias = span_text(project, segments[0])?;
            let dependency = owner
                .dependencies
                .iter()
                .find(|edge| edge.alias == alias)
                .ok_or_else(invalid)?;
            graph
                .packages
                .get(dependency.target.0)
                .ok_or_else(invalid)?
                .root
                .0
        }
        PathRoot::Std | PathRoot::LocalType => return Err(invalid()),
    };
    // Source imports target original declarations, never re-export/alias chains.
    // Follow only retained direct-child metadata, not filesystem discovery.
    for segment in &segments[1..segments.len() - 1] {
        let name = span_text(project, *segment)?;
        let mut child = None;
        for (index, header) in project.modules().iter().enumerate() {
            if header.parent == Some(ModuleId(target))
                && span_text(project, header.declaration.ok_or_else(invalid)?)? == name
            {
                child = Some(index);
                break;
            }
        }
        target = child.ok_or_else(invalid)?;
    }
    let header = project.modules().get(target).ok_or_else(invalid)?;
    let target_ast = project.try_file_ast(header.file).ok_or_else(invalid)?;
    let name = span_text(project, *segments.last().ok_or_else(invalid)?)?;
    for record in &target_ast.records {
        if span_text(project, record.name)? == name {
            return Ok(true);
        }
    }
    for enumeration in &target_ast.enums {
        if span_text(project, enumeration.name)? == name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn mentions(project: &ProjectSources, ty: TypeSyntax, alias: Span) -> io::Result<bool> {
    let name = match ty.kind {
        TypeSyntaxKind::Name(ItemPath::Unqualified(name))
        | TypeSyntaxKind::Reference {
            referent: ItemPath::Unqualified(name),
            ..
        } => name,
        _ => return Ok(false),
    };
    let name = span_text(project, name)?;
    Ok(!primitive(name) && name == span_text(project, alias)?)
}

fn signature_import(project: &ProjectSources, ast: &ast::Program, alias: Span) -> io::Result<bool> {
    for function in &ast.functions {
        if function.public.is_some() {
            if mentions(project, function.result, alias)? {
                return Ok(true);
            }
            for parameter in &function.params {
                if mentions(project, parameter.ty, alias)? {
                    return Ok(true);
                }
            }
        }
    }
    for record in &ast.records {
        if record.public.is_some() {
            for field in &record.fields {
                if field.public.is_some() && mentions(project, field.ty, alias)? {
                    return Ok(true);
                }
            }
        }
    }
    for enumeration in &ast.enums {
        if enumeration.public.is_some() {
            for variant in &enumeration.variants {
                if let Some(payload) = variant.payload {
                    if mentions(project, payload.value_type(), alias)? {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

fn imports(
    project: &ProjectSources,
    module: usize,
    ast: &ast::Program,
    output: &mut impl Write,
) -> io::Result<()> {
    let mut opened = false;
    for import in &ast.imports {
        if !type_import(project, module, ast, import)?
            || !signature_import(project, ast, import.alias)?
        {
            continue;
        }
        if !opened {
            output.write_all(b"### Signature imports\n\n```oxid\n")?;
            opened = true;
        }
        output.write_all(b"use ")?;
        qualified_path(project, ast, import.path, output)?;
        if path_segments(ast, import.path)?.last().copied() != Some(import.alias) {
            write!(output, " as {}", span_text(project, import.alias)?)?;
        }
        output.write_all(b";\n")?;
    }
    if opened {
        output.write_all(b"```\n\n")?;
    }
    Ok(())
}

fn heading(
    project: &ProjectSources,
    root: usize,
    module: usize,
    kind: &str,
    name: Span,
    output: &mut impl Write,
) -> io::Result<()> {
    write!(output, "### {kind} ")?;
    module_path(project, root, module, output)?;
    writeln!(output, "::{}\n", Text(span_text(project, name)?))?;
    let (line, column) = project.sources().get(name.file).location(name.start);
    writeln!(
        output,
        "Source: {}:{line}:{column}\n",
        Text(source_label(project, root, module)?)
    )?;
    output.write_all(b"```oxid\n")
}

fn function(
    project: &ProjectSources,
    ast: &ast::Program,
    function: &ast::Function,
    output: &mut impl Write,
) -> io::Result<()> {
    write!(output, "pub fn {}(", span_text(project, function.name)?)?;
    for (index, parameter) in function.params.iter().enumerate() {
        if index != 0 {
            output.write_all(b", ")?;
        }
        write!(output, "{}: ", span_text(project, parameter.name)?)?;
        ty(project, ast, parameter.ty, output)?;
    }
    output.write_all(b") -> ")?;
    ty(project, ast, function.result, output)?;
    output.write_all(b";\n```\n\n")
}

fn record(
    project: &ProjectSources,
    ast: &ast::Program,
    record: &ast::StructDecl,
    output: &mut impl Write,
) -> io::Result<()> {
    writeln!(output, "pub struct {} {{", span_text(project, record.name)?)?;
    let mut hidden = false;
    for field in &record.fields {
        if field.public.is_none() {
            hidden = true;
            continue;
        }
        write!(output, "    pub {}: ", span_text(project, field.name)?)?;
        ty(project, ast, field.ty, output)?;
        output.write_all(b",\n")?;
    }
    if hidden {
        output.write_all(b"    // private fields omitted\n")?;
    }
    output.write_all(b"}\n```\n\n")
}

fn enumeration(
    project: &ProjectSources,
    ast: &ast::Program,
    enumeration: &ast::EnumDecl,
    output: &mut impl Write,
) -> io::Result<()> {
    writeln!(
        output,
        "pub enum {} {{",
        span_text(project, enumeration.name)?
    )?;
    for variant in &enumeration.variants {
        write!(output, "    {}", span_text(project, variant.name)?)?;
        if let Some(payload) = variant.payload {
            output.write_all(b"(")?;
            ty(project, ast, payload.value_type(), output)?;
            output.write_all(b")")?;
        }
        output.write_all(b",\n")?;
    }
    output.write_all(b"}\n```\n\n")
}

pub(super) fn document(project: &ProjectSources, output: &mut impl Write) -> io::Result<()> {
    let selected = project
        .graph()
        .and_then(|graph| graph.packages.first())
        .ok_or_else(invalid)?;
    let root = selected.root.0;
    let modules = project
        .modules()
        .get(root..selected.end)
        .filter(|modules| !modules.is_empty())
        .ok_or_else(invalid)?;
    if modules[0].parent.is_some() {
        return Err(invalid());
    }
    let name = if selected.name.is_empty() {
        "(unnamed)"
    } else {
        &selected.name
    };
    writeln!(output, "# Typed API: {}\n", Text(name))?;
    output.write_all(b"Edition: typed-preview (experimental).\n\nThe selected package was checked; dependencies are checked but not documented.\nNo code was executed. Source paths are relative to the selected entry directory.\n\n")?;
    for (offset, header) in modules.iter().enumerate() {
        let module = root + offset;
        if !visible(project, root, module)? {
            continue;
        }
        let ast = project.try_file_ast(header.file).ok_or_else(invalid)?;
        output.write_all(b"## Module ")?;
        module_path(project, root, module, output)?;
        writeln!(
            output,
            "\n\nSource: {}\n",
            Text(source_label(project, root, module)?)
        )?;
        if let Some(anchor) = header.public {
            let parent_ast = project.try_file_ast(anchor.file).ok_or_else(invalid)?;
            comments::write(project, parent_ast, anchor, output)?;
        }
        imports(project, module, ast, output)?;
        for item in &ast.items {
            let anchor = match *item {
                ItemId::Function(index) => {
                    let item = ast.functions.get(index).ok_or_else(invalid)?;
                    let Some(anchor) = item.public else {
                        continue;
                    };
                    heading(project, root, module, "fn", item.name, output)?;
                    function(project, ast, item, output)?;
                    anchor
                }
                ItemId::Struct(index) => {
                    let item = ast.records.get(index).ok_or_else(invalid)?;
                    let Some(anchor) = item.public else {
                        continue;
                    };
                    heading(project, root, module, "struct", item.name, output)?;
                    record(project, ast, item, output)?;
                    anchor
                }
                ItemId::Enum(index) => {
                    let item = ast.enums.get(index).ok_or_else(invalid)?;
                    let Some(anchor) = item.public else {
                        continue;
                    };
                    heading(project, root, module, "enum", item.name, output)?;
                    enumeration(project, ast, item, output)?;
                    anchor
                }
                ItemId::Module(_) | ItemId::Import(_) => continue,
            };
            comments::write(project, ast, anchor, output)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{builtin_type, primitive, Text};

    #[test]
    fn imported_function_names_are_not_builtin_types() {
        assert!(builtin_type(["std", "io", "ReadStatus"]));
        assert!(!builtin_type(["std", "io", "read_stdin"]));
        assert!(builtin_type(["alloc", "bytes", "ByteBuf"]));
        assert!(builtin_type(["alloc", "bytes", "GrowStatus"]));
        assert!(!builtin_type(["alloc", "bytes", "take"]));
        assert!(primitive("bool") && primitive("i32") && primitive("u8"));
        assert!(!primitive("Local") && !primitive("ByteBuf"));
    }

    #[test]
    fn source_identifiers_are_plain_markdown_text() {
        // Heading/module callers wrap source names, never generated separators.
        assert_eq!(
            format!("crate::{}", Text("__name__")),
            "crate::\\_\\_name\\_\\_"
        );
        assert_eq!(format!("{}", Text("a_b")), "a\\_b");
    }

    #[test]
    fn prose_does_not_inject_markup_or_terminal_controls() {
        assert_eq!(
            format!("{}", Text("<b>[x](y)`\\")),
            "\\<b\\>\\[x\\]\\(y\\)\\`\\\\"
        );
        assert_eq!(
            format!("{}", Text("é\u{7f}\t")),
            "é\\[U\\+007F\\]\\[U\\+0009\\]"
        );
    }
}
