//! Read-only debug layouts borrowed from the sealed executable witness.
//!
//! These views carry no source/execution authority, own no storage and cannot
//! supply a caller-selected layout. Their IDs remain compilation-local.
use super::{verified::VerifiedOwnedProgram, *};

#[derive(Clone, Copy)]
pub(in crate::frontend) struct DebugRecords<'a> {
    declarations: &'a Declarations,
}

impl<'a> DebugRecords<'a> {
    pub(super) fn new(witness: &'a VerifiedOwnedProgram) -> Self {
        Self {
            declarations: witness.declarations(),
        }
    }

    pub(in crate::frontend) fn len(self) -> usize {
        self.declarations.records().len()
    }

    pub(in crate::frontend) fn metadata_nodes(self) -> Option<usize> {
        self.len()
            .checked_mul(2)?
            .checked_add(self.declarations.usage().fields)
    }

    pub(in crate::frontend) fn record(self, id: usize) -> Option<DebugRecord<'a>> {
        let id = RecordId(id);
        Some(DebugRecord {
            declaration: self.declarations.record(id).ok()?,
            fields: self.declarations.fields(id).ok()?,
        })
    }
}

#[derive(Clone, Copy)]
pub(in crate::frontend) struct DebugRecord<'a> {
    declaration: &'a RecordDecl,
    fields: &'a [FieldDecl],
}

impl DebugRecord<'_> {
    pub(in crate::frontend) fn span(self) -> Span {
        self.declaration.span()
    }

    pub(in crate::frontend) fn is_copy(self) -> bool {
        self.declaration.is_copy()
    }

    pub(in crate::frontend) fn size(self) -> usize {
        self.declaration.layout().size()
    }

    pub(in crate::frontend) fn align(self) -> usize {
        self.declaration.layout().align()
    }

    pub(in crate::frontend) fn metadata_offset(self) -> Option<usize> {
        self.declaration
            .id()
            .0
            .checked_mul(2)?
            .checked_add(self.declaration.field_start())
    }

    pub(in crate::frontend) fn len(self) -> usize {
        self.fields.len()
    }

    pub(in crate::frontend) fn field(self, index: usize) -> Option<DebugField> {
        let field = self.fields.get(index)?;
        let ValueTy::Scalar(ty) = field.value_ty() else {
            return None;
        };
        let layout = Layout::scalar(ty);
        Some(DebugField {
            span: field.span(),
            ty,
            offset: field.offset(),
            size: layout.size(),
            align: layout.align(),
        })
    }
}

#[derive(Clone, Copy)]
pub(in crate::frontend) struct DebugField {
    pub(in crate::frontend) span: Span,
    pub(in crate::frontend) ty: hir::Ty,
    pub(in crate::frontend) offset: usize,
    pub(in crate::frontend) size: usize,
    pub(in crate::frontend) align: usize,
}

#[cfg(test)]
#[test]
fn borrowed_debug_layout_views_retain_no_table_or_witness_payload() {
    use std::mem::{align_of, size_of};
    assert_eq!(size_of::<DebugRecords<'_>>(), size_of::<&Declarations>());
    assert_eq!(align_of::<DebugRecords<'_>>(), align_of::<&Declarations>());
    assert_eq!(
        size_of::<DebugRecord<'_>>(),
        size_of::<(&RecordDecl, &[FieldDecl])>()
    );
    assert_eq!(
        align_of::<DebugRecord<'_>>(),
        align_of::<(&RecordDecl, &[FieldDecl])>()
    );
}
