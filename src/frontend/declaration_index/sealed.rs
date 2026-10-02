//! Constructor authority. Sibling consumers cannot manufacture or replace these
//! associations, including descendants of the outer declaration_index module.
use super::*;

#[derive(Debug)]
pub(in crate::frontend) struct DeclarationFacts<'s> {
    tables: Tables<'s>,
    scratch: Scratch,
    plan: IndexPlan,
}
#[derive(Debug)]
struct CleanOriginals<'s> {
    facts: DeclarationFacts<'s>,
}
#[derive(Debug)]
pub(in crate::frontend) struct DeclarationIndex<'s> {
    tables: Tables<'s>,
}
const _: () = assert!(size_of::<DeclarationIndex<'static>>() <= 4096);

pub(in crate::frontend) fn collect_originals<'s>(
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<DeclarationFacts<'s>, Box<Diagnostic>> {
    work.restrict(limits.work);
    work.phase("collection");
    let at = sources.eof();
    // The old aggregate declaration envelope keeps its original first position.
    let mut remaining = 0usize;
    for module in 0..sources.count() {
        remaining = remaining
            .checked_add(sources.ast(ModuleId(module))?.records.len())
            .ok_or_else(|| overflow(at))?;
    }
    let fields = RecordFieldCounts {
        sources,
        module: 0,
        local: 0,
        remaining,
    };
    if super::super::oir::owned_types::admit_declaration_counts(fields).is_err() {
        return Err(diagnostic(
            "E0400",
            "resolve",
            "owned declaration resource limit exceeded",
            sources.file(ModuleId(0))?.span(0, 0),
        ));
    }
    let mut c = Counts {
        modules: u64::try_from(sources.count()).map_err(|_| overflow(at))?,
        ..Counts::default()
    };
    if c.modules == 0 || c.modules > 256 {
        return Err(bad(at));
    }
    for m in 0..sources.count() {
        work.preflight(at)?;
        let module = ModuleId(m);
        let ast = sources.ast(module)?;
        let file = sources.file(module)?;
        let mut failure = None;
        let valid = ast.validate_spans_and_ids_counted(|span| {
            if let Err(error) = work.preflight(span.unwrap_or(at)) {
                failure = Some(error);
                return false;
            }
            span.is_none_or(|span| {
                span.file == file.span(0, 0).file && file.try_text(span).is_some()
            })
        });
        if let Some(error) = failure {
            return Err(error);
        }
        if !valid {
            return Err(bad(at));
        }
        if sources.is_original_adapter() && ast.uses_project_syntax() {
            return Err(bad(at));
        }
        c.functions = add(c.functions, ast.functions.len() as u64, at)?;
        c.records = add(c.records, ast.records.len() as u64, at)?;
        c.imports = add(c.imports, ast.imports.len() as u64, at)?;
        for item in &ast.items {
            work.preflight(at)?;
            let name = match *item {
                ast::ItemId::Function(i) => Some(ast.functions[i].name),
                ast::ItemId::Struct(i) => Some(ast.records[i].name),
                ast::ItemId::Module(i) => Some(ast.modules[i].name),
                ast::ItemId::Import(_) => None,
            };
            if let Some(name) = name {
                c.original_bytes = add(c.original_bytes, sources.text(name)?.len() as u64, at)?;
            }
        }
        for record in &ast.records {
            work.preflight(record.name)?;
            c.fields = add(c.fields, record.fields.len() as u64, at)?;
        }
        for import in &ast.imports {
            work.preflight(import.alias)?;
            c.alias_bytes = add(c.alias_bytes, sources.text(import.alias)?.len() as u64, at)?;
            for segment in sources.segments(ItemPathRef {
                file: import.span.file,
                path: ast::ItemPath::Absolute(import.path),
            })? {
                work.preflight(*segment)?;
                c.path_weight = add(
                    c.path_weight,
                    add(sources.text(*segment)?.len() as u64, 2, at)?,
                    at,
                )?;
            }
        }
    }
    c.originals = add(add(c.functions, c.records, at)?, c.modules - 1, at)?;
    for count in [
        c.originals,
        c.functions,
        c.records,
        c.fields,
        c.modules,
        c.imports,
    ] {
        if count >= u64::from(BUILTIN_CONFLICT) {
            return Err(bad(at));
        }
    }
    let plan = IndexPlan::calculate(
        c,
        size_of::<DeclarationIndex<'s>>(),
        FIXED_SCRATCH,
        limits,
        at,
    )?;
    if add(work.used(), plan.build_work, at)? > work.limit().min(limits.work) {
        return Err(resource(
            "declaration index mandatory build work limit exceeded",
            at,
        ));
    }
    #[cfg(test)]
    work.observe(Observation::Plan(plan));
    let mut tables = Tables {
        sources,
        originals: allocate(
            c.originals as usize,
            OriginalRow::default(),
            allocator,
            "index originals",
            at,
            work,
        )?,
        original_order: allocate(
            c.originals as usize,
            0,
            allocator,
            "index original order",
            at,
            work,
        )?,
        functions: allocate(
            c.functions as usize,
            FunctionRow::default(),
            allocator,
            "index functions",
            at,
            work,
        )?,
        records: allocate(
            c.records as usize,
            RecordRow::default(),
            allocator,
            "index records",
            at,
            work,
        )?,
        fields: allocate(
            c.fields as usize,
            FieldRow::default(),
            allocator,
            "index fields",
            at,
            work,
        )?,
        modules: allocate(
            c.modules as usize,
            ModuleRow::default(),
            allocator,
            "index modules",
            at,
            work,
        )?,
        children: allocate(
            c.modules as usize - 1,
            NONE,
            allocator,
            "index children",
            at,
            work,
        )?,
        imports: allocate(
            c.imports as usize,
            ImportRow::default(),
            allocator,
            "index imports",
            at,
            work,
        )?,
        aliases: allocate(
            c.imports as usize,
            AliasCell::default(),
            allocator,
            "index aliases",
            at,
            work,
        )?,
        alias_order: allocate(
            c.imports as usize,
            0,
            allocator,
            "index alias order",
            at,
            work,
        )?,
        root_main: NONE,
    };
    let mut scratch = Scratch {
        merge: allocate(
            c.originals.max(c.imports) as usize,
            0,
            allocator,
            "index merge workspace",
            at,
            work,
        )?,
        targets: allocate(
            c.imports as usize,
            0,
            allocator,
            "index target order",
            at,
            work,
        )?,
        seen: allocate(
            c.imports as usize,
            SeenCell::default(),
            allocator,
            "index target seen",
            at,
            work,
        )?,
        next_child: allocate(
            c.modules as usize,
            0,
            allocator,
            "index child cursors",
            at,
            work,
        )?,
    };
    let (mut function_base, mut record_base, mut original_base, mut import_base, mut child_base) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    for m in 0..sources.count() {
        work.debit(1, at, "module prefix")?;
        let ast = sources.ast(ModuleId(m))?;
        let (parent, depth) = match sources.as_project() {
            Some(p) => {
                let h = &p.modules()[m];
                (h.parent.map_or(NONE, |id| id.0 as u32), h.depth)
            }
            _ => (NONE, 0),
        };
        if (m == 0 && (parent != NONE || depth != 0))
            || (m > 0 && (parent as usize >= m || depth == 0 || depth > 32))
        {
            return Err(bad(at));
        }
        let original_len = ast.functions.len() + ast.records.len() + ast.modules.len();
        tables.modules[m] = ModuleRow {
            parent,
            subtree_end: c.modules as u32,
            original: NONE,
            function_base: compact(function_base, at)?,
            function_len: compact(ast.functions.len(), at)?,
            record_base: compact(record_base, at)?,
            record_len: compact(ast.records.len(), at)?,
            original_start: compact(original_base, at)?,
            original_len: compact(original_len, at)?,
            import_start: compact(import_base, at)?,
            import_len: compact(ast.imports.len(), at)?,
            domain: NONE,
            restrictor: NONE,
            child_start: compact(child_base, at)?,
            child_len: compact(ast.modules.len(), at)?,
        };
        function_base += ast.functions.len();
        record_base += ast.records.len();
        original_base += original_len;
        import_base += ast.imports.len();
        child_base += ast.modules.len();
    }
    if (
        function_base,
        record_base,
        original_base,
        import_base,
        child_base,
    ) != (
        c.functions as usize,
        c.records as usize,
        c.originals as usize,
        c.imports as usize,
        c.modules as usize - 1,
    ) {
        return Err(bad(at));
    }
    let mut stack = [0u32; 33];
    let mut stack_len = 0usize;
    for m in 0..sources.count() {
        work.debit(1, at, "module ancestry")?;
        let row = tables.modules[m];
        if m > 0 {
            let parent = row.parent as usize;
            while stack_len > 0 {
                work.debit(1, at, "module parent edge")?;
                if stack[stack_len - 1] as usize == parent {
                    break;
                }
                work.debit(1, at, "module subtree close")?;
                stack_len -= 1;
                tables.modules[stack[stack_len] as usize].subtree_end = m as u32;
            }
            if stack_len == 0 || stack_len > 32 {
                return Err(bad(at));
            }
            if let Some(p) = sources.as_project() {
                if p.modules()[m].depth != stack_len {
                    return Err(bad(at));
                }
            }
            let cursor = scratch.next_child[parent] as usize;
            let parent_row = tables.modules[parent];
            if cursor >= parent_row.child_len as usize {
                return Err(bad(at));
            }
            let expected = sources.ast(ModuleId(parent))?.modules[cursor].name;
            if let Some(p) = sources.as_project() {
                if p.modules()[m].declaration != Some(expected) {
                    return Err(bad(at));
                }
            }
            tables.children[parent_row.child_start as usize + cursor] = m as u32;
            scratch.next_child[parent] += 1;
        }
        if stack_len >= stack.len() {
            return Err(bad(at));
        }
        stack[stack_len] = m as u32;
        stack_len += 1;
    }
    for (m, row) in tables.modules.iter().enumerate() {
        work.debit(1, at, "child cursor validation")?;
        if scratch.next_child[m] != row.child_len {
            return Err(bad(at));
        }
    }
    let mut field_count = 0usize;
    for m in 0..sources.count() {
        let module = tables.modules[m];
        let ast = sources.ast(ModuleId(m))?;
        let file = compact(sources.file(ModuleId(m))?.span(0, 0).file.0, at)?;
        let mut original = module.original_start as usize;
        for item in &ast.items {
            work.debit(1, at, "declaration fill")?;
            let (name, public, kind, target) = match *item {
                ast::ItemId::Function(local) => {
                    let f = &ast.functions[local];
                    let id = module.function_base as usize + local;
                    tables.functions[id] = FunctionRow {
                        file,
                        local_function: compact(local, at)?,
                        original: compact(original, at)?,
                    };
                    (f.name, f.public, FUNCTION, id)
                }
                ast::ItemId::Struct(local) => {
                    let r = &ast.records[local];
                    let id = module.record_base as usize + local;
                    tables.records[id] = RecordRow {
                        file,
                        local_record: compact(local, at)?,
                        original: compact(original, at)?,
                        field_start: compact(field_count, at)?,
                        field_len: compact(r.fields.len(), at)?,
                        first_private: NONE,
                        construction_domain: NONE,
                    };
                    for field in &r.fields {
                        work.debit(1, field.name, "field fill")?;
                        tables.fields[field_count] = FieldRow {
                            name: CompactSpan::new(field.name)?,
                            domain: NONE,
                        };
                        field_count += 1;
                    }
                    (r.name, r.public, RECORD, id)
                }
                ast::ItemId::Module(local) => {
                    let d = ast.modules[local];
                    let child = tables.children[module.child_start as usize + local] as usize;
                    tables.modules[child].original = compact(original, at)?;
                    (d.name, d.public, MODULE, child)
                }
                ast::ItemId::Import(local) => {
                    let id = module.import_start as usize + local;
                    tables.imports[id] = ImportRow {
                        module: m as u32,
                        file,
                        local_import: compact(local, at)?,
                        alias_group: compact(id, at)?,
                        target_group: compact(id, at)?,
                    };
                    tables.alias_order[id] = id as u32;
                    scratch.targets[id] = id as u32;
                    continue;
                }
            };
            tables.originals[original] = OriginalRow {
                name: CompactSpan::new(name)?,
                owner: m as u32,
                target: compact(target, at)?,
                flags: kind | if public.is_some() { PUBLIC } else { 0 },
                domain: NONE,
                restrictor: NONE,
                conflict: NONE,
            };
            #[cfg(test)]
            if kind != MODULE {
                work.observe(Observation::Original {
                    kind: if kind == FUNCTION {
                        "function"
                    } else {
                        "record"
                    },
                    id: target,
                    module: ModuleId(m),
                    name,
                    local: if kind == FUNCTION {
                        tables.functions[target].local_function as usize
                    } else {
                        tables.records[target].local_record as usize
                    },
                });
            }
            tables.original_order[original] = original as u32;
            original += 1;
        }
        if original != (module.original_start + module.original_len) as usize {
            return Err(bad(at));
        }
    }
    if field_count != c.fields as usize {
        return Err(bad(at));
    }
    for m in 1..tables.modules.len() {
        work.debit(1, at, "module domain")?;
        let row = tables.modules[m];
        let parent = tables.modules[row.parent as usize];
        let original = tables.originals[row.original as usize];
        let own = if original.flags & PUBLIC != 0 {
            NONE
        } else {
            row.parent
        };
        let (domain, restrictor) = intersect(parent.domain, parent.restrictor, own, row.original);
        tables.modules[m].domain = domain;
        tables.modules[m].restrictor = restrictor;
    }
    for original in &mut tables.originals {
        work.debit(1, original.name.span(), "original domain")?;
        let module = tables.modules[original.owner as usize];
        let id = if original.flags & 3 == MODULE {
            tables.modules[original.target as usize].original
        } else {
            NONE
        };
        let (domain, restrictor) = if id != NONE {
            let child = tables.modules[original.target as usize];
            (child.domain, child.restrictor)
        } else {
            intersect(
                module.domain,
                module.restrictor,
                if original.flags & PUBLIC != 0 {
                    NONE
                } else {
                    original.owner
                },
                NONE,
            )
        };
        original.domain = domain;
        original.restrictor = restrictor;
    }
    // Assign own-item restriction witnesses only after the domain pass.
    for (id, row) in tables.originals.iter_mut().enumerate() {
        work.debit(1, row.name.span(), "restriction witness")?;
        if row.domain != NONE && row.restrictor == NONE {
            row.restrictor = id as u32;
        }
    }
    for r in 0..tables.records.len() {
        work.debit(1, at, "record construction domain")?;
        let row = tables.records[r];
        let original = tables.originals[row.original as usize];
        let ast = sources.ast(ModuleId(original.owner as usize))?;
        tables.records[r].construction_domain = original.domain;
        for (local, field) in ast.records[row.local_record as usize]
            .fields
            .iter()
            .enumerate()
        {
            work.debit(1, field.name, "field domain")?;
            let domain = if field.public.is_some() {
                original.domain
            } else {
                intersect(
                    original.domain,
                    original.restrictor,
                    original.owner,
                    row.original,
                )
                .0
            };
            tables.fields[row.field_start as usize + local].domain = domain;
            if field.public.is_none() {
                if tables.records[r].first_private == NONE {
                    tables.records[r].first_private = local as u32;
                }
                tables.records[r].construction_domain = domain;
            }
        }
    }
    let mut order = std::mem::take(&mut tables.original_order);
    merge_sort(
        &mut order,
        &mut scratch.merge,
        |a, b| tables.original_cmp(a, b, work, at),
        work,
        at,
    )?;
    tables.original_order = order;
    let mut first = NONE;
    for position in 0..tables.original_order.len() {
        work.debit(1, at, "original grouping")?;
        let id = tables.original_order[position];
        let row = tables.originals[id as usize];
        if position == 0
            || tables.original_cmp(
                tables.original_order[position - 1],
                id,
                work,
                row.name.span(),
            )? != Ordering::Equal
        {
            first = id;
        }
        let spelling = sources.text(row.name.span())?;
        if row.flags & 3 != FUNCTION
            && (compare_bytes(spelling, "bool", work, row.name.span())? == Ordering::Equal
                || compare_bytes(spelling, "i32", work, row.name.span())? == Ordering::Equal)
        {
            tables.originals[id as usize].conflict = BUILTIN_CONFLICT;
        } else if first != id {
            tables.originals[id as usize].conflict = first;
        }
        if row.owner == 0
            && row.flags & 3 == FUNCTION
            && compare_bytes(spelling, "main", work, row.name.span())? == Ordering::Equal
            && tables.root_main == NONE
        {
            tables.root_main = row.target;
        }
    }
    Ok(DeclarationFacts {
        tables,
        scratch,
        plan,
    })
}

fn intersect(parent: u32, witness: u32, own: u32, own_witness: u32) -> (u32, u32) {
    // Both domains are from one declaration path. Larger DFS identity is the
    // narrower domain; equality preserves the first ancestor restriction.
    if parent == NONE {
        (own, if own == NONE { NONE } else { own_witness })
    } else if own == NONE || own <= parent {
        (parent, witness)
    } else {
        (own, own_witness)
    }
}

impl<'s> DeclarationFacts<'s> {
    pub fn plan(&self) -> IndexPlan {
        self.plan
    }
    pub fn sources(&self) -> SourceOwner<'s> {
        self.tables.sources
    }
    pub fn original_count(&self) -> usize {
        self.tables.originals.len()
    }
    pub fn function_count(&self) -> usize {
        self.tables.functions.len()
    }
    pub fn record_count(&self) -> usize {
        self.tables.records.len()
    }
    pub fn function_original(&self, id: DefId) -> Result<usize, Box<Diagnostic>> {
        self.tables
            .functions
            .get(id.0)
            .map(|r| r.original as usize)
            .ok_or_else(|| bad(self.tables.sources.eof()))
    }
    pub fn function(&self, id: DefId) -> Result<(FunctionAstKey, ModuleId), Box<Diagnostic>> {
        function(&self.tables, id)
    }
    pub fn record(&self, id: RecordId) -> Result<(RecordAstKey, ModuleId), Box<Diagnostic>> {
        record(&self.tables, id)
    }
    pub fn conflict(
        &self,
        id: usize,
        work: &WorkMeter,
    ) -> Result<Option<Box<Diagnostic>>, Box<Diagnostic>> {
        let row = self
            .tables
            .originals
            .get(id)
            .ok_or_else(|| bad(self.tables.sources.eof()))?;
        work.debit(1, row.name.span(), "conflict replay")?;
        Ok(match row.conflict {
            NONE => None,
            BUILTIN_CONFLICT => Some(diagnostic(
                "E0202",
                "resolve",
                "scalar type names cannot be redeclared",
                row.name.span(),
            )),
            first => Some(duplicate(
                row.name.span(),
                self.tables.original(first)?.name.span(),
            )),
        })
    }
    pub fn signature_view<'i>(&'i self, work: &'i WorkMeter) -> QuerySession<'i, 's> {
        QuerySession {
            tables: &self.tables,
            work,
        }
    }
    pub fn finish(
        self,
        work: &WorkMeter,
        _allocator: &mut Allocator,
    ) -> Result<DeclarationIndex<'s>, Vec<Diagnostic>> {
        work.phase("original-conflicts");
        let mut errors = Vec::new();
        for i in 0..self.original_count() {
            match self.conflict(i, work) {
                Ok(Some(error)) => {
                    work.record_error(&error);
                    errors.push(*error)
                }
                Ok(None) => (),
                Err(error) => return Err(vec![*error]),
            }
            if errors.len() >= super::super::parser::MAX_DIAGNOSTICS {
                break;
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        CleanOriginals { facts: self }.freeze(work)
    }
}

impl<'s> CleanOriginals<'s> {
    fn freeze(self, work: &WorkMeter) -> Result<DeclarationIndex<'s>, Vec<Diagnostic>> {
        work.phase("imports");
        let DeclarationFacts {
            mut tables,
            mut scratch,
            ..
        } = self.facts;
        let at = tables.sources.eof();
        let grouping = (|| {
            let mut aliases = std::mem::take(&mut tables.alias_order);
            merge_sort(
                &mut aliases,
                &mut scratch.merge,
                |a, b| tables.alias_cmp(a, b, work, at),
                work,
                at,
            )?;
            tables.alias_order = aliases;
            merge_sort(
                &mut scratch.targets,
                &mut scratch.merge,
                |a, b| tables.target_cmp(a, b, work, at),
                work,
                at,
            )?;
            let (mut alias_group, mut target_group) = (NONE, NONE);
            for position in 0..tables.imports.len() {
                work.debit(2, at, "import syntactic grouping")?;
                let a = tables.alias_order[position];
                let t = scratch.targets[position];
                if position == 0
                    || tables.alias_cmp(tables.alias_order[position - 1], a, work, at)?
                        != Ordering::Equal
                {
                    alias_group = a;
                }
                if position == 0
                    || tables.target_cmp(scratch.targets[position - 1], t, work, at)?
                        != Ordering::Equal
                {
                    target_group = t;
                }
                tables.imports[a as usize].alias_group = alias_group;
                tables.imports[t as usize].target_group = target_group;
            }
            Ok::<_, Box<Diagnostic>>(())
        })();
        if let Err(error) = grouping {
            return Err(vec![*error]);
        }
        let mut errors = Vec::new();
        for i in 0..tables.imports.len() {
            match stage_import(&tables, &scratch.seen, i as u32, work) {
                Ok(txn) => {
                    // All fallible work completed. No allocation, query or debit
                    // occurs between the paired alias/seen writes.
                    let cell = &mut tables.aliases[txn.alias];
                    let seen = &mut scratch.seen[txn.seen];
                    if txn.ty != NONE {
                        cell.type_target = txn.ty;
                        cell.type_first_import = txn.import;
                        seen.type_first_import = txn.import;
                    }
                    if txn.value != NONE {
                        cell.value_target = txn.value;
                        cell.value_first_import = txn.import;
                        seen.value_first_import = txn.import;
                    }
                    #[cfg(test)]
                    observe_import(&tables, &scratch.seen, i, true, work);
                }
                Err(error) => {
                    #[cfg(test)]
                    observe_import(&tables, &scratch.seen, i, false, work);
                    let terminal = matches!(error.code, "E0400" | "E0500");
                    work.record_error(&error);
                    errors.push(*error);
                    if terminal {
                        break;
                    }
                }
            }
            if errors.len() >= super::super::parser::MAX_DIAGNOSTICS {
                break;
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        for row in &tables.originals {
            work.debit(1, row.name.span(), "freeze validation")
                .map_err(|e| vec![*e])?;
            if row.conflict != NONE {
                return Err(vec![*bad(at)]);
            }
        }
        #[cfg(test)]
        work.observe(Observation::Frozen {
            root_main: (tables.root_main != NONE).then_some(DefId(tables.root_main as usize)),
        });
        Ok(DeclarationIndex { tables })
    }
}

fn stage_import(
    tables: &Tables<'_>,
    seen: &[SeenCell],
    id: u32,
    work: &WorkMeter,
) -> Result<ImportTxn, Box<Diagnostic>> {
    let row = tables
        .imports
        .get(id as usize)
        .ok_or_else(|| bad(tables.sources.eof()))?;
    let import = tables.import(id)?;
    let at = import.alias;
    let requester = ModuleId(row.module as usize);
    work.debit(1, at, "import transaction")?;
    let (ty, value) = tables.absolute_endpoint(
        requester,
        ItemPathRef {
            file: SourceFileId(row.file as usize),
            path: ast::ItemPath::Absolute(import.path),
        },
        work,
        true,
    )?;
    let endpoint = *tables
        .sources
        .segments(ItemPathRef {
            file: import.span.file,
            path: ast::ItemPath::Absolute(import.path),
        })?
        .last()
        .ok_or_else(|| bad(at))?;
    for target in [ty, value].into_iter().flatten() {
        tables.access_original(target, requester, work, endpoint)?;
    }
    let seen = seen.get(row.target_group as usize).ok_or_else(|| bad(at))?;
    for (target, previous) in [
        (ty, seen.type_first_import),
        (value, seen.value_first_import),
    ] {
        work.debit(1, at, "import repeated target")?;
        if target.is_some() && previous != NONE {
            return Err(duplicate(at, tables.import(previous)?.alias));
        }
    }
    let cell = tables
        .aliases
        .get(row.alias_group as usize)
        .ok_or_else(|| bad(at))?;
    for (type_lane, target, previous) in [
        (true, ty, cell.type_first_import),
        (false, value, cell.value_first_import),
    ] {
        if target.is_none() {
            continue;
        }
        if type_lane {
            let spelling = tables.sources.text(at)?;
            if compare_bytes(spelling, "bool", work, at)? == Ordering::Equal
                || compare_bytes(spelling, "i32", work, at)? == Ordering::Equal
            {
                return Err(diagnostic(
                    "E0202",
                    "resolve",
                    "scalar type names cannot be redeclared",
                    at,
                ));
            }
        }
        if let Some(original) = tables.lookup_original(requester, type_lane, at, work)? {
            return Err(duplicate(at, tables.original(original)?.name.span()));
        }
        if previous != NONE {
            return Err(duplicate(at, tables.import(previous)?.alias));
        }
    }
    work.debit(1, at, "import staging")?;
    Ok(ImportTxn {
        alias: row.alias_group as usize,
        seen: row.target_group as usize,
        import: id,
        ty: ty.map_or(NONE, |id| tables.originals[id as usize].target),
        value: value.map_or(NONE, |id| tables.originals[id as usize].target),
    })
}

fn function(tables: &Tables<'_>, id: DefId) -> Result<(FunctionAstKey, ModuleId), Box<Diagnostic>> {
    let row = tables
        .functions
        .get(id.0)
        .ok_or_else(|| bad(tables.sources.eof()))?;
    let owner = ModuleId(tables.original(row.original)?.owner as usize);
    let key = FunctionAstKey {
        file: SourceFileId(row.file as usize),
        index: row.local_function as usize,
    };
    if tables.sources.module_for_file(key.file)? != owner
        || tables
            .sources
            .ast(owner)?
            .functions
            .get(key.index)
            .is_none()
    {
        return Err(bad(tables.sources.eof()));
    }
    Ok((key, owner))
}
fn record(tables: &Tables<'_>, id: RecordId) -> Result<(RecordAstKey, ModuleId), Box<Diagnostic>> {
    let row = tables
        .records
        .get(id.0)
        .ok_or_else(|| bad(tables.sources.eof()))?;
    let owner = ModuleId(tables.original(row.original)?.owner as usize);
    let key = RecordAstKey {
        file: SourceFileId(row.file as usize),
        index: row.local_record as usize,
    };
    if tables.sources.module_for_file(key.file)? != owner
        || tables.sources.ast(owner)?.records.get(key.index).is_none()
    {
        return Err(bad(tables.sources.eof()));
    }
    Ok((key, owner))
}
impl<'s> DeclarationIndex<'s> {
    pub fn row_lengths(&self) -> [usize; 10] {
        [
            self.tables.originals.len(),
            self.tables.original_order.len(),
            self.tables.functions.len(),
            self.tables.records.len(),
            self.tables.fields.len(),
            self.tables.modules.len(),
            self.tables.children.len(),
            self.tables.imports.len(),
            self.tables.aliases.len(),
            self.tables.alias_order.len(),
        ]
    }
    pub fn sources(&self) -> SourceOwner<'s> {
        self.tables.sources
    }
    pub fn function_count(&self) -> usize {
        self.tables.functions.len()
    }
    pub fn record_count(&self) -> usize {
        self.tables.records.len()
    }
    pub fn function(&self, id: DefId) -> Result<(FunctionAstKey, ModuleId), Box<Diagnostic>> {
        function(&self.tables, id)
    }
    pub fn record(&self, id: RecordId) -> Result<(RecordAstKey, ModuleId), Box<Diagnostic>> {
        record(&self.tables, id)
    }
    pub fn def_for(&self, key: FunctionAstKey) -> Result<DefId, Box<Diagnostic>> {
        let module = self.tables.sources.module_for_file(key.file)?;
        let row = self.tables.modules[module.0];
        if key.index >= row.function_len as usize {
            return Err(bad(self.tables.sources.eof()));
        }
        let id = DefId(row.function_base as usize + key.index);
        if self.function(id)?.0 != key {
            return Err(bad(self.tables.sources.eof()));
        }
        Ok(id)
    }
    pub fn record_for(&self, key: RecordAstKey) -> Result<RecordId, Box<Diagnostic>> {
        let module = self.tables.sources.module_for_file(key.file)?;
        let row = self.tables.modules[module.0];
        if key.index >= row.record_len as usize {
            return Err(bad(self.tables.sources.eof()));
        }
        let id = RecordId(row.record_base as usize + key.index);
        if self.record(id)?.0 != key {
            return Err(bad(self.tables.sources.eof()));
        }
        Ok(id)
    }
    pub fn root_original_main(&self) -> Option<DefId> {
        (self.tables.root_main != NONE).then_some(DefId(self.tables.root_main as usize))
    }
    pub fn query<'i>(&'i self, work: &'i WorkMeter) -> QuerySession<'i, 's> {
        QuerySession {
            tables: &self.tables,
            work,
        }
    }
}

struct RecordFieldCounts<'s> {
    sources: SourceOwner<'s>,
    module: usize,
    local: usize,
    remaining: usize,
}
impl Iterator for RecordFieldCounts<'_> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        while self.module < self.sources.count() {
            let ast = self.sources.ast(ModuleId(self.module)).ok()?;
            if let Some(record) = ast.records.get(self.local) {
                self.local += 1;
                self.remaining -= 1;
                return Some(record.fields.len());
            }
            self.module += 1;
            self.local = 0;
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for RecordFieldCounts<'_> {}

#[cfg(test)]
fn observe_import(
    tables: &Tables<'_>,
    seen: &[SeenCell],
    id: usize,
    committed: bool,
    work: &WorkMeter,
) {
    if !work.observing() {
        return;
    }
    let row = tables.imports[id];
    let import = tables.import(id as u32).expect("checked import");
    let cell = tables.aliases[row.alias_group as usize];
    let aliases = tables
        .imports
        .iter()
        .enumerate()
        .filter(|(i, r)| r.alias_group as usize == *i)
        .map(|(i, r)| {
            let c = tables.aliases[i];
            AliasObservation {
                module: ModuleId(r.module as usize),
                alias: tables.import(i as u32).expect("checked alias").alias,
                ty: (c.type_target != NONE).then_some(RecordId(c.type_target as usize)),
                value: (c.value_target != NONE).then_some(DefId(c.value_target as usize)),
                type_first: (c.type_first_import != NONE).then_some(c.type_first_import as usize),
                value_first: (c.value_first_import != NONE)
                    .then_some(c.value_first_import as usize),
            }
        })
        .collect();
    let seen = tables
        .imports
        .iter()
        .enumerate()
        .filter(|(i, r)| r.target_group as usize == *i)
        .map(|(i, r)| {
            let c = seen[i];
            SeenObservation {
                module: ModuleId(r.module as usize),
                group: i,
                type_first: (c.type_first_import != NONE).then_some(c.type_first_import as usize),
                value_first: (c.value_first_import != NONE)
                    .then_some(c.value_first_import as usize),
            }
        })
        .collect();
    work.observe(Observation::Import {
        id,
        module: ModuleId(row.module as usize),
        alias: import.alias,
        committed,
        ty: (committed && cell.type_first_import == id as u32)
            .then_some(RecordId(cell.type_target as usize)),
        value: (committed && cell.value_first_import == id as u32)
            .then_some(DefId(cell.value_target as usize)),
        aliases,
        seen,
    });
}
