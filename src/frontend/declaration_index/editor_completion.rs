//! Read-only source-name namespace. No executable index is constructed here.
use super::*;
use crate::frontend::{
    parser::{
        self,
        editor_query::{Declaration, DeclarationKind, PathQuery},
    },
    project::editor_sources::QuerySources,
};

#[derive(Clone, Copy, Debug)]
struct Row {
    module: ModuleId,
    declaration: Declaration,
}
pub(in crate::frontend) struct Namespace<'a, 's> {
    sources: &'a QuerySources<'s>,
    rows: Vec<Row>,
    order: Vec<u32>,
    ends: [usize; 256],
    work: &'a WorkMeter,
}
impl<'a, 's> Namespace<'a, 's> {
    pub(in crate::frontend) fn build(
        sources: &'a QuerySources<'s>,
        work: &'a WorkMeter,
        allocator: &mut Allocator,
    ) -> Result<Self, Box<Diagnostic>> {
        Self::build_limited(sources, work, allocator, IndexLimits::default().scratch)
    }
    fn build_limited(
        sources: &'a QuerySources<'s>,
        work: &'a WorkMeter,
        allocator: &mut Allocator,
        limit: u64,
    ) -> Result<Self, Box<Diagnostic>> {
        let project = sources.project();
        let at = project.sources().get(SourceFileId(0)).span(0, 0);
        let mut count = 0usize;
        for (ordinal, file) in project.editor_files().iter().enumerate() {
            if !file.belongs_to(project.sources().get(SourceFileId(ordinal))) {
                return Err(bad(at));
            }
            work.debit(1, at, "editor namespace file")?;
            count = count
                .checked_add(file.declarations().len())
                .ok_or_else(|| overflow(at))?;
        }
        // Real row/order/merge capacities and complete construction/return
        // carriers spend the existing scratch allowance before allocation.
        let fixed = 3 * size_of::<Self>()
            + size_of::<Result<Self, Box<Diagnostic>>>()
            + size_of::<Vec<u32>>()
            + 2 * size_of::<PathQuery>();
        let requested = count
            .checked_mul(size_of::<Row>() + 2 * size_of::<u32>())
            .and_then(|bytes| bytes.checked_add(fixed))
            .ok_or_else(|| overflow(at))?;
        if requested as u64 > limit.min(IndexLimits::default().scratch) {
            return Err(resource("editor namespace scratch limit exceeded", at));
        }
        let mut rows = Vec::new();
        allocator
            .vector_exact(&mut rows, count, "editor namespace rows")
            .map_err(|_| resource("editor namespace allocation failed", at))?;
        let mut order = allocate(count, 0u32, allocator, "editor namespace order", at, work)?;
        let mut scratch = allocate(count, 0u32, allocator, "editor namespace merge", at, work)?;
        for (module, file) in project.editor_files().iter().enumerate() {
            for &declaration in file.declarations() {
                work.debit(1, declaration.name, "editor namespace declaration")?;
                rows.push(Row {
                    module: ModuleId(module),
                    declaration,
                });
            }
        }
        for (index, slot) in order.iter_mut().enumerate() {
            *slot = compact(index, at)?;
        }
        let compare = |a: u32, b: u32| {
            let (a, b) = (&rows[a as usize], &rows[b as usize]);
            work.debit(1, at, "editor namespace comparison")?;
            let order = (a.module.0, a.declaration.kind != DeclarationKind::Function)
                .cmp(&(b.module.0, b.declaration.kind != DeclarationKind::Function));
            if order != Ordering::Equal {
                return Ok(order);
            }
            compare_bytes(
                project.sources().text(a.declaration.name),
                project.sources().text(b.declaration.name),
                work,
                at,
            )
        };
        merge_sort(&mut order, &mut scratch, compare, work, at)?;
        for pair in order.windows(2) {
            if compare(pair[0], pair[1])? == Ordering::Equal {
                return Err(duplicate(
                    rows[pair[1] as usize].declaration.name,
                    rows[pair[0] as usize].declaration.name,
                ));
            }
        }
        let headers = project.editor_modules();
        if headers.len() > 256 {
            return Err(resource("editor module limit exceeded", at));
        }
        let mut ends = [0usize; 256];
        for (index, header) in headers.iter().enumerate() {
            let mut end = index + 1;
            while end < headers.len() {
                work.debit(1, at, "editor namespace subtree")?;
                if headers[end].parent.is_none() || headers[end].depth <= header.depth {
                    break;
                }
                end += 1;
            }
            ends[index] = end;
        }
        Ok(Self {
            sources,
            rows,
            order,
            ends,
            work,
        })
    }

    fn visible(&self, row: &Row, requester: ModuleId) -> Result<bool, Box<Diagnostic>> {
        let headers = self.sources.project().editor_modules();
        let mut module = row.module;
        // This is the same intersection of declaring-module restrictions used
        // by ordinary original rows. The nearest restriction is the narrowest.
        let domain = if !row.declaration.public {
            Some(module)
        } else {
            let mut domain = None;
            for _ in 0..=32 {
                self.work
                    .debit(1, row.declaration.name, "editor namespace permission")?;
                let header = headers
                    .get(module.0)
                    .ok_or_else(|| bad(row.declaration.name))?;
                let Some(parent) = header.parent else {
                    break;
                };
                if header.public.is_none() {
                    domain = Some(parent);
                    break;
                }
                module = parent;
            }
            domain
        };
        Ok(domain.is_none_or(|domain| module_contains(domain.0, self.ends[domain.0], requester.0)))
    }

    pub(in crate::frontend) fn visit<'v>(
        &'v self,
        requester: ModuleId,
        query: &PathQuery,
        mut add: impl FnMut(&'v str, DeclarationKind) -> Result<(), Box<Diagnostic>>,
    ) -> Result<(), Box<Diagnostic>> {
        let map = self.sources.project().sources();
        let at = query.replacement();
        if query.segments().is_empty()
            || query.segments().len() >= parser::MAX_PATH_SEGMENTS
            || requester.0 >= self.sources.project().editor_files().len()
            || at.file.0 != requester.0
        {
            return Err(bad(at));
        }
        if map.try_text(query.segments()[0]) != Some("crate") {
            return Err(bad(at));
        }
        if !query.belongs_to(map.get(at.file)) {
            return Err(bad(at));
        }
        let mut selected = self.sources.root(requester).ok_or_else(|| bad(at))?;
        for (position, &segment) in query.segments()[1..query.segments().len()]
            .iter()
            .enumerate()
        {
            self.work.debit(1, segment, "editor qualified prefix")?;
            let name = map.try_text(segment).ok_or_else(|| bad(at))?;
            if position == 0 {
                let mut dependency = None;
                for edge in self.sources.dependencies(requester) {
                    if compare_bytes(&edge.alias, name, self.work, segment)? == Ordering::Equal {
                        dependency = self.sources.dependency_target(edge.package);
                        break;
                    }
                }
                if let Some(target) = dependency {
                    selected = target;
                    continue;
                }
            }
            let mut child = None;
            for row in &self.rows {
                self.work.debit(1, segment, "editor prefix declaration")?;
                if row.module != selected || row.declaration.kind != DeclarationKind::Module {
                    continue;
                }
                if compare_bytes(map.text(row.declaration.name), name, self.work, segment)?
                    != Ordering::Equal
                {
                    continue;
                }
                if !self.visible(row, requester)? {
                    return Ok(());
                }
                child = self
                    .sources
                    .project()
                    .editor_modules()
                    .iter()
                    .position(|header| header.declaration == Some(row.declaration.name));
                break;
            }
            let Some(child) = child else {
                return Ok(());
            };
            selected = ModuleId(child);
        }
        if query.segments().len() == 1 {
            for edge in self.sources.dependencies(requester) {
                self.work.debit(1, at, "editor dependency candidate")?;
                add(&edge.alias, DeclarationKind::Module)?;
            }
        }
        for &ordinal in &self.order {
            let row = &self.rows[ordinal as usize];
            self.work.debit(1, at, "editor qualified candidate")?;
            if row.module == selected && self.visible(row, requester)? {
                add(map.text(row.declaration.name), row.declaration.kind)?;
            }
        }
        Ok(())
    }
}
