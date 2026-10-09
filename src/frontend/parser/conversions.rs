//! Closed named-receiver intrinsics. No method or aggregate grammar expansion.
use super::*;

impl Parser<'_> {
    /// Inspect only one dot, its identifier and the opening parenthesis. Bare
    /// field spellings still take the existing field path, including `.len`.
    /// Trivia is inspected at most twice (lookahead and ordinary consumption).
    pub(super) fn named_conversion_ahead(&self, path: ItemPath) -> Option<ConversionOp> {
        if !matches!(path, ItemPath::Unqualified(_)) || self.peek().kind != Kind::Dot {
            return None;
        }
        let mut tokens = self.tokens[self.cursor + 1..]
            .iter()
            .filter(|token| token.kind != Kind::Trivia);
        let name = tokens.next()?;
        if name.kind != Kind::Ident {
            return None;
        }
        let op = match self.source.text_at(name.span) {
            "to_u8_checked" => ConversionOp::ToU8Checked,
            "to_i32" => ConversionOp::ToI32,
            _ => return None,
        };
        (tokens.next()?.kind == Kind::LParen).then_some(op)
    }

    pub(super) fn conversion_close(&mut self) -> Result<usize, Box<Diagnostic>> {
        if let Some(close) = self.take(Kind::RParen) {
            return Ok(close.span.end);
        }
        if matches!(self.peek().kind, Kind::Eof | Kind::Semi | Kind::RBrace)
            || self.array_punctuation("]")
        {
            return Err(self.array_missing("conversion requires `)`"));
        }
        Err(self.array_unsupported(self.peek().span))
    }
}
