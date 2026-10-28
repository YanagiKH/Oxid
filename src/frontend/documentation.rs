//! Public API text is projected from the retained, checked source session.
//! Rendering cannot reopen files, execute source or select compiler instructions.
use super::{ast, project::ProjectSources, source::Span};
use std::{
    fmt::{self, Write},
    ops::Range,
};

struct Counter {
    length: usize,
    maximum: usize,
}
impl Write for Counter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.length = self
            .length
            .checked_add(value.len())
            .filter(|n| *n <= self.maximum)
            .ok_or(fmt::Error)?;
        Ok(())
    }
}

fn escaped(out: &mut impl Write, text: &str) -> fmt::Result {
    for character in text.chars() {
        match character {
            '&' => out.write_str("&amp;")?,
            '<' => out.write_str("&lt;")?,
            '>' => out.write_str("&gt;")?,
            '"' => out.write_str("&quot;")?,
            '\'' => out.write_str("&#39;")?,
            other => out.write_char(other)?,
        }
    }
    Ok(())
}

fn text(project: &ProjectSources, span: Span) -> Result<&str, fmt::Error> {
    project.try_text(span).ok_or(fmt::Error)
}

fn comments(out: &mut impl Write, project: &ProjectSources, at: Span) -> fmt::Result {
    let source = project.sources().get(at.file).text();
    let prefix = source.get(..at.start).ok_or(fmt::Error)?;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    if !prefix[line_start..].trim().is_empty() {
        return Ok(());
    }
    let mut start = line_start;
    for line in source[..line_start].split_inclusive('\n').rev() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("///") || trimmed.starts_with("////") {
            break;
        }
        start -= line.len();
    }
    if start == line_start {
        return Ok(());
    }
    out.write_str("<p class=\"description\">")?;
    for (index, line) in source[start..line_start].lines().enumerate() {
        if index != 0 {
            out.write_char('\n')?;
        }
        let line = line.trim_start().strip_prefix("///").ok_or(fmt::Error)?;
        escaped(out, line.strip_prefix(' ').unwrap_or(line))?;
    }
    out.write_str("</p>\n")
}

fn module_comments(out: &mut impl Write, source: &str) -> fmt::Result {
    let mut opened = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if let Some(description) = trimmed.strip_prefix("//!") {
            if opened {
                out.write_char('\n')?;
            } else {
                out.write_str("<p class=\"description\">")?;
            }
            escaped(out, description.strip_prefix(' ').unwrap_or(description))?;
            opened = true;
        } else if !opened
            && (trimmed.is_empty() || trimmed.starts_with("//") && !trimmed.starts_with("///"))
        {
            continue;
        } else {
            break;
        }
    }
    if opened {
        out.write_str("</p>\n")?;
    }
    Ok(())
}

fn generics(
    out: &mut impl Write,
    project: &ProjectSources,
    ast: &ast::Program,
    name: Span,
) -> fmt::Result {
    if let Ok(index) = ast
        .generic_headers()
        .binary_search_by_key(&name.start, |header| header.declaration.start)
    {
        let header = &ast.generic_headers()[index];
        if header.declaration != name {
            return Err(fmt::Error);
        }
        out.write_str("&lt;")?;
        for (index, parameter) in header.parameters.iter().enumerate() {
            if index != 0 {
                out.write_str(", ")?;
            }
            escaped(out, text(project, parameter.span)?)?;
        }
        out.write_str("&gt;")?;
    }
    Ok(())
}

fn module_visible(project: &ProjectSources, root: usize, mut module: usize) -> bool {
    for _ in 0..=32 {
        if module == root {
            return true;
        }
        let Some(header) = project.modules().get(module) else {
            return false;
        };
        if header.public.is_none() {
            return false;
        }
        let Some(parent) = header.parent else {
            return false;
        };
        if parent.0 < root || parent.0 >= module {
            return false;
        }
        module = parent.0;
    }
    false
}

fn module_name(
    out: &mut impl Write,
    project: &ProjectSources,
    root: usize,
    mut module: usize,
) -> fmt::Result {
    let mut parts = [None; 33];
    let mut count = 0;
    while module != root {
        let header = project.modules().get(module).ok_or(fmt::Error)?;
        let part = parts.get_mut(count).ok_or(fmt::Error)?;
        *part = header.declaration;
        if part.is_none() {
            return Err(fmt::Error);
        }
        count += 1;
        module = header.parent.ok_or(fmt::Error)?.0;
    }
    out.write_str("crate")?;
    for span in parts[..count].iter().rev().flatten() {
        out.write_str("::")?;
        escaped(out, text(project, *span)?)?;
    }
    Ok(())
}

fn begin_item(
    out: &mut impl Write,
    project: &ProjectSources,
    kind: &str,
    name: Span,
) -> fmt::Result {
    write!(out, "<article><h4>{kind} ")?;
    escaped(out, text(project, name)?)?;
    out.write_str("</h4>\n<pre><code>")
}

fn render(
    out: &mut impl Write,
    project: &ProjectSources,
    modules: Range<usize>,
    name: &str,
    version: Option<&str>,
) -> fmt::Result {
    out.write_str("<section class=\"package\"><h2>")?;
    escaped(out, name)?;
    if let Some(version) = version {
        out.write_char(' ')?;
        escaped(out, version)?;
    }
    out.write_str("</h2>\n")?;
    let root = modules.start;
    for module in modules {
        if !module_visible(project, root, module) {
            continue;
        }
        let header = project.modules().get(module).ok_or(fmt::Error)?;
        let ast = project.try_file_ast(header.file).ok_or(fmt::Error)?;
        if ast
            .generic_headers()
            .windows(2)
            .any(|pair| pair[0].declaration.start >= pair[1].declaration.start)
        {
            return Err(fmt::Error);
        }
        out.write_str("<section class=\"module\"><h3>")?;
        module_name(out, project, root, module)?;
        out.write_str("</h3>\n")?;
        if let Some(public) = header.public {
            comments(out, project, public)?;
        }
        module_comments(out, project.sources().get(header.file).text())?;
        let mut items = 0;
        for item in &ast.items {
            match *item {
                ast::ItemId::Function(index) => {
                    let function = ast.functions.get(index).ok_or(fmt::Error)?;
                    let Some(public) = function.public else {
                        continue;
                    };
                    let body = function.blocks.get(function.body.0).ok_or(fmt::Error)?;
                    begin_item(out, project, "Function", function.name)?;
                    let signature = Span {
                        file: public.file,
                        start: public.start,
                        end: body.span.start,
                    };
                    escaped(out, text(project, signature)?.trim_end())?;
                    out.write_str(";</code></pre>\n")?;
                    comments(out, project, public)?;
                }
                ast::ItemId::Struct(index) => {
                    let record = ast.records.get(index).ok_or(fmt::Error)?;
                    let Some(public) = record.public else {
                        continue;
                    };
                    begin_item(out, project, "Struct", record.name)?;
                    if record.derive_copy.is_some() {
                        out.write_str("#[derive(Copy)]\n")?;
                    }
                    out.write_str("pub struct ")?;
                    escaped(out, text(project, record.name)?)?;
                    generics(out, project, ast, record.name)?;
                    out.write_str(" {\n")?;
                    for field in &record.fields {
                        if field.public.is_none() {
                            continue;
                        }
                        out.write_str("    pub ")?;
                        escaped(out, text(project, field.name)?)?;
                        out.write_str(": ")?;
                        escaped(out, text(project, field.ty.span)?)?;
                        out.write_str(",\n")?;
                    }
                    if record.fields.iter().any(|field| field.public.is_none()) {
                        out.write_str("    // private fields\n")?;
                    }
                    out.write_str("}</code></pre>\n")?;
                    if let Some(attribute) = record.derive_copy {
                        comments(out, project, attribute)?;
                    }
                    comments(out, project, public)?;
                    for field in &record.fields {
                        if let Some(public) = field.public {
                            out.write_str("<div class=\"field\"><h5>")?;
                            escaped(out, text(project, field.name)?)?;
                            out.write_str("</h5>")?;
                            comments(out, project, public)?;
                            out.write_str("</div>\n")?;
                        }
                    }
                }
                ast::ItemId::Enum(index) => {
                    let enumeration = ast.enums.get(index).ok_or(fmt::Error)?;
                    let Some(public) = enumeration.public else {
                        continue;
                    };
                    begin_item(out, project, "Enum", enumeration.name)?;
                    out.write_str("pub enum ")?;
                    escaped(out, text(project, enumeration.name)?)?;
                    generics(out, project, ast, enumeration.name)?;
                    out.write_str(" {\n")?;
                    for variant in &enumeration.variants {
                        out.write_str("    ")?;
                        escaped(out, text(project, variant.name)?)?;
                        if let Some(payload) = variant.payload {
                            out.write_char('(')?;
                            escaped(out, text(project, payload.span)?)?;
                            out.write_char(')')?;
                        }
                        out.write_str(",\n")?;
                    }
                    out.write_str("}</code></pre>\n")?;
                    comments(out, project, public)?;
                    for variant in &enumeration.variants {
                        out.write_str("<div class=\"variant\"><h5>")?;
                        escaped(out, text(project, variant.name)?)?;
                        out.write_str("</h5>")?;
                        comments(out, project, variant.name)?;
                        out.write_str("</div>\n")?;
                    }
                }
                _ => continue,
            }
            items += 1;
            out.write_str("</article>\n")?;
        }
        if items == 0 {
            out.write_str("<p>No public declarations in this module.</p>\n")?;
        }
        out.write_str("</section>\n")?;
    }
    out.write_str("</section>\n")
}

pub(super) fn fragment(
    project: &ProjectSources,
    modules: Range<usize>,
    name: &str,
    version: Option<&str>,
    maximum: usize,
) -> Result<String, String> {
    if modules.is_empty() || modules.end > project.modules().len() {
        return Err("invalid documentation source range".into());
    }
    let mut counter = Counter { length: 0, maximum };
    render(&mut counter, project, modules.clone(), name, version)
        .map_err(|_| "documentation exceeds its output limit or has invalid retained spans")?;
    let mut html = String::new();
    html.try_reserve_exact(counter.length)
        .map_err(|_| "cannot allocate API documentation")?;
    render(&mut html, project, modules, name, version)
        .map_err(|_| "documentation changed during rendering")?;
    if html.len() != counter.length {
        return Err("documentation byte count changed".into());
    }
    Ok(html)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn escaped_unicode_fragments_obey_the_exact_output_byte_budget() {
        let root = std::env::temp_dir().join(format!(
            "oxid-doc-budget-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let source = root.join("lib.ox");
        fs::write(
            &source,
            "/// 中文 <>&\"'.\npub fn answer() -> i32 { return 42; }\n",
        )
        .unwrap();
        let project = ProjectSources::load_typed(
            source.to_str().unwrap(),
            super::super::project::ProjectLimits::default(),
        )
        .unwrap();
        let html = fragment(&project, 0..1, "library<&", Some("1.0.0"), 4096).unwrap();
        assert!(html.contains("中文 &lt;&gt;&amp;&quot;&#39;."));
        assert!(html.contains("library&lt;&amp;"));
        assert_eq!(
            fragment(&project, 0..1, "library<&", Some("1.0.0"), html.len()).unwrap(),
            html
        );
        assert!(fragment(&project, 0..1, "library<&", Some("1.0.0"), html.len() - 1).is_err());
        assert!(fragment(&project, 0..1, "library<&", Some("1.0.0"), 0).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn copy_record_fragments_preserve_comments_privacy_and_exact_output_limits() {
        let root = std::env::temp_dir().join(format!(
            "oxid-copy-doc-budget-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let source = root.join("lib.ox");
        let declaration = "pub struct Value { pub number: i32, hidden_number: i32 }\n";
        let description = "/// A snapshot <&>.\n";
        let mut pages = Vec::new();
        for attribute in ["", "# /* not API text <script> */ [ derive ( Copy ) ]\n"] {
            fs::write(&source, format!("{description}{attribute}{declaration}")).unwrap();
            let project = ProjectSources::load_typed(
                source.to_str().unwrap(),
                super::super::project::ProjectLimits::default(),
            )
            .unwrap();
            let html = fragment(&project, 0..1, "library", Some("1.2.3"), 4096).unwrap();
            assert!(html.contains("A snapshot &lt;&amp;&gt;."));
            assert!(!html.contains("hidden_number"));
            assert!(!html.contains("not API text"));
            assert!(!html.contains("<script>"));
            assert_eq!(
                fragment(&project, 0..1, "library", Some("1.2.3"), html.len()).unwrap(),
                html
            );
            assert!(fragment(&project, 0..1, "library", Some("1.2.3"), html.len() - 1).is_err());
            pages.push(html);
        }
        let attribute = "#[derive(Copy)]\n";
        assert!(!pages[0].contains(attribute));
        assert_eq!(pages[1].matches(attribute).count(), 1);
        assert_eq!(pages[1].replace(attribute, ""), pages[0]);
        assert_eq!(pages[1].len(), pages[0].len() + attribute.len());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn counter_overflow_returns_an_error_without_wrapping() {
        let mut counter = Counter {
            length: usize::MAX,
            maximum: usize::MAX,
        };
        assert!(counter.write_str("x").is_err());
        let mut counter = Counter {
            length: 0,
            maximum: 3,
        };
        assert!(escaped(&mut counter, "&").is_err());
    }
}
