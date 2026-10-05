//! Bounded fixed-scalar-array grammar over the original lossless token tape.
use super::*;
use crate::frontend::source::Span;

impl Parser<'_> {
    pub(super) fn arrays_enabled(&self) -> bool {
        self.mode.owned() && self.arrays.enabled()
    }

    /// Refine only newly admitted array targets. Existing scalar/record invalid
    /// assignments still fail at the required semicolon with their old origin.
    pub(super) fn array_assignment_expression(&self, mut target: ExprId) -> bool {
        while let ExprKind::Group(inner) = self.expressions[target.0].kind {
            target = inner;
        }
        matches!(
            self.expressions[target.0].kind,
            ExprKind::ArrayLiteral { .. }
                | ExprKind::IndexRead { .. }
                | ExprKind::ArrayLength { .. }
        )
    }

    pub(super) fn array_punctuation(&self, spelling: &str) -> bool {
        self.peek().kind == Kind::Unsupported && self.source.text_at(self.peek().span) == spelling
    }

    pub(super) fn array_unsupported(&self, span: Span) -> Box<Diagnostic> {
        self.diagnostic(
            "E0101",
            "parse",
            format_args!(
                "unsupported typed-preview construct `{}`",
                self.source.text_at(span)
            ),
            Some(span),
        )
    }

    pub(super) fn array_missing(&self, message: &'static str) -> Box<Diagnostic> {
        self.diagnostic("E0100", "parse", message, Some(self.peek().span))
    }

    fn array_reserve_error(&self, error: ReserveFailure, at: Span) -> Box<Diagnostic> {
        self.diagnostic(
            "E0400",
            "parse",
            match error {
                ReserveFailure::Overflow => "array syntax count overflow",
                ReserveFailure::Allocation => "array syntax allocation failed",
            },
            Some(at),
        )
    }

    pub(super) fn array_close(&mut self, message: &'static str) -> Result<usize, Box<Diagnostic>> {
        if self.array_punctuation("]") {
            Ok(self.bump().span.end)
        } else {
            Err(self.array_missing(message))
        }
    }

    pub(super) fn array_type(&mut self) -> Result<(FixedArraySyntax, usize), Box<Diagnostic>> {
        let element = self.array_element_type()?;
        self.fixed_array_type_tail(element)
    }

    pub(super) fn array_parameter_type(
        &mut self,
        mutable: bool,
    ) -> Result<(TypeSyntaxKind, usize), Box<Diagnostic>> {
        let element = self.array_element_type()?;
        if self.array_punctuation("]") {
            let end = self.bump().span.end;
            Ok((TypeSyntaxKind::SliceReference { mutable, element }, end))
        } else {
            let (array, end) = self.fixed_array_type_tail(element)?;
            Ok((TypeSyntaxKind::ArrayReference { mutable, array }, end))
        }
    }

    fn array_element_type(&mut self) -> Result<ScalarTypeSyntax, Box<Diagnostic>> {
        self.bump(); // The caller checked the opening bracket.
        let token = self.peek();
        let element = match token.kind {
            Kind::Ident if self.source.text_at(token.span) == "bool" => {
                self.bump();
                ScalarTypeSyntax::Bool
            }
            Kind::Ident if self.source.text_at(token.span) == "i32" => {
                self.bump();
                ScalarTypeSyntax::I32
            }
            Kind::LParen if self.next_kind() == Kind::RParen => {
                self.bump();
                self.bump();
                ScalarTypeSyntax::Unit
            }
            _ => return Err(self.array_unsupported(token.span)),
        };
        Ok(element)
    }

    fn fixed_array_type_tail(
        &mut self,
        element: ScalarTypeSyntax,
    ) -> Result<(FixedArraySyntax, usize), Box<Diagnostic>> {
        if self.take(Kind::Semi).is_none() {
            return Err(self.array_missing("array type requires `;` after its element type"));
        }
        let length_token = self.peek();
        let spelling = self.source.text_at(length_token.span);
        if length_token.kind != Kind::Number || !spelling.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(self.array_unsupported(length_token.span));
        }
        // Spellings, including all-zero 65,536-byte tokens, never overflow.
        // Validate spelling first so suffixes remain excluded grammar.
        let mut length = 0usize;
        for byte in spelling.bytes() {
            length = length
                .checked_mul(10)
                .and_then(|n| n.checked_add(usize::from(byte - b'0')))
                .filter(|&n| n <= MAX_ARRAY_ELEMENTS)
                .ok_or_else(|| {
                    self.diagnostic(
                        "E0400",
                        "parse",
                        "array length limit exceeded (1024)",
                        Some(length_token.span),
                    )
                })?;
        }
        self.bump();
        if matches!(
            self.peek().kind,
            Kind::Plus | Kind::Minus | Kind::Star | Kind::Slash | Kind::Percent
        ) {
            return Err(self.array_unsupported(self.peek().span));
        }
        let end = self.array_close("array type requires `]`")?;
        Ok((
            FixedArraySyntax {
                element,
                length: length as u16,
            },
            end,
        ))
    }

    pub(super) fn array_literal(
        &mut self,
        depth: usize,
    ) -> Result<(Vec<ExprId>, usize), Box<Diagnostic>> {
        self.bump();
        let mut elements = Vec::new();
        while !self.array_punctuation("]") {
            // A missing close/nonstarter is not an actual excess element.
            // Use the same primary dispatch and prefix grammar as expressions.
            if !self.expression_starter() {
                return Err(if self.unsupported_token() {
                    self.array_unsupported(self.peek().span)
                } else {
                    self.array_missing("expected a bool, i32 or unit expression")
                });
            }
            let at = self.peek().span;
            let next = elements
                .len()
                .checked_add(1)
                .ok_or_else(|| self.array_reserve_error(ReserveFailure::Overflow, at))?;
            if next > MAX_ARRAY_ELEMENTS {
                return Err(self.diagnostic(
                    "E0400",
                    "parse",
                    "array element limit exceeded (1024)",
                    Some(at),
                ));
            }
            if self.enums_enabled() {
                // The candidate's child node/depth gate precedes retained growth.
                let element = self.expression(depth + 1, LiteralContext::Allowed)?;
                self.candidate_reserve(
                    &mut elements,
                    MAX_ARRAY_ELEMENTS,
                    "array literal elements",
                    at,
                )?;
                elements.push(element);
            } else {
                // Preserve the existing array reservation order and observer ordinals.
                self.allocator
                    .vector(&mut elements, 1, "array literal elements")
                    .map_err(|error| self.array_reserve_error(error, at))?;
                elements.push(self.expression(depth + 1, LiteralContext::Allowed)?);
            }
            if self.array_punctuation("]") {
                break;
            }
            if self.peek().kind == Kind::Semi {
                return Err(self.array_unsupported(self.peek().span));
            }
            if self.take(Kind::Comma).is_none() {
                return Err(self.array_missing("array literal requires `,` or `]`"));
            }
        }
        Ok((elements, self.array_close("array literal requires `]`")?))
    }

    pub(super) fn array_length_close(&mut self) -> Result<usize, Box<Diagnostic>> {
        if let Some(close) = self.take(Kind::RParen) {
            return Ok(close.span.end);
        }
        if self.expression_starter() {
            return Err(self.array_unsupported(self.peek().span));
        }
        Err(self.array_missing("call requires `)`"))
    }
}
