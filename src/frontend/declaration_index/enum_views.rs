//! Frozen declaration views; catalog facts do not themselves authorize execution.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum NominalId {
    Record(RecordId),
    Enum(EnumId),
    ByteBuf,
    File,
}
#[cfg(test)]
impl NominalId {
    pub(super) fn legacy_record(self) -> RecordId {
        match self {
            Self::Record(record) => record,
            Self::Enum(_) | Self::ByteBuf | Self::File => {
                panic!("legacy index observation excludes enum projection")
            }
        }
    }
}
pub(in crate::frontend) struct EnumView<'a> {
    id: EnumId,
    index: &'a DeclarationIndex<'a>,
}
pub(in crate::frontend) struct VariantView<'a> {
    id: VariantId,
    index: &'a DeclarationIndex<'a>,
}
impl fmt::Debug for EnumView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnumView").field("id", &self.id).finish()
    }
}
impl fmt::Debug for VariantView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VariantView").field("id", &self.id).finish()
    }
}
impl<'a> EnumView<'a> {
    pub(super) fn from_index(
        index: &'a DeclarationIndex<'_>,
        id: EnumId,
    ) -> Result<Self, Box<Diagnostic>> {
        index.enum_origin(id)?;
        Ok(Self { id, index })
    }
    pub fn id(&self) -> EnumId {
        self.id
    }
    pub fn origin(&self) -> DeclarationOrigin {
        self.index.frozen_enum_origin(self.id)
    }
    pub fn name(&self) -> &'a str {
        self.index.frozen_enum_name(self.id)
    }
    pub fn diagnostic_span(&self) -> Span {
        self.index.frozen_enum_anchor(self.id)
    }
    /// Source syntax exists only for the original prefix, never for builtins.
    pub fn source_syntax(&self) -> Option<&'a ast::EnumDecl> {
        self.index.frozen_enum_syntax(self.id)
    }
    pub fn name_span(&self) -> Span {
        self.source_syntax()
            .expect("source-only enum name span")
            .name
    }
    pub fn span(&self) -> Span {
        self.source_syntax().expect("source-only enum span").span
    }
    pub fn end(&self) -> Span {
        self.source_syntax().expect("source-only enum end").end
    }
    pub fn variant_count(&self) -> usize {
        self.index.frozen_variant_count(self.id)
    }
    pub fn variant(&self, id: VariantId) -> Result<VariantView<'a>, Box<Diagnostic>> {
        if id.enumeration != self.id || id.index >= self.variant_count() {
            return Err(bad(self.diagnostic_span()));
        }
        Ok(VariantView {
            id,
            index: self.index,
        })
    }
}
impl<'a> VariantView<'a> {
    pub fn id(&self) -> VariantId {
        self.id
    }
    pub fn origin(&self) -> DeclarationOrigin {
        self.index.frozen_enum_origin(self.id.enumeration)
    }
    pub fn name(&self) -> &'a str {
        match self.source_syntax() {
            Some(syntax) => self.index.sources().frozen_text(syntax.name),
            None => match self.origin() {
                DeclarationOrigin::Builtin(item) => item
                    .member_name(self.id.index)
                    .expect("checked builtin variant"),
                _ => unreachable!("checked builtin variant"),
            },
        }
    }
    pub fn diagnostic_span(&self) -> Span {
        self.source_syntax().map_or_else(
            || self.index.frozen_enum_anchor(self.id.enumeration),
            |syntax| syntax.name,
        )
    }
    pub fn source_syntax(&self) -> Option<&'a ast::EnumVariantSyntax> {
        self.index
            .frozen_enum_syntax(self.id.enumeration)
            .map(|syntax| &syntax.variants[self.id.index])
    }
    pub fn name_span(&self) -> Span {
        self.source_syntax()
            .expect("source-only variant name span")
            .name
    }
    pub fn span(&self) -> Span {
        self.source_syntax().expect("source-only variant span").span
    }
    pub fn payload_span(&self) -> Option<Span> {
        self.source_syntax()
            .and_then(|syntax| syntax.payload.map(|payload| payload.span))
    }
    pub fn has_owned_payload(&self) -> bool {
        if let DeclarationOrigin::Builtin(BuiltinItem::Experimental(item)) = self.origin() {
            return item
                .enumeration()
                .and_then(|kind| kind.value_payload(self.id.index))
                .is_some_and(|payload| matches!(payload, ValueTy::Owned(_)));
        }
        self.source_syntax().is_some_and(|syntax| {
            syntax.payload.is_some_and(|payload| {
                matches!(
                    payload.kind,
                    ast::ScalarTypeSyntax::Name
                        | ast::ScalarTypeSyntax::Absolute(_)
                        | ast::ScalarTypeSyntax::Array(_)
                )
            })
        })
    }
    pub fn has_payload(&self) -> bool {
        self.has_owned_payload() || self.payload().is_some()
    }
    pub fn payload(&self) -> Option<Ty> {
        match self.source_syntax() {
            Some(syntax) => syntax.payload.map(|payload| match payload.kind {
                ast::ScalarTypeSyntax::Bool => Ty::Bool,
                ast::ScalarTypeSyntax::I32 => Ty::I32,
                ast::ScalarTypeSyntax::Unit => Ty::Unit,
                ast::ScalarTypeSyntax::Name
                | ast::ScalarTypeSyntax::Absolute(_)
                | ast::ScalarTypeSyntax::Array(_) => {
                    panic!("owned enum payload requires value_payload")
                }
            }),
            None => match self.origin() {
                DeclarationOrigin::Builtin(item) => item.member_payload(self.id.index),
                _ => unreachable!("checked builtin variant"),
            },
        }
    }
    /// Resolve an owned payload in its declaration's module. The consumer's
    /// scope cannot grant access to a private type or change nominal identity.
    /// Existing scalar and builtin payloads retain their original query work.
    pub fn value_payload(&self, work: &WorkMeter) -> Result<Option<ValueTy>, Box<Diagnostic>> {
        if let DeclarationOrigin::Builtin(BuiltinItem::Experimental(item)) = self.origin() {
            return Ok(item
                .enumeration()
                .and_then(|kind| kind.value_payload(self.id.index)));
        }
        if !self.has_owned_payload() {
            return Ok(self.payload().map(ValueTy::Scalar));
        }
        let payload = self
            .source_syntax()
            .and_then(|syntax| syntax.payload)
            .ok_or_else(|| bad(self.diagnostic_span()))?;
        let requester = self.index.sources().module_for_file(payload.span.file)?;
        self.index
            .query(work)
            .value_type(requester, payload.value_type(), TypeContext::Value)
            .map(Some)
    }
}

impl QuerySession<'_, '_> {
    /// A variant's nominal payload must be visible everywhere its enum is.
    /// Field visibility remains governed by the record's existing access rules.
    pub fn enum_payload_exposure(
        &mut self,
        enumeration: EnumId,
        nominal: NominalId,
        at: Span,
    ) -> Result<NominalExposure, Box<Diagnostic>> {
        self.work.debit(1, at, "enum payload exposure")?;
        let enumeration = self
            .tables
            .enums
            .get(enumeration.0)
            .ok_or_else(|| bad(at))?;
        let declared = self.tables.original(enumeration.original)?;
        self.requester(ModuleId(declared.owner as usize), at)?;
        let handle = self.tables.original_for_nominal(nominal, at)?;
        let original = match self.tables.project_handle(handle)? {
            DeclarationProjection::Builtin(_) => {
                return Ok(NominalExposure::Allowed);
            }
            DeclarationProjection::SourceOriginal(id) => id,
        };
        let payload = self.tables.original(original)?;
        if payload.domain == NONE
            || (declared.domain != NONE && self.tables.ancestor(payload.domain, declared.domain)?)
        {
            return Ok(NominalExposure::Allowed);
        }
        let restrictor = if payload.restrictor != NONE && payload.restrictor != original {
            Some(self.tables.original(payload.restrictor)?.name.span())
        } else {
            None
        };
        Ok(NominalExposure::Denied {
            declaration: payload.name.span(),
            restrictor,
        })
    }
}

impl DeclarationIndex<'_> {
    /// Feature selection over immutable source syntax; this is not a type or
    /// layout proof. Scalar enum compilation retains its original accounting.
    pub fn has_owned_enum_payloads(&self, work: &WorkMeter) -> Result<bool, Box<Diagnostic>> {
        if self
            .builtin_set()
            .experimental()
            .contains(experimental::Item::OpenResult)
        {
            work.debit(1, self.sources().eof(), "file result payload inventory")?;
            return Ok(true);
        }
        let mut inspected = 0u64;
        for ordinal in 0..self.source_enum_count() {
            let Some(enumeration) = self.frozen_enum_syntax(EnumId(ordinal)) else {
                return Err(bad(self.sources().eof()));
            };
            for variant in &enumeration.variants {
                inspected = inspected.checked_add(1).ok_or_else(|| {
                    diagnostic(
                        "E0400",
                        "resolve",
                        "enum payload inventory overflow",
                        variant.span,
                    )
                })?;
                if variant.payload.is_some_and(|payload| {
                    matches!(
                        payload.kind,
                        ast::ScalarTypeSyntax::Name
                            | ast::ScalarTypeSyntax::Absolute(_)
                            | ast::ScalarTypeSyntax::Array(_)
                    )
                }) {
                    // Scalar-only compilation keeps its historical work. Every
                    // visited row is charged when this new feature is selected;
                    // the selector itself is bounded by the admitted AST/index.
                    work.debit(inspected, variant.span, "owned enum payload inventory")?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
/// Exact count of the frozen source prefix and the closed optional suffix.
#[derive(Clone)]
pub(in crate::frontend) struct EnumVariantCounts<'a> {
    pub(super) index: &'a DeclarationIndex<'a>,
    pub(super) next: usize,
}
impl Iterator for EnumVariantCounts<'_> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        if self.next == self.index.enum_count() {
            return None;
        }
        let count = self.index.frozen_variant_count(EnumId(self.next));
        self.next += 1;
        Some(count)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.index.enum_count() - self.next;
        (remaining, Some(remaining))
    }
}
impl ExactSizeIterator for EnumVariantCounts<'_> {}
