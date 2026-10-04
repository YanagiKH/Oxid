//! One private declaration, import and permission implementation for both routes.
//! No linked execution witness is constructed in this module.
#![allow(dead_code)] // Private ProjectCandidate qualification precedes activation.

mod resource;
mod sealed;
mod source_owner;
pub(super) use source_owner::SourceOwner;
#[cfg(test)]
mod tests;
use super::{
    ast,
    diagnostic::Diagnostic,
    hir::{DefId, Ty},
    oir::owned_types::{
        AggregateTy, BorrowKind, FieldId, FixedArrayTy, ParameterTy, RecordId, ValueTy,
    },
    owned_diagnostic,
    project::{
        budget::{Allocator, ReserveFailure},
        FunctionAstKey, ItemPathRef, ModuleId, ProjectSources, RecordAstKey, SyntaxFlavor,
    },
    source::{SourceFile, SourceFileId, SourceView, Span},
};
use resource::{add, allocate, compact, compare_bytes, merge_sort};
#[cfg(test)]
pub(super) use resource::{AliasObservation, Observation, SeenObservation};
pub(super) use resource::{Counts, IndexLimits, IndexPlan, WorkMeter};
pub(super) use sealed::{collect_originals, DeclarationFacts, DeclarationIndex};
use std::{cmp::Ordering, fmt, mem::size_of};

const NONE: u32 = u32::MAX;
const BUILTIN_CONFLICT: u32 = u32::MAX - 1;
const FUNCTION: u32 = 0;
const RECORD: u32 = 1;
const MODULE: u32 = 2;
const PUBLIC: u32 = 4;

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
struct ModuleRow {
    parent: u32,
    subtree_end: u32,
    original: u32,
    function_base: u32,
    function_len: u32,
    record_base: u32,
    record_len: u32,
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
    type_target: u32,
    value_target: u32,
    type_first_import: u32,
    value_first_import: u32,
}
impl Default for AliasCell {
    fn default() -> Self {
        Self {
            type_target: NONE,
            value_target: NONE,
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
    assert!(size_of::<ModuleRow>() == 60);
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
    modules: Vec<ModuleRow>,
    children: Vec<u32>,
    imports: Vec<ImportRow>,
    aliases: Vec<AliasCell>,
    alias_order: Vec<u32>,
    root_main: u32,
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
// counters, ranges and formatter arithmetic; row copies and association handles
// are listed separately. Disjoint build/query/format phases are deliberately
// summed. Test-only event Vec headers/payloads are excluded.
const FIXED_SCRATCH: usize = size_of::<Scratch>()
    + size_of::<IndexPlan>()
    + size_of::<Counts>()
    + size_of::<Tables<'static>>()
    + size_of::<[u32; 33]>() * 3
    + size_of::<ImportTxn>()
    + size_of::<PreparedTypeName<'static>>() * 2
    + size_of::<[usize; 128]>()
    + size_of::<[u64; 2]>()
    + size_of::<IndexLimits>()
    + size_of::<SourceOwner<'static>>()
    + size_of::<QuerySession<'static, 'static>>()
    + size_of::<OriginalRow>()
    + size_of::<ModuleRow>()
    + size_of::<FunctionRow>()
    + size_of::<RecordRow>()
    + size_of::<FieldRow>()
    + size_of::<ImportRow>()
    + size_of::<AliasCell>()
    + size_of::<SeenCell>()
    + size_of::<[Span; 4]>();
const _: () = assert!(FIXED_SCRATCH <= 4096);

impl Tables<'_> {
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
        let numeric = (a.owner, a.flags & 3 != FUNCTION).cmp(&(b.owner, b.flags & 3 != FUNCTION));
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
        let a = self.sources.segments(ItemPathRef {
            file: SourceFileId(ra.file as usize),
            path: ast::ItemPath::Absolute(self.import(a)?.path),
        })?;
        let b = self.sources.segments(ItemPathRef {
            file: SourceFileId(rb.file as usize),
            path: ast::ItemPath::Absolute(self.import(b)?.path),
        })?;
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
                (row.owner as usize, row.flags & 3 != FUNCTION).cmp(&(module.0, type_lane));
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
    fn absolute_endpoint(
        &self,
        requester: ModuleId,
        path: ItemPathRef,
        work: &WorkMeter,
        importing: bool,
    ) -> Result<(Option<u32>, Option<u32>), Box<Diagnostic>> {
        let segments = self.sources.segments(path)?;
        let mut owner = ModuleId(0);
        for &segment in &segments[1..segments.len() - 1] {
            work.debit(1, segment, "absolute path segment")?;
            let found = self.lookup_original(owner, true, segment, work)?;
            let Some(id) = found else {
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
            if row.flags & 3 != MODULE {
                return Err(diagnostic(
                    "E0205",
                    "resolve",
                    "invalid absolute item path",
                    segment,
                ));
            }
            self.access_original(id, requester, work, segment)?;
            owner = ModuleId(row.target as usize);
        }
        let endpoint = *segments.last().ok_or_else(|| bad(self.sources.eof()))?;
        work.debit(1, endpoint, "absolute path segment")?;
        let original_type = self.lookup_original(owner, true, endpoint, work)?;
        let ty = original_type.filter(|id| self.originals[*id as usize].flags & 3 == RECORD);
        let value = self.lookup_original(owner, false, endpoint, work)?;
        if importing && ty.is_none() && value.is_none() {
            let mut error = diagnostic(
                "E0205",
                "resolve",
                "absolute path endpoint is not an original function or record",
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
        Ok((ty, value))
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
    ) -> Result<Option<u32>, Box<Diagnostic>> {
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
                    Some(id)
                } else if let Some(alias) = self.tables.lookup_alias(requester, name, self.work)? {
                    let cell = self.tables.aliases[alias as usize];
                    let target = if type_lane {
                        cell.type_target
                    } else {
                        cell.value_target
                    };
                    if target == NONE {
                        None
                    } else {
                        Some(if type_lane {
                            self.tables.records[target as usize].original
                        } else {
                            self.tables.functions[target as usize].original
                        })
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
                .access_original(id, requester, self.work, use_at)?
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
            ast::TypeSyntaxKind::Reference { .. } | ast::TypeSyntaxKind::ArrayReference { .. } => {
                Err(diagnostic(
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
                ))
            }
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
                self.record_type(
                    requester,
                    ItemPathRef {
                        file: ty.span.file,
                        path,
                    },
                    context,
                )
                .map(|record| ValueTy::Owned(AggregateTy::Record(record)))
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
        let (mutable, aggregate) = match ty.kind {
            ast::TypeSyntaxKind::Reference { mutable, referent } => {
                let record = self.record_type(
                    requester,
                    ItemPathRef {
                        file: ty.span.file,
                        path: referent,
                    },
                    TypeContext::Reference,
                )?;
                (mutable, AggregateTy::Record(record))
            }
            ast::TypeSyntaxKind::ArrayReference { mutable, array } => (
                mutable,
                AggregateTy::FixedArray(self.array_type(requester, array, ty.span)?),
            ),
            _ => {
                return self
                    .value_type(requester, ty, TypeContext::Value)
                    .map(ParameterTy::Value)
            }
        };
        Ok(ParameterTy::Reference {
            aggregate,
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
        let at = self.tables.sources.path_span(path)?;
        self.work.debit(1, at, "query record type")?;
        if let Some(id) = self.select(requester, path, true)? {
            let row = self.tables.original(id)?;
            if row.flags & 3 == RECORD {
                #[cfg(test)]
                self.work.observe(Observation::Target {
                    operation: match context {
                        TypeContext::Constructor => "constructor-type",
                        TypeContext::Reference => "reference-type",
                        _ => "value-type",
                    },
                    origin: at,
                    kind: "record",
                    id: row.target as usize,
                });
                return Ok(RecordId(row.target as usize));
            }
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
            let target = self.tables.original(id)?.target as usize;
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
        self.work.debit(1, at, "signature exposure")?;
        let function = self
            .tables
            .functions
            .get(function.0)
            .ok_or_else(|| bad(at))?;
        let record = self.tables.records.get(record.0).ok_or_else(|| bad(at))?;
        let (f, r) = (
            self.tables.original(function.original)?,
            self.tables.original(record.original)?,
        );
        self.requester(ModuleId(f.owner as usize), at)?;
        let allowed =
            r.domain == NONE || (f.domain != NONE && self.tables.ancestor(r.domain, f.domain)?);
        if allowed {
            return Ok(Exposure::Allowed);
        }
        let restrictor = if r.restrictor != NONE && r.restrictor != record.original {
            Some(self.tables.original(r.restrictor)?.name.span())
        } else {
            None
        };
        Ok(Exposure::Denied {
            record: r.name.span(),
            restrictor,
        })
    }
    pub fn prepare_type_name(
        &mut self,
        record: RecordId,
        at: Span,
    ) -> Result<PreparedTypeName<'s>, Box<Diagnostic>> {
        self.tables.sources.text(at)?;
        self.work.debit(1, at, "nominal format preparation")?;
        let row = self.tables.records.get(record.0).ok_or_else(|| bad(at))?;
        let original = self.tables.original(row.original)?;
        let terminal = self.tables.sources.text(original.name.span())?;
        let mut names = [""; 33];
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
            names[count] = self
                .tables
                .sources
                .text(self.tables.original(module.original)?.name.span())?;
            count += 1;
            owner = module.parent;
        }
        let mut total = 7usize
            .checked_add(terminal.len())
            .ok_or_else(|| overflow(at))?;
        for name in &names[..count] {
            total = total
                .checked_add(2)
                .and_then(|n| n.checked_add(name.len()))
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
            count,
            terminal,
            record: record.0,
            total,
            original,
        })
    }
}

#[derive(Debug)]
pub(super) struct PreparedTypeName<'s> {
    names: [&'s str; 33],
    count: usize,
    terminal: &'s str,
    record: usize,
    total: usize,
    original: bool,
}
impl fmt::Display for PreparedTypeName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.original {
            return write!(f, "{}", owned_diagnostic::name(self.terminal));
        }
        if self.total <= 160 {
            f.write_str("crate")?;
            for name in self.names[..self.count].iter().rev() {
                write!(f, "::{name}")?;
            }
            return write!(f, "::{}", self.terminal);
        }
        let terminal_len = self.terminal.len().min(64);
        let digits = if self.record == 0 {
            1
        } else {
            self.record.ilog10() as usize + 1
        };
        let suffix_len = 11 + digits;
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
            write_piece(name)?;
        }
        if truncated {
            f.write_str("...")?;
        }
        write!(
            f,
            "::{} [record #{}]",
            owned_diagnostic::name(self.terminal),
            self.record
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct ImportTxn {
    alias: usize,
    seen: usize,
    import: u32,
    ty: u32,
    value: u32,
}
