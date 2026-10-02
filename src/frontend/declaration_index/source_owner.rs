//! Checked, immutable source/AST associations; constructor authority is local.
use super::*;

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
        let segments = self
            .ast(self.module_for_file(path.file)?)?
            .path_segments(id)
            .ok_or_else(|| bad(self.eof()))?;
        // Collection checks every segment's file/span once before using this
        // access. Immutable source/AST borrows preserve that association; do not
        // rescan full paths inside each prefix comparison or semantic lookup.
        if !(2..=34).contains(&segments.len()) {
            return Err(bad(self.eof()));
        }
        Ok(segments)
    }
    pub fn owned(self, work: &WorkMeter) -> Result<bool, Box<Diagnostic>> {
        let mut owned = false;
        for m in 0..self.count() {
            let module = ModuleId(m);
            let program = self.ast(module)?;
            work.preflight(self.file(module)?.span(0, 0))?;
            if !program.records.is_empty() {
                owned = true;
                continue;
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
                            ast::StmtKind::FieldAssign { .. } => owned = true,
                            _ => (),
                        }
                    }
                }
            }
            for expression in &program.expressions {
                work.preflight(expression.span)?;
                match &expression.kind {
                    ast::ExprKind::StructLiteral { .. } | ast::ExprKind::FieldRead { .. } => {
                        owned = true
                    }
                    ast::ExprKind::Call { args, .. } => {
                        for argument in args {
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
