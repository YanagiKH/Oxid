//! One private declaration, import and permission implementation for both routes.
//! No linked execution witness is constructed in this module.
#![allow(dead_code)] // Private ProjectCandidate qualification precedes activation.

mod enum_views;
mod resource;
mod sealed;
mod source_owner;
pub(super) use source_owner::SourceOwner;
#[cfg(test)]
mod builtin_tests;
#[cfg(test)]
mod enum_query_tests;
#[cfg(test)]
mod enum_tests;
#[cfg(test)]
mod tests;
use super::{
    ast,
    builtin_catalog::{BuiltinSet, BuiltinItem, BuiltinEnum, BuiltinFunction, DeclarationOrigin},
    diagnostic::Diagnostic,
    hir::{DefId, Ty},
    oir::owned_types::{
        AggregateTy, BorrowKind, BorrowedTy, EnumId, FieldId, FixedArrayTy, ParameterTy, RecordId,
        ValueTy, VariantId,
    },
    owned_diagnostic,
    project::{
        budget::{Allocator, ReserveFailure},
        EnumAstKey, FunctionAstKey, ItemPathRef, ModuleId, ProjectSources, QualifiedPathRef,
        RecordAstKey, SyntaxFlavor,
    },
    source::{SourceFile, SourceFileId, SourceView, Span},
};
pub(super) use enum_views::{EnumVariantCounts, EnumView, NominalId, VariantView};
use resource::{add, allocate, allocate_exact, compact, compare_bytes, merge_sort};
#[cfg(test)]
pub(super) use resource::{
    AliasObservation, NominalAliasObservation, Observation, SeenObservation,
};
pub(super) use resource::{Counts, IndexLimits, IndexPlan, WorkMeter};
#[cfg(test)]
pub(super) use sealed::{collect_closed, collect_enum_candidate, collect_builtin_candidate};
pub(super) use sealed::{collect_originals, DeclarationFacts, DeclarationIndex};
use sealed::{BuiltinAdmission, CandidateOrigin};
use std::{cmp::Ordering, fmt, mem::size_of};

const NONE: u32 = u32::MAX;
const BUILTIN_CONFLICT: u32 = u32::MAX - 1;
const FUNCTION: u32 = 0;
const RECORD: u32 = 1;
const MODULE: u32 = 2;
const ENUM: u32 = 3;
const KIND_MASK: u32 = 3;
const PUBLIC: u32 = 4;

/// Both alias lanes share the original-row prefix and checked builtin suffix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DeclarationHandle(u32);
const NO_DECLARATION: DeclarationHandle = DeclarationHandle(NONE);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeclarationProjection { SourceOriginal(u32), Builtin(BuiltinItem) }


fn diagnostic(
    code: &'static str,
    stage: &'static str,
    message: &'static str,
    at: Span,
) -> Box<Diagnostic> {
    owned_diagnostic::diagnostic(code, stage, format_args!("{message}"), Some(at))
}
fn bad(at: Span) -> Box<Diagnostic> {
    diagnostic(
        "E0500",
        "resolve-project",
        "invalid declaration index source or identity",
        at,
    )
}
fn resource(message: &'static str, at: Span) -> Box<Diagnostic> {
    diagnostic("E0400", "resolve-project", message, at)
}
fn overflow(at: Span) -> Box<Diagnostic> {
    resource("declaration index count overflow", at)
}
fn duplicate(at: Span, previous: Span) -> Box<Diagnostic> {
    owned_diagnostic::secondary(
        diagnostic(
            "E0201",
            "resolve",
            "duplicate binding; shadowing is unavailable in typed-preview",
            at,
        ),
        previous,
        format_args!("first declared here"),
    )
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct CompactSpan {
    file: u32,
    start: u32,
    end: u32,
}
impl CompactSpan {
    fn new(span: Span) -> Result<Self, Box<Diagnostic>> {
        Ok(Self {
            file: compact(span.file.0, span)?,
            start: compact(span.start, span)?,
            end: compact(span.end, span)?,
        })
    }
    fn span(self) -> Span {
        Span {
            file: SourceFileId(self.file as usize),
            start: self.start as usize,
            end: self.end as usize,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct OriginalRow {
    name: CompactSpan,
    owner: u32,
    target: u32,
    flags: u32,
    domain: u32,
    restrictor: u32,
    conflict: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct FunctionRow {
    file: u32,
    local_function: u32,
    original: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct RecordRow {
    file: u32,
    local_record: u32,
    original: u32,
    field_start: u32,
    field_len: u32,
    first_private: u32,
    construction_domain: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct FieldRow {
    name: CompactSpan,
    domain: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct EnumRow {
    file: u32,
    local_enum: u32,
    original: u32,
    variant_start: u32,
    variant_len: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct VariantRow {
    name: CompactSpan,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct ModuleRow {
    parent: u32,
    subtree_end: u32,
    original: u32,
    function_base: u32,
    function_len: u32,
    record_base: u32,
    record_len: u32,
    enum_base: u32,
    enum_len: u32,
    original_start: u32,
    original_len: u32,
    import_start: u32,
    import_len: u32,
    domain: u32,
    restrictor: u32,
    child_start: u32,
    child_len: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct ImportRow {
    module: u32,
    file: u32,
    local_import: u32,
    alias_group: u32,
    target_group: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct AliasCell {
    // Original-row identity (RECORD or ENUM), never a record-table ordinal.
    type_target: DeclarationHandle,
    value_target: DeclarationHandle,
    type_first_import: u32,
    value_first_import: u32,
}
impl Default for AliasCell {
    fn default() -> Self {
        Self {
            type_target: NO_DECLARATION,
            value_target: NO_DECLARATION,
            type_first_import: NONE,
            value_first_import: NONE,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct SeenCell {
    type_first_import: u32,
    value_first_import: u32,
}
impl Default for SeenCell {
    fn default() -> Self {
        Self {
            type_first_import: NONE,
            value_first_import: NONE,
        }
    }
}
const _: () = {
    assert!(size_of::<OriginalRow>() == 36);
    assert!(size_of::<FunctionRow>() == 12);
    assert!(size_of::<RecordRow>() == 28);
    assert!(size_of::<FieldRow>() == 16);
    assert!(size_of::<ModuleRow>() == 68);
    assert!(size_of::<EnumRow>() == 20);
    assert!(size_of::<VariantRow>() == 12);
    assert!(size_of::<ImportRow>() == 20);
    assert!(size_of::<AliasCell>() == 16);
    assert!(size_of::<SeenCell>() == 8);
};

#[derive(Debug)]
struct Tables<'s> {
    sources: SourceOwner<'s>,
    originals: Vec<OriginalRow>,
    original_order: Vec<u32>,
    functions: Vec<FunctionRow>,
    records: Vec<RecordRow>,
    fields: Vec<FieldRow>,
    enums: Vec<EnumRow>,
    variants: Vec<VariantRow>,
    modules: Vec<ModuleRow>,
    children: Vec<u32>,
    imports: Vec<ImportRow>,
    aliases: Vec<AliasCell>,
    alias_order: Vec<u32>,
    root_main: u32,
    candidate_source_origin: CandidateOrigin,
    builtins: BuiltinAdmission,
}
#[derive(Debug)]
struct Scratch {
    merge: Vec<u32>,
    targets: Vec<u32>,
    seen: Vec<SeenCell>,
    next_child: Vec<u32>,
}
// Explicit state ledger: conservative sum of nonoverlapping phase state plus
// two complete prepared nominal-name arrays. Rows are allocation-free.
// Conservative explicit fixed-state envelope. The 128-word bank covers scalar
// item handles, counters, ranges and formatter arithmetic; row copies and other
// association handles are listed separately. Disjoint build/query/format phases are deliberately
// summed. Test-only event Vec headers/payloads are excluded.
// The prior absolute record/callee -> select -> absolute_endpoint -> segments
// chain has four by-value ItemPathRef handles (20 measured words). Name that
// inherited obligation inside the unchanged 128-word bank. The other 108 words
// retain the historical scalar/range/arithmetic envelope; this change does not
// independently re-prove that older bank's complete slot-level occupancy.
const ITEM_HANDLE_WORDS: usize = size_of::<[ItemPathRef; 4]>() / size_of::<usize>();
const FIXED_SCRATCH: usize = size_of::<Scratch>()
    + size_of::<IndexPlan>()
    + size_of::<Counts>()
    + size_of::<Tables<'static>>()
    + size_of::<[u32; 33]>() * 3
    + size_of::<ImportTxn>()
    + size_of::<PreparedTypeName<'static>>() * 2
    + size_of::<[ItemPathRef; 4]>()
    + size_of::<[usize; 128 - ITEM_HANDLE_WORDS]>()
    + size_of::<[u64; 2]>()
    + size_of::<IndexLimits>()
    + size_of::<SourceOwner<'static>>()
    // The checked path view and its full associated handle coexist at query sites.
    + size_of::<source_owner::QualifiedPathView<'static>>()
    + size_of::<super::project::QualifiedPathRef>()
    + size_of::<QuerySession<'static, 'static>>()
    + size_of::<OriginalRow>()
    + size_of::<ModuleRow>()
    + size_of::<FunctionRow>()
    + size_of::<RecordRow>()
    + size_of::<FieldRow>()
    + size_of::<EnumRow>()
    + size_of::<VariantRow>()
    + size_of::<EnumAstKey>()
    + size_of::<NominalId>()
    + size_of::<Option<CompactSpan>>()
    + size_of::<Option<EnumView<'static>>>()
    + size_of::<Option<VariantView<'static>>>()
    + size_of::<EnumVariantCounts<'static>>()
    + size_of::<sealed::EnumSourceCounts<'static>>()
    // Record inventory remains live while enum count admission returns its summary.
    + size_of::<super::oir::owned_types::DeclarationUsage>()
    + size_of::<super::oir::owned_types::EnumUsage>()
    + size_of::<ImportRow>()
    + size_of::<AliasCell>()
    + size_of::<SeenCell>()
    + size_of::<[Span; 4]>()
    // Complete fallible query results may coexist with legacy wrapper returns.
    + size_of::<Result<AbsolutePrefix, Box<Diagnostic>>>()
    + size_of::<Result<QualifiedValueEndpoint, Box<Diagnostic>>>()
    + size_of::<Result<NominalExposure, Box<Diagnostic>>>()
    // The nominal request and reverse-map validation result can coexist.
    + size_of::<Result<NominalId, Box<Diagnostic>>>()
    // The shared nominal helper borrows a handle rather than making a fifth copy.
    + size_of::<&ItemPathRef>()
    // Added import/identity state is priced in full, even when build/query
    // phases do not overlap. The admission local is separate from Tables.
    + size_of::<BuiltinAdmission>()
    + size_of::<Result<DeclarationProjection, Box<Diagnostic>>>()
    + size_of::<Result<Option<DeclarationHandle>, Box<Diagnostic>>>()
    + size_of::<Result<BuiltinItem, Box<Diagnostic>>>()
    + size_of::<Option<&ast::EnumDecl>>()
    + size_of::<Option<&ast::EnumVariantSyntax>>()
    // Target grouping now holds two complete root-bearing path views.
    + size_of::<source_owner::QualifiedPathView<'static>>()
    // Std walk report and checked source-order inventory cursor.
    + size_of::<bool>()
    + size_of::<Option<usize>>()
    + size_of::<super::parser::StdImportPolicy>()
    + size_of::<Result<DeclarationOrigin, Box<Diagnostic>>>()
    + size_of::<[usize; 2]>();
const _: () = {
    assert!(FIXED_SCRATCH <= 4096);
    assert!(size_of::<[ItemPathRef; 4]>().is_multiple_of(size_of::<usize>()));
    assert!(ITEM_HANDLE_WORDS <= 128);
    // The existing four-Span envelope covers the legacy exposure return and use
    // origin (56 + 24 on the measured target). The generic nominal result above
    // is separate, so conversion does not borrow arbitrary counter-bank credit.
    assert!(
        size_of::<Result<Exposure, Box<Diagnostic>>>() + size_of::<Span>()
            <= size_of::<[Span; 4]>()
    );
};

#[derive(Clone, Copy, Debug)]
enum AbsolutePrefix {
    Module(ModuleId),
    Enumeration(u32), // The visible enum original, not a module/record ordinal.
}

impl Tables<'_> {
    fn require_current_source_pipeline(&self) -> Result<(), Box<Diagnostic>> {
        self.require_no_builtin_candidate()?;
        if let Some(origin) = self.candidate_source_origin.span() {
            return Err(diagnostic("E0101", "resolve", "enum source syntax is unavailable", origin));
        }
        Ok(())
    }
    fn require_no_builtin_candidate(&self) -> Result<(), Box<Diagnostic>> {
        if self.candidate_source_origin.is_builtin() || self.builtins.set() != BuiltinSet::None {
            return Err(diagnostic("E0101", "resolve", "builtin source syntax is unavailable",
                self.candidate_source_origin.span().unwrap_or(self.sources.eof())));
        }
        Ok(())
    }
    fn require_builtin_candidate_pipeline(&self) -> Result<(), Box<Diagnostic>> {
        if !self.candidate_source_origin.is_builtin() { return Err(bad(self.sources.eof())); }
        Ok(())
    }
    fn builtin_handle(&self, item: BuiltinItem) -> Result<DeclarationHandle, Box<Diagnostic>> {
        let extra = match item {
            BuiltinItem::Enum(_) if self.builtins.set().extra_enums() == 1 => 0,
            BuiltinItem::Function(_) if self.builtins.set().extra_functions() == 1 => 1,
            _ => return Err(bad(self.sources.eof())),
        };
        let target = self.originals.len().checked_add(extra).ok_or_else(|| overflow(self.sources.eof()))?;
        Ok(DeclarationHandle(compact(target, self.sources.eof())?))
    }
    fn project_handle(&self, handle: DeclarationHandle) -> Result<DeclarationProjection, Box<Diagnostic>> {
        if (handle.0 as usize) < self.originals.len() { return Ok(DeclarationProjection::SourceOriginal(handle.0)); }
        for item in [BuiltinItem::Enum(BuiltinEnum::ReadStatus), BuiltinItem::Function(BuiltinFunction::ReadStdin)] {
            if self.builtin_handle(item).is_ok_and(|expected| expected == handle) {
                return Ok(DeclarationProjection::Builtin(item));
            }
        }
        Err(bad(self.sources.eof()))
    }
    fn nominal_handle(&self, handle: DeclarationHandle) -> Result<NominalId, Box<Diagnostic>> {
        match self.project_handle(handle)? {
            DeclarationProjection::SourceOriginal(id) => self.nominal_original(id),
            DeclarationProjection::Builtin(BuiltinItem::Enum(_)) => Ok(NominalId::Enum(EnumId(self.enums.len()))),
            _ => Err(bad(self.sources.eof())),
        }
    }
    fn function_handle(&self, handle: DeclarationHandle) -> Result<DefId, Box<Diagnostic>> {
        match self.project_handle(handle)? {
            DeclarationProjection::SourceOriginal(id) => {
                let row = self.original(id)?;
                if row.flags & KIND_MASK != FUNCTION { return Err(bad(row.name.span())); }
                let target = DefId(row.target as usize);
                if self.functions.get(target.0).is_none_or(|row| row.original != id) { return Err(bad(self.sources.eof())); }
                Ok(target)
            },
            DeclarationProjection::Builtin(BuiltinItem::Function(_)) => Ok(DefId(self.functions.len())),
            _ => Err(bad(self.sources.eof())),
        }
    }
    fn access_handle(&self, handle: DeclarationHandle, requester: ModuleId, work: &WorkMeter, at: Span) -> Result<(), Box<Diagnostic>> {
        match self.project_handle(handle)? {
            DeclarationProjection::SourceOriginal(id) => self.access_original(id, requester, work, at),
            DeclarationProjection::Builtin(_) => {
                work.debit(1, at, "target permission")?;
                if !self.permission(NONE, requester, work, at)? { return Err(bad(at)); }
                Ok(())
            }
        }
    }
    fn nominal_original(&self, id: u32) -> Result<NominalId, Box<Diagnostic>> {
        let original = self.original(id)?;
        match original.flags & KIND_MASK {
            RECORD => {
                let row = self
                    .records
                    .get(original.target as usize)
                    .ok_or_else(|| bad(original.name.span()))?;
                if row.original != id {
                    return Err(bad(original.name.span()));
                }
                Ok(NominalId::Record(RecordId(original.target as usize)))
            }
            ENUM => {
                let row = self
                    .enums
                    .get(original.target as usize)
                    .ok_or_else(|| bad(original.name.span()))?;
                if row.original != id {
                    return Err(bad(original.name.span()));
                }
                Ok(NominalId::Enum(EnumId(original.target as usize)))
            }
            _ => Err(bad(original.name.span())),
        }
    }
    fn original_for_nominal(&self, nominal: NominalId, at: Span) -> Result<DeclarationHandle, Box<Diagnostic>> {
        if nominal == NominalId::Enum(EnumId(self.enums.len())) && self.builtins.set().extra_enums() == 1 {
            return self.builtin_handle(BuiltinItem::Enum(BuiltinEnum::ReadStatus));
        }
        let original = match nominal {
            NominalId::Record(id) => self.records.get(id.0).map(|row| row.original),
            NominalId::Enum(id) => self.enums.get(id.0).map(|row| row.original),
        }
        .ok_or_else(|| bad(at))?;
        if self.nominal_original(original)? != nominal {
            return Err(bad(at));
        }
        Ok(DeclarationHandle(original))
    }
    fn original(&self, id: u32) -> Result<&OriginalRow, Box<Diagnostic>> {
        self.originals
            .get(id as usize)
            .ok_or_else(|| bad(self.sources.eof()))
    }
    fn import(&self, id: u32) -> Result<&ast::ImportDecl, Box<Diagnostic>> {
        let row = self
            .imports
            .get(id as usize)
            .ok_or_else(|| bad(self.sources.eof()))?;
        self.sources
            .ast(ModuleId(row.module as usize))?
            .imports
            .get(row.local_import as usize)
            .ok_or_else(|| bad(self.sources.eof()))
    }
    fn original_cmp(
        &self,
        a: u32,
        b: u32,
        work: &WorkMeter,
        at: Span,
    ) -> Result<Ordering, Box<Diagnostic>> {
        let (a, b) = (self.original(a)?, self.original(b)?);
        let numeric = (a.owner, a.flags & KIND_MASK != FUNCTION)
            .cmp(&(b.owner, b.flags & KIND_MASK != FUNCTION));
        if numeric != Ordering::Equal {
            work.debit(1, at, "comparison")?;
            return Ok(numeric);
        }
        compare_bytes(
            self.sources.text(a.name.span())?,
            self.sources.text(b.name.span())?,
            work,
            at,
        )
    }
    fn alias_cmp(
        &self,
        a: u32,
        b: u32,
        work: &WorkMeter,
        at: Span,
    ) -> Result<Ordering, Box<Diagnostic>> {
        let order = self.imports[a as usize]
            .module
            .cmp(&self.imports[b as usize].module);
        if order != Ordering::Equal {
            work.debit(1, at, "comparison")?;
            return Ok(order);
        }
        compare_bytes(
            self.sources.text(self.import(a)?.alias)?,
            self.sources.text(self.import(b)?.alias)?,
            work,
            at,
        )
    }
    fn target_cmp(
        &self,
        a: u32,
        b: u32,
        work: &WorkMeter,
        at: Span,
    ) -> Result<Ordering, Box<Diagnostic>> {
        work.debit(1, at, "path comparison")?;
        let (ra, rb) = (&self.imports[a as usize], &self.imports[b as usize]);
        let order = ra.module.cmp(&rb.module);
        if order != Ordering::Equal {
            return Ok(order);
        }
        let a = self.sources.import_path(QualifiedPathRef { file: SourceFileId(ra.file as usize), path: self.import(a)?.path })?;
        let b = self.sources.import_path(QualifiedPathRef { file: SourceFileId(rb.file as usize), path: self.import(b)?.path })?;
        let roots = (a.root() as u8).cmp(&(b.root() as u8));
        if roots != Ordering::Equal { return Ok(roots); }
        let (a,b) = (a.segments(), b.segments());
        for (a, b) in a.iter().zip(b) {
            work.debit(2, at, "path segment comparison")?;
            let (a, b) = (self.sources.text(*a)?, self.sources.text(*b)?);
            // The enclosing comparator already paid its invocation.
            for (x, y) in a.bytes().zip(b.bytes()) {
                work.debit(1, at, "compared byte")?;
                if x != y {
                    return Ok(x.cmp(&y));
                }
            }
            let order = a.len().cmp(&b.len());
            if order != Ordering::Equal {
                return Ok(order);
            }
        }
        Ok(a.len().cmp(&b.len()))
    }
    fn ancestor(&self, a: u32, b: u32) -> Result<bool, Box<Diagnostic>> {
        let row = self
            .modules
            .get(a as usize)
            .ok_or_else(|| bad(self.sources.eof()))?;
        if b as usize >= self.modules.len() {
            return Err(bad(self.sources.eof()));
        }
        Ok(a <= b && b < row.subtree_end)
    }
    fn permission(
        &self,
        domain: u32,
        requester: ModuleId,
        work: &WorkMeter,
        at: Span,
    ) -> Result<bool, Box<Diagnostic>> {
        work.debit(1, at, "domain permission")?;
        let r = compact(requester.0, at)?;
        if r as usize >= self.modules.len() {
            return Err(bad(at));
        }
        Ok(domain == NONE || self.ancestor(domain, r)?)
    }
    fn lookup_original(
        &self,
        module: ModuleId,
        type_lane: bool,
        name: Span,
        work: &WorkMeter,
    ) -> Result<Option<u32>, Box<Diagnostic>> {
        let query = self.sources.text(name)?;
        let mut low = 0;
        let mut high = self.original_order.len();
        while low < high {
            work.debit(1, name, "original lookup probe")?;
            let mid = low + (high - low) / 2;
            let id = self.original_order[mid];
            let row = self.original(id)?;
            let numeric =
                (row.owner as usize, row.flags & KIND_MASK != FUNCTION).cmp(&(module.0, type_lane));
            let order = if numeric == Ordering::Equal {
                compare_bytes(self.sources.text(row.name.span())?, query, work, name)?
            } else {
                work.debit(1, name, "comparison")?;
                numeric
            };
            match order {
                Ordering::Less => low = mid + 1,
                Ordering::Greater => high = mid,
                Ordering::Equal => {
                    return Ok(Some(if row.conflict < NONE - 1 {
                        row.conflict
                    } else {
                        id
                    }))
                }
            }
        }
        Ok(None)
    }
    fn lookup_alias(
        &self,
        module: ModuleId,
        name: Span,
        work: &WorkMeter,
    ) -> Result<Option<u32>, Box<Diagnostic>> {
        let query = self.sources.text(name)?;
        let (mut low, mut high) = (0, self.alias_order.len());
        while low < high {
            work.debit(1, name, "alias lookup probe")?;
            let mid = low + (high - low) / 2;
            let id = self.alias_order[mid];
            let row = &self.imports[id as usize];
            let numeric = (row.module as usize).cmp(&module.0);
            let order = if numeric == Ordering::Equal {
                compare_bytes(
                    self.sources.text(self.import(id)?.alias)?,
                    query,
                    work,
                    name,
                )?
            } else {
                work.debit(1, name, "comparison")?;
                numeric
            };
            match order {
                Ordering::Less => low = mid + 1,
                Ordering::Greater => high = mid,
                Ordering::Equal => return Ok(Some(row.alias_group)),
            }
        }
        Ok(None)
    }
    fn item_private(
        &self,
        original: u32,
        at: Span,
        stage: &'static str,
    ) -> Result<Box<Diagnostic>, Box<Diagnostic>> {
        let row = self.original(original)?;
        let witness = row;
        Ok(owned_diagnostic::secondary(
            diagnostic(
                "E0206",
                stage,
                "item is private to its declaring module",
                at,
            ),
            witness.name.span(),
            format_args!("private declaration here"),
        ))
    }
    fn access_original(
        &self,
        id: u32,
        requester: ModuleId,
        work: &WorkMeter,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        work.debit(1, at, "target permission")?;
        if !self.permission(self.original(id)?.domain, requester, work, at)? {
            return Err(self.item_private(id, at, "resolve")?);
        }
        Ok(())
    }
    fn absolute_prefix(
        &self,
        requester: ModuleId,
        segments: &[Span],
        work: &WorkMeter,
        allow_enum_member: bool,
    ) -> Result<AbsolutePrefix, Box<Diagnostic>> {
        if !(2..=super::parser::MAX_PATH_SEGMENTS).contains(&segments.len()) {
            return Err(bad(self.sources.eof()));
        }
        let mut owner = ModuleId(0);
        for (position, &segment) in segments[1..segments.len() - 1].iter().enumerate() {
            work.debit(1, segment, "absolute path segment")?;
            let Some(id) = self.lookup_original(owner, true, segment, work)? else {
                let mut error =
                    diagnostic("E0205", "resolve", "invalid absolute item path", segment);
                if let Some(alias) = self.lookup_alias(owner, segment, work)? {
                    error = owned_diagnostic::secondary(
                        error,
                        self.import(alias)?.alias,
                        format_args!("import alias declared here"),
                    );
                }
                return Err(error);
            };
            let row = self.original(id)?;
            match row.flags & KIND_MASK {
                MODULE => {
                    self.access_original(id, requester, work, segment)?;
                    owner = ModuleId(row.target as usize);
                }
                ENUM if allow_enum_member && position + 3 == segments.len() => {
                    self.access_original(id, requester, work, segment)?;
                    return Ok(AbsolutePrefix::Enumeration(id));
                }
                _ => {
                    return Err(diagnostic(
                        "E0205",
                        "resolve",
                        "invalid absolute item path",
                        segment,
                    ))
                }
            }
        }
        Ok(AbsolutePrefix::Module(owner))
    }
    fn lookup_variant(
        &self,
        enumeration: EnumId,
        member: Span,
        work: &WorkMeter,
    ) -> Result<VariantId, Box<Diagnostic>> {
        if enumeration.0 == self.enums.len() && self.builtins.set().extra_enums() == 1 {
            let spelling = self.sources.text(member)?;
            for index in 0..BuiltinEnum::ReadStatus.variant_count() {
                work.debit(1, member, "variant lookup probe")?;
                if compare_bytes(BuiltinEnum::ReadStatus.member_name(index).expect("closed member"), spelling, work, member)? == Ordering::Equal {
                    return Ok(VariantId { enumeration, index });
                }
            }
            work.debit((spelling.len().min(64) + BuiltinEnum::ReadStatus.name().len()) as u64, member, "variant diagnostic name bytes")?;
            return Err(owned_diagnostic::diagnostic("E0200", "resolve", format_args!("unknown variant `{}` of enum `ReadStatus`", owned_diagnostic::name(spelling)), Some(member)));
        }
        let row = self.enums.get(enumeration.0).ok_or_else(|| bad(member))?;
        let end = (row.variant_start as usize)
            .checked_add(row.variant_len as usize)
            .ok_or_else(|| bad(member))?;
        let variants = self
            .variants
            .get(row.variant_start as usize..end)
            .ok_or_else(|| bad(member))?;
        if !(1..=256).contains(&variants.len()) {
            return Err(bad(member));
        }
        let spelling = self.sources.text(member)?;
        for (index, variant) in variants.iter().enumerate() {
            work.debit(1, member, "variant lookup probe")?;
            if compare_bytes(
                self.sources.text(variant.name.span())?,
                spelling,
                work,
                member,
            )? == Ordering::Equal
            {
                return Ok(VariantId { enumeration, index });
            }
        }
        let declaration = self
            .sources
            .text(self.original(row.original)?.name.span())?;
        work.debit(
            (spelling.len().min(64) + declaration.len().min(64)) as u64,
            member,
            "variant diagnostic name bytes",
        )?;
        Err(owned_diagnostic::diagnostic(
            "E0200",
            "resolve",
            format_args!(
                "unknown variant `{}` of enum `{}`",
                owned_diagnostic::name(spelling),
                owned_diagnostic::name(declaration)
            ),
            Some(member),
        ))
    }
    fn absolute_endpoint(
        &self,
        requester: ModuleId,
        path: ItemPathRef,
        work: &WorkMeter,
        importing: bool,
    ) -> Result<(Option<DeclarationHandle>, Option<DeclarationHandle>), Box<Diagnostic>> {
        let segments = self.sources.segments(path)?;
        let owner = match self.absolute_prefix(requester, segments, work, false)? {
            AbsolutePrefix::Module(owner) => owner,
            AbsolutePrefix::Enumeration(_) => return Err(bad(self.sources.eof())),
        };
        let endpoint = *segments.last().ok_or_else(|| bad(self.sources.eof()))?;
        work.debit(1, endpoint, "absolute path segment")?;
        let original_type = self.lookup_original(owner, true, endpoint, work)?;
        let ty = original_type.filter(|id| {
            matches!(
                self.originals[*id as usize].flags & KIND_MASK,
                RECORD | ENUM
            )
        });
        let value = self.lookup_original(owner, false, endpoint, work)?;
        if importing && ty.is_none() && value.is_none() {
            let mut error = diagnostic(
                "E0205",
                "resolve",
                if self.enums.is_empty() {
                    "absolute path endpoint is not an original function or record"
                } else {
                    "absolute path endpoint is not an original function or nominal type"
                },
                endpoint,
            );
            if let Some(alias) = if original_type.is_none() {
                self.lookup_alias(owner, endpoint, work)?
            } else {
                None
            } {
                error = owned_diagnostic::secondary(
                    error,
                    self.import(alias)?.alias,
                    format_args!("import alias declared here"),
                );
            }
            return Err(error);
        }
        Ok((ty.map(DeclarationHandle), value.map(DeclarationHandle)))
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum TypeContext {
    Scalar,
    Value,
    Reference,
    Constructor,
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Access {
    Allowed,
    Denied(FieldId),
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Exposure {
    Allowed,
    Denied {
        record: Span,
        restrictor: Option<Span>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QualifiedValueEndpoint {
    Function(DefId),
    Variant(VariantId),
}
#[derive(Clone, Copy, Debug)]
pub(super) enum NominalExposure {
    Allowed,
    Denied {
        declaration: Span,
        restrictor: Option<Span>,
    },
}

pub(super) struct QuerySession<'i, 's> {
    tables: &'i Tables<'s>,
    work: &'i WorkMeter,
}
impl<'i, 's> QuerySession<'i, 's> {
    fn requester(&self, requester: ModuleId, at: Span) -> Result<(), Box<Diagnostic>> {
        self.tables.sources.text(at)?;
        if requester.0 >= self.tables.modules.len()
            || self.tables.sources.module_for_file(at.file)? != requester
        {
            return Err(bad(at));
        }
        Ok(())
    }
    fn select(
        &mut self,
        requester: ModuleId,
        path: ItemPathRef,
        type_lane: bool,
    ) -> Result<Option<DeclarationHandle>, Box<Diagnostic>> {
        let at = self.tables.sources.path_span(path)?;
        if requester.0 >= self.tables.modules.len()
            || self.tables.sources.module_for_file(path.file)? != requester
        {
            return Err(bad(at));
        }
        let result = match path.path {
            ast::ItemPath::Unqualified(name) => {
                if let Some(id) = self
                    .tables
                    .lookup_original(requester, type_lane, name, self.work)?
                {
                    Some(DeclarationHandle(id))
                } else if let Some(alias) = self.tables.lookup_alias(requester, name, self.work)? {
                    let cell = self.tables.aliases[alias as usize];
                    let target = if type_lane {
                        cell.type_target
                    } else {
                        cell.value_target
                    };
                    if target == NO_DECLARATION { None } else {
                        if type_lane { self.tables.nominal_handle(target)?; }
                        else { self.tables.function_handle(target)?; }
                        Some(target)
                    }
                } else {
                    None
                }
            }
            ast::ItemPath::Absolute(_) => {
                let (ty, value) = self
                    .tables
                    .absolute_endpoint(requester, path, self.work, false)?;
                if type_lane {
                    ty
                } else {
                    value
                }
            }
        };
        if let Some(id) = result {
            let use_at = match path.path {
                ast::ItemPath::Absolute(_) => *self
                    .tables
                    .sources
                    .segments(path)?
                    .last()
                    .ok_or_else(|| bad(at))?,
                _ => at,
            };
            self.tables
                .access_handle(id, requester, self.work, use_at)?
        }
        Ok(result)
    }
    pub fn value_type(
        &mut self,
        requester: ModuleId,
        ty: ast::TypeSyntax,
        context: TypeContext,
    ) -> Result<ValueTy, Box<Diagnostic>> {
        self.work.debit(1, ty.span, "query value type")?;
        self.requester(requester, ty.span)?;
        match ty.kind {
            ast::TypeSyntaxKind::Array(array) => self
                .array_type(requester, array, ty.span)
                .map(|array| ValueTy::Owned(AggregateTy::FixedArray(array))),
            ast::TypeSyntaxKind::Unit => Ok(ValueTy::Scalar(Ty::Unit)),
            ast::TypeSyntaxKind::Reference { .. }
            | ast::TypeSyntaxKind::ArrayReference { .. }
            | ast::TypeSyntaxKind::SliceReference { .. } => Err(diagnostic(
                if matches!(context, TypeContext::Scalar) {
                    "E0500"
                } else {
                    "E0202"
                },
                "resolve",
                if matches!(context, TypeContext::Scalar) {
                    "owned syntax entered scalar resolution"
                } else {
                    "reference types are restricted to parameters"
                },
                ty.span,
            )),
            ast::TypeSyntaxKind::Name(path) => {
                self.tables.sources.path_span(ItemPathRef {
                    file: ty.span.file,
                    path,
                })?;
                if let ast::ItemPath::Unqualified(name) = path {
                    let spelling = self.tables.sources.text(name)?;
                    if compare_bytes(spelling, "bool", self.work, name)? == Ordering::Equal {
                        return Ok(ValueTy::Scalar(Ty::Bool));
                    }
                    if compare_bytes(spelling, "i32", self.work, name)? == Ordering::Equal {
                        return Ok(ValueTy::Scalar(Ty::I32));
                    }
                }
                self.lookup_nominal_type(
                    requester,
                    &ItemPathRef {
                        file: ty.span.file,
                        path,
                    },
                    context,
                    false,
                )
                .map(|nominal| {
                    ValueTy::Owned(match nominal {
                        NominalId::Record(record) => AggregateTy::Record(record),
                        NominalId::Enum(enumeration) => AggregateTy::Enum(enumeration),
                    })
                })
            }
        }
    }
    /// Structural query only: no nominal row, HIR or executable authority.
    pub fn array_type(
        &mut self,
        requester: ModuleId,
        array: ast::FixedArraySyntax,
        at: Span,
    ) -> Result<FixedArrayTy, Box<Diagnostic>> {
        self.work.debit(1, at, "query array type")?;
        self.requester(requester, at)?;
        self.tables.sources.text(at)?;
        let element = match array.element {
            ast::ScalarTypeSyntax::Bool => Ty::Bool,
            ast::ScalarTypeSyntax::I32 => Ty::I32,
            ast::ScalarTypeSyntax::Unit => Ty::Unit,
        };
        FixedArrayTy::check(element, usize::from(array.length)).map_err(|_| bad(at))
    }
    /// Type-only parameter view. References never become stored value types.
    pub fn parameter_type(
        &mut self,
        requester: ModuleId,
        ty: ast::TypeSyntax,
    ) -> Result<ParameterTy, Box<Diagnostic>> {
        let (mutable, referent) = match ty.kind {
            ast::TypeSyntaxKind::Reference { mutable, referent } => {
                let record = self.record_type(
                    requester,
                    ItemPathRef {
                        file: ty.span.file,
                        path: referent,
                    },
                    TypeContext::Reference,
                )?;
                (mutable, BorrowedTy::Exact(AggregateTy::Record(record)))
            }
            ast::TypeSyntaxKind::ArrayReference { mutable, array } => (
                mutable,
                BorrowedTy::Exact(AggregateTy::FixedArray(
                    self.array_type(requester, array, ty.span)?,
                )),
            ),
            ast::TypeSyntaxKind::SliceReference { mutable, element } => {
                self.work.debit(1, ty.span, "query slice type")?;
                self.requester(requester, ty.span)?;
                self.tables.sources.text(ty.span)?;
                let element = match element {
                    ast::ScalarTypeSyntax::Bool => Ty::Bool,
                    ast::ScalarTypeSyntax::I32 => Ty::I32,
                    ast::ScalarTypeSyntax::Unit => Ty::Unit,
                };
                (mutable, BorrowedTy::ScalarSlice(element))
            }
            _ => {
                return self
                    .value_type(requester, ty, TypeContext::Value)
                    .map(ParameterTy::Value)
            }
        };
        Ok(ParameterTy::Reference {
            referent,
            kind: if mutable {
                BorrowKind::Exclusive
            } else {
                BorrowKind::Shared
            },
        })
    }
    pub fn record_type(
        &mut self,
        requester: ModuleId,
        path: ItemPathRef,
        context: TypeContext,
    ) -> Result<RecordId, Box<Diagnostic>> {
        match self.lookup_nominal_type(requester, &path, context, true)? {
            NominalId::Record(record) => Ok(record),
            NominalId::Enum(_) => Err(bad(self.tables.sources.eof())),
        }
    }
    pub fn nominal_type(
        &mut self,
        requester: ModuleId,
        path: ItemPathRef,
        context: TypeContext,
    ) -> Result<NominalId, Box<Diagnostic>> {
        self.lookup_nominal_type(requester, &path, context, false)
    }
    fn lookup_nominal_type(
        &mut self,
        requester: ModuleId,
        path: &ItemPathRef,
        context: TypeContext,
        records_only: bool,
    ) -> Result<NominalId, Box<Diagnostic>> {
        let at = self.tables.sources.path_span(*path)?;
        self.work.debit(
            1,
            at,
            if records_only || self.tables.enums.is_empty() {
                "query record type"
            } else {
                "query nominal type"
            },
        )?;
        if let Some(id) = self.select(requester, *path, true)? {
            let accepted = match self.tables.project_handle(id)? {
                DeclarationProjection::SourceOriginal(original) => {
                    let kind = self.tables.original(original)?.flags & KIND_MASK;
                    kind == RECORD || (!records_only && kind == ENUM)
                }
                DeclarationProjection::Builtin(BuiltinItem::Enum(_)) => !records_only,
                DeclarationProjection::Builtin(BuiltinItem::Function(_)) => false,
            };
            if accepted {
                let nominal = self.tables.nominal_handle(id)?;
                #[cfg(test)]
                self.work.observe(Observation::Target {
                    operation: match context { TypeContext::Constructor => "constructor-type", TypeContext::Reference => "reference-type", _ => "value-type" },
                    origin: at,
                    kind: match nominal { NominalId::Record(_) => "record", NominalId::Enum(_) => "enum" },
                    id: match nominal { NominalId::Record(id) => id.0, NominalId::Enum(id) => id.0 },
                });
                return Ok(nominal);
            }
        }
        let at = match path.path {
            ast::ItemPath::Absolute(_) => *self
                .tables
                .sources
                .segments(*path)?
                .last()
                .ok_or_else(|| bad(at))?,
            _ => at,
        };
        let spelling = self.tables.sources.text(at)?;
        if matches!(context, TypeContext::Scalar)
            && self.tables.sources.flavor() == SyntaxFlavor::OriginalSingleFile
        {
            return Err(Diagnostic::new(
                "E0202",
                "resolve",
                format!("unknown typed-preview type `{spelling}`; expected bool, i32 or ()"),
                Some(at),
            ));
        }
        let prefix = match context {
            TypeContext::Constructor => "unknown record type",
            TypeContext::Reference => "reference parameter requires a record type, found",
            _ => "unknown type",
        };
        Err(owned_diagnostic::diagnostic(
            "E0202",
            "resolve",
            format_args!("{prefix} `{}`", owned_diagnostic::name(spelling)),
            Some(at),
        ))
    }
    pub fn callee(
        &mut self,
        requester: ModuleId,
        path: ItemPathRef,
        scalar: bool,
    ) -> Result<DefId, Box<Diagnostic>> {
        let at = self.tables.sources.path_span(path)?;
        self.work.debit(1, at, "query callee")?;
        if let Some(id) = self.select(requester, path, false)? {
            let target = self.tables.function_handle(id)?.0;
            #[cfg(test)]
            self.work.observe(Observation::Target {
                operation: "callee",
                origin: at,
                kind: "function",
                id: target,
            });
            return Ok(DefId(target));
        }
        let at = match path.path {
            ast::ItemPath::Absolute(_) => *self
                .tables
                .sources
                .segments(path)?
                .last()
                .ok_or_else(|| bad(at))?,
            _ => at,
        };
        let spelling = self.tables.sources.text(at)?;
        if scalar && self.tables.sources.flavor() == SyntaxFlavor::OriginalSingleFile {
            return Err(Diagnostic::new(
                "E0200",
                "resolve",
                format!("unknown direct function `{spelling}`"),
                Some(at),
            ));
        }
        Err(owned_diagnostic::diagnostic(
            "E0200",
            "resolve",
            format_args!(
                "unknown direct function `{}`",
                owned_diagnostic::name(spelling)
            ),
            Some(at),
        ))
    }
    pub fn qualified_value_endpoint(
        &mut self,
        requester: ModuleId,
        path: QualifiedPathRef,
    ) -> Result<QualifiedValueEndpoint, Box<Diagnostic>> {
        let view = self.tables.sources.qualified_path(path)?;
        let at = view.span();
        self.requester(requester, at)?;
        self.work.debit(1, at, "query qualified value")?;
        let segments = view.segments();
        let member = *segments.last().ok_or_else(|| bad(at))?;
        let enumeration = match view.root() {
            ast::PathRoot::Std => return Err(bad(at)),
            ast::PathRoot::LocalType => {
                let prefix = segments[0];
                self.work.debit(1, prefix, "qualified path segment")?;
                let selected = self.select(
                    requester,
                    ItemPathRef {
                        file: path.file,
                        path: ast::ItemPath::Unqualified(prefix),
                    },
                    true,
                )?;
                let Some(original) = selected else {
                    return Err(owned_diagnostic::diagnostic(
                        "E0202",
                        "resolve",
                        format_args!(
                            "unknown enum type `{}`",
                            owned_diagnostic::name(self.tables.sources.text(prefix)?)
                        ),
                        Some(prefix),
                    ));
                };
                if !matches!(self.tables.nominal_handle(original)?, NominalId::Enum(_)) {
                    return Err(diagnostic(
                        "E0202",
                        "resolve",
                        "variant qualification requires an enum type",
                        prefix,
                    ));
                }
                match self.tables.nominal_handle(original)? {
                    NominalId::Enum(enumeration) => enumeration,
                    NominalId::Record(_) => return Err(bad(prefix)),
                }
            }
            ast::PathRoot::Crate => match self
                .tables
                .absolute_prefix(requester, segments, self.work, true)?
            {
                AbsolutePrefix::Enumeration(original) => {
                    match self.tables.nominal_original(original)? {
                        NominalId::Enum(enumeration) => enumeration,
                        NominalId::Record(_) => return Err(bad(member)),
                    }
                }
                AbsolutePrefix::Module(owner) => {
                    self.work.debit(1, member, "absolute path segment")?;
                    let Some(original) = self
                        .tables
                        .lookup_original(owner, false, member, self.work)?
                    else {
                        return Err(owned_diagnostic::diagnostic(
                            "E0200",
                            "resolve",
                            format_args!(
                                "unknown direct function `{}`",
                                owned_diagnostic::name(self.tables.sources.text(member)?)
                            ),
                            Some(member),
                        ));
                    };
                    self.tables
                        .access_original(original, requester, self.work, member)?;
                    let target = DefId(self.tables.original(original)?.target as usize);
                    #[cfg(test)]
                    self.work.observe(Observation::Target {
                        operation: "callee",
                        origin: at,
                        kind: "function",
                        id: target.0,
                    });
                    return Ok(QualifiedValueEndpoint::Function(target));
                }
            },
        };
        self.work.debit(1, member, "qualified path segment")?;
        let variant = self.tables.lookup_variant(enumeration, member, self.work)?;
        #[cfg(test)]
        self.work.observe(Observation::VariantTarget {
            operation: "qualified-value",
            origin: at,
            variant,
        });
        Ok(QualifiedValueEndpoint::Variant(variant))
    }
    pub fn variant(
        &mut self,
        requester: ModuleId,
        path: QualifiedPathRef,
    ) -> Result<VariantId, Box<Diagnostic>> {
        match self.qualified_value_endpoint(requester, path)? {
            QualifiedValueEndpoint::Variant(variant) => Ok(variant),
            QualifiedValueEndpoint::Function(_) => Err(diagnostic(
                "E0202",
                "resolve",
                "qualified pattern requires an enum variant",
                self.tables.sources.qualified_path(path)?.span(),
            )),
        }
    }
    pub fn value_binding_for_local_conflict(
        &mut self,
        requester: ModuleId,
        name: Span,
    ) -> Result<Option<Span>, Box<Diagnostic>> {
        self.work.debit(1, name, "query local conflict")?;
        if self.tables.sources.module_for_file(name.file)? != requester {
            return Err(bad(name));
        }
        if let Some(id) = self
            .tables
            .lookup_original(requester, false, name, self.work)?
        {
            return Ok(Some(self.tables.original(id)?.name.span()));
        }
        if let Some(group) = self.tables.lookup_alias(requester, name, self.work)? {
            let first = self.tables.aliases[group as usize].value_first_import;
            if first != NONE {
                return Ok(Some(self.tables.import(first)?.alias));
            }
        }
        Ok(None)
    }
    pub fn construction_access(
        &mut self,
        requester: ModuleId,
        record: RecordId,
        at: Span,
    ) -> Result<Access, Box<Diagnostic>> {
        self.requester(requester, at)?;
        self.work.debit(1, at, "construction permission")?;
        let row = self.tables.records.get(record.0).ok_or_else(|| bad(at))?;
        if self
            .tables
            .permission(row.construction_domain, requester, self.work, at)?
        {
            Ok(Access::Allowed)
        } else if row.first_private != NONE {
            Ok(Access::Denied(FieldId {
                record,
                index: row.first_private as usize,
            }))
        } else {
            Err(bad(at))
        }
    }
    pub fn field_access(
        &mut self,
        requester: ModuleId,
        field: FieldId,
        at: Span,
    ) -> Result<Access, Box<Diagnostic>> {
        self.requester(requester, at)?;
        self.work.debit(1, at, "field permission")?;
        let record = self
            .tables
            .records
            .get(field.record.0)
            .ok_or_else(|| bad(at))?;
        if field.index >= record.field_len as usize {
            return Err(bad(at));
        }
        let row = self
            .tables
            .fields
            .get(record.field_start as usize + field.index)
            .ok_or_else(|| bad(at))?;
        Ok(
            if self
                .tables
                .permission(row.domain, requester, self.work, at)?
            {
                Access::Allowed
            } else {
                Access::Denied(field)
            },
        )
    }
    pub fn private_field_diagnostic(
        &self,
        field: FieldId,
        at: Span,
        stage: &'static str,
    ) -> Result<Box<Diagnostic>, Box<Diagnostic>> {
        let record = self
            .tables
            .records
            .get(field.record.0)
            .ok_or_else(|| bad(at))?;
        if field.index >= record.field_len as usize {
            return Err(bad(at));
        }
        let row = self
            .tables
            .fields
            .get(record.field_start as usize + field.index)
            .ok_or_else(|| bad(at))?;
        Ok(owned_diagnostic::secondary(
            diagnostic(
                "E0206",
                stage,
                "record field is private to its declaring module",
                at,
            ),
            row.name.span(),
            format_args!("private declaration here"),
        ))
    }
    pub fn initializer_matches_field(
        &mut self,
        field: FieldId,
        name: Span,
    ) -> Result<bool, Box<Diagnostic>> {
        self.work.debit(1, name, "denied constructor initializer")?;
        let record = self
            .tables
            .records
            .get(field.record.0)
            .ok_or_else(|| bad(name))?;
        if field.index >= record.field_len as usize {
            return Err(bad(name));
        }
        let field = self
            .tables
            .fields
            .get(record.field_start as usize + field.index)
            .ok_or_else(|| bad(name))?;
        Ok(compare_bytes(
            self.tables.sources.text(field.name.span())?,
            self.tables.sources.text(name)?,
            self.work,
            name,
        )? == Ordering::Equal)
    }
    pub fn signature_exposure(
        &mut self,
        function: DefId,
        record: RecordId,
        at: Span,
    ) -> Result<Exposure, Box<Diagnostic>> {
        Ok(
            match self.nominal_signature_exposure(function, NominalId::Record(record), at)? {
                NominalExposure::Allowed => Exposure::Allowed,
                NominalExposure::Denied {
                    declaration,
                    restrictor,
                } => Exposure::Denied {
                    record: declaration,
                    restrictor,
                },
            },
        )
    }
    pub fn nominal_signature_exposure(
        &mut self,
        function: DefId,
        nominal: NominalId,
        at: Span,
    ) -> Result<NominalExposure, Box<Diagnostic>> {
        self.work.debit(1, at, "signature exposure")?;
        let function = self
            .tables
            .functions
            .get(function.0)
            .ok_or_else(|| bad(at))?;
        let handle = self.tables.original_for_nominal(nominal, at)?;
        let f = self.tables.original(function.original)?;
        self.requester(ModuleId(f.owner as usize), at)?;
        let original = match self.tables.project_handle(handle)? {
            DeclarationProjection::Builtin(BuiltinItem::Enum(_)) => return Ok(NominalExposure::Allowed),
            DeclarationProjection::SourceOriginal(id) => id,
            _ => return Err(bad(at)),
        };
        let r = self.tables.original(original)?;
        let allowed =
            r.domain == NONE || (f.domain != NONE && self.tables.ancestor(r.domain, f.domain)?);
        if allowed {
            return Ok(NominalExposure::Allowed);
        }
        let restrictor = if r.restrictor != NONE && r.restrictor != original {
            Some(self.tables.original(r.restrictor)?.name.span())
        } else {
            None
        };
        Ok(NominalExposure::Denied {
            declaration: r.name.span(),
            restrictor,
        })
    }
    pub fn prepare_type_name(
        &mut self,
        record: RecordId,
        at: Span,
    ) -> Result<PreparedTypeName<'s>, Box<Diagnostic>> {
        self.prepare_nominal_type_name(NominalId::Record(record), at)
    }
    pub fn prepare_nominal_type_name(
        &mut self,
        nominal: NominalId,
        at: Span,
    ) -> Result<PreparedTypeName<'s>, Box<Diagnostic>> {
        self.tables.sources.text(at)?;
        self.work.debit(1, at, "nominal format preparation")?;
        let handle = self.tables.original_for_nominal(nominal, at)?;
        let id = match self.tables.project_handle(handle)? {
            DeclarationProjection::Builtin(BuiltinItem::Enum(_)) => {
                self.work.debit(19, at, "nominal display bytes")?;
                return Ok(PreparedTypeName { names: [CompactSpan::default(); 32], sources: self.tables.sources, count: 0, terminal: "ReadStatus",
                    ordinal: self.tables.enums.len(), total: 19, original: false, is_enum: true, builtin: true });
            }
            DeclarationProjection::SourceOriginal(id) => id,
            _ => return Err(bad(at)),
        };
        let original = self.tables.original(id)?;
        let terminal = self.tables.sources.text(original.name.span())?;
        let mut names = [CompactSpan::default(); 32];
        let mut count = 0;
        let mut owner = original.owner;
        while owner != 0 {
            self.work.debit(1, at, "nominal name ancestor")?;
            if count >= 32 {
                return Err(bad(at));
            }
            let module = self
                .tables
                .modules
                .get(owner as usize)
                .ok_or_else(|| bad(at))?;
            let name = self.tables.original(module.original)?.name;
            self.tables.sources.text(name.span())?;
            names[count] = name;
            count += 1;
            owner = module.parent;
        }
        let mut total = 7usize
            .checked_add(terminal.len())
            .ok_or_else(|| overflow(at))?;
        for name in &names[..count] {
            total = total
                .checked_add(2)
                .and_then(|n| n.checked_add(self.tables.sources.text(name.span()).expect("validated prepared name").len()))
                .ok_or_else(|| overflow(at))?;
        }
        let original = self.tables.sources.flavor() == SyntaxFlavor::OriginalSingleFile;
        self.work.debit(
            if original {
                terminal.len().min(64)
            } else {
                total.min(160)
            } as u64,
            at,
            "nominal display bytes",
        )?;
        Ok(PreparedTypeName {
            names,
            sources: self.tables.sources,
            count,
            terminal,
            ordinal: match nominal {
                NominalId::Record(id) => id.0,
                NominalId::Enum(id) => id.0,
            },
            total,
            original,
            is_enum: matches!(nominal, NominalId::Enum(_)),
            builtin: false,
        })
    }
}

#[derive(Debug)]
pub(super) struct PreparedTypeName<'s> {
    names: [CompactSpan; 32],
    sources: SourceOwner<'s>,
    count: usize,
    terminal: &'s str,
    ordinal: usize,
    total: usize,
    original: bool,
    is_enum: bool,
    builtin: bool,
}
impl fmt::Display for PreparedTypeName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.builtin { return f.write_str("std::io::ReadStatus"); }
        if self.original {
            return write!(f, "{}", owned_diagnostic::name(self.terminal));
        }
        if self.total <= 160 {
            f.write_str("crate")?;
            for name in self.names[..self.count].iter().rev() {
                write!(f, "::{}", self.sources.text(name.span()).expect("validated prepared name"))?;
            }
            return write!(f, "::{}", self.terminal);
        }
        let terminal_len = self.terminal.len().min(64);
        let digits = if self.ordinal == 0 {
            1
        } else {
            self.ordinal.ilog10() as usize + 1
        };
        let kind = if self.is_enum { "enum" } else { "record" };
        let suffix_len = 5 + kind.len() + digits;
        let allowance = 160 - terminal_len - suffix_len - 2;
        let module_len = self.total - self.terminal.len() - 2;
        let truncated = module_len > allowance;
        let mut remaining = if truncated { allowance - 3 } else { allowance };
        let mut write_piece = |piece: &str| -> fmt::Result {
            let mut n = remaining.min(piece.len());
            while !piece.is_char_boundary(n) {
                n -= 1;
            }
            f.write_str(&piece[..n])?;
            remaining -= n;
            Ok(())
        };
        write_piece("crate")?;
        for name in self.names[..self.count].iter().rev() {
            write_piece("::")?;
            write_piece(self.sources.text(name.span()).expect("validated prepared name"))?;
        }
        if truncated {
            f.write_str("...")?;
        }
        write!(
            f,
            "::{} [{kind} #{}]",
            owned_diagnostic::name(self.terminal),
            self.ordinal
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct ImportTxn {
    alias: usize,
    seen: usize,
    import: u32,
    // Original-row identity, matching AliasCell.type_target.
    ty: DeclarationHandle,
    value: DeclarationHandle,
}
