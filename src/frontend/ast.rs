use super::lexer::Token;
use super::source::Span;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExprId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyBlockId(pub usize);
#[derive(Debug)]
pub enum ExprKind {
    Bool(bool),
    /// Exact decimal digits, separate literal-only sign, and full origin on Expr.
    Number {
        digits: Span,
        negative: bool,
    },
    Unit,
    Name(Span),
    Call {
        callee: Span,
        args: Vec<ExprId>,
    },
    Group(ExprId),
}
#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}
#[derive(Clone, Copy, Debug)]
pub struct TypeSyntax {
    pub span: Span,
}
#[derive(Debug)]
pub struct Param {
    pub name: Span,
    pub ty: TypeSyntax,
}
#[derive(Debug)]
pub enum StmtKind {
    Let {
        name: Span,
        annotation: Option<TypeSyntax>,
        init: ExprId,
    },
    Expr(ExprId),
    Return(Option<ExprId>),
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
pub struct BodyBlock {
    pub body: Vec<Stmt>,
    pub span: Span,
    pub end: Span,
}
#[derive(Debug)]
pub struct Function {
    pub name: Span,
    pub params: Vec<Param>,
    pub result: TypeSyntax,
    pub body: BodyBlockId,
    pub blocks: Vec<BodyBlock>,
    pub end: Span,
}
#[derive(Debug)]
pub struct Program {
    /// The complete lossless token tape, not a full public CST.
    pub tokens: Vec<Token>,
    pub functions: Vec<Function>,
    pub expressions: Vec<Expr>,
}
