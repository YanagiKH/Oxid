//! Construction-local storage for the test-only resolver statistics probe.
//!
//! Nothing here grants source admission. Ordinary resolution keeps its absent
//! paid policy; private observations retain no resolved owner or typed witness.
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

/// Independently read initialized rows and backing capacities from completed
/// private parts. This is neither a typecheck nor a source/ownership witness.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ResolverInventory {
    counts: [usize; RETAINED_KINDS],
    capacities: [usize; KINDS],
}
impl ResolverInventory {
    fn vector<T>(
        &mut self,
        kind: Kind,
        values: &Vec<T>,
        work: &WorkMeter,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        let k = kind as usize;
        if k >= RETAINED_KINDS || WIDTHS[k] != size_of::<T>() {
            return Err(invalid(at));
        }
        work.debit(1, at, "paid resolver inventory vector")?;
        self.counts[k] = add(self.counts[k], values.len(), at)?;
        self.capacities[k] = add(self.capacities[k], values.capacity(), at)?;
        Ok(())
    }
}
fn inventory_value(ty: &ValueTy, at: Span) -> Result<(), Box<Diagnostic>> {
    if matches!(ty, ValueTy::Owned(AggregateTy::Enum(_))) {
        Err(invalid(at))
    } else {
        Ok(())
    }
}
/// No copied rows, auxiliary tables, recursion or heap allocation on success.
/// Charge before every vector/row inspected, and retain only scalar totals.
pub(super) fn inventory_parts(
    parts: &ResolvedParts,
    work: &WorkMeter,
    at: Span,
) -> Result<ResolverInventory, Box<Diagnostic>> {
    let (records, signatures, functions) = parts;
    let mut inventory = ResolverInventory::default();
    inventory.vector(Kind::Records, records, work, at)?;
    inventory.vector(Kind::Signatures, signatures, work, at)?;
    inventory.vector(Kind::Functions, functions, work, at)?;
    for record in records {
        work.debit(1, record.span, "paid resolver inventory row")?;
        inventory.vector(Kind::Fields, &record.fields, work, record.span)?;
        for field in &record.fields {
            work.debit(1, field.span, "paid resolver inventory row")?;
            inventory_value(&field.ty, field.span)?;
        }
    }
    for signature in signatures {
        work.debit(1, signature.span, "paid resolver inventory row")?;
        inventory_value(&signature.result, signature.span)?;
        inventory.vector(Kind::Parameters, &signature.params, work, signature.span)?;
        for parameter in &signature.params {
            work.debit(1, signature.span, "paid resolver inventory row")?;
            match parameter {
                ParameterTy::Value(value) => inventory_value(value, signature.span)?,
                ParameterTy::Reference {
                    referent: BorrowedTy::Exact(AggregateTy::Enum(_)),
                    ..
                } => {
                    return Err(invalid(signature.span));
                }
                ParameterTy::Reference { .. } => (),
            }
        }
    }
    for function in functions {
        work.debit(1, function.end, "paid resolver inventory row")?;
        inventory.vector(Kind::Bindings, &function.bindings, work, function.end)?;
        inventory.vector(Kind::Expressions, &function.expressions, work, function.end)?;
        inventory.vector(Kind::Blocks, &function.blocks, work, function.end)?;
        for binding in &function.bindings {
            work.debit(1, binding.span, "paid resolver inventory row")?;
            if let Some(annotation) = &binding.annotation {
                inventory_value(annotation, binding.span)?;
            }
        }
        for expression in &function.expressions {
            work.debit(1, expression.span, "paid resolver inventory row")?;
            match &expression.kind {
                ExprKind::ConstructEnum { .. } => return Err(invalid(expression.span)),
                ExprKind::Call { args, .. } => {
                    inventory.vector(Kind::Arguments, args, work, expression.span)?
                }
                ExprKind::StructLiteral { fields, .. } => {
                    inventory.vector(Kind::FieldInitializers, fields, work, expression.span)?
                }
                ExprKind::ArrayLiteral { elements } => {
                    inventory.vector(Kind::ArrayEntries, elements, work, expression.span)?
                }
                _ => (),
            }
        }
        for block in &function.blocks {
            work.debit(1, block.span, "paid resolver inventory row")?;
            inventory.vector(Kind::Statements, &block.body, work, block.span)?;
            for statement in &block.body {
                work.debit(1, statement.span, "paid resolver inventory row")?;
                if matches!(statement.kind, StmtKind::Match { .. }) {
                    return Err(invalid(statement.span));
                }
            }
        }
    }
    Ok(inventory)
}
impl PaidStorage {
    /// Reconcile facts read from real vectors, never manufacture them from the
    /// plan or remaining quotas. This helper grants no source/consumer admission.
    pub(super) fn reconcile(
        &self,
        plan: &HirPlan,
        mut inventory: ResolverInventory,
        reservation_attempts: usize,
        work: &WorkMeter,
        at: Span,
    ) -> Result<ResolverStorageObservation, Box<Diagnostic>> {
        let bounds = limits(plan.counts);
        let mut retained_bytes = 0usize;
        let mut scratch_capacity_bytes = 0usize;
        for k in 0..KINDS {
            work.debit(1, at, "paid resolver inventory reconcile")?;
            if k < RETAINED_KINDS {
                if inventory.counts[k] > bounds[k] || inventory.counts[k] > inventory.capacities[k]
                {
                    return Err(failure(
                        "affected HIR initialized rows exceed admitted capacity",
                        at,
                    ));
                }
            } else {
                // Only the actual once-per-function endpoint samples can fill
                // scratch slots. An inventory caller cannot supply substitutes.
                if inventory.capacities[k] != 0 {
                    return Err(invalid(at));
                }
                inventory.capacities[k] = self.scratch[k - RETAINED_KINDS];
            }
            let capacity = inventory.capacities[k];
            let consumed = bounds[k]
                .checked_sub(self.remaining[k])
                .ok_or_else(|| invalid(at))?;
            if capacity != self.reserved[k] || capacity > bounds[k] || capacity > consumed {
                return Err(failure(
                    "affected HIR actual capacity does not match paid reserves",
                    at,
                ));
            }
            let bytes = capacity
                .checked_mul(WIDTHS[k])
                .ok_or_else(|| failure("affected HIR size overflow", at))?;
            if k < RETAINED_KINDS {
                retained_bytes = add(retained_bytes, bytes, at)?;
            } else {
                scratch_capacity_bytes = add(scratch_capacity_bytes, bytes, at)?;
            }
        }
        if retained_bytes > plan.resolved || scratch_capacity_bytes > plan.resolver_scratch {
            return Err(failure(
                "affected HIR observed payload exceeds prepaid envelope",
                at,
            ));
        }
        Ok(ResolverStorageObservation {
            plan: *plan,
            retained_counts: inventory.counts,
            capacities: inventory.capacities,
            retained_bytes,
            scratch_capacity_bytes,
            reservation_attempts,
        })
    }
}

/// Fixed observation only. No HIR values, source references or owner can escape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ResolverStorageObservation {
    pub(super) plan: HirPlan,
    pub(super) retained_counts: [usize; RETAINED_KINDS],
    pub(super) capacities: [usize; KINDS],
    /// Actual retained vector backing payload; excludes named fixed envelopes.
    pub(super) retained_bytes: usize,
    /// Cumulative actual scratch backing payload, not simultaneous peak bytes.
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
    // Source deny_checkpoint_enum and post-resolution inventory_value execute
    // in disjoint phases and share this named value-guard borrow bank.
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
// Inner branch-local vectors are named separately from the old outer pending
// vectors. Moving headers does not authorize assuming compiler slot reuse.
struct FunctionBranchHeaders {
    parameters: Vec<ParameterTy>,
    blocks: Vec<BodyBlock>,
    frames: Vec<ResolveFrame>,
}
pub(super) const fn function_branch_header_bytes() -> usize {
    size_of::<FunctionBranchHeaders>()
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
// The builder, its caller and reconciliation's by-value input are separate
// named values. Whole fallible returns include their payload exactly once.
struct InventoryInputs {
    plan: &'static HirPlan,
    parts: &'static ResolvedParts,
    records: &'static Vec<Record>,
    signatures: &'static Vec<Signature>,
    functions: &'static Vec<Function>,
    vector: &'static Vec<Expr>,
    accumulator: &'static mut ResolverInventory,
    paid: &'static PaidStorage,
    work: [&'static WorkMeter; 2],
    // inventory_parts and its nested vector/value helper have distinct inputs.
    origins: [Span; 2],
    kind: Kind,
}
// A Vec borrow is not a slice iterator. Price each newly introduced named
// loop's cursor, complete next result and current borrowed row explicitly.
// Summing these disjoint loop states is conservative; nesting is at most three.
struct InventorySliceLoop<T: 'static> {
    cursor: std::slice::Iter<'static, T>,
    next: Option<&'static T>,
    current: &'static T,
}
struct InventoryWalkCarriers {
    records: InventorySliceLoop<Record>,
    fields: InventorySliceLoop<Field>,
    signatures: InventorySliceLoop<Signature>,
    parameters: InventorySliceLoop<ParameterTy>,
    functions: InventorySliceLoop<Function>,
    bindings: InventorySliceLoop<Binding>,
    expressions: InventorySliceLoop<Expr>,
    blocks: InventorySliceLoop<BodyBlock>,
    statements: InventorySliceLoop<Stmt>,
    reconcile_cursor: std::ops::Range<usize>,
    reconcile_next: Option<usize>,
    reconcile_current: usize,
}
struct InventoryCarriers {
    inventories: [ResolverInventory; 3],
    inventory_return: Result<ResolverInventory, Box<Diagnostic>>,
    reconcile_return: Result<ResolverStorageObservation, Box<Diagnostic>>,
    observation: ResolverStorageObservation,
    inputs: InventoryInputs,
    walk: InventoryWalkCarriers,
    bounds: [usize; KINDS],
    byte_subtotals: [usize; 2],
    arithmetic_returns: [Result<usize, Box<Diagnostic>>; 3],
    scalar_returns: [Result<(), Box<Diagnostic>>; 3],
}
pub(super) const fn inventory_carrier_bytes() -> usize {
    size_of::<InventoryCarriers>()
}
// Inputs and checked counter state of the test-only observation
// entry. Plan/parts/inventory/reconcile/final Result carriers are charged in
// their existing complete envelopes; direct matching avoids new map_err slots.
struct ProbeCarriers {
    index: &'static DeclarationIndex<'static>,
    work: &'static WorkMeter,
    allocator: &'static mut Allocator,
    origin: Span,
    attempts_before: usize,
    attempts_after: usize,
    delta_option: Option<usize>,
    delta: usize,
}
pub(super) const fn probe_carrier_bytes() -> usize {
    size_of::<ProbeCarriers>()
}
pub(super) const fn fixed_carrier_bytes() -> usize {
    size_of::<FixedCarriers>() + inventory_carrier_bytes() + probe_carrier_bytes()
}
pub(super) const fn function_carrier_bytes() -> usize {
    size_of::<FunctionCarriers>()
}

#[cfg(test)]
#[path = "resolver_storage_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "resolver_inventory_tests.rs"]
mod inventory_tests;
