//! Borrowed source declarations, not checked raw layouts or execution witnesses.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum NominalId {
    Record(RecordId),
    Enum(EnumId),
}

#[cfg(test)]
impl NominalId {
    pub(super) fn legacy_record(self) -> RecordId {
        match self {
            Self::Record(record) => record,
            Self::Enum(_) => panic!("legacy index observation excludes enum projection"),
        }
    }
}

#[derive(Debug)]
pub(in crate::frontend) struct EnumView<'a> {
    id: EnumId,
    syntax: &'a ast::EnumDecl,
}
#[derive(Debug)]
pub(in crate::frontend) struct VariantView<'a> {
    id: VariantId,
    syntax: &'a ast::EnumVariantSyntax,
}
impl<'a> EnumView<'a> {
    // The only ID/syntax pairing accepts a frozen index, never caller syntax.
    pub(super) fn from_index(
        index: &'a DeclarationIndex<'_>,
        id: EnumId,
    ) -> Result<Self, Box<Diagnostic>> {
        let (key, owner) = index.enumeration(id)?;
        let syntax = &index.sources().ast(owner)?.enums[key.index];
        Ok(Self { id, syntax })
    }
    // ID-only views confer neither requester visibility nor execution authority.
    pub fn id(&self) -> EnumId {
        self.id
    }
    pub fn name_span(&self) -> Span {
        self.syntax.name
    }
    pub fn span(&self) -> Span {
        self.syntax.span
    }
    pub fn end(&self) -> Span {
        self.syntax.end
    }
    pub fn variant_count(&self) -> usize {
        self.syntax.variants.len()
    }
    pub fn variant(&self, id: VariantId) -> Result<VariantView<'_>, Box<Diagnostic>> {
        if id.enumeration != self.id {
            return Err(bad(self.syntax.name));
        }
        let syntax = self
            .syntax
            .variants
            .get(id.index)
            .ok_or_else(|| bad(self.syntax.name))?;
        Ok(VariantView { id, syntax })
    }
}
impl VariantView<'_> {
    pub fn id(&self) -> VariantId {
        self.id
    }
    pub fn name_span(&self) -> Span {
        self.syntax.name
    }
    pub fn span(&self) -> Span {
        self.syntax.span
    }
    pub fn payload_span(&self) -> Option<Span> {
        self.syntax.payload.map(|payload| payload.span)
    }
    pub fn payload(&self) -> Option<Ty> {
        self.syntax.payload.map(|payload| match payload.kind {
            ast::ScalarTypeSyntax::Bool => Ty::Bool,
            ast::ScalarTypeSyntax::I32 => Ty::I32,
            ast::ScalarTypeSyntax::Unit => Ty::Unit,
        })
    }
}

/// Frozen row lengths only; users meter their surrounding traversal separately.
#[derive(Clone)]
pub(in crate::frontend) struct EnumVariantCounts<'a> {
    pub(super) rows: std::slice::Iter<'a, EnumRow>,
}
impl Iterator for EnumVariantCounts<'_> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        self.rows.next().map(|row| row.variant_len as usize)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.rows.size_hint()
    }
}
impl ExactSizeIterator for EnumVariantCounts<'_> {}
