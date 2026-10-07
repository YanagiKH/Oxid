//! Complete private source/OPA comparison. This only borrows compiler-owned
//! syntax and observations; it cannot construct a compiler authority.
use super::{ast, BoundObservation, Boundary, Wire, MAX_ROWS};
use crate::frontend::{lexer::Kind, project::ModuleId, source::Span};

const NONE: u8 = u8::MAX;
// Charge each row its entry event plus every event it emits. Entries emitted by
// a parent are charged to the child instead, so they are not counted twice.
// The maxima are Function 13, Parameter 6, Type 4, Block 5, Statement 9, and
// Expression 9 (including its argument-list continuation/comma). Empty list
// sentinels are charged to their containing row. Sixteen events per row is
// therefore conservative; one final Function/EOF pair is covered by the slack
// of each nonempty function, or by the two-event empty-program special case.
// No event recursively calls the walker.
const MAX_EVENTS: usize = 16 * MAX_ROWS + 1;
// Event visits, row reads (at most three per row), token slots and source bytes.
pub(super) const MAX_WORK: usize = MAX_EVENTS + 5 * MAX_ROWS + 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Row {
    pub(super) kind: u8,
    pub(super) start: u8,
    pub(super) end: u8,
    pub(super) next: u8,
    pub(super) a: u8,
    pub(super) b: u8,
    pub(super) c: u8,
    pub(super) d: u8,
}
impl Row {
    fn read(wire: Wire<'_>, reference: u8) -> Result<Self, Boundary> {
        if reference == 0 || reference > wire.rows {
            return Err(Boundary::Frame);
        }
        let cell = usize::from(reference - 1);
        let header = u32::try_from(wire.word(0, cell)?).map_err(|_| Boundary::Frame)?;
        let ab = u32::try_from(wire.word(1, cell)?).map_err(|_| Boundary::Frame)?;
        let cd = u32::try_from(wire.word(2, cell)?).map_err(|_| Boundary::Frame)?;
        let kind = (header & 63) as u8;
        let start = ((header >> 6) & 255) as u8;
        let end = ((header >> 14) & 255) as u8;
        let next = header >> 22;
        let values = [ab & 255, ab >> 8, cd & 255, cd >> 8];
        if !(1..=36).contains(&kind)
            || start > end
            || usize::from(end) > MAX_ROWS
            || next > u32::from(wire.rows)
            || values.iter().any(|&value| value > MAX_ROWS as u32)
        {
            return Err(Boundary::Frame);
        }
        Ok(Self {
            kind,
            start,
            end,
            next: next as u8,
            a: values[0] as u8,
            b: values[1] as u8,
            c: values[2] as u8,
            d: values[3] as u8,
        })
    }
}

pub(super) struct ComparedSyntax<'b, 's, 'w> {
    pub(super) bound: &'b BoundObservation<'s, 'w>,
    visits: usize,
    expression_rows: [u8; MAX_ROWS],
    function_rows: [u8; MAX_ROWS],
    block_functions: [u8; MAX_ROWS],
    block_ids: [u8; MAX_ROWS],
}
impl ComparedSyntax<'_, '_, '_> {
    pub(super) fn visits(&self) -> usize {
        self.visits
    }
    pub(super) fn row(&self, reference: u8) -> Result<Row, Boundary> {
        Row::read(self.bound.wire, reference)
    }
    pub(super) fn expr_row(&self, id: ast::ExprId) -> Result<u8, Boundary> {
        self.expression_rows
            .get(id.0)
            .copied()
            .filter(|&r| r != 0)
            .ok_or(Boundary::Frame)
    }
    pub(super) fn function_row(&self, index: usize) -> Result<u8, Boundary> {
        self.function_rows
            .get(index)
            .copied()
            .filter(|&r| r != 0)
            .ok_or(Boundary::Frame)
    }
    pub(super) fn block_location(
        &self,
        reference: u8,
    ) -> Result<(usize, ast::BodyBlockId), Boundary> {
        let at = usize::from(reference.checked_sub(1).ok_or(Boundary::Frame)?);
        let function = *self.block_functions.get(at).ok_or(Boundary::Frame)?;
        let block = *self.block_ids.get(at).ok_or(Boundary::Frame)?;
        if function == NONE || block == NONE {
            return Err(Boundary::Frame);
        }
        Ok((usize::from(function), ast::BodyBlockId(usize::from(block))))
    }
}

#[derive(Clone, Copy)]
enum Event {
    Empty,
    Token {
        kind: Kind,
        reference: u8,
        start: u8,
        end: u8,
    },
    Allocate(u8),
    Function {
        index: u8,
        row: u8,
    },
    FunctionEnd(u8),
    Params {
        function: u8,
        index: u8,
        row: u8,
    },
    Type {
        function: u8,
        block: u8,
        slot: u8,
        row: u8,
    },
    Block {
        function: u8,
        id: u8,
        row: u8,
        depth: u8,
    },
    Statements {
        function: u8,
        block: u8,
        index: u8,
        row: u8,
        depth: u8,
    },
    Expression {
        id: u8,
        row: u8,
        linked: bool,
        floor: u8,
        depth: u8,
    },
    ExpressionEnd {
        id: u8,
        row: u8,
    },
    Arguments {
        expression: u8,
        index: u8,
        row: u8,
        depth: u8,
    },
}
struct Scratch {
    events: [Event; MAX_EVENTS],
    pending: usize,
    scheduled: usize,
    claimed: [bool; MAX_ROWS],
    heights: [u8; MAX_ROWS],
    cursor: usize,
    next_row: usize,
    next_expression: usize,
    next_block: usize,
}
impl Scratch {
    fn new() -> Self {
        Self {
            events: [Event::Empty; MAX_EVENTS],
            pending: 0,
            scheduled: 0,
            claimed: [false; MAX_ROWS],
            heights: [0; MAX_ROWS],
            cursor: 0,
            next_row: 1,
            next_expression: 0,
            next_block: 0,
        }
    }
    fn push(&mut self, event: Event) -> Result<(), Boundary> {
        if self.scheduled == MAX_EVENTS {
            return Err(Boundary::Frame);
        }
        *self.events.get_mut(self.pending).ok_or(Boundary::Frame)? = event;
        self.pending += 1;
        self.scheduled += 1;
        Ok(())
    }
    fn pop(&mut self) -> Option<Event> {
        self.pending = self.pending.checked_sub(1)?;
        Some(std::mem::replace(
            &mut self.events[self.pending],
            Event::Empty,
        ))
    }
}
// Includes the actual explicit traversal stack, ownership bits and heights.
pub(super) const SCRATCH_BYTES: usize = std::mem::size_of::<Scratch>();

struct Walker<'b, 's, 'w> {
    syntax: ComparedSyntax<'b, 's, 'w>,
    program: &'s ast::Program,
    file: crate::frontend::source::SourceFileId,
    scratch: Scratch,
}
fn require(condition: bool) -> Result<(), Boundary> {
    if condition {
        Ok(())
    } else {
        Err(Boundary::Frame)
    }
}
fn small(value: usize) -> Result<u8, Boundary> {
    u8::try_from(value)
        .ok()
        .filter(|&v| usize::from(v) < MAX_ROWS)
        .ok_or(Boundary::Frame)
}
impl Walker<'_, '_, '_> {
    fn push(&mut self, event: Event) -> Result<(), Boundary> {
        self.scratch.push(event)
    }
    fn token(&mut self, kind: Kind) -> Result<(), Boundary> {
        self.push(Event::Token {
            kind,
            reference: 0,
            start: NONE,
            end: NONE,
        })
    }
    fn token_span(&mut self, kind: Kind, reference: u8, span: Span) -> Result<(), Boundary> {
        self.span(span)?;
        self.push(Event::Token {
            kind,
            reference,
            start: span.start as u8,
            end: span.end as u8,
        })
    }
    fn edge_token(&mut self, kind: Kind, start: u8, end: u8) -> Result<(), Boundary> {
        self.push(Event::Token {
            kind,
            reference: 0,
            start,
            end,
        })
    }
    fn span(&self, span: Span) -> Result<(), Boundary> {
        require(
            span.file == self.file
                && span.start <= span.end
                && span.end <= self.syntax.bound.source.len(),
        )
    }
    fn row_span(&self, row: Row, span: Span) -> Result<(), Boundary> {
        self.span(span)?;
        require(usize::from(row.start) == span.start && usize::from(row.end) == span.end)
    }
    fn claim(&mut self, reference: u8, kind: u8, linked: bool) -> Result<Row, Boundary> {
        self.syntax.visits += 1;
        let row = self.syntax.row(reference)?;
        require(
            row.kind == kind
                && (linked || row.next == 0)
                && usize::from(row.end) <= self.syntax.bound.source.len(),
        )?;
        let mark = &mut self.scratch.claimed[usize::from(reference - 1)];
        require(!*mark)?;
        *mark = true;
        Ok(row)
    }
    fn expression_event(
        &mut self,
        id: ast::ExprId,
        row: u8,
        linked: bool,
        floor: u8,
        depth: u8,
    ) -> Result<(), Boundary> {
        self.push(Event::Expression {
            id: small(id.0)?,
            row,
            linked,
            floor,
            depth,
        })
    }
    fn expression(&self, id: ast::ExprId) -> Result<&ast::Expr, Boundary> {
        self.program.expressions.get(id.0).ok_or(Boundary::Frame)
    }
    fn function(&self, id: u8) -> Result<&ast::Function, Boundary> {
        self.program
            .functions
            .get(usize::from(id))
            .ok_or(Boundary::Frame)
    }
    fn run(&mut self) -> Result<(), Boundary> {
        self.push(Event::Function {
            index: 0,
            row: self.syntax.bound.wire.bytes[9],
        })?;
        while let Some(event) = self.scratch.pop() {
            self.syntax.visits += 1;
            match event {
                Event::Empty => return Err(Boundary::Frame),
                Event::Token {
                    kind,
                    reference,
                    start,
                    end,
                } => {
                    while self
                        .program
                        .tokens
                        .get(self.scratch.cursor)
                        .is_some_and(|t| t.kind == Kind::Trivia)
                    {
                        self.scratch.cursor += 1;
                    }
                    let token = self
                        .program
                        .tokens
                        .get(self.scratch.cursor)
                        .ok_or(Boundary::Frame)?;
                    require(
                        token.kind == kind
                            && (reference == 0
                                || usize::from(reference) == self.scratch.cursor + 1)
                            && (start == NONE || usize::from(start) == token.span.start)
                            && (end == NONE || usize::from(end) == token.span.end),
                    )?;
                    self.scratch.cursor += 1;
                }
                Event::Allocate(row) => {
                    require(usize::from(row) == self.scratch.next_row)?;
                    self.scratch.next_row += 1;
                }
                Event::Function { index, row } => self.function_start(index, row)?,
                Event::FunctionEnd(index) => {
                    require(self.scratch.next_block == self.function(index)?.blocks.len())?;
                }
                Event::Params {
                    function,
                    index,
                    row,
                } => self.parameter(function, index, row)?,
                Event::Type {
                    function,
                    block,
                    slot,
                    row,
                } => self.ty(function, block, slot, row)?,
                Event::Block {
                    function,
                    id,
                    row,
                    depth,
                } => self.block(function, id, row, depth)?,
                Event::Statements {
                    function,
                    block,
                    index,
                    row,
                    depth,
                } => self.statement(function, block, index, row, depth)?,
                Event::Expression {
                    id,
                    row,
                    linked,
                    floor,
                    depth,
                } => self.expr(id, row, linked, floor, depth)?,
                Event::ExpressionEnd { id, row } => self.expression_end(id, row)?,
                Event::Arguments {
                    expression,
                    index,
                    row,
                    depth,
                } => self.argument(expression, index, row, depth)?,
            }
        }
        require(
            self.scratch.cursor == self.program.tokens.len()
                && self.scratch.next_row == usize::from(self.syntax.bound.wire.rows) + 1
                && self.scratch.next_expression == self.program.expressions.len()
                && self.scratch.claimed[..usize::from(self.syntax.bound.wire.rows)]
                    .iter()
                    .all(|&v| v),
        )
    }
    fn function_start(&mut self, index: u8, reference: u8) -> Result<(), Boundary> {
        if usize::from(index) == self.program.functions.len() {
            require(reference == 0)?;
            return self.token(Kind::Eof);
        }
        let function = self
            .program
            .functions
            .get(usize::from(index))
            .ok_or(Boundary::Frame)?;
        require(
            matches!(self.program.items.get(usize::from(index)), Some(ast::ItemId::Function(id)) if *id == usize::from(index)),
        )?;
        let row = self.claim(reference, 1, true)?;
        self.row_span(row, function.name)?;
        self.syntax.function_rows[usize::from(index)] = reference;
        self.scratch.next_block = 0;
        let body = function
            .blocks
            .get(function.body.0)
            .ok_or(Boundary::Frame)?;
        require(function.end == body.end)?;
        self.push(Event::Function {
            index: index + 1,
            row: row.next,
        })?;
        self.push(Event::FunctionEnd(index))?;
        self.push(Event::Block {
            function: index,
            id: small(function.body.0)?,
            row: row.d,
            depth: 1,
        })?;
        self.push(Event::Type {
            function: index,
            block: NONE,
            slot: NONE,
            row: row.c,
        })?;
        self.token(Kind::Arrow)?;
        self.token(Kind::RParen)?;
        self.push(Event::Params {
            function: index,
            index: 0,
            row: row.b,
        })?;
        self.token(Kind::LParen)?;
        self.token_span(Kind::Ident, 0, function.name)?;
        self.token(Kind::Fn)?;
        if let Some(public) = function.public {
            require(row.a != 0)?;
            self.token_span(Kind::Pub, row.a, public)?;
        } else {
            require(row.a == 0)?;
        }
        self.push(Event::Allocate(reference))
    }
    fn parameter(&mut self, function: u8, index: u8, reference: u8) -> Result<(), Boundary> {
        let params = &self
            .program
            .functions
            .get(usize::from(function))
            .ok_or(Boundary::Frame)?
            .params;
        if usize::from(index) == params.len() {
            return require(reference == 0);
        }
        let param = params.get(usize::from(index)).ok_or(Boundary::Frame)?;
        let row = self.claim(reference, 2, true)?;
        self.row_span(row, param.name)?;
        require(row.b == 0 && row.c == 0 && row.d == 0)?;
        self.push(Event::Params {
            function,
            index: index + 1,
            row: row.next,
        })?;
        self.push(Event::Type {
            function,
            block: NONE,
            slot: index,
            row: row.a,
        })?;
        self.token(Kind::Colon)?;
        self.token_span(Kind::Ident, 0, param.name)?;
        self.push(Event::Allocate(reference))?;
        if index != 0 {
            self.token(Kind::Comma)?;
        }
        Ok(())
    }
    fn ty(&mut self, function: u8, block: u8, slot: u8, reference: u8) -> Result<(), Boundary> {
        let function = self.function(function)?;
        let ty = if block == NONE {
            if slot == NONE {
                function.result
            } else {
                function
                    .params
                    .get(usize::from(slot))
                    .ok_or(Boundary::Frame)?
                    .ty
            }
        } else {
            let statement = function
                .blocks
                .get(usize::from(block))
                .and_then(|b| b.body.get(usize::from(slot)))
                .ok_or(Boundary::Frame)?;
            let ast::StmtKind::Let {
                annotation: Some(ty),
                ..
            } = statement.kind
            else {
                return Err(Boundary::Frame);
            };
            ty
        };
        let kind = match ty.kind {
            ast::TypeSyntaxKind::Name(ast::ItemPath::Unqualified(_)) => 3,
            ast::TypeSyntaxKind::Unit => 4,
            _ => return Err(Boundary::Frame),
        };
        let row = self.claim(reference, kind, false)?;
        self.row_span(row, ty.span)?;
        require(row.b == 0 && row.c == 0 && row.d == 0)?;
        if let ast::TypeSyntaxKind::Name(ast::ItemPath::Unqualified(name)) = ty.kind {
            require(name == ty.span && row.a != 0)?;
            self.token_span(Kind::Ident, row.a, name)?;
        } else {
            require(row.a == 0)?;
            self.edge_token(Kind::RParen, NONE, row.end)?;
            self.edge_token(Kind::LParen, row.start, NONE)?;
        }
        self.push(Event::Allocate(reference))
    }
    fn block(&mut self, function: u8, id: u8, reference: u8, depth: u8) -> Result<(), Boundary> {
        require((1..=64).contains(&depth) && usize::from(id) == self.scratch.next_block)?;
        let block = self
            .program
            .functions
            .get(usize::from(function))
            .and_then(|f| f.blocks.get(usize::from(id)))
            .ok_or(Boundary::Frame)?;
        let row = self.claim(reference, 5, false)?;
        self.row_span(row, block.span)?;
        require(row.c == 0 && row.d == 0 && row.a != 0 && block.end.end == block.span.end)?;
        self.scratch.next_block += 1;
        self.syntax.block_functions[usize::from(reference - 1)] = function;
        self.syntax.block_ids[usize::from(reference - 1)] = id;
        self.token_span(Kind::RBrace, row.a, block.end)?;
        self.push(Event::Statements {
            function,
            block: id,
            index: 0,
            row: row.b,
            depth,
        })?;
        self.edge_token(Kind::LBrace, row.start, NONE)?;
        self.push(Event::Allocate(reference))
    }
    fn statement(
        &mut self,
        function: u8,
        block: u8,
        index: u8,
        reference: u8,
        depth: u8,
    ) -> Result<(), Boundary> {
        let body = &self
            .program
            .functions
            .get(usize::from(function))
            .and_then(|f| f.blocks.get(usize::from(block)))
            .ok_or(Boundary::Frame)?
            .body;
        if usize::from(index) == body.len() {
            return require(reference == 0);
        }
        let statement = body.get(usize::from(index)).ok_or(Boundary::Frame)?;
        let tag = match statement.kind {
            ast::StmtKind::Let { mutable, .. } => {
                if mutable {
                    7
                } else {
                    6
                }
            }
            ast::StmtKind::Assign { .. } => 8,
            ast::StmtKind::Expr(_) => 9,
            ast::StmtKind::Return(_) => 10,
            ast::StmtKind::Break => 11,
            ast::StmtKind::Continue => 12,
            ast::StmtKind::If { .. } => 13,
            ast::StmtKind::While { .. } => 14,
            _ => return Err(Boundary::Frame),
        };
        let row = self.claim(reference, tag, true)?;
        self.row_span(row, statement.span)?;
        require(row.d == 0)?;
        self.push(Event::Statements {
            function,
            block,
            index: index + 1,
            row: row.next,
            depth,
        })?;
        if tag == 9 {
            self.push(Event::Allocate(reference))?;
        }
        if tag < 13 {
            self.edge_token(Kind::Semi, NONE, row.end)?;
        }
        match statement.kind {
            ast::StmtKind::Let {
                mutable,
                name,
                annotation,
                init,
            } => {
                self.expression_event(init, row.c, false, 0, 1)?;
                self.token(Kind::Equal)?;
                if annotation.is_some() {
                    self.push(Event::Type {
                        function,
                        block,
                        slot: index,
                        row: row.b,
                    })?;
                    self.token(Kind::Colon)?;
                } else {
                    require(row.b == 0)?;
                }
                require(row.a != 0)?;
                self.token_span(Kind::Ident, row.a, name)?;
                if mutable {
                    self.token(Kind::Mut)?;
                }
                self.edge_token(Kind::Let, row.start, NONE)?;
            }
            ast::StmtKind::Assign {
                name,
                operator_span,
                value,
            } => {
                require(name.start == statement.span.start && row.a != 0 && row.b != 0)?;
                self.expression_event(value, row.c, false, 0, 1)?;
                self.token_span(Kind::Equal, row.b, operator_span)?;
                self.token_span(Kind::Ident, row.a, name)?;
            }
            ast::StmtKind::Expr(value) => {
                require(
                    row.b == 0
                        && row.c == 0
                        && self.expression(value)?.span.start == statement.span.start,
                )?;
                self.expression_event(value, row.a, false, 0, 1)?;
            }
            ast::StmtKind::Return(value) => {
                require(row.b == 0 && row.c == 0)?;
                if let Some(value) = value {
                    self.expression_event(value, row.a, false, 0, 1)?;
                } else {
                    require(row.a == 0)?;
                }
                self.edge_token(Kind::Return, row.start, NONE)?;
            }
            ast::StmtKind::Break | ast::StmtKind::Continue => {
                require(row.a == 0 && row.b == 0 && row.c == 0)?;
                self.edge_token(
                    if tag == 11 {
                        Kind::Break
                    } else {
                        Kind::Continue
                    },
                    row.start,
                    NONE,
                )?;
            }
            ast::StmtKind::If {
                condition,
                then_block,
                else_block,
            } => {
                let last = else_block.unwrap_or(then_block);
                let end = self
                    .function(function)?
                    .blocks
                    .get(last.0)
                    .ok_or(Boundary::Frame)?
                    .end;
                require(end.end == statement.span.end)?;
                if let Some(otherwise) = else_block {
                    self.push(Event::Block {
                        function,
                        id: small(otherwise.0)?,
                        row: row.c,
                        depth: depth + 1,
                    })?;
                    self.token(Kind::Else)?;
                } else {
                    require(row.c == 0)?;
                }
                self.push(Event::Block {
                    function,
                    id: small(then_block.0)?,
                    row: row.b,
                    depth: depth + 1,
                })?;
                self.expression_event(condition, row.a, false, 0, 1)?;
                self.edge_token(Kind::If, row.start, NONE)?;
            }
            ast::StmtKind::While { condition, body } => {
                require(
                    row.c == 0
                        && self
                            .function(function)?
                            .blocks
                            .get(body.0)
                            .ok_or(Boundary::Frame)?
                            .end
                            .end
                            == statement.span.end,
                )?;
                self.push(Event::Block {
                    function,
                    id: small(body.0)?,
                    row: row.b,
                    depth: depth + 1,
                })?;
                self.expression_event(condition, row.a, false, 0, 1)?;
                self.edge_token(Kind::While, row.start, NONE)?;
            }
            _ => return Err(Boundary::Frame),
        }
        if tag != 9 {
            self.push(Event::Allocate(reference))?;
        }
        Ok(())
    }
    fn expr(
        &mut self,
        id: u8,
        reference: u8,
        linked: bool,
        floor: u8,
        depth: u8,
    ) -> Result<(), Boundary> {
        require((1..=64).contains(&depth))?;
        let expression = self
            .program
            .expressions
            .get(usize::from(id))
            .ok_or(Boundary::Frame)?;
        let (tag, binary) = expression_tag(&expression.kind)?;
        let row = self.claim(reference, tag, linked)?;
        self.row_span(row, expression.span)?;
        self.push(Event::ExpressionEnd { id, row: reference })?;
        if let Some((operator, precedence, comparison)) = binary {
            require(precedence >= floor && row.c != 0)?;
            let (left, right, operator_span) = match expression.kind {
                ast::ExprKind::Arithmetic {
                    left,
                    right,
                    operator_span,
                    ..
                }
                | ast::ExprKind::Comparison {
                    left,
                    right,
                    operator_span,
                    ..
                }
                | ast::ExprKind::Logical {
                    left,
                    right,
                    operator_span,
                    ..
                } => (left, right, operator_span),
                _ => return Err(Boundary::Frame),
            };
            require(
                expression.span.start == self.expression(left)?.span.start
                    && expression.span.end == self.expression(right)?.span.end,
            )?;
            self.expression_event(right, row.b, false, precedence + 1, depth + 1)?;
            self.push(Event::Allocate(reference))?;
            self.token_span(operator, row.c, operator_span)?;
            return self.expression_event(
                left,
                row.a,
                false,
                precedence + u8::from(comparison),
                depth + 1,
            );
        }
        require(row.c == 0)?;
        match &expression.kind {
            ast::ExprKind::Number { digits, negative } => {
                require(
                    row.a != 0 && row.b == u8::from(*negative) && digits.end == expression.span.end,
                )?;
                self.span(*digits)?;
                require(
                    !self.syntax.bound.source[digits.start..digits.end].is_empty()
                        && self.syntax.bound.source[digits.start..digits.end]
                            .iter()
                            .all(u8::is_ascii_digit),
                )?;
                self.token_span(Kind::Number, row.a, *digits)?;
                if *negative {
                    self.edge_token(Kind::Minus, row.start, NONE)?;
                } else {
                    require(*digits == expression.span)?;
                }
            }
            ast::ExprKind::Bool(value) => {
                require(row.a == 0 && row.b == 0)?;
                self.token_span(
                    if *value { Kind::True } else { Kind::False },
                    0,
                    expression.span,
                )?;
            }
            ast::ExprKind::Unit => {
                require(row.a == 0 && row.b == 0)?;
                self.edge_token(Kind::RParen, NONE, row.end)?;
                self.edge_token(Kind::LParen, row.start, NONE)?;
            }
            ast::ExprKind::Name(name) => {
                require(row.a != 0 && row.b == 0 && *name == expression.span)?;
                self.token_span(Kind::Ident, row.a, *name)?;
            }
            ast::ExprKind::Call {
                callee: ast::ItemPath::Unqualified(name),
                ..
            } => {
                require(row.a != 0 && name.start == expression.span.start)?;
                self.edge_token(Kind::RParen, NONE, row.end)?;
                self.push(Event::Arguments {
                    expression: id,
                    index: 0,
                    row: row.b,
                    depth: depth + 1,
                })?;
                self.token(Kind::LParen)?;
                self.token_span(Kind::Ident, row.a, *name)?;
            }
            ast::ExprKind::Group(inner) => {
                require(row.b == 0)?;
                self.edge_token(Kind::RParen, NONE, row.end)?;
                self.expression_event(*inner, row.a, false, 0, depth + 1)?;
                self.edge_token(Kind::LParen, row.start, NONE)?;
            }
            ast::ExprKind::Negate {
                operand,
                operator_span,
            }
            | ast::ExprKind::Not {
                operand,
                operator_span,
            } => {
                require(
                    row.b != 0
                        && operator_span.start == expression.span.start
                        && self.expression(*operand)?.span.end == expression.span.end,
                )?;
                if tag == 22 {
                    require(!matches!(
                        self.expression(*operand)?.kind,
                        ast::ExprKind::Number {
                            negative: false,
                            ..
                        }
                    ))?;
                }
                self.expression_event(*operand, row.a, false, 6, depth + 1)?;
                self.token_span(
                    if tag == 22 { Kind::Minus } else { Kind::Not },
                    row.b,
                    *operator_span,
                )?;
            }
            _ => return Err(Boundary::Frame),
        }
        self.push(Event::Allocate(reference))
    }
    fn argument(
        &mut self,
        expression: u8,
        index: u8,
        reference: u8,
        depth: u8,
    ) -> Result<(), Boundary> {
        let ast::ExprKind::Call { args, .. } = &self
            .program
            .expressions
            .get(usize::from(expression))
            .ok_or(Boundary::Frame)?
            .kind
        else {
            return Err(Boundary::Frame);
        };
        if usize::from(index) == args.len() {
            return require(reference == 0);
        }
        let ast::Argument::Value(value) = args.get(usize::from(index)).ok_or(Boundary::Frame)?
        else {
            return Err(Boundary::Frame);
        };
        self.syntax.visits += 1;
        let next = self.syntax.row(reference)?.next;
        self.push(Event::Arguments {
            expression,
            index: index + 1,
            row: next,
            depth,
        })?;
        self.expression_event(*value, reference, true, 0, depth)?;
        if index != 0 {
            self.token(Kind::Comma)?;
        }
        Ok(())
    }
    fn expression_end(&mut self, id: u8, reference: u8) -> Result<(), Boundary> {
        require(usize::from(id) == self.scratch.next_expression)?;
        let expression = &self.program.expressions[usize::from(id)];
        let height = |id: ast::ExprId| -> Result<u8, Boundary> {
            self.scratch
                .heights
                .get(id.0)
                .copied()
                .filter(|&v| v != 0)
                .ok_or(Boundary::Frame)
        };
        let children = match &expression.kind {
            ast::ExprKind::Group(inner)
            | ast::ExprKind::Negate { operand: inner, .. }
            | ast::ExprKind::Not { operand: inner, .. } => height(*inner)?,
            ast::ExprKind::Arithmetic { left, right, .. }
            | ast::ExprKind::Comparison { left, right, .. }
            | ast::ExprKind::Logical { left, right, .. } => height(*left)?.max(height(*right)?),
            ast::ExprKind::Call { args, .. } => {
                let mut maximum = 0;
                for argument in args {
                    let ast::Argument::Value(value) = argument else {
                        return Err(Boundary::Frame);
                    };
                    maximum = maximum.max(height(*value)?);
                }
                maximum
            }
            _ => 0,
        };
        let height = children + 1;
        self.syntax.visits += 1;
        require(height <= 64 && self.syntax.row(reference)?.d == height)?;
        self.scratch.heights[usize::from(id)] = height;
        self.syntax.expression_rows[usize::from(id)] = reference;
        self.scratch.next_expression += 1;
        Ok(())
    }
}

type Binary = Option<(Kind, u8, bool)>;
fn expression_tag(kind: &ast::ExprKind) -> Result<(u8, Binary), Boundary> {
    use ast::{ArithmeticOp as A, ComparisonOp as C, ExprKind as E, LogicalOp as L};
    let (tag, token, precedence, comparison) = match kind {
        E::Number { .. } => return Ok((15, None)),
        E::Bool(false) => return Ok((16, None)),
        E::Bool(true) => return Ok((17, None)),
        E::Unit => return Ok((18, None)),
        E::Name(_) => return Ok((19, None)),
        E::Call { .. } => return Ok((20, None)),
        E::Group(_) => return Ok((21, None)),
        E::Negate { .. } => return Ok((22, None)),
        E::Not { .. } => return Ok((23, None)),
        E::Arithmetic { op, .. } => match op {
            A::Add => (24, Kind::Plus, 4, false),
            A::Subtract => (25, Kind::Minus, 4, false),
            A::Multiply => (26, Kind::Star, 5, false),
            A::Divide => (27, Kind::Slash, 5, false),
            A::Remainder => (28, Kind::Percent, 5, false),
        },
        E::Comparison { op, .. } => match op {
            C::Equal => (29, Kind::EqualEqual, 3, true),
            C::NotEqual => (30, Kind::NotEqual, 3, true),
            C::Less => (31, Kind::Less, 3, true),
            C::LessEqual => (32, Kind::LessEqual, 3, true),
            C::Greater => (33, Kind::Greater, 3, true),
            C::GreaterEqual => (34, Kind::GreaterEqual, 3, true),
        },
        E::Logical { op: L::And, .. } => (35, Kind::AndAnd, 2, false),
        E::Logical { op: L::Or, .. } => (36, Kind::OrOr, 1, false),
        _ => return Err(Boundary::Frame),
    };
    Ok((tag, Some((token, precedence, comparison))))
}

// Validate the retained tape in place. This recognizes lexical extents only;
// syntax, row ownership and all parser allocation events are compared above.
fn tokens(
    program: &ast::Program,
    source: &[u8],
    file: crate::frontend::source::SourceFileId,
) -> Result<(), Boundary> {
    require(!program.tokens.is_empty() && program.tokens.len() <= MAX_ROWS + 1)?;
    let mut cursor = 0;
    for (index, token) in program.tokens.iter().enumerate() {
        require(
            token.span.file == file && token.span.start == cursor && token.span.end <= source.len(),
        )?;
        if index + 1 == program.tokens.len() {
            require(token.kind == Kind::Eof && cursor == source.len() && token.span.end == cursor)?;
            continue;
        }
        let (kind, end) = lexical_extent(source, cursor)?;
        require(token.kind == kind && token.span.end == end)?;
        cursor = end;
    }
    Ok(())
}
fn lexical_extent(source: &[u8], start: usize) -> Result<(Kind, usize), Boundary> {
    let first = *source.get(start).ok_or(Boundary::Frame)?;
    let mut end = start + 1;
    let rest = &source[start..];
    let whitespace = |byte: u8| byte == b' ' || (b'\t'..=b'\r').contains(&byte);
    if whitespace(first) {
        while source.get(end).is_some_and(|&byte| whitespace(byte)) {
            end += 1;
        }
        return Ok((Kind::Trivia, end));
    }
    if rest.starts_with(b"//") {
        while source.get(end).is_some_and(|&byte| byte != b'\n') {
            end += 1;
        }
        return Ok((Kind::Trivia, end));
    }
    if rest.starts_with(b"/*") {
        let close = rest[2..]
            .windows(2)
            .position(|pair| pair == b"*/")
            .ok_or(Boundary::Frame)?;
        return Ok((Kind::Trivia, start + close + 4));
    }
    if first.is_ascii_alphabetic() || first == b'_' {
        while source
            .get(end)
            .is_some_and(|&byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            end += 1;
        }
        let kind = match &source[start..end] {
            b"fn" => Kind::Fn,
            b"pub" => Kind::Pub,
            b"let" => Kind::Let,
            b"mut" => Kind::Mut,
            b"return" => Kind::Return,
            b"break" => Kind::Break,
            b"continue" => Kind::Continue,
            b"if" => Kind::If,
            b"else" => Kind::Else,
            b"while" => Kind::While,
            b"true" => Kind::True,
            b"false" => Kind::False,
            b"struct" | b"mod" | b"use" | b"import" | b"macro" | b"macro_rules" | b"const"
            | b"for" | b"loop" | b"match" | b"async" | b"await" | b"move" | b"ref" | b"unsafe"
            | b"extern" | b"enum" | b"trait" | b"impl" | b"type" | b"null" | b"and" | b"or" => {
                return Err(Boundary::Frame)
            }
            _ => Kind::Ident,
        };
        return Ok((kind, end));
    }
    if first.is_ascii_digit() {
        while source
            .get(end)
            .is_some_and(|&byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.'))
        {
            end += 1;
        }
        return Ok((Kind::Number, end));
    }
    for (spelling, kind) in [
        (b"==", Kind::EqualEqual),
        (b"!=", Kind::NotEqual),
        (b"&&", Kind::AndAnd),
        (b"||", Kind::OrOr),
        (b"<=", Kind::LessEqual),
        (b">=", Kind::GreaterEqual),
        (b"->", Kind::Arrow),
    ] {
        if rest.starts_with(spelling) {
            return Ok((kind, start + 2));
        }
    }
    let kind = match first {
        b'(' => Kind::LParen,
        b')' => Kind::RParen,
        b'{' => Kind::LBrace,
        b'}' => Kind::RBrace,
        b':' => Kind::Colon,
        b',' => Kind::Comma,
        b';' => Kind::Semi,
        b'=' => Kind::Equal,
        b'!' => Kind::Not,
        b'<' => Kind::Less,
        b'>' => Kind::Greater,
        b'-' => Kind::Minus,
        b'+' => Kind::Plus,
        b'*' => Kind::Star,
        b'/' => Kind::Slash,
        b'%' => Kind::Percent,
        _ => return Err(Boundary::Frame),
    };
    Ok((kind, end))
}

pub(super) fn compare<'b, 's, 'w>(
    bound: &'b BoundObservation<'s, 'w>,
) -> Result<ComparedSyntax<'b, 's, 'w>, Boundary> {
    let program = bound.owner.ast(ModuleId(0)).map_err(|_| Boundary::Source)?;
    let file = bound
        .owner
        .file(ModuleId(0))
        .map_err(|_| Boundary::Source)?
        .span(0, 0)
        .file;
    require(program.items.len() == program.functions.len())?;
    tokens(program, bound.source, file)?;
    let syntax = ComparedSyntax {
        bound,
        visits: program.tokens.len() + bound.source.len(),
        expression_rows: [0; MAX_ROWS],
        function_rows: [0; MAX_ROWS],
        block_functions: [NONE; MAX_ROWS],
        block_ids: [NONE; MAX_ROWS],
    };
    let mut walker = Walker {
        syntax,
        program,
        file,
        scratch: Scratch::new(),
    };
    walker.run()?;
    Ok(walker.syntax)
}

#[cfg(test)]
mod tests;
