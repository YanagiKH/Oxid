//! Generic headers are sparse, but remain bound to the actual source spelling.
use super::*;
use crate::frontend::lexer::Kind;

impl Program {
    pub(in crate::frontend) fn generic_parameters(&self, declaration: Span) -> &[GenericParam] {
        self.generic_headers()
            .binary_search_by_key(&declaration.start, |header| header.declaration.start)
            .ok()
            .map(|id| self.generic_headers()[id].parameters.as_slice())
            .unwrap_or(&[])
    }

    pub(in crate::frontend) fn generic_headers(&self) -> &[GenericHeader] {
        self.source.generic_headers()
    }

    pub(in crate::frontend) fn has_generics(&self) -> bool {
        self.source.has_generics()
    }

    pub(in crate::frontend) fn has_record_patterns(&self) -> bool {
        self.functions.iter().any(|function| {
            function.blocks.iter().any(|block| {
                block
                    .body
                    .iter()
                    .any(|statement| matches!(statement.kind, StmtKind::DestructureRecord { .. }))
            })
        })
    }

    pub(in crate::frontend) fn generic_calls(&self) -> &[GenericArguments] {
        self.source.generic_calls()
    }

    pub(in crate::frontend) fn generic_arguments(&self, callee: Span) -> Option<&[TypeSyntax]> {
        self.generic_calls()
            .binary_search_by_key(&callee.start, |call| call.callee.start)
            .ok()
            .and_then(|id| self.generic_calls().get(id))
            .filter(|call| {
                call.callee.file == callee.file
                    && call.context == GenericArgumentContext::Expression
            })
            .map(|call| call.arguments.as_slice())
    }

    pub(in crate::frontend) fn generic_application(
        &self,
        id: GenericApplicationId,
    ) -> Option<&GenericArguments> {
        self.generic_calls().get(id.0)
    }

    /// Generic functions can use the original single-file owned producer.
    /// Imports, module paths and visibility still require the project owner.
    pub(in crate::frontend) fn has_only_original_generic_syntax(&self) -> bool {
        (self.has_generics() || self.has_record_patterns())
            && self.modules.is_empty()
            && self.imports.is_empty()
            && self
                .paths
                .iter()
                .all(|path| path.root == PathRoot::LocalType)
            && self
                .functions
                .iter()
                .all(|function| function.public.is_none())
            && self.records.iter().all(|record| {
                record.public.is_none() && record.fields.iter().all(|field| field.public.is_none())
            })
            && self
                .enums
                .iter()
                .all(|enumeration| enumeration.public.is_none())
    }

    pub(super) fn generic_headers_match_source(&self, source: &SourceFile) -> bool {
        if !self.has_generics() {
            return self.generic_headers().is_empty();
        }
        if self
            .generic_headers()
            .windows(2)
            .any(|pair| pair[0].declaration.start >= pair[1].declaration.start)
        {
            return false;
        }
        let mut seen = 0usize;
        for declaration in self
            .functions
            .iter()
            .map(|item| item.name)
            .chain(self.records.iter().map(|item| item.name))
            .chain(self.enums.iter().map(|item| item.name))
        {
            let Ok(token) = self
                .tokens
                .binary_search_by_key(&declaration.start, |token| token.span.start)
            else {
                return false;
            };
            if self.tokens[token].span != declaration {
                return false;
            }
            let mut tokens = self.tokens[token + 1..]
                .iter()
                .filter(|token| token.kind != Kind::Trivia);
            let next = tokens.next();
            let parameters = self.generic_parameters(declaration);
            if parameters.is_empty() {
                if next.is_some_and(|token| source.try_text(token.span) == Some("<")) {
                    return false;
                }
                continue;
            }
            seen += 1;
            if next.is_none_or(|token| {
                token.kind != Kind::Less || source.try_text(token.span) != Some("<")
            }) {
                return false;
            }
            for (ordinal, parameter) in parameters.iter().enumerate() {
                if ordinal != 0
                    && tokens.next().is_none_or(|token| {
                        token.kind != Kind::Comma || source.try_text(token.span) != Some(",")
                    })
                {
                    return false;
                }
                let Some(name) = tokens.next() else {
                    return false;
                };
                if name.kind != Kind::Ident
                    || name.span != parameter.name
                    || parameter.span.start != name.span.start
                {
                    return false;
                }
                let mut end = name.span.end;
                if !parameter.bounds.is_empty() {
                    if tokens.next().is_none_or(|token| {
                        token.kind != Kind::Colon || source.try_text(token.span) != Some(":")
                    }) {
                        return false;
                    }
                    for (bound_ordinal, bound) in parameter.bounds.iter().enumerate() {
                        if bound_ordinal != 0
                            && tokens.next().is_none_or(|token| {
                                token.kind != Kind::Plus || source.try_text(token.span) != Some("+")
                            })
                        {
                            return false;
                        }
                        let mut segment = |span: Span| {
                            end = span.end;
                            tokens.next().is_some_and(|token| {
                                token.span == span && matches!(token.kind, Kind::Ident | Kind::Mod)
                            })
                        };
                        match bound {
                            ItemPath::Unqualified(span) => {
                                if !segment(*span) {
                                    return false;
                                }
                            }
                            ItemPath::Absolute(id) => {
                                let Some(segments) = self.path_segments(*id) else {
                                    return false;
                                };
                                for (i, span) in segments.iter().enumerate() {
                                    if i != 0 {
                                        for _ in 0..2 {
                                            if tokens.next().is_none_or(|token| {
                                                token.kind != Kind::Colon
                                                    || source.try_text(token.span) != Some(":")
                                            }) {
                                                return false;
                                            }
                                        }
                                    }
                                    if tokens.next().is_none_or(|token| token.span != *span) {
                                        return false;
                                    }
                                    end = span.end;
                                }
                            }
                        }
                    }
                }
                if parameter.span.end != end {
                    return false;
                }
            }
            let mut close = tokens.next();
            if close.is_some_and(|token| {
                token.kind == Kind::Comma && source.try_text(token.span) == Some(",")
            }) {
                close = tokens.next();
            }
            if close.is_none_or(|token| {
                token.kind != Kind::Greater || source.try_text(token.span) != Some(">")
            }) {
                return false;
            }
        }
        seen == self.generic_headers().len()
    }
}
