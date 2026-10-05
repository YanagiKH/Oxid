//! Disconnected T0 typed-buffer helpers, not source or typing admission.
//!
//! No checker or owner calls these helpers. The active HirPlan does NOT yet pay
//! their new carrier envelopes; measured passive integration is a separate gate.
//! The local quotas do not prove the origin of a caller-supplied byte cell.
//! Diagnostics, observer traces, allocator metadata and machine stack/RSS are
//! outside this named-buffer model. All ordinary source routes are unchanged.
#![allow(dead_code)]

use super::{
    hir::*,
    hir_budget::{Capacity, MAX_HIR_BYTES},
    typeck::{BorrowProjection, FlowSummary, TypeFrame, TypedBody},
};
use crate::frontend::{
    diagnostic::Diagnostic, owned_diagnostic, project::budget::Allocator, source::Span,
};
use std::{cell::Cell, mem::size_of};

pub(super) const KINDS: usize = 14;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(super) enum Kind {
    Bodies,
    BindingStage,
    ExpressionStage,
    FlowStage,
    ExpressionProjections,
    StatementRows,
    StatementProjections,
    BorrowProjections,
    TypeFrames,
    CallActuals,
    RecordPresence,
    BindingFinal,
    FlowFinal,
    ExpressionFinal,
}
impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Bodies => "paid typed bodies",
            Self::BindingStage => "paid typed binding stage",
            Self::ExpressionStage => "paid typed expression stage",
            Self::FlowStage => "paid typed flow stage",
            Self::ExpressionProjections => "paid typed expression projections",
            Self::StatementRows => "paid typed statement rows",
            Self::StatementProjections => "paid typed statement projections",
            Self::BorrowProjections => "paid typed borrow projections",
            Self::TypeFrames => "paid typed frames",
            Self::CallActuals => "paid typed call actuals",
            Self::RecordPresence => "paid typed record presence",
            Self::BindingFinal => "paid typed final bindings",
            Self::FlowFinal => "paid typed final flows",
            Self::ExpressionFinal => "paid typed final expressions",
        }
    }
}
pub(super) const WIDTHS: [usize; KINDS] = [
    size_of::<TypedBody>(),
    size_of::<Option<ParameterTy>>(),
    size_of::<Option<ValueTy>>(),
    size_of::<Option<FlowSummary>>(),
    size_of::<Option<Projection>>(),
    size_of::<Vec<Option<Projection>>>(),
    size_of::<Option<Projection>>(),
    size_of::<BorrowProjection>(),
    size_of::<TypeFrame>(),
    size_of::<(ParameterTy, Span)>(),
    size_of::<bool>(),
    size_of::<ParameterTy>(),
    size_of::<FlowSummary>(),
    size_of::<ValueTy>(),
];

/// Scalar input to the synthetic helper checkpoint. Deriving/reconciling these
/// counts from immutable HIR is a later disconnected slice, not implemented yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct TypeCounts {
    pub(super) functions: usize,
    pub(super) bindings: usize,
    pub(super) expressions: usize,
    pub(super) blocks: usize,
    pub(super) statements: usize,
    pub(super) borrow_arguments: usize,
    pub(super) type_frames: usize,
    pub(super) call_arguments: usize,
    pub(super) calls: usize,
    pub(super) presence_slots: usize,
    pub(super) record_literals: usize,
}
impl TypeCounts {
    fn slots(&self) -> [usize; KINDS] {
        [
            self.functions,
            self.bindings,
            self.expressions,
            self.blocks,
            self.expressions,
            self.blocks,
            self.statements,
            self.borrow_arguments,
            self.type_frames,
            self.call_arguments,
            self.presence_slots,
            self.bindings,
            self.blocks,
            self.expressions,
        ]
    }
    fn requests(&self) -> [usize; KINDS] {
        [
            1,
            self.functions,
            self.functions,
            self.functions,
            self.functions,
            self.functions,
            self.blocks,
            self.functions,
            self.functions,
            self.calls,
            self.record_literals,
            self.functions,
            self.functions,
            self.functions,
        ]
    }
}

fn failure(message: &'static str, at: Span) -> Box<Diagnostic> {
    owned_diagnostic::diagnostic("E0400", "type", format_args!("{message}"), Some(at))
}
fn invalid(at: Span) -> Box<Diagnostic> {
    owned_diagnostic::diagnostic(
        "E0500",
        "type",
        format_args!("invalid paid typed storage state"),
        Some(at),
    )
}

/// Construction-local consumable rights. Requests count whole reservations,
/// including empty ones; pushes reuse admitted backing slots without spending
/// them again. There is no retained plan table or universal owner ledger.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct PaidStorage {
    slots: [usize; KINDS],
    requests: [usize; KINDS],
}
impl PaidStorage {
    pub(super) fn new(counts: &TypeCounts) -> Self {
        Self {
            slots: counts.slots(),
            requests: counts.requests(),
        }
    }

    /// `expected` is the independently known local buffer length. The future
    /// consumer must derive it from the actual function/block/call/record, never
    /// from this global remaining quota. No consumer is connected in T0.
    pub(super) fn reserve<T>(
        &mut self,
        allocator: &mut Allocator,
        kind: Kind,
        slots: usize,
        expected: usize,
        at: Span,
    ) -> Result<Vec<T>, Box<Diagnostic>> {
        let k = kind as usize;
        if slots != expected || size_of::<T>() != WIDTHS[k] {
            return Err(invalid(at));
        }
        let remaining_slots = self.slots[k]
            .checked_sub(slots)
            .ok_or_else(|| failure("typed storage slot quota exhausted", at))?;
        let remaining_requests = self.requests[k]
            .checked_sub(1)
            .ok_or_else(|| failure("typed storage request quota exhausted", at))?;
        let bytes = slots
            .checked_mul(size_of::<T>())
            .ok_or_else(|| failure("typed storage size overflow", at))?;
        let ticket = Capacity::new::<T>(slots, bytes, at)?;
        // All arithmetic/admission checks precede both mutations. A failed
        // actual reserve remains spent; callers must abort/drop that attempt.
        self.slots[k] = remaining_slots;
        self.requests[k] = remaining_requests;
        ticket.reserve(allocator, Vec::new(), at, kind.label())
    }

    /// None is initialized directly even for non-Copy Projection payloads.
    /// No clone, collect or unconstrained allocation-capable callback is used.
    pub(super) fn none<T>(
        &mut self,
        allocator: &mut Allocator,
        kind: Kind,
        expected: usize,
        at: Span,
    ) -> Result<Vec<Option<T>>, Box<Diagnostic>> {
        if !matches!(
            kind,
            Kind::BindingStage
                | Kind::ExpressionStage
                | Kind::FlowStage
                | Kind::ExpressionProjections
                | Kind::StatementProjections
        ) {
            return Err(invalid(at));
        }
        let mut values = self.reserve(allocator, kind, expected, expected, at)?;
        while values.len() < expected {
            room(&values, expected, at)?;
            values.push(None);
        }
        Ok(values)
    }

    pub(super) fn presence(
        &mut self,
        allocator: &mut Allocator,
        expected: usize,
        at: Span,
    ) -> Result<Vec<bool>, Box<Diagnostic>> {
        let mut values = self.reserve(allocator, Kind::RecordPresence, expected, expected, at)?;
        while values.len() < expected {
            room(&values, expected, at)?;
            values.push(false);
        }
        Ok(values)
    }

    /// Only current Copy stage elements use this helper. The caller still owns
    /// the complete staging allocation throughout the separate final reserve
    /// and fill; this never consumes an IntoIter or assumes buffer reuse.
    pub(super) fn finalize<T: Copy>(
        &mut self,
        allocator: &mut Allocator,
        kind: Kind,
        stage: &[Option<T>],
        at: Span,
    ) -> Result<Vec<T>, Box<Diagnostic>> {
        if !matches!(
            kind,
            Kind::BindingFinal | Kind::FlowFinal | Kind::ExpressionFinal
        ) {
            return Err(invalid(at));
        }
        let mut values = self.reserve(allocator, kind, stage.len(), stage.len(), at)?;
        for slot in stage {
            let value = slot.ok_or_else(|| invalid(at))?;
            room(&values, stage.len(), at)?;
            values.push(value);
        }
        Ok(values)
    }
}

/// No row value is accepted before checking capacity. The actual capacity must
/// equal the exact admitted request, including for empty vectors. Pop/push frame
/// reuse is allowed; implicit growth and oversize observations are not.
pub(super) fn room<T>(values: &Vec<T>, admitted: usize, at: Span) -> Result<(), Box<Diagnostic>> {
    let capacity = values.capacity();
    if capacity != admitted || values.len() >= admitted {
        Err(failure("typed storage append exceeds exact capacity", at))
    } else {
        Ok(())
    }
}

/// Borrow, do not create/reset/seed, the future owner's existing aggregate cell.
/// This helper neither certifies that seed nor transfers source admission.
/// T1 must not call the old admit_projection and then this helper: that would
/// charge the same path twice. Its validation/work sequence needs an explicitly
/// reviewed paid branch; this primitive does not replace that whole pipeline.
/// FieldId backing capacity alone is charged; embedded path headers are prepaid
/// in the complete projection carriers. No dynamic growth API is provided.
pub(super) fn projection_fields(
    total: &Cell<usize>,
    allocator: &mut Allocator,
    slots: usize,
    at: Span,
) -> Result<Vec<FieldId>, Box<Diagnostic>> {
    if slots == 0 || slots > 64 {
        return Err(failure("record access path depth limit exceeded", at));
    }
    let bytes = slots
        .checked_mul(size_of::<FieldId>())
        .ok_or_else(|| failure("typed storage size overflow", at))?;
    let previous = total.get();
    let next = previous
        .checked_add(bytes)
        .filter(|value| *value <= MAX_HIR_BYTES)
        .ok_or_else(|| failure("affected HIR and projection payload limit exceeded", at))?;
    let ticket = Capacity::new::<FieldId>(slots, bytes, at)?;
    // The same seed+dynamic ceiling is checked before allocation. Keep a spent
    // charge after reserve failure or later semantic rejection/drop.
    total.set(next);
    ticket.reserve(allocator, Vec::new(), at, "paid typed projection fields")
}

// Complete NEW type_storage named controls, plus explicitly selected existing
// Capacity transports. Unchanged Capacity/Allocator internals are not modeled as
// a complete all-call-chain envelope. These are not instantiated compiler frames
// or a stack/RSS bound. Count independent local, return and caller storage
// without assuming elision; embedded payloads appear
// only inside their complete enclosing Option/Result. These are standalone full
// models of this stated surface: passive integration must reconcile the selected
// existing Capacity envelopes rather than blindly adding already-accounted
// primitive carriers a second time.
struct AccountCarriers {
    counts: TypeCounts,
    count_borrows: [&'static TypeCounts; 3],
    // Constructor, return slot and caller; scalar-only, no heap payload.
    accounts: [PaidStorage; 3],
    // slots/requests array construction and complete returned arrays.
    arrays: [[usize; KINDS]; 4],
}
struct ReserveCarriers<T: 'static> {
    storage: &'static mut PaidStorage,
    allocator: &'static mut Allocator,
    kind: Kind,
    // Explicit call inputs and named checked intermediates.
    slots: usize,
    expected: usize,
    origin: Span,
    kind_index: usize,
    remaining_slots: usize,
    remaining_requests: usize,
    bytes: usize,
    arithmetic_options: [Option<usize>; 3],
    arithmetic_results: [Result<usize, Box<Diagnostic>>; 3],
    // Capacity constructor, caller, consumed reserve input and nested check.
    capacities: [Capacity; 4],
    capacity_return: Result<Capacity, Box<Diagnostic>>,
    fresh: Vec<T>,
    primitive_input: Vec<T>,
    primitive_return: Result<Vec<T>, Box<Diagnostic>>,
    returned: Result<Vec<T>, Box<Diagnostic>>,
    caller: Vec<T>,
    scalar_returns: [Result<(), Box<Diagnostic>>; 3],
}
// The room helper has its own by-value inputs and actual-capacity local. Its
// origin copy and Vec borrow do not reuse an outer fill/finalize input slot.
struct RoomCarriers<T: 'static> {
    values: &'static Vec<T>,
    admitted: usize,
    origin: Span,
    capacity: usize,
    returned: Result<(), Box<Diagnostic>>,
}
struct FillCarriers<T: 'static> {
    storage: &'static mut PaidStorage,
    allocator: &'static mut Allocator,
    kind: Kind,
    expected: usize,
    origin: Span,
    values: Vec<T>,
    reserved: Result<Vec<T>, Box<Diagnostic>>,
    returned: Result<Vec<T>, Box<Diagnostic>>,
    caller: Vec<T>,
    pending_row: T,
    room: RoomCarriers<T>,
    room_return: Result<(), Box<Diagnostic>>,
}
struct FinalizeCarriers<T: 'static> {
    storage: &'static mut PaidStorage,
    allocator: &'static mut Allocator,
    kind: Kind,
    stage: &'static [Option<T>],
    origin: Span,
    values: Vec<T>,
    reserved: Result<Vec<T>, Box<Diagnostic>>,
    returned: Result<Vec<T>, Box<Diagnostic>>,
    caller: Vec<T>,
    iterator: std::slice::Iter<'static, Option<T>>,
    next: Option<&'static Option<T>>,
    slot: &'static Option<T>,
    copied_option: Option<T>,
    selected: Result<T, Box<Diagnostic>>,
    value: T,
    room: RoomCarriers<T>,
    room_return: Result<(), Box<Diagnostic>>,
}
struct ProjectionCarriers {
    total: &'static Cell<usize>,
    allocator: &'static mut Allocator,
    slots: usize,
    origin: Span,
    bytes: usize,
    previous: usize,
    next: usize,
    arithmetic_options: [Option<usize>; 3],
    arithmetic_results: [Result<usize, Box<Diagnostic>>; 2],
    capacities: [Capacity; 4],
    capacity_return: Result<Capacity, Box<Diagnostic>>,
    fresh: Vec<FieldId>,
    primitive_input: Vec<FieldId>,
    primitive_return: Result<Vec<FieldId>, Box<Diagnostic>>,
    returned: Result<Vec<FieldId>, Box<Diagnostic>>,
    caller: Vec<FieldId>,
    scalar_returns: [Result<(), Box<Diagnostic>>; 3],
}

pub(super) const fn account_carrier_bytes() -> usize {
    size_of::<AccountCarriers>()
}
pub(super) const fn reserve_carrier_bytes() -> usize {
    let mut largest = 0;
    macro_rules! include {
        ($($ty:ty),* $(,)?) => { $(
            let bytes = size_of::<ReserveCarriers<$ty>>();
            if bytes > largest { largest = bytes; }
        )* };
    }
    include!(
        TypedBody,
        Option<ParameterTy>,
        Option<ValueTy>,
        Option<FlowSummary>,
        Option<Projection>,
        Vec<Option<Projection>>,
        BorrowProjection,
        TypeFrame,
        (ParameterTy, Span),
        bool,
        ParameterTy,
        FlowSummary,
        ValueTy,
    );
    largest
}
pub(super) const fn fill_carrier_bytes() -> usize {
    let mut largest = 0;
    macro_rules! include {
        ($($ty:ty),* $(,)?) => { $(
            let bytes = size_of::<FillCarriers<$ty>>();
            if bytes > largest { largest = bytes; }
        )* };
    }
    include!(
        Option<ParameterTy>,
        Option<ValueTy>,
        Option<FlowSummary>,
        Option<Projection>,
        bool
    );
    largest
}
pub(super) const fn finalize_carrier_bytes() -> usize {
    size_of::<FinalizeCarriers<ParameterTy>>()
        + size_of::<FinalizeCarriers<FlowSummary>>()
        + size_of::<FinalizeCarriers<ValueTy>>()
}
pub(super) const fn projection_carrier_bytes() -> usize {
    size_of::<ProjectionCarriers>()
}

#[cfg(test)]
#[path = "type_storage_tests.rs"]
mod tests;
