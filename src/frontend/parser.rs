use super::{
    ast::*,
    diagnostic::Diagnostic,
    lexer::{Kind, Token},
    source::SourceFile,
};
pub const MAX_NODES: usize = 100_000;
pub const MAX_NESTING: usize = 64;
/// Active statement blocks, including the outer function body; independent of expressions.
pub const MAX_BLOCK_NESTING: usize = 64;
pub const MAX_PARAMS: usize = 256;
pub const MAX_DIAGNOSTICS: usize = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceMode {
    #[cfg(test)]
    ScalarOnly,
    OwnedCandidate,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum LiteralContext {
    Allowed,
    ConditionRoot,
}
pub fn parse(source: &SourceFile, tokens: Vec<Token>) -> Result<Program, Vec<Diagnostic>> {
    parse_with_mode(source, tokens, SourceMode::OwnedCandidate)
}
pub(super) fn parse_with_mode(
    source: &SourceFile,
    tokens: Vec<Token>,
    mode: SourceMode,
) -> Result<Program, Vec<Diagnostic>> {
    parse_with_node_limit(source, tokens, mode, MAX_NODES)
}
fn parse_with_node_limit(
    source: &SourceFile,
    tokens: Vec<Token>,
    mode: SourceMode,
    node_limit: usize,
) -> Result<Program, Vec<Diagnostic>> {
    let mut parser = Parser {
        source,
        mode,
        tokens,
        cursor: 0,
        expressions: Vec::new(),
        heights: Vec::new(),
        nodes: 0,
        node_limit: node_limit.min(MAX_NODES),
    };
    parser.skip();
    let mut functions = Vec::new();
    let mut records = Vec::new();
    let mut items = Vec::new();
    let mut diagnostics = Vec::new();
    while parser.peek().kind != Kind::Eof && diagnostics.len() < MAX_DIAGNOSTICS {
        let before = parser.cursor;
        let result = if mode == SourceMode::OwnedCandidate && parser.peek().kind == Kind::Struct {
            parser.record().map(|record| {
                items.push(ItemId::Struct(records.len()));
                records.push(record);
            })
        } else {
            parser.function().map(|function| {
                items.push(ItemId::Function(functions.len()));
                functions.push(function);
            })
        };
        if let Err(error) = result {
            diagnostics.push(*error);
            // Recovery must consume the failing keyword before synchronizing.
            if parser.cursor == before {
                parser.bump();
            }
            while !matches!(parser.peek().kind, Kind::Fn | Kind::Eof)
                && !(mode == SourceMode::OwnedCandidate && parser.peek().kind == Kind::Struct)
            {
                parser.bump();
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(Program {
            tokens: parser.tokens,
            functions,
            records,
            items,
            expressions: parser.expressions,
        })
    } else {
        Err(diagnostics)
    }
}
struct Parser<'a> {
    source: &'a SourceFile,
    mode: SourceMode,
    tokens: Vec<Token>,
    cursor: usize,
    expressions: Vec<Expr>,
    heights: Vec<usize>,
    nodes: usize,
    node_limit: usize,
}
impl Parser<'_> {
    fn skip(&mut self) {
        while self.tokens[self.cursor].kind == Kind::Trivia {
            self.cursor += 1;
        }
    }
    fn peek(&self) -> Token {
        self.tokens[self.cursor]
    }
    fn bump(&mut self) -> Token {
        let token = self.peek();
        if token.kind != Kind::Eof {
            self.cursor += 1;
            self.skip();
        }
        token
    }
    fn take(&mut self, kind: Kind) -> Option<Token> {
        if self.peek().kind == kind {
            Some(self.bump())
        } else {
            None
        }
    }
    fn diagnostic(
        &self,
        code: &'static str,
        stage: &'static str,
        message: impl std::fmt::Display,
        primary: Option<super::source::Span>,
    ) -> Box<Diagnostic> {
        if self.mode == SourceMode::OwnedCandidate {
            super::owned_diagnostic::diagnostic(code, stage, format_args!("{message}"), primary)
        } else {
            Diagnostic::new(code, stage, message.to_string(), primary)
        }
    }
    fn error(&self, message: &str) -> Box<Diagnostic> {
        let token = self.peek();
        let unsupported = matches!(
            token.kind,
            Kind::Unsupported
                | Kind::Number
                | Kind::Minus
                | Kind::Plus
                | Kind::String
                | Kind::Struct
                | Kind::Ampersand
                | Kind::Dot
        ) || (token.kind == Kind::Ident
            && &self.source.text()[token.span.start..token.span.end] == "as");
        // Report an unsupported cast in an already-invalid grammar position,
        // without reserving `as` as a declaration or expression identifier.
        // Shared parser diagnostics retain the original scalar formatting. The
        // new owned-only retained-text limit excludes this token-bounded path.
        Diagnostic::new(
            if unsupported { "E0101" } else { "E0100" },
            "parse",
            if unsupported {
                format!(
                    "unsupported typed-preview construct `{}`",
                    &self.source.text()[token.span.start..token.span.end]
                )
            } else {
                message.to_string()
            },
            Some(token.span),
        )
    }
    fn expect(&mut self, kind: Kind, message: &str) -> Result<Token, Box<Diagnostic>> {
        self.take(kind).ok_or_else(|| self.error(message))
    }
    fn node(&mut self) -> Result<(), Box<Diagnostic>> {
        if self.nodes >= self.node_limit {
            return Err(self.diagnostic(
                "E0400",
                "parse",
                "syntax node limit exceeded",
                Some(self.peek().span),
            ));
        }
        self.nodes += 1;
        Ok(())
    }
    fn function(&mut self) -> Result<Function, Box<Diagnostic>> {
        self.node()?;
        self.expect(Kind::Fn, "expected a top-level function declaration")?;
        let name = self.expect(Kind::Ident, "expected function name")?.span;
        self.expect(Kind::LParen, "expected `(`")?;
        let mut params = Vec::new();
        if self.peek().kind != Kind::RParen {
            loop {
                if params.len() >= MAX_PARAMS {
                    return Err(self.diagnostic(
                        "E0400",
                        "parse",
                        "parameter limit exceeded",
                        Some(self.peek().span),
                    ));
                }
                self.node()?;
                let name = self.expect(Kind::Ident, "expected parameter name")?.span;
                self.expect(Kind::Colon, "parameter requires an explicit type")?;
                params.push(Param {
                    name,
                    ty: self.parameter_ty()?,
                });
                if self.take(Kind::Comma).is_none() {
                    break;
                }
            }
        }
        self.expect(Kind::RParen, "expected `)`")?;
        self.expect(
            Kind::Arrow,
            "function requires an explicit return type after `->`",
        )?;
        let result = self.ty()?;
        let mut blocks = Vec::new();
        let body = self.block(&mut blocks, 0, "expected function body `{`")?;
        let end = blocks[body.0].end;
        Ok(Function {
            name,
            params,
            result,
            body,
            blocks,
            end,
        })
    }
    fn block(
        &mut self,
        blocks: &mut Vec<BodyBlock>,
        depth: usize,
        opening_message: &str,
    ) -> Result<BodyBlockId, Box<Diagnostic>> {
        // Nested callers check depth before entry. Arena edges prevent a
        // recursive drop chain, including for malformed syntax.
        let start = self.expect(Kind::LBrace, opening_message)?.span;
        let id = BodyBlockId(blocks.len());
        blocks.push(BodyBlock {
            body: Vec::new(),
            span: start,
            end: start,
        });
        let mut body = Vec::new();
        while !matches!(self.peek().kind, Kind::RBrace | Kind::Eof) {
            body.push(self.statement(blocks, depth + 1)?);
        }
        let end = self
            .expect(Kind::RBrace, "expected `}` before end of file")?
            .span;
        blocks[id.0] = BodyBlock {
            body,
            span: self.source.span(start.start, end.end),
            end,
        };
        Ok(id)
    }
    fn ty(&mut self) -> Result<TypeSyntax, Box<Diagnostic>> {
        let token = self.peek();
        let (kind, end) = if self.take(Kind::LParen).is_some() {
            (
                TypeSyntaxKind::Unit,
                self.expect(Kind::RParen, "only the unit type `()` is supported here")?
                    .span
                    .end,
            )
        } else {
            let name = self
                .expect(Kind::Ident, "expected `bool`, `i32` or `()` type")?
                .span;
            (TypeSyntaxKind::Name(name), name.end)
        };
        Ok(TypeSyntax {
            span: self.source.span(token.span.start, end),
            kind,
        })
    }
    fn parameter_ty(&mut self) -> Result<TypeSyntax, Box<Diagnostic>> {
        if self.mode == SourceMode::OwnedCandidate && self.peek().kind == Kind::Ampersand {
            let start = self.bump().span.start;
            let mutable = self.take(Kind::Mut).is_some();
            let referent = self
                .expect(Kind::Ident, "reference parameter requires a record name")?
                .span;
            return Ok(TypeSyntax {
                span: self.source.span(start, referent.end),
                kind: TypeSyntaxKind::Reference { mutable, referent },
            });
        }
        self.ty()
    }
    fn record(&mut self) -> Result<StructDecl, Box<Diagnostic>> {
        self.node()?;
        let start = self
            .expect(Kind::Struct, "expected struct declaration")?
            .span;
        let name = self.expect(Kind::Ident, "expected record name")?.span;
        self.expect(Kind::LBrace, "expected record fields `{`")?;
        let mut fields = Vec::new();
        while self.peek().kind != Kind::RBrace {
            self.node()?;
            let name = self.expect(Kind::Ident, "expected field name")?.span;
            self.expect(Kind::Colon, "field requires an explicit scalar type")?;
            let ty = self.ty()?;
            fields.push(StructField {
                name,
                ty,
                span: self.source.span(name.start, ty.span.end),
            });
            if self.take(Kind::Comma).is_none() {
                break;
            }
        }
        let end = self
            .expect(Kind::RBrace, "expected record fields `}`")?
            .span;
        Ok(StructDecl {
            name,
            fields,
            span: self.source.span(start.start, end.end),
            end,
        })
    }
    fn argument(&mut self, depth: usize) -> Result<Argument, Box<Diagnostic>> {
        if self.mode == SourceMode::OwnedCandidate && self.peek().kind == Kind::Ampersand {
            if depth >= MAX_NESTING {
                return Err(self.diagnostic(
                    "E0400",
                    "parse",
                    "expression nesting limit exceeded",
                    Some(self.peek().span),
                ));
            }
            self.node()?;
            let start = self.bump().span.start;
            let mutable = self.take(Kind::Mut).is_some();
            let star = self.take(Kind::Star);
            let name = self
                .expect(Kind::Ident, "borrow argument requires a binding name")?
                .span;
            let place = star.map_or(BorrowPlace::OwnerName(name), |star| {
                BorrowPlace::ForwardedParameter {
                    name,
                    star_span: star.span,
                }
            });
            if !matches!(self.peek().kind, Kind::Comma | Kind::RParen) {
                return Err(self.error("borrow must be the complete call argument"));
            }
            return Ok(Argument::Borrow {
                mutable,
                place,
                span: self.source.span(start, name.end),
            });
        }
        Ok(Argument::Value(
            self.expression(depth, LiteralContext::Allowed)?,
        ))
    }
    fn statement(
        &mut self,
        blocks: &mut Vec<BodyBlock>,
        depth: usize,
    ) -> Result<Stmt, Box<Diagnostic>> {
        self.node()?;
        let start = self.peek().span.start;
        if self.take(Kind::While).is_some() {
            let condition = self.expression(0, LiteralContext::ConditionRoot)?;
            if depth >= MAX_BLOCK_NESTING {
                return Err(self.diagnostic(
                    "E0400",
                    "parse",
                    "statement block nesting limit exceeded",
                    Some(self.peek().span),
                ));
            }
            let body = self.block(blocks, depth, "expected while body `{`")?;
            return Ok(Stmt {
                kind: StmtKind::While { condition, body },
                span: self.source.span(start, blocks[body.0].end.end),
            });
        }
        if self.take(Kind::If).is_some() {
            let condition = self.expression(0, LiteralContext::ConditionRoot)?;
            // `depth` is the number of active statement blocks. Check before
            // recursive entry or arena allocation for either arm.
            if depth >= MAX_BLOCK_NESTING {
                return Err(self.diagnostic(
                    "E0400",
                    "parse",
                    "statement block nesting limit exceeded",
                    Some(self.peek().span),
                ));
            }
            let then_block = self.block(blocks, depth, "expected if body `{`")?;
            let else_block = if self.take(Kind::Else).is_some() {
                Some(self.block(blocks, depth, "expected else body `{`")?)
            } else {
                None
            };
            let end = blocks[else_block.unwrap_or(then_block).0].end.end;
            return Ok(Stmt {
                kind: StmtKind::If {
                    condition,
                    then_block,
                    else_block,
                },
                span: self.source.span(start, end),
            });
        }
        let kind = if self.take(Kind::Let).is_some() {
            let mutable = self.take(Kind::Mut).is_some();
            let name = self.expect(Kind::Ident, "expected binding name")?.span;
            let annotation = if self.take(Kind::Colon).is_some() {
                Some(self.ty()?)
            } else {
                None
            };
            self.expect(Kind::Equal, "binding requires an initializer")?;
            StmtKind::Let {
                mutable,
                name,
                annotation,
                init: self.expression(0, LiteralContext::Allowed)?,
            }
        } else if self.take(Kind::Break).is_some() {
            StmtKind::Break
        } else if self.take(Kind::Continue).is_some() {
            StmtKind::Continue
        } else if self.take(Kind::Return).is_some() {
            StmtKind::Return(if self.peek().kind == Kind::Semi {
                None
            } else {
                Some(self.expression(0, LiteralContext::Allowed)?)
            })
        } else if self.mode == SourceMode::OwnedCandidate
            && self.peek().kind == Kind::Ident
            && self.tokens[self.cursor + 1..]
                .iter()
                .filter(|token| token.kind != Kind::Trivia)
                .take(3)
                .map(|token| token.kind)
                .eq([Kind::Dot, Kind::Ident, Kind::Equal])
        {
            let base = self.bump().span;
            self.bump();
            let field = self.bump().span;
            let operator_span = self.bump().span;
            StmtKind::FieldAssign {
                base,
                field,
                target_span: self.source.span(base.start, field.end),
                operator_span,
                value: self.expression(0, LiteralContext::Allowed)?,
            }
        } else if self.peek().kind == Kind::Ident
            && self.tokens[self.cursor + 1..]
                .iter()
                .find(|token| token.kind != Kind::Trivia)
                .is_some_and(|token| token.kind == Kind::Equal)
        {
            let name = self.bump().span;
            let operator_span = self.expect(Kind::Equal, "expected assignment `=`")?.span;
            StmtKind::Assign {
                name,
                operator_span,
                value: self.expression(0, LiteralContext::Allowed)?,
            }
        } else {
            StmtKind::Expr(self.expression(0, LiteralContext::Allowed)?)
        };
        if matches!(kind, StmtKind::Break | StmtKind::Continue) && self.peek().kind != Kind::Semi {
            return Err(self.diagnostic(
                "E0100",
                "parse",
                "loop transfer requires `;`; values and labels are unavailable",
                Some(self.peek().span),
            ));
        }
        let end = self.expect(Kind::Semi, "statement requires `;`")?.span.end;
        Ok(Stmt {
            kind,
            span: self.source.span(start, end),
        })
    }
    fn comparison_op(&self) -> Option<ComparisonOp> {
        Some(match self.peek().kind {
            Kind::EqualEqual => ComparisonOp::Equal,
            Kind::NotEqual => ComparisonOp::NotEqual,
            Kind::Less => ComparisonOp::Less,
            Kind::LessEqual => ComparisonOp::LessEqual,
            Kind::Greater => ComparisonOp::Greater,
            Kind::GreaterEqual => ComparisonOp::GreaterEqual,
            _ => return None,
        })
    }
    fn expression(
        &mut self,
        depth: usize,
        context: LiteralContext,
    ) -> Result<ExprId, Box<Diagnostic>> {
        self.logical(depth, LogicalOp::Or, context)
    }
    fn logical(
        &mut self,
        depth: usize,
        op: LogicalOp,
        context: LiteralContext,
    ) -> Result<ExprId, Box<Diagnostic>> {
        let child = |parser: &mut Self| match op {
            LogicalOp::Or => parser.logical(depth, LogicalOp::And, context),
            LogicalOp::And => parser.comparison(depth, context),
        };
        let mut left = child(self)?;
        let token = match op {
            LogicalOp::And => Kind::AndAnd,
            LogicalOp::Or => Kind::OrOr,
        };
        while let Some(operator) = self.take(token) {
            self.node()?;
            let right = child(self)?;
            let span = self.source.span(
                self.expressions[left.0].span.start,
                self.expressions[right.0].span.end,
            );
            left = self.push_expr(
                ExprKind::Logical {
                    op,
                    left,
                    right,
                    operator_span: operator.span,
                },
                span,
            )?;
        }
        Ok(left)
    }
    fn comparison(
        &mut self,
        depth: usize,
        context: LiteralContext,
    ) -> Result<ExprId, Box<Diagnostic>> {
        let left = self.sum(depth, context)?;
        let Some(op) = self.comparison_op() else {
            return Ok(left);
        };
        let operator_span = self.bump().span;
        self.node()?;
        let right = self.sum(depth, context)?;
        if self.comparison_op().is_some() {
            return Err(self.error("comparison operators cannot be chained; use parentheses"));
        }
        self.push_expr(
            ExprKind::Comparison {
                op,
                left,
                right,
                operator_span,
            },
            self.source.span(
                self.expressions[left.0].span.start,
                self.expressions[right.0].span.end,
            ),
        )
    }
    fn sum(&mut self, depth: usize, context: LiteralContext) -> Result<ExprId, Box<Diagnostic>> {
        let mut left = self.product(depth, context)?;
        loop {
            let op = match self.peek().kind {
                Kind::Plus => ArithmeticOp::Add,
                Kind::Minus => ArithmeticOp::Subtract,
                _ => return Ok(left),
            };
            let operator_span = self.bump().span;
            self.node()?;
            let right = self.product(depth, context)?;
            left = self.binary(op, left, right, operator_span)?;
        }
    }
    fn product(
        &mut self,
        depth: usize,
        context: LiteralContext,
    ) -> Result<ExprId, Box<Diagnostic>> {
        let mut left = self.unary(depth, context)?;
        while let Some(token) = self.take(Kind::Star) {
            self.node()?;
            let right = self.unary(depth, context)?;
            left = self.binary(ArithmeticOp::Multiply, left, right, token.span)?;
        }
        Ok(left)
    }
    fn unary(&mut self, depth: usize, context: LiteralContext) -> Result<ExprId, Box<Diagnostic>> {
        let mut prefixes = Vec::new();
        while self.peek().kind == Kind::Not {
            if depth + prefixes.len() >= MAX_NESTING {
                return Err(self.diagnostic(
                    "E0400",
                    "parse",
                    "expression nesting limit exceeded",
                    Some(self.peek().span),
                ));
            }
            self.node()?;
            prefixes.push(self.bump().span);
        }
        let mut operand = self.primary(depth + prefixes.len(), context)?;
        for operator_span in prefixes.into_iter().rev() {
            let span = self
                .source
                .span(operator_span.start, self.expressions[operand.0].span.end);
            operand = self.push_expr(
                ExprKind::Not {
                    operand,
                    operator_span,
                },
                span,
            )?;
        }
        Ok(operand)
    }
    fn binary(
        &mut self,
        op: ArithmeticOp,
        left: ExprId,
        right: ExprId,
        operator_span: super::source::Span,
    ) -> Result<ExprId, Box<Diagnostic>> {
        let span = self.source.span(
            self.expressions[left.0].span.start,
            self.expressions[right.0].span.end,
        );
        self.push_expr(
            ExprKind::Arithmetic {
                op,
                left,
                right,
                operator_span,
            },
            span,
        )
    }
    fn push_expr(
        &mut self,
        kind: ExprKind,
        span: super::source::Span,
    ) -> Result<ExprId, Box<Diagnostic>> {
        // A flat left-associative chain is a deep tree too. Bound total tree
        // height before resolution, whose recursive visits now remain <= 64.
        let height = 1 + match &kind {
            ExprKind::Group(inner) | ExprKind::Not { operand: inner, .. } => self.heights[inner.0],
            ExprKind::Call { args, .. } => args
                .iter()
                .map(|arg| match arg {
                    Argument::Value(id) => self.heights[id.0],
                    Argument::Borrow { .. } => 1,
                })
                .max()
                .unwrap_or(0),
            ExprKind::StructLiteral { fields, .. } => fields
                .iter()
                .map(|f| self.heights[f.value.0])
                .max()
                .unwrap_or(0),
            ExprKind::Arithmetic { left, right, .. }
            | ExprKind::Comparison { left, right, .. }
            | ExprKind::Logical { left, right, .. } => {
                self.heights[left.0].max(self.heights[right.0])
            }
            _ => 0,
        };
        if height > MAX_NESTING {
            return Err(self.diagnostic(
                "E0400",
                "parse",
                "expression nesting limit exceeded",
                Some(span),
            ));
        }
        let id = ExprId(self.expressions.len());
        self.expressions.push(Expr { kind, span });
        self.heights.push(height);
        Ok(id)
    }
    fn primary(
        &mut self,
        depth: usize,
        context: LiteralContext,
    ) -> Result<ExprId, Box<Diagnostic>> {
        if depth >= MAX_NESTING {
            return Err(self.diagnostic(
                "E0400",
                "parse",
                "expression nesting limit exceeded",
                Some(self.peek().span),
            ));
        }
        self.node()?;
        let token = self.peek();
        let mut end = token.span.end;
        let kind = match token.kind {
            Kind::True | Kind::False => {
                self.bump();
                ExprKind::Bool(token.kind == Kind::True)
            }
            Kind::Number | Kind::Minus => {
                let negative = token.kind == Kind::Minus;
                if negative {
                    self.bump();
                    if self.peek().kind != Kind::Number {
                        return Err(self.diagnostic(
                            "E0101",
                            "parse",
                            "only a minus followed by decimal literal digits is supported",
                            Some(token.span),
                        ));
                    }
                }
                let digits = self.bump().span;
                end = digits.end;
                if !self.source.text()[digits.start..digits.end]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit())
                {
                    return Err(self.diagnostic(
                        "E0101",
                        "parse",
                        "unsupported numeric spelling; expected ASCII decimal literal digits",
                        Some(self.source.span(token.span.start, end)),
                    ));
                }
                ExprKind::Number { digits, negative }
            }
            Kind::Ident => {
                self.bump();
                if self.take(Kind::LParen).is_some() {
                    let mut args = Vec::new();
                    if self.peek().kind != Kind::RParen {
                        loop {
                            if args.len() >= MAX_PARAMS {
                                return Err(self.diagnostic(
                                    "E0400",
                                    "parse",
                                    "argument limit exceeded",
                                    Some(self.peek().span),
                                ));
                            }
                            args.push(self.argument(depth + 1)?);
                            if self.take(Kind::Comma).is_none() {
                                break;
                            }
                        }
                    }
                    end = self.expect(Kind::RParen, "call requires `)`")?.span.end;
                    ExprKind::Call {
                        callee: token.span,
                        args,
                    }
                } else if self.mode == SourceMode::OwnedCandidate && self.take(Kind::Dot).is_some()
                {
                    let field = self
                        .expect(Kind::Ident, "expected field name after `.`")?
                        .span;
                    end = field.end;
                    ExprKind::FieldRead {
                        base: token.span,
                        field,
                    }
                } else if self.mode == SourceMode::OwnedCandidate
                    && context == LiteralContext::Allowed
                    && self.take(Kind::LBrace).is_some()
                {
                    let mut fields = Vec::new();
                    while self.peek().kind != Kind::RBrace {
                        self.node()?;
                        let name = self
                            .expect(Kind::Ident, "expected literal field name")?
                            .span;
                        self.expect(Kind::Colon, "literal field requires `:` and a value")?;
                        let value = self.expression(depth + 1, LiteralContext::Allowed)?;
                        fields.push(FieldInit {
                            name,
                            value,
                            span: self
                                .source
                                .span(name.start, self.expressions[value.0].span.end),
                        });
                        if self.take(Kind::Comma).is_none() {
                            break;
                        }
                    }
                    end = self.expect(Kind::RBrace, "expected literal `}`")?.span.end;
                    ExprKind::StructLiteral {
                        record: token.span,
                        fields,
                    }
                } else {
                    ExprKind::Name(token.span)
                }
            }
            Kind::LParen => {
                self.bump();
                if let Some(close) = self.take(Kind::RParen) {
                    end = close.span.end;
                    ExprKind::Unit
                } else {
                    let inner = self.expression(depth + 1, LiteralContext::Allowed)?;
                    end = self.expect(Kind::RParen, "grouping requires `)`")?.span.end;
                    ExprKind::Group(inner)
                }
            }
            _ => return Err(self.error("expected a bool, i32 or unit expression")),
        };
        self.push_expr(kind, self.source.span(token.span.start, end))
    }
}

#[cfg(test)]
pub(super) fn parse_with_lowered_node_limit(
    source: &SourceFile,
    tokens: Vec<Token>,
    mode: SourceMode,
    limit: usize,
) -> Result<Program, Vec<Diagnostic>> {
    parse_with_node_limit(source, tokens, mode, limit)
}
