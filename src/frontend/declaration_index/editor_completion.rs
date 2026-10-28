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
pub(in crate::frontend) type NamespaceBuildResult<'a, 's> =
    Result<Namespace<'a, 's>, Box<Diagnostic>>;
pub(in crate::frontend) type NamespaceCompletionResult<'a, 's> =
    Result<Namespace<'a, 's>, Vec<Diagnostic>>;
impl<'a, 's> Namespace<'a, 's> {
    pub(in crate::frontend) fn build(
        sources: &'a QuerySources<'s>,
        work: &'a WorkMeter,
        allocator: &mut Allocator,
    ) -> NamespaceBuildResult<'a, 's> {
        Self::build_limited(sources, work, allocator, IndexLimits::default().scratch)
    }
    fn build_limited(
        sources: &'a QuerySources<'s>,
        work: &'a WorkMeter,
        allocator: &mut Allocator,
        limit: u64,
    ) -> NamespaceBuildResult<'a, 's> {
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
        // This bank covers namespace construction and its complete return
        // transports, conservatively summing successive owned roles. It is
        // not an editor-wide memory cap. The selected caller query stays live;
        // its earlier Result/Option transports have already completed.
        let fixed = size_of::<Self>() // construction fields
            + size_of::<Vec<u32>>()
            + size_of::<NamespaceBuildResult<'a, 's>>() // limited return
            + size_of::<NamespaceBuildResult<'a, 's>>() // forwarding return
            + size_of::<NamespaceCompletionResult<'a, 's>>() // caller mapping
            + size_of::<Self>() // selected caller namespace
            + size_of::<PathQuery>(); // selected caller query, once
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
        if rows.capacity() != count {
            return Err(resource("editor namespace allocation failed", at));
        }
        let mut order = allocate_exact(count, 0u32, allocator, "editor namespace order", at, work)?;
        let mut scratch =
            allocate_exact(count, 0u32, allocator, "editor namespace merge", at, work)?;
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
        for &index in &order {
            let declaration = rows[index as usize].declaration;
            work.debit(1, declaration.name, "editor declaration conflict")?;
            if declaration.kind != DeclarationKind::Function {
                let spelling = project.sources().text(declaration.name);
                if compare_bytes(spelling, "bool", work, declaration.name)? == Ordering::Equal
                    || compare_bytes(spelling, "i32", work, declaration.name)? == Ordering::Equal
                {
                    return Err(diagnostic(
                        "E0202",
                        "resolve",
                        "scalar type names cannot be redeclared",
                        declaration.name,
                    ));
                }
            }
        }
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
                for (ordinal, header) in self.sources.project().editor_modules().iter().enumerate()
                {
                    self.work.debit(1, segment, "editor child identity probe")?;
                    if header.declaration == Some(row.declaration.name) {
                        child = Some(ordinal);
                        break;
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{
        parser::editor_query::EditorFile,
        project::{
            editor_sources::SourceOverlay,
            packages::{PackageSources, PackageSpec},
        },
    };
    use std::{
        fs,
        mem::align_of,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            if std::thread::panicking() {
                eprintln!("retained editor namespace fixture: {}", self.0.display());
            } else {
                fs::remove_dir_all(&self.0).unwrap();
            }
        }
    }
    fn with_sources(text: &str, child: Option<&str>, run: impl FnOnce(&QuerySources<'_>)) {
        let path = std::env::temp_dir().join(format!(
            "oxid-editor-namespace-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let fixture = Fixture(path.canonicalize().unwrap());
        fs::write(fixture.0.join("main.ox"), text).unwrap();
        if let Some(text) = child {
            fs::write(fixture.0.join("child.ox"), text).unwrap();
        }
        let specs = [PackageSpec {
            root: fixture.0.clone(),
            entry: fixture.0.join("main.ox"),
            dependencies: vec![],
        }];
        let overlay = SourceOverlay::new(&[]).unwrap();
        let session = QuerySources::load(&specs, None, &overlay).unwrap();
        run(&session);
    }
    #[test]
    fn editor_namespace_layout_and_exact_scratch_boundary_are_independently_derived() {
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(size_of::<usize>(), 8);
            assert_eq!(size_of::<Span>(), 24);
            assert_eq!(align_of::<Span>(), 8);
            assert_eq!(size_of::<DeclarationKind>(), 1);
            assert_eq!(size_of::<bool>(), 1);
            assert_eq!(size_of::<ModuleId>(), 8);
            assert_eq!(size_of::<Vec<Row>>(), 24);
            assert_eq!(size_of::<Vec<u32>>(), 24);
            assert_eq!(size_of::<&QuerySources<'_>>(), 8);
            assert_eq!(size_of::<&WorkMeter>(), 8);
            assert_eq!(size_of::<[usize; 256]>(), 2048);
            assert_eq!(size_of::<Row>(), 40); // module word + 32B declaration
            assert_eq!(size_of::<Declaration>(), 32); // Span24 + kind1 + bool1 + pad6
            assert_eq!(size_of::<Namespace<'_, '_>>(), 2112); // two Vec24 + two refs8 +256 words
            assert_eq!(size_of::<PathQuery>(), 872); //34 Span24 + Span24 +four8B facts
            assert_eq!(size_of::<NamespaceBuildResult<'_, '_>>(), 2112);
            assert_eq!(size_of::<NamespaceCompletionResult<'_, '_>>(), 2112);
            assert_eq!(align_of::<Declaration>(), 8);
            assert_eq!(align_of::<Row>(), 8);
            assert_eq!(align_of::<PathQuery>(), 8);
            assert_eq!(align_of::<Namespace<'_, '_>>(), 8);
            assert_eq!(align_of::<NamespaceBuildResult<'_, '_>>(), 8);
            assert_eq!(align_of::<NamespaceCompletionResult<'_, '_>>(), 8);
            assert_eq!(
                size_of::<ProjectSources>(),
                size_of::<ProjectSources<EditorFile>>()
            );
            assert_eq!(
                size_of::<PackageSources<'_>>(),
                size_of::<PackageSources<'_, EditorFile>>()
            );
            with_sources(
                "fn answer()->i32{return 42;}fn main()->i32{return crate::;}",
                None,
                |sources| {
                    // Construction, two real raw return envelopes, one mapped
                    // return, one selected namespace; a merge Vec; one selected
                    // cursor query; and two actual rows/order/merge cells.
                    let fixed = 5 * 2112 + 24 + 872;
                    let exact = fixed + 2 * (40 + 4 + 4);
                    assert_eq!(fixed, 11456);
                    assert_eq!(exact, 11552);
                    for (limit, pass) in [(exact - 1, false), (exact, true)] {
                        let work = WorkMeter::default();
                        let mut allocator = Allocator::default();
                        let result =
                            Namespace::build_limited(sources, &work, &mut allocator, limit as u64);
                        assert_eq!(result.is_ok(), pass);
                        assert_eq!(allocator.attempts, if pass { 3 } else { 0 });
                        if !pass {
                            assert_eq!(
                                result.err().unwrap().message,
                                "editor namespace scratch limit exceeded"
                            );
                        }
                    }
                    for ordinal in 1..=3 {
                        let work = WorkMeter::default();
                        let mut allocator = Allocator {
                            fail_at: Some(ordinal),
                            ..Default::default()
                        };
                        let failure =
                            Namespace::build_limited(sources, &work, &mut allocator, exact as u64)
                                .err()
                                .expect("failed allocation must not publish a namespace");
                        assert_eq!(failure.code, "E0400");
                        assert_eq!(allocator.attempts, ordinal);
                    }
                },
            );
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn editor_namespace_each_child_identity_probe_spends_work_before_publication() {
        let text = "mod child;fn main()->i32{return crate::child::;}";
        with_sources(text, Some("pub fn answer()->i32{return 42;}"), |sources| {
            let file = sources.project().sources().get(SourceFileId(0));
            let cursor = text.find("crate::child::").unwrap() + 14;
            let query = sources.project().editor_files()[0]
                .query(file, cursor)
                .unwrap()
                .unwrap();
            for budget in [13u64, 14] {
                let work = WorkMeter::default();
                let namespace =
                    Namespace::build(sources, &work, &mut Allocator::default()).unwrap();
                let before = work.used();
                work.restrict(before + budget);
                let mut names = Vec::new();
                let result = namespace.visit(ModuleId(0), &query, |name, _| {
                    names.push(name.to_owned());
                    Ok(())
                });
                // prefix1 + declaration1 + comparison(1+5) + two real child
                // header probes + three candidate rows + one permission =14.
                assert_eq!(work.used(), before + budget);
                if budget == 14 {
                    result.unwrap();
                    assert_eq!(names, ["answer"]);
                } else {
                    assert_eq!(result.unwrap_err().code, "E0400");
                    assert!(names.is_empty());
                }
            }
        });
    }
}
