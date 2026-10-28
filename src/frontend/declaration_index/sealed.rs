//! Constructor authority. Sibling consumers cannot manufacture or replace these
//! associations, including descendants of the outer declaration_index module.
use super::*;

/// The sealed collector stores import ordinals, never source-supplied origins.
/// Four legacy anchors retain their first-use ordering; each experimental family
/// shares its first canonical import anchor. The same immutable source owner and
/// checked ImportRow table recover the exact borrowed endpoint on demand.
#[derive(Debug)]
#[repr(C, align(8))]
pub(super) struct BuiltinAdmission<'s> {
    anchors: [u32; 10],
    experimental: super::super::builtin_catalog::experimental::Inventory,
    source: std::marker::PhantomData<&'s Span>,
}
impl<'s> BuiltinAdmission<'s> {
    fn none() -> Self {
        Self {
            anchors: [NONE; 10],
            experimental: Default::default(),
            source: std::marker::PhantomData,
        }
    }
    pub(super) fn set(&self) -> BuiltinSet {
        let present = |index| self.anchors[index] != NONE;
        let legacy = match (present(0), present(1), present(2), present(3)) {
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
        };
        legacy.with_experimental(self.experimental)
    }
    fn admit(&mut self, item: BuiltinItem, import: u32) {
        let slot = match item {
            BuiltinItem::Enum(BuiltinEnum::ReadStatus) => 0,
            BuiltinItem::Function(BuiltinFunction::ReadStdin) => {
                if self.anchors[0] == NONE {
                    self.anchors[0] = import;
                }
                1
            }
            BuiltinItem::Enum(BuiltinEnum::WriteStatus) => 2,
            BuiltinItem::Function(BuiltinFunction::WriteStdout) => {
                if self.anchors[2] == NONE {
                    self.anchors[2] = import;
                }
                3
            }
            BuiltinItem::Experimental(item) => {
                self.experimental = self.experimental.admit(item);
                if let Some(enumeration) = item
                    .function()
                    .and_then(experimental::Function::result_enum)
                {
                    let dependency = 4 + enumeration.item().family();
                    if self.anchors[dependency] == NONE {
                        self.anchors[dependency] = import;
                    }
                }
                4 + item.family()
            }
            BuiltinItem::Vector(item) => {
                self.experimental = self.experimental.admit_vector(item);
                if item
                    .operation()
                    .is_some_and(|operation| operation.requires_grow_status())
                    && self.anchors[4] == NONE
                {
                    self.anchors[4] = import;
                }
                9
            }
        };
        if self.anchors[slot] == NONE {
            self.anchors[slot] = import;
        }
    }
    pub(super) fn enum_anchor(&self, item: BuiltinEnum) -> Option<u32> {
        self.anchor(match item {
            BuiltinEnum::ReadStatus => 0,
            BuiltinEnum::WriteStatus => 2,
        })
    }
    pub(super) fn function_anchor(&self, item: BuiltinFunction) -> Option<u32> {
        self.anchor(match item {
            BuiltinFunction::ReadStdin => 1,
            BuiltinFunction::WriteStdout => 3,
        })
    }
    fn experimental_anchor(&self, item: experimental::Item) -> Option<u32> {
        self.experimental
            .contains(item)
            .then(|| self.anchor(4 + item.family()))
            .flatten()
    }
    fn vector_anchor(&self, item: vector::Item) -> Option<u32> {
        self.experimental
            .contains_vector(item)
            .then(|| self.anchor(9))
            .flatten()
    }
    fn anchor(&self, slot: usize) -> Option<u32> {
        (self.anchors[slot] != NONE).then_some(self.anchors[slot])
    }
}
#[derive(Debug)]
pub(super) enum CandidateOrigin {
    Current,
    CurrentGeneric(Box<[super::generics::Plan; 1]>),
    CurrentRanges,
    CurrentGenericRanges(Box<[super::generics::Plan; 1]>),
    Enum(CompactSpan),
    Builtin(CompactSpan),
    #[cfg(test)]
    Output(CompactSpan),
}
impl CandidateOrigin {
    pub(super) fn span(&self) -> Option<Span> {
        match self {
            Self::Current
            | Self::CurrentGeneric(_)
            | Self::CurrentRanges
            | Self::CurrentGenericRanges(_) => None,
            Self::Enum(at) | Self::Builtin(at) => Some(at.span()),
            #[cfg(test)]
            Self::Output(at) => Some(at.span()),
        }
    }
    pub(super) fn is_builtin(&self) -> bool {
        matches!(self, Self::Builtin(_))
    }
}

#[cfg(test)]
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
        || sources.text(segments[0])? != "std"
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

fn current_builtin_endpoint(
    sources: SourceOwner<'_>,
    path: QualifiedPathRef,
    work: &WorkMeter,
) -> Result<BuiltinItem, Box<Diagnostic>> {
    let view = sources.import_path(path)?;
    let segments = view.segments();
    let at = view.span();
    if view.root() == ast::PathRoot::Std
        && segments.len() == 3
        && (sources.text(segments[0])? == "alloc"
            || (sources.text(segments[0])? == "std"
                && matches!(sources.text(segments[1])?, "fs" | "env")))
    {
        return experimental_endpoint(sources, segments, work, at);
    }
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
    experimental_endpoint(sources, segments, work, at)
}
fn experimental_endpoint(
    sources: SourceOwner<'_>,
    segments: &[Span],
    work: &WorkMeter,
    at: Span,
) -> Result<BuiltinItem, Box<Diagnostic>> {
    if segments.len() == 3 {
        let mut alias = true;
        for (name, segment) in ["alloc", "vec", "GrowStatus"].into_iter().zip(segments) {
            if compare_bytes(sources.text(*segment)?, name, work, *segment)? != Ordering::Equal {
                alias = false;
                break;
            }
        }
        if alias {
            return Ok(BuiltinItem::Experimental(experimental::Item::GrowStatus));
        }
        for item in experimental::Item::ALL {
            let mut matches = true;
            for (name, segment) in item.path().into_iter().zip(segments) {
                if compare_bytes(sources.text(*segment)?, name, work, *segment)? != Ordering::Equal
                {
                    matches = false;
                    break;
                }
            }
            if matches {
                return Ok(BuiltinItem::Experimental(item));
            }
        }
        for item in vector::Item::ALL {
            let mut matches = true;
            for (name, segment) in item.path().into_iter().zip(segments) {
                if compare_bytes(sources.text(*segment)?, name, work, *segment)? != Ordering::Equal
                {
                    matches = false;
                    break;
                }
            }
            if matches {
                return Ok(BuiltinItem::Vector(item));
            }
        }
    }
    Err(diagnostic(
        "E0205",
        "resolve",
        "unsupported standard library import",
        at,
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
    let mut builtin_import = 0u32;
    if syntax.std_policy().enabled()
        && (syntax != CollectionSyntax::Enabled || source_features & 2 != 0)
    {
        for module in 0..sources.count() {
            work.preflight(at)?;
            let ast = sources.ast(ModuleId(module))?;
            let mut previous = NONE;
            for import in &ast.imports {
                let import_ordinal = builtin_import;
                builtin_import = builtin_import
                    .checked_add(1)
                    .ok_or_else(|| overflow(import.span))?;
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
                    if syntax == CollectionSyntax::BuiltinCandidate {
                        builtin_endpoint(sources, path, work)?
                    } else {
                        current_builtin_endpoint(sources, path, work)?
                    }
                    #[cfg(not(test))]
                    {
                        current_builtin_endpoint(sources, path, work)?
                    }
                };
                // Complete file/start/end validation still follows catalog success.
                // Only its validated start enters the narrower reserved-domain cursor.
                let checked = CompactSpan::new(*endpoint)?;
                previous = checked.start;
                builtins.admit(item, import_ordinal);
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
    let mut has_element_ranges = false;
    let mut has_source_slices = false;
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
        let mut features = ast::SyntaxFeatures::default();
        let valid = ast.validate_spans_and_ids_counted_with_features(
            Some(file),
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
            &mut features,
        );
        if let Some(error) = failure {
            return Err(error);
        }
        if !valid {
            return Err(bad(at));
        }
        has_element_ranges |= features.element_ranges;
        has_source_slices |= features.source_slices;
        if ast.has_range_for() && matches!(candidate_source_origin, CandidateOrigin::Current) {
            candidate_source_origin = CandidateOrigin::CurrentRanges;
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
        if sources.is_original_adapter()
            && ast.uses_project_syntax()
            && !ast.has_only_original_generic_syntax()
        {
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
        c.modules
            .checked_sub(sources.package_roots() as u64)
            .ok_or_else(|| bad(at))?,
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
            builtins.set().extra_enums()
                + builtins.set().extra_functions()
                + usize::from(
                    builtins
                        .set()
                        .experimental()
                        .contains(experimental::Item::ByteBuf),
                )
                + usize::from(
                    builtins
                        .set()
                        .experimental()
                        .contains(experimental::Item::File),
                )
                + builtins.set().vector_templates()
                + usize::from(builtins.set().contains_vector(vector::Item::Vec)),
        ),
        (c.functions, builtins.set().extra_template_functions()),
        (c.enums, builtins.set().extra_enums()),
    ] {
        let total = prefix
            .checked_add(extra as u64)
            .ok_or_else(|| overflow(at))?;
        if total >= u64::from(BUILTIN_CONFLICT) {
            return Err(bad(at));
        }
    }
    let syntax_scratch = SYNTAX_FEATURE_CONTROL_BYTES
        .checked_add(if has_source_slices {
            ast::SLICE_SOURCE_CONTROL_BYTES
        } else {
            0
        })
        .ok_or_else(|| overflow(at))?;
    let fixed_scratch = FIXED_SCRATCH
        .checked_add(VECTOR_INDEX_HEADER_GROWTH)
        .and_then(|bytes| {
            bytes.checked_add(super::super::builtin_catalog::builtin_set_control_bytes())
        })
        .and_then(|bytes| bytes.checked_add(syntax_scratch))
        .and_then(|bytes| {
            bytes.checked_add(if builtins.set().experimental().is_empty() {
                0
            } else {
                experimental::compiler_carrier_bytes()
            })
        })
        .ok_or_else(|| overflow(at))?;
    let plan = IndexPlan::calculate(
        c,
        size_of::<DeclarationIndex<'s>>(),
        fixed_scratch,
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
        has_element_ranges,
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
        let package_root = sources.package_root(ModuleId(m))?;
        if (m == package_root.0 && (parent != NONE || depth != 0))
            || (m != package_root.0
                && (parent as usize >= m
                    || depth == 0
                    || depth > 32
                    || sources.package_root(ModuleId(parent as usize))? != package_root))
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
        c.modules as usize - sources.package_roots(),
    ) {
        return Err(bad(at));
    }
    let mut stack = [0u32; 33];
    let mut stack_len = 0usize;
    for m in 0..sources.count() {
        work.debit(1, at, "module ancestry")?;
        let row = tables.modules[m];
        if row.parent == NONE && m != 0 {
            while stack_len > 0 {
                work.debit(1, at, "package subtree close")?;
                stack_len -= 1;
                tables.modules[stack[stack_len] as usize].subtree_end = m as u32;
            }
        }
        if row.parent != NONE {
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
        if tables.modules[m].parent == NONE {
            continue;
        }
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
    #[cfg(test)]
    pub fn is_output_candidate_pipeline(&self) -> bool {
        self.tables.is_output_candidate_pipeline()
    }
    #[cfg(test)]
    pub fn require_output_candidate_pipeline(&self) -> Result<(), Box<Diagnostic>> {
        self.tables.require_output_candidate_pipeline()
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
        allocator: &mut Allocator,
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
        let mut index = CleanOriginals { facts: self }.freeze(work)?;
        // A bare vector call can omit generic syntax altogether. The sealed
        // import inventory is a constant-time activation bit, so new() still
        // reaches the planner's explicit/inferred element checks. Likewise, an
        // immutable parse fact forces full syntax authentication for named
        // slices even if all their mutable AST kinds have been replaced.
        let has_generics = index.builtin_set().contains_vector(vector::Item::Vec)
            || (0..index.sources().count()).any(|module| {
                index.sources().ast(ModuleId(module)).is_ok_and(|program| {
                    program.has_generics()
                        || program.has_parsed_named_slice()
                        || program.has_parsed_copy_records()
                        || program.has_record_patterns()
                })
            });
        if has_generics {
            index
                .require_current_source_pipeline()
                .map_err(|error| vec![*error])?;
            let plan = super::generics::Plan::build(&index, work, allocator)
                .map_err(|error| vec![*error])?;
            let ranges = index.has_range_for();
            let plan = allocator
                .boxed_carrier(plan, "generic instantiation owner")
                .map_err(|_| {
                    vec![*resource(
                        "generic instantiation owner allocation failed",
                        index.sources().eof(),
                    )]
                })?;
            index.tables.candidate_source_origin = if ranges {
                CandidateOrigin::CurrentGenericRanges(plan)
            } else {
                CandidateOrigin::CurrentGeneric(plan)
            };
        }
        Ok(index)
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
        super::u8_reservation::scan(&tables, work, &at).map_err(|e| vec![*e])?;
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
            if matches!(tables.candidate_source_origin, CandidateOrigin::Builtin(_)) {
                builtin_endpoint(tables.sources, path, work)?
            } else {
                current_builtin_endpoint(tables.sources, path, work)?
            }
            #[cfg(not(test))]
            {
                current_builtin_endpoint(tables.sources, path, work)?
            }
        };
        let handle = tables.builtin_handle(item)?;
        match item {
            BuiltinItem::Enum(_) => (Some(handle), None),
            BuiltinItem::Function(_) => (None, Some(handle)),
            BuiltinItem::Vector(item) => {
                if item.operation().is_some() {
                    (None, Some(handle))
                } else {
                    (Some(handle), None)
                }
            }
            BuiltinItem::Experimental(item) => {
                if item.function().is_some() {
                    (None, Some(handle))
                } else {
                    (Some(handle), None)
                }
            }
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
    /// A borrowed predicate over the sealed constructor origin, not authority
    /// to construct or relabel an index. The source admission guard pays this
    /// receiver/result separately from the existing fallible wrapper.
    pub fn is_current_source_pipeline(&self) -> bool {
        matches!(
            self.tables.candidate_source_origin,
            CandidateOrigin::Current
                | CandidateOrigin::CurrentGeneric(_)
                | CandidateOrigin::CurrentRanges
                | CandidateOrigin::CurrentGenericRanges(_)
        )
    }
    /// Independently collected during the counted source argument walk.
    pub fn has_element_ranges(&self) -> bool {
        self.tables.has_element_ranges
    }
    pub fn has_range_for(&self) -> bool {
        matches!(
            self.tables.candidate_source_origin,
            CandidateOrigin::CurrentRanges | CandidateOrigin::CurrentGenericRanges(_)
        )
    }
    #[cfg(test)]
    pub fn is_input_candidate_pipeline(&self) -> bool {
        matches!(
            self.tables.candidate_source_origin,
            CandidateOrigin::Builtin(_)
        )
    }
    pub fn require_current_source_pipeline(&self) -> Result<(), Box<Diagnostic>> {
        self.tables.require_current_source_pipeline()
    }
    pub fn enum_count(&self) -> usize {
        self.source_enum_count() + self.builtin_set().extra_enums()
    }
    pub fn source_enum_count(&self) -> usize {
        self.generic_plan()
            .map_or(self.tables.enums.len(), |plan| plan.enum_count())
    }
    pub(super) fn template_enum_count(&self) -> usize {
        self.tables.enums.len()
    }
    pub(super) fn template_enumeration(
        &self,
        id: EnumId,
    ) -> Result<(EnumAstKey, ModuleId), Box<Diagnostic>> {
        enumeration(&self.tables, id)
    }
    pub(super) fn template_builtin_enum_item(
        &self,
        id: EnumId,
    ) -> Result<BuiltinItem, Box<Diagnostic>> {
        let rank =
            id.0.checked_sub(self.tables.enums.len())
                .ok_or_else(|| bad(self.sources().eof()))?;
        let read = usize::from(self.builtin_set().contains_enum(BuiltinEnum::ReadStatus));
        if read != 0 && rank == 0 {
            return Ok(BuiltinItem::Enum(BuiltinEnum::ReadStatus));
        }
        if self.builtin_set().contains_enum(BuiltinEnum::WriteStatus) && rank == read {
            return Ok(BuiltinItem::Enum(BuiltinEnum::WriteStatus));
        }
        for item in experimental::Enumeration::ALL {
            if self.builtin_set().experimental_enum_rank(item) == Some(rank) {
                return Ok(BuiltinItem::Experimental(item.item()));
            }
        }
        Err(bad(self.sources().eof()))
    }
    pub fn builtin_set(&self) -> BuiltinSet {
        self.tables.builtins.set()
    }
    pub fn require_builtin_candidate_pipeline(&self) -> Result<(), Box<Diagnostic>> {
        self.tables.require_builtin_candidate_pipeline()
    }
    #[cfg(test)]
    pub fn is_output_candidate_pipeline(&self) -> bool {
        self.tables.is_output_candidate_pipeline()
    }
    #[cfg(test)]
    pub fn require_output_candidate_pipeline(&self) -> Result<(), Box<Diagnostic>> {
        self.tables.require_output_candidate_pipeline()
    }
    pub fn require_no_builtin_candidate(&self) -> Result<(), Box<Diagnostic>> {
        self.tables.require_no_builtin_candidate()
    }
    pub fn enumeration(&self, id: EnumId) -> Result<(EnumAstKey, ModuleId), Box<Diagnostic>> {
        enumeration(&self.tables, self.template_enum_for_instance(id)?)
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
    pub fn experimental_enum_id(
        &self,
        item: experimental::Enumeration,
    ) -> Result<EnumId, Box<Diagnostic>> {
        self.tables
            .builtin_handle(BuiltinItem::Experimental(item.item()))?;
        self.builtin_set()
            .experimental_enum_rank(item)
            .map(|rank| EnumId(self.source_enum_count() + rank))
            .ok_or_else(|| bad(self.sources().eof()))
    }
    pub fn experimental_function_id(
        &self,
        item: experimental::Function,
    ) -> Result<DefId, Box<Diagnostic>> {
        self.tables
            .builtin_handle(BuiltinItem::Experimental(item.item()))?;
        self.builtin_set()
            .experimental_function_rank(item)
            .map(|rank| DefId(self.source_function_count() + rank))
            .ok_or_else(|| bad(self.sources().eof()))
    }
    pub fn experimental_anchor(&self, item: experimental::Item) -> Result<Span, Box<Diagnostic>> {
        self.tables
            .builtin_handle(BuiltinItem::Experimental(item))?;
        self.tables
            .builtin_anchor(
                self.tables
                    .builtins
                    .experimental_anchor(item)
                    .ok_or_else(|| bad(self.sources().eof()))?,
            )
            .copied()
    }
    pub fn experimental_signature(
        &self,
        item: experimental::Function,
    ) -> Result<experimental::Signature, Box<Diagnostic>> {
        self.experimental_function_id(item)?;
        let enumeration = item
            .result_enum()
            .map(|item| self.experimental_enum_id(item))
            .transpose()?;
        Ok(item.signature(|_| enumeration.expect("checked enum dependency")))
    }
    pub fn builtin_enum_anchor(&self, item: BuiltinEnum) -> Result<Span, Box<Diagnostic>> {
        self.builtin_enum_id(item)?;
        self.tables
            .builtin_anchor(
                self.tables
                    .builtins
                    .enum_anchor(item)
                    .ok_or_else(|| bad(self.sources().eof()))?,
            )
            .copied()
    }
    pub fn builtin_function_anchor(&self, item: BuiltinFunction) -> Result<Span, Box<Diagnostic>> {
        self.builtin_function_id(item)?;
        self.tables
            .builtin_anchor(
                self.tables
                    .builtins
                    .function_anchor(item)
                    .ok_or_else(|| bad(self.sources().eof()))?,
            )
            .copied()
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
        for item in experimental::Enumeration::ALL {
            if self
                .builtin_set()
                .experimental_enum_rank(item)
                .is_some_and(|rank| id.0 == self.source_enum_count() + rank)
            {
                return Ok(DeclarationOrigin::Builtin(BuiltinItem::Experimental(
                    item.item(),
                )));
            }
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
        for item in experimental::Function::ALL {
            if self
                .builtin_set()
                .experimental_function_rank(item)
                .is_some_and(|rank| id.0 == self.source_function_count() + rank)
            {
                return Ok(DeclarationOrigin::Builtin(BuiltinItem::Experimental(
                    item.item(),
                )));
            }
        }
        let base = self.source_function_count() + self.builtin_set().extra_functions();
        if let Some(key) =
            id.0.checked_sub(base)
                .and_then(|rank| self.vector_functions().get(rank))
        {
            // The sealed ordered Plan already authenticates this indexed key.
            // No second linear key search is required to name its origin.
            return Ok(DeclarationOrigin::Builtin(BuiltinItem::Vector(key.item())));
        }
        Err(bad(self.sources().eof()))
    }
    // Only checked view constructors use these narrow immutable projections.
    pub(super) fn frozen_enum_syntax(&self, id: EnumId) -> Option<&ast::EnumDecl> {
        if id.0 >= self.source_enum_count() {
            return None;
        }
        let id = self.template_enum_for_instance(id).ok()?;
        self.tables.enums.get(id.0).map(|row| {
            self.tables.sources.frozen_enum(EnumAstKey {
                file: SourceFileId(row.file as usize),
                index: row.local_enum as usize,
            })
        })
    }
    pub(super) fn frozen_enum_origin(&self, id: EnumId) -> DeclarationOrigin {
        self.enum_origin(id).expect("checked enum identity")
    }
    pub(super) fn frozen_enum_name(&self, id: EnumId) -> &str {
        self.frozen_enum_syntax(id).map_or_else(
            || match self.frozen_enum_origin(id) {
                DeclarationOrigin::Builtin(item) => item.name(),
                _ => unreachable!("checked builtin enum"),
            },
            |syntax| self.tables.sources.frozen_text(syntax.name),
        )
    }
    pub(super) fn frozen_enum_anchor(&self, id: EnumId) -> Span {
        self.frozen_enum_syntax(id).map_or_else(
            || match self.frozen_enum_origin(id) {
                DeclarationOrigin::Builtin(BuiltinItem::Enum(item)) => self
                    .builtin_enum_anchor(item)
                    .expect("checked builtin enum"),
                DeclarationOrigin::Builtin(BuiltinItem::Experimental(item)) => self
                    .experimental_anchor(item)
                    .expect("checked experimental enum"),
                _ => unreachable!("checked builtin enum"),
            },
            |syntax| syntax.name,
        )
    }
    pub(super) fn frozen_variant_count(&self, id: EnumId) -> usize {
        self.frozen_enum_syntax(id).map_or_else(
            || match self.frozen_enum_origin(id) {
                DeclarationOrigin::Builtin(item) => item.variant_count(),
                _ => unreachable!("checked enum count"),
            },
            |syntax| syntax.variants.len(),
        )
    }

    pub fn enum_for(&self, key: EnumAstKey) -> Result<EnumId, Box<Diagnostic>> {
        let module = self.tables.sources.module_for_file(key.file)?;
        let row = self.tables.modules[module.0];
        if key.index >= row.enum_len as usize {
            return Err(bad(self.tables.sources.eof()));
        }
        let id = EnumId(row.enum_base as usize + key.index);
        if self.template_enumeration(id)?.0 != key {
            return Err(bad(self.tables.sources.eof()));
        }
        self.ordinary_enum_instance(id)
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
        self.source_function_count()
            + self.builtin_set().extra_functions()
            + self.vector_functions().len()
    }
    pub fn vector_functions(&self) -> &[vector::Function] {
        self.generic_plan()
            .map_or(&[], |plan| plan.vector_functions())
    }
    /// Interpret source-template IDs only. Executable specialization IDs are
    /// assigned after the generic worklist and use vector_function_id instead.
    pub(super) fn vector_template(
        &self,
        id: DefId,
        work: &WorkMeter,
        at: Span,
    ) -> Result<Option<vector::Operation>, Box<Diagnostic>> {
        let Some(wanted) =
            id.0.checked_sub(self.template_function_count() + self.builtin_set().extra_functions())
        else {
            return Ok(None);
        };
        let mut rank = 0;
        for operation in vector::Operation::ALL {
            work.debit(1, at, "vector template selection")?;
            if self.builtin_set().contains_vector(operation.item()) {
                if rank == wanted {
                    return Ok(Some(operation));
                }
                rank += 1;
            }
        }
        Ok(None)
    }
    pub fn vector_function_id(&self, key: vector::Function) -> Result<DefId, Box<Diagnostic>> {
        self.tables
            .builtin_handle(BuiltinItem::Vector(key.item()))?;
        self.generic_plan()
            .and_then(|plan| plan.vector_function_rank(key))
            .map(|rank| {
                DefId(self.source_function_count() + self.builtin_set().extra_functions() + rank)
            })
            .ok_or_else(|| bad(self.sources().eof()))
    }
    pub fn vector_anchor(&self, item: vector::Item) -> Result<Span, Box<Diagnostic>> {
        self.tables.builtin_handle(BuiltinItem::Vector(item))?;
        self.tables
            .builtin_anchor(
                self.tables
                    .builtins
                    .vector_anchor(item)
                    .ok_or_else(|| bad(self.sources().eof()))?,
            )
            .copied()
    }
    pub fn vector_signature(
        &self,
        key: vector::Function,
    ) -> Result<vector::Signature, Box<Diagnostic>> {
        self.vector_function_id(key)?;
        let status = if key.operation().requires_grow_status() {
            self.experimental_enum_id(experimental::Enumeration::Grow)?
        } else {
            EnumId(0)
        };
        Ok(key.signature(status))
    }
    pub fn source_function_count(&self) -> usize {
        self.generic_plan()
            .map_or(self.tables.functions.len(), |plan| plan.len())
    }
    pub(super) fn generic_plan(&self) -> Option<&super::generics::Plan> {
        match &self.tables.candidate_source_origin {
            CandidateOrigin::CurrentGeneric(plan) | CandidateOrigin::CurrentGenericRanges(plan) => {
                Some(&plan[0])
            }
            _ => None,
        }
    }
    pub(super) fn template_function_count(&self) -> usize {
        self.tables.functions.len()
    }
    pub(super) fn template_function(
        &self,
        id: DefId,
    ) -> Result<(FunctionAstKey, ModuleId), Box<Diagnostic>> {
        function(&self.tables, id)
    }
    pub(super) fn template_record_count(&self) -> usize {
        self.tables.records.len()
    }
    pub(super) fn template_record(
        &self,
        id: RecordId,
    ) -> Result<(RecordAstKey, ModuleId), Box<Diagnostic>> {
        record(&self.tables, id)
    }
    pub fn record_count(&self) -> usize {
        self.generic_plan()
            .map_or(self.tables.records.len(), |plan| plan.record_count())
    }
    pub fn function(&self, id: DefId) -> Result<(FunctionAstKey, ModuleId), Box<Diagnostic>> {
        let definition = match self.generic_plan() {
            Some(plan) => plan.definition(id)?,
            None => id,
        };
        function(&self.tables, definition)
    }
    pub fn record(&self, id: RecordId) -> Result<(RecordAstKey, ModuleId), Box<Diagnostic>> {
        let id = self.template_record_for_instance(id)?;
        record(&self.tables, id)
    }
    pub fn def_for(&self, key: FunctionAstKey) -> Result<DefId, Box<Diagnostic>> {
        let module = self.tables.sources.module_for_file(key.file)?;
        let row = self.tables.modules[module.0];
        if key.index >= row.function_len as usize {
            return Err(bad(self.tables.sources.eof()));
        }
        let id = DefId(row.function_base as usize + key.index);
        if self.template_function(id)?.0 != key {
            return Err(bad(self.tables.sources.eof()));
        }
        match self.generic_plan() {
            Some(plan) => plan
                .non_generic(id)
                .ok_or_else(|| bad(self.tables.sources.eof())),
            None => Ok(id),
        }
    }
    pub fn record_for(&self, key: RecordAstKey) -> Result<RecordId, Box<Diagnostic>> {
        let module = self.tables.sources.module_for_file(key.file)?;
        let row = self.tables.modules[module.0];
        if key.index >= row.record_len as usize {
            return Err(bad(self.tables.sources.eof()));
        }
        let id = RecordId(row.record_base as usize + key.index);
        if self.template_record(id)?.0 != key {
            return Err(bad(self.tables.sources.eof()));
        }
        self.ordinary_record_instance(id)
    }
    pub fn root_original_main(&self) -> Option<DefId> {
        let definition =
            (self.tables.root_main != NONE).then_some(DefId(self.tables.root_main as usize))?;
        match self.generic_plan() {
            Some(plan) => plan.non_generic(definition),
            None => Some(definition),
        }
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
        [0, 1, 2, 3].map(|slot| {
            self.tables
                .builtins
                .anchor(slot)
                .and_then(|import| self.tables.builtin_anchor(import).ok())
        })
    }
}

#[cfg(test)]
impl DeclarationIndex<'_> {
    pub(super) fn scan_u8_for_test(&self, work: &WorkMeter) -> Result<(), Box<Diagnostic>> {
        super::u8_reservation::scan(&self.tables, work, &self.tables.sources.eof())
    }
    pub(super) fn corrupt_u8_import_for_test(&mut self, alias: bool) {
        if alias {
            self.tables.imports[0].alias_group = NONE;
        } else {
            self.tables.modules[0].import_start = NONE;
        }
    }
}

#[cfg(test)]
mod vector_source_authority_tests {
    use super::*;
    use crate::frontend::project::ProjectLimits;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering as AtomicOrdering},
    };

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new(source: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "oxid-vector-source-authority-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, AtomicOrdering::Relaxed),
            ));
            fs::create_dir(&root).unwrap();
            fs::write(root.join("main.ox"), source).unwrap();
            Self(root)
        }
        fn load(&self) -> ProjectSources {
            ProjectSources::load_typed(
                self.0.join("main.ox").to_str().unwrap(),
                ProjectLimits::default(),
            )
            .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn imported_vector_new_without_generic_syntax_reaches_element_validation() {
        for text in [
            "use alloc::vec::new;fn main()->i32{new();return 0;}",
            "use alloc::vec::new;fn main()->i32{let values=new();return 0;}",
            "use alloc::vec::new as create;fn main()->i32{create();return 0;}",
        ] {
            let fixture = Fixture::new(text);
            let project = fixture.load();
            let owner = SourceOwner::project(&project);
            let program = owner.ast(ModuleId(0)).unwrap();
            assert!(!program.has_generics());
            assert!(!program.has_record_patterns());
            let work = WorkMeter::default();
            let mut allocator = Allocator::default();
            let facts =
                collect_originals(owner, IndexLimits::default(), &work, &mut allocator).unwrap();
            let errors = facts.finish(&work, &mut allocator).unwrap_err();
            assert_eq!(errors.len(), 1, "{text}: {errors:?}");
            assert_eq!((errors[0].code, errors[0].stage), ("E0300", "generics"));
            assert_eq!(
                errors[0].message,
                "cannot infer new element; specify new::<T>"
            );
        }
        for (imports, planned) in [("", false), ("use alloc::vec::new;", true)] {
            let fixture = Fixture::new(&format!("{imports}fn main()->i32{{return 0;}}"));
            let project = fixture.load();
            let work = WorkMeter::default();
            let mut allocator = Allocator::default();
            let index = collect_originals(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &work,
                &mut allocator,
            )
            .unwrap()
            .finish(&work, &mut allocator)
            .unwrap();
            assert_eq!(index.generic_plan().is_some(), planned);
            if let Some(plan) = index.generic_plan() {
                assert!(
                    plan.vector_functions().is_empty(),
                    "unused imports cannot manufacture instances"
                );
            }
        }
    }

    #[test]
    fn primitive_and_named_slice_authentication_share_an_enforced_scratch_row() {
        let mut scratch = [0; 3];
        for (position, parameter) in ["i32", "&[i32]", "&[T]"].into_iter().enumerate() {
            let fixture = Fixture::new(&format!(
                "fn f<T:Copy>(items:{parameter})->i32{{return 0;}}fn main()->i32{{return 0;}}"
            ));
            let project = fixture.load();
            let facts = collect_originals(
                SourceOwner::project(&project),
                IndexLimits::default(),
                &WorkMeter::default(),
                &mut Allocator::default(),
            )
            .unwrap();
            scratch[position] = facts.plan().scratch;
            let limits = IndexLimits {
                scratch: scratch[position] - 1,
                ..IndexLimits::default()
            };
            let error = collect_originals(
                SourceOwner::project(&project),
                limits,
                &WorkMeter::default(),
                &mut Allocator::default(),
            )
            .unwrap_err();
            assert_eq!(error.code, "E0400");
            assert!(error.message.contains("scratch"), "{error:?}");
        }
        assert_eq!(
            scratch[1] - scratch[0],
            ast::SLICE_SOURCE_CONTROL_BYTES as u64
        );
        assert_eq!(scratch[2], scratch[1]);
    }

    #[test]
    fn original_adapter_refuses_nongeneric_named_slice_relabelled_as_primitive() {
        use crate::frontend::{lexer, parser, source::SourceMap};
        let mut map = SourceMap::new();
        let file = map.add(
            "relabelled-slice.ox".into(),
            "struct T{x:i32} fn f(p:&[T])->i32{return 0;} fn main()->i32{let a=[1];return f(&a);}"
                .into(),
        );
        let source = map.get(file);
        let (mut program, _) = parser::parse_typed_counted(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::ProjectCandidate,
            parser::MAX_NODES,
            &mut Allocator::default(),
            &mut Default::default(),
        )
        .unwrap();
        assert!(!program.has_generics());
        assert!(!program.has_record_patterns());
        assert!(!program.uses_project_syntax());
        let parameter = program.functions[0].params[0].ty;
        assert!(matches!(
            parameter.kind,
            ast::TypeSyntaxKind::GenericSliceReference { .. }
        ));
        {
            // The original source's record element is semantically inadmissible.
            let work = WorkMeter::default();
            let mut allocator = Allocator::default();
            let owner = SourceOwner::original(source, &program, SourceView::Map(&map)).unwrap();
            let facts =
                collect_originals(owner, IndexLimits::default(), &work, &mut allocator).unwrap();
            let error = facts
                .signature_view(&work)
                .parameter_type(ModuleId(0), parameter)
                .unwrap_err();
            assert_eq!(error.code, "E0300");
            assert_eq!(error.message, "slice elements must be scalar Copy types");
            // The immutable parse fact now also rejects this semantic error
            // while finishing, rather than returning an unchecked index.
            let errors = facts.finish(&work, &mut allocator).unwrap_err();
            assert_eq!(errors[0].code, "E0300");
        }
        program.functions[0].params[0].ty.kind = ast::TypeSyntaxKind::SliceReference {
            mutable: false,
            element: ast::ArrayElementTypeSyntax::I32,
        };
        assert!(program.belongs_to(source), "cheap provenance is unchanged");
        assert_eq!(program.functions[0].params[0].ty.span, parameter.span);
        let mut features = ast::SyntaxFeatures::default();
        assert!(!program.validate_spans_and_ids_counted_with_features(
            Some(source),
            |at| at.is_none_or(|at| source.try_text(at).is_some()),
            &mut None,
            parser::StdImportPolicy::Enabled,
            &mut false,
            &mut features,
        ));
        assert!(features.source_slices);
        assert!(
            !features.generic_slices,
            "forged variant cannot select its authentication budget"
        );
        let owner = SourceOwner::original(source, &program, SourceView::Map(&map)).unwrap();
        let mut allocator = Allocator::default();
        let error = collect_originals(
            owner,
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut allocator,
        )
        .unwrap_err();
        assert_eq!((error.code, error.stage), ("E0500", "resolve-project"));
        assert_eq!(allocator.attempts, 0, "reject before allocating index rows");
    }

    #[test]
    fn nongeneric_named_slice_relabels_cannot_remove_paid_full_authentication() {
        use crate::frontend::{lexer, parser, source::SourceMap};
        for mixed in [false, true] {
            let parameters = if mixed { "p:&[T],q:&[T]" } else { "p:&[T]" };
            for replacement in 0..7 {
                let mut map = SourceMap::new();
                let file = map.add(
                    "relabelled-named-slice.ox".into(),
                    format!(
                        "struct T{{x:i32}} fn f({parameters})->i32{{return 0;}} fn main()->i32{{return 0;}}"
                    ),
                );
                let source = map.get(file);
                let (mut program, _) = parser::parse_typed_counted(
                    source,
                    lexer::lex(source).unwrap(),
                    parser::SourceMode::ProjectCandidate,
                    parser::MAX_NODES,
                    &mut Allocator::default(),
                    &mut Default::default(),
                )
                .unwrap();
                assert!(!program.has_generics());
                assert!(!program.has_record_patterns());
                assert!(!program.uses_project_syntax());
                assert!(program.has_parsed_named_slice());
                let parameter = program.functions[0].params[0].ty;
                let ast::TypeSyntaxKind::GenericSliceReference { element, .. } = parameter.kind
                else {
                    panic!("expected the source's named slice");
                };
                let array = ast::FixedArraySyntax {
                    element: ast::ArrayElementTypeSyntax::I32,
                    length: 1,
                };
                program.functions[0].params[0].ty.kind = match replacement {
                    0 | 1 => ast::TypeSyntaxKind::ArrayReference {
                        mutable: replacement == 1,
                        array,
                    },
                    2 | 3 => ast::TypeSyntaxKind::Reference {
                        mutable: replacement == 3,
                        referent: element,
                    },
                    4 => ast::TypeSyntaxKind::Unit,
                    5 => ast::TypeSyntaxKind::Array(array),
                    6 => ast::TypeSyntaxKind::Name(element),
                    _ => unreachable!(),
                };
                assert_eq!(program.functions[0].params[0].ty.span, parameter.span);
                assert!(program.belongs_to(source), "provenance remains cheap");
                assert!(program.has_parsed_named_slice());
                let mut features = ast::SyntaxFeatures::default();
                assert!(program.validate_spans_and_ids_counted_with_features(
                    Some(source),
                    |at| at.is_none_or(|at| source.try_text(at).is_some()),
                    &mut None,
                    parser::StdImportPolicy::Enabled,
                    &mut false,
                    &mut features,
                ));
                // A single relabel can erase all mutable slice observations;
                // leaving another named parameter cannot authorize this one.
                assert_eq!(features.source_slices, mixed);
                assert_eq!(features.generic_slices, mixed);
                let owner = SourceOwner::original(source, &program, SourceView::Map(&map)).unwrap();
                let work = WorkMeter::default();
                work.enable_observation();
                let mut allocator = Allocator::default();
                let facts = collect_originals(owner, IndexLimits::default(), &work, &mut allocator)
                    .unwrap();
                let errors = facts.finish(&work, &mut allocator).unwrap_err();
                assert_eq!(errors.len(), 1, "mixed={mixed} replacement={replacement}");
                assert_eq!(
                    (errors[0].code, errors[0].stage),
                    ("E0500", "resolve-project"),
                    "mixed={mixed} replacement={replacement}: {errors:?}"
                );
                assert!(work.events.borrow().iter().any(|event| {
                    event.operation == "generic syntax authentication" && event.units > 0
                }));
            }
        }
    }

    #[test]
    fn original_primitive_slices_do_not_activate_named_slice_planning() {
        use crate::frontend::{lexer, parser, source::SourceMap};
        let mut map = SourceMap::new();
        let file = map.add(
            "original-primitive-slices.ox".into(),
            "fn f(p:&[i32],q:&mut[bool])->i32{return 0;} fn main()->i32{return 0;}".into(),
        );
        let source = map.get(file);
        let (program, _) = parser::parse_typed_counted(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::ProjectCandidate,
            parser::MAX_NODES,
            &mut Allocator::default(),
            &mut Default::default(),
        )
        .unwrap();
        assert!(!program.has_parsed_named_slice());
        let work = WorkMeter::default();
        work.enable_observation();
        let mut allocator = Allocator::default();
        let index = collect_originals(
            SourceOwner::original(source, &program, SourceView::Map(&map)).unwrap(),
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
        assert!(index.generic_plan().is_none());
        assert!(work
            .events
            .borrow()
            .iter()
            .all(|event| { event.operation != "generic syntax authentication" }));
    }
}
