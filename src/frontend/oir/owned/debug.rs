//! Read-only debug layouts borrowed from the sealed executable witness.
//!
//! These views carry no source/execution authority, own no storage and cannot
//! supply a caller-selected layout. Their IDs remain compilation-local.
use super::{verified::PlanWitness, *};
use std::mem::size_of;

// Bind the actual table-query results used below to their physical models.
pub(super) type RecordDeclaration<'a> = Result<&'a RecordDecl, DeclarationError>;
type RecordFields<'a> = Result<&'a [FieldDecl], DeclarationError>;

#[allow(dead_code)]
struct RecordQueryCarriers {
    // Construction, return and caller roles are a conservative sum; this is
    // not a claim that three separately owned copies survive simultaneously.
    handles: [DebugRecords<'static>; 3],
    identities: [RecordId; 3],
    direct_declarations: [RecordDeclaration<'static>; 3],
    // fields(id) performs its own record(id) query. Price that nested query's
    // construction/return/caller roles as well as the preceding direct lookup.
    field_declarations: [RecordDeclaration<'static>; 3],
    selected_declaration: [Option<&'static RecordDecl>; 3],
    fields: [RecordFields<'static>; 3],
    selected_fields: [Option<&'static [FieldDecl]>; 3],
    construction: DebugRecord<'static>,
    result: [Option<DebugRecord<'static>>; 3],
}

#[allow(dead_code)]
struct FieldQueryCarriers {
    records: [DebugRecord<'static>; 3],
    indices: [usize; 3],
    selected_declaration: [Option<&'static FieldDecl>; 3],
    values: [ValueTy; 2],
    scalar: hir::Ty,
    layout: Layout,
    construction: DebugField,
    result: [Option<DebugField>; 3],
}

#[derive(Clone, Copy)]
pub(in crate::frontend) struct DebugRecords<'a> {
    declarations: &'a Declarations,
}

impl<'a> DebugRecords<'a> {
    pub(in crate::frontend) const fn control_bytes() -> usize {
        // Record and scalar-field queries are successive, never recursive.
        // The renderer pays the maximum query chain in addition to its own
        // support-result and formatting roles, without a retained table.
        if size_of::<RecordQueryCarriers>() > size_of::<FieldQueryCarriers>() {
            size_of::<RecordQueryCarriers>()
        } else {
            size_of::<FieldQueryCarriers>()
        }
    }

    pub(super) fn new<W: PlanWitness>(witness: &'a W) -> Self {
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
        let declaration: RecordDeclaration<'a> = self.declarations.record(id);
        let declaration = declaration.ok()?;
        let fields: RecordFields<'a> = self.declarations.fields(id);
        let fields = fields.ok()?;
        Some(DebugRecord {
            declaration,
            fields,
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

#[cfg(all(test, target_pointer_width = "64"))]
#[test]
fn debug_layout_query_roles_have_independent_physical_oracles() {
    use std::mem::align_of;
    // Qualified 64-bit representation: a nominal record view has a thin
    // declaration reference and a two-word field slice. Declaration errors
    // include a three-word Span plus their discriminant. These numeric widths
    // are independent of control_bytes() and its carrier structs.
    assert_eq!(
        (
            size_of::<DebugRecords<'_>>(),
            align_of::<DebugRecords<'_>>()
        ),
        (8, 8)
    );
    assert_eq!(
        (size_of::<DebugRecord<'_>>(), align_of::<DebugRecord<'_>>()),
        (24, 8)
    );
    assert_eq!(
        (
            size_of::<Option<DebugRecord<'_>>>(),
            align_of::<Option<DebugRecord<'_>>>()
        ),
        (24, 8)
    );
    assert_eq!(
        (
            size_of::<DeclarationError>(),
            align_of::<DeclarationError>()
        ),
        (32, 8)
    );
    assert_eq!(
        (
            size_of::<RecordDeclaration<'_>>(),
            align_of::<RecordDeclaration<'_>>()
        ),
        (32, 8)
    );
    assert_eq!(
        (
            size_of::<RecordFields<'_>>(),
            align_of::<RecordFields<'_>>()
        ),
        (32, 8)
    );
    assert_eq!((size_of::<DebugField>(), align_of::<DebugField>()), (56, 8));
    assert_eq!(
        (
            size_of::<Option<DebugField>>(),
            align_of::<Option<DebugField>>()
        ),
        (56, 8)
    );
    assert_eq!((size_of::<ValueTy>(), align_of::<ValueTy>()), (16, 8));
    assert_eq!((size_of::<Layout>(), align_of::<Layout>()), (16, 8));
    assert_eq!((size_of::<hir::Ty>(), align_of::<hir::Ty>()), (1, 1));
    // Three construction/return/caller transports for each real lookup; one
    // record construction. Query phases share the larger fixed envelope.
    let record_roles: usize = 3 * 8 + 3 * 8 + 2 * (3 * 32) + 3 * 8 + 3 * 32 + 3 * 16 + 24 + 3 * 24;
    let field_unpadded: usize = 3 * 24 + 3 * 8 + 3 * 8 + 2 * 16 + 1 + 16 + 56 + 3 * 56;
    let field_roles = field_unpadded.div_ceil(8) * 8;
    assert_eq!(
        (
            size_of::<RecordQueryCarriers>(),
            align_of::<RecordQueryCarriers>()
        ),
        (record_roles, 8)
    );
    assert_eq!(
        (
            size_of::<FieldQueryCarriers>(),
            align_of::<FieldQueryCarriers>()
        ),
        (field_roles, 8)
    );
    assert_eq!(DebugRecords::control_bytes(), record_roles.max(field_roles));
}
