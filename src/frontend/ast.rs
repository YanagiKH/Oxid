mod generic_syntax;
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
    Std,
    LocalType,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QualifiedPath {
    pub span: Span,
    pub segment_start: usize,
    pub segment_len: u8,
    pub root: PathRoot,
}
/// Compatibility spelling for the still-absolute production parser.
pub type AbsolutePath = QualifiedPath;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportDecl {
    pub path: PathId,
    pub alias: Span,
    pub span: Span,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExprId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyBlockId(pub usize);
/// Closed named-receiver scalar conversions; no general cast or method dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversionOp {
    ToCharChecked,
    CharToU32,
    ToFloat(FloatTypeSyntax),
    FloatToBits,
    FloatSqrt,
    FloatFromBits(FloatTypeSyntax),
    ToU8Checked,
    ToI32,
    ToI64,
    ToI32Checked,
    ToIntegerChecked(IntegerTypeSyntax),
}
/// Integer spellings added after the retained i32/u8/i64 source contracts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum IntegerTypeSyntax {
    I8,
    U8,
    I16,
    I32,
    I64,
    U16,
    U32,
    U64,
    Isize,
    Usize,
    I128,
    U128,
}
impl IntegerTypeSyntax {
    pub const ADDITIONAL_LITERAL_SUFFIXES: [Self; 10] = [
        Self::I8,
        Self::U8,
        Self::I16,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::Isize,
        Self::Usize,
        Self::I128,
        Self::U128,
    ];
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::U8 => "u8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::Isize => "isize",
            Self::Usize => "usize",
            Self::I128 => "i128",
            Self::U128 => "u128",
        }
    }
    pub fn from_spelling(text: &str) -> Option<Self> {
        match text {
            "i8" => Some(Self::I8),
            "u8" => Some(Self::U8),
            "i16" => Some(Self::I16),
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "u16" => Some(Self::U16),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "isize" => Some(Self::Isize),
            "usize" => Some(Self::Usize),
            "i128" => Some(Self::I128),
            "u128" => Some(Self::U128),
            _ => None,
        }
    }
    pub const fn bits(self) -> u8 {
        match self {
            Self::I8 | Self::U8 => 8,
            Self::I16 | Self::U16 => 16,
            Self::I32 | Self::U32 => 32,
            Self::I64 | Self::U64 | Self::Isize | Self::Usize => 64,
            Self::I128 | Self::U128 => 128,
        }
    }
    pub const fn signed(self) -> bool {
        matches!(
            self,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 | Self::Isize | Self::I128
        )
    }
    pub const fn checked_conversion(self) -> &'static str {
        match self {
            Self::I8 => "to_i8_checked",
            Self::U8 => "to_u8_checked",
            Self::I16 => "to_i16_checked",
            Self::I32 => "to_i32_checked",
            Self::I64 => "to_i64_checked",
            Self::U16 => "to_u16_checked",
            Self::U32 => "to_u32_checked",
            Self::U64 => "to_u64_checked",
            Self::Isize => "to_isize_checked",
            Self::Usize => "to_usize_checked",
            Self::I128 => "to_i128_checked",
            Self::U128 => "to_u128_checked",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatTypeSyntax {
    F32,
    F64,
}
impl FloatTypeSyntax {
    pub fn from_spelling(text: &str) -> Option<Self> {
        match text {
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            _ => None,
        }
    }
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArithmeticOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitwiseOp {
    And,
    Or,
    Xor,
    ShiftLeft,
    ShiftRight,
}
impl BitwiseOp {
    pub const fn shift(self) -> bool {
        matches!(self, Self::ShiftLeft | Self::ShiftRight)
    }
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
#[derive(Debug, PartialEq, Eq)]
pub enum ExprKind {
    /// A local reference initializer. Borrowed values are not ordinary values.
    Borrow {
        mutable: bool,
        name: Span,
        forwarded: bool,
    },
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
    },
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
    Character(char),
    /// Exact decimal digits, separate literal-only sign, and full origin on Expr.
    Number {
        digits: Span,
        negative: bool,
    },
    I64Number {
        digits: Span,
        negative: bool,
    },
    FloatNumber {
        digits: Span,
        negative: bool,
        kind: FloatTypeSyntax,
    },
    IntegerNumber {
        digits: Span,
        negative: bool,
        kind: IntegerTypeSyntax,
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
    GenericVariant {
        application: GenericApplicationId,
        variant: Span,
        args: Option<Vec<Argument>>,
    },
    FieldRead {
        base: Span,
        /// The bounded dot-separated field path, including retained trivia.
        field: Span,
    },
    ArrayLiteral {
        elements: Vec<ExprId>,
    },
    ByteString {
        contents: Span,
        length: u16,
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
#[derive(Debug, PartialEq, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarTypeSyntax {
    Bool,
    I32,
    Unit,
    /// The payload span itself is the unqualified nominal type name.
    Name,
    /// Compact index into the existing bounded qualified-path table.
    Absolute(u32),
    Applied(u32),
    Array(FixedArraySyntax),
    I64,
    Integer(IntegerTypeSyntax),
    Float(FloatTypeSyntax),
    Char,
}
/// Scalar elements in the existing fixed-array grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrayElementTypeSyntax {
    Bool,
    I32,
    U8,
    Unit,
    I64,
    Integer(IntegerTypeSyntax),
    Float(FloatTypeSyntax),
    Char,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedArraySyntax {
    pub element: ArrayElementTypeSyntax,
    pub length: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeSyntaxKind {
    Name(ItemPath),
    Applied(GenericApplicationId),
    Unit,
    Reference {
        mutable: bool,
        referent: ItemPath,
    },
    AppliedReference {
        mutable: bool,
        application: GenericApplicationId,
    },
    Array(FixedArraySyntax),
    SliceReference {
        mutable: bool,
        element: ArrayElementTypeSyntax,
    },
    /// Named slice syntax is confined to parameters. Semantic resolution must
    /// establish an admitted scalar or a Copy-bound type parameter.
    GenericSliceReference {
        mutable: bool,
        element: ItemPath,
    },
    ArrayReference {
        mutable: bool,
        array: FixedArraySyntax,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeSyntax {
    pub span: Span,
    pub kind: TypeSyntaxKind,
}
#[derive(Debug, PartialEq, Eq)]
pub struct StructField {
    pub public: Option<Span>,
    pub name: Span,
    pub ty: TypeSyntax,
    pub span: Span,
}
#[derive(Debug, PartialEq, Eq)]
pub struct GenericParam {
    pub name: Span,
    pub bounds: Vec<ItemPath>,
    pub span: Span,
}
#[derive(Debug, PartialEq, Eq)]
pub struct GenericHeader {
    pub declaration: Span,
    pub parameters: Vec<GenericParam>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenericApplicationId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenericArgumentContext {
    Expression,
    Type,
}
#[derive(Debug, PartialEq, Eq)]
pub struct GenericArguments {
    pub callee: Span,
    pub path: ItemPath,
    pub arguments: Vec<TypeSyntax>,
    pub span: Span,
    pub context: GenericArgumentContext,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct GenericSyntax {
    pub headers: Vec<GenericHeader>,
    pub calls: Vec<GenericArguments>,
}
#[derive(PartialEq, Eq)]
pub struct StructDecl {
    pub derive_copy: Option<Span>,
    pub public: Option<Span>,
    pub name: Span,
    pub fields: Vec<StructField>,
    pub span: Span,
    pub end: Span,
}
impl std::fmt::Debug for StructDecl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = formatter.debug_struct("StructDecl");
        if let Some(mark) = self.derive_copy {
            out.field("derive_copy", &mark);
        }
        out.field("public", &self.public)
            .field("name", &self.name)
            .field("fields", &self.fields)
            .field("span", &self.span)
            .field("end", &self.end)
            .finish()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnumPayloadSyntax {
    #[cfg_attr(not(test), allow(dead_code))] // Read by private structural qualification.
    pub kind: ScalarTypeSyntax,
    pub span: Span,
}
impl EnumPayloadSyntax {
    pub fn value_type(self) -> TypeSyntax {
        TypeSyntax {
            span: self.span,
            kind: match self.kind {
                ScalarTypeSyntax::Bool
                | ScalarTypeSyntax::I32
                | ScalarTypeSyntax::I64
                | ScalarTypeSyntax::Float(_)
                | ScalarTypeSyntax::Char
                | ScalarTypeSyntax::Integer(_)
                | ScalarTypeSyntax::Name => TypeSyntaxKind::Name(ItemPath::Unqualified(self.span)),
                ScalarTypeSyntax::Unit => TypeSyntaxKind::Unit,
                ScalarTypeSyntax::Absolute(id) => {
                    TypeSyntaxKind::Name(ItemPath::Absolute(PathId(id as usize)))
                }
                ScalarTypeSyntax::Applied(id) => {
                    TypeSyntaxKind::Applied(GenericApplicationId(id as usize))
                }
                ScalarTypeSyntax::Array(array) => TypeSyntaxKind::Array(array),
            },
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct EnumVariantSyntax {
    pub name: Span,
    pub payload: Option<EnumPayloadSyntax>,
    pub span: Span,
}
#[derive(Debug, PartialEq, Eq)]
pub struct EnumDecl {
    pub public: Option<Span>,
    pub name: Span,
    pub variants: Vec<EnumVariantSyntax>,
    pub span: Span,
    pub end: Span,
}
/// Private source-discovery grammar; no namespace or runtime semantics yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModuleDecl {
    pub name: Span,
    pub public: Option<Span>,
    #[allow(dead_code)] // Full original declaration origin for the staged shared index.
    pub span: Span,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemId {
    Module(usize),
    Import(usize),
    Function(usize),
    Struct(usize),
    #[cfg_attr(not(test), allow(dead_code))] // No enum parser production yet.
    Enum(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorrowPlace {
    /// Complete bounded named-root field path, or a single whole binding name.
    OwnerName(Span),
    ForwardedParameter {
        name: Span,
        star_span: Span,
    },
    Range {
        name: Span,
        start: ExprId,
        end: ExprId,
        star: Option<u32>,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub enum Argument {
    Value(ExprId),
    Borrow {
        mutable: bool,
        place: BorrowPlace,
        span: Span,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub struct FieldInit {
    pub name: Span,
    pub value: ExprId,
    pub span: Span,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Param {
    pub name: Span,
    pub ty: TypeSyntax,
}
#[derive(Debug, PartialEq, Eq)]
pub struct MatchArmSyntax {
    pub variant: PathId,
    pub binding: Option<Span>,
    pub body: BodyBlockId,
    pub span: Span,
}
#[derive(Debug, PartialEq, Eq)]
pub struct RecordPatternField {
    pub field: Span,
    pub binding: Span,
    pub span: Span,
}
#[derive(Debug, PartialEq, Eq)]
pub enum StmtKind {
    DestructureRecord {
        record: ItemPath,
        fields: Vec<RecordPatternField>,
        init: ExprId,
    },
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
    ForRange {
        name: Span,
        start: ExprId,
        end: ExprId,
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
#[derive(Debug, PartialEq, Eq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}
#[derive(Debug, PartialEq, Eq)]
pub struct BodyBlock {
    pub body: Vec<Stmt>,
    pub span: Span,
    pub end: Span,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Function {
    pub public: Option<Span>,
    pub name: Span,
    pub params: Vec<Param>,
    pub result: TypeSyntax,
    pub body: BodyBlockId,
    pub blocks: Vec<BodyBlock>,
    pub end: Span,
}
/// Parsed feature summary, reconciled by the counted source-validation walk.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum ProjectSyntax {
    Original,
    Project,
    StdImports,
    OriginalRanges,
    ProjectRanges,
    StdImportsRanges,
}
impl ProjectSyntax {
    fn with_ranges(self, ranges: bool) -> Self {
        if !ranges {
            return self;
        }
        match self {
            Self::Original => Self::OriginalRanges,
            Self::Project => Self::ProjectRanges,
            Self::StdImports => Self::StdImportsRanges,
            other => other,
        }
    }
    fn new(project: bool, std_imports: bool) -> Self {
        if std_imports {
            Self::StdImports
        } else if project {
            Self::Project
        } else {
            Self::Original
        }
    }
}
impl std::fmt::Debug for ProjectSyntax {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // This explicit representation conversion preserves the historical
        // project_syntax boolean. Std roots remain visible in every AST path.
        std::fmt::Debug::fmt(
            &(!matches!(self, Self::Original | Self::OriginalRanges)),
            formatter,
        )
    }
}
const _: () = {
    assert!(std::mem::size_of::<ProjectSyntax>() == std::mem::size_of::<bool>());
    assert!(std::mem::align_of::<ProjectSyntax>() == std::mem::align_of::<bool>());
};

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
    project_syntax: ProjectSyntax,
    // Set during parsing, never recomputed from the publicly mutable AST.
    parsed_has_named_slice: bool,
    parsed_has_copy_records: bool,
}

impl std::fmt::Debug for Program {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = formatter.debug_struct("Program");
        out.field("tokens", &self.tokens);
        out.field("functions", &self.functions);
        out.field("expressions", &self.expressions);
        out.field("records", &self.records);
        out.field("enums", &self.enums);
        out.field("items", &self.items);
        out.field("modules", &self.modules);
        out.field("paths", &self.paths);
        out.field("path_segments", &self.path_segments);
        out.field("imports", &self.imports);
        if !self.generic_headers().is_empty() {
            out.field("generic_headers", &self.generic_headers());
        }
        if !self.generic_calls().is_empty() {
            out.field("generic_calls", &self.generic_calls());
        }
        out.field("source", &self.source);
        out.field("project_syntax", &self.project_syntax);
        out.finish()
    }
}

/// Summaries produced only by the counted validator, never a second AST walk.
#[derive(Default)]
pub(super) struct SyntaxFeatures {
    pub element_ranges: bool,
    pub generic_slices: bool,
    /// At least one primitive or named slice used source-token authentication.
    pub source_slices: bool,
}

/// Source authority is optional for compatibility validators. Every slice in
/// a source-backed validation uses this cursor, irrespective of its AST variant.
struct SliceSourceTokens<'s, 'v> {
    program: &'s Program,
    source: &'s SourceFile,
    inspect: &'v mut dyn FnMut(Option<Span>) -> bool,
    span: Span,
    cursor: usize,
    previous_end: usize,
}
impl SliceSourceTokens<'_, '_> {
    fn find_start(&mut self) -> bool {
        let mut search = 0..self.program.tokens.len();
        while search.start < search.end {
            // Pay before each token access/comparison in the binary search.
            if !(self.inspect)(None) {
                return false;
            }
            let middle = search.start + (search.end - search.start) / 2;
            match self.program.tokens[middle].span.start.cmp(&self.span.start) {
                std::cmp::Ordering::Less => search.start = middle + 1,
                std::cmp::Ordering::Greater => search.end = middle,
                std::cmp::Ordering::Equal => {
                    self.cursor = middle;
                    return true;
                }
            }
        }
        false
    }
    fn text_equal(&mut self, left: &str, right: &str) -> Option<bool> {
        if !(self.inspect)(None) {
            return None;
        }
        if left.len() != right.len() {
            return Some(false);
        }
        for (left, right) in left.bytes().zip(right.bytes()) {
            if !(self.inspect)(None) {
                return None;
            }
            if left != right {
                return Some(false);
            }
        }
        Some(true)
    }
    fn trivia(&mut self, text: &str) -> bool {
        // Authenticate skipped trivia rather than trusting a forgeable token
        // kind. This reproduces only the lexer's whitespace/comment shapes.
        // Prepay every raw byte before the bounded, allocation-free check.
        for _ in text.bytes() {
            if !(self.inspect)(None) {
                return false;
            }
        }
        if text.starts_with("//") {
            !text.contains('\n')
        } else if let Some(body) = text.strip_prefix("/*") {
            body.len() >= 2 && body.find("*/") == Some(body.len() - 2)
        } else {
            !text.is_empty() && text.chars().all(char::is_whitespace)
        }
    }
    fn take(
        &mut self,
        kind: super::lexer::Kind,
        spelling: Option<&str>,
        span: Option<Span>,
    ) -> Option<Token> {
        use super::lexer::Kind;
        loop {
            // Every token, including trivia and refused tokens, is paid before
            // inspection. Contiguity prevents omitted source bytes authorizing
            // a different type or mutability.
            if !(self.inspect)(None) {
                return None;
            }
            let token = *self.program.tokens.get(self.cursor)?;
            if !(self.inspect)(Some(token.span))
                || token.span.file != self.span.file
                || token.span.start != self.previous_end
                || token.span.start >= token.span.end
                || token.span.end > self.span.end
            {
                return None;
            }
            let text = self.source.try_text(token.span)?;
            self.cursor = self.cursor.checked_add(1)?;
            self.previous_end = token.span.end;
            if token.kind == Kind::Trivia {
                if !self.trivia(text) {
                    return None;
                }
                continue;
            }
            if token.kind != kind || span.is_some_and(|span| token.span != span) {
                return None;
            }
            if let Some(spelling) = spelling {
                if !self.text_equal(text, spelling)? {
                    return None;
                }
            }
            return Some(token);
        }
    }
}

// Supplemental controls are named by their actual Rust types, not borrowed
// words from the historical fixed-state bank. No allocation or limit grows.
// The index charges the feature row on every collection, and the source row
// only after the paid walk reports a source-authenticated slice. Source-free
// compatibility wrappers admit only primitive slices and do not use the cursor.
pub(super) const SYNTAX_FEATURE_CONTROL_BYTES: usize = size_of::<SyntaxFeatures>()                  // per-module summary
    + size_of::<&mut SyntaxFeatures>()           // validator output handle
    + size_of::<Option<&SourceFile>>()           // source-authority argument
    + size_of::<&Option<&SourceFile>>()          // type closure's source capture
    + size_of::<[bool; 2]>()                     // collector aggregate flags
    + size_of::<[&mut bool; 2]>(); // type closure's named/source-slice feature handles
pub(super) const SLICE_SOURCE_CONTROL_BYTES: usize = size_of::<SliceSourceTokens<'static, 'static>>()
    + size_of::<TypeSyntax>()                    // complete authentication request
    + size_of::<ItemPath>()                      // current named element identity
    + size_of::<ArrayElementTypeSyntax>()        // primitive element identity
    + size_of::<bool>()                          // primitive token-match result
    + size_of::<Option<&str>>()                  // primitive spelling or unit discriminator
    + size_of::<bool>()                          // extracted source mutability
    + size_of::<super::lexer::Kind>()            // expected token kind
    + size_of::<&SourceFile>()                   // authentication source argument
    + size_of::<&Program>()                      // authentication program receiver
    + size_of::<[&mut SliceSourceTokens<'static, 'static>; 2]>() // take/leaf receivers
    + size_of::<std::ops::Range<usize>>()         // binary-search bounds
    + size_of::<usize>()                         // binary-search midpoint
    + size_of::<Option<&Token>>()                // checked token lookup
    + size_of::<Token>()                        // current token
    + size_of::<[Option<Token>; 2]>()            // colon/closing-token results
    + size_of::<Option<Span>>()                  // expected source-token identity
    + size_of::<Option<&str>>()                  // optional expected spelling
    + size_of::<Option<&str>>()                  // source text lookup
    + size_of::<[&str; 2]>()                     // comparison operands
    + size_of::<std::iter::Zip<std::str::Bytes<'static>, std::str::Bytes<'static>>>()
    + size_of::<(u8, u8)>()                      // current comparison bytes
    + size_of::<Option<bool>>()                  // comparison/refusal result
    + size_of::<std::str::Bytes<'static>>()      // trivia prepayment cursor
    + size_of::<std::str::Chars<'static>>()      // whitespace inspection cursor
    + size_of::<Option<&str>>()                  // block-comment body
    + size_of::<Option<usize>>()                 // block-comment terminator
    + size_of::<std::array::IntoIter<&str, 16>>() // concrete-name candidates
    + size_of::<Option<&QualifiedPath>>()        // checked absolute-path row
    + size_of::<Option<&[Span]>>()               // checked path-segment slice
    + size_of::<std::iter::Enumerate<std::slice::Iter<'static, Span>>>()
    + size_of::<(usize, &Span)>()                // current path segment
    + size_of::<&std::cell::RefCell<&mut dyn FnMut(Option<Span>) -> bool>>() // callback capture
    + size_of::<&mut dyn FnMut(Option<Span>) -> bool>(); // callback transport

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
        parsed_has_named_slice: bool,
    parsed_has_copy_records: bool,
    ) -> Self {
        let mut project_syntax =
            !modules.is_empty() || !imports.is_empty() || source.has_generics();
        let mut std_import_syntax = false;
        let range_syntax = functions.iter().any(|f| {
            f.blocks.iter().any(|b| {
                b.body
                    .iter()
                    .any(|s| matches!(s.kind, StmtKind::ForRange { .. }))
            })
        });
        // Both private summaries are derived during bounded AST construction.
        // The index never needs an unmetered path/import scan to read them.
        for path in &paths {
            project_syntax |= matches!(path.root, PathRoot::Crate | PathRoot::Std);
            std_import_syntax |= path.root == PathRoot::Std;
        }
        project_syntax |= functions.iter().any(|function| function.public.is_some())
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
            project_syntax: ProjectSyntax::new(project_syntax, std_import_syntax)
                .with_ranges(range_syntax),
            parsed_has_named_slice,
            parsed_has_copy_records,
        }
    }
    /// This immutable parse fact survives edits to any TypeSyntaxKind. It only
    /// selects full source authentication; it grants no slice/type authority.
    pub(super) fn has_parsed_named_slice(&self) -> bool {
        self.parsed_has_named_slice
    }
    /// Immutable parse authority selects the paid full syntax comparison even
    /// when a public AST mark is removed or attached to a different record.
    pub(super) fn has_parsed_copy_records(&self) -> bool {
        self.parsed_has_copy_records
    }
    pub(super) fn syntax_matches(&self, other: &Self) -> bool {
        self.tokens == other.tokens
            && self.functions == other.functions
            && self.expressions == other.expressions
            && self.records == other.records
            && self.enums == other.enums
            && self.items == other.items
            && self.modules == other.modules
            && self.paths == other.paths
            && self.path_segments == other.path_segments
            && self.imports == other.imports
            && self.generic_headers() == other.generic_headers()
            && self.generic_calls() == other.generic_calls()
            && self.project_syntax == other.project_syntax
            && self.parsed_has_named_slice == other.parsed_has_named_slice
            && self.parsed_has_copy_records == other.parsed_has_copy_records
    }
    pub(super) fn belongs_to(&self, source: &SourceFile) -> bool {
        self.source.belongs_to(source) && self.generic_headers_match_source(source)
    }
    /// Authenticate both primitive and named slice representations inside the
    /// paid type walk. An AST relabel cannot decide whether authentication runs.
    /// Provenance queries deliberately do not rescan parameters or tokens.
    fn slice_matches_source(
        &self,
        source: &SourceFile,
        ty: TypeSyntax,
        inspect: &mut dyn FnMut(Option<Span>) -> bool,
    ) -> bool {
        use super::lexer::Kind;
        let mutable = match ty.kind {
            TypeSyntaxKind::GenericSliceReference { mutable, .. }
            | TypeSyntaxKind::SliceReference { mutable, .. } => mutable,
            _ => return false,
        };
        if !inspect(None)
            || !self.source.belongs_to(source)
            || !inspect(Some(ty.span))
            || source.try_text(ty.span).is_none()
        {
            return false;
        }
        let mut tokens = SliceSourceTokens {
            program: self,
            source,
            inspect,
            span: ty.span,
            cursor: 0,
            previous_end: ty.span.start,
        };
        if !tokens.find_start()
            || tokens.take(Kind::Ampersand, Some("&"), None).is_none()
            || (mutable && tokens.take(Kind::Mut, Some("mut"), None).is_none())
            || tokens.take(Kind::Unsupported, Some("["), None).is_none()
        {
            return false;
        }
        if let TypeSyntaxKind::SliceReference { element, .. } = ty.kind {
            let spelling = match element {
                ArrayElementTypeSyntax::Bool => Some("bool"),
                ArrayElementTypeSyntax::I32 => Some("i32"),
                ArrayElementTypeSyntax::U8 => Some("u8"),
                ArrayElementTypeSyntax::Unit => None,
                ArrayElementTypeSyntax::I64 => Some("i64"),
                ArrayElementTypeSyntax::Integer(kind) => Some(kind.spelling()),
                ArrayElementTypeSyntax::Float(kind) => Some(kind.spelling()),
                ArrayElementTypeSyntax::Char => Some("char"),
            };
            let element_matches = match spelling {
                Some(spelling) => tokens.take(Kind::Ident, Some(spelling), None).is_some(),
                None => {
                    tokens.take(Kind::LParen, Some("("), None).is_some()
                        && tokens.take(Kind::RParen, Some(")"), None).is_some()
                }
            };
            return element_matches
                && tokens
                    .take(Kind::Unsupported, Some("]"), None)
                    .is_some_and(|close| close.span.end == ty.span.end);
        }
        let TypeSyntaxKind::GenericSliceReference { element, .. } = ty.kind else {
            return false;
        };
        match element {
            ItemPath::Unqualified(span) => {
                if !(tokens.inspect)(None) || !(tokens.inspect)(Some(span)) {
                    return false;
                }
                let Some(name) = source.try_text(span) else {
                    return false;
                };
                // Scalar spellings keep the existing SliceReference identity.
                // Every candidate and byte comparison is separately prepaid.
                for scalar in [
                    "bool", "i32", "u8", "i64", "char", "i8", "i16", "u16", "u32", "u64", "isize",
                    "usize", "i128", "u128", "f32", "f64",
                ] {
                    if !(tokens.inspect)(None) {
                        return false;
                    }
                    match tokens.text_equal(name, scalar) {
                        Some(false) => {}
                        Some(true) | None => return false,
                    }
                }
                if tokens.take(Kind::Ident, None, Some(span)).is_none() {
                    return false;
                }
            }
            ItemPath::Absolute(id) => {
                if !(tokens.inspect)(None) {
                    return false;
                }
                let Some(path) = self.paths.get(id.0) else {
                    return false;
                };
                if !(tokens.inspect)(None) || !(tokens.inspect)(Some(path.span)) {
                    return false;
                }
                let Some(segments) = self.path_segments(id) else {
                    return false;
                };
                if path.root != PathRoot::Crate
                    || !(2..=super::parser::MAX_PATH_SEGMENTS).contains(&segments.len())
                    || segments
                        .first()
                        .is_none_or(|span| span.start != path.span.start)
                    || segments.last().is_none_or(|span| span.end != path.span.end)
                {
                    return false;
                }
                for (position, segment) in segments.iter().enumerate() {
                    if !(tokens.inspect)(None) || !(tokens.inspect)(Some(*segment)) {
                        return false;
                    }
                    if position != 0 {
                        let Some(first) = tokens.take(Kind::Colon, Some(":"), None) else {
                            return false;
                        };
                        let Some(second) = tokens.take(Kind::Colon, Some(":"), None) else {
                            return false;
                        };
                        if first.span.end != second.span.start {
                            return false;
                        }
                    }
                    if tokens
                        .take(
                            Kind::Ident,
                            (position == 0).then_some("crate"),
                            Some(*segment),
                        )
                        .is_none()
                    {
                        return false;
                    }
                }
            }
        }
        tokens
            .take(Kind::Unsupported, Some("]"), None)
            .is_some_and(|close| close.span.end == ty.span.end)
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
        !matches!(
            self.project_syntax,
            ProjectSyntax::Original | ProjectSyntax::OriginalRanges
        )
    }
    pub(super) fn uses_std_imports(&self) -> bool {
        matches!(
            self.project_syntax,
            ProjectSyntax::StdImports | ProjectSyntax::StdImportsRanges
        )
    }
    pub(super) fn has_range_for(&self) -> bool {
        matches!(
            self.project_syntax,
            ProjectSyntax::OriginalRanges
                | ProjectSyntax::ProjectRanges
                | ProjectSyntax::StdImportsRanges
        )
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
    #[cfg(test)]
    pub(super) fn validate_spans_and_ids_counted_with_enum_syntax(
        &self,
        inspect: impl FnMut(Option<Span>) -> bool,
        enum_syntax: &mut Option<Span>,
    ) -> bool {
        self.validate_spans_and_ids_counted_with_syntax(
            inspect,
            enum_syntax,
            super::parser::StdImportPolicy::Closed,
            &mut false,
        )
    }
    #[cfg(test)]
    pub(super) fn validate_spans_and_ids_counted_with_syntax(
        &self,
        inspect: impl FnMut(Option<Span>) -> bool,
        enum_syntax: &mut Option<Span>,
        std_policy: super::parser::StdImportPolicy,
        std_syntax: &mut bool,
    ) -> bool {
        self.validate_spans_and_ids_counted_with_features(
            None,
            inspect,
            enum_syntax,
            std_policy,
            std_syntax,
            &mut SyntaxFeatures::default(),
        )
    }
    /// Reconcile ranges and authenticate new source-bearing types during the
    /// same paid walk. Callers without source authority retain the old grammar.
    /// The sealed index retains the summary; consumers never rescan the AST.
    pub(super) fn validate_spans_and_ids_counted_with_features(
        &self,
        source: Option<&SourceFile>,
        inspect: impl FnMut(Option<Span>) -> bool,
        enum_syntax: &mut Option<Span>,
        std_policy: super::parser::StdImportPolicy,
        std_syntax: &mut bool,
        features: &mut SyntaxFeatures,
    ) -> bool {
        *features = SyntaxFeatures::default();
        *enum_syntax = None;
        *std_syntax = false;
        let inspect = std::cell::RefCell::new(inspect);
        let mut valid = |span| (inspect.borrow_mut())(Some(span));
        let visit = || (inspect.borrow_mut())(None);
        if !visit() {
            return false;
        }
        let mut project_syntax =
            !self.modules.is_empty() || !self.imports.is_empty() || self.has_generics();
        let mut range_syntax = false;
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
        for header in self.generic_headers() {
            if !visit()
                || !valid(header.declaration)
                || header.parameters.is_empty()
                || header.parameters.len() > super::parser::MAX_PARAMS
            {
                return false;
            }
            for parameter in &header.parameters {
                if !visit()
                    || !valid(parameter.name)
                    || !valid(parameter.span)
                    || parameter.bounds.len() > super::parser::MAX_PARAMS
                    || parameter
                        .bounds
                        .iter()
                        .any(|bound| !visit() || !path_valid(*bound, &mut valid))
                {
                    return false;
                }
            }
        }
        let mut type_valid = |ty: TypeSyntax, valid: &mut dyn FnMut(Span) -> bool| {
            valid(ty.span)
                && match ty.kind {
                    TypeSyntaxKind::Unit => true,
                    TypeSyntaxKind::SliceReference { .. } => source.is_none_or(|source| {
                        features.source_slices = true;
                        self.slice_matches_source(source, ty, &mut |span| {
                            (inspect.borrow_mut())(span)
                        })
                    }),
                    TypeSyntaxKind::Array(array) | TypeSyntaxKind::ArrayReference { array, .. } => {
                        usize::from(array.length) <= super::parser::MAX_ARRAY_ELEMENTS
                    }
                    TypeSyntaxKind::Name(path)
                    | TypeSyntaxKind::Reference { referent: path, .. } => path_valid(path, valid),
                    TypeSyntaxKind::GenericSliceReference { element, .. } => {
                        features.generic_slices = true;
                        path_valid(element, valid)
                            && self.item_path_span(element).is_some_and(|element| {
                                element.file == ty.span.file
                                    && ty.span.start < element.start
                                    && element.start < element.end
                                    && element.end < ty.span.end
                            })
                            && source.is_some_and(|source| {
                                features.source_slices = true;
                                self.slice_matches_source(source, ty, &mut |span| {
                                    (inspect.borrow_mut())(span)
                                })
                            })
                    }
                    TypeSyntaxKind::Applied(id)
                    | TypeSyntaxKind::AppliedReference {
                        application: id, ..
                    } => self.generic_application(id).is_some_and(|app| {
                        app.context == GenericArgumentContext::Type && path_valid(app.path, valid)
                    }),
                }
        };
        for call in self.generic_calls() {
            if !visit()
                || !valid(call.callee)
                || !valid(call.span)
                || !match (call.context, call.path) {
                    (GenericArgumentContext::Expression, ItemPath::Absolute(id)) => {
                        visit()
                            && self.paths.get(id.0).is_some_and(|path| {
                                matches!(path.root, PathRoot::Crate | PathRoot::LocalType)
                            })
                    }
                    _ => path_valid(call.path, &mut valid),
                }
                || call.arguments.is_empty()
                || call.arguments.len() > super::parser::MAX_PARAMS
                || call.arguments.iter().any(|ty| {
                    !visit()
                        || matches!(ty.kind, TypeSyntaxKind::GenericSliceReference { .. })
                        || !type_valid(*ty, &mut valid)
                })
            {
                return false;
            }
        }
        let mut std_paths = 0usize;
        let mut std_imports = 0usize;
        let mut path_end = 0;
        let mut previous_path: Option<Span> = None;
        for (index, path) in self.paths.iter().enumerate() {
            if !visit() {
                return false;
            }
            let length = usize::from(path.segment_len);
            project_syntax |= matches!(path.root, PathRoot::Crate | PathRoot::Std);
            if path.root == PathRoot::Std {
                *std_syntax = true;
                if !std_policy.enabled() {
                    return false;
                }
                let Some(next) = std_paths.checked_add(1) else {
                    return false;
                };
                std_paths = next;
            }
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
            if self
                .paths
                .get(import.path.0)
                .is_some_and(|path| path.root == PathRoot::Std)
            {
                let Some(next) = std_imports.checked_add(1) else {
                    return false;
                };
                std_imports = next;
            }
            if !self.paths.get(import.path.0).is_some_and(|path| {
                path.root == PathRoot::Crate || (std_policy.enabled() && path.root == PathRoot::Std)
            }) || !valid(import.alias)
                || !valid(import.span)
            {
                return false;
            }
        }
        // The checked import inventory separately checks strict source order,
        // so these equal cardinalities also exclude detached/reused std rows.
        if std_paths != std_imports {
            return false;
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
                    || variant.payload.is_some_and(|payload| {
                        !visit()
                            || !valid(payload.span)
                            || match payload.kind {
                                ScalarTypeSyntax::Absolute(id) => {
                                    !path_valid(ItemPath::Absolute(PathId(id as usize)), &mut valid)
                                }
                                ScalarTypeSyntax::Array(array) => {
                                    usize::from(array.length) > super::parser::MAX_ARRAY_ELEMENTS
                                }
                                _ => false,
                            }
                    })
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
            if record.derive_copy.is_some_and(|mark| {
                !self.parsed_has_copy_records || !valid(mark)
            }) || !valid(record.name)
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
                            | TypeSyntaxKind::GenericSliceReference { .. }
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
                || matches!(
                    function.result.kind,
                    TypeSyntaxKind::GenericSliceReference { .. }
                )
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
                        StmtKind::DestructureRecord {
                            record,
                            fields,
                            init,
                        } => {
                            path_valid(*record, &mut valid)
                                && fields.iter().all(|field| {
                                    visit()
                                        && valid(field.field)
                                        && valid(field.binding)
                                        && valid(field.span)
                                })
                                && expr_valid(*init)
                        }
                        StmtKind::Let {
                            name,
                            annotation,
                            init,
                            ..
                        } => {
                            valid(*name)
                                && annotation.is_none_or(|ty| {
                                    !matches!(ty.kind, TypeSyntaxKind::GenericSliceReference { .. })
                                        && type_valid(ty, &mut valid)
                                })
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
                        StmtKind::ForRange {
                            name,
                            start,
                            end,
                            body,
                        } => {
                            range_syntax = true;
                            enum_syntax.get_or_insert(statement.span);
                            valid(*name)
                                && expr_valid(*start)
                                && expr_valid(*end)
                                && block_valid(*body)
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
                                                || (path.root == PathRoot::Crate
                                                    && path.segment_len >= 3)
                                        })
                                        && arm.binding.is_none_or(valid)
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
                ExprKind::Borrow { name, .. } => {
                    valid(*name)
                        && name.file == expression.span.file
                        && name.start >= expression.span.start
                        && name.end <= expression.span.end
                }
                // Structural ID/span admission shares the established unary
                // traversal. The source-association witness independently checks
                // the named receiver, operation and exact authenticated origins.
                ExprKind::Conversion {
                    operand,
                    name_span: operator_span,
                    ..
                }
                | ExprKind::Negate {
                    operand,
                    operator_span,
                }
                | ExprKind::BitNot {
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
                | ExprKind::Bitwise {
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
                ExprKind::Bool(_) | ExprKind::Unit | ExprKind::Character(_) => true,
                ExprKind::Number { digits, .. }
                | ExprKind::I64Number { digits, .. }
                | ExprKind::FloatNumber { digits, .. }
                | ExprKind::IntegerNumber { digits, .. } => valid(*digits),
                ExprKind::Name(name) => valid(*name),
                ExprKind::GenericVariant {
                    application,
                    variant,
                    args,
                } => {
                    enum_syntax.get_or_insert(expression.span);
                    self.generic_application(*application)
                        .is_some_and(|app| app.context == GenericArgumentContext::Expression)
                        && valid(*variant)
                        && args.iter().flatten().all(|arg| match arg {
                            Argument::Value(id) => earlier(*id),
                            Argument::Borrow { .. } => false,
                        })
                }
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
                                            BorrowPlace::Range {
                                                name,
                                                start,
                                                end,
                                                star,
                                            } => {
                                                features.element_ranges = true;
                                                valid(*name)
                                                    && earlier(*start)
                                                    && earlier(*end)
                                                    && star.is_none_or(|start| {
                                                        valid(Span {
                                                            file: name.file,
                                                            start: start as usize,
                                                            end: start as usize + 1,
                                                        })
                                                    })
                                            }
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
                        && self
                            .paths
                            .get(path.0)
                            .is_some_and(|path| path.root != PathRoot::Std)
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
                                                    BorrowPlace::Range {
                                                        name,
                                                        start,
                                                        end,
                                                        star,
                                                    } => {
                                                        features.element_ranges = true;
                                                        valid(*name)
                                                            && earlier(*start)
                                                            && earlier(*end)
                                                            && star.is_none_or(|start| {
                                                                valid(Span {
                                                                    file: name.file,
                                                                    start: start as usize,
                                                                    end: start as usize + 1,
                                                                })
                                                            })
                                                    }
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
                ExprKind::ByteString { contents, length } => {
                    valid(*contents)
                        && contents.file == expression.span.file
                        && contents.start.checked_sub(2) == Some(expression.span.start)
                        && contents.end.checked_add(1) == Some(expression.span.end)
                        && usize::from(*length) <= super::byte_strings::MAX_BYTES
                }
                ExprKind::IndexRead { base, index } => valid(*base) && earlier(*index),
                ExprKind::ArrayLength { base } => valid(*base),
                ExprKind::Group(inner) => earlier(*inner),
            };
            if !ok {
                return false;
            }
        }
        visit()
            && ProjectSyntax::new(project_syntax, *std_syntax).with_ranges(range_syntax)
                == self.project_syntax
    }
    /// Float literals and annotations select the current typed checker.
    pub(super) fn uses_float_syntax(&self, source: &SourceFile) -> bool {
        let float_type = |ty: &TypeSyntax| {
            matches!(ty.kind,
            TypeSyntaxKind::Name(ItemPath::Unqualified(name)) if FloatTypeSyntax::from_spelling(source.text_at(name)).is_some())
        };
        self.expressions.iter().any(|e| matches!(e.kind, ExprKind::FloatNumber { .. }))
            || self.functions.iter().any(|f| float_type(&f.result)
                || f.params.iter().any(|p| float_type(&p.ty))
                || f.blocks.iter().any(|b| b.body.iter().any(|s| matches!(&s.kind, StmtKind::Let { annotation: Some(ty), .. } if float_type(ty)))))
    }
    pub(super) fn uses_character_syntax(&self, source: &SourceFile) -> bool {
        let character_type = |ty: &TypeSyntax| {
            matches!(ty.kind,
            TypeSyntaxKind::Name(ItemPath::Unqualified(name)) if source.text_at(name) == "char")
        };
        self.expressions.iter().any(|e| matches!(e.kind, ExprKind::Character(_)))
            || self.functions.iter().any(|f| character_type(&f.result)
                || f.params.iter().any(|p| character_type(&p.ty))
                || f.blocks.iter().any(|b| b.body.iter().any(|s| matches!(&s.kind, StmtKind::Let { annotation: Some(ty), .. } if character_type(ty)))))
    }
    pub(super) fn uses_bitwise_syntax(&self) -> bool {
        self.expressions.iter().any(|expression| {
            matches!(
                expression.kind,
                ExprKind::Bitwise { .. } | ExprKind::BitNot { .. }
            )
        })
    }
    pub(super) fn uses_byte_scalars(&self, source: &SourceFile) -> bool {
        let byte_type = |ty: &TypeSyntax| {
            matches!(ty.kind,
            TypeSyntaxKind::Name(ItemPath::Unqualified(name)) if source.text_at(name) == "u8")
        };
        self.functions.iter().any(|function| {
            byte_type(&function.result)
                || function.params.iter().any(|parameter| byte_type(&parameter.ty))
                || function.blocks.iter().any(|block| block.body.iter().any(|statement|
                    matches!(&statement.kind, StmtKind::Let { annotation: Some(ty), .. } if byte_type(ty))))
        }) || self.expressions.iter().any(|expression|
            matches!(expression.kind, ExprKind::Conversion { op: ConversionOp::ToU8Checked | ConversionOp::ToI32, .. }))
    }

    #[allow(dead_code)] // Used by private qualification until source dispatch activates.
    pub(super) fn uses_owned_syntax(&self, source: &super::source::SourceFile) -> bool {
        let owned_type = |ty: &TypeSyntax| match ty.kind {
            TypeSyntaxKind::Unit => false,
            TypeSyntaxKind::Applied(_) | TypeSyntaxKind::AppliedReference { .. } => true,
            TypeSyntaxKind::Reference { .. } => true,
            TypeSyntaxKind::Array(_)
            | TypeSyntaxKind::ArrayReference { .. }
            | TypeSyntaxKind::SliceReference { .. }
            | TypeSyntaxKind::GenericSliceReference { .. } => true,
            TypeSyntaxKind::Name(ItemPath::Unqualified(name)) => {
                !matches!(source.text_at(name), "bool" | "i32" | "u8")
            }
            TypeSyntaxKind::Name(ItemPath::Absolute(_)) => true,
        };
        self.parsed_has_copy_records
            || !self.records.is_empty()
            || !self.enums.is_empty()
            || self.uses_std_imports()
            || self.functions.iter().any(|f| {
                owned_type(&f.result)
                    || f.params.iter().any(|p| owned_type(&p.ty))
                    || f.blocks.iter().any(|b| {
                        b.body.iter().any(|s| match &s.kind {
                            StmtKind::Let {
                                annotation: Some(ty),
                                ..
                            } => owned_type(ty),
                            StmtKind::DestructureRecord { .. }
                            | StmtKind::FieldAssign { .. }
                            | StmtKind::IndexAssign { .. }
                            | StmtKind::ForRange { .. }
                            | StmtKind::Match { .. } => true,
                            _ => false,
                        })
                    })
            })
            || self.expressions.iter().any(|e| match &e.kind {
                ExprKind::Borrow { .. } => true,
                ExprKind::StructLiteral { .. }
                | ExprKind::GenericVariant { .. }
                | ExprKind::FieldRead { .. }
                | ExprKind::ArrayLiteral { .. }
                | ExprKind::ByteString { .. }
                | ExprKind::IndexRead { .. }
                | ExprKind::ArrayLength { .. }
                | ExprKind::I64Number { .. }
                | ExprKind::IntegerNumber { .. }
                | ExprKind::Conversion {
                    op:
                        ConversionOp::ToI64
                        | ConversionOp::ToI32Checked
                        | ConversionOp::ToIntegerChecked(_),
                    ..
                } => true,
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

#[cfg(test)]
mod builtin_feature_tests {
    use super::*;
    use crate::frontend::{lexer, parser, project::budget::Allocator, source::SourceMap};

    #[test]
    fn builtin_parsed_feature_summary_is_exact_and_has_one_byte_layout() {
        for text in [
            "fn main()->(){return;}",
            "use crate::m::f; fn main()->(){return;}",
            "use std::io::read_stdin; fn main()->(){return;}",
        ] {
            let mut sources = SourceMap::new();
            let file = sources.add("feature-summary.ox".into(), text.into());
            let source = sources.get(file);
            let (mut program, _) = parser::parse_typed_counted(
                source,
                lexer::lex(source).unwrap(),
                parser::SourceMode::ProjectCandidate,
                parser::MAX_NODES,
                &mut Allocator::default(),
                &mut Default::default(),
            )
            .unwrap();
            let expected = program.project_syntax;
            for state in [
                ProjectSyntax::Original,
                ProjectSyntax::Project,
                ProjectSyntax::StdImports,
            ] {
                program.project_syntax = state;
                let valid = program.validate_spans_and_ids_counted_with_syntax(
                    |at| at.is_none_or(|at| source.try_text(at).is_some()),
                    &mut None,
                    parser::StdImportPolicy::Enabled,
                    &mut false,
                );
                assert_eq!(valid, state == expected);
            }
            program.project_syntax = expected;
            assert_eq!(
                format!("{:?}", program.project_syntax),
                format!("{}", program.uses_project_syntax())
            );
        }
        println!(
            "builtin-feature-layout feature={}/{} program={}/{}",
            std::mem::size_of::<ProjectSyntax>(),
            std::mem::align_of::<ProjectSyntax>(),
            std::mem::size_of::<Program>(),
            std::mem::align_of::<Program>()
        );
        assert_eq!(std::mem::size_of::<ProjectSyntax>(), 1);
    }
}
