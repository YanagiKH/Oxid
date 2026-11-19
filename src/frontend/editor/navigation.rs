//! Editor queries reuse the retained AST and ordinary namespace permissions.
use super::*;
use crate::frontend::{
    ast,
    declaration_index::{self as index, IndexLimits, SourceOwner, WorkMeter},
    hir::DefId,
    project::{ItemPathRef, ModuleId},
};

fn contains(span: Span, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}
fn same_path(source: &str, path: &Path) -> bool {
    let source = Path::new(source);
    source == path || source.canonicalize().is_ok_and(|source| source == path)
}

pub(in crate::frontend) fn definition(
    owner: SourceOwner<'_>,
    sources: &SourceMap,
    path: &Path,
    offset: usize,
) -> Result<Option<EditorLocation>, Vec<Diagnostic>> {
    let found = (0..owner.count()).find_map(|ordinal| {
        let module = ModuleId(ordinal);
        let source = owner.file(module).ok()?;
        same_path(source.path(), path).then_some((module, source))
    });
    let Some((module, source)) = found else {
        return Ok(None);
    };
    if offset > source.text().len() || !source.text().is_char_boundary(offset) {
        return Ok(None);
    }
    let syntax = owner.ast(module).map_err(|error| vec![*error])?;
    for function in &syntax.functions {
        if contains(function.name, offset) {
            return Ok(location(sources, function.name));
        }
        if let Some(span) = local_definition(source, syntax, function, offset) {
            return Ok(location(sources, span));
        }
    }
    let Some((callee, qualified)) = syntax.expressions.iter().find_map(|expression| {
        let (callee, qualified) = match expression.kind {
            ast::ExprKind::Call { callee, .. } => (callee, false),
            ast::ExprKind::QualifiedValue { path, .. } => (ast::ItemPath::Absolute(path), true),
            _ => return None,
        };
        let selected = match callee {
            ast::ItemPath::Unqualified(span) => span,
            ast::ItemPath::Absolute(id) => {
                let path = syntax.paths.get(id.0)?;
                *syntax
                    .path_segments
                    .get(path.segment_start..path.segment_start + usize::from(path.segment_len))?
                    .last()?
            }
        };
        contains(selected, offset).then_some((callee, qualified))
    }) else {
        return Ok(None);
    };
    let limits = IndexLimits::default();
    let work = WorkMeter::new(limits.work);
    let mut allocator = Allocator::default();
    let facts = index::collect_originals(owner, limits, &work, &mut allocator)
        .map_err(|error| vec![*error])?;
    let mut names = Vec::new();
    allocator
        .vector(
            &mut names,
            facts.source_function_count(),
            "editor function declarations",
        )
        .map_err(|_| {
            vec![*Diagnostic::new(
                "E0400",
                "editor",
                "cannot reserve function navigation",
                None,
            )]
        })?;
    for ordinal in 0..facts.source_function_count() {
        let (key, decl_module) = facts
            .function(DefId(ordinal))
            .map_err(|error| vec![*error])?;
        let name = owner
            .ast(decl_module)
            .map_err(|error| vec![*error])?
            .functions
            .get(key.index)
            .ok_or_else(|| {
                vec![*Diagnostic::new(
                    "E0500",
                    "editor",
                    "invalid function declaration",
                    None,
                )]
            })?
            .name;
        names.push(name);
    }
    let index = facts.finish(&work, &mut allocator)?;
    let mut query = index.query(&work);
    let selected = if qualified {
        let ast::ItemPath::Absolute(path) = callee else {
            return Ok(None);
        };
        query
            .qualified_value_endpoint(
                module,
                crate::frontend::project::QualifiedPathRef {
                    file: source.span(0, 0).file,
                    path,
                },
            )
            .map(|target| match target {
                index::QualifiedValueEndpoint::Function(target) => Some(target),
                index::QualifiedValueEndpoint::Variant(_) => None,
            })
    } else {
        query
            .callee(
                module,
                ItemPathRef {
                    file: source.span(0, 0).file,
                    path: callee,
                },
                false,
            )
            .map(Some)
    };
    let target = match selected {
        Ok(Some(target)) => target,
        Ok(None) => return Ok(None),
        Err(error) if matches!(error.code, "E0400" | "E0500") => return Err(vec![*error]),
        Err(_) => return Ok(None),
    };
    Ok(names
        .get(target.0)
        .and_then(|span| location(sources, *span)))
}

fn local_definition(
    source: &super::super::source::SourceFile,
    syntax: &ast::Program,
    function: &ast::Function,
    offset: usize,
) -> Option<Span> {
    let body = function.blocks.get(function.body.0)?;
    if offset < body.span.start || offset > function.end.end {
        return None;
    }
    let name = syntax
        .expressions
        .iter()
        .find_map(|expression| match expression.kind {
            ast::ExprKind::Name(name) | ast::ExprKind::Borrow { name, .. }
                if contains(name, offset) =>
            {
                Some(name)
            }
            ast::ExprKind::FieldRead { base, .. }
            | ast::ExprKind::IndexRead { base, .. }
            | ast::ExprKind::ArrayLength { base }
                if contains(base, offset) =>
            {
                Some(base)
            }
            ast::ExprKind::Call { ref args, .. }
            | ast::ExprKind::QualifiedValue {
                args: Some(ref args),
                ..
            }
            | ast::ExprKind::GenericVariant {
                args: Some(ref args),
                ..
            } => args.iter().find_map(|argument| {
                let ast::Argument::Borrow { place, .. } = argument else {
                    return None;
                };
                let name = match *place {
                    ast::BorrowPlace::OwnerName(name)
                    | ast::BorrowPlace::ForwardedParameter { name, .. }
                    | ast::BorrowPlace::Range { name, .. } => name,
                };
                contains(name, offset).then_some(name)
            }),
            _ => None,
        })
        .or_else(|| {
            function
                .blocks
                .iter()
                .flat_map(|block| &block.body)
                .find_map(|statement| match statement.kind {
                    ast::StmtKind::Assign { name, .. } if contains(name, offset) => Some(name),
                    ast::StmtKind::FieldAssign { base, .. } if contains(base, offset) => Some(base),
                    _ => None,
                })
        })?;
    for parameter in &function.params {
        if source.text_at(parameter.name) == source.text_at(name) {
            return Some(parameter.name);
        }
    }
    let mut found = None;
    for block in &function.blocks {
        if offset < block.span.start || offset > block.end.end {
            continue;
        }
        for statement in &block.body {
            match statement.kind {
                ast::StmtKind::Let {
                    name: declaration,
                    init,
                    ..
                } if syntax.expressions.get(init.0)?.span.end <= name.start
                    && source.text_at(declaration) == source.text_at(name) =>
                {
                    if found.is_none_or(|prior: Span| prior.start < declaration.start) {
                        found = Some(declaration);
                    }
                }
                ast::StmtKind::ForRange {
                    name: declaration,
                    body,
                    ..
                } if source.text_at(declaration) == source.text_at(name) => {
                    let scope = function.blocks.get(body.0)?;
                    if offset >= scope.span.start && offset < scope.end.end {
                        found = Some(declaration);
                    }
                }
                _ => {}
            }
        }
    }
    found
}

pub(in crate::frontend) fn symbols(
    owner: SourceOwner<'_>,
    sources: &SourceMap,
    path: Option<&Path>,
    query: &str,
) -> Result<EditorSymbols, Vec<Diagnostic>> {
    let mut entries = Vec::new();
    let mut bytes = 0;
    for ordinal in 0..owner.count() {
        let module = ModuleId(ordinal);
        let source = owner.file(module).map_err(|error| vec![*error])?;
        if path.is_some_and(|path| !same_path(source.path(), path)) {
            continue;
        }
        let syntax = owner.ast(module).map_err(|error| vec![*error])?;
        let positions = super::positions::Positions::new(source.text()).map_err(|_| {
            vec![*Diagnostic::new(
                "E0400",
                "editor",
                "cannot reserve editor position index",
                None,
            )]
        })?;
        let mut add = |name: Span, kind, container: Option<Span>| -> Result<(), Vec<Diagnostic>> {
            if name.file != source.span(0, 0).file || !sources.is_valid_span(name) {
                return Err(vec![*Diagnostic::new(
                    "E0500",
                    "editor",
                    "invalid source symbol location",
                    None,
                )]);
            }
            let spelling = source.text_at(name);
            if !spelling.contains(query) {
                return Ok(());
            }
            let location = EditorLocation {
                path: source.path().into(),
                start: positions.at(name.start),
                end: positions.at(name.end),
            };
            let name = spelling;
            let container = container.map(|span| source.text_at(span));
            bytes += name.len() + location.path.len() + container.map_or(0, str::len);
            if entries.len() >= 4096 || bytes > 8 * 1024 * 1024 {
                return Err(vec![*Diagnostic::new(
                    "E0400",
                    "editor",
                    "too many symbols; narrow the workspace query",
                    None,
                )]);
            }
            entries.try_reserve(1).map_err(|_| {
                vec![*Diagnostic::new(
                    "E0400",
                    "editor",
                    "cannot reserve editor symbols",
                    None,
                )]
            })?;
            entries.push(EditorSymbol {
                name: name.to_owned(),
                kind,
                location,
                container: container.map(str::to_owned),
            });
            Ok(())
        };
        for module in &syntax.modules {
            add(module.name, 2, None)?;
        }
        for function in &syntax.functions {
            add(function.name, 12, None)?;
        }
        for record in &syntax.records {
            add(record.name, 23, None)?;
            for field in &record.fields {
                add(field.name, 8, Some(record.name))?;
            }
        }
        for enumeration in &syntax.enums {
            add(enumeration.name, 10, None)?;
            for variant in &enumeration.variants {
                add(variant.name, 22, Some(enumeration.name))?;
            }
        }
    }
    Ok(EditorSymbols {
        entries,
        files: sources.files().len(),
        bytes: sources
            .files()
            .iter()
            .map(|source| source.text().len())
            .sum(),
    })
}
