//! Checked, immutable source/AST associations; constructor authority is local.
use super::*;
use crate::frontend::project::QualifiedPathRef;

/// A borrowed view from this exact source/AST owner, never a second path row.
pub(in crate::frontend) struct QualifiedPathView<'s> {
    root: ast::PathRoot,
    span: Span,
    segments: &'s [Span],
}
impl<'s> QualifiedPathView<'s> {
    pub fn root(&self) -> ast::PathRoot {
        self.root
    }
    pub fn span(&self) -> Span {
        self.span
    }
    pub fn segments(&self) -> &'s [Span] {
        self.segments
    }
}

/// No owning source copy. The legacy adapter binds the exact source allocation
/// recorded by the real parser, and validates the enclosing map association.
#[derive(Clone, Copy, Debug)]
enum Kind<'s> {
    Project(&'s ProjectSources),
    Original {
        file: &'s SourceFile,
        ast: &'s ast::Program,
        view: SourceView<'s>,
    },
}
#[derive(Clone, Copy, Debug)]
pub(in crate::frontend) struct SourceOwner<'s> {
    kind: Kind<'s>,
}
impl<'s> SourceOwner<'s> {
    pub fn original(
        file: &'s SourceFile,
        ast: &'s ast::Program,
        view: SourceView<'s>,
    ) -> Result<Self, Box<Diagnostic>> {
        let at = file.span(0, 0);
        let same = match view {
            SourceView::Single(other) => std::ptr::eq(file, other),
            SourceView::Map(map) => map
                .files()
                .get(at.file.0)
                .is_some_and(|other| std::ptr::eq(file, other)),
        };
        if !same || !ast.belongs_to(file) {
            // No supplied origin is authorized until both actual-map membership
            // and parser identity agree, even if a substitute map has that range.
            return Err(Diagnostic::new(
                "E0500",
                "resolve-project",
                "invalid declaration index source or identity",
                None,
            ));
        }
        Ok(Self {
            kind: Kind::Original { file, ast, view },
        })
    }
    pub fn project(sources: &'s ProjectSources) -> Self {
        Self {
            kind: Kind::Project(sources),
        }
    }
    pub(super) fn as_project(self) -> Option<&'s ProjectSources> {
        match self.kind {
            Kind::Project(p) => Some(p),
            Kind::Original { .. } => None,
        }
    }
    pub(super) fn is_original_adapter(self) -> bool {
        matches!(self.kind, Kind::Original { .. })
    }
    pub fn flavor(self) -> SyntaxFlavor {
        match self.kind {
            Kind::Project(p) => p.syntax_flavor(),
            Kind::Original { .. } => SyntaxFlavor::OriginalSingleFile,
        }
    }
    pub fn count(self) -> usize {
        match self.kind {
            Kind::Project(p) => p.modules().len(),
            Kind::Original { .. } => 1,
        }
    }
    pub fn file(self, module: ModuleId) -> Result<&'s SourceFile, Box<Diagnostic>> {
        match self.kind {
            Kind::Original { file, .. } if module.0 == 0 => Ok(file),
            Kind::Project(p) => p
                .modules()
                .get(module.0)
                .and_then(|h| p.sources().files().get(h.file.0))
                .ok_or_else(|| bad(self.eof())),
            _ => Err(bad(self.eof())),
        }
    }
    pub fn ast(self, module: ModuleId) -> Result<&'s ast::Program, Box<Diagnostic>> {
        let result = match self.kind {
            Kind::Original { ast, .. } if module.0 == 0 => Some(ast),
            Kind::Project(p) => p
                .modules()
                .get(module.0)
                .and_then(|h| p.try_file_ast(h.file)),
            _ => None,
        }
        .ok_or_else(|| bad(self.eof()))?;
        if !result.belongs_to(self.file(module)?) {
            return Err(bad(self.eof()));
        }
        Ok(result)
    }
    pub fn module_for_file(self, file: SourceFileId) -> Result<ModuleId, Box<Diagnostic>> {
        match self.kind {
            Kind::Original { file: source, .. } if source.span(0, 0).file == file => {
                Ok(ModuleId(0))
            }
            Kind::Project(p) if p.modules().get(file.0).is_some_and(|h| h.file == file) => {
                Ok(ModuleId(file.0))
            }
            _ => Err(bad(self.eof())),
        }
    }
    pub fn text(self, span: Span) -> Result<&'s str, Box<Diagnostic>> {
        match self.kind {
            Kind::Original { file, .. } => file.try_text(span),
            Kind::Project(p) => p.try_text(span),
        }
        .ok_or_else(|| bad(self.eof()))
    }
    /// Only prepared names from this immutable owner use this projection.
    /// Preparation already checked file membership and UTF-8 boundaries. Keep
    /// safe source access, without constructing a fallible diagnostic transport.
    pub(super) fn prepared_text(&self, name: CompactSpan) -> &'s str {
        self.frozen_text(name.span())
    }
    /// Frozen view constructors already validated this exact row key and source
    /// association. Safe projections retain bounds/file checks without minting
    /// a new boxed-error return inside each infallible getter.
    pub(super) fn frozen_enum(&self, key: EnumAstKey) -> &'s ast::EnumDecl {
        match &self.kind {
            Kind::Original { ast, .. } => &ast.enums[key.index],
            Kind::Project(project) => project.try_enum(key).expect("frozen enum association"),
        }
    }
    /// Internal projection of a span already checked against this immutable
    /// owner. This accepts no spelling as declaration or execution authority.
    pub(super) fn frozen_text(&self, span: Span) -> &'s str {
        match &self.kind {
            Kind::Original { file, .. } => file.text_at(span),
            Kind::Project(project) => project.sources().text(span),
        }
    }
    pub fn view(self) -> SourceView<'s> {
        match self.kind {
            Kind::Original { view, .. } => view,
            Kind::Project(p) => SourceView::Map(p.sources()),
        }
    }
    pub fn eof(self) -> Span {
        let file = match self.kind {
            Kind::Original { file, .. } => file,
            Kind::Project(p) => p.sources().get(SourceFileId(0)),
        };
        file.span(file.text().len(), file.text().len())
    }
    pub fn path_span(self, path: ItemPathRef) -> Result<Span, Box<Diagnostic>> {
        if let ast::ItemPath::Absolute(id) = path.path {
            let view = self.qualified_path(QualifiedPathRef {
                file: path.file,
                path: id,
            })?;
            if view.root != ast::PathRoot::Crate {
                return Err(bad(self.eof()));
            }
            return Ok(view.span);
        }
        let module = self.module_for_file(path.file)?;
        let span = self
            .ast(module)?
            .item_path_span(path.path)
            .ok_or_else(|| bad(self.eof()))?;
        if span.file != path.file {
            return Err(bad(self.eof()));
        }
        self.text(span)?;
        Ok(span)
    }
    pub(super) fn segments(self, path: ItemPathRef) -> Result<&'s [Span], Box<Diagnostic>> {
        let ast::ItemPath::Absolute(id) = path.path else {
            return Err(bad(self.eof()));
        };
        let view = self.qualified_path(QualifiedPathRef {
            file: path.file,
            path: id,
        })?;
        if view.root != ast::PathRoot::Crate {
            return Err(bad(self.eof()));
        }
        // Collection checks every segment's file/span once before using this
        // access. Immutable source/AST borrows preserve that association; do not
        // rescan full paths inside each prefix comparison or semantic lookup.
        Ok(view.segments)
    }
    pub fn import_path(
        self,
        path: QualifiedPathRef,
    ) -> Result<QualifiedPathView<'s>, Box<Diagnostic>> {
        let view = self.qualified_path(path)?;
        if !matches!(view.root, ast::PathRoot::Crate | ast::PathRoot::Std) {
            return Err(bad(self.eof()));
        }
        Ok(view)
    }
    pub fn qualified_path(
        self,
        path: QualifiedPathRef,
    ) -> Result<QualifiedPathView<'s>, Box<Diagnostic>> {
        let ast = self.ast(self.module_for_file(path.file)?)?;
        let row = ast.paths.get(path.path.0).ok_or_else(|| bad(self.eof()))?;
        let segments = ast
            .path_segments(path.path)
            .ok_or_else(|| bad(self.eof()))?;
        if row.span.file != path.file
            || !(2..=crate::frontend::parser::MAX_PATH_SEGMENTS).contains(&segments.len())
            || (row.root == ast::PathRoot::LocalType && segments.len() != 2)
        {
            return Err(bad(self.eof()));
        }
        self.text(row.span)?;
        let first = segments.first().ok_or_else(|| bad(self.eof()))?;
        let last = segments.last().ok_or_else(|| bad(self.eof()))?;
        if first.file != path.file
            || last.file != path.file
            || first.start != row.span.start
            || last.end != row.span.end
            || (row.root == ast::PathRoot::Crate && self.text(*first)? != "crate")
            || (row.root == ast::PathRoot::Std && self.text(*first)? != "std")
        {
            return Err(bad(self.eof()));
        }
        Ok(QualifiedPathView {
            root: row.root,
            span: row.span,
            segments,
        })
    }
    pub fn owned(self, work: &WorkMeter) -> Result<bool, Box<Diagnostic>> {
        let mut owned = false;
        for m in 0..self.count() {
            let module = ModuleId(m);
            let program = self.ast(module)?;
            work.preflight(self.file(module)?.span(0, 0))?;
            if !program.records.is_empty() || !program.enums.is_empty() {
                owned = true;
                continue;
            }
            // The private parsed summary avoids charging old crate imports for
            // stdin discovery. Collection rechecks the summary before allocation.
            if program.uses_std_imports() {
                for import in &program.imports {
                    work.preflight(import.span)?;
                    owned |= self
                        .import_path(QualifiedPathRef {
                            file: import.span.file,
                            path: import.path,
                        })?
                        .root()
                        == ast::PathRoot::Std;
                }
            }
            for function in &program.functions {
                work.preflight(function.name)?;
                owned |= self.owned_type(function.result, work)?;
                for parameter in &function.params {
                    work.preflight(parameter.name)?;
                    owned |= self.owned_type(parameter.ty, work)?;
                }
                for block in &function.blocks {
                    work.preflight(block.span)?;
                    for statement in &block.body {
                        work.preflight(statement.span)?;
                        match statement.kind {
                            ast::StmtKind::Let {
                                annotation: Some(ty),
                                ..
                            } => owned |= self.owned_type(ty, work)?,
                            ast::StmtKind::FieldAssign { .. }
                            | ast::StmtKind::IndexAssign { .. }
                            | ast::StmtKind::Match { .. } => owned = true,
                            _ => (),
                        }
                    }
                }
            }
            for expression in &program.expressions {
                work.preflight(expression.span)?;
                match &expression.kind {
                    ast::ExprKind::StructLiteral { .. }
                    | ast::ExprKind::FieldRead { .. }
                    | ast::ExprKind::ArrayLiteral { .. }
                    | ast::ExprKind::IndexRead { .. }
                    | ast::ExprKind::ArrayLength { .. } => owned = true,
                    ast::ExprKind::Call { args, .. } => {
                        for argument in args {
                            work.preflight(expression.span)?;
                            owned |= matches!(argument, ast::Argument::Borrow { .. });
                        }
                    }
                    ast::ExprKind::QualifiedValue { path, args } => {
                        let path = self.qualified_path(QualifiedPathRef {
                            file: expression.span.file,
                            path: *path,
                        })?;
                        owned |= path.root == ast::PathRoot::LocalType;
                        for argument in args.iter().flatten() {
                            work.preflight(expression.span)?;
                            owned |= matches!(argument, ast::Argument::Borrow { .. });
                        }
                    }
                    _ => (),
                }
            }
        }
        Ok(owned)
    }
    fn owned_type(self, ty: ast::TypeSyntax, work: &WorkMeter) -> Result<bool, Box<Diagnostic>> {
        work.preflight(ty.span)?;
        match ty.kind {
            ast::TypeSyntaxKind::Unit => Ok(false),
            ast::TypeSyntaxKind::Reference { .. }
            | ast::TypeSyntaxKind::Array(_)
            | ast::TypeSyntaxKind::ArrayReference { .. }
            | ast::TypeSyntaxKind::SliceReference { .. }
            | ast::TypeSyntaxKind::Name(ast::ItemPath::Absolute(_)) => Ok(true),
            ast::TypeSyntaxKind::Name(ast::ItemPath::Unqualified(name)) => {
                let text = self.text(name)?;
                let mut builtin = false;
                for spelling in ["bool", "i32"] {
                    work.preflight(name)?;
                    let mut equal = text.len() == spelling.len();
                    if equal {
                        for (a, b) in text.bytes().zip(spelling.bytes()) {
                            work.preflight(name)?;
                            if a != b {
                                equal = false;
                                break;
                            }
                        }
                    }
                    builtin |= equal;
                    if builtin {
                        break;
                    }
                }
                Ok(!builtin)
            }
        }
    }
}

#[cfg(test)]
mod enum_carrier_tests {
    use super::*;
    use crate::frontend::{lexer, parser, source::SourceMap};

    #[test]
    fn enum_carrier_qualified_path_view_layout_is_explicit() {
        use std::mem::{align_of, size_of};
        println!(
            "qualified-path-state view={} align={} handle={} align={}",
            size_of::<QualifiedPathView<'static>>(),
            align_of::<QualifiedPathView<'static>>(),
            size_of::<QualifiedPathRef>(),
            align_of::<QualifiedPathRef>(),
        );
        #[cfg(target_pointer_width = "64")]
        assert_eq!(
            (
                size_of::<QualifiedPathView<'static>>(),
                size_of::<QualifiedPathRef>()
            ),
            (48, 16),
        );
    }

    #[test]
    fn enum_carrier_absolute_segments_reject_local_and_forged_roots() {
        let mut sources = SourceMap::new();
        let file = sources.add("paths.ox".into(), "/* E::V */ fn f()->(){return;}".into());
        let source = sources.get(file);
        let mut ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let start = source.text().find("E::V").unwrap();
        ast.paths.push(ast::QualifiedPath {
            span: source.span(start, start + 4),
            segment_start: 0,
            segment_len: 2,
            root: ast::PathRoot::LocalType,
        });
        ast.path_segments.extend([
            source.span(start, start + 1),
            source.span(start + 3, start + 4),
        ]);
        let handle = ItemPathRef {
            file,
            path: ast::ItemPath::Absolute(ast::PathId(0)),
        };
        for root in [
            ast::PathRoot::LocalType,
            ast::PathRoot::Crate,
            ast::PathRoot::Std,
        ] {
            ast.paths[0].root = root;
            let owner = SourceOwner::original(source, &ast, SourceView::Single(source)).unwrap();
            assert!(owner.segments(handle).is_err(), "{root:?}");
        }
    }
}
