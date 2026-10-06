//! Frozen declaration views; catalog facts do not themselves authorize execution.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum NominalId { Record(RecordId), Enum(EnumId) }
#[cfg(test)]
impl NominalId {
    pub(super) fn legacy_record(self) -> RecordId {
        match self { Self::Record(record) => record, Self::Enum(_) => panic!("legacy index observation excludes enum projection") }
    }
}
pub(in crate::frontend) struct EnumView<'a> { id: EnumId, index: &'a DeclarationIndex<'a> }
pub(in crate::frontend) struct VariantView<'a> { id: VariantId, index: &'a DeclarationIndex<'a> }
impl fmt::Debug for EnumView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct("EnumView").field("id", &self.id).finish() }
}
impl fmt::Debug for VariantView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct("VariantView").field("id", &self.id).finish() }
}
impl<'a> EnumView<'a> {
    pub(super) fn from_index(index: &'a DeclarationIndex<'_>, id: EnumId) -> Result<Self, Box<Diagnostic>> {
        index.enum_origin(id)?;
        Ok(Self { id, index })
    }
    pub fn id(&self) -> EnumId { self.id }
    pub fn origin(&self) -> DeclarationOrigin { self.index.frozen_enum_origin(self.id) }
    pub fn name(&self) -> &'a str { self.index.frozen_enum_name(self.id) }
    pub fn diagnostic_span(&self) -> Span { self.index.frozen_enum_anchor(self.id) }
    /// Source syntax exists only for the original prefix, never for builtins.
    pub fn source_syntax(&self) -> Option<&'a ast::EnumDecl> { self.index.frozen_enum_syntax(self.id) }
    pub fn name_span(&self) -> Span { self.source_syntax().expect("source-only enum name span").name }
    pub fn span(&self) -> Span { self.source_syntax().expect("source-only enum span").span }
    pub fn end(&self) -> Span { self.source_syntax().expect("source-only enum end").end }
    pub fn variant_count(&self) -> usize { self.index.frozen_variant_count(self.id) }
    pub fn variant(&self, id: VariantId) -> Result<VariantView<'a>, Box<Diagnostic>> {
        if id.enumeration != self.id || id.index >= self.variant_count() { return Err(bad(self.diagnostic_span())); }
        Ok(VariantView { id, index: self.index })
    }
}
impl<'a> VariantView<'a> {
    pub fn id(&self) -> VariantId { self.id }
    pub fn origin(&self) -> DeclarationOrigin { self.index.frozen_enum_origin(self.id.enumeration) }
    pub fn name(&self) -> &'a str {
        match self.source_syntax() {
            Some(syntax) => self.index.sources().text(syntax.name).expect("frozen source variant name"),
            None => BuiltinEnum::ReadStatus.member_name(self.id.index).expect("checked builtin variant"),
        }
    }
    pub fn diagnostic_span(&self) -> Span {
        self.source_syntax().map_or_else(|| self.index.frozen_enum_anchor(self.id.enumeration), |syntax| syntax.name)
    }
    pub fn source_syntax(&self) -> Option<&'a ast::EnumVariantSyntax> {
        self.index.frozen_enum_syntax(self.id.enumeration).map(|syntax| &syntax.variants[self.id.index])
    }
    pub fn name_span(&self) -> Span { self.source_syntax().expect("source-only variant name span").name }
    pub fn span(&self) -> Span { self.source_syntax().expect("source-only variant span").span }
    pub fn payload_span(&self) -> Option<Span> { self.source_syntax().and_then(|syntax| syntax.payload.map(|payload| payload.span)) }
    pub fn payload(&self) -> Option<Ty> {
        match self.source_syntax() {
            Some(syntax) => syntax.payload.map(|payload| match payload.kind {
                ast::ScalarTypeSyntax::Bool => Ty::Bool, ast::ScalarTypeSyntax::I32 => Ty::I32, ast::ScalarTypeSyntax::Unit => Ty::Unit,
            }),
            None => BuiltinEnum::ReadStatus.member_payload(self.id.index),
        }
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
        if self.next == self.index.enum_count() { return None; }
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
