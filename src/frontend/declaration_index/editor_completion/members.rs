//! Nominal record lookup for member completion. Handles borrow this exact source
//! session; neither spelling nor a numeric record ID is a cross-session identity.
use super::*;
use crate::frontend::parser::editor_query::{
    members::{Fields, TypePath},
    EditorFile,
};

#[derive(Clone, Copy)]
pub(in crate::frontend) struct Record<'a, 's> {
    owner: &'a QuerySources<'s>,
    row: &'a Row,
}
impl Record<'_, '_> {
    pub(in crate::frontend) fn module(self) -> ModuleId {
        self.row.module
    }
    pub(in crate::frontend) fn name(self) -> Span {
        self.row.declaration.name
    }
    pub(in crate::frontend) fn same(self, other: Self) -> bool {
        std::ptr::eq(self.owner, other.owner) && std::ptr::eq(self.row, other.row)
    }
}

// The private Row never escapes this namespace. Account the complete lookup
// transports here, where their real types remain nameable without exposing IDs.
#[repr(C)]
#[allow(dead_code)]
struct RowLookupRoles<'a> {
    receiver: &'a Namespace<'a, 'a>,
    module: ModuleId,
    requester: ModuleId,
    name: Span,
    kind: DeclarationKind,
    spelling: &'a str,
    iterator: std::slice::Iter<'a, Row>,
    selected: Option<&'a Row>,
    returned: Result<Option<&'a Row>, Box<Diagnostic>>,
    caller: Option<&'a Row>,
    comparison: Result<Ordering, Box<Diagnostic>>,
    permission: Result<bool, Box<Diagnostic>>,
}
pub(in crate::frontend) fn lookup_control_bytes() -> usize {
    size_of::<RowLookupRoles<'_>>()
}

impl<'a, 's> Namespace<'a, 's> {
    pub(in crate::frontend) fn build_for_members(
        sources: &'a QuerySources<'s>,
        work: &'a WorkMeter,
        allocator: &mut Allocator,
        extra: usize,
        limit: u64,
    ) -> NamespaceBuildResult<'a, 's> {
        let at = sources.project().sources().get(SourceFileId(0)).span(0, 0);
        let remaining = limit
            .min(IndexLimits::default().scratch)
            .checked_sub(extra as u64)
            .ok_or_else(|| resource("editor member scratch limit exceeded", at))?;
        Self::build_limited(sources, work, allocator, remaining)
    }

    /// The unchanged namespace ledger includes its merge storage and complete
    /// construction/return transports. Member-only storage is admitted alongside
    /// that bank; it never borrows an unused part of the old PathQuery allowance.
    pub(in crate::frontend) fn member_namespace_bytes(&self) -> Option<usize> {
        let fixed = size_of::<Self>()
            + size_of::<Vec<u32>>()
            + 2 * size_of::<NamespaceBuildResult<'a, 's>>()
            + size_of::<NamespaceCompletionResult<'a, 's>>()
            + size_of::<Self>()
            + size_of::<PathQuery>();
        self.rows
            .len()
            .checked_mul(size_of::<Row>() + 2 * size_of::<u32>())?
            .checked_add(fixed)
    }

    fn member_row(
        &self,
        module: ModuleId,
        name: Span,
        kind: DeclarationKind,
        requester: ModuleId,
    ) -> Result<Option<&Row>, Box<Diagnostic>> {
        let map = self.sources.project().sources();
        let spelling = map.try_text(name).ok_or_else(|| bad(name))?;
        for row in &self.rows {
            self.work.debit(1, name, "editor member declaration")?;
            if row.module == module
                && row.declaration.kind == kind
                && compare_bytes(map.text(row.declaration.name), spelling, self.work, name)?
                    == Ordering::Equal
            {
                return Ok(self.visible(row, requester)?.then_some(row));
            }
        }
        Ok(None)
    }

    pub(in crate::frontend) fn member_record<'n>(
        &'n self,
        requester: ModuleId,
        path: TypePath,
    ) -> Result<Option<Record<'n, 's>>, Box<Diagnostic>> {
        let project = self.sources.project();
        let at = path.span();
        if at.file.0 != requester.0 {
            return Err(bad(at));
        }
        let file = project
            .editor_files()
            .get(requester.0)
            .ok_or_else(|| bad(at))?;
        let source = project.sources().get(at.file);
        let mut names = file.member_path(source, path, self.work)?;
        let Some(first) = names.next()? else {
            return Err(bad(at));
        };
        let mut current = names.next()?;
        if current.is_none() {
            if file.has_member_import(source, source.text_at(first), self.work)? {
                return Ok(None);
            }
            return Ok(
                match self.member_row(requester, first, DeclarationKind::Record, requester)? {
                    Some(row) => Some(Record {
                        owner: self.sources,
                        row,
                    }),
                    None => None,
                },
            );
        }
        if source.text_at(first) != "crate" {
            return Err(bad(at));
        }
        let mut selected = self.sources.root(requester).ok_or_else(|| bad(at))?;
        let mut first_segment = true;
        while let Some(segment) = current {
            let next = names.next()?;
            if next.is_none() {
                return Ok(
                    match self.member_row(selected, segment, DeclarationKind::Record, requester)? {
                        Some(row) => Some(Record {
                            owner: self.sources,
                            row,
                        }),
                        None => None,
                    },
                );
            }
            let mut child = None;
            if first_segment {
                let name = source.text_at(segment);
                for edge in self.sources.dependencies(requester) {
                    self.work.debit(1, segment, "editor member dependency")?;
                    if compare_bytes(&edge.alias, name, self.work, segment)? == Ordering::Equal {
                        child = self.sources.dependency_target(edge.package);
                        break;
                    }
                }
            }
            if child.is_none() {
                let Some(row) =
                    self.member_row(selected, segment, DeclarationKind::Module, requester)?
                else {
                    return Ok(None);
                };
                for (ordinal, header) in project.editor_modules().iter().enumerate() {
                    self.work
                        .debit(1, segment, "editor member module identity")?;
                    if header.declaration == Some(row.declaration.name) {
                        child = Some(ModuleId(ordinal));
                        break;
                    }
                }
            }
            let Some(child) = child else {
                return Ok(None);
            };
            selected = child;
            current = next;
            first_segment = false;
        }
        Err(bad(at))
    }

    fn member_file<'n>(
        &'n self,
        record: Record<'_, '_>,
    ) -> Result<(&'n EditorFile, &'n SourceFile), Box<Diagnostic>> {
        if !std::ptr::eq(self.sources, record.owner) {
            return Err(bad(record.name()));
        }
        let project = self.sources.project();
        let file = project
            .editor_files()
            .get(record.module().0)
            .ok_or_else(|| bad(record.name()))?;
        let source = project.sources().get(record.name().file);
        if !file.belongs_to(source) {
            return Err(bad(record.name()));
        }
        Ok((file, source))
    }
    pub(in crate::frontend) fn member_fields<'n>(
        &'n self,
        record: Record<'_, '_>,
    ) -> Result<Option<Fields<'n>>, Box<Diagnostic>> {
        let (file, source) = self.member_file(record)?;
        file.member_fields(source, record.name(), self.work)
    }
    pub(in crate::frontend) fn member_private_visible(
        &self,
        record: Record<'_, '_>,
        requester: ModuleId,
    ) -> Result<bool, Box<Diagnostic>> {
        self.member_file(record)?;
        self.work
            .debit(1, record.name(), "editor member field permission")?;
        Ok(module_contains(
            record.module().0,
            self.ends[record.module().0],
            requester.0,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of};
    #[test]
    fn member_lookup_complete_option_and_result_carriers_are_independently_priced() {
        let members = [
            (
                offset_of!(RowLookupRoles<'_>, receiver),
                size_of::<&Namespace<'_, '_>>(),
                align_of::<&Namespace<'_, '_>>(),
            ),
            (
                offset_of!(RowLookupRoles<'_>, module),
                size_of::<ModuleId>(),
                align_of::<ModuleId>(),
            ),
            (
                offset_of!(RowLookupRoles<'_>, requester),
                size_of::<ModuleId>(),
                align_of::<ModuleId>(),
            ),
            (
                offset_of!(RowLookupRoles<'_>, name),
                size_of::<Span>(),
                align_of::<Span>(),
            ),
            (
                offset_of!(RowLookupRoles<'_>, kind),
                size_of::<DeclarationKind>(),
                align_of::<DeclarationKind>(),
            ),
            (
                offset_of!(RowLookupRoles<'_>, spelling),
                size_of::<&str>(),
                align_of::<&str>(),
            ),
            (
                offset_of!(RowLookupRoles<'_>, iterator),
                size_of::<std::slice::Iter<'_, Row>>(),
                align_of::<std::slice::Iter<'_, Row>>(),
            ),
            (
                offset_of!(RowLookupRoles<'_>, selected),
                size_of::<Option<&Row>>(),
                align_of::<Option<&Row>>(),
            ),
            (
                offset_of!(RowLookupRoles<'_>, returned),
                size_of::<Result<Option<&Row>, Box<Diagnostic>>>(),
                align_of::<Result<Option<&Row>, Box<Diagnostic>>>(),
            ),
            (
                offset_of!(RowLookupRoles<'_>, caller),
                size_of::<Option<&Row>>(),
                align_of::<Option<&Row>>(),
            ),
            (
                offset_of!(RowLookupRoles<'_>, comparison),
                size_of::<Result<Ordering, Box<Diagnostic>>>(),
                align_of::<Result<Ordering, Box<Diagnostic>>>(),
            ),
            (
                offset_of!(RowLookupRoles<'_>, permission),
                size_of::<Result<bool, Box<Diagnostic>>>(),
                align_of::<Result<bool, Box<Diagnostic>>>(),
            ),
        ];
        let (mut total, mut alignment) = (0usize, 1usize);
        for (offset, bytes, align) in members {
            total = total.div_ceil(align) * align;
            assert_eq!(offset, total);
            total += bytes;
            alignment = alignment.max(align);
        }
        assert_eq!(
            lookup_control_bytes(),
            total.div_ceil(alignment) * alignment
        );
        assert_eq!(align_of::<RowLookupRoles<'_>>(), alignment);
    }
}
