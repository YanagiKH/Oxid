//! Resolved source identities and immutable producer views, never an ownership proof.
pub(super) use crate::frontend::ast::{
    ArithmeticOp, BitwiseOp, ComparisonOp, ConversionOp, LogicalOp,
};
pub(super) use crate::frontend::hir::{DefId, Ty};
pub(super) use crate::frontend::oir::owned_types::{
    AggregateTy, BorrowKind, BorrowedTy, FieldId, FixedArrayTy, ParameterTy, RecordId, ValueTy,
    VariantId,
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
    pub(super) ty: ValueTy,
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
    BorrowRange {
        kind: BorrowKind,
        place: BorrowPlace,
        span: Span,
        name_span: Span,
        start: ExprId,
        end: ExprId,
    },
    Borrow {
        kind: BorrowKind,
        place: BorrowPlace,
        span: Span,
        // Complete named-root path; no separately allocated syntactic path.
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
    BitNot {
        operand: ExprId,
        operator_span: Span,
    },
    Bitwise {
        op: BitwiseOp,
        left: ExprId,
        right: ExprId,
        operator_span: Span,
    },
    Conversion {
        op: ConversionOp,
        operand: ExprId,
        name_span: Span,
        source_expr: crate::frontend::ast::ExprId,
    },
    Bool(bool),
    Character(char),
    I32(i32),
    I64(i64),
    Float {
        bits: u64,
        ty: Ty,
    },
    Integer {
        bits: u64,
        ty: Ty,
    },
    Integer128 {
        low: u64,
        high: u64,
        ty: Ty,
    },
    Unit,
    Binding(BindingId),
    Group(ExprId),
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
    /// Passive C3a carrier; no source entrypoint constructs it yet.
    #[allow(dead_code)]
    ConstructEnum {
        variant: VariantId,
        payload: Option<ExprId>,
    },
    StructLiteral {
        record: RecordId,
        fields: Vec<FieldInit>,
    },
    ArrayLiteral {
        elements: Vec<ExprId>,
    },
    ByteString {
        bytes: Vec<u8>,
    },
    IndexRead {
        base: BindingId,
        base_span: Span,
        index: ExprId,
    },
    ArrayLength {
        base: BindingId,
        base_span: Span,
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
/// Written-order arm metadata. Its body owns the scalar payload binding.
#[derive(Debug)]
#[allow(dead_code)] // Measured before enum source resolution is admitted.
pub(super) struct MatchArm {
    pub(super) variant: VariantId,
    pub(super) binding: Option<BindingId>,
    pub(super) body: BodyBlockId,
    pub(super) span: Span,
}
#[derive(Debug)]
pub(super) struct RecordPatternField {
    pub(super) field: FieldId,
    pub(super) binding: BindingId,
    pub(super) span: Span,
}
#[derive(Debug)]
pub(super) enum StmtKind {
    DestructureRecord {
        record: RecordId,
        fields: Vec<RecordPatternField>,
        init: ExprId,
    },
    /// A named owner, never an expression that would move it before dispatch.
    #[allow(dead_code)]
    Match {
        scrutinee: BindingId,
        arms: Vec<MatchArm>,
    },
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
    IndexAssign {
        base: BindingId,
        base_span: Span,
        target_span: Span,
        operator_span: Span,
        value: ExprId,
        index: ExprId,
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Projection {
    pub(super) base: AccessBase,
    pub(super) field: FieldId,
    pub(super) path: Vec<FieldId>,
}

// Complete record-pattern fields share the already priced initializer carrier.
const _: [(); std::mem::size_of::<RecordPatternField>()] = [(); std::mem::size_of::<FieldInit>()];
