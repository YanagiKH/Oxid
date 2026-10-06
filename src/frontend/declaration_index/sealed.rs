//! Constructor authority. Sibling consumers cannot manufacture or replace these
//! associations, including descendants of the outer declaration_index module.
use super::*;

/// Constructor and anchor updates remain sealed. Borrowed endpoints belong to
/// the same immutable source owner as Tables; moves never invalidate them.
/// The constructor installs each function's dependency before its function.
#[derive(Debug)]
pub(super) struct BuiltinAdmission<'s> {
    read_status: Option<&'s Span>,
    read_stdin: Option<&'s Span>,
    write_status: Option<&'s Span>,
    write_stdout: Option<&'s Span>,
}
impl<'s> BuiltinAdmission<'s> {
    fn none() -> Self {
        Self {
            read_status: None,
            read_stdin: None,
            write_status: None,
            write_stdout: None,
        }
    }
    pub(super) fn set(&self) -> BuiltinSet {
        match (
            self.read_status.is_some(),
            self.read_stdin.is_some(),
            self.write_status.is_some(),
            self.write_stdout.is_some(),
        ) {
            (false, false, false, false) => BuiltinSet::None,
            (true, false, false, false) => BuiltinSet::ReadStatus,
            (true, true, false, false) => BuiltinSet::ReadStdin,
            (false, false, true, false) => BuiltinSet::WriteStatus,
            (false, false, true, true) => BuiltinSet::WriteStdout,
            (true, false, true, false) => BuiltinSet::ReadStatusWriteStatus,
            (true, false, true, true) => BuiltinSet::ReadStatusWriteStdout,
            (true, true, true, false) => BuiltinSet::ReadStdinWriteStatus,
            (true, true, true, true) => BuiltinSet::ReadStdinWriteStdout,
            _ => unreachable!("sealed function dependency"),
        }
    }
    fn admit(&mut self, item: BuiltinItem, anchor: &'s Span) {
        // Inventory is module preorder, then source order. First anchors are minima.
        match item {
            BuiltinItem::Enum(BuiltinEnum::ReadStatus) => {
                if self.read_status.is_none() {
                    self.read_status = Some(anchor);
                }
            }
            BuiltinItem::Function(BuiltinFunction::ReadStdin) => {
                if self.read_status.is_none() {
                    self.read_status = Some(anchor);
                }
                if self.read_stdin.is_none() {
                    self.read_stdin = Some(anchor);
                }
            }
            BuiltinItem::Enum(BuiltinEnum::WriteStatus) => {
                if self.write_status.is_none() {
                    self.write_status = Some(anchor);
                }
            }
            BuiltinItem::Function(BuiltinFunction::WriteStdout) => {
                if self.write_status.is_none() {
                    self.write_status = Some(anchor);
                }
                if self.write_stdout.is_none() {
                    self.write_stdout = Some(anchor);
                }
            }
        }
    }
    pub(super) fn enum_anchor(&self, item: BuiltinEnum) -> Option<Span> {
        match item {
            BuiltinEnum::ReadStatus => self.read_status.copied(),
            BuiltinEnum::WriteStatus => self.write_status.copied(),
        }
    }
    pub(super) fn function_anchor(&self, item: BuiltinFunction) -> Option<Span> {
        match item {
            BuiltinFunction::ReadStdin => self.read_stdin.copied(),
            BuiltinFunction::WriteStdout => self.write_stdout.copied(),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) enum CandidateOrigin {
    Current,
    Enum(CompactSpan),
    Builtin(CompactSpan),
    #[cfg(test)]
    Output(CompactSpan),
}
impl CandidateOrigin {
    pub(super) fn span(self) -> Option<Span> {
        match self {
            Self::Current => None,
            Self::Enum(at) | Self::Builtin(at) => Some(at.span()),
            #[cfg(test)]
            Self::Output(at) => Some(at.span()),
        }
    }
    pub(super) fn is_builtin(self) -> bool {
        matches!(self, Self::Builtin(_))
    }
}

fn builtin_endpoint(
    sources: SourceOwner<'_>,
    path: QualifiedPathRef,
    work: &WorkMeter,
) -> Result<BuiltinItem, Box<Diagnostic>> {
    let view = sources.import_path(path)?;
    let segments = view.segments();
    let at = view.span();
    if view.root() != ast::PathRoot::Std
        || segments.len() != 3
        || compare_bytes(sources.text(segments[1])?, "io", work, at)? != Ordering::Equal
    {
        return Err(diagnostic(
            "E0205",
            "resolve",
            "unsupported standard library import",
            at,
        ));
    }
    let endpoint = sources.text(segments[2])?;
    // Keep input's comparison/work order exactly. The private successor adds
    // two finite branches, not an unaccounted family iterator.
    if compare_bytes(endpoint, BuiltinEnum::ReadStatus.name(), work, segments[2])?
        == Ordering::Equal
    {
        return Ok(BuiltinItem::Enum(BuiltinEnum::ReadStatus));
    }
    if compare_bytes(
        endpoint,
        BuiltinFunction::ReadStdin.name(),
        work,
        segments[2],
    )? == Ordering::Equal
    {
        return Ok(BuiltinItem::Function(BuiltinFunction::ReadStdin));
    }
    Err(diagnostic(
        "E0205",
        "resolve",
        "unsupported standard library import",
        segments[2],
    ))
}

#[cfg(test)]
fn output_candidate_endpoint(
    sources: SourceOwner<'_>,
    path: QualifiedPathRef,
    work: &WorkMeter,
) -> Result<BuiltinItem, Box<Diagnostic>> {
    let view = sources.import_path(path)?;
    let segments = view.segments();
    let at = view.span();
    if view.root() != ast::PathRoot::Std
        || segments.len() != 3
        || compare_bytes(sources.text(segments[1])?, "io", work, at)? != Ordering::Equal
    {
        return Err(diagnostic(
            "E0205",
            "resolve",
            "unsupported standard library import",
            at,
        ));
    }
    let endpoint = sources.text(segments[2])?;
    if compare_bytes(endpoint, BuiltinEnum::ReadStatus.name(), work, segments[2])?
        == Ordering::Equal
    {
        return Ok(BuiltinItem::Enum(BuiltinEnum::ReadStatus));
    }
    if compare_bytes(
        endpoint,
        BuiltinFunction::ReadStdin.name(),
        work,
        segments[2],
    )? == Ordering::Equal
    {
        return Ok(BuiltinItem::Function(BuiltinFunction::ReadStdin));
    }
    if compare_bytes(endpoint, BuiltinEnum::WriteStatus.name(), work, segments[2])?
        == Ordering::Equal
    {
        return Ok(BuiltinItem::Enum(BuiltinEnum::WriteStatus));
    }
    if compare_bytes(
        endpoint,
        BuiltinFunction::WriteStdout.name(),
        work,
        segments[2],
    )? == Ordering::Equal
    {
        return Ok(BuiltinItem::Function(BuiltinFunction::WriteStdout));
    }
    Err(diagnostic(
        "E0205",
        "resolve",
        "unsupported standard library import",
        segments[2],
    ))
}

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
    collect(sources, limits, work, allocator, CollectionSyntax::Enabled)
}
/// Preserve the historical closed policy for negative qualification controls.
/// This never admits enum syntax or clears a private candidate origin.
#[cfg(test)]
pub(in crate::frontend) fn collect_closed<'s>(
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<DeclarationFacts<'s>, Box<Diagnostic>> {
    collect(sources, limits, work, allocator, CollectionSyntax::Closed)
}

/// Typed declarations with the historical closed standard-import policy.
#[cfg(test)]
pub(in crate::frontend) fn collect_std_closed<'s>(
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<DeclarationFacts<'s>, Box<Diagnostic>> {
    collect(
        sources,
        limits,
        work,
        allocator,
        CollectionSyntax::StdClosed,
    )
}

#[cfg(test)]
pub(in crate::frontend) fn collect_enum_candidate<'s>(
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<DeclarationFacts<'s>, Box<Diagnostic>> {
    collect(
        sources,
        limits,
        work,
        allocator,
        CollectionSyntax::EnumCandidate,
    )
}
#[cfg(test)]
pub(in crate::frontend) fn collect_builtin_candidate<'s>(
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<DeclarationFacts<'s>, Box<Diagnostic>> {
    collect(
        sources,
        limits,
        work,
        allocator,
        CollectionSyntax::BuiltinCandidate,
    )
}
/// Private identity-only successor. No executable consumer accepts this marker.
#[cfg(test)]
pub(in crate::frontend) fn collect_output_candidate<'s>(
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<DeclarationFacts<'s>, Box<Diagnostic>> {
    collect(
        sources,
        limits,
        work,
        allocator,
        CollectionSyntax::OutputCandidate,
    )
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum CollectionSyntax {
    #[cfg(test)]
    OutputCandidate,
    #[cfg(test)]
    BuiltinCandidate,
    #[cfg(test)]
    StdClosed,
    Closed,
    #[allow(dead_code)]
    Enabled,
    #[cfg(test)]
    EnumCandidate,
}
impl CollectionSyntax {
    fn std_policy(self) -> super::super::parser::StdImportPolicy {
        if self == Self::Enabled {
            return super::super::parser::StdImportPolicy::Enabled;
        }
        #[cfg(test)]
        if self == Self::OutputCandidate {
            return super::super::parser::StdImportPolicy::OutputCandidate;
        }
        #[cfg(test)]
        if self == Self::BuiltinCandidate {
            return super::super::parser::StdImportPolicy::Candidate;
        }
        super::super::parser::StdImportPolicy::Closed
    }
}
fn collect<'s>(
    sources: SourceOwner<'s>,
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
    syntax: CollectionSyntax,
) -> Result<DeclarationFacts<'s>, Box<Diagnostic>> {
    work.restrict(limits.work);
    work.phase("collection");
    let at = sources.eof();
    // The old aggregate declaration envelope keeps its original first position.
    let mut remaining = 0usize;
    // Replace the historical one-byte enum flag with two summary bits. The
    // parsed std bit is reconciled in the counted walk before any allocation.
    let mut source_features = 0u8;
    for module in 0..sources.count() {
        let ast = sources.ast(ModuleId(module))?;
        source_features |=
            u8::from(!ast.enums.is_empty()) | (u8::from(ast.uses_std_imports()) << 1);
        remaining = remaining
            .checked_add(ast.records.len())
            .ok_or_else(|| overflow(at))?;
    }
    let fields = RecordFieldCounts {
        sources,
        module: 0,
        local: 0,
        remaining,
    };
    let declaration_limit = || {
        diagnostic(
            "E0400",
            "resolve",
            "owned declaration resource limit exceeded",
            sources.file(ModuleId(0)).map_or(at, |file| file.span(0, 0)),
        )
    };
    let record_usage = super::super::oir::owned_types::admit_declaration_counts(fields)
        .map_err(|_| declaration_limit())?;
    let mut builtins = BuiltinAdmission::none();
    if syntax.std_policy().enabled()
        && (syntax != CollectionSyntax::Enabled || source_features & 2 != 0)
    {
        for module in 0..sources.count() {
            work.preflight(at)?;
            let ast = sources.ast(ModuleId(module))?;
            let mut previous = NONE;
            for import in &ast.imports {
                work.preflight(import.span)?;
                let path = QualifiedPathRef {
                    file: import.span.file,
                    path: import.path,
                };
                let view = sources.import_path(path)?;
                if view.root() != ast::PathRoot::Std {
                    continue;
                }
                let endpoint = view.segments().last().ok_or_else(|| bad(at))?;
                if previous != NONE && endpoint.start <= previous as usize {
                    return Err(bad(*endpoint));
                }
                let item = {
                    #[cfg(test)]
                    if syntax == CollectionSyntax::OutputCandidate {
                        output_candidate_endpoint(sources, path, work)?
                    } else {
                        builtin_endpoint(sources, path, work)?
                    }
                    #[cfg(not(test))]
                    {
                        builtin_endpoint(sources, path, work)?
                    }
                };
                // Complete file/start/end validation still follows catalog success.
                // Only its validated start enters the narrower reserved-domain cursor.
                let checked = CompactSpan::new(*endpoint)?;
                previous = checked.start;
                builtins.admit(item, endpoint);
            }
        }
    }
    if syntax != CollectionSyntax::Closed
        && (source_features & 1 != 0 || builtins.set().extra_enums() != 0)
    {
        let mut remaining = 0usize;
        for module in 0..sources.count() {
            work.preflight(at)?;
            let ast = sources.ast(ModuleId(module))?;
            remaining = remaining
                .checked_add(ast.enums.len())
                .ok_or_else(|| overflow(at))?;
            for enumeration in &ast.enums {
                work.preflight(enumeration.name)?;
            }
        }
        super::super::oir::owned_types::admit_enum_counts(
            EnumSourceCounts {
                sources,
                module: 0,
                local: 0,
                remaining: remaining
                    .checked_add(builtins.set().extra_enums())
                    .ok_or_else(|| overflow(at))?,
            },
            record_usage,
        )
        .map_err(|_| declaration_limit())?;
    }
    #[allow(unused_mut)]
    let mut candidate_source_origin = CandidateOrigin::Current;
    #[cfg(test)]
    if syntax == CollectionSyntax::BuiltinCandidate {
        candidate_source_origin = CandidateOrigin::Builtin(CompactSpan::new(at)?);
    }
    #[cfg(test)]
    if syntax == CollectionSyntax::OutputCandidate {
        candidate_source_origin = CandidateOrigin::Output(CompactSpan::new(at)?);
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
        let mut enum_syntax = None;
        let mut std_syntax = false;
        let valid = ast.validate_spans_and_ids_counted_with_syntax(
            |span| {
                if let Err(error) = work.preflight(span.unwrap_or(at)) {
                    failure = Some(error);
                    return false;
                }
                span.is_none_or(|span| {
                    span.file == file.span(0, 0).file && file.try_text(span).is_some()
                })
            },
            &mut enum_syntax,
            syntax.std_policy(),
            &mut std_syntax,
        );
        if let Some(error) = failure {
            return Err(error);
        }
        if !valid {
            return Err(bad(at));
        }
        if let Some(span) = enum_syntax {
            if syntax == CollectionSyntax::Closed {
                return Err(diagnostic(
                    "E0101",
                    "resolve",
                    "enum source syntax is unavailable",
                    span,
                ));
            }
            #[cfg(test)]
            if syntax == CollectionSyntax::EnumCandidate && candidate_source_origin.span().is_none()
            {
                candidate_source_origin = CandidateOrigin::Enum(CompactSpan::new(span)?);
            }
        }
        if sources.is_original_adapter() && ast.uses_project_syntax() {
            return Err(bad(at));
        }
        c.functions = add(c.functions, ast.functions.len() as u64, at)?;
        c.records = add(c.records, ast.records.len() as u64, at)?;
        c.enums = add(c.enums, ast.enums.len() as u64, at)?;
        c.imports = add(c.imports, ast.imports.len() as u64, at)?;
        for item in &ast.items {
            work.preflight(at)?;
            let name = match *item {
                ast::ItemId::Function(i) => Some(ast.functions[i].name),
                ast::ItemId::Struct(i) => Some(ast.records[i].name),
                ast::ItemId::Module(i) => Some(ast.modules[i].name),
                ast::ItemId::Import(_) => None,
                ast::ItemId::Enum(i) => Some(ast.enums[i].name),
            };
            if let Some(name) = name {
                c.original_bytes = add(c.original_bytes, sources.text(name)?.len() as u64, at)?;
            }
        }
        for record in &ast.records {
            work.preflight(record.name)?;
            c.fields = add(c.fields, record.fields.len() as u64, at)?;
        }
        for enumeration in &ast.enums {
            work.preflight(enumeration.name)?;
            let variants = enumeration.variants.len() as u64;
            c.variants = add(c.variants, variants, at)?;
            let mut bytes = 0u64;
            for variant in &enumeration.variants {
                work.preflight(variant.name)?;
                bytes = add(bytes, sources.text(variant.name)?.len() as u64, at)?;
            }
            let repetitions = variants
                .checked_sub(1)
                .ok_or_else(|| bad(enumeration.name))?;
            // Pair visit + compare invocation, then a conservative byte bound.
            let duplicate_work = add(
                variants
                    .checked_mul(repetitions)
                    .ok_or_else(|| overflow(at))?,
                bytes.checked_mul(repetitions).ok_or_else(|| overflow(at))?,
                at,
            )?;
            c.variant_duplicate_work = add(c.variant_duplicate_work, duplicate_work, at)?;
        }
        for import in &ast.imports {
            work.preflight(import.alias)?;
            c.alias_bytes = add(c.alias_bytes, sources.text(import.alias)?.len() as u64, at)?;
            for segment in sources
                .import_path(QualifiedPathRef {
                    file: import.span.file,
                    path: import.path,
                })?
                .segments()
            {
                work.preflight(*segment)?;
                c.path_weight = add(
                    c.path_weight,
                    add(sources.text(*segment)?.len() as u64, 2, at)?,
                    at,
                )?;
            }
        }
    }
    c.originals = add(
        add(add(c.functions, c.records, at)?, c.enums, at)?,
        c.modules - 1,
        at,
    )?;
    for count in [
        c.originals,
        c.functions,
        c.records,
        c.fields,
        c.enums,
        c.variants,
        c.modules,
        c.imports,
    ] {
        if count >= u64::from(BUILTIN_CONFLICT) {
            return Err(bad(at));
        }
    }
    // Suffix identities never consume sentinel values, and totals narrow before allocation.
    for (prefix, extra) in [
        (
            c.originals,
            builtins.set().extra_enums() + builtins.set().extra_functions(),
        ),
        (c.functions, builtins.set().extra_functions()),
        (c.enums, builtins.set().extra_enums()),
    ] {
        let total = prefix
            .checked_add(extra as u64)
            .ok_or_else(|| overflow(at))?;
        if total >= u64::from(BUILTIN_CONFLICT) {
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
        enums: allocate_exact(
            c.enums as usize,
            EnumRow::default(),
            allocator,
            "index enums",
            at,
            work,
        )?,
        variants: allocate_exact(
            c.variants as usize,
            VariantRow::default(),
            allocator,
            "index variants",
            at,
            work,
        )?,
        modules: allocate_exact(
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
        candidate_source_origin,
        builtins,
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
    let (
        mut function_base,
        mut record_base,
        mut enum_base,
        mut original_base,
        mut import_base,
        mut child_base,
    ) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
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
        let original_len =
            ast.functions.len() + ast.records.len() + ast.enums.len() + ast.modules.len();
        tables.modules[m] = ModuleRow {
            parent,
            subtree_end: c.modules as u32,
            original: NONE,
            function_base: compact(function_base, at)?,
            function_len: compact(ast.functions.len(), at)?,
            record_base: compact(record_base, at)?,
            record_len: compact(ast.records.len(), at)?,
            enum_base: compact(enum_base, at)?,
            enum_len: compact(ast.enums.len(), at)?,
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
        enum_base += ast.enums.len();
        original_base += original_len;
        import_base += ast.imports.len();
        child_base += ast.modules.len();
    }
    if (
        function_base,
        record_base,
        enum_base,
        original_base,
        import_base,
        child_base,
    ) != (
        c.functions as usize,
        c.records as usize,
        c.enums as usize,
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
    let mut variant_count = 0usize;
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
                ast::ItemId::Enum(local) => {
                    let declaration = &ast.enums[local];
                    let id = module.enum_base as usize + local;
                    tables.enums[id] = EnumRow {
                        file,
                        local_enum: compact(local, at)?,
                        original: compact(original, at)?,
                        variant_start: compact(variant_count, at)?,
                        variant_len: compact(declaration.variants.len(), at)?,
                    };
                    for variant in &declaration.variants {
                        work.debit(1, variant.name, "variant fill")?;
                        tables.variants[variant_count] = VariantRow {
                            name: CompactSpan::new(variant.name)?,
                        };
                        variant_count += 1;
                    }
                    (declaration.name, declaration.public, ENUM, id)
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
                    kind: match kind {
                        FUNCTION => "function",
                        RECORD => "record",
                        ENUM => "enum",
                        _ => unreachable!("nonmodule original"),
                    },
                    id: target,
                    module: ModuleId(m),
                    name,
                    local: match kind {
                        FUNCTION => tables.functions[target].local_function as usize,
                        RECORD => tables.records[target].local_record as usize,
                        ENUM => tables.enums[target].local_enum as usize,
                        _ => unreachable!("nonmodule original"),
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
    if field_count != c.fields as usize || variant_count != c.variants as usize {
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
        let id = if original.flags & KIND_MASK == MODULE {
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
        if row.flags & KIND_MASK != FUNCTION
            && (compare_bytes(spelling, "bool", work, row.name.span())? == Ordering::Equal
                || compare_bytes(spelling, "i32", work, row.name.span())? == Ordering::Equal)
        {
            tables.originals[id as usize].conflict = BUILTIN_CONFLICT;
        } else if first != id {
            tables.originals[id as usize].conflict = first;
        }
        if row.owner == 0
            && row.flags & KIND_MASK == FUNCTION
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
    pub fn require_current_source_pipeline(&self) -> Result<(), Box<Diagnostic>> {
        self.tables.require_current_source_pipeline()
    }
    pub fn enum_count(&self) -> usize {
        self.source_enum_count() + self.builtin_set().extra_enums()
    }
    pub fn source_enum_count(&self) -> usize {
        self.tables.enums.len()
    }
    pub fn builtin_set(&self) -> BuiltinSet {
        self.tables.builtins.set()
    }
    pub fn require_builtin_candidate_pipeline(&self) -> Result<(), Box<Diagnostic>> {
        self.tables.require_builtin_candidate_pipeline()
    }
    pub fn require_no_builtin_candidate(&self) -> Result<(), Box<Diagnostic>> {
        self.tables.require_no_builtin_candidate()
    }
    pub fn enumeration(&self, id: EnumId) -> Result<(EnumAstKey, ModuleId), Box<Diagnostic>> {
        enumeration(&self.tables, id)
    }
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
        self.source_function_count() + self.builtin_set().extra_functions()
    }
    pub fn source_function_count(&self) -> usize {
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
        if !self.tables.enums.is_empty() {
            work.phase("enum-variants");
            for row in &self.tables.enums {
                work.debit(1, at_enum(&self.tables, row), "enum variant declaration")
                    .map_err(|e| vec![*e])?;
                for current in 0..row.variant_len as usize {
                    let name = self.tables.variants[row.variant_start as usize + current]
                        .name
                        .span();
                    work.debit(1, name, "variant uniqueness")
                        .map_err(|e| vec![*e])?;
                    for earlier in 0..current {
                        let previous = self.tables.variants[row.variant_start as usize + earlier]
                            .name
                            .span();
                        work.debit(1, name, "variant duplicate pair")
                            .map_err(|e| vec![*e])?;
                        if compare_bytes(
                            self.tables.sources.text(previous).map_err(|e| vec![*e])?,
                            self.tables.sources.text(name).map_err(|e| vec![*e])?,
                            work,
                            name,
                        )
                        .map_err(|e| vec![*e])?
                            == Ordering::Equal
                        {
                            let error = duplicate(name, previous);
                            work.record_error(&error);
                            errors.push(*error);
                            break;
                        }
                    }
                    if errors.len() >= super::super::parser::MAX_DIAGNOSTICS {
                        return Err(errors);
                    }
                }
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
                    if txn.ty != NO_DECLARATION {
                        cell.type_target = txn.ty;
                        cell.type_first_import = txn.import;
                        seen.type_first_import = txn.import;
                    }
                    if txn.value != NO_DECLARATION {
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
    let path = QualifiedPathRef {
        file: SourceFileId(row.file as usize),
        path: import.path,
    };
    let view = tables.sources.import_path(path)?;
    let endpoint = *view.segments().last().ok_or_else(|| bad(at))?;
    let (ty, value) = if view.root() == ast::PathRoot::Std {
        let item = {
            #[cfg(test)]
            if matches!(tables.candidate_source_origin, CandidateOrigin::Output(_)) {
                output_candidate_endpoint(tables.sources, path, work)?
            } else {
                builtin_endpoint(tables.sources, path, work)?
            }
            #[cfg(not(test))]
            {
                builtin_endpoint(tables.sources, path, work)?
            }
        };
        let handle = tables.builtin_handle(item)?;
        match item {
            BuiltinItem::Enum(_) => (Some(handle), None),
            BuiltinItem::Function(_) => (None, Some(handle)),
        }
    } else {
        tables.absolute_endpoint(
            requester,
            ItemPathRef {
                file: path.file,
                path: ast::ItemPath::Absolute(path.path),
            },
            work,
            true,
        )?
    };
    for target in [ty, value].into_iter().flatten() {
        tables.access_handle(target, requester, work, endpoint)?;
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
        ty: ty.unwrap_or(NO_DECLARATION),
        value: value.unwrap_or(NO_DECLARATION),
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
fn at_enum(tables: &Tables<'_>, row: &EnumRow) -> Span {
    tables.originals[row.original as usize].name.span()
}
fn enumeration(tables: &Tables<'_>, id: EnumId) -> Result<(EnumAstKey, ModuleId), Box<Diagnostic>> {
    let row = tables
        .enums
        .get(id.0)
        .ok_or_else(|| bad(tables.sources.eof()))?;
    if tables.nominal_original(row.original)? != NominalId::Enum(id) {
        return Err(bad(tables.sources.eof()));
    }
    let owner = ModuleId(tables.original(row.original)?.owner as usize);
    let key = EnumAstKey {
        file: SourceFileId(row.file as usize),
        index: row.local_enum as usize,
    };
    let declaration = tables
        .sources
        .ast(owner)?
        .enums
        .get(key.index)
        .ok_or_else(|| bad(tables.sources.eof()))?;
    if tables.sources.module_for_file(key.file)? != owner
        || declaration.name.file != key.file
        || declaration.variants.len() != row.variant_len as usize
    {
        return Err(bad(tables.sources.eof()));
    }
    let end = (row.variant_start as usize)
        .checked_add(row.variant_len as usize)
        .ok_or_else(|| bad(tables.sources.eof()))?;
    if tables
        .variants
        .get(row.variant_start as usize..end)
        .is_none()
    {
        return Err(bad(tables.sources.eof()));
    }
    Ok((key, owner))
}
impl<'s> DeclarationIndex<'s> {
    pub fn require_current_source_pipeline(&self) -> Result<(), Box<Diagnostic>> {
        self.tables.require_current_source_pipeline()
    }
    pub fn enum_count(&self) -> usize {
        self.source_enum_count() + self.builtin_set().extra_enums()
    }
    pub fn source_enum_count(&self) -> usize {
        self.tables.enums.len()
    }
    pub fn builtin_set(&self) -> BuiltinSet {
        self.tables.builtins.set()
    }
    pub fn require_builtin_candidate_pipeline(&self) -> Result<(), Box<Diagnostic>> {
        self.tables.require_builtin_candidate_pipeline()
    }
    pub fn require_no_builtin_candidate(&self) -> Result<(), Box<Diagnostic>> {
        self.tables.require_no_builtin_candidate()
    }
    pub fn enumeration(&self, id: EnumId) -> Result<(EnumAstKey, ModuleId), Box<Diagnostic>> {
        enumeration(&self.tables, id)
    }
    pub fn builtin_enum_id(&self, item: BuiltinEnum) -> Result<EnumId, Box<Diagnostic>> {
        self.tables.builtin_handle(BuiltinItem::Enum(item))?;
        Ok(EnumId(
            self.source_enum_count()
                + match item {
                    BuiltinEnum::ReadStatus => 0,
                    BuiltinEnum::WriteStatus => {
                        usize::from(self.builtin_set().contains_enum(BuiltinEnum::ReadStatus))
                    }
                },
        ))
    }
    pub fn builtin_function_id(&self, item: BuiltinFunction) -> Result<DefId, Box<Diagnostic>> {
        self.tables.builtin_handle(BuiltinItem::Function(item))?;
        Ok(DefId(
            self.source_function_count()
                + match item {
                    BuiltinFunction::ReadStdin => 0,
                    BuiltinFunction::WriteStdout => usize::from(
                        self.builtin_set()
                            .contains_function(BuiltinFunction::ReadStdin),
                    ),
                },
        ))
    }
    pub fn builtin_enum_anchor(&self, item: BuiltinEnum) -> Result<Span, Box<Diagnostic>> {
        self.builtin_enum_id(item)?;
        self.tables
            .builtins
            .enum_anchor(item)
            .ok_or_else(|| bad(self.sources().eof()))
    }
    pub fn builtin_function_anchor(&self, item: BuiltinFunction) -> Result<Span, Box<Diagnostic>> {
        self.builtin_function_id(item)?;
        self.tables
            .builtins
            .function_anchor(item)
            .ok_or_else(|| bad(self.sources().eof()))
    }
    pub fn enum_origin(&self, id: EnumId) -> Result<DeclarationOrigin, Box<Diagnostic>> {
        if id.0 < self.source_enum_count() {
            self.enumeration(id)?;
            return Ok(DeclarationOrigin::Source);
        }
        if self.builtin_set().contains_enum(BuiltinEnum::ReadStatus)
            && id.0 == self.source_enum_count()
        {
            return Ok(DeclarationOrigin::Builtin(BuiltinItem::Enum(
                BuiltinEnum::ReadStatus,
            )));
        }
        if self.builtin_set().contains_enum(BuiltinEnum::WriteStatus)
            && id.0
                == self.source_enum_count()
                    + usize::from(self.builtin_set().contains_enum(BuiltinEnum::ReadStatus))
        {
            return Ok(DeclarationOrigin::Builtin(BuiltinItem::Enum(
                BuiltinEnum::WriteStatus,
            )));
        }
        Err(bad(self.sources().eof()))
    }
    pub fn function_origin(&self, id: DefId) -> Result<DeclarationOrigin, Box<Diagnostic>> {
        if id.0 < self.source_function_count() {
            self.function(id)?;
            return Ok(DeclarationOrigin::Source);
        }
        if self
            .builtin_set()
            .contains_function(BuiltinFunction::ReadStdin)
            && id.0 == self.source_function_count()
        {
            return Ok(DeclarationOrigin::Builtin(BuiltinItem::Function(
                BuiltinFunction::ReadStdin,
            )));
        }
        if self
            .builtin_set()
            .contains_function(BuiltinFunction::WriteStdout)
            && id.0
                == self.source_function_count()
                    + usize::from(
                        self.builtin_set()
                            .contains_function(BuiltinFunction::ReadStdin),
                    )
        {
            return Ok(DeclarationOrigin::Builtin(BuiltinItem::Function(
                BuiltinFunction::WriteStdout,
            )));
        }
        Err(bad(self.sources().eof()))
    }
    // Only checked view constructors use these narrow immutable projections.
    pub(super) fn frozen_enum_syntax(&self, id: EnumId) -> Option<&ast::EnumDecl> {
        self.tables.enums.get(id.0).map(|row| {
            self.tables.sources.frozen_enum(EnumAstKey {
                file: SourceFileId(row.file as usize),
                index: row.local_enum as usize,
            })
        })
    }
    pub(super) fn frozen_enum_origin(&self, id: EnumId) -> DeclarationOrigin {
        if id.0 < self.source_enum_count() {
            DeclarationOrigin::Source
        } else if self.builtin_set().contains_enum(BuiltinEnum::ReadStatus)
            && id.0 == self.source_enum_count()
        {
            DeclarationOrigin::Builtin(BuiltinItem::Enum(BuiltinEnum::ReadStatus))
        } else {
            DeclarationOrigin::Builtin(BuiltinItem::Enum(BuiltinEnum::WriteStatus))
        }
    }
    pub(super) fn frozen_enum_name(&self, id: EnumId) -> &str {
        self.frozen_enum_syntax(id).map_or_else(
            || match self.frozen_enum_origin(id) {
                DeclarationOrigin::Builtin(BuiltinItem::Enum(item)) => item.name(),
                _ => unreachable!("checked builtin enum"),
            },
            |syntax| self.tables.sources.frozen_text(syntax.name),
        )
    }
    pub(super) fn frozen_enum_anchor(&self, id: EnumId) -> Span {
        self.frozen_enum_syntax(id).map_or_else(
            || match self.frozen_enum_origin(id) {
                DeclarationOrigin::Builtin(BuiltinItem::Enum(item)) => self
                    .tables
                    .builtins
                    .enum_anchor(item)
                    .expect("checked builtin enum"),
                _ => unreachable!("checked builtin enum"),
            },
            |syntax| syntax.name,
        )
    }
    pub(super) fn frozen_variant_count(&self, id: EnumId) -> usize {
        self.tables
            .enums
            .get(id.0)
            .map_or(3, |row| row.variant_len as usize)
    }
    pub fn enum_for(&self, key: EnumAstKey) -> Result<EnumId, Box<Diagnostic>> {
        let module = self.tables.sources.module_for_file(key.file)?;
        let row = self.tables.modules[module.0];
        if key.index >= row.enum_len as usize {
            return Err(bad(self.tables.sources.eof()));
        }
        let id = EnumId(row.enum_base as usize + key.index);
        if self.enumeration(id)?.0 != key {
            return Err(bad(self.tables.sources.eof()));
        }
        Ok(id)
    }
    pub fn enum_view(&self, id: EnumId) -> Result<EnumView<'_>, Box<Diagnostic>> {
        EnumView::from_index(self, id)
    }
    pub fn enum_variant_counts(&self) -> EnumVariantCounts<'_> {
        EnumVariantCounts {
            index: self,
            next: 0,
        }
    }
    /// The historical observation shape cannot represent enum rows.
    #[cfg(test)]
    pub fn row_lengths(&self) -> [usize; 10] {
        assert!(
            self.tables.enums.is_empty(),
            "legacy index row observation excludes enum projection"
        );
        let complete = self.complete_row_lengths();
        complete[..10]
            .try_into()
            .expect("ten legacy index row lanes")
    }
    #[cfg(test)]
    pub fn enum_scoped_row_capacities(&self) -> [usize; 3] {
        [
            self.tables.enums.capacity(),
            self.tables.variants.capacity(),
            self.tables.modules.capacity(),
        ]
    }
    pub fn complete_row_lengths(&self) -> [usize; 12] {
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
            self.tables.enums.len(),
            self.tables.variants.len(),
        ]
    }
    pub fn sources(&self) -> SourceOwner<'s> {
        self.tables.sources
    }
    pub fn function_count(&self) -> usize {
        self.source_function_count() + self.builtin_set().extra_functions()
    }
    pub fn source_function_count(&self) -> usize {
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

pub(super) struct EnumSourceCounts<'s> {
    sources: SourceOwner<'s>,
    module: usize,
    local: usize,
    remaining: usize,
}
impl Iterator for EnumSourceCounts<'_> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        while self.module < self.sources.count() {
            let ast = self.sources.ast(ModuleId(self.module)).ok()?;
            if let Some(declaration) = ast.enums.get(self.local) {
                self.local += 1;
                self.remaining -= 1;
                return Some(declaration.variants.len());
            }
            self.module += 1;
            self.local = 0;
        }
        if self.remaining != 0 {
            self.remaining -= 1;
            return Some(3);
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for EnumSourceCounts<'_> {}

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
    // Complete enum identities choose the schema, not candidate AST syntax.
    if !tables.enums.is_empty() || tables.builtins.set().extra_enums() != 0 {
        observe_nominal_import(tables, seen, id, committed, work);
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
                ty: (c.type_target != NO_DECLARATION).then(|| {
                    tables
                        .nominal_handle(c.type_target)
                        .expect("checked alias type")
                        .legacy_record()
                }),
                value: (c.value_target != NO_DECLARATION).then(|| {
                    tables
                        .function_handle(c.value_target)
                        .expect("checked alias function")
                }),
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
        ty: (committed && cell.type_first_import == id as u32).then(|| {
            tables
                .nominal_handle(cell.type_target)
                .expect("checked import type")
                .legacy_record()
        }),
        value: (committed && cell.value_first_import == id as u32).then(|| {
            tables
                .function_handle(cell.value_target)
                .expect("checked import function")
        }),
        aliases,
        seen,
    });
}

#[cfg(test)]
fn observe_nominal_import(
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
            NominalAliasObservation {
                module: ModuleId(r.module as usize),
                alias: tables.import(i as u32).expect("checked alias").alias,
                ty: (c.type_target != NO_DECLARATION).then(|| {
                    tables
                        .nominal_handle(c.type_target)
                        .expect("checked alias type")
                }),
                value: (c.value_target != NO_DECLARATION).then(|| {
                    tables
                        .function_handle(c.value_target)
                        .expect("checked alias function")
                }),
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
    work.observe(Observation::NominalImport {
        id,
        module: ModuleId(row.module as usize),
        alias: import.alias,
        committed,
        ty: (committed && cell.type_first_import == id as u32).then(|| {
            tables
                .nominal_handle(cell.type_target)
                .expect("checked import type")
        }),
        value: (committed && cell.value_first_import == id as u32).then(|| {
            tables
                .function_handle(cell.value_target)
                .expect("checked import function")
        }),
        aliases,
        seen,
    });
}

#[test]
fn bounded_enum_production_collection_policy_layout() {
    println!(
        "ENUM_PRODUCTION_COLLECTION_LAYOUT policy={} index={} facts={}",
        size_of::<CollectionSyntax>(),
        size_of::<DeclarationIndex<'_>>(),
        size_of::<DeclarationFacts<'_>>()
    );
    assert_eq!(size_of::<CollectionSyntax>(), 1);
}

#[test]
fn bounded_enum_public_index_rejects_every_unpaid_scalar_producer() {
    use crate::frontend::{lexer, parser, source::SourceMap};
    let mut sources = SourceMap::new();
    let id = sources.add(
        "public-enum-guard.ox".into(),
        "enum E{N} fn main()->i32{return 0;}".into(),
    );
    let file = sources.get(id);
    let ast = parser::parse_typed_counted(
        file,
        lexer::lex(file).unwrap(),
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut Allocator::default(),
        &mut Default::default(),
    )
    .unwrap()
    .0;
    let owner = SourceOwner::original(file, &ast, SourceView::Map(&sources)).unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let facts = collect_originals(owner, IndexLimits::default(), &work, &mut allocator).unwrap();
    facts.require_current_source_pipeline().unwrap();
    assert_eq!(facts.enum_count(), 1);
    let phase = WorkMeter::default();
    let errors = super::super::hir::original_signatures(&facts, &phase).unwrap_err();
    assert_eq!((errors[0].code, errors[0].stage), ("E0101", "resolve"));
    assert_eq!(phase.used(), 0);
    let index = facts.finish(&work, &mut allocator).unwrap();
    index.require_current_source_pipeline().unwrap();
    for errors in [
        super::super::hir::resolve_project(&index, &phase).unwrap_err(),
        super::super::hir::resolve_bodies(&index, &phase, Vec::new()).unwrap_err(),
    ] {
        assert_eq!((errors[0].code, errors[0].stage), ("E0101", "resolve"));
    }
    assert_eq!(phase.used(), 0);
}

#[cfg(test)]
impl<'s> DeclarationIndex<'s> {
    /// Read-only pointer evidence for the sealed source-owner association.
    pub(super) fn builtin_anchor_borrows_for_test(&self) -> [Option<&'s Span>; 4] {
        [
            self.tables.builtins.read_status,
            self.tables.builtins.read_stdin,
            self.tables.builtins.write_status,
            self.tables.builtins.write_stdout,
        ]
    }
}
