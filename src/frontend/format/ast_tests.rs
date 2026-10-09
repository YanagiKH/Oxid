//! Independent structural oracle for formatter tests. This deliberately uses
//! typed, exhaustive AST matches rather than Debug output or formatter events.

use super::format_source;
use crate::frontend::{
    ast::*,
    lexer::{self, Kind, Token},
    parser::{self, SourceMode},
    source::{SourceFile, SourceMap, Span},
};

#[derive(Debug, PartialEq, Eq)]
enum Origin {
    Covered {
        first: usize,
        end: usize,
        first_byte: usize,
        last_byte: usize,
    },
    Empty {
        next: usize,
        byte: usize,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum Part {
    Tag(&'static str),
    Count(usize),
    Flag(bool),
    Text(String),
    Token(Kind, String),
    Span(Origin),
}

// Raw arena positions are storage details. Number nodes at first encounter
// through the source-ordered item roots, retaining sharing and all edge order.
struct Ids {
    canonical: Vec<Option<usize>>,
    next: usize,
}

impl Ids {
    fn new(length: usize) -> Self {
        Self {
            canonical: vec![None; length],
            next: 0,
        }
    }

    fn visit(&mut self, raw: usize) -> (usize, bool) {
        if let Some(id) = self.canonical[raw] {
            return (id, false);
        }
        let id = self.next;
        self.next += 1;
        self.canonical[raw] = Some(id);
        (id, true)
    }

    fn assert_complete(&self) {
        assert_eq!(self.next, self.canonical.len(), "unvisited arena entries");
    }
}

struct Fingerprint<'a> {
    source: &'a SourceFile,
    program: &'a Program,
    tokens: Vec<Token>,
    expressions: Ids,
    paths: Ids,
    parts: Vec<Part>,
}

impl<'a> Fingerprint<'a> {
    fn new(source: &'a SourceFile, program: &'a Program) -> Self {
        assert!(program.belongs_to(source));
        Self {
            source,
            program,
            tokens: program
                .tokens
                .iter()
                .copied()
                .filter(|token| !matches!(token.kind, Kind::Trivia | Kind::Eof))
                .collect(),
            expressions: Ids::new(program.expressions.len()),
            paths: Ids::new(program.paths.len()),
            parts: Vec::new(),
        }
    }

    fn tag(&mut self, tag: &'static str) {
        self.parts.push(Part::Tag(tag));
    }
    fn count(&mut self, count: usize) {
        self.parts.push(Part::Count(count));
    }
    fn flag(&mut self, flag: bool) {
        self.parts.push(Part::Flag(flag));
    }

    // Ignore trivia at either edge, but retain the covered token interval and
    // byte boundaries inside its first and last tokens. Identical spellings at
    // different token positions must never normalize to the same origin.
    fn origin(&self, span: Span) -> Origin {
        assert!(self.source.try_text(span).is_some(), "invalid AST origin");
        let first = self
            .tokens
            .partition_point(|token| token.span.end <= span.start);
        let end = self
            .tokens
            .partition_point(|token| token.span.start < span.end);
        if span.start < span.end && first < end {
            Origin::Covered {
                first,
                end,
                first_byte: span.start.saturating_sub(self.tokens[first].span.start),
                last_byte: span.end.min(self.tokens[end - 1].span.end)
                    - self.tokens[end - 1].span.start,
            }
        } else {
            Origin::Empty {
                next: first,
                byte: self
                    .tokens
                    .get(first)
                    .map_or(0, |token| span.start.saturating_sub(token.span.start)),
            }
        }
    }

    fn span(&mut self, span: Span) {
        self.parts.push(Part::Span(self.origin(span)));
    }

    fn spelling(&mut self, span: Span) {
        self.span(span);
        self.parts
            .push(Part::Text(self.source.text_at(span).to_owned()));
    }

    fn access_path(&mut self, span: Span) {
        self.span(span);
        // Unlike one-token names, retained named-root/field paths contain
        // formatter-owned gaps. Preserve each covered lexical token and the
        // token-relative origin, not trivia spelling inside the path.
        let spelling = self
            .tokens
            .iter()
            .filter(|token| token.span.start >= span.start && token.span.end <= span.end)
            .map(|token| self.source.text_at(token.span))
            .collect::<String>();
        self.parts.push(Part::Text(spelling));
    }

    fn optional_span(&mut self, span: Option<Span>) {
        self.flag(span.is_some());
        if let Some(span) = span {
            self.span(span);
        }
    }

    fn path(&mut self, path: ItemPath) {
        match path {
            ItemPath::Unqualified(name) => {
                self.tag("unqualified");
                self.spelling(name);
            }
            ItemPath::Absolute(id) => {
                self.tag("absolute");
                self.absolute(id);
            }
        }
    }

    fn absolute(&mut self, id: PathId) {
        let (canonical, first) = self.paths.visit(id.0);
        self.count(canonical);
        self.flag(first);
        if !first {
            return;
        }
        let program = self.program;
        let AbsolutePath {
            span,
            segment_start,
            segment_len,
            root,
        } = program.paths[id.0];
        self.tag(match root {
            PathRoot::Crate => "crate-root",
            PathRoot::LocalType => "local-type-root",
            PathRoot::Std => "std-import-root",
        });
        self.span(span);
        let segment_len = usize::from(segment_len);
        self.count(segment_len);
        for &segment in &program.path_segments[segment_start..segment_start + segment_len] {
            self.spelling(segment);
        }
    }

    fn array_type(&mut self, array: FixedArraySyntax) {
        let FixedArraySyntax { element, length } = array;
        self.tag(match element {
            ScalarTypeSyntax::Bool => "bool-element",
            ScalarTypeSyntax::I32 => "i32-element",
            ScalarTypeSyntax::Unit => "unit-element",
        });
        self.count(usize::from(length));
    }

    fn ty(&mut self, ty: TypeSyntax) {
        let TypeSyntax { span, kind } = ty;
        self.span(span);
        match kind {
            TypeSyntaxKind::Name(path) => {
                self.tag("name-type");
                self.path(path);
            }
            TypeSyntaxKind::Unit => self.tag("unit-type"),
            TypeSyntaxKind::Array(array) => {
                self.tag("array-type");
                self.array_type(array);
            }
            TypeSyntaxKind::SliceReference { mutable, element } => {
                self.tag("slice-reference-type");
                self.flag(mutable);
                self.tag(match element {
                    ScalarTypeSyntax::Bool => "bool",
                    ScalarTypeSyntax::I32 => "i32",
                    ScalarTypeSyntax::Unit => "unit",
                });
            }
            TypeSyntaxKind::ArrayReference { mutable, array } => {
                self.tag("array-reference-type");
                self.flag(mutable);
                self.array_type(array);
            }
            TypeSyntaxKind::Reference { mutable, referent } => {
                self.tag("reference-type");
                self.flag(mutable);
                self.path(referent);
            }
        }
    }

    fn argument(&mut self, argument: &Argument) {
        match argument {
            Argument::Value(value) => {
                self.tag("value-argument");
                self.expression(*value);
            }
            Argument::Borrow {
                mutable,
                place,
                span,
            } => {
                self.tag("borrow-argument");
                self.flag(*mutable);
                self.span(*span);
                match *place {
                    BorrowPlace::OwnerName(name) => {
                        self.tag("owner");
                        self.access_path(name);
                    }
                    BorrowPlace::ForwardedParameter { name, star_span } => {
                        self.tag("forwarded");
                        self.access_path(name);
                        self.span(star_span);
                    }
                }
            }
        }
    }

    fn expression(&mut self, id: ExprId) {
        let (canonical, first) = self.expressions.visit(id.0);
        self.count(canonical);
        self.flag(first);
        if !first {
            return;
        }
        let program = self.program;
        let Expr { kind, span } = &program.expressions[id.0];
        self.span(*span);
        match kind {
            ExprKind::Conversion {
                op,
                operand,
                name_span,
            } => {
                self.tag(match op {
                    ConversionOp::ToU8Checked => "to-u8-checked",
                    ConversionOp::ToI32 => "to-i32",
                });
                self.spelling(*name_span);
                self.expression(*operand);
            }
            ExprKind::Negate {
                operand,
                operator_span,
            } => {
                self.tag("negate");
                self.span(*operator_span);
                self.expression(*operand);
            }
            ExprKind::Not {
                operand,
                operator_span,
            } => {
                self.tag("not");
                self.span(*operator_span);
                self.expression(*operand);
            }
            ExprKind::Logical {
                op,
                left,
                right,
                operator_span,
            } => {
                self.tag(match op {
                    LogicalOp::And => "and",
                    LogicalOp::Or => "or",
                });
                self.span(*operator_span);
                self.expression(*left);
                self.expression(*right);
            }
            ExprKind::Comparison {
                op,
                left,
                right,
                operator_span,
            } => {
                self.tag(match op {
                    ComparisonOp::Equal => "equal",
                    ComparisonOp::NotEqual => "not-equal",
                    ComparisonOp::Less => "less",
                    ComparisonOp::LessEqual => "less-equal",
                    ComparisonOp::Greater => "greater",
                    ComparisonOp::GreaterEqual => "greater-equal",
                });
                self.span(*operator_span);
                self.expression(*left);
                self.expression(*right);
            }
            ExprKind::Bool(value) => {
                self.tag("bool");
                self.flag(*value);
            }
            ExprKind::Number { digits, negative } => {
                self.tag("number");
                self.spelling(*digits);
                self.flag(*negative);
            }
            ExprKind::Unit => self.tag("unit"),
            ExprKind::Name(name) => {
                self.tag("name");
                self.spelling(*name);
            }
            ExprKind::Call { callee, args } => {
                self.tag("call");
                self.path(*callee);
                self.count(args.len());
                for argument in args {
                    self.argument(argument);
                }
            }
            ExprKind::QualifiedValue { path, args } => {
                self.tag("qualified-value");
                self.absolute(*path);
                self.flag(args.is_some());
                if let Some(args) = args {
                    self.count(args.len());
                    for argument in args {
                        self.argument(argument);
                    }
                }
            }
            ExprKind::StructLiteral { record, fields } => {
                self.tag("record-literal");
                self.path(*record);
                self.count(fields.len());
                for FieldInit { name, value, span } in fields {
                    self.spelling(*name);
                    self.span(*span);
                    self.expression(*value);
                }
            }
            ExprKind::FieldRead { base, field } => {
                self.tag("field-read");
                self.spelling(*base);
                self.access_path(*field);
            }
            ExprKind::ArrayLiteral { elements } => {
                self.tag("array-literal");
                self.count(elements.len());
                for element in elements {
                    self.expression(*element);
                }
            }
            ExprKind::IndexRead { base, index } => {
                self.tag("index-read");
                self.access_path(*base);
                self.expression(*index);
            }
            ExprKind::ArrayLength { base } => {
                self.tag("array-length");
                self.access_path(*base);
            }
            ExprKind::Group(inner) => {
                self.tag("group");
                self.expression(*inner);
            }
            ExprKind::Arithmetic {
                op,
                left,
                right,
                operator_span,
            } => {
                self.tag(match op {
                    ArithmeticOp::Add => "add",
                    ArithmeticOp::Subtract => "subtract",
                    ArithmeticOp::Multiply => "multiply",
                    ArithmeticOp::Divide => "div",
                    ArithmeticOp::Remainder => "rem",
                });
                self.span(*operator_span);
                self.expression(*left);
                self.expression(*right);
            }
        }
    }

    fn statement(&mut self, statement: &Stmt, function: &Function, blocks: &mut Ids) {
        let Stmt { kind, span } = statement;
        self.span(*span);
        match kind {
            StmtKind::Let {
                mutable,
                name,
                annotation,
                init,
            } => {
                self.tag("let");
                self.flag(*mutable);
                self.spelling(*name);
                self.flag(annotation.is_some());
                if let Some(ty) = annotation {
                    self.ty(*ty);
                }
                self.expression(*init);
            }
            StmtKind::Assign {
                name,
                operator_span,
                value,
            } => {
                self.tag("assign");
                self.spelling(*name);
                self.span(*operator_span);
                self.expression(*value);
            }
            StmtKind::FieldAssign {
                base,
                field,
                target_span,
                operator_span,
                value,
            } => {
                self.tag("field-assign");
                self.spelling(*base);
                self.access_path(*field);
                self.span(*target_span);
                self.span(*operator_span);
                self.expression(*value);
            }
            StmtKind::IndexAssign {
                target,
                operator_span,
                value,
            } => {
                self.tag("index-assign");
                self.expression(*target);
                self.span(*operator_span);
                self.expression(*value);
            }
            StmtKind::Expr(value) => {
                self.tag("expression-statement");
                self.expression(*value);
            }
            StmtKind::Return(value) => {
                self.tag("return");
                self.flag(value.is_some());
                if let Some(value) = value {
                    self.expression(*value);
                }
            }
            StmtKind::Break => self.tag("break"),
            StmtKind::Continue => self.tag("continue"),
            StmtKind::Match { scrutinee, arms } => {
                self.tag("match");
                self.spelling(*scrutinee);
                self.count(arms.len());
                for arm in arms {
                    self.absolute(arm.variant);
                    self.optional_span(arm.binding);
                    if let Some(binding) = arm.binding {
                        self.spelling(binding);
                    }
                    self.span(arm.span);
                    self.block(function, blocks, arm.body);
                }
            }
            StmtKind::While { condition, body } => {
                self.tag("while");
                self.expression(*condition);
                self.block(function, blocks, *body);
            }
            StmtKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.tag("if");
                self.expression(*condition);
                self.block(function, blocks, *then_block);
                self.flag(else_block.is_some());
                if let Some(id) = else_block {
                    self.block(function, blocks, *id);
                }
            }
        }
    }

    fn block(&mut self, function: &Function, blocks: &mut Ids, id: BodyBlockId) {
        let (canonical, first) = blocks.visit(id.0);
        self.count(canonical);
        self.flag(first);
        if !first {
            return;
        }
        let BodyBlock { body, span, end } = &function.blocks[id.0];
        self.span(*span);
        self.span(*end);
        self.count(body.len());
        for statement in body {
            self.statement(statement, function, blocks);
        }
    }

    fn program(mut self) -> Vec<Part> {
        let program = self.program;
        self.tag("program");
        self.flag(program.uses_project_syntax());
        for length in [
            program.functions.len(),
            program.expressions.len(),
            program.records.len(),
            program.enums.len(),
            program.items.len(),
            program.modules.len(),
            program.paths.len(),
            program.path_segments.len(),
            program.imports.len(),
        ] {
            self.count(length);
        }
        // Token contents are semantic leaves; trivia/comment preservation is
        // checked separately. AST and origin checks below are not replaced by
        // this token tape check (see same-tape mutation controls).
        for token in &self.tokens {
            self.parts.push(Part::Token(
                token.kind,
                self.source.text_at(token.span).to_owned(),
            ));
        }
        self.tag("items");
        let mut functions = Ids::new(program.functions.len());
        let mut records = Ids::new(program.records.len());
        let mut enums = Ids::new(program.enums.len());
        let mut modules = Ids::new(program.modules.len());
        let mut imports = Ids::new(program.imports.len());
        for item in &program.items {
            match *item {
                ItemId::Module(index) => {
                    assert!(modules.visit(index).1, "duplicate item root");
                    let ModuleDecl { name, public, span } = program.modules[index];
                    self.tag("module");
                    self.spelling(name);
                    self.optional_span(public);
                    self.span(span);
                }
                ItemId::Import(index) => {
                    assert!(imports.visit(index).1, "duplicate item root");
                    let ImportDecl { path, alias, span } = program.imports[index];
                    self.tag("import");
                    self.absolute(path);
                    self.spelling(alias);
                    self.span(span);
                }
                ItemId::Struct(index) => {
                    assert!(records.visit(index).1, "duplicate item root");
                    let StructDecl {
                        public,
                        name,
                        fields,
                        span,
                        end,
                    } = &program.records[index];
                    self.tag("struct");
                    self.optional_span(*public);
                    self.spelling(*name);
                    self.span(*span);
                    self.span(*end);
                    self.count(fields.len());
                    for StructField {
                        public,
                        name,
                        ty,
                        span,
                    } in fields
                    {
                        self.optional_span(*public);
                        self.spelling(*name);
                        self.ty(*ty);
                        self.span(*span);
                    }
                }
                ItemId::Function(index) => {
                    assert!(functions.visit(index).1, "duplicate item root");
                    let function = &program.functions[index];
                    let Function {
                        public,
                        name,
                        params,
                        result,
                        body,
                        blocks,
                        end,
                    } = function;
                    self.tag("function");
                    self.optional_span(*public);
                    self.spelling(*name);
                    self.span(*end);
                    self.count(params.len());
                    for Param { name, ty } in params {
                        self.spelling(*name);
                        self.ty(*ty);
                    }
                    self.ty(*result);
                    self.count(blocks.len());
                    let mut blocks = Ids::new(blocks.len());
                    self.block(function, &mut blocks, *body);
                    blocks.assert_complete();
                }
                ItemId::Enum(index) => {
                    assert!(enums.visit(index).1, "duplicate item root");
                    let enumeration = &program.enums[index];
                    self.tag("enum");
                    self.optional_span(enumeration.public);
                    self.spelling(enumeration.name);
                    self.span(enumeration.span);
                    self.span(enumeration.end);
                    self.count(enumeration.variants.len());
                    for variant in &enumeration.variants {
                        self.tag("variant");
                        self.spelling(variant.name);
                        self.span(variant.span);
                        self.flag(variant.payload.is_some());
                        if let Some(payload) = variant.payload {
                            self.tag(match payload.kind {
                                ScalarTypeSyntax::Bool => "bool-payload",
                                ScalarTypeSyntax::I32 => "i32-payload",
                                ScalarTypeSyntax::Unit => "unit-payload",
                            });
                            self.span(payload.span);
                        }
                    }
                }
            }
        }
        for ids in [
            &functions,
            &records,
            &enums,
            &modules,
            &imports,
            &self.expressions,
            &self.paths,
        ] {
            ids.assert_complete();
        }
        assert_eq!(
            program.path_segments.len(),
            program
                .paths
                .iter()
                .map(|p| usize::from(p.segment_len))
                .sum()
        );
        self.parts
    }
}

fn assert_expression_source_order(program: &Program) {
    for pair in program.expressions.windows(2) {
        let left = pair[0].span;
        let right = pair[1].span;
        assert_eq!(left.file, right.file);
        assert!(
            left.end < right.end || (left.end == right.end && left.start > right.start),
            "parser postorder must have strict (end, reverse-start) keys: {left:?}, {right:?}"
        );
    }
}

fn parse(source: &SourceFile) -> Program {
    let program = parser::parse_counted_with_arrays(
        source,
        lexer::lex(source).expect("corpus must lex"),
        SourceMode::ProjectCandidate,
        parser::MAX_NODES,
        &mut crate::frontend::project::budget::Allocator::default(),
        parser::ArraySyntaxPolicy::Enabled,
    )
    .map(|(program, _)| program)
    .unwrap_or_else(|errors| panic!("corpus must parse: {errors:?}\n{}", source.text()));
    assert_expression_source_order(&program);
    program
}

fn fingerprint(source: &SourceFile, program: &Program) -> Vec<Part> {
    Fingerprint::new(source, program).program()
}

fn normalized(text: &str) -> Vec<Part> {
    let mut map = SourceMap::new();
    let id = map.add("independent-owner.ox".into(), text.into());
    let source = map.get(id);
    fingerprint(source, &parse(source))
}

// Hand-written parser-only syntax, deliberately independent of resolver or
// runtime fixtures. No packages or unrelated language proposals are needed.
const CORPUS: &[(&str, &str)] = &[
    ("empty", ""),
    ("named-byte-conversions", "// 雪\r\nfn f(x:i32,b:u8)->i32{x /*é*/ . /*dot*/ to_u8_checked /*name*/ ( /*empty*/ );return (b . to_i32 ());}fn u8(x:i32)->i32{return - -x.to_i32()+x.to_i32()*2;}"),
    ("projected-borrow-paths", "fn use_paths()->(){f(&batch /* root */ . inner . samples,&mut batch.inner.samples,&*p . inner . samples,&mut *p /* ref */ .inner.samples);return;}"),
    ("comments-only", "// 雪\r\n/* é\n braces { } and punctuation :: */\r\n"),
    ("scalar-expressions", r#"
// leading comment
fn arithmetic(a:i32,b:i32)->i32{let mut n:i32=00012;n=(a+3)*(b- -2);
let untyped=900719925474099312345678901234567890;
// separated unary literal sign
let minus=- /* sign */ 0007;return n;}
fn logic(a:i32,b:i32)->bool{let p:bool=!(true&&false)||a==b;
a!=b;a<b;a<=b;a>b;a>=b;return p;}
fn unit(x:())->(){();empty();return ();}fn empty()->(){return;}
"#),
    ("control-flow", r#"
fn flow(limit:i32)->(){let mut n=0;while n<limit{
if n==2{n=n+1;continue;}else{if false{break;}}
if !(n!=3){break;}n=n+1;}if true{}else{}return;}
"#),
    ("records-and-borrows", r#"
struct Empty{}struct Pair{left:i32,right:bool,}
fn inspect(p:&Pair,q:&mut Pair)->(){let mut x:Pair=Pair{left:1,right:true,};
let z=Empty{};x.left=(x.left+2);consume(x,Pair{left:-9,right:false});
inspect(&x,&mut x);inspect(&*p,&mut *q);inspect(&mut *q,&*p);return;}
"#),
    ("record-composition", r#"
struct Batch { meta: crate::state::Meta, samples: [i32; 3], }
fn work(b: &mut Batch)->i32 {
b /* root */ . meta /* hop */ . completed = b.meta.completed + 1;
b.samples /* index */ [0] = b.meta.completed;
return b.samples.len() + b.samples[0];}
"#),
    ("project-items", r#"
/* header */ pub mod child;mod private_child;
use crate::child::Pair as Alias;use crate::child::consume;
pub struct Pair{pub left:i32,right:bool,}struct Empty{}
pub fn build(p:&crate::child::Pair,q:&mut crate::child::Pair)->crate::child::Pair{
let value:crate::child::Pair=crate::child::Pair{left:1,right:false,};
crate::child::consume(&value,&mut *q,crate::private_child::make());return value;}
fn local()->(){return;}
"#),
    ("array-types-and-borrows", r#"
mod absent;use crate::absent::item;
fn types(a:[bool;0],b:[i32;0002],c:[();1],s:&[bool;0],m:&mut [i32;2])->[();0]{
let z:[();0]=([]);let mut values=[0001,- 2,];values[(0)]=m[1]+values.len();
sink(&values,&mut *m,&*s);return [];}
"#),
    ("slice-types-and-reborrows", r#"
fn view(s:&[i32],b:&[bool],u:&[()])->i32{return s.len();}
fn mutate(p:&mut /* view */ [i32])->(){p[0]=p[0]+1;mutate(&mut *p);view(&*p,&*p,&*p);return;}
"#),
    ("array-syntax-only-and-multiline", r#"
fn loose()->(){let nested=[[1],[]];let mixed=[true,(),missing(),Record{x:1},[2],];
missing[-1];missing.len();sink([
a[
0
],[
1,
2
]]);let x=[/* [ ] */1,
// next
2,];return;}
"#),
    ("contextual-names", r#"
use crate::as as crate;struct crate{as:i32}fn as(self:i32)->i32{return self;}
fn self()->(){let crate=crate{as:as(2)};let as=crate.as;crate::self::as();return;}
"#),
    ("division-remainder", "fn f(x:i32)->i32{return x/ /* divisor */ -3%2*7+1;}fn g()->i32{let a=[17,23,35];return(a[0]/10)*100+a[0]%10;}"),
    ("comment-boundaries", "// first\r\npub/*a*/mod/*b*/child;\r\nuse crate/*c*/::/*d*/child::f;\r\nfn f(x\u{2003}: i32)->(){\r\nlet n=-/* minus */1; /* middle\n雪 */if true{/* inside */return;}// tail\r\n}\r\n// eof"),
];

#[test]
fn formatting_preserves_structural_ast_and_token_relative_origins() {
    for &(label, text) in CORPUS {
        let mut original = SourceMap::new();
        let id = original.add(format!("{label}.ox"), text.into());
        let source = original.get(id);
        let before = fingerprint(source, &parse(source));
        let formatted =
            format_source(source).unwrap_or_else(|errors| panic!("{label}: {errors:?}"));
        // A second owner and a different file ID prove origin normalization
        // cannot accidentally depend on either source identity or raw offsets.
        let mut output = SourceMap::new();
        output.add("unrelated.ox".into(), "".into());
        let out_id = output.add(format!("formatted-{label}.ox"), formatted);
        let after = output.get(out_id);
        assert_eq!(before, fingerprint(after, &parse(after)), "{label}");
        assert_eq!(format_source(after).unwrap(), after.text(), "{label}");
    }
}

#[test]
fn normalization_ignores_whitespace_and_source_owner() {
    let tight = "pub mod m;use crate::m::R as A;pub struct R{pub n:i32,}pub fn f(x:&mut crate::m::R)->(){let n=-001;x.n=(n+2);if true{return;}else{return ();}}";
    let spaced = "\n pub\tmod m ;\r\n use crate /*path*/ :: m :: R as A ;\n pub struct R { pub n : i32 , }\n pub fn f ( x : & mut crate :: m :: R ) -> ( ) {\n let n = - /*sign*/ 001 ; x . n = ( n + 2 ) ;\n if true { return ; } else { return ( ) ; } }\n";
    assert_eq!(normalized(tight), normalized(spaced));
    assert_eq!(normalized(tight), normalized(tight));
}

#[test]
fn normalization_ignores_arena_storage_order_but_retains_edges() {
    let text = "pub mod a;mod b;use crate::a::R;use crate::b::S;struct R{}struct S{}fn f()->(){if true{f(1);}else{f(2);}}fn g()->(){return;}";
    let mut map = SourceMap::new();
    let id = map.add("relocated.ox".into(), text.into());
    let source = map.get(id);
    let mut program = parse(source);
    let expected = fingerprint(source, &program);

    let expression_count = program.expressions.len();
    let relocate_expression = |id: &mut ExprId| id.0 = expression_count - 1 - id.0;
    program.expressions.reverse();
    for expression in &mut program.expressions {
        match &mut expression.kind {
            ExprKind::Bool(_) | ExprKind::Number { .. } => {}
            ExprKind::Call { args, .. } => {
                for argument in args {
                    let Argument::Value(id) = argument else {
                        panic!("fixture has value arguments")
                    };
                    relocate_expression(id);
                }
            }
            _ => panic!("relocation fixture has only booleans, numbers and calls"),
        }
    }
    for function in &mut program.functions {
        let block_count = function.blocks.len();
        let relocate_block = |id: &mut BodyBlockId| id.0 = block_count - 1 - id.0;
        relocate_block(&mut function.body);
        function.blocks.reverse();
        for block in &mut function.blocks {
            for statement in &mut block.body {
                match &mut statement.kind {
                    StmtKind::Expr(id) => relocate_expression(id),
                    StmtKind::Return(None) => {}
                    StmtKind::If {
                        condition,
                        then_block,
                        else_block,
                    } => {
                        relocate_expression(condition);
                        relocate_block(then_block);
                        relocate_block(else_block.as_mut().unwrap());
                    }
                    _ => panic!("relocation fixture has only calls, ifs and empty returns"),
                }
            }
        }
    }

    // Relocate both path records and their independently stored segment ranges.
    let path_count = program.paths.len();
    program.paths.reverse();
    for import in &mut program.imports {
        import.path.0 = path_count - 1 - import.path.0;
    }
    let mut segments = Vec::new();
    for path in &mut program.paths {
        let old_start = path.segment_start;
        path.segment_start = segments.len();
        segments.extend_from_slice(
            &program.path_segments[old_start..old_start + usize::from(path.segment_len)],
        );
    }
    program.path_segments = segments;
    program.functions.reverse();
    program.records.reverse();
    program.enums.reverse();
    program.modules.reverse();
    program.imports.reverse();
    for item in &mut program.items {
        match item {
            ItemId::Function(index) => *index = program.functions.len() - 1 - *index,
            ItemId::Struct(index) => *index = program.records.len() - 1 - *index,
            ItemId::Enum(index) => *index = program.enums.len() - 1 - *index,
            ItemId::Module(index) => *index = program.modules.len() - 1 - *index,
            ItemId::Import(index) => *index = program.imports.len() - 1 - *index,
        }
    }
    assert_eq!(expected, fingerprint(source, &program));
}

#[test]
fn handwritten_corpus_reaches_every_ast_variant_and_operator() {
    let mut reached = std::collections::BTreeSet::new();
    for &(_, text) in CORPUS {
        reached.extend(normalized(text).into_iter().filter_map(|part| {
            if let Part::Tag(tag) = part {
                Some(tag)
            } else {
                None
            }
        }));
    }
    for tag in [
        "module",
        "import",
        "struct",
        "function",
        "absolute",
        "unqualified",
        "name-type",
        "unit-type",
        "reference-type",
        "array-type",
        "array-reference-type",
        "bool-element",
        "i32-element",
        "unit-element",
        "array-literal",
        "index-read",
        "array-length",
        "index-assign",
        "value-argument",
        "borrow-argument",
        "owner",
        "forwarded",
        "not",
        "and",
        "or",
        "equal",
        "not-equal",
        "less",
        "less-equal",
        "greater",
        "greater-equal",
        "bool",
        "number",
        "unit",
        "name",
        "call",
        "record-literal",
        "field-read",
        "group",
        "add",
        "subtract",
        "multiply",
        "let",
        "assign",
        "field-assign",
        "expression-statement",
        "return",
        "break",
        "continue",
        "while",
        "if",
    ] {
        assert!(reached.contains(tag), "corpus does not exercise {tag}");
    }
}

#[test]
fn normalization_detects_same_tape_semantic_mutations() {
    let text = "pub struct R{pub n:i32}pub fn f(x:&mut R)->(){let mut n=123;if true{n=n+2;}else{return;}f(&mut x);return;}";
    let mut map = SourceMap::new();
    let id = map.add("mutations.ox".into(), text.into());
    let source = map.get(id);
    let expected = fingerprint(source, &parse(source));
    let mutations: &[fn(&mut Program)] = &[
        |p| {
            p.functions[0].public = None;
        },
        |p| {
            p.records[0].fields[0].public = None;
        },
        |p| {
            let TypeSyntaxKind::Reference { mutable, .. } = &mut p.functions[0].params[0].ty.kind
            else {
                panic!()
            };
            *mutable = false;
        },
        |p| {
            let body = p.functions[0].body.0;
            let StmtKind::Let { mutable, .. } = &mut p.functions[0].blocks[body].body[0].kind
            else {
                panic!()
            };
            *mutable = false;
        },
        |p| {
            let expression = p
                .expressions
                .iter_mut()
                .find(|e| matches!(e.kind, ExprKind::Bool(_)))
                .unwrap();
            expression.kind = ExprKind::Bool(false);
        },
        |p| {
            let expression = p
                .expressions
                .iter_mut()
                .find(|e| matches!(e.kind, ExprKind::Number { .. }))
                .unwrap();
            let ExprKind::Number { negative, .. } = &mut expression.kind else {
                panic!()
            };
            *negative = true;
        },
        |p| {
            let expression = p
                .expressions
                .iter_mut()
                .find(|e| matches!(e.kind, ExprKind::Arithmetic { .. }))
                .unwrap();
            let ExprKind::Arithmetic { op, .. } = &mut expression.kind else {
                panic!()
            };
            *op = ArithmeticOp::Subtract;
        },
        |p| {
            let expression = p
                .expressions
                .iter_mut()
                .find(|e| matches!(e.kind, ExprKind::Arithmetic { .. }))
                .unwrap();
            let ExprKind::Arithmetic { left, right, .. } = &mut expression.kind else {
                panic!()
            };
            std::mem::swap(left, right);
        },
        |p| {
            let expression = p
                .expressions
                .iter_mut()
                .find(|e| matches!(e.kind, ExprKind::Call { .. }))
                .unwrap();
            let ExprKind::Call { args, .. } = &mut expression.kind else {
                panic!()
            };
            let Argument::Borrow { mutable, .. } = &mut args[0] else {
                panic!()
            };
            *mutable = false;
        },
        |p| {
            let body = p.functions[0].body.0;
            let StmtKind::If {
                then_block,
                else_block,
                ..
            } = &mut p.functions[0].blocks[body].body[1].kind
            else {
                panic!()
            };
            std::mem::swap(then_block, else_block.as_mut().unwrap());
        },
    ];
    for (index, mutation) in mutations.iter().enumerate() {
        let mut changed = parse(source);
        mutation(&mut changed);
        assert_ne!(expected, fingerprint(source, &changed), "mutation {index}");
    }
}

#[test]
fn normalization_detects_changed_origins_tokens_and_item_order() {
    let text = "fn f()->(){123;123;return;}fn g()->(){return;}";
    let mut map = SourceMap::new();
    let id = map.add("origins.ox".into(), text.into());
    let source = map.get(id);
    let expected = fingerprint(source, &parse(source));

    let mut changed = parse(source);
    // Equal spelling is insufficient: the same-tape origin moved to another
    // occurrence must be detected even though all expression values agree.
    changed.expressions[0].span = changed.expressions[1].span;
    assert_ne!(expected, fingerprint(source, &changed));
    let mut changed = parse(source);
    changed.expressions[0].span.start += 1;
    assert_ne!(expected, fingerprint(source, &changed));
    let mut changed = parse(source);
    changed.expressions[0].span.end -= 1;
    assert_ne!(expected, fingerprint(source, &changed));
    let mut changed = parse(source);
    changed.items.swap(0, 1);
    assert_ne!(expected, fingerprint(source, &changed));

    assert_ne!(
        normalized("fn f()->(){1;2;}"),
        normalized("fn f()->(){2;1;}")
    );
    assert_ne!(normalized("fn f()->(){1;}"), normalized("fn f()->(){1;1;}"));
    assert_ne!(
        normalized("fn f()->(){123;}"),
        normalized("fn f()->(){124;}")
    );
    assert_ne!(
        normalized("use crate::m::A;"),
        normalized("use crate::m::B;")
    );
}

#[test]
fn token_relative_spans_keep_intra_token_and_empty_boundaries() {
    let mut map = SourceMap::new();
    let id = map.add("boundaries.ox".into(), "fn f()->(){12345;return;}  ".into());
    let source = map.get(id);
    let program = parse(source);
    let oracle = Fingerprint::new(source, &program);
    let token = oracle
        .tokens
        .iter()
        .position(|t| t.kind == Kind::Number)
        .unwrap();
    let start = oracle.tokens[token].span.start;
    assert_eq!(
        oracle.origin(source.span(start + 1, start + 4)),
        Origin::Covered {
            first: token,
            end: token + 1,
            first_byte: 1,
            last_byte: 4,
        }
    );
    assert_eq!(
        oracle.origin(source.span(start + 2, start + 2)),
        Origin::Empty {
            next: token,
            byte: 2
        }
    );
    let end = source.text().len();
    assert_eq!(
        oracle.origin(source.span(end, end)),
        Origin::Empty {
            next: oracle.tokens.len(),
            byte: 0
        }
    );
}

#[test]
fn normalization_detects_same_tape_array_mutations() {
    let text = "fn f(a:&mut [i32;2],b:[bool;0])->[();0]{let mut x:[i32;2]=[1,2];x[0]=a[1];x.len();return [];}";
    let mut map = SourceMap::new();
    let id = map.add("array-mutations.ox".into(), text.into());
    let source = map.get(id);
    let expected = fingerprint(source, &parse(source));
    let mutations: &[fn(&mut Program)] = &[
        |p| {
            let TypeSyntaxKind::ArrayReference { mutable, .. } =
                &mut p.functions[0].params[0].ty.kind
            else {
                panic!()
            };
            *mutable = false;
        },
        |p| {
            let TypeSyntaxKind::ArrayReference { array, .. } =
                &mut p.functions[0].params[0].ty.kind
            else {
                panic!()
            };
            array.element = ScalarTypeSyntax::Bool;
        },
        |p| {
            let TypeSyntaxKind::Array(array) = &mut p.functions[0].params[1].ty.kind else {
                panic!()
            };
            array.length = 1;
        },
        |p| {
            let TypeSyntaxKind::Array(array) = &mut p.functions[0].result.kind else {
                panic!()
            };
            array.element = ScalarTypeSyntax::I32;
        },
        |p| {
            let expression = p
                .expressions
                .iter_mut()
                .find(|e| matches!(e.kind, ExprKind::ArrayLiteral { .. }))
                .unwrap();
            let ExprKind::ArrayLiteral { elements } = &mut expression.kind else {
                panic!()
            };
            elements.reverse();
        },
        |p| {
            let name = p.functions[0].params[1].name;
            let expression = p
                .expressions
                .iter_mut()
                .find(|e| matches!(e.kind, ExprKind::IndexRead { .. }))
                .unwrap();
            let ExprKind::IndexRead { base, .. } = &mut expression.kind else {
                panic!()
            };
            *base = name;
        },
        |p| {
            let mut indices = p.expressions.iter_mut().filter_map(|e| match &mut e.kind {
                ExprKind::IndexRead { index, .. } => Some(index),
                _ => None,
            });
            std::mem::swap(indices.next().unwrap(), indices.next().unwrap());
        },
        |p| {
            let name = p.functions[0].params[1].name;
            let expression = p
                .expressions
                .iter_mut()
                .find(|e| matches!(e.kind, ExprKind::ArrayLength { .. }))
                .unwrap();
            let ExprKind::ArrayLength { base } = &mut expression.kind else {
                panic!()
            };
            *base = name;
        },
        |p| {
            let body = p.functions[0].body.0;
            let StmtKind::IndexAssign { target, value, .. } =
                &mut p.functions[0].blocks[body].body[1].kind
            else {
                panic!()
            };
            std::mem::swap(target, value);
        },
    ];
    for (index, mutation) in mutations.iter().enumerate() {
        let mut changed = parse(source);
        mutation(&mut changed);
        assert_ne!(
            expected,
            fingerprint(source, &changed),
            "array mutation {index}"
        );
    }
}

fn parse_enum_fingerprint(source: &SourceFile) -> Program {
    let program = parser::parse_enum_candidate_counted(
        source,
        lexer::lex(source).unwrap(),
        SourceMode::ProjectCandidate,
        parser::MAX_NODES,
        &mut crate::frontend::project::budget::Allocator::default(),
        &mut Default::default(),
    )
    .unwrap()
    .0;
    assert_expression_source_order(&program);
    program
}

#[test]
fn builtin_current_formatter_preserves_structural_ast_and_fixed_point() {
    fn parse_current(source: &SourceFile) -> Program {
        parser::parse_typed_counted(
            source,
            lexer::lex(source).unwrap(),
            SourceMode::ProjectCandidate,
            parser::MAX_NODES,
            &mut crate::frontend::project::budget::Allocator::default(),
            &mut Default::default(),
        )
        .unwrap()
        .0
    }
    for text in [
        "use std::io::read_stdin as input;use std::io::ReadStatus as S;fn main()->i32{let mut bytes=[-1,0];let status=input(&mut bytes);match status{S::Eof(n)=>{return n;},S::Full=>{return 2;},S::IoError=>{return -1;},}}",
        "mod absent;use std/*root*/::/*edge*/io::ReadStatus as S;use crate::std::ReadStatus as User;fn f(s:S)->(){match s{S/*type*/::Full=>{return;},S::IoError=>{},S::Eof(n)=>{n;},}}",
        "// input\r\nuse std :: io :: read_stdin as read;\r\nfn f(b:&mut [i32])->(){read(&mut *b);return;}\r\n",
        "use std::io::write_stdout as output;use std::io::WriteStatus as W;fn main()->i32{let bytes=[0,255];let status=output(&bytes);match status{W::Complete=>{return 0;},W::InvalidInput=>{return 1;},W::IoError(n)=>{return n;},}}",
        "mod absent;use std/*root*/::/*edge*/io::WriteStatus as W;use std::io::read_stdin as input;fn f(s:W)->(){match s{W::Complete=>{return;},W::InvalidInput=>{},W::IoError(n)=>{n;},}}",
    ] {
        let mut sources = SourceMap::new();
        let id = sources.add("stdin-format.ox".into(), text.into());
        let source = sources.get(id);
        let program = parse_current(source);
        // The convenience default stays closed; current source validation must
        // select its import policy explicitly before the checked index walk.
        assert!(!program.validate_spans_and_ids(|at| source.try_text(at).is_some()));
        let mut enum_syntax = None;
        let mut std_syntax = false;
        assert!(program.validate_spans_and_ids_counted_with_syntax(
            |at| at.is_none_or(|at| source.try_text(at).is_some()),
            &mut enum_syntax,
            parser::StdImportPolicy::Enabled,
            &mut std_syntax,
        ));
        assert!(std_syntax);
        let expected = fingerprint(source, &program);
        let output = format_source(source).unwrap();
        let mut formatted = SourceMap::new();
        formatted.add("unrelated.ox".into(), "".into());
        let id = formatted.add("stdin-formatted.ox".into(), output);
        let after = formatted.get(id);
        let reparsed = parse_current(after);
        assert_eq!(expected, fingerprint(after, &reparsed));
        assert!(super::same_projection(source, &program.tokens, after, &reparsed.tokens));
        assert_eq!(format_source(after).unwrap(), after.text());
    }
}

#[test]
fn enum_formatter_independent_fingerprint_roundtrips_every_new_carrier() {
    let cases = [
        "enum E{Z,I(i32),B(bool),U(()),} fn f(e:E)->(){E::Z;E::Z();E::U(());match e{E::I(v)=>{v;},E::B(b)=>{b;},E::U(u)=>{u;},E::Z=>{},}}",
        "pub enum E{Z,I(i32)}fn f(e:crate::m::E,r:&mut R)->(){crate::m::f(1,2,&*r,&mut *r);let a=[E::I(1),E::Z];R{x:E::I(2)};match e{crate::m::E::I(v)=>{return;},crate::m::E::Z=>{},}}",
        "/* enum match => :: } */\r\nenum E{Z,U(()),}\nfn f(e:E)->(){\nmatch e{E/*x*/::/*y*/Z/*a*/=>/*b*/{},E::U(u)=>{match e{E::Z=>{},E::U(v)=>{v;},}},}\n}",
        "enum E{I(i32),Z}fn f(e:E)->(){while true{match e{E::I(v)=>{if v>0{continue;}else{break;}},E::Z=>{return;},}}}",
    ];
    for text in cases {
        let mut sources = SourceMap::new();
        let id = sources.add("enum-oracle.ox".into(), text.into());
        let source = sources.get(id);
        let program = parse_enum_fingerprint(source);
        assert!(program.validate_spans_and_ids(|at| source.try_text(at).is_some()));
        let expected = fingerprint(source, &program);
        let (result, metrics) = super::format_enum_candidate_observed(
            source,
            &mut crate::frontend::project::budget::Allocator::default(),
        );
        let output = result.unwrap();
        assert_eq!(metrics.parse_calls, 2);
        let mut candidates = SourceMap::new();
        let id = candidates.add("enum-output.ox".into(), output.clone());
        let candidate = candidates.get(id);
        let reparsed = parse_enum_fingerprint(candidate);
        assert!(reparsed.validate_spans_and_ids(|at| candidate.try_text(at).is_some()));
        assert_eq!(expected, fingerprint(candidate, &reparsed));
        assert!(super::same_projection(
            source,
            &program.tokens,
            candidate,
            &reparsed.tokens
        ));
        assert_eq!(
            super::format_enum_candidate_observed(
                candidate,
                &mut crate::frontend::project::budget::Allocator::default()
            )
            .0
            .unwrap(),
            output
        );
    }
}

fn enum_fingerprint_arms(program: &mut Program) -> &mut Vec<MatchArmSyntax> {
    let statement = program.functions[0].blocks[0]
        .body
        .iter_mut()
        .find(|statement| matches!(statement.kind, StmtKind::Match { .. }))
        .unwrap();
    let StmtKind::Match { arms, .. } = &mut statement.kind else {
        unreachable!()
    };
    arms
}

#[test]
fn enum_formatter_independent_fingerprint_detects_same_tape_mutations() {
    let text = "pub enum E{Z,I(i32),U(()),B(bool)}fn f(e:E,r:&R)->(){E::Z;E::Z();E::U(());crate::m::f(1,2);crate::m::g(&*r);match e{E::I(v)=>{v;},E::Z=>{},E::U(u)=>{u;},E::B(b)=>{b;},}}";
    let mut sources = SourceMap::new();
    let id = sources.add("enum-mutations.ox".into(), text.into());
    let source = sources.get(id);
    let original = parse_enum_fingerprint(source);
    let expected = fingerprint(source, &original);
    let tape: Vec<_> = original
        .tokens
        .iter()
        .map(|token| (token.kind, token.span))
        .collect();
    let mutations: &[fn(&mut Program)] = &[
        |p| p.enums[0].variants.swap(0, 1),
        |p| {
            let name = p.enums[0].variants[1].name;
            p.enums[0].variants[0].name = name;
        },
        |p| p.enums[0].variants[1].payload.as_mut().unwrap().kind = ScalarTypeSyntax::Bool,
        |p| {
            let payload = p.enums[0].variants[2].payload;
            p.enums[0].variants[0].payload = payload;
        },
        |p| p.enums[0].variants[2].payload = None,
        |p| {
            p.enums[0].span = p.enums[0].name;
        },
        |p| {
            p.enums[0].variants[1].span = p.enums[0].variants[1].name;
        },
        |p| {
            let expr = p
                .expressions
                .iter_mut()
                .find(|expr| matches!(expr.kind, ExprKind::QualifiedValue { args: None, .. }))
                .unwrap();
            let ExprKind::QualifiedValue { args, .. } = &mut expr.kind else {
                unreachable!()
            };
            *args = Some(Vec::new());
        },
        |p| {
            let expr = p.expressions.iter_mut().find(|expr| matches!(&expr.kind, ExprKind::QualifiedValue { args: Some(args), .. } if args.is_empty())).unwrap();
            let ExprKind::QualifiedValue { args, .. } = &mut expr.kind else {
                unreachable!()
            };
            *args = None;
        },
        |p| {
            let expr = p.expressions.iter_mut().find(|expr| matches!(&expr.kind, ExprKind::QualifiedValue { args: Some(args), .. } if args.len() == 2)).unwrap();
            let ExprKind::QualifiedValue {
                args: Some(args), ..
            } = &mut expr.kind
            else {
                unreachable!()
            };
            args.swap(0, 1);
        },
        |p| {
            let expr = p.expressions.iter_mut().find(|expr| matches!(&expr.kind, ExprKind::QualifiedValue { args: Some(args), .. } if matches!(args.first(), Some(Argument::Borrow { .. })))).unwrap();
            let ExprKind::QualifiedValue {
                args: Some(args), ..
            } = &mut expr.kind
            else {
                unreachable!()
            };
            let Argument::Borrow { mutable, .. } = &mut args[0] else {
                unreachable!()
            };
            *mutable = !*mutable;
        },
        |p| enum_fingerprint_arms(p).swap(0, 1),
        |p| enum_fingerprint_arms(p)[0].binding = None,
        |p| {
            let arms = enum_fingerprint_arms(p);
            arms[0].binding = arms[2].binding;
        },
        |p| {
            let arms = enum_fingerprint_arms(p);
            let first = arms[0].body;
            arms[0].body = arms[1].body;
            arms[1].body = first;
        },
        |p| {
            let arms = enum_fingerprint_arms(p);
            arms[0].span = arms[1].span;
        },
        |p| p.paths[0].root = PathRoot::Crate,
        |p| {
            p.paths[0].span = p.paths[1].span;
        },
        |p| {
            let name = p.enums[0].name;
            p.enums[0].variants[1].payload.as_mut().unwrap().span = name;
        },
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut changed = parse_enum_fingerprint(source);
        mutate(&mut changed);
        assert_eq!(
            tape,
            changed
                .tokens
                .iter()
                .map(|token| (token.kind, token.span))
                .collect::<Vec<_>>()
        );
        assert_ne!(expected, fingerprint(source, &changed), "mutation {index}");
    }
}

fn enum_fingerprint_qualified_borrow(program: &mut Program, index: usize) -> &mut BorrowPlace {
    program
        .expressions
        .iter_mut()
        .filter_map(|expression| match &mut expression.kind {
            ExprKind::QualifiedValue {
                args: Some(args), ..
            } => Some(args),
            _ => None,
        })
        .flat_map(|args| args.iter_mut())
        .filter_map(|argument| match argument {
            Argument::Borrow { place, .. } => Some(place),
            _ => None,
        })
        .nth(index)
        .unwrap()
}

#[test]
fn enum_formatter_independent_fingerprint_covers_qualified_values_and_match_origins() {
    let text = "enum E{I(i32),Z}fn f(e:E,r:&R,s:&R)->(){crate::m::f(true,&*r);crate::m::g(&*r);crate::m::h(&*s);match e{E::I(v)=>{v;},E::Z=>{},}}";
    let mut sources = SourceMap::new();
    let id = sources.add("enum-origin-mutations.ox".into(), text.into());
    let source = sources.get(id);
    let mut original = parse_enum_fingerprint(source);
    let expected = fingerprint(source, &original);
    let tape: Vec<_> = original
        .tokens
        .iter()
        .map(|token| (token.kind, token.span))
        .collect();
    let BorrowPlace::ForwardedParameter {
        name: first_name,
        star_span: first_star,
    } = *enum_fingerprint_qualified_borrow(&mut original, 0)
    else {
        panic!("first forwarded borrow")
    };
    let BorrowPlace::ForwardedParameter {
        name: second_name,
        star_span: second_star,
    } = *enum_fingerprint_qualified_borrow(&mut original, 1)
    else {
        panic!("second forwarded borrow")
    };
    assert_eq!(source.text_at(first_name), source.text_at(second_name));
    assert_ne!(
        first_name, second_name,
        "identical names must retain distinct source origins"
    );
    assert_eq!(source.text_at(first_star), source.text_at(second_star));
    assert_ne!(
        first_star, second_star,
        "identical stars must retain distinct source origins"
    );

    type NamedMutation = (&'static str, fn(&mut Program));
    let mutations: &[NamedMutation] = &[
        ("match arm variant paths", |program| {
            // Swap only path edges, keeping written arm/body/binding order and
            // every arena occurrence reachable for the unchanged completeness oracle.
            let arms = enum_fingerprint_arms(program);
            let first = arms[0].variant;
            arms[0].variant = arms[1].variant;
            arms[1].variant = first;
        }),
        ("match statement origin", |program| {
            let statement = program.functions[0].blocks[0]
                .body
                .iter_mut()
                .find(|statement| matches!(statement.kind, StmtKind::Match { .. }))
                .unwrap();
            let StmtKind::Match { arms, .. } = &statement.kind else {
                unreachable!()
            };
            statement.span = arms[0].span;
        }),
        ("qualified call argument value", |program| {
            let expression = program
                .expressions
                .iter()
                .find(|expression| {
                    matches!(&expression.kind, ExprKind::QualifiedValue { args: Some(args), .. }
                    if matches!(args.first(), Some(Argument::Value(_))))
                })
                .unwrap();
            let ExprKind::QualifiedValue {
                args: Some(args), ..
            } = &expression.kind
            else {
                unreachable!()
            };
            let Argument::Value(value) = args[0] else {
                unreachable!()
            };
            let ExprKind::Bool(value) = &mut program.expressions[value.0].kind else {
                panic!("boolean call argument")
            };
            *value = !*value;
        }),
        ("qualified borrow place", |program| {
            let place = enum_fingerprint_qualified_borrow(program, 0);
            let BorrowPlace::ForwardedParameter { name, .. } = *place else {
                unreachable!()
            };
            *place = BorrowPlace::OwnerName(name);
        }),
        ("qualified borrow name", |program| {
            let BorrowPlace::ForwardedParameter { name: other, .. } =
                *enum_fingerprint_qualified_borrow(program, 2)
            else {
                unreachable!()
            };
            let BorrowPlace::ForwardedParameter { name, .. } =
                enum_fingerprint_qualified_borrow(program, 0)
            else {
                unreachable!()
            };
            *name = other;
        }),
        ("qualified borrow identical-name origin", |program| {
            let BorrowPlace::ForwardedParameter { name: other, .. } =
                *enum_fingerprint_qualified_borrow(program, 1)
            else {
                unreachable!()
            };
            let BorrowPlace::ForwardedParameter { name, .. } =
                enum_fingerprint_qualified_borrow(program, 0)
            else {
                unreachable!()
            };
            *name = other;
        }),
        ("forwarded-star origin", |program| {
            let BorrowPlace::ForwardedParameter {
                star_span: other, ..
            } = *enum_fingerprint_qualified_borrow(program, 1)
            else {
                unreachable!()
            };
            let BorrowPlace::ForwardedParameter { star_span, .. } =
                enum_fingerprint_qualified_borrow(program, 0)
            else {
                unreachable!()
            };
            *star_span = other;
        }),
    ];
    for &(name, mutate) in mutations {
        let mut changed = parse_enum_fingerprint(source);
        mutate(&mut changed);
        assert_eq!(
            tape,
            changed
                .tokens
                .iter()
                .map(|token| (token.kind, token.span))
                .collect::<Vec<_>>(),
            "{name}"
        );
        assert_ne!(
            expected,
            fingerprint(source, &changed),
            "missing mutation control: {name}"
        );
    }
}

#[test]
fn u8_formatter_fingerprint_binds_operation_receiver_and_intrinsic_origins() {
    let text = "fn f(x:i32,y:i32)->(){x.to_u8_checked();y.to_u8_checked();return;}";
    let mut map = SourceMap::new();
    let file = map.add("u8-origins.ox".into(), text.into());
    let source = map.get(file);
    let expected = fingerprint(source, &parse(source));
    for mutation in 0..3 {
        let mut ast = parse(source);
        let ExprKind::Conversion {
            operand: second_operand,
            name_span: second_name,
            ..
        } = ast.expressions[3].kind
        else {
            unreachable!()
        };
        let ExprKind::Conversion {
            op,
            operand,
            name_span,
        } = &mut ast.expressions[1].kind
        else {
            unreachable!()
        };
        match mutation {
            0 => *op = ConversionOp::ToI32,
            1 => *name_span = second_name,
            2 => {
                let first = *operand;
                *operand = second_operand;
                let ExprKind::Conversion { operand, .. } = &mut ast.expressions[3].kind else {
                    unreachable!()
                };
                *operand = first;
            }
            _ => unreachable!(),
        }
        assert_ne!(expected, fingerprint(source, &ast));
    }
}
