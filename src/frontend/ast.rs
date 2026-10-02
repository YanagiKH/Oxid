use super::lexer::Token;
use super::source::Span;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExprId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyBlockId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArithmeticOp {
    Add,
    Subtract,
    Multiply,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparisonOp {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicalOp {
    And,
    Or,
}
#[derive(Debug)]
pub enum ExprKind {
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
    /// Exact decimal digits, separate literal-only sign, and full origin on Expr.
    Number {
        digits: Span,
        negative: bool,
    },
    Unit,
    Name(Span),
    Call {
        callee: Span,
        args: Vec<Argument>,
    },
    StructLiteral {
        record: Span,
        fields: Vec<FieldInit>,
    },
    FieldRead {
        base: Span,
        field: Span,
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
#[derive(Clone, Copy, Debug)]
pub enum TypeSyntaxKind {
    Name(Span),
    Unit,
    Reference { mutable: bool, referent: Span },
}
#[derive(Clone, Copy, Debug)]
pub struct TypeSyntax {
    pub span: Span,
    pub kind: TypeSyntaxKind,
}
#[derive(Debug)]
pub struct StructField {
    pub name: Span,
    pub ty: TypeSyntax,
    pub span: Span,
}
#[derive(Debug)]
pub struct StructDecl {
    pub name: Span,
    pub fields: Vec<StructField>,
    pub span: Span,
    pub end: Span,
}
/// Private source-discovery grammar; no namespace or runtime semantics yet.
#[derive(Clone, Copy, Debug)]
pub struct ModuleDecl {
    pub name: Span,
    pub public: Option<Span>,
    #[allow(dead_code)] // Full original declaration origin for the staged shared index.
    pub span: Span,
}
#[derive(Clone, Copy, Debug)]
pub enum ItemId {
    Module(usize),
    Function(usize),
    Struct(usize),
}
#[derive(Clone, Copy, Debug)]
pub enum BorrowPlace {
    OwnerName(Span),
    ForwardedParameter { name: Span, star_span: Span },
}
#[derive(Debug)]
pub enum Argument {
    Value(ExprId),
    Borrow {
        mutable: bool,
        place: BorrowPlace,
        span: Span,
    },
}
#[derive(Debug)]
pub struct FieldInit {
    pub name: Span,
    pub value: ExprId,
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
        mutable: bool,
        name: Span,
        annotation: Option<TypeSyntax>,
        init: ExprId,
    },
    Assign {
        name: Span,
        operator_span: Span,
        value: ExprId,
    },
    FieldAssign {
        base: Span,
        field: Span,
        target_span: Span,
        operator_span: Span,
        value: ExprId,
    },
    Expr(ExprId),
    Return(Option<ExprId>),
    Break,
    Continue,
    While {
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
    pub records: Vec<StructDecl>,
    pub items: Vec<ItemId>,
    pub modules: Vec<ModuleDecl>,
}

impl Program {
    #[allow(dead_code)] // Used by private qualification until source dispatch activates.
    pub(super) fn uses_owned_syntax(&self, source: &super::source::SourceFile) -> bool {
        let owned_type = |ty: &TypeSyntax| match ty.kind {
            TypeSyntaxKind::Unit => false,
            TypeSyntaxKind::Reference { .. } => true,
            TypeSyntaxKind::Name(name) => !matches!(source.text_at(name), "bool" | "i32"),
        };
        !self.records.is_empty()
            || self.functions.iter().any(|f| {
                owned_type(&f.result)
                    || f.params.iter().any(|p| owned_type(&p.ty))
                    || f.blocks.iter().any(|b| {
                        b.body.iter().any(|s| match &s.kind {
                            StmtKind::Let {
                                annotation: Some(ty),
                                ..
                            } => owned_type(ty),
                            StmtKind::FieldAssign { .. } => true,
                            _ => false,
                        })
                    })
            })
            || self.expressions.iter().any(|e| match &e.kind {
                ExprKind::StructLiteral { .. } | ExprKind::FieldRead { .. } => true,
                ExprKind::Call { args, .. } => {
                    args.iter().any(|a| matches!(a, Argument::Borrow { .. }))
                }
                _ => false,
            })
    }
}
