//! Editor queries reuse the retained AST and ordinary namespace permissions.
use super::*;
use crate::frontend::{
    ast,
    declaration_index::{self as index, IndexLimits, SourceOwner, WorkMeter},
    project::{ItemPathRef, ModuleId, QualifiedPathRef},
};

fn contains(span: Span, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}
fn same_path(source: &str, path: &Path) -> bool {
    let source = Path::new(source);
    source == path || source.canonicalize().is_ok_and(|source| source == path)
}

fn path_name(syntax: &ast::Program, path: ast::ItemPath) -> Option<Span> {
    match path {
        ast::ItemPath::Unqualified(span) => Some(span),
        ast::ItemPath::Absolute(id) => {
            let path = syntax.paths.get(id.0)?;
            let end = path
                .segment_start
                .checked_add(usize::from(path.segment_len))?;
            syntax
                .path_segments
                .get(path.segment_start..end)?
                .last()
                .copied()
        }
    }
}

fn type_path(
    syntax: &ast::Program,
    mut ty: ast::TypeSyntax,
    offset: usize,
) -> Option<ast::ItemPath> {
    while contains(ty.span, offset) {
        let path = match ty.kind {
            ast::TypeSyntaxKind::Name(path)
            | ast::TypeSyntaxKind::Reference { referent: path, .. } => path,
            ast::TypeSyntaxKind::Applied(id)
            | ast::TypeSyntaxKind::AppliedReference {
                application: id, ..
            } => {
                let application = syntax.generic_application(id)?;
                if path_name(syntax, application.path).is_some_and(|name| contains(name, offset)) {
                    return Some(application.path);
                }
                ty = application
                    .arguments
                    .iter()
                    .copied()
                    .find(|arg| contains(arg.span, offset))?;
                continue;
            }
            _ => return None,
        };
        return path_name(syntax, path)
            .filter(|span| contains(*span, offset))
            .map(|_| path);
    }
    None
}

fn type_use(syntax: &ast::Program, offset: usize) -> Option<(ast::ItemPath, Span)> {
    for function in &syntax.functions {
        if offset < function.name.start || offset >= function.end.end {
            continue;
        }
        let types = function
            .params
            .iter()
            .map(|parameter| parameter.ty)
            .chain(std::iter::once(function.result))
            .chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.body)
                    .filter_map(|statement| match statement.kind {
                        ast::StmtKind::Let { annotation, .. } => annotation,
                        _ => None,
                    }),
            );
        for ty in types {
            if let Some(path) = type_path(syntax, ty, offset) {
                return Some((path, function.name));
            }
        }
        for expression in &syntax.expressions {
            if contains(expression.span, offset) {
                if let ast::ExprKind::StructLiteral { record, .. } = expression.kind {
                    if path_name(syntax, record).is_some_and(|name| contains(name, offset)) {
                        return Some((record, function.name));
                    }
                }
            }
        }
    }
    for record in &syntax.records {
        for field in &record.fields {
            if let Some(path) = type_path(syntax, field.ty, offset) {
                return Some((path, record.name));
            }
        }
    }
    for enumeration in &syntax.enums {
        for variant in &enumeration.variants {
            if let Some(path) = variant
                .payload
                .and_then(|payload| type_path(syntax, payload.value_type(), offset))
            {
                return Some((path, enumeration.name));
            }
        }
    }
    None
}

enum ValueUse {
    Call(ast::ItemPath),
    Qualified(ast::PathId),
    VariantPattern(ast::PathId),
    GenericVariant(ast::ItemPath, Span),
}

fn value_use(syntax: &ast::Program, offset: usize) -> Option<ValueUse> {
    syntax
        .expressions
        .iter()
        .find_map(|expression| match expression.kind {
            ast::ExprKind::Call { callee, .. } => {
                contains(path_name(syntax, callee)?, offset).then_some(ValueUse::Call(callee))
            }
            ast::ExprKind::QualifiedValue { path, .. } => {
                contains(path_name(syntax, ast::ItemPath::Absolute(path))?, offset)
                    .then_some(ValueUse::Qualified(path))
            }
            ast::ExprKind::GenericVariant {
                application,
                variant,
                ..
            } if contains(variant, offset) => Some(ValueUse::GenericVariant(
                syntax.generic_application(application)?.path,
                variant,
            )),
            _ => None,
        })
        .or_else(|| {
            syntax
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.body)
                .filter_map(|statement| match &statement.kind {
                    ast::StmtKind::Match { arms, .. } if contains(statement.span, offset) => {
                        Some(arms)
                    }
                    _ => None,
                })
                .flatten()
                .find_map(|arm| {
                    let name = path_name(syntax, ast::ItemPath::Absolute(arm.variant))?;
                    contains(name, offset).then_some(ValueUse::VariantPattern(arm.variant))
                })
        })
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
    let selected_type = type_use(syntax, offset);
    if let Some((ast::ItemPath::Unqualified(name), declaration)) = selected_type {
        if let Some(parameter) = syntax
            .generic_parameters(declaration)
            .iter()
            .find(|parameter| source.text_at(parameter.name) == source.text_at(name))
        {
            return Ok(location(sources, parameter.name));
        }
    }
    let selected_value = value_use(syntax, offset);
    if selected_type.is_none() && selected_value.is_none() {
        return Ok(None);
    }
    let limits = IndexLimits::default();
    let work = WorkMeter::new(limits.work);
    let mut allocator = Allocator::default();
    let facts = index::collect_originals(owner, limits, &work, &mut allocator)
        .map_err(|error| vec![*error])?;
    let index = facts.finish(&work, &mut allocator)?;
    let mut query = index.query(&work);
    let file = source.span(0, 0).file;
    let target = match (selected_type, selected_value) {
        (Some((path, _)), _) => query.type_declaration_name(module, ItemPathRef { file, path }),
        (_, Some(ValueUse::Call(path))) => query
            .callee(module, ItemPathRef { file, path }, false)
            .and_then(|target| query.function_declaration_name(target)),
        (_, Some(ValueUse::Qualified(path))) => query
            .qualified_value_endpoint(module, QualifiedPathRef { file, path })
            .and_then(|target| match target {
                index::QualifiedValueEndpoint::Function(id) => query.function_declaration_name(id),
                index::QualifiedValueEndpoint::Variant(id) => query.variant_declaration_name(id),
            }),
        (_, Some(ValueUse::VariantPattern(path))) => query
            .variant(module, QualifiedPathRef { file, path })
            .and_then(|target| query.variant_declaration_name(target)),
        (_, Some(ValueUse::GenericVariant(path, name))) => index
            .generic_variant_template(module, path, name, &work)
            .and_then(|target| query.variant_declaration_name(target)),
        (None, None) => return Ok(None),
    };
    match target {
        Ok(target) => Ok(target.and_then(|span| location(sources, span))),
        Err(error) if matches!(error.code, "E0400" | "E0500") => Err(vec![*error]),
        Err(_) => Ok(None),
    }
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
            ast::ExprKind::Name(name)
            | ast::ExprKind::Borrow { name, .. }
            | ast::ExprKind::BorrowElement { name, .. }
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
