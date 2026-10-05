//! Resolved IDs are compilation-local, deterministic in source order.
pub use super::ast::{ArithmeticOp, ComparisonOp, LogicalOp};
use super::{
    ast,
    declaration_index::{
        self as index, DeclarationFacts, DeclarationIndex, IndexLimits, QuerySession, SourceOwner,
        TypeContext, WorkMeter,
    },
    diagnostic::Diagnostic,
    oir::owned_types::ValueTy,
    parser::MAX_DIAGNOSTICS,
    project::{budget::Allocator, ItemPathRef, ModuleId, SyntaxFlavor},
    source::{SourceFile, SourceView, Span},
};
use std::collections::HashMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Bool,
    I32,
    Unit,
}
impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Bool => "bool",
            Self::I32 => "i32",
            Self::Unit => "()",
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DefId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExprId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyBlockId(pub usize);
/// Function-local identity, indexed by the unique while-body block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoopId(pub usize);
#[derive(Debug)]
pub enum ExprKind {
    Negate {
        operand: ExprId,
        operator_span: Span,
    },
    Not {
        operand: ExprId,
        operator_span: Span,
    },
    Logical {
        op: LogicalOp,
        left: ExprId,
        right: ExprId,
        operator_span: Span,
    },
    Comparison {
        op: ComparisonOp,
        left: ExprId,
        right: ExprId,
        operator_span: Span,
    },
    Bool(bool),
    I32(i32),
    Unit,
    Local(LocalId),
    Call {
        target: DefId,
        args: Vec<ExprId>,
    },
    Group(ExprId),
    Arithmetic {
        op: ArithmeticOp,
        left: ExprId,
        right: ExprId,
        operator_span: Span,
    },
}
#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}
#[derive(Debug)]
pub struct Local {
    pub mutable: bool,
    pub span: Span,
    pub annotation: Option<Ty>,
}
#[derive(Debug)]
pub enum StmtKind {
    Let {
        local: LocalId,
        init: ExprId,
    },
    Assign {
        local: LocalId,
        target_span: Span,
        operator_span: Span,
        value: ExprId,
    },
    Expr(ExprId),
    Return(Option<ExprId>),
    Break {
        target: LoopId,
    },
    Continue {
        target: LoopId,
    },
    While {
        loop_id: LoopId,
        condition: ExprId,
        body: BodyBlockId,
    },
    If {
        condition: ExprId,
        then_block: BodyBlockId,
        else_block: Option<BodyBlockId>,
    },
}
#[derive(Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}
#[derive(Debug)]
pub struct Signature {
    pub params: Vec<Ty>,
    pub result: Ty,
    pub span: Span,
}
#[derive(Debug)]
pub struct BodyBlock {
    pub body: Vec<Stmt>,
    pub span: Span,
    pub end: Span,
}
#[derive(Debug)]
pub struct Function {
    pub id: DefId,
    pub locals: Vec<Local>,
    pub expressions: Vec<Expr>,
    pub body: BodyBlockId,
    pub blocks: Vec<BodyBlock>,
    pub end: Span,
}
#[derive(Debug)]
pub struct Program {
    pub signatures: Vec<Signature>,
    pub functions: Vec<Function>,
}

fn type_syntax(
    query: &mut QuerySession<'_, '_>,
    requester: ModuleId,
    ty: ast::TypeSyntax,
) -> Result<Ty, Box<Diagnostic>> {
    if matches!(
        ty.kind,
        ast::TypeSyntaxKind::Array(_)
            | ast::TypeSyntaxKind::ArrayReference { .. }
            | ast::TypeSyntaxKind::SliceReference { .. }
    ) {
        return Err(Diagnostic::new(
            "E0500",
            "resolve",
            "array source execution is unavailable in this dormant syntax checkpoint",
            Some(ty.span),
        ));
    }
    match query.value_type(requester, ty, TypeContext::Scalar)? {
        ValueTy::Scalar(ty) => Ok(ty),
        ValueTy::Owned(_) => Err(Diagnostic::new(
            "E0500",
            "resolve",
            "owned syntax entered scalar resolution",
            Some(ty.span),
        )),
    }
}
/// The parser has validated every byte before resolution. Accumulate directly
/// with the chosen sign: MIN never constructs an unrepresentable positive i32.
/// Arbitrarily many leading zeroes within the token limit remain exact zero.
fn decimal_i32(digits: &str, negative: bool, span: Span) -> Result<i32, Box<Diagnostic>> {
    let mut value = 0i32;
    for byte in digits.bytes() {
        let digit = i32::from(byte - b'0');
        value = value
            .checked_mul(10)
            .and_then(|value| {
                if negative {
                    value.checked_sub(digit)
                } else {
                    value.checked_add(digit)
                }
            })
            .ok_or_else(|| {
                Diagnostic::new(
                    "E0203",
                    "resolve",
                    "decimal literal is outside the i32 range [-2147483648, 2147483647]",
                    Some(span),
                )
            })?;
    }
    Ok(value)
}
fn duplicate(span: Span, original: Span) -> Box<Diagnostic> {
    Diagnostic::new(
        "E0201",
        "resolve",
        "duplicate binding; shadowing is unavailable in typed-preview",
        Some(span),
    )
    .secondary(original, "first declared here")
}

pub fn resolve(source: &SourceFile, ast: &ast::Program) -> Result<Program, Vec<Diagnostic>> {
    let sources =
        SourceOwner::original(source, ast, SourceView::Single(source)).map_err(|e| vec![*e])?;
    resolve_sources(sources)
}

pub(super) fn resolve_sources(sources: SourceOwner<'_>) -> Result<Program, Vec<Diagnostic>> {
    resolve_sources_with_meter(sources, &WorkMeter::default(), &mut Allocator::default())
}
#[cfg(test)]
pub(super) fn resolve_observed(
    source: &SourceFile,
    ast: &ast::Program,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<Program, Vec<Diagnostic>> {
    let sources =
        SourceOwner::original(source, ast, SourceView::Single(source)).map_err(|e| vec![*e])?;
    resolve_sources_with_meter(sources, work, allocator)
}
fn resolve_sources_with_meter(
    sources: SourceOwner<'_>,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<Program, Vec<Diagnostic>> {
    if let Some(record) = sources
        .ast(ModuleId(0))
        .map_err(|e| vec![*e])?
        .records
        .first()
    {
        return Err(vec![*Diagnostic::new(
            "E0500",
            "resolve",
            "owned syntax entered scalar resolution",
            Some(record.span),
        )]);
    }
    let facts = index::collect_originals(sources, IndexLimits::default(), work, allocator)
        .map_err(|e| vec![*e])?;
    if sources.flavor() == SyntaxFlavor::OriginalSingleFile {
        let signatures = original_signatures(&facts, work)?;
        let frozen = facts.finish(work, allocator)?;
        resolve_bodies(&frozen, work, signatures)
    } else {
        let frozen = facts.finish(work, allocator)?;
        resolve_project(&frozen, work)
    }
}

fn signature(
    query: &mut QuerySession<'_, '_>,
    requester: ModuleId,
    function: &ast::Function,
) -> Result<Signature, Box<Diagnostic>> {
    let params = function
        .params
        .iter()
        .map(|p| type_syntax(query, requester, p.ty))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Signature {
        params,
        result: type_syntax(query, requester, function.result)?,
        span: function.name,
    })
}

pub(super) fn original_signatures(
    facts: &DeclarationFacts<'_>,
    work: &WorkMeter,
) -> Result<Vec<Signature>, Vec<Diagnostic>> {
    let mut signatures = Vec::new();
    let mut diagnostics = Vec::new();
    for index in 0..facts.function_count() {
        let id = DefId(index);
        work.phase("original-conflicts");
        let original = facts.function_original(id).map_err(|e| vec![*e])?;
        if let Some(error) = facts.conflict(original, work).map_err(|e| vec![*e])? {
            work.record_error(&error);
            diagnostics.push(*error);
            if diagnostics.len() >= MAX_DIAGNOSTICS {
                break;
            }
        }
        let (key, module) = facts.function(id).map_err(|e| vec![*e])?;
        let function = &facts.sources().ast(module).map_err(|e| vec![*e])?.functions[key.index];
        work.phase("signatures");
        work.signature_start(id, function.name);
        match signature(&mut facts.signature_view(work), module, function) {
            Ok(s) => signatures.push(s),
            Err(error) => {
                work.record_error(&error);
                diagnostics.push(*error)
            }
        }
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
    }
    if diagnostics.is_empty() {
        Ok(signatures)
    } else {
        Err(diagnostics)
    }
}

pub(super) fn resolve_project(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
) -> Result<Program, Vec<Diagnostic>> {
    work.phase("signatures");
    let mut signatures = Vec::new();
    let mut diagnostics = Vec::new();
    for id in 0..index.function_count() {
        let (key, module) = index.function(DefId(id)).map_err(|e| vec![*e])?;
        let function = &index.sources().ast(module).map_err(|e| vec![*e])?.functions[key.index];
        work.signature_start(DefId(id), function.name);
        match signature(&mut index.query(work), module, function) {
            Ok(s) => signatures.push(s),
            Err(error) => {
                work.record_error(&error);
                diagnostics.push(*error)
            }
        }
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    resolve_bodies(index, work, signatures)
}

pub(super) fn resolve_bodies(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    signatures: Vec<Signature>,
) -> Result<Program, Vec<Diagnostic>> {
    work.phase("body-resolution");
    let mut functions = Vec::new();
    let mut diagnostics = Vec::new();
    for id in 0..index.function_count() {
        let (key, requester) = index.function(DefId(id)).map_err(|e| vec![*e])?;
        let ast = index.sources().ast(requester).map_err(|e| vec![*e])?;
        let source = index.sources().file(requester).map_err(|e| vec![*e])?;
        let mut resolver = Resolver {
            source,
            ast,
            index,
            work,
            requester,
            scope: HashMap::new(),
            locals: Vec::new(),
            expressions: Vec::new(),
        };
        match resolver.function(DefId(id), &ast.functions[key.index]) {
            Ok(function) => functions.push(function),
            Err(error) => {
                work.record_error(&error);
                diagnostics.push(*error)
            }
        }
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
    }
    if diagnostics.is_empty() {
        Ok(Program {
            signatures,
            functions,
        })
    } else {
        Err(diagnostics)
    }
}
struct Resolver<'i, 'a> {
    source: &'a SourceFile,
    ast: &'a ast::Program,
    index: &'i DeclarationIndex<'a>,
    work: &'i WorkMeter,
    requester: ModuleId,
    scope: HashMap<&'a str, (LocalId, Span)>,
    locals: Vec<Local>,
    expressions: Vec<Expr>,
}
impl<'a> Resolver<'_, 'a> {
    fn text(&self, span: Span) -> &'a str {
        self.source.text_at(span)
    }
    fn bind(
        &mut self,
        span: Span,
        annotation: Option<Ty>,
        mutable: bool,
    ) -> Result<LocalId, Box<Diagnostic>> {
        let name = self.text(span);
        if let Some((_, previous)) = self.scope.get(name) {
            return Err(duplicate(span, *previous));
        }
        if let Some(previous) = self
            .index
            .query(self.work)
            .value_binding_for_local_conflict(self.requester, span)?
        {
            return Err(duplicate(span, previous));
        }
        let id = LocalId(self.locals.len());
        self.locals.push(Local {
            span,
            annotation,
            mutable,
        });
        self.scope.insert(name, (id, span));
        Ok(id)
    }
    fn function(
        &mut self,
        id: DefId,
        function: &ast::Function,
    ) -> Result<Function, Box<Diagnostic>> {
        for param in &function.params {
            self.bind(
                param.name,
                Some(type_syntax(
                    &mut self.index.query(self.work),
                    self.requester,
                    param.ty,
                )?),
                false,
            )?;
        }
        // Keep block IDs stable while resolving statements depth first. Only
        // currently active names stay in the lookup table; each scope removes
        // its own names on exit, so neither cloning nor ancestor scans are needed.
        enum Frame {
            Enter(ast::BodyBlockId),
            Next(ast::BodyBlockId, usize),
            Leave,
            LeaveLoop,
        }
        let mut blocks: Vec<_> = function
            .blocks
            .iter()
            .map(|block| BodyBlock {
                body: Vec::with_capacity(block.body.len()),
                span: block.span,
                end: block.end,
            })
            .collect();
        let mut scopes: Vec<Vec<&'a str>> = Vec::new();
        let mut loops = Vec::new();
        let mut frames = vec![Frame::Enter(function.body)];
        while let Some(frame) = frames.pop() {
            let (block, index) = match frame {
                Frame::Enter(block) => {
                    scopes.push(Vec::new());
                    frames.push(Frame::Leave);
                    frames.push(Frame::Next(block, 0));
                    continue;
                }
                Frame::Leave => {
                    for name in scopes.pop().expect("entered scope") {
                        self.scope.remove(name);
                    }
                    continue;
                }
                Frame::LeaveLoop => {
                    loops.pop().expect("entered loop context");
                    continue;
                }
                Frame::Next(block, index) => (block, index),
            };
            let Some(statement) = function.blocks[block.0].body.get(index) else {
                continue;
            };
            frames.push(Frame::Next(block, index + 1));
            let kind = match &statement.kind {
                ast::StmtKind::Let {
                    mutable,
                    name,
                    annotation,
                    init,
                } => {
                    let init = self.expression(*init)?;
                    let annotation = annotation
                        .map(|ty| type_syntax(&mut self.index.query(self.work), self.requester, ty))
                        .transpose()?;
                    let local = self.bind(*name, annotation, *mutable)?;
                    scopes
                        .last_mut()
                        .expect("active body scope")
                        .push(self.text(*name));
                    StmtKind::Let { local, init }
                }
                ast::StmtKind::Assign {
                    name,
                    operator_span,
                    value,
                } => {
                    let text = self.text(*name);
                    let local = self
                        .scope
                        .get(text)
                        .ok_or_else(|| {
                            Diagnostic::new(
                                "E0200",
                                "resolve",
                                format!("unknown local `{text}`"),
                                Some(*name),
                            )
                        })?
                        .0;
                    StmtKind::Assign {
                        local,
                        target_span: *name,
                        operator_span: *operator_span,
                        value: self.expression(*value)?,
                    }
                }
                ast::StmtKind::FieldAssign { target_span, .. } => {
                    return Err(Diagnostic::new(
                        "E0500",
                        "resolve",
                        "owned syntax entered scalar resolution",
                        Some(*target_span),
                    ))
                }
                ast::StmtKind::IndexAssign { target, .. } => {
                    return Err(Diagnostic::new(
                        "E0500",
                        "resolve",
                        "array source execution is unavailable in this dormant syntax checkpoint",
                        Some(self.ast.expressions[target.0].span),
                    ));
                }
                ast::StmtKind::Expr(expr) => StmtKind::Expr(self.expression(*expr)?),
                ast::StmtKind::Match { .. } => {
                    return Err(Diagnostic::new(
                        "E0101",
                        "resolve",
                        "enum source syntax is unavailable",
                        Some(statement.span),
                    ));
                }
                ast::StmtKind::Return(expr) => {
                    StmtKind::Return(expr.map(|expr| self.expression(expr)).transpose()?)
                }
                ast::StmtKind::Break | ast::StmtKind::Continue => {
                    let is_break = matches!(statement.kind, ast::StmtKind::Break);
                    let keyword = if is_break { "break" } else { "continue" };
                    let target = loops.last().copied().ok_or_else(|| {
                        Diagnostic::new(
                            "E0204",
                            "resolve",
                            format!("`{keyword}` requires an enclosing while in the same function"),
                            Some(statement.span),
                        )
                    })?;
                    if is_break {
                        StmtKind::Break { target }
                    } else {
                        StmtKind::Continue { target }
                    }
                }
                ast::StmtKind::While { condition, body } => {
                    let condition = self.expression(*condition)?;
                    let loop_id = LoopId(body.0);
                    loops.push(loop_id);
                    frames.push(Frame::LeaveLoop);
                    frames.push(Frame::Enter(*body));
                    StmtKind::While {
                        loop_id,
                        condition,
                        body: BodyBlockId(body.0),
                    }
                }
                ast::StmtKind::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    let condition = self.expression(*condition)?;
                    if let Some(otherwise) = else_block {
                        frames.push(Frame::Enter(*otherwise));
                    }
                    frames.push(Frame::Enter(*then_block));
                    StmtKind::If {
                        condition,
                        then_block: BodyBlockId(then_block.0),
                        else_block: else_block.map(|id| BodyBlockId(id.0)),
                    }
                }
            };
            blocks[block.0].body.push(Stmt {
                kind,
                span: statement.span,
            });
        }
        Ok(Function {
            id,
            locals: std::mem::take(&mut self.locals),
            expressions: std::mem::take(&mut self.expressions),
            body: BodyBlockId(function.body.0),
            blocks,
            end: function.end,
        })
    }
    fn expression(&mut self, id: ast::ExprId) -> Result<ExprId, Box<Diagnostic>> {
        let expr = &self.ast.expressions[id.0];
        let kind = match &expr.kind {
            ast::ExprKind::QualifiedValue { .. } => {
                return Err(Diagnostic::new(
                    "E0101",
                    "resolve",
                    "enum source syntax is unavailable",
                    Some(expr.span),
                ));
            }
            ast::ExprKind::ArrayLiteral { .. }
            | ast::ExprKind::IndexRead { .. }
            | ast::ExprKind::ArrayLength { .. } => {
                return Err(Diagnostic::new(
                    "E0500",
                    "resolve",
                    "array source execution is unavailable in this dormant syntax checkpoint",
                    Some(expr.span),
                ));
            }
            ast::ExprKind::StructLiteral { .. } | ast::ExprKind::FieldRead { .. } => {
                return Err(Diagnostic::new(
                    "E0500",
                    "resolve",
                    "owned syntax entered scalar resolution",
                    Some(expr.span),
                ))
            }
            ast::ExprKind::Bool(value) => ExprKind::Bool(*value),
            ast::ExprKind::Number { digits, negative } => {
                ExprKind::I32(decimal_i32(self.text(*digits), *negative, expr.span)?)
            }
            ast::ExprKind::Unit => ExprKind::Unit,
            ast::ExprKind::Name(span) => {
                let name = self.text(*span);
                let local = self.scope.get(name).ok_or_else(|| {
                    Diagnostic::new(
                        "E0200",
                        "resolve",
                        format!("unknown local `{name}`"),
                        Some(*span),
                    )
                })?;
                ExprKind::Local(local.0)
            }
            ast::ExprKind::Call { callee, args } => {
                let target = self.index.query(self.work).callee(
                    self.requester,
                    ItemPathRef {
                        file: expr.span.file,
                        path: *callee,
                    },
                    true,
                )?;
                let args = args
                    .iter()
                    .map(|arg| match arg {
                        ast::Argument::Value(id) => self.expression(*id),
                        ast::Argument::Borrow { span, .. } => Err(Diagnostic::new(
                            "E0500",
                            "resolve",
                            "owned syntax entered scalar resolution",
                            Some(*span),
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                ExprKind::Call { target, args }
            }
            ast::ExprKind::Group(inner) => ExprKind::Group(self.expression(*inner)?),
            ast::ExprKind::Negate {
                operand,
                operator_span,
            } => ExprKind::Negate {
                operand: self.expression(*operand)?,
                operator_span: *operator_span,
            },
            ast::ExprKind::Not {
                operand,
                operator_span,
            } => ExprKind::Not {
                operand: self.expression(*operand)?,
                operator_span: *operator_span,
            },
            ast::ExprKind::Logical {
                op,
                left,
                right,
                operator_span,
            } => {
                let left = self.expression(*left)?;
                let right = self.expression(*right)?;
                ExprKind::Logical {
                    op: *op,
                    left,
                    right,
                    operator_span: *operator_span,
                }
            }
            ast::ExprKind::Comparison {
                op,
                left,
                right,
                operator_span,
            } => {
                let left = self.expression(*left)?;
                let right = self.expression(*right)?;
                ExprKind::Comparison {
                    op: *op,
                    left,
                    right,
                    operator_span: *operator_span,
                }
            }
            ast::ExprKind::Arithmetic {
                op,
                left,
                right,
                operator_span,
            } => {
                // Complete the left subtree before starting the right, including calls.
                let left = self.expression(*left)?;
                let right = self.expression(*right)?;
                ExprKind::Arithmetic {
                    op: *op,
                    left,
                    right,
                    operator_span: *operator_span,
                }
            }
        };
        let id = ExprId(self.expressions.len());
        self.expressions.push(Expr {
            kind,
            span: expr.span,
        });
        Ok(id)
    }
}
