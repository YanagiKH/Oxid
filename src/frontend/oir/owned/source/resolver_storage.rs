//! Construction-local storage helpers for the closed resolver-only probe.
//!
//! Nothing here grants source admission. The ordinary resolver installs only
//! `legacy`; the paid consumer is deliberately not connected in this checkpoint.
#![allow(dead_code)]

use super::{
    hir::*,
    hir_budget::{Capacity, HirCounts, HirPlan, ScopeName},
    resolve::ResolveFrame,
};
use crate::frontend::{
    ast,
    declaration_index::{DeclarationIndex, WorkMeter},
    diagnostic::Diagnostic,
    owned_diagnostic,
    project::budget::Allocator,
    source::Span,
};
use std::{cmp::Ordering, mem::size_of};

pub(super) const KINDS: usize = 17;
pub(super) const RETAINED_KINDS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(super) enum Kind {
    Records,
    Fields,
    Signatures,
    Parameters,
    Functions,
    Bindings,
    Expressions,
    Blocks,
    Statements,
    Arguments,
    FieldInitializers,
    ArrayEntries,
    Names,
    Exits,
    Marks,
    Loops,
    Frames,
}
impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Records => "paid HIR records",
            Self::Fields => "paid HIR record fields",
            Self::Signatures => "paid HIR signatures",
            Self::Parameters => "paid HIR parameters",
            Self::Functions => "paid HIR functions",
            Self::Bindings => "paid HIR bindings",
            Self::Expressions => "paid HIR expressions",
            Self::Blocks => "paid HIR blocks",
            Self::Statements => "paid HIR statements",
            Self::Arguments => "paid HIR arguments",
            Self::FieldInitializers => "paid HIR field initializers",
            Self::ArrayEntries => "paid HIR array entries",
            Self::Names => "paid HIR scope names",
            Self::Exits => "paid HIR scope exits",
            Self::Marks => "paid HIR scope marks",
            Self::Loops => "paid HIR loops",
            Self::Frames => "paid HIR resolve frames",
        }
    }
}
pub(super) const WIDTHS: [usize; KINDS] = [
    size_of::<Record>(),
    size_of::<Field>(),
    size_of::<Signature>(),
    size_of::<ParameterTy>(),
    size_of::<Function>(),
    size_of::<Binding>(),
    size_of::<Expr>(),
    size_of::<BodyBlock>(),
    size_of::<Stmt>(),
    size_of::<Argument>(),
    size_of::<FieldInit>(),
    size_of::<ExprId>(),
    size_of::<ScopeName>(),
    size_of::<usize>(),
    size_of::<usize>(),
    size_of::<LoopId>(),
    size_of::<ResolveFrame>(),
];
fn limits(c: HirCounts) -> [usize; KINDS] {
    [
        c.records,
        c.record_fields,
        c.functions,
        c.parameters,
        c.functions,
        c.bindings,
        c.expressions,
        c.blocks,
        c.statements,
        c.call_arguments,
        c.field_initializers,
        c.array_entries,
        c.bindings,
        c.bindings,
        c.scope_marks,
        c.loop_slots,
        c.resolve_frames,
    ]
}
pub(super) fn failure(message: &'static str, at: Span) -> Box<Diagnostic> {
    owned_diagnostic::diagnostic("E0400", "resolve", format_args!("{message}"), Some(at))
}
fn invalid(at: Span) -> Box<Diagnostic> {
    owned_diagnostic::diagnostic(
        "E0500",
        "resolve",
        format_args!("invalid paid resolver storage state"),
        Some(at),
    )
}
fn add(left: usize, right: usize, at: Span) -> Result<usize, Box<Diagnostic>> {
    left.checked_add(right)
        .ok_or_else(|| failure("affected HIR size overflow", at))
}
/// Checks capacity without accepting/moving the row: callers retain ordinary
/// push ordering, and a paid append cannot trigger implicit vector growth.
pub(super) fn room<T>(
    values: &[T],
    capacity: usize,
    paid: bool,
    at: Span,
) -> Result<(), Box<Diagnostic>> {
    if paid && values.len() >= capacity {
        Err(failure("affected HIR append exceeds prepaid capacity", at))
    } else {
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct PaidStorage {
    remaining: [usize; KINDS],
    reserved: [usize; KINDS],
    // Cumulative actual scratch capacities, sampled at function lifetime ends.
    scratch: [usize; KINDS - RETAINED_KINDS],
}
impl PaidStorage {
    pub(super) fn new(counts: HirCounts) -> Self {
        Self {
            remaining: limits(counts),
            reserved: [0; KINDS],
            scratch: [0; KINDS - RETAINED_KINDS],
        }
    }
    pub(super) fn reserve<T>(
        &mut self,
        allocator: &mut Allocator,
        kind: Kind,
        slots: usize,
        at: Span,
    ) -> Result<Vec<T>, Box<Diagnostic>> {
        let k = kind as usize;
        if size_of::<T>() != WIDTHS[k] {
            return Err(invalid(at));
        }
        let remaining = self.remaining[k]
            .checked_sub(slots)
            .ok_or_else(|| failure("affected HIR kind quota exhausted", at))?;
        let bytes = slots
            .checked_mul(size_of::<T>())
            .ok_or_else(|| failure("affected HIR size overflow", at))?;
        let ticket = Capacity::new::<T>(slots, bytes, at)?;
        // Consume before the observed fallible reserve. A failed construction is
        // dropped; its quota is never refunded and reused in the same attempt.
        self.remaining[k] = remaining;
        let values = ticket.reserve(allocator, Vec::new(), at, kind.label())?;
        self.reserved[k] = add(self.reserved[k], values.capacity(), at)?;
        Ok(values)
    }
    pub(super) fn observe_scratch(
        &mut self,
        scope: &PaidScope,
        loops: &Vec<LoopId>,
        frames: &Vec<ResolveFrame>,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        let capacities = [
            scope.names.capacity(),
            scope.exits.capacity(),
            scope.marks.capacity(),
            loops.capacity(),
            frames.capacity(),
        ];
        for (offset, capacity) in capacities.into_iter().enumerate() {
            let total = add(self.scratch[offset], capacity, at)?;
            if total > self.reserved[RETAINED_KINDS + offset] {
                return Err(failure(
                    "affected HIR scratch exceeds observed reserves",
                    at,
                ));
            }
            self.scratch[offset] = total;
        }
        Ok(())
    }
}

/// Fixed observation only. No HIR values, source references or owner can escape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ResolverStorageObservation {
    pub(super) plan: HirPlan,
    pub(super) retained_counts: [usize; RETAINED_KINDS],
    pub(super) capacities: [usize; KINDS],
    pub(super) retained_bytes: usize,
    pub(super) scratch_capacity_bytes: usize,
    pub(super) reservation_attempts: usize,
}

#[derive(Debug)]
pub(super) struct PaidScope {
    names: Vec<ScopeName>,
    exits: Vec<usize>,
    marks: Vec<usize>,
}
/// Only the temporary resolver owns this policy. No program witness gains it.
pub(super) struct ResolverStorage<'a> {
    pub(super) paid: Option<&'a mut PaidStorage>,
    pub(super) scope: Option<PaidScope>,
}
impl ResolverStorage<'_> {
    pub(super) fn legacy() -> Self {
        Self {
            paid: None,
            scope: None,
        }
    }
}

/// Lexicographic comparison charges before inspecting each source byte and can
/// fail in sort/lookup/duplicate scans, including comparisons of long prefixes.
pub(super) fn compare(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    left: Span,
    right: Span,
) -> Result<Ordering, Box<Diagnostic>> {
    let sources = index.sources();
    let left = sources.text(left)?.as_bytes();
    let right_bytes = sources.text(right)?.as_bytes();
    for (&a, &b) in left.iter().zip(right_bytes) {
        work.debit(1, right, "paid resolver name byte")?;
        match a.cmp(&b) {
            Ordering::Equal => (),
            other => return Ok(other),
        }
    }
    work.debit(1, right, "paid resolver name length")?;
    Ok(left.len().cmp(&right_bytes.len()))
}
impl PaidScope {
    pub(super) fn new(
        index: &DeclarationIndex<'_>,
        work: &WorkMeter,
        function: &ast::Function,
        counts: &HirCounts,
        storage: &mut PaidStorage,
        allocator: &mut Allocator,
    ) -> Result<Self, Box<Diagnostic>> {
        let at = function.name;
        let mut names = storage.reserve(allocator, Kind::Names, counts.bindings, at)?;
        let exits = storage.reserve(allocator, Kind::Exits, counts.bindings, at)?;
        let marks = storage.reserve(allocator, Kind::Marks, counts.scope_marks, at)?;
        for parameter in &function.params {
            work.debit(1, parameter.name, "paid resolver name inventory")?;
            room(&names, names.capacity(), true, parameter.name)?;
            names.push(ScopeName {
                name: parameter.name,
                active: None,
            });
        }
        for block in &function.blocks {
            for statement in &block.body {
                work.debit(1, statement.span, "paid resolver name inventory")?;
                if let ast::StmtKind::Let { name, .. } = statement.kind {
                    room(&names, names.capacity(), true, name)?;
                    names.push(ScopeName { name, active: None });
                }
            }
        }
        // Fallible in-place heapsort. Neither sort nor duplicate coalescing
        // changes capacity; the original pre-dedup request remains paid.
        let len = names.len();
        for root in (0..len / 2).rev() {
            Self::sift(&mut names, root, len, index, work)?;
        }
        for end in (1..len).rev() {
            names.swap(0, end);
            Self::sift(&mut names, 0, end, index, work)?;
        }
        let mut written = 0;
        for read in 0..len {
            if written == 0
                || compare(index, work, names[written - 1].name, names[read].name)?
                    != Ordering::Equal
            {
                names[written] = names[read];
                written += 1;
            }
        }
        names.truncate(written);
        Ok(Self {
            names,
            exits,
            marks,
        })
    }
    fn sift(
        names: &mut [ScopeName],
        mut root: usize,
        end: usize,
        index: &DeclarationIndex<'_>,
        work: &WorkMeter,
    ) -> Result<(), Box<Diagnostic>> {
        while root < end / 2 {
            // root < end/2 proves 2*root+1 < end, with no arithmetic overflow.
            let mut child = root * 2 + 1;
            if child + 1 < end
                && compare(index, work, names[child].name, names[child + 1].name)? == Ordering::Less
            {
                child += 1;
            }
            if compare(index, work, names[root].name, names[child].name)? != Ordering::Less {
                break;
            }
            names.swap(root, child);
            root = child;
        }
        Ok(())
    }
    pub(super) fn row(
        &self,
        index: &DeclarationIndex<'_>,
        work: &WorkMeter,
        name: Span,
    ) -> Result<Option<usize>, Box<Diagnostic>> {
        let mut first = 0;
        let mut end = self.names.len();
        while first < end {
            let middle = first + (end - first) / 2;
            match compare(index, work, self.names[middle].name, name)? {
                Ordering::Less => first = middle + 1,
                Ordering::Greater => end = middle,
                Ordering::Equal => return Ok(Some(middle)),
            }
        }
        Ok(None)
    }
    pub(super) fn active(
        &self,
        index: &DeclarationIndex<'_>,
        work: &WorkMeter,
        name: Span,
    ) -> Result<Option<BindingId>, Box<Diagnostic>> {
        Ok(self
            .row(index, work, name)?
            .and_then(|row| self.names[row].active))
    }
    pub(super) fn activate(
        &mut self,
        row: usize,
        binding: BindingId,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        let name = self.names.get_mut(row).ok_or_else(|| invalid(at))?;
        if name.active.is_some() {
            return Err(invalid(at));
        }
        room(&self.exits, self.exits.capacity(), true, at)?;
        name.active = Some(binding);
        self.exits.push(row);
        Ok(())
    }
    pub(super) fn enter(&mut self, at: Span) -> Result<(), Box<Diagnostic>> {
        room(&self.marks, self.marks.capacity(), true, at)?;
        self.marks.push(self.exits.len());
        Ok(())
    }
    pub(super) fn leave(&mut self, work: &WorkMeter, at: Span) -> Result<(), Box<Diagnostic>> {
        let mark = self.marks.pop().ok_or_else(|| invalid(at))?;
        if mark > self.exits.len() {
            return Err(invalid(at));
        }
        while self.exits.len() > mark {
            work.debit(1, at, "paid resolver scope exit")?;
            let row = self.exits.pop().ok_or_else(|| invalid(at))?;
            self.names.get_mut(row).ok_or_else(|| invalid(at))?.active = None;
        }
        Ok(())
    }
}

// Conservative named-carrier envelopes, not exact simultaneous live bytes or
// machine-stack/RSS bounds. Complete enclosing Result/Option payloads count once;
// separately named constructor, transfer and caller values are explicit copies.
type ResolvedParts = (Vec<Record>, Vec<Signature>, Vec<Function>);
struct FixedCarriers {
    storage: [PaidStorage; 2],
    // Shared implementation argument and probe's temporary Some borrow. These
    // are distinct from the policy embedded in each complete Resolver.
    shared_paid_borrows: [Option<&'static mut PaidStorage>; 2],
    type_guard_borrow: &'static ValueTy,
    counts: [HirCounts; 2],
    quota_arrays: [[usize; KINDS]; 4],
    // The paid wrapper adds one nested vector-return layer above Capacity.
    reserve_return: [u8; super::hir_budget::VECTOR_RETURN_ENVELOPE_BYTES],
    reserve_scalar_returns: [Result<usize, Box<Diagnostic>>; 3],
    observation: [ResolverStorageObservation; 2],
    observation_option: Option<ResolverStorageObservation>,
    returned: Result<Option<ResolverStorageObservation>, Vec<Diagnostic>>,
    parts: ResolvedParts,
    parts_return: Result<ResolvedParts, Vec<Diagnostic>>,
}
struct FunctionCarriers {
    counts: HirCounts,
    optional_counts: Option<HirCounts>,
    counts_return: Result<HirCounts, Box<Diagnostic>>,
    // Scope's three builder headers, constructed Self and full return carrier.
    builder_names: Vec<ScopeName>,
    builder_exits: Vec<usize>,
    builder_marks: Vec<usize>,
    scope: PaidScope,
    scope_return: Result<PaidScope, Box<Diagnostic>>,
    scope_option: Option<PaidScope>,
    // Sorting/lookup/bind chain: source spans, row copies and fallible results.
    names: [ScopeName; 2],
    comparison: Result<Ordering, Box<Diagnostic>>,
    row: Result<Option<usize>, Box<Diagnostic>>,
    // PaidScope::active and Resolver::active each have a complete return slot;
    // forwarding does not assume that the compiler reuses either carrier.
    active: [Result<Option<BindingId>, Box<Diagnostic>>; 2],
    scalar_returns: [Result<(), Box<Diagnostic>>; 5],
    capacity_array: [usize; KINDS - RETAINED_KINDS],
    legacy_scope_header: Vec<Vec<&'static str>>,
    binding_row: Option<usize>,
}
struct ArgumentCarriers {
    row: Argument,
    returned: Result<Argument, Box<Diagnostic>>,
}
pub(super) const fn argument_carrier_bytes() -> usize {
    size_of::<ArgumentCarriers>()
}
// The selected identity remains live while recursively resolving its value.
// Retain both the candidate and complete fallible lookup return; do not infer
// return-slot reuse or price only the reference that the old resolver held.
struct LiteralLookupCarriers {
    candidate: Option<&'static Field>,
    selected: FieldId,
    returned: Result<&'static Field, Box<Diagnostic>>,
}
pub(super) const fn literal_lookup_carrier_bytes() -> usize {
    size_of::<LiteralLookupCarriers>()
}
pub(super) const fn fixed_carrier_bytes() -> usize {
    size_of::<FixedCarriers>()
}
pub(super) const fn function_carrier_bytes() -> usize {
    size_of::<FunctionCarriers>()
}

#[cfg(test)]
#[path = "resolver_storage_tests.rs"]
mod tests;
