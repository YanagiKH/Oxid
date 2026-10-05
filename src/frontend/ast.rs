use super::lexer::Token;
use super::source::{SourceFile, Span};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemPath {
    Unqualified(Span),
    Absolute(PathId),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathRoot {
    Crate,
    LocalType,
}
#[derive(Clone, Copy, Debug)]
pub struct QualifiedPath {
    pub span: Span,
    pub segment_start: usize,
    pub segment_len: u8,
    pub root: PathRoot,
}
/// Compatibility spelling for the still-absolute production parser.
pub type AbsolutePath = QualifiedPath;
#[derive(Clone, Copy, Debug)]
pub struct ImportDecl {
    pub path: PathId,
    pub alias: Span,
    pub span: Span,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExprId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyBlockId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArithmeticOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
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
    /// Exact decimal digits, separate literal-only sign, and full origin on Expr.
    Number {
        digits: Span,
        negative: bool,
    },
    Unit,
    Name(Span),
    Call {
        callee: ItemPath,
        args: Vec<Argument>,
    },
    /// Private carrier only. Ordinary parsing does not produce this form yet.
    #[cfg_attr(not(test), allow(dead_code))]
    QualifiedValue {
        path: PathId,
        args: Option<Vec<Argument>>,
    },
    StructLiteral {
        record: ItemPath,
        fields: Vec<FieldInit>,
    },
    FieldRead {
        base: Span,
        /// The bounded dot-separated field path, including retained trivia.
        field: Span,
    },
    ArrayLiteral {
        elements: Vec<ExprId>,
    },
    IndexRead {
        /// A named root, optionally followed by a bounded record field path.
        base: Span,
        index: ExprId,
    },
    ArrayLength {
        base: Span,
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
pub enum ScalarTypeSyntax {
    Bool,
    I32,
    Unit,
}
#[derive(Clone, Copy, Debug)]
pub struct FixedArraySyntax {
    pub element: ScalarTypeSyntax,
    pub length: u16,
}
#[derive(Clone, Copy, Debug)]
pub enum TypeSyntaxKind {
    Name(ItemPath),
    Unit,
    Reference {
        mutable: bool,
        referent: ItemPath,
    },
    Array(FixedArraySyntax),
    SliceReference {
        mutable: bool,
        element: ScalarTypeSyntax,
    },
    ArrayReference {
        mutable: bool,
        array: FixedArraySyntax,
    },
}
#[derive(Clone, Copy, Debug)]
pub struct TypeSyntax {
    pub span: Span,
    pub kind: TypeSyntaxKind,
}
#[derive(Debug)]
pub struct StructField {
    pub public: Option<Span>,
    pub name: Span,
    pub ty: TypeSyntax,
    pub span: Span,
}
#[derive(Debug)]
pub struct StructDecl {
    pub public: Option<Span>,
    pub name: Span,
    pub fields: Vec<StructField>,
    pub span: Span,
    pub end: Span,
}
#[derive(Clone, Copy, Debug)]
pub struct EnumPayloadSyntax {
    #[cfg_attr(not(test), allow(dead_code))] // Read by private structural qualification.
    pub kind: ScalarTypeSyntax,
    pub span: Span,
}
#[derive(Debug)]
pub struct EnumVariantSyntax {
    pub name: Span,
    pub payload: Option<EnumPayloadSyntax>,
    pub span: Span,
}
#[derive(Debug)]
pub struct EnumDecl {
    pub public: Option<Span>,
    pub name: Span,
    pub variants: Vec<EnumVariantSyntax>,
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
    Import(usize),
    Function(usize),
    Struct(usize),
    #[cfg_attr(not(test), allow(dead_code))] // No enum parser production yet.
    Enum(usize),
}
#[derive(Clone, Copy, Debug)]
pub enum BorrowPlace {
    /// Complete bounded named-root field path, or a single whole binding name.
    OwnerName(Span),
    ForwardedParameter {
        name: Span,
        star_span: Span,
    },
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
pub struct MatchArmSyntax {
    pub variant: PathId,
    pub binding: Option<Span>,
    pub body: BodyBlockId,
    pub span: Span,
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
    /// The target is one retained direct IndexRead syntax node, not a load.
    IndexAssign {
        target: ExprId,
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
    #[cfg_attr(not(test), allow(dead_code))] // No match parser production yet.
    Match {
        scrutinee: Span,
        arms: Vec<MatchArmSyntax>,
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
    pub public: Option<Span>,
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
    pub enums: Vec<EnumDecl>,
    pub items: Vec<ItemId>,
    pub modules: Vec<ModuleDecl>,
    pub paths: Vec<QualifiedPath>,
    pub path_segments: Vec<Span>,
    pub imports: Vec<ImportDecl>,
    source: super::parser::SourceProvenance,
    project_syntax: bool,
}

impl Program {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn parsed(
        source: super::parser::SourceProvenance,
        tokens: Vec<Token>,
        functions: Vec<Function>,
        expressions: Vec<Expr>,
        records: Vec<StructDecl>,
        enums: Vec<EnumDecl>,
        items: Vec<ItemId>,
        modules: Vec<ModuleDecl>,
        paths: Vec<QualifiedPath>,
        path_segments: Vec<Span>,
        imports: Vec<ImportDecl>,
    ) -> Self {
        let project_syntax = !modules.is_empty()
            || !imports.is_empty()
            || paths.iter().any(|path| path.root == PathRoot::Crate)
            || functions.iter().any(|function| function.public.is_some())
            || records.iter().any(|record| {
                record.public.is_some() || record.fields.iter().any(|field| field.public.is_some())
            })
            || enums.iter().any(|enumeration| enumeration.public.is_some());
        Self {
            tokens,
            functions,
            expressions,
            records,
            enums,
            items,
            modules,
            paths,
            path_segments,
            imports,
            source,
            project_syntax,
        }
    }
    pub(super) fn belongs_to(&self, source: &SourceFile) -> bool {
        self.source.belongs_to(source)
    }
    pub(super) fn item_path_span(&self, path: ItemPath) -> Option<Span> {
        match path {
            ItemPath::Unqualified(span) => Some(span),
            ItemPath::Absolute(id) => self
                .paths
                .get(id.0)
                .filter(|path| path.root == PathRoot::Crate)
                .map(|path| path.span),
        }
    }
    pub(super) fn path_segments(&self, path: PathId) -> Option<&[Span]> {
        let path = self.paths.get(path.0)?;
        self.path_segments.get(
            path.segment_start
                ..path
                    .segment_start
                    .checked_add(usize::from(path.segment_len))?,
        )
    }
    pub(super) fn uses_project_syntax(&self) -> bool {
        self.project_syntax
    }
    /// Visit every retained source-span occurrence once, and check every arena
    /// edge without indexing it first. Source identity and UTF-8 policy belong
    /// to the caller. No validation allocation or recursive traversal is used.
    #[cfg(test)]
    pub(super) fn validate_spans_and_ids(&self, mut valid: impl FnMut(Span) -> bool) -> bool {
        self.validate_spans_and_ids_counted(|span| span.is_none_or(&mut valid))
    }
    /// None reports an arena/item/edge visit; Some reports a span inspection.
    /// This is the same validation used above, with allocation-free accounting.
    #[cfg(test)]
    pub(super) fn validate_spans_and_ids_counted(
        &self,
        inspect: impl FnMut(Option<Span>) -> bool,
    ) -> bool {
        self.validate_spans_and_ids_counted_with_enum_syntax(inspect, &mut None)
    }
    /// Report the closed feature during the already counted structural walk.
    /// This avoids another unmetered scan of every pre-existing source node.
    pub(super) fn validate_spans_and_ids_counted_with_enum_syntax(
        &self,
        inspect: impl FnMut(Option<Span>) -> bool,
        enum_syntax: &mut Option<Span>,
    ) -> bool {
        *enum_syntax = None;
        let inspect = std::cell::RefCell::new(inspect);
        let mut valid = |span| (inspect.borrow_mut())(Some(span));
        let visit = || (inspect.borrow_mut())(None);
        if !visit() {
            return false;
        }
        let mut project_syntax = !self.modules.is_empty() || !self.imports.is_empty();
        let path_valid = |path: ItemPath, valid: &mut dyn FnMut(Span) -> bool| match path {
            ItemPath::Unqualified(span) => valid(span),
            ItemPath::Absolute(id) => {
                visit()
                    && self
                        .paths
                        .get(id.0)
                        .is_some_and(|path| path.root == PathRoot::Crate)
            }
        };
        let type_valid = |ty: TypeSyntax, valid: &mut dyn FnMut(Span) -> bool| {
            valid(ty.span)
                && match ty.kind {
                    TypeSyntaxKind::Unit | TypeSyntaxKind::SliceReference { .. } => true,
                    TypeSyntaxKind::Array(array) | TypeSyntaxKind::ArrayReference { array, .. } => {
                        usize::from(array.length) <= super::parser::MAX_ARRAY_ELEMENTS
                    }
                    TypeSyntaxKind::Name(path)
                    | TypeSyntaxKind::Reference { referent: path, .. } => path_valid(path, valid),
                }
        };
        let mut path_end = 0;
        let mut previous_path: Option<Span> = None;
        for (index, path) in self.paths.iter().enumerate() {
            if !visit() {
                return false;
            }
            let length = usize::from(path.segment_len);
            project_syntax |= path.root == PathRoot::Crate;
            if path.segment_start != path_end
                || previous_path.is_some_and(|previous| {
                    previous.file != path.span.file || previous.end > path.span.start
                })
                || !(2..=super::parser::MAX_PATH_SEGMENTS).contains(&length)
                || (path.root == PathRoot::LocalType && length != 2)
                || !valid(path.span)
            {
                return false;
            }
            let Some(segments) = self.path_segments(PathId(index)) else {
                return false;
            };
            if segments
                .first()
                .is_none_or(|first| first.start != path.span.start)
                || segments.last().is_none_or(|last| last.end != path.span.end)
            {
                return false;
            }
            let mut segment_end = path.span.start;
            for segment in segments {
                if !visit() {
                    return false;
                }
                if !valid(*segment)
                    || segment.file != path.span.file
                    || segment.start < segment_end
                    || segment.start >= segment.end
                    || segment.end > path.span.end
                {
                    return false;
                }
                segment_end = segment.end;
            }
            previous_path = Some(path.span);
            let Some(end) = path_end.checked_add(length) else {
                return false;
            };
            path_end = end;
        }
        if path_end != self.path_segments.len() {
            return false;
        }
        for token in &self.tokens {
            if !visit() {
                return false;
            }
            if !valid(token.span) {
                return false;
            }
        }
        for module in &self.modules {
            if !visit() {
                return false;
            }
            if !valid(module.name)
                || !valid(module.span)
                || module.public.is_some_and(|span| !valid(span))
            {
                return false;
            }
        }
        for import in &self.imports {
            if !visit() {
                return false;
            }
            if !self
                .paths
                .get(import.path.0)
                .is_some_and(|path| path.root == PathRoot::Crate)
                || !valid(import.alias)
                || !valid(import.span)
            {
                return false;
            }
        }
        let mut counts = [0usize; 5];
        for item in &self.items {
            if !visit() {
                return false;
            }
            let (lane, index, length) = match *item {
                ItemId::Function(index) => (0, index, self.functions.len()),
                ItemId::Struct(index) => (1, index, self.records.len()),
                ItemId::Module(index) => (2, index, self.modules.len()),
                ItemId::Import(index) => (3, index, self.imports.len()),
                ItemId::Enum(index) => (4, index, self.enums.len()),
            };
            if index != counts[lane] || index >= length {
                return false;
            }
            let Some(count) = counts[lane].checked_add(1) else {
                return false;
            };
            counts[lane] = count;
        }
        if counts
            != [
                self.functions.len(),
                self.records.len(),
                self.modules.len(),
                self.imports.len(),
                self.enums.len(),
            ]
        {
            return false;
        }
        for enumeration in &self.enums {
            if !visit() {
                return false;
            }
            enum_syntax.get_or_insert(enumeration.span);
            project_syntax |= enumeration.public.is_some();
            if !valid(enumeration.name)
                || !valid(enumeration.span)
                || !valid(enumeration.end)
                || enumeration.public.is_some_and(|span| !valid(span))
                || !(1..=super::parser::MAX_ENUM_VARIANTS).contains(&enumeration.variants.len())
            {
                return false;
            }
            for variant in &enumeration.variants {
                if !visit()
                    || !valid(variant.name)
                    || !valid(variant.span)
                    || variant
                        .payload
                        .is_some_and(|payload| !visit() || !valid(payload.span))
                {
                    return false;
                }
            }
        }
        for record in &self.records {
            if !visit() {
                return false;
            }
            project_syntax |= record.public.is_some();
            if !valid(record.name)
                || !valid(record.span)
                || !valid(record.end)
                || record.public.is_some_and(|span| !valid(span))
            {
                return false;
            }
            for field in &record.fields {
                if !visit() {
                    return false;
                }
                project_syntax |= field.public.is_some();
                if !valid(field.name)
                    || !valid(field.span)
                    || !type_valid(field.ty, &mut valid)
                    || matches!(
                        field.ty.kind,
                        TypeSyntaxKind::Reference { .. }
                            | TypeSyntaxKind::ArrayReference { .. }
                            | TypeSyntaxKind::SliceReference { .. }
                    )
                    || field.public.is_some_and(|span| !valid(span))
                {
                    return false;
                }
            }
        }
        for function in &self.functions {
            if !visit() {
                return false;
            }
            project_syntax |= function.public.is_some();
            if !valid(function.name)
                || !valid(function.end)
                || !type_valid(function.result, &mut valid)
                || function.public.is_some_and(|span| !valid(span))
                || function.body.0 != 0
                || function.blocks.get(function.body.0).is_none()
            {
                return false;
            }
            for parameter in &function.params {
                if !visit() {
                    return false;
                }
                if !valid(parameter.name) || !type_valid(parameter.ty, &mut valid) {
                    return false;
                }
            }
            for (block_index, block) in function.blocks.iter().enumerate() {
                if !visit() {
                    return false;
                }
                if !valid(block.span) || !valid(block.end) {
                    return false;
                }
                let block_valid = |id: BodyBlockId| {
                    visit() && id.0 > block_index && function.blocks.get(id.0).is_some()
                };
                let expr_valid = |id: ExprId| visit() && self.expressions.get(id.0).is_some();
                for statement in &block.body {
                    if !visit() {
                        return false;
                    }
                    if !valid(statement.span) {
                        return false;
                    }
                    let ok = match &statement.kind {
                        StmtKind::Let {
                            name,
                            annotation,
                            init,
                            ..
                        } => {
                            valid(*name)
                                && annotation.is_none_or(|ty| type_valid(ty, &mut valid))
                                && expr_valid(*init)
                        }
                        StmtKind::Assign {
                            name,
                            operator_span,
                            value,
                        } => valid(*name) && valid(*operator_span) && expr_valid(*value),
                        StmtKind::FieldAssign {
                            base,
                            field,
                            target_span,
                            operator_span,
                            value,
                        } => {
                            valid(*base)
                                && valid(*field)
                                && valid(*target_span)
                                && valid(*operator_span)
                                && expr_valid(*value)
                        }
                        StmtKind::IndexAssign {
                            target,
                            operator_span,
                            value,
                        } => {
                            expr_valid(*target)
                                && matches!(
                                    self.expressions[target.0].kind,
                                    ExprKind::IndexRead { .. }
                                )
                                && valid(*operator_span)
                                && expr_valid(*value)
                        }
                        StmtKind::Expr(expression) => expr_valid(*expression),
                        StmtKind::Return(expression) => expression.is_none_or(expr_valid),
                        StmtKind::Break | StmtKind::Continue => true,
                        StmtKind::While { condition, body } => {
                            expr_valid(*condition) && block_valid(*body)
                        }
                        StmtKind::If {
                            condition,
                            then_block,
                            else_block,
                        } => {
                            expr_valid(*condition)
                                && block_valid(*then_block)
                                && else_block.is_none_or(block_valid)
                        }
                        StmtKind::Match { scrutinee, arms } => {
                            enum_syntax.get_or_insert(statement.span);
                            valid(*scrutinee)
                                && (1..=super::parser::MAX_ENUM_VARIANTS).contains(&arms.len())
                                && arms.iter().all(|arm| {
                                    visit()
                                        && visit()
                                        && self.paths.get(arm.variant.0).is_some_and(|path| {
                                            path.root == PathRoot::LocalType
                                                || path.segment_len >= 3
                                        })
                                        && arm.binding.is_none_or(&mut valid)
                                        && valid(arm.span)
                                        && block_valid(arm.body)
                                })
                        }
                    };
                    if !ok {
                        return false;
                    }
                }
            }
        }
        for (index, expression) in self.expressions.iter().enumerate() {
            if !visit() {
                return false;
            }
            if !valid(expression.span) {
                return false;
            }
            let earlier = |id: ExprId| visit() && id.0 < index;
            let ok = match &expression.kind {
                ExprKind::Negate {
                    operand,
                    operator_span,
                }
                | ExprKind::Not {
                    operand,
                    operator_span,
                } => earlier(*operand) && valid(*operator_span),
                ExprKind::Logical {
                    left,
                    right,
                    operator_span,
                    ..
                }
                | ExprKind::Comparison {
                    left,
                    right,
                    operator_span,
                    ..
                }
                | ExprKind::Arithmetic {
                    left,
                    right,
                    operator_span,
                    ..
                } => earlier(*left) && earlier(*right) && valid(*operator_span),
                ExprKind::Bool(_) | ExprKind::Unit => true,
                ExprKind::Number { digits, .. } => valid(*digits),
                ExprKind::Name(name) => valid(*name),
                ExprKind::Call { callee, args } => {
                    path_valid(*callee, &mut valid)
                        && args.iter().all(|arg| {
                            if !visit() {
                                return false;
                            }
                            match arg {
                                Argument::Value(id) => earlier(*id),
                                Argument::Borrow { place, span, .. } => {
                                    valid(*span)
                                        && match place {
                                            BorrowPlace::OwnerName(name) => valid(*name),
                                            BorrowPlace::ForwardedParameter { name, star_span } => {
                                                valid(*name) && valid(*star_span)
                                            }
                                        }
                                }
                            }
                        })
                }
                ExprKind::QualifiedValue { path, args } => {
                    enum_syntax.get_or_insert(expression.span);
                    visit()
                        && self.paths.get(path.0).is_some()
                        && args.as_ref().is_none_or(|args| {
                            args.len() <= super::parser::MAX_PARAMS
                                && args.iter().all(|arg| {
                                    if !visit() {
                                        return false;
                                    }
                                    match arg {
                                        Argument::Value(id) => earlier(*id),
                                        Argument::Borrow { place, span, .. } => {
                                            valid(*span)
                                                && match place {
                                                    BorrowPlace::OwnerName(name) => valid(*name),
                                                    BorrowPlace::ForwardedParameter {
                                                        name,
                                                        star_span,
                                                    } => valid(*name) && valid(*star_span),
                                                }
                                        }
                                    }
                                })
                        })
                }
                ExprKind::StructLiteral { record, fields } => {
                    path_valid(*record, &mut valid)
                        && fields.iter().all(|field| {
                            if !visit() {
                                return false;
                            }
                            valid(field.name) && valid(field.span) && earlier(field.value)
                        })
                }
                ExprKind::FieldRead { base, field } => valid(*base) && valid(*field),
                ExprKind::ArrayLiteral { elements } => {
                    elements.len() <= super::parser::MAX_ARRAY_ELEMENTS
                        && elements.iter().all(|element| earlier(*element))
                }
                ExprKind::IndexRead { base, index } => valid(*base) && earlier(*index),
                ExprKind::ArrayLength { base } => valid(*base),
                ExprKind::Group(inner) => earlier(*inner),
            };
            if !ok {
                return false;
            }
        }
        visit() && project_syntax == self.project_syntax
    }
    #[allow(dead_code)] // Used by private qualification until source dispatch activates.
    pub(super) fn uses_owned_syntax(&self, source: &super::source::SourceFile) -> bool {
        let owned_type = |ty: &TypeSyntax| match ty.kind {
            TypeSyntaxKind::Unit => false,
            TypeSyntaxKind::Reference { .. } => true,
            TypeSyntaxKind::Array(_)
            | TypeSyntaxKind::ArrayReference { .. }
            | TypeSyntaxKind::SliceReference { .. } => true,
            TypeSyntaxKind::Name(ItemPath::Unqualified(name)) => {
                !matches!(source.text_at(name), "bool" | "i32")
            }
            TypeSyntaxKind::Name(ItemPath::Absolute(_)) => true,
        };
        !self.records.is_empty()
            || !self.enums.is_empty()
            || self.functions.iter().any(|f| {
                owned_type(&f.result)
                    || f.params.iter().any(|p| owned_type(&p.ty))
                    || f.blocks.iter().any(|b| {
                        b.body.iter().any(|s| match &s.kind {
                            StmtKind::Let {
                                annotation: Some(ty),
                                ..
                            } => owned_type(ty),
                            StmtKind::FieldAssign { .. }
                            | StmtKind::IndexAssign { .. }
                            | StmtKind::Match { .. } => true,
                            _ => false,
                        })
                    })
            })
            || self.expressions.iter().any(|e| match &e.kind {
                ExprKind::StructLiteral { .. }
                | ExprKind::FieldRead { .. }
                | ExprKind::ArrayLiteral { .. }
                | ExprKind::IndexRead { .. }
                | ExprKind::ArrayLength { .. } => true,
                ExprKind::Call { args, .. } => {
                    args.iter().any(|a| matches!(a, Argument::Borrow { .. }))
                }
                ExprKind::QualifiedValue { path, args } => {
                    self.paths
                        .get(path.0)
                        .is_some_and(|path| path.root == PathRoot::LocalType)
                        || args.as_ref().is_some_and(|args| {
                            args.iter()
                                .any(|arg| matches!(arg, Argument::Borrow { .. }))
                        })
                }
                _ => false,
            })
    }
}
