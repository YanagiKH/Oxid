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

pub fn parse(source: &SourceFile, tokens: Vec<Token>) -> Result<Program, Vec<Diagnostic>> {
    let mut parser = Parser {
        source,
        tokens,
        cursor: 0,
        expressions: Vec::new(),
        nodes: 0,
    };
    parser.skip();
    let mut functions = Vec::new();
    let mut diagnostics = Vec::new();
    while parser.peek().kind != Kind::Eof && diagnostics.len() < MAX_DIAGNOSTICS {
        match parser.function() {
            Ok(function) => functions.push(function),
            Err(error) => {
                diagnostics.push(*error);
                // Function synchronization always consumes or stops at a new fn.
                while !matches!(parser.peek().kind, Kind::Fn | Kind::Eof) {
                    parser.bump();
                }
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(Program {
            tokens: parser.tokens,
            functions,
            expressions: parser.expressions,
        })
    } else {
        Err(diagnostics)
    }
}
struct Parser<'a> {
    source: &'a SourceFile,
    tokens: Vec<Token>,
    cursor: usize,
    expressions: Vec<Expr>,
    nodes: usize,
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
    fn error(&self, message: &str) -> Box<Diagnostic> {
        let token = self.peek();
        let unsupported = matches!(
            token.kind,
            Kind::Unsupported | Kind::Number | Kind::String | Kind::Equal
        );
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
        if self.nodes >= MAX_NODES {
            return Err(Diagnostic::new(
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
                    return Err(Diagnostic::new(
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
                    ty: self.ty()?,
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
        let start = self.peek().span.start;
        let end = if self.take(Kind::LParen).is_some() {
            self.expect(Kind::RParen, "only the unit type `()` is supported here")?
                .span
                .end
        } else {
            self.expect(Kind::Ident, "expected `bool` or `()` type")?
                .span
                .end
        };
        Ok(TypeSyntax {
            span: self.source.span(start, end),
        })
    }
    fn statement(
        &mut self,
        blocks: &mut Vec<BodyBlock>,
        depth: usize,
    ) -> Result<Stmt, Box<Diagnostic>> {
        self.node()?;
        let start = self.peek().span.start;
        if self.take(Kind::If).is_some() {
            let condition = self.expression(0)?;
            // `depth` is the number of active statement blocks. Check before
            // recursive entry or arena allocation for either arm.
            if depth >= MAX_BLOCK_NESTING {
                return Err(Diagnostic::new(
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
            let name = self
                .expect(Kind::Ident, "expected immutable binding name")?
                .span;
            let annotation = if self.take(Kind::Colon).is_some() {
                Some(self.ty()?)
            } else {
                None
            };
            self.expect(Kind::Equal, "binding requires an initializer")?;
            StmtKind::Let {
                name,
                annotation,
                init: self.expression(0)?,
            }
        } else if self.take(Kind::Return).is_some() {
            StmtKind::Return(if self.peek().kind == Kind::Semi {
                None
            } else {
                Some(self.expression(0)?)
            })
        } else {
            StmtKind::Expr(self.expression(0)?)
        };
        let end = self.expect(Kind::Semi, "statement requires `;`")?.span.end;
        Ok(Stmt {
            kind,
            span: self.source.span(start, end),
        })
    }
    fn expression(&mut self, depth: usize) -> Result<ExprId, Box<Diagnostic>> {
        if depth >= MAX_NESTING {
            return Err(Diagnostic::new(
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
            Kind::Ident => {
                self.bump();
                if self.take(Kind::LParen).is_some() {
                    let mut args = Vec::new();
                    if self.peek().kind != Kind::RParen {
                        loop {
                            if args.len() >= MAX_PARAMS {
                                return Err(Diagnostic::new(
                                    "E0400",
                                    "parse",
                                    "argument limit exceeded",
                                    Some(self.peek().span),
                                ));
                            }
                            args.push(self.expression(depth + 1)?);
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
                    let inner = self.expression(depth + 1)?;
                    end = self.expect(Kind::RParen, "grouping requires `)`")?.span.end;
                    ExprKind::Group(inner)
                }
            }
            _ => return Err(self.error("expected a bool/unit expression")),
        };
        let id = ExprId(self.expressions.len());
        self.expressions.push(Expr {
            kind,
            span: self.source.span(token.span.start, end),
        });
        Ok(id)
    }
}
