//! Resolved source identities and immutable producer views, never an ownership proof.
pub(super) use crate::frontend::ast::{ArithmeticOp, ComparisonOp, LogicalOp};
pub(super) use crate::frontend::hir::{DefId, Ty};
pub(super) use crate::frontend::oir::owned_types::{
    AggregateTy, BorrowKind, FieldId, ParameterTy, RecordId, ValueTy,
};
use crate::frontend::source::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BindingId(pub(super) usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ExprId(pub(super) usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BodyBlockId(pub(super) usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LoopId(pub(super) usize);

#[derive(Debug)]
pub(super) struct Record {
    pub(super) id: RecordId,
    pub(super) name_span: Span,
    pub(super) span: Span,
    pub(super) fields: Vec<Field>,
    pub(super) end: Span,
}
#[derive(Debug)]
pub(super) struct Field {
    pub(super) id: FieldId,
    pub(super) ty: Ty,
    pub(super) name_span: Span,
    pub(super) span: Span,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BorrowPlace {
    Owner(BindingId),
    Forwarded(BindingId),
}
#[derive(Debug)]
pub(super) enum Argument {
    Value(ExprId),
    Borrow {
        kind: BorrowKind,
        place: BorrowPlace,
        span: Span,
        name_span: Span,
        star_span: Option<Span>,
    },
}
#[derive(Debug)]
pub(super) struct FieldInit {
    pub(super) field: FieldId,
    pub(super) value: ExprId,
    pub(super) span: Span,
}
#[derive(Debug)]
pub(super) enum ExprKind {
    Bool(bool),
    I32(i32),
    Unit,
    Binding(BindingId),
    Group(ExprId),
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
    Arithmetic {
        op: ArithmeticOp,
        left: ExprId,
        right: ExprId,
        operator_span: Span,
    },
    Call {
        target: DefId,
        args: Vec<Argument>,
    },
    StructLiteral {
        record: RecordId,
        fields: Vec<FieldInit>,
    },
    FieldRead {
        base: BindingId,
        base_span: Span,
        field_span: Span,
    },
}
#[derive(Debug)]
pub(super) struct Expr {
    pub(super) kind: ExprKind,
    pub(super) span: Span,
}
#[derive(Debug)]
pub(super) struct Binding {
    pub(super) mutable: bool,
    pub(super) span: Span,
    pub(super) annotation: Option<ValueTy>,
    pub(super) scope: BodyBlockId,
    pub(super) parameter_position: Option<usize>,
}
#[derive(Debug)]
pub(super) enum StmtKind {
    Let {
        binding: BindingId,
        init: ExprId,
    },
    Assign {
        binding: BindingId,
        target_span: Span,
        operator_span: Span,
        value: ExprId,
    },
    FieldAssign {
        base: BindingId,
        base_span: Span,
        field_span: Span,
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
pub(super) struct Stmt {
    pub(super) kind: StmtKind,
    pub(super) span: Span,
}
#[derive(Debug)]
pub(super) struct Signature {
    pub(super) params: Vec<ParameterTy>,
    pub(super) result: ValueTy,
    pub(super) span: Span,
}
#[derive(Debug)]
pub(super) struct BodyBlock {
    pub(super) body: Vec<Stmt>,
    pub(super) span: Span,
    pub(super) end: Span,
}
#[derive(Debug)]
pub(super) struct Function {
    pub(super) id: DefId,
    pub(super) bindings: Vec<Binding>,
    pub(super) expressions: Vec<Expr>,
    pub(super) body: BodyBlockId,
    pub(super) blocks: Vec<BodyBlock>,
    pub(super) end: Span,
}
/// Type-validated access category. Borrow permission is retained, not checked here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AccessBase {
    Owner(BindingId),
    Reference {
        binding: BindingId,
        kind: BorrowKind,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Projection {
    pub(super) base: AccessBase,
    pub(super) field: FieldId,
}
