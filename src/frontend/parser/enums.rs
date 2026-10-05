//! Private bounded enum grammar over the unchanged lossless token tape.
use super::*;
use crate::frontend::source::Span;
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::frontend) struct SyntaxStorage {
    /// Candidate parser vector capacities, excluding pre-existing lexer tokens.
    pub retained_capacity: usize,
    pub scratch_capacity: usize,
    /// Conservative coexistence bound, including old and new grow buffers.
    pub peak_capacity_bound: usize,
    pub copied_bytes_bound: usize,
    pub growths: usize,
}

fn reserve_error(at: Span, message: &'static str) -> Box<Diagnostic> {
    super::super::owned_diagnostic::diagnostic(
        "E0400",
        "parse",
        format_args!("{message}"),
        Some(at),
    )
}

/// Candidate-only fallible growth. Closed callers retain their old reservations.
pub(super) fn reserve<T>(
    allocator: &mut Allocator,
    storage: &mut SyntaxStorage,
    values: &mut Vec<T>,
    bound: usize,
    kind: &'static str,
    scratch: bool,
    at: Span,
) -> Result<(), Box<Diagnostic>> {
    let overflow = || reserve_error(at, "syntax storage count overflow");
    let needed = values.len().checked_add(1).ok_or_else(overflow)?;
    if needed > bound || values.capacity() > bound {
        return Err(reserve_error(at, "syntax storage limit exceeded"));
    }
    needed.checked_mul(size_of::<T>()).ok_or_else(overflow)?;
    if needed <= values.capacity() {
        return Ok(());
    }
    let target = values
        .capacity()
        .checked_mul(2)
        .ok_or_else(overflow)?
        .max(4)
        .min(bound)
        .max(needed);
    let old_bytes = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or_else(overflow)?;
    let target_bytes = target.checked_mul(size_of::<T>()).ok_or_else(overflow)?;
    let live = storage
        .retained_capacity
        .checked_add(storage.scratch_capacity)
        .ok_or_else(overflow)?;
    let peak = live.checked_add(target_bytes).ok_or_else(overflow)?;
    let copied = storage
        .copied_bytes_bound
        .checked_add(old_bytes)
        .ok_or_else(overflow)?;
    let growths = storage.growths.checked_add(1).ok_or_else(overflow)?;
    let current = if scratch {
        storage.scratch_capacity
    } else {
        storage.retained_capacity
    };
    current
        .checked_sub(old_bytes)
        .and_then(|n| n.checked_add(target_bytes))
        .ok_or_else(overflow)?;
    let additional = target - values.len();
    allocator
        .vector_exact(values, additional, kind)
        .map_err(|_| reserve_error(at, "syntax storage allocation failed"))?;
    // Vec reports its usable capacity, not allocator/OS metadata. Never allow
    // unadmitted spare slots to escape into the returned candidate AST.
    let actual_bytes = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or_else(overflow)?;
    let actual = current
        .checked_sub(old_bytes)
        .and_then(|n| n.checked_add(actual_bytes))
        .ok_or_else(overflow)?;
    if scratch {
        storage.scratch_capacity = actual;
    } else {
        storage.retained_capacity = actual;
    }
    storage.peak_capacity_bound = storage
        .peak_capacity_bound
        .max(peak)
        .max(live.checked_add(actual_bytes).ok_or_else(overflow)?);
    storage.copied_bytes_bound = copied;
    storage.growths = growths;
    if values.capacity() != target {
        return Err(reserve_error(
            at,
            "syntax storage capacity exceeded admission",
        ));
    }
    Ok(())
}

impl Parser<'_> {
    pub(super) fn enums_enabled(&self) -> bool {
        self.mode.owned() && self.enums.enabled()
    }
    pub(super) fn candidate_reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        bound: usize,
        kind: &'static str,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        if self.enums_enabled() {
            reserve(
                self.allocator,
                &mut self.storage,
                values,
                bound,
                kind,
                false,
                at,
            )?;
        }
        Ok(())
    }
    pub(super) fn enum_keyword(&self, spelling: &str) -> bool {
        self.enums_enabled()
            && self.peek().kind == Kind::Unsupported
            && self.source.text_at(self.peek().span) == spelling
    }
    pub(super) fn next_enum_keyword(&self, spelling: &str) -> bool {
        self.enums_enabled()
            && self.tokens[self.cursor + 1..]
                .iter()
                .find(|token| token.kind != Kind::Trivia)
                .is_some_and(|token| {
                    token.kind == Kind::Unsupported && self.source.text_at(token.span) == spelling
                })
    }
    pub(super) fn enum_qualified_ahead(&self) -> bool {
        if !self.enums_enabled() || self.peek().kind != Kind::Ident {
            return false;
        }
        let Some(offset) = self.tokens[self.cursor + 1..]
            .iter()
            .position(|token| token.kind != Kind::Trivia)
        else {
            return false;
        };
        let index = self.cursor + 1 + offset;
        let colon = self.tokens[index];
        colon.kind == Kind::Colon
            && self
                .tokens
                .get(index + 1)
                .is_some_and(|next| next.kind == Kind::Colon && colon.span.end == next.span.start)
    }
    fn enum_error(&self, message: &'static str) -> Box<Diagnostic> {
        self.diagnostic("E0100", "parse", message, Some(self.peek().span))
    }
    pub(super) fn enumeration(
        &mut self,
        public: Option<Span>,
    ) -> Result<EnumDecl, Box<Diagnostic>> {
        self.node()?;
        let start = self.bump().span;
        let name = self.expect(Kind::Ident, "expected enum name")?.span;
        self.expect(Kind::LBrace, "expected enum variants `{`")?;
        let mut variants = Vec::new();
        if self.peek().kind == Kind::RBrace {
            return Err(self.enum_error("enum requires at least one variant"));
        }
        loop {
            if self.peek().kind != Kind::Ident {
                return Err(self.error("expected enum variant name"));
            }
            if variants.len() >= MAX_ENUM_VARIANTS {
                return Err(self.diagnostic(
                    "E0400",
                    "parse",
                    "enum variant limit exceeded (256)",
                    Some(self.peek().span),
                ));
            }
            self.node()?;
            let name = self.bump().span;
            let mut end = name.end;
            let payload = if self.take(Kind::LParen).is_some() {
                let payload = self.enum_payload_type()?;
                end = self
                    .expect(Kind::RParen, "variant payload requires `)`")?
                    .span
                    .end;
                Some(payload)
            } else {
                None
            };
            self.candidate_reserve(
                &mut variants,
                MAX_ENUM_VARIANTS,
                "enum declaration variants",
                name,
            )?;
            variants.push(EnumVariantSyntax {
                name,
                payload,
                span: self.source.span(name.start, end),
            });
            if self.take(Kind::Comma).is_none() || self.peek().kind == Kind::RBrace {
                break;
            }
        }
        let end = self
            .expect(Kind::RBrace, "expected enum variants `}`")?
            .span;
        Ok(EnumDecl {
            public,
            name,
            variants,
            span: self.source.span(public.unwrap_or(start).start, end.end),
            end,
        })
    }
    fn enum_payload_type(&mut self) -> Result<EnumPayloadSyntax, Box<Diagnostic>> {
        let start = self.peek().span;
        let (kind, end) = match self.peek().kind {
            Kind::Ident if self.source.text_at(start) == "bool" => {
                self.bump();
                (ScalarTypeSyntax::Bool, start.end)
            }
            Kind::Ident if self.source.text_at(start) == "i32" => {
                self.bump();
                (ScalarTypeSyntax::I32, start.end)
            }
            Kind::LParen => {
                self.bump();
                let end = self
                    .expect(
                        Kind::RParen,
                        "only bool, i32 and () enum payloads are supported",
                    )?
                    .span
                    .end;
                (ScalarTypeSyntax::Unit, end)
            }
            _ => return Err(self.enum_error("only bool, i32 and () enum payloads are supported")),
        };
        Ok(EnumPayloadSyntax {
            kind,
            span: self.source.span(start.start, end),
        })
    }
    pub(super) fn enum_path(&mut self) -> Result<PathId, Box<Diagnostic>> {
        let first = self
            .expect(Kind::Ident, "expected qualified enum variant path")?
            .span;
        let root = if self.source.text_at(first) == "crate" {
            if self.mode != SourceMode::ProjectCandidate {
                return Err(self.enum_error("absolute paths require project syntax"));
            }
            self.project_recovery = true;
            PathRoot::Crate
        } else {
            PathRoot::LocalType
        };
        if !self.double_colon() {
            return Err(self.enum_error("variant path requires `::`"));
        }
        let start = self.path_segments.len();
        let mut length = 0;
        self.path_segment(&mut length, first)?;
        let end = loop {
            if root == PathRoot::LocalType && length == 2 {
                return Err(self.enum_error("local variant paths have exactly two segments"));
            }
            self.bump();
            self.bump();
            let segment = self
                .expect(Kind::Ident, "expected path segment after `::`")?
                .span;
            self.path_segment(&mut length, segment)?;
            if !self.double_colon() {
                break segment.end;
            }
        };
        let span = self.source.span(first.start, end);
        let segment_len = u8::try_from(length)
            .map_err(|_| reserve_error(span, "syntax storage count overflow"))?;
        reserve(
            self.allocator,
            &mut self.storage,
            &mut self.paths,
            MAX_NODES,
            "qualified paths",
            false,
            span,
        )?;
        let id = PathId(self.paths.len());
        self.paths.push(QualifiedPath {
            span,
            segment_start: start,
            segment_len,
            root,
        });
        Ok(id)
    }
    fn fat_arrow(&mut self) -> Result<(), Box<Diagnostic>> {
        let equal = self.peek();
        if equal.kind != Kind::Equal
            || !self
                .tokens
                .get(self.cursor + 1)
                .is_some_and(|next| next.kind == Kind::Greater && equal.span.end == next.span.start)
        {
            return Err(self.enum_error("match arm requires contiguous `=>`"));
        }
        self.bump();
        self.bump();
        Ok(())
    }
    pub(super) fn match_statement(
        &mut self,
        blocks: &mut Vec<BodyBlock>,
        depth: usize,
        start: usize,
    ) -> Result<Stmt, Box<Diagnostic>> {
        self.bump(); // statement() already charged the match node.
        let scrutinee = self
            .expect(Kind::Ident, "match requires a bare binding name")?
            .span;
        self.expect(
            Kind::LBrace,
            "match requires a bare binding followed by `{`",
        )?;
        let mut arms = Vec::new();
        if self.peek().kind == Kind::RBrace {
            return Err(self.enum_error("match requires at least one arm"));
        }
        loop {
            if self.peek().kind != Kind::Ident {
                return Err(self.error("expected qualified variant pattern"));
            }
            if arms.len() >= MAX_ENUM_VARIANTS {
                return Err(self.diagnostic(
                    "E0400",
                    "parse",
                    "match arm limit exceeded (256)",
                    Some(self.peek().span),
                ));
            }
            self.node()?;
            let arm_start = self.peek().span.start;
            let variant = self.enum_path()?;
            if self.paths[variant.0].root == PathRoot::Crate
                && self.paths[variant.0].segment_len < 3
            {
                return Err(
                    self.enum_error("absolute variant patterns require an enum type prefix")
                );
            }
            let binding = if self.take(Kind::LParen).is_some() {
                let name = self
                    .expect(
                        Kind::Ident,
                        "payload pattern requires one immutable binding name",
                    )?
                    .span;
                if self.source.text_at(name) == "_" {
                    return Err(self.diagnostic(
                        "E0100",
                        "parse",
                        "wildcard patterns are unavailable",
                        Some(name),
                    ));
                }
                self.expect(Kind::RParen, "payload pattern requires exactly one binding")?;
                Some(name)
            } else {
                None
            };
            self.fat_arrow()?;
            if depth >= MAX_BLOCK_NESTING {
                return Err(self.diagnostic(
                    "E0400",
                    "parse",
                    "statement block nesting limit exceeded",
                    Some(self.peek().span),
                ));
            }
            let body = self.block(blocks, depth, "match arm requires a braced block")?;
            let span = self.source.span(arm_start, blocks[body.0].end.end);
            self.candidate_reserve(&mut arms, MAX_ENUM_VARIANTS, "enum match arms", span)?;
            arms.push(MatchArmSyntax {
                variant,
                binding,
                body,
                span,
            });
            if self.take(Kind::Comma).is_none() || self.peek().kind == Kind::RBrace {
                break;
            }
        }
        let end = self
            .expect(Kind::RBrace, "expected match arms `}`")?
            .span
            .end;
        Ok(Stmt {
            kind: StmtKind::Match { scrutinee, arms },
            span: self.source.span(start, end),
        })
    }
}
