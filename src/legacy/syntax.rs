#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SourceSpan {
    pub(super) source: String,
    pub(super) start_line: usize,
    pub(super) start_col: usize,
    pub(super) end_line: usize,
    pub(super) end_col: usize,
}

impl SourceSpan {
    pub(super) fn render(&self) -> String {
        format!(
            "{}:{}:{}-{}:{}",
            self.source, self.start_line, self.start_col, self.end_line, self.end_col
        )
    }
}

#[derive(Clone, Debug)]
struct Token {
    kind: TokenKind,
    line: usize,
    col: usize,
    end_line: usize,
    end_col: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum TokenKind {
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Colon,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Star,
    Percent,
    PipeGreater,
    FatArrow,
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Identifier(String),
    String(String),
    Number(f64),
    Invalid(String),
    Fn,
    Async,
    Await,
    Let,
    Const,
    If,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    Return,
    True,
    False,
    Null,
    Print,
    Use,
    And,
    Or,
    Eof,
}

#[derive(Clone, Debug)]
pub(super) enum Literal {
    Number(f64),
    String(String),
    Bool(bool),
    Null,
}

#[derive(Clone, Debug)]
pub(super) enum Expr {
    Literal(Literal),
    Variable(String),
    Assign(String, Box<Expr>),
    AssignIndex(Box<Expr>, Box<Expr>, Box<Expr>),
    Unary(TokenKind, Box<Expr>),
    Binary(Box<Expr>, TokenKind, Box<Expr>),
    Logical(Box<Expr>, TokenKind, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Array(Vec<Expr>),
    Record(Vec<(String, Expr)>),
    Property(Box<Expr>, String),
    AssignProperty(Box<Expr>, String, Box<Expr>),
    Await(Box<Expr>),
    Grouping(Box<Expr>),
}

#[derive(Clone, Debug)]
pub(super) enum Stmt {
    Located(SourceSpan, Box<Stmt>),
    Let(String, Expr),
    Const(String, Expr),
    Print(Expr),
    Expr(Expr),
    Block(Vec<Stmt>),
    If {
        cond: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    For {
        name: String,
        iterable: Expr,
        body: Box<Stmt>,
    },
    Function {
        name: String,
        params: Vec<String>,
        body: Vec<Stmt>,
        is_async: bool,
    },
    Return(Option<Expr>),
    Break,
    Continue,
    Use(String),
}

#[derive(Clone, Debug)]
pub(super) struct Program {
    pub(super) stmts: Vec<Stmt>,
}

#[derive(Debug)]
pub(super) struct Parser {
    tokens: Vec<Token>,
    current: usize,
    source: String,
}

impl Parser {
    pub(super) fn new(source: &str) -> Self {
        Self::new_with_source(source, "<memory>")
    }

    pub(super) fn new_with_source(source: &str, source_name: impl Into<String>) -> Self {
        Self {
            tokens: Lexer::new(source).lex(),
            current: 0,
            source: source_name.into(),
        }
    }

    pub(super) fn parse_program(&mut self) -> Result<Program, String> {
        let mut stmts = Vec::new();
        while !self.is_at_end() {
            stmts.push(self.declaration()?);
        }
        Ok(Program { stmts })
    }

    fn declaration(&mut self) -> Result<Stmt, String> {
        let start = self.current;
        let statement = self.declaration_inner()?;
        Ok(Stmt::Located(self.span_from(start), Box::new(statement)))
    }

    fn declaration_inner(&mut self) -> Result<Stmt, String> {
        let is_async = self.match_simple(&[TokenKind::Async]);
        if self.match_simple(&[TokenKind::Fn]) {
            return self.function_decl(is_async);
        }
        if self.match_simple(&[TokenKind::Const]) {
            return self.const_decl();
        }
        if self.match_simple(&[TokenKind::Let]) {
            return self.let_decl();
        }
        if self.match_simple(&[TokenKind::Use]) {
            return self.use_decl();
        }
        if is_async {
            return Err(self.error_here("`async` must be followed by `fn`"));
        }
        self.statement()
    }

    fn function_decl(&mut self, is_async: bool) -> Result<Stmt, String> {
        let name = self.consume_identifier("function name is required")?;
        self.consume_simple(TokenKind::LeftParen, "function definition requires `(`")?;
        let mut params = Vec::new();
        if !self.check_simple(&TokenKind::RightParen) {
            loop {
                params.push(self.consume_identifier("parameter name is required")?);
                if !self.match_simple(&[TokenKind::Comma]) {
                    break;
                }
            }
        }
        self.consume_simple(TokenKind::RightParen, "function definition requires `)`")?;
        if self.match_simple(&[TokenKind::FatArrow, TokenKind::Equal]) {
            let expr = self.expression()?;
            self.consume_simple(TokenKind::Semicolon, "short function must end with `;`")?;
            return Ok(Stmt::Function {
                name,
                params,
                body: vec![Stmt::Return(Some(expr))],
                is_async,
            });
        }
        self.consume_simple(TokenKind::LeftBrace, "function body requires `{`")?;
        let body = self.block_stmts()?;
        Ok(Stmt::Function {
            name,
            params,
            body,
            is_async,
        })
    }

    fn const_decl(&mut self) -> Result<Stmt, String> {
        let name = self.consume_identifier("constant name is required")?;
        self.consume_simple(TokenKind::Equal, "constant declaration requires `=`")?;
        let expr = self.expression()?;
        self.consume_simple(
            TokenKind::Semicolon,
            "constant declaration must end with `;`",
        )?;
        Ok(Stmt::Const(name, expr))
    }

    fn let_decl(&mut self) -> Result<Stmt, String> {
        let name = self.consume_identifier("variable name is required")?;
        self.consume_simple(TokenKind::Equal, "assignment requires `=`")?;
        let expr = self.expression()?;
        self.consume_simple(TokenKind::Semicolon, "declaration must end with `;`")?;
        Ok(Stmt::Let(name, expr))
    }

    fn use_decl(&mut self) -> Result<Stmt, String> {
        let path = match self.advance().kind.clone() {
            TokenKind::String(s) => s,
            other => {
                return Err(
                    self.error_here(&format!("`use` requires a string path, found {:?}", other))
                )
            }
        };
        self.consume_simple(TokenKind::Semicolon, "`use` must end with `;`")?;
        Ok(Stmt::Use(path))
    }

    fn statement(&mut self) -> Result<Stmt, String> {
        if self.match_simple(&[TokenKind::Print]) {
            let expr = self.expression()?;
            self.consume_simple(TokenKind::Semicolon, "`print` must end with `;`")?;
            return Ok(Stmt::Print(expr));
        }
        if self.match_simple(&[TokenKind::Return]) {
            if self.check_simple(&TokenKind::Semicolon) {
                self.advance();
                return Ok(Stmt::Return(None));
            }
            let expr = self.expression()?;
            self.consume_simple(TokenKind::Semicolon, "`return` must end with `;`")?;
            return Ok(Stmt::Return(Some(expr)));
        }
        if self.match_simple(&[TokenKind::Break]) {
            self.consume_simple(TokenKind::Semicolon, "`break` must end with `;`")?;
            return Ok(Stmt::Break);
        }
        if self.match_simple(&[TokenKind::Continue]) {
            self.consume_simple(TokenKind::Semicolon, "`continue` must end with `;`")?;
            return Ok(Stmt::Continue);
        }
        if self.match_simple(&[TokenKind::If]) {
            return self.if_stmt();
        }
        if self.match_simple(&[TokenKind::While]) {
            return self.while_stmt();
        }
        if self.match_simple(&[TokenKind::For]) {
            return self.for_stmt();
        }
        if self.match_simple(&[TokenKind::LeftBrace]) {
            return Ok(Stmt::Block(self.block_stmts()?));
        }
        let expr = self.expression()?;
        self.consume_simple(
            TokenKind::Semicolon,
            "expression statements must end with `;`",
        )?;
        Ok(Stmt::Expr(expr))
    }

    fn if_stmt(&mut self) -> Result<Stmt, String> {
        let parenthesized = self.match_simple(&[TokenKind::LeftParen]);
        let cond = self.expression()?;
        if parenthesized {
            self.consume_simple(TokenKind::RightParen, "`if` condition requires `)`")?;
        }
        let then_branch = Box::new(self.statement()?);
        let else_branch = if self.match_simple(&[TokenKind::Else]) {
            Some(Box::new(self.statement()?))
        } else {
            None
        };
        Ok(Stmt::If {
            cond,
            then_branch,
            else_branch,
        })
    }

    fn while_stmt(&mut self) -> Result<Stmt, String> {
        let parenthesized = self.match_simple(&[TokenKind::LeftParen]);
        let cond = self.expression()?;
        if parenthesized {
            self.consume_simple(TokenKind::RightParen, "`while` condition requires `)`")?;
        }
        let body = Box::new(self.statement()?);
        Ok(Stmt::While { cond, body })
    }

    fn for_stmt(&mut self) -> Result<Stmt, String> {
        let name = self.consume_identifier("`for` requires an item name")?;
        self.consume_simple(TokenKind::In, "`for` requires `in`")?;
        let iterable = self.expression()?;
        let body = Box::new(self.statement()?);
        Ok(Stmt::For {
            name,
            iterable,
            body,
        })
    }

    fn block_stmts(&mut self) -> Result<Vec<Stmt>, String> {
        let mut stmts = Vec::new();
        while !self.check_simple(&TokenKind::RightBrace) && !self.is_at_end() {
            stmts.push(self.declaration()?);
        }
        self.consume_simple(TokenKind::RightBrace, "block requires `}`")?;
        Ok(stmts)
    }

    fn expression(&mut self) -> Result<Expr, String> {
        self.assignment()
    }

    fn assignment(&mut self) -> Result<Expr, String> {
        let expr = self.pipeline()?;
        if self.match_simple(&[TokenKind::Equal]) {
            let value = self.assignment()?;
            return match expr {
                Expr::Variable(name) => Ok(Expr::Assign(name, Box::new(value))),
                Expr::Index(target, index) => Ok(Expr::AssignIndex(target, index, Box::new(value))),
                Expr::Property(target, name) => {
                    Ok(Expr::AssignProperty(target, name, Box::new(value)))
                }
                _ => {
                    Err(self
                        .error_here("assignment target must be an identifier, index, or property"))
                }
            };
        }
        Ok(expr)
    }

    fn pipeline(&mut self) -> Result<Expr, String> {
        let mut expr = self.or()?;
        while self.match_simple(&[TokenKind::PipeGreater]) {
            let next = self.or()?;
            expr = match next {
                Expr::Call(callee, mut args) => {
                    args.insert(0, expr);
                    Expr::Call(callee, args)
                }
                callee => Expr::Call(Box::new(callee), vec![expr]),
            };
        }
        Ok(expr)
    }

    fn or(&mut self) -> Result<Expr, String> {
        let mut expr = self.and()?;
        while self.match_simple(&[TokenKind::Or]) {
            let right = self.and()?;
            expr = Expr::Logical(Box::new(expr), TokenKind::Or, Box::new(right));
        }
        Ok(expr)
    }

    fn and(&mut self) -> Result<Expr, String> {
        let mut expr = self.equality()?;
        while self.match_simple(&[TokenKind::And]) {
            let right = self.equality()?;
            expr = Expr::Logical(Box::new(expr), TokenKind::And, Box::new(right));
        }
        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr, String> {
        let mut expr = self.comparison()?;
        while self.match_simple(&[TokenKind::BangEqual, TokenKind::EqualEqual]) {
            let op = self.previous().kind.clone();
            let right = self.comparison()?;
            expr = Expr::Binary(Box::new(expr), op, Box::new(right));
        }
        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, String> {
        let mut expr = self.term()?;
        while self.match_simple(&[
            TokenKind::Greater,
            TokenKind::GreaterEqual,
            TokenKind::Less,
            TokenKind::LessEqual,
        ]) {
            let op = self.previous().kind.clone();
            let right = self.term()?;
            expr = Expr::Binary(Box::new(expr), op, Box::new(right));
        }
        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, String> {
        let mut expr = self.factor()?;
        while self.match_simple(&[TokenKind::Plus, TokenKind::Minus]) {
            let op = self.previous().kind.clone();
            let right = self.factor()?;
            expr = Expr::Binary(Box::new(expr), op, Box::new(right));
        }
        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, String> {
        let mut expr = self.unary()?;
        while self.match_simple(&[TokenKind::Star, TokenKind::Slash, TokenKind::Percent]) {
            let op = self.previous().kind.clone();
            let right = self.unary()?;
            expr = Expr::Binary(Box::new(expr), op, Box::new(right));
        }
        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.match_simple(&[TokenKind::Await]) {
            return Ok(Expr::Await(Box::new(self.unary()?)));
        }
        if self.match_simple(&[TokenKind::Bang, TokenKind::Minus]) {
            let op = self.previous().kind.clone();
            let right = self.unary()?;
            return Ok(Expr::Unary(op, Box::new(right)));
        }
        self.call()
    }

    fn call(&mut self) -> Result<Expr, String> {
        let mut expr = self.primary()?;
        loop {
            if self.match_simple(&[TokenKind::LeftParen]) {
                let mut args = Vec::new();
                if !self.check_simple(&TokenKind::RightParen) {
                    loop {
                        args.push(self.expression()?);
                        if !self.match_simple(&[TokenKind::Comma]) {
                            break;
                        }
                    }
                }
                self.consume_simple(TokenKind::RightParen, "call requires `)`")?;
                expr = Expr::Call(Box::new(expr), args);
                continue;
            }
            if self.match_simple(&[TokenKind::LeftBracket]) {
                let index = self.expression()?;
                self.consume_simple(TokenKind::RightBracket, "index requires `]`")?;
                expr = Expr::Index(Box::new(expr), Box::new(index));
                continue;
            }
            if self.match_simple(&[TokenKind::Dot]) {
                let name = self.consume_identifier("property name is required after `.`")?;
                expr = Expr::Property(Box::new(expr), name);
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        if self.match_simple(&[TokenKind::False]) {
            return Ok(Expr::Literal(Literal::Bool(false)));
        }
        if self.match_simple(&[TokenKind::True]) {
            return Ok(Expr::Literal(Literal::Bool(true)));
        }
        if self.match_simple(&[TokenKind::Null]) {
            return Ok(Expr::Literal(Literal::Null));
        }
        if self.check_number() {
            if let TokenKind::Number(n) = self.advance().kind.clone() {
                return Ok(Expr::Literal(Literal::Number(n)));
            }
        }
        if self.check_string() {
            if let TokenKind::String(s) = self.advance().kind.clone() {
                return Ok(Expr::Literal(Literal::String(s)));
            }
        }
        if self.check_identifier() {
            if let TokenKind::Identifier(name) = self.advance().kind.clone() {
                return Ok(Expr::Variable(name));
            }
        }
        if self.match_simple(&[TokenKind::LeftBracket]) {
            let mut items = Vec::new();
            if !self.check_simple(&TokenKind::RightBracket) {
                loop {
                    items.push(self.expression()?);
                    if !self.match_simple(&[TokenKind::Comma]) {
                        break;
                    }
                }
            }
            self.consume_simple(TokenKind::RightBracket, "array requires `]`")?;
            return Ok(Expr::Array(items));
        }
        if self.match_simple(&[TokenKind::LeftBrace]) {
            let mut fields = Vec::new();
            if !self.check_simple(&TokenKind::RightBrace) {
                loop {
                    let key_token = self.advance().clone();
                    let key = match key_token.kind.clone() {
                        TokenKind::Identifier(name) | TokenKind::String(name) => name,
                        other => {
                            return Err(self.error_at(
                                &key_token,
                                &format!(
                                    "record key must be an identifier or string, found {:?}",
                                    other
                                ),
                            ))
                        }
                    };
                    self.consume_simple(TokenKind::Colon, "record field requires `:`")?;
                    let value = self.expression()?;
                    fields.push((key, value));
                    if !self.match_simple(&[TokenKind::Comma]) {
                        break;
                    }
                    if self.check_simple(&TokenKind::RightBrace) {
                        break;
                    }
                }
            }
            self.consume_simple(TokenKind::RightBrace, "record requires `}`")?;
            return Ok(Expr::Record(fields));
        }
        if self.match_simple(&[TokenKind::LeftParen]) {
            let expr = self.expression()?;
            self.consume_simple(TokenKind::RightParen, "grouping requires `)`")?;
            return Ok(Expr::Grouping(Box::new(expr)));
        }
        if let TokenKind::Invalid(message) = &self.peek().kind {
            return Err(self.error_here(message));
        }
        if self.match_simple(&[TokenKind::Eof]) {
            return Err(self.error_here("unexpected end of file"));
        }
        Err(self.error_here(&format!(
            "token cannot be parsed as an expression: {:?}",
            self.peek().kind
        )))
    }

    fn consume_identifier(&mut self, message: &str) -> Result<String, String> {
        if self.check_identifier() {
            if let TokenKind::Identifier(name) = self.advance().kind.clone() {
                return Ok(name);
            }
        }
        Err(self.error_here(message))
    }

    fn consume_simple(&mut self, kind: TokenKind, message: &str) -> Result<(), String> {
        if self.check_simple(&kind) {
            self.advance();
            Ok(())
        } else {
            Err(self.error_here(message))
        }
    }

    fn error_here(&self, message: &str) -> String {
        self.error_at(self.peek(), message)
    }

    fn error_at(&self, token: &Token, message: &str) -> String {
        format!(
            "{}:{}:{}-{}:{}: {}",
            self.source, token.line, token.col, token.end_line, token.end_col, message
        )
    }

    fn span_from(&self, start: usize) -> SourceSpan {
        let first = self.tokens.get(start).unwrap_or_else(|| self.peek());
        let last_index = self.current.saturating_sub(1).max(start);
        let last = self.tokens.get(last_index).unwrap_or(first);
        SourceSpan {
            source: self.source.clone(),
            start_line: first.line,
            start_col: first.col,
            end_line: last.end_line,
            end_col: last.end_col,
        }
    }

    fn match_simple(&mut self, kinds: &[TokenKind]) -> bool {
        for kind in kinds {
            if self.check_simple(kind) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn check_simple(&self, kind: &TokenKind) -> bool {
        use TokenKind::*;
        matches!(
            (&self.peek().kind, kind),
            (LeftParen, LeftParen)
                | (RightParen, RightParen)
                | (LeftBrace, LeftBrace)
                | (RightBrace, RightBrace)
                | (LeftBracket, LeftBracket)
                | (RightBracket, RightBracket)
                | (Comma, Comma)
                | (Colon, Colon)
                | (Dot, Dot)
                | (Minus, Minus)
                | (Plus, Plus)
                | (Semicolon, Semicolon)
                | (Slash, Slash)
                | (Star, Star)
                | (Percent, Percent)
                | (PipeGreater, PipeGreater)
                | (FatArrow, FatArrow)
                | (Bang, Bang)
                | (BangEqual, BangEqual)
                | (Equal, Equal)
                | (EqualEqual, EqualEqual)
                | (Greater, Greater)
                | (GreaterEqual, GreaterEqual)
                | (Less, Less)
                | (LessEqual, LessEqual)
                | (Fn, Fn)
                | (Async, Async)
                | (Await, Await)
                | (Let, Let)
                | (Const, Const)
                | (If, If)
                | (Else, Else)
                | (While, While)
                | (For, For)
                | (In, In)
                | (Break, Break)
                | (Continue, Continue)
                | (Return, Return)
                | (True, True)
                | (False, False)
                | (Null, Null)
                | (Print, Print)
                | (Use, Use)
                | (And, And)
                | (Or, Or)
                | (Eof, Eof)
        )
    }

    fn check_identifier(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Identifier(_))
    }
    fn check_number(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Number(_))
    }
    fn check_string(&self) -> bool {
        matches!(self.peek().kind, TokenKind::String(_))
    }
    fn is_at_end(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }
    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }
    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }
    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }
}

struct Lexer<'a> {
    chars: Vec<char>,
    current: usize,
    line: usize,
    col: usize,
    _source: &'a str,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            chars: source.chars().collect(),
            current: 0,
            line: 1,
            col: 1,
            _source: source,
        }
    }

    fn lex(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while !self.is_at_end() {
            if let Some(token) = self.scan_token() {
                tokens.push(token);
            }
        }
        tokens.push(Token {
            kind: TokenKind::Eof,
            line: self.line,
            col: self.col,
            end_line: self.line,
            end_col: self.col,
        });
        tokens
    }

    fn scan_token(&mut self) -> Option<Token> {
        let c = self.advance();
        let line = self.line;
        let col = self.col.saturating_sub(1);
        use TokenKind::*;
        let kind = match c {
            '(' => LeftParen,
            ')' => RightParen,
            '{' => LeftBrace,
            '}' => RightBrace,
            '[' => LeftBracket,
            ']' => RightBracket,
            ',' => Comma,
            ':' => Colon,
            '.' => Dot,
            '-' => Minus,
            '+' => Plus,
            ';' => Semicolon,
            '*' => Star,
            '%' => Percent,
            '|' => {
                if self.match_char('>') {
                    PipeGreater
                } else {
                    Invalid("`|` must be followed by `>`".to_string())
                }
            }
            '!' => {
                if self.match_char('=') {
                    BangEqual
                } else {
                    Bang
                }
            }
            '=' => {
                if self.match_char('=') {
                    EqualEqual
                } else if self.match_char('>') {
                    FatArrow
                } else {
                    Equal
                }
            }
            '<' => {
                if self.match_char('=') {
                    LessEqual
                } else {
                    Less
                }
            }
            '>' => {
                if self.match_char('=') {
                    GreaterEqual
                } else {
                    Greater
                }
            }
            '/' => {
                if self.match_char('/') {
                    while self.peek() != '\n' && !self.is_at_end() {
                        self.advance();
                    }
                    return None;
                } else if self.match_char('*') {
                    self.block_comment();
                    return None;
                } else {
                    Slash
                }
            }
            '#' => {
                while self.peek() != '\n' && !self.is_at_end() {
                    self.advance();
                }
                return None;
            }
            ' ' | '\r' | '\t' => return None,
            '\n' => {
                self.line += 1;
                self.col = 1;
                return None;
            }
            '"' => return Some(self.string_token(line, col)),
            c if c.is_ascii_digit() => return Some(self.number_token(c, line, col)),
            c if is_alpha(c) => return Some(self.identifier_token(c, line, col)),
            _ => Invalid(format!("unknown character `{}`", c)),
        };
        Some(Token {
            kind,
            line,
            col,
            end_line: self.line,
            end_col: self.col,
        })
    }

    fn block_comment(&mut self) {
        while !self.is_at_end() {
            if self.peek() == '*' && self.peek_next() == '/' {
                self.advance();
                self.advance();
                return;
            }
            if self.peek() == '\n' {
                self.line += 1;
                self.col = 1;
            }
            self.advance();
        }
    }

    fn string_token(&mut self, line: usize, col: usize) -> Token {
        let mut value = String::new();
        while !self.is_at_end() && self.peek() != '"' {
            let c = self.advance();
            if c == '\\' && !self.is_at_end() {
                let esc = self.advance();
                value.push(match esc {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '"' => '"',
                    '\\' => '\\',
                    other => other,
                });
            } else {
                if c == '\n' {
                    self.line += 1;
                    self.col = 1;
                }
                value.push(c);
            }
        }
        if self.is_at_end() {
            Token {
                kind: TokenKind::Invalid("unterminated string".to_string()),
                line,
                col,
                end_line: self.line,
                end_col: self.col,
            }
        } else {
            self.advance();
            Token {
                kind: TokenKind::String(value),
                line,
                col,
                end_line: self.line,
                end_col: self.col,
            }
        }
    }

    fn number_token(&mut self, first: char, line: usize, col: usize) -> Token {
        let mut text = String::new();
        text.push(first);
        while self.peek().is_ascii_digit() {
            text.push(self.advance());
        }
        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            text.push(self.advance());
            while self.peek().is_ascii_digit() {
                text.push(self.advance());
            }
        }
        let kind = match text.parse::<f64>() {
            Ok(number) if number.is_finite() => TokenKind::Number(number),
            _ => {
                TokenKind::Invalid("number literal is outside the finite numeric range".to_string())
            }
        };
        Token {
            kind,
            line,
            col,
            end_line: self.line,
            end_col: self.col,
        }
    }

    fn identifier_token(&mut self, first: char, line: usize, col: usize) -> Token {
        let mut text = String::new();
        text.push(first);
        while is_alpha_numeric(self.peek()) {
            text.push(self.advance());
        }
        let kind = match text.as_str() {
            "fn" | "fun" => TokenKind::Fn,
            "async" | "work" => TokenKind::Async,
            "await" => TokenKind::Await,
            "let" | "var" => TokenKind::Let,
            "const" => TokenKind::Const,
            "if" | "when" => TokenKind::If,
            "else" | "otherwise" => TokenKind::Else,
            "while" | "loop" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "return" | "give" => TokenKind::Return,
            "true" | "yes" => TokenKind::True,
            "false" | "no" => TokenKind::False,
            "null" | "none" => TokenKind::Null,
            "print" | "say" => TokenKind::Print,
            "use" | "import" => TokenKind::Use,
            "and" | "all" => TokenKind::And,
            "or" | "any" => TokenKind::Or,
            _ => TokenKind::Identifier(text),
        };
        Token {
            kind,
            line,
            col,
            end_line: self.line,
            end_col: self.col,
        }
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.chars.len()
    }
    fn advance(&mut self) -> char {
        let ch = self.chars[self.current];
        self.current += 1;
        self.col += 1;
        ch
    }
    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.chars[self.current] != expected {
            return false;
        }
        self.current += 1;
        self.col += 1;
        true
    }
    fn peek(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.chars[self.current]
        }
    }
    fn peek_next(&self) -> char {
        if self.current + 1 >= self.chars.len() {
            '\0'
        } else {
            self.chars[self.current + 1]
        }
    }
}

pub(super) fn is_alpha(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}
pub(super) fn is_alpha_numeric(c: char) -> bool {
    is_alpha(c) || c.is_ascii_digit()
}
