//! Disconnected T0 typed-buffer helpers, not source or typing admission.
//!
//! The disconnected paid checker names these helpers but has no admitted caller.
//! HirPlan prepays the measured T0 controls; complete T1 pricing stays gated.
//! The local quotas do not prove the origin of a caller-supplied byte cell.
//! Diagnostics, observer traces, allocator metadata and machine stack/RSS are
//! outside this named-buffer model. All ordinary source routes are unchanged.
#![allow(dead_code)]

use super::{
    hir::*,
    hir_budget::{Capacity, CapacityReturnEnvelope, HirPlan, VectorReturnEnvelope, MAX_HIR_BYTES},
    typeck::{BorrowProjection, FlowSummary, TypeFrame, TypedBody},
};
use crate::frontend::{
    declaration_index::WorkMeter, diagnostic::Diagnostic, owned_diagnostic,
    parser::MAX_BLOCK_NESTING, project::budget::Allocator, source::Span,
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

/// Scalar resource counts, never a source-association or typing witness.
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

/// Disconnected paid-checker path. Precharge precedes the historical work
/// debit; the debit precedes reserve. A failed debit keeps bytes spent but makes
/// no allocator request. Neither this helper nor its caller uses legacy charges.
pub(super) fn projection_fields_metered(
    total: &Cell<usize>,
    allocator: &mut Allocator,
    slots: usize,
    work: &WorkMeter,
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
    total.set(next);
    if slots > 1 {
        work.debit(slots as u64, at, "record projection path")?;
    }
    ticket.reserve(allocator, Vec::new(), at, "paid typed projection fields")
}

fn add(left: usize, right: usize, at: Span) -> Result<usize, Box<Diagnostic>> {
    left.checked_add(right)
        .ok_or_else(|| failure("typed storage count overflow", at))
}
fn visit(work: &WorkMeter, at: Span) -> Result<(), Box<Diagnostic>> {
    work.debit(1, at, "typed storage preparation")
}
fn ordinary_value(value: &ValueTy, at: Span) -> Result<(), Box<Diagnostic>> {
    if matches!(value, ValueTy::Owned(AggregateTy::Enum(_))) {
        Err(invalid(at))
    } else {
        Ok(())
    }
}
fn ordinary_parameter(value: &ParameterTy, at: Span) -> Result<(), Box<Diagnostic>> {
    match value {
        ParameterTy::Value(value) => ordinary_value(value, at),
        ParameterTy::Reference {
            referent: BorrowedTy::Exact(AggregateTy::Enum(_)),
            ..
        } => Err(invalid(at)),
        ParameterTy::Reference { .. } => Ok(()),
    }
}
impl TypeCounts {
    fn combined(&self, other: &Self, at: Span) -> Result<Self, Box<Diagnostic>> {
        // Build a complete candidate before replacing the caller's accumulator.
        Ok(Self {
            functions: add(self.functions, other.functions, at)?,
            bindings: add(self.bindings, other.bindings, at)?,
            expressions: add(self.expressions, other.expressions, at)?,
            blocks: add(self.blocks, other.blocks, at)?,
            statements: add(self.statements, other.statements, at)?,
            borrow_arguments: add(self.borrow_arguments, other.borrow_arguments, at)?,
            type_frames: add(self.type_frames, other.type_frames, at)?,
            call_arguments: add(self.call_arguments, other.call_arguments, at)?,
            calls: add(self.calls, other.calls, at)?,
            presence_slots: add(self.presence_slots, other.presence_slots, at)?,
            record_literals: add(self.record_literals, other.record_literals, at)?,
        })
    }
}

#[derive(Clone, Copy)]
struct BlockCursor {
    id: BodyBlockId,
    statement: usize,
    child: usize,
}
/// Exact ordinary HIR tree certificate, with no bitmap, recursion or heap.
fn body_counts(function: &Function, work: &WorkMeter) -> Result<(usize, usize), Box<Diagnostic>> {
    let at = function.end;
    if function.body.0 != 0 || function.blocks.is_empty() {
        return Err(invalid(at));
    }
    let mut frames = [None; MAX_BLOCK_NESTING];
    let mut depth = 1usize;
    let mut maximum = 1usize;
    let mut next_block = 1usize;
    let mut statements = 0usize;
    visit(work, at)?;
    frames[0] = Some(BlockCursor {
        id: function.body,
        statement: 0,
        child: 0,
    });
    while depth != 0 {
        let cursor = frames[depth - 1].as_mut().ok_or_else(|| invalid(at))?;
        let block = function
            .blocks
            .get(cursor.id.0)
            .ok_or_else(|| invalid(at))?;
        let Some(statement) = block.body.get(cursor.statement) else {
            frames[depth - 1] = None;
            depth -= 1;
            continue;
        };
        if cursor.child == 0 {
            visit(work, statement.span)?;
            statements = add(statements, 1, statement.span)?;
            if matches!(statement.kind, StmtKind::Match { .. }) {
                return Err(invalid(statement.span));
            }
        }
        let child = match &statement.kind {
            StmtKind::While { body, .. } if cursor.child == 0 => Some(*body),
            StmtKind::If {
                then_block,
                else_block,
                ..
            } => match cursor.child {
                0 => Some(*then_block),
                1 => *else_block,
                _ => None,
            },
            _ => None,
        };
        if let Some(child) = child {
            cursor.child = add(cursor.child, 1, statement.span)?;
            visit(work, statement.span)?;
            // Exact preorder rules out shared, cyclic, skipped and detached
            // child blocks. Merely checking a forward edge would be weaker.
            if child.0 != next_block || child.0 >= function.blocks.len() {
                return Err(invalid(statement.span));
            }
            if depth == MAX_BLOCK_NESTING {
                return Err(failure(
                    "typed storage block depth limit exceeded",
                    statement.span,
                ));
            }
            next_block = add(next_block, 1, statement.span)?;
            frames[depth] = Some(BlockCursor {
                id: child,
                statement: 0,
                child: 0,
            });
            depth = add(depth, 1, statement.span)?;
            maximum = maximum.max(depth);
        } else {
            cursor.statement = add(cursor.statement, 1, statement.span)?;
            cursor.child = 0;
        }
    }
    if next_block != function.blocks.len() {
        return Err(invalid(at));
    }
    Ok((statements, maximum))
}

fn count_function(
    records: &[Record],
    function: &Function,
    work: &WorkMeter,
) -> Result<TypeCounts, Box<Diagnostic>> {
    let at = function.end;
    visit(work, at)?;
    let mut counts = TypeCounts {
        functions: 1,
        bindings: function.bindings.len(),
        expressions: function.expressions.len(),
        blocks: function.blocks.len(),
        ..TypeCounts::default()
    };
    for binding in &function.bindings {
        visit(work, binding.span)?;
        if let Some(value) = &binding.annotation {
            ordinary_value(value, binding.span)?;
        }
    }
    for expression in &function.expressions {
        visit(work, expression.span)?;
        match &expression.kind {
            ExprKind::ConstructEnum { .. } => return Err(invalid(expression.span)),
            ExprKind::Call { args, .. } => {
                counts.calls = add(counts.calls, 1, expression.span)?;
                counts.call_arguments = add(counts.call_arguments, args.len(), expression.span)?;
                for argument in args {
                    visit(work, expression.span)?;
                    if matches!(argument, Argument::Borrow { .. }) {
                        counts.borrow_arguments = add(counts.borrow_arguments, 1, expression.span)?;
                    }
                }
            }
            ExprKind::StructLiteral { record, .. } => {
                let selected = records
                    .get(record.0)
                    .ok_or_else(|| invalid(expression.span))?;
                if selected.id != *record {
                    return Err(invalid(expression.span));
                }
                counts.record_literals = add(counts.record_literals, 1, expression.span)?;
                // The complete declaration, not the supplied initializers. A
                // missing-field diagnostic cannot shrink pending presence data.
                counts.presence_slots = add(
                    counts.presence_slots,
                    selected.fields.len(),
                    expression.span,
                )?;
            }
            _ => {}
        }
    }
    let (statements, maximum) = body_counts(function, work)?;
    counts.statements = statements;
    // Current TypeFrame schedule: at most join+pending else per ancestor and
    // one active block, <=2*H-1. While and ordinary continuations are smaller.
    // The already-agreed 3*H+8 allowance dominates, without pricing any Match.
    counts.type_frames = maximum
        .checked_mul(3)
        .and_then(|frames| frames.checked_add(8))
        .ok_or_else(|| failure("typed storage frame count overflow", at))?;
    Ok(counts)
}

fn reconcile(
    counts: &TypeCounts,
    source: &HirPlan,
    work: &WorkMeter,
    at: Span,
) -> Result<(), Box<Diagnostic>> {
    visit(work, at)?;
    let limits = TypeCounts {
        functions: source.counts.functions,
        bindings: source.counts.bindings,
        expressions: source.counts.expressions,
        blocks: source.counts.blocks,
        statements: source.counts.statements,
        borrow_arguments: source.counts.borrow_arguments,
        type_frames: source.counts.type_frames,
        call_arguments: source.counts.call_arguments,
        calls: source.counts.calls,
        presence_slots: source
            .counts
            .record_literals
            .checked_mul(source.counts.max_record_fields)
            .ok_or_else(|| failure("typed storage count overflow", at))?,
        record_literals: source.counts.record_literals,
    };
    let actual_slots = counts.slots();
    let source_slots = limits.slots();
    let actual_requests = counts.requests();
    let source_requests = limits.requests();
    for k in 0..KINDS {
        visit(work, at)?;
        if actual_slots[k] > source_slots[k] || actual_requests[k] > source_requests[k] {
            return Err(failure("typed storage exceeds source prepaid bounds", at));
        }
    }
    Ok(())
}

/// A local resource preparation over exact immutable HIR slices. It neither
/// proves source association nor permits typing. No program retains this plan.
#[derive(Debug)]
pub(super) struct TypePlan<'hir> {
    counts: TypeCounts,
    storage: PaidStorage,
    records: &'hir [Record],
    functions: &'hir [Function],
    next_function: usize,
}
#[derive(Debug)]
pub(super) struct FunctionQuota {
    pub(super) ordinal: usize,
    pub(super) counts: TypeCounts,
    pub(super) storage: PaidStorage,
}

pub(super) fn prepare<'hir>(
    records: &'hir [Record],
    signatures: &[Signature],
    functions: &'hir [Function],
    source: &HirPlan,
    work: &WorkMeter,
    at: Span,
) -> Result<TypePlan<'hir>, Box<Diagnostic>> {
    visit(work, at)?;
    if functions.len() != signatures.len()
        || functions.len() != source.counts.functions
        || records.len() != source.counts.records
    {
        return Err(invalid(at));
    }
    if source.total > MAX_HIR_BYTES {
        return Err(failure(
            "affected HIR and projection payload limit exceeded",
            at,
        ));
    }
    let mut record_fields = 0usize;
    for (ordinal, record) in records.iter().enumerate() {
        visit(work, record.span)?;
        if record.id != RecordId(ordinal) {
            return Err(invalid(record.span));
        }
        record_fields = add(record_fields, record.fields.len(), record.span)?;
        for field in &record.fields {
            visit(work, field.span)?;
            ordinary_value(&field.ty, field.span)?;
        }
    }
    if record_fields > source.counts.record_fields {
        return Err(failure("typed storage exceeds source prepaid bounds", at));
    }
    for signature in signatures {
        visit(work, signature.span)?;
        ordinary_value(&signature.result, signature.span)?;
        for parameter in &signature.params {
            visit(work, signature.span)?;
            ordinary_parameter(parameter, signature.span)?;
        }
    }
    let mut counts = TypeCounts::default();
    for (ordinal, function) in functions.iter().enumerate() {
        visit(work, function.end)?;
        if function.id != DefId(ordinal) {
            return Err(invalid(function.end));
        }
        let current = count_function(records, function, work)?;
        counts = counts.combined(&current, function.end)?;
    }
    reconcile(&counts, source, work, at)?;
    Ok(TypePlan {
        storage: PaidStorage::new(&counts),
        counts,
        records,
        functions,
        next_function: 0,
    })
}
impl TypePlan<'_> {
    pub(super) fn reserve_bodies(
        &mut self,
        allocator: &mut Allocator,
        at: Span,
    ) -> Result<Vec<TypedBody>, Box<Diagnostic>> {
        self.storage.reserve(
            allocator,
            Kind::Bodies,
            self.counts.functions,
            self.counts.functions,
            at,
        )
    }

    pub(super) fn partition_next(
        &mut self,
        work: &WorkMeter,
        at: Span,
    ) -> Result<FunctionQuota, Box<Diagnostic>> {
        visit(work, at)?;
        let function = self
            .functions
            .get(self.next_function)
            .ok_or_else(|| invalid(at))?;
        if function.id != DefId(self.next_function) {
            return Err(invalid(at));
        }
        let counts = count_function(self.records, function, work)?;
        let mut storage = PaidStorage::new(&counts);
        // A function can never allocate a fresh top-level bodies collection.
        storage.slots[Kind::Bodies as usize] = 0;
        storage.requests[Kind::Bodies as usize] = 0;
        let mut slots = [0; KINDS];
        let mut requests = [0; KINDS];
        for k in 0..KINDS {
            visit(work, at)?;
            slots[k] = self.storage.slots[k]
                .checked_sub(storage.slots[k])
                .ok_or_else(|| failure("typed storage function slot quota exhausted", at))?;
            requests[k] = self.storage.requests[k]
                .checked_sub(storage.requests[k])
                .ok_or_else(|| failure("typed storage function request quota exhausted", at))?;
        }
        let next = add(self.next_function, 1, at)?;
        let ordinal = self.next_function;
        // Commit only after every count, work, subtraction and ordinal check.
        self.storage.slots = slots;
        self.storage.requests = requests;
        self.next_function = next;
        Ok(FunctionQuota {
            ordinal,
            counts,
            storage,
        })
    }
}

// Stage A observation primitives only. No source caller/context borrows this
// state yet; no fixed successful source observation is exposed. These helpers
// read actual Vec properties, never quota/request counts posing as capacities.
pub(super) const SCRATCH_KINDS: [Kind; 6] = [
    Kind::BindingStage,
    Kind::ExpressionStage,
    Kind::FlowStage,
    Kind::TypeFrames,
    Kind::CallActuals,
    Kind::RecordPresence,
];
pub(super) const RETAINED_KINDS: [Kind; 8] = [
    Kind::Bodies,
    Kind::ExpressionProjections,
    Kind::StatementRows,
    Kind::StatementProjections,
    Kind::BorrowProjections,
    Kind::BindingFinal,
    Kind::FlowFinal,
    Kind::ExpressionFinal,
];
pub(super) fn scratch_index(kind: Kind) -> Option<usize> {
    match kind {
        Kind::BindingStage => Some(0),
        Kind::ExpressionStage => Some(1),
        Kind::FlowStage => Some(2),
        Kind::TypeFrames => Some(3),
        Kind::CallActuals => Some(4),
        Kind::RecordPresence => Some(5),
        _ => None,
    }
}
pub(super) fn retained_index(kind: Kind) -> Option<usize> {
    match kind {
        Kind::Bodies => Some(0),
        Kind::ExpressionProjections => Some(1),
        Kind::StatementRows => Some(2),
        Kind::StatementProjections => Some(3),
        Kind::BorrowProjections => Some(4),
        Kind::BindingFinal => Some(5),
        Kind::FlowFinal => Some(6),
        Kind::ExpressionFinal => Some(7),
        _ => None,
    }
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct TypedObserved {
    pub(super) materialized_vectors: [usize; KINDS],
    pub(super) materialized_capacity: [usize; KINDS],
    pub(super) scratch_endpoints: [usize; 6],
    pub(super) scratch_endpoint_capacity: [usize; 6],
    pub(super) path_vectors: usize,
    pub(super) path_capacity_fields: usize,
}
impl TypedObserved {
    pub(super) fn new() -> Self {
        Self {
            materialized_vectors: [0; KINDS],
            materialized_capacity: [0; KINDS],
            scratch_endpoints: [0; 6],
            scratch_endpoint_capacity: [0; 6],
            path_vectors: 0,
            path_capacity_fields: 0,
        }
    }
    /// O(1), allocation-free on success; no work debit is inserted into the
    /// historical reserve/semantic ordering. Final independent scans are metered.
    pub(super) fn materialized<T>(
        &mut self,
        kind: Kind,
        values: &Vec<T>,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        let k = kind as usize;
        let len = values.len();
        let capacity = values.capacity();
        let width = size_of::<T>();
        let filled = matches!(
            kind,
            Kind::BindingStage
                | Kind::ExpressionStage
                | Kind::FlowStage
                | Kind::ExpressionProjections
                | Kind::StatementProjections
                | Kind::RecordPresence
                | Kind::BindingFinal
                | Kind::FlowFinal
                | Kind::ExpressionFinal
        );
        if width != WIDTHS[k]
            || len > capacity
            || (filled && len != capacity)
            || (!filled && len != 0)
        {
            return Err(invalid(at));
        }
        let vectors = self.materialized_vectors[k]
            .checked_add(1)
            .ok_or_else(|| failure("typed observation vector count overflow", at))?;
        let capacities = self.materialized_capacity[k]
            .checked_add(capacity)
            .ok_or_else(|| failure("typed observation capacity sum overflow", at))?;
        // Both independent candidates are checked before either mutation.
        self.materialized_vectors[k] = vectors;
        self.materialized_capacity[k] = capacities;
        Ok(())
    }
    /// Actual scratch capacity is read again at its successful lifetime endpoint.
    /// Empty TypeFrames is normal; the other scratch vectors are fully filled.
    pub(super) fn scratch_endpoint<T>(
        &mut self,
        kind: Kind,
        values: &Vec<T>,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        let k = kind as usize;
        let slot = scratch_index(kind).ok_or_else(|| invalid(at))?;
        let len = values.len();
        let capacity = values.capacity();
        let width = size_of::<T>();
        let empty = kind == Kind::TypeFrames;
        if width != WIDTHS[k]
            || len > capacity
            || (empty && len != 0)
            || (!empty && len != capacity)
        {
            return Err(invalid(at));
        }
        let vectors = self.scratch_endpoints[slot]
            .checked_add(1)
            .ok_or_else(|| failure("typed observation endpoint count overflow", at))?;
        let capacities = self.scratch_endpoint_capacity[slot]
            .checked_add(capacity)
            .ok_or_else(|| failure("typed observation endpoint capacity overflow", at))?;
        self.scratch_endpoints[slot] = vectors;
        self.scratch_endpoint_capacity[slot] = capacities;
        Ok(())
    }
    /// Called immediately after path reserve, before field walking: len==0 is
    /// required here. The later retained inventory must enforce initialized
    /// len==capacity, but must not reuse this fresh-allocation shape check.
    pub(super) fn path(&mut self, values: &Vec<FieldId>, at: Span) -> Result<(), Box<Diagnostic>> {
        let len = values.len();
        let capacity = values.capacity();
        if len != 0 || !(1..=64).contains(&capacity) {
            return Err(invalid(at));
        }
        let vectors = self
            .path_vectors
            .checked_add(1)
            .ok_or_else(|| failure("typed observation path count overflow", at))?;
        let capacities = self
            .path_capacity_fields
            .checked_add(capacity)
            .ok_or_else(|| failure("typed observation path capacity overflow", at))?;
        self.path_vectors = vectors;
        self.path_capacity_fields = capacities;
        Ok(())
    }
}

impl PaidStorage {
    /// Read-only successful completion, not authority to repeat any reservation.
    /// Each actual kind is visited under the same unchanged work ceiling.
    fn complete(&self, work: &WorkMeter, at: Span) -> Result<(), Box<Diagnostic>> {
        for k in 0..KINDS {
            work.debit(1, at, "typed storage completion")?;
            if self.slots[k] != 0 || self.requests[k] != 0 {
                return Err(invalid(at));
            }
        }
        Ok(())
    }
}
impl FunctionQuota {
    pub(super) fn complete(&self, work: &WorkMeter, at: Span) -> Result<(), Box<Diagnostic>> {
        self.storage.complete(work, at)
    }
}
impl TypePlan<'_> {
    pub(super) fn counts(&self) -> &TypeCounts {
        &self.counts
    }
    pub(super) fn complete(&self, work: &WorkMeter, at: Span) -> Result<(), Box<Diagnostic>> {
        work.debit(1, at, "typed plan completion")?;
        if self.next_function != self.functions.len()
            || self.counts.functions != self.functions.len()
        {
            return Err(invalid(at));
        }
        self.storage.complete(work, at)
    }
}

// Complete Stage A helper surfaces, still disconnected and UNPRICED. No owned
// sampler receiver in a future source helper or context field is invented here.
struct ObservedConstructionCarriers {
    constructed: TypedObserved,
    returned: TypedObserved,
}
struct KindMappingCarriers {
    kind: Kind,
    returned: Option<usize>,
    caller: Option<usize>,
    selected: Result<usize, Box<Diagnostic>>,
    slot: usize,
}
struct SampleCarriers<T: 'static> {
    observed: &'static mut TypedObserved,
    values: &'static Vec<T>,
    kind: Kind,
    origin: Span,
    k: usize,
    len: usize,
    capacity: usize,
    width: usize,
    shape: bool,
    additions: [Option<usize>; 2],
    addition_returns: [Result<usize, Box<Diagnostic>>; 2],
    vectors: usize,
    capacities: usize,
    returned: Result<(), Box<Diagnostic>>,
    caller_result: Result<(), Box<Diagnostic>>,
}
struct EndpointSampleCarriers<T: 'static> {
    sample: SampleCarriers<T>,
    mapping: KindMappingCarriers,
}
struct PathSampleCarriers {
    observed: &'static mut TypedObserved,
    values: &'static Vec<FieldId>,
    origin: Span,
    len: usize,
    capacity: usize,
    range: std::ops::RangeInclusive<usize>,
    range_input: &'static usize,
    range_return: bool,
    additions: [Option<usize>; 2],
    addition_returns: [Result<usize, Box<Diagnostic>>; 2],
    vectors: usize,
    capacities: usize,
    returned: Result<(), Box<Diagnostic>>,
    caller_result: Result<(), Box<Diagnostic>>,
}
struct CompletionCarriers {
    storage: &'static PaidStorage,
    work: &'static WorkMeter,
    origin: Span,
    kinds: std::ops::Range<usize>,
    next: Option<usize>,
    kind: usize,
    debit: Result<(), Box<Diagnostic>>,
    returned: Result<(), Box<Diagnostic>>,
}
struct QuotaCompletionCarriers {
    quota: &'static FunctionQuota,
    work: &'static WorkMeter,
    origin: Span,
    storage_input: &'static PaidStorage,
    returned: Result<(), Box<Diagnostic>>,
    caller_result: Result<(), Box<Diagnostic>>,
}
struct PlanCompletionCarriers {
    plan: &'static TypePlan<'static>,
    work: &'static WorkMeter,
    origin: Span,
    debit: Result<(), Box<Diagnostic>>,
    next_function: usize,
    function_len: usize,
    counts_functions: usize,
    storage_input: &'static PaidStorage,
    returned: Result<(), Box<Diagnostic>>,
    caller_result: Result<(), Box<Diagnostic>>,
}
struct CountsAccessCarriers {
    plan: &'static TypePlan<'static>,
    returned: &'static TypeCounts,
    caller: &'static TypeCounts,
}

// Non-test measurement-only forcing surface for each real generic sampler
// type. No source caller uses these getters and no new bank is priced here.
// There is no synthetic Vec/owner value, callback, or production pricing array.
pub(super) const fn sample_carrier_bytes() -> usize {
    let mut largest = 0;
    macro_rules! include {
        ($($ty:ty),* $(,)?) => { $(
            let bytes = size_of::<SampleCarriers<$ty>>();
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
        ValueTy
    );
    largest
}
pub(super) const fn endpoint_sample_carrier_bytes() -> usize {
    let mut largest = 0;
    macro_rules! include {
        ($($ty:ty),* $(,)?) => { $(
            let bytes = size_of::<EndpointSampleCarriers<$ty>>();
            if bytes > largest { largest = bytes; }
        )* };
    }
    include!(
        Option<ParameterTy>,
        Option<ValueTy>,
        Option<FlowSummary>,
        TypeFrame,
        (ParameterTy, Span),
        bool
    );
    largest
}
// Getter-local accumulator/candidate/return roles. A future pricing caller's
// separately named receiver remains a later authored obligation, not implied
// authority from this unpriced measurement helper.
struct SampleSizingCarriers {
    largest: usize,
    bytes: usize,
    comparison: bool,
    returned: usize,
}
pub(super) const fn sample_sizing_carrier_bytes() -> usize {
    size_of::<SampleSizingCarriers>()
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
// Caller-owned counts and the account receiver belong to their call sites;
// retain the separate constructor and return values in the execution controls.
struct AccountControls {
    count_borrows: [&'static TypeCounts; 3],
    constructed: PaidStorage,
    returned: PaidStorage,
    arrays: [[usize; KINDS]; 4],
}
struct AccountCarriers {
    controls: AccountControls,
    counts: TypeCounts,
    caller: PaidStorage,
}
// Reuse the exact already-priced primitive carrier definitions, not an estimated
// duplicate. Their one fixed bank is shared by nonrecursive helper invocations.
struct PrimitiveTransports<T> {
    capacity: CapacityReturnEnvelope,
    vector: VectorReturnEnvelope<T>,
}
struct ReserveControls<T: 'static> {
    storage: &'static mut PaidStorage,
    allocator: &'static mut Allocator,
    kind: Kind,
    slots: usize,
    expected: usize,
    origin: Span,
    kind_index: usize,
    remaining_slots: usize,
    remaining_requests: usize,
    bytes: usize,
    arithmetic_options: [Option<usize>; 3],
    arithmetic_results: [Result<usize, Box<Diagnostic>>; 3],
    fresh: Vec<T>,
    returned: Result<Vec<T>, Box<Diagnostic>>,
    scalar_returns: [Result<(), Box<Diagnostic>>; 3],
}
struct ReserveCarriers<T: 'static> {
    controls: ReserveControls<T>,
    primitive: PrimitiveTransports<T>,
    caller: Vec<T>,
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
struct FillControls<T: 'static> {
    storage: &'static mut PaidStorage,
    allocator: &'static mut Allocator,
    kind: Kind,
    expected: usize,
    origin: Span,
    values: Vec<T>,
    reserved: Result<Vec<T>, Box<Diagnostic>>,
    returned: Result<Vec<T>, Box<Diagnostic>>,
    pending_row: T,
    room: RoomCarriers<T>,
    room_return: Result<(), Box<Diagnostic>>,
}
struct FillCarriers<T: 'static> {
    controls: FillControls<T>,
    caller: Vec<T>,
}
struct FinalizeControls<T: 'static> {
    storage: &'static mut PaidStorage,
    allocator: &'static mut Allocator,
    kind: Kind,
    stage: &'static [Option<T>],
    origin: Span,
    values: Vec<T>,
    reserved: Result<Vec<T>, Box<Diagnostic>>,
    returned: Result<Vec<T>, Box<Diagnostic>>,
    iterator: std::slice::Iter<'static, Option<T>>,
    next: Option<&'static Option<T>>,
    slot: &'static Option<T>,
    copied_option: Option<T>,
    selected: Result<T, Box<Diagnostic>>,
    value: T,
    room: RoomCarriers<T>,
    room_return: Result<(), Box<Diagnostic>>,
}
struct FinalizeCarriers<T: 'static> {
    controls: FinalizeControls<T>,
    caller: Vec<T>,
}
struct ProjectionControls {
    total: &'static Cell<usize>,
    allocator: &'static mut Allocator,
    slots: usize,
    origin: Span,
    bytes: usize,
    previous: usize,
    next: usize,
    arithmetic_options: [Option<usize>; 3],
    arithmetic_results: [Result<usize, Box<Diagnostic>>; 2],
    fresh: Vec<FieldId>,
    returned: Result<Vec<FieldId>, Box<Diagnostic>>,
    scalar_returns: [Result<(), Box<Diagnostic>>; 3],
}
// Only this additional work surface joins checker-only passive pricing. The
// metered/nonmetered helpers share the existing nonrecursive ProjectionControls
// and Capacity/Vec transport roles; their complete banks are never summed twice.
pub(super) struct MeteredProjectionWorkControls {
    work: &'static WorkMeter,
    work_units: u64,
    debit: Result<(), Box<Diagnostic>>,
}
struct MeteredProjectionCarriers {
    controls: ProjectionControls,
    work: MeteredProjectionWorkControls,
    primitive: PrimitiveTransports<FieldId>,
}
pub(super) const fn metered_projection_carrier_bytes() -> usize {
    size_of::<MeteredProjectionCarriers>()
}
#[test]
fn c3_t1_metered_projection_actual_layout() {
    println!(
        "C3_T1_PAID_CONTEXT_LAYOUT MeteredProjectionCarriers {} {}",
        size_of::<MeteredProjectionCarriers>(),
        std::mem::align_of::<MeteredProjectionCarriers>()
    );
    assert_eq!(
        size_of::<MeteredProjectionWorkControls>(),
        size_of::<&WorkMeter>() + size_of::<u64>() + size_of::<Result<(), Box<Diagnostic>>>()
    );
    assert_eq!(
        size_of::<MeteredProjectionCarriers>(),
        size_of::<ProjectionControls>()
            + size_of::<MeteredProjectionWorkControls>()
            + size_of::<PrimitiveTransports<FieldId>>()
    );
    println!(
        "C3_T1_PAID_CONTEXT_LAYOUT MeteredProjectionWorkControls {} {}",
        size_of::<MeteredProjectionWorkControls>(),
        std::mem::align_of::<MeteredProjectionWorkControls>()
    );
}

struct ProjectionCarriers {
    controls: ProjectionControls,
    primitive: PrimitiveTransports<FieldId>,
    caller: Vec<FieldId>,
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

// The count/partition phase stays disconnected. HirPlan now prepays its T0
// controls only. These models enumerate new named inputs, cursors and
// complete returns without claiming inherited WorkMeter internals or all stack.
struct SliceLoop<T: 'static> {
    cursor: std::slice::Iter<'static, T>,
    next: Option<&'static T>,
    current: &'static T,
}
struct EnumeratedLoop<T: 'static> {
    cursor: std::iter::Enumerate<std::slice::Iter<'static, T>>,
    next: Option<(usize, &'static T)>,
    current: (usize, &'static T),
    ordinal: usize,
    row: &'static T,
}
struct CountReturnCarriers {
    // Default constructor/return/accumulator, per-function initializer/local,
    // caller's current row, and combined candidate/caller assignment value.
    counts: [TypeCounts; 8],
    function_return: Result<TypeCounts, Box<Diagnostic>>,
    combined_return: Result<TypeCounts, Box<Diagnostic>>,
    combined_inputs: [&'static TypeCounts; 2],
    combined_origin: Span,
    addition_inputs: [usize; 2],
    addition_origin: Span,
    addition_option: Option<usize>,
    addition_return: Result<usize, Box<Diagnostic>>,
}
struct CountGuardCarriers {
    parameter: &'static ParameterTy,
    // The Value pattern binding precedes the distinct ordinary_value input.
    parameter_value: &'static ValueTy,
    value: &'static ValueTy,
    // ordinary_parameter forwards a distinct by-value origin to ordinary_value.
    origins: [Span; 2],
    returns: [Result<(), Box<Diagnostic>>; 2],
    visit_work: &'static WorkMeter,
    visit_origin: Span,
    visit_return: Result<(), Box<Diagnostic>>,
}
// Sum the distinct borrowed bindings in the disjoint While/If match arms.
// They coexist with the selected child copy; no branch slot reuse is assumed.
struct BodyChildBranches {
    while_body: &'static BodyBlockId,
    if_then: &'static BodyBlockId,
    if_else: &'static Option<BodyBlockId>,
}
struct BodyCountCarriers {
    function: &'static Function,
    work: &'static WorkMeter,
    origin: Span,
    frames: [Option<BlockCursor>; MAX_BLOCK_NESTING],
    inserted: BlockCursor,
    inserted_option: Option<BlockCursor>,
    cursor: &'static mut BlockCursor,
    cursor_option: Option<&'static mut BlockCursor>,
    cursor_result: Result<&'static mut BlockCursor, Box<Diagnostic>>,
    block: &'static BodyBlock,
    block_option: Option<&'static BodyBlock>,
    block_result: Result<&'static BodyBlock, Box<Diagnostic>>,
    statement: &'static Stmt,
    statement_option: Option<&'static Stmt>,
    child_option: Option<BodyBlockId>,
    child: BodyBlockId,
    child_branches: BodyChildBranches,
    depth: usize,
    maximum: usize,
    next_block: usize,
    statements: usize,
    returned_pair: (usize, usize),
    returned: Result<(usize, usize), Box<Diagnostic>>,
    caller_return: Result<(usize, usize), Box<Diagnostic>>,
    increment_returns: [Result<usize, Box<Diagnostic>>; 4],
    visit_returns: [Result<(), Box<Diagnostic>>; 3],
}
struct FunctionCountCarriers {
    records: &'static [Record],
    function: &'static Function,
    work: &'static WorkMeter,
    origin: Span,
    bindings: SliceLoop<Binding>,
    annotation: &'static Option<ValueTy>,
    annotation_value: &'static ValueTy,
    expressions: SliceLoop<Expr>,
    arguments: SliceLoop<Argument>,
    // Borrowed Call/StructLiteral pattern bindings and selected full record.
    call_arguments: &'static Vec<Argument>,
    record_id: &'static RecordId,
    record_option: Option<&'static Record>,
    record_result: Result<&'static Record, Box<Diagnostic>>,
    selected: &'static Record,
    statements: usize,
    maximum: usize,
    frame_product: Option<usize>,
    frame_sum: Option<usize>,
    frame_closure_input: usize,
    frame_result: Result<usize, Box<Diagnostic>>,
    addition_returns: [Result<usize, Box<Diagnostic>>; 5],
    visit_returns: [Result<(), Box<Diagnostic>>; 4],
}
struct ReconcileCarriers {
    counts: &'static TypeCounts,
    source: &'static HirPlan,
    work: &'static WorkMeter,
    origin: Span,
    limits: TypeCounts,
    // Four retained arrays plus constructor and returned arrays at each of the
    // four slots()/requests() call sites. No caller/return elision is assumed.
    arrays: [[usize; KINDS]; 12],
    array_inputs: [&'static TypeCounts; 4],
    cursor: std::ops::Range<usize>,
    next: Option<usize>,
    current: usize,
    presence_product: Option<usize>,
    presence_result: Result<usize, Box<Diagnostic>>,
    returns: [Result<(), Box<Diagnostic>>; 3],
}
struct PreparationCarriers {
    records: &'static [Record],
    signatures: &'static [Signature],
    functions: &'static [Function],
    source: &'static HirPlan,
    work: &'static WorkMeter,
    origin: Span,
    record_fields: usize,
    records_loop: EnumeratedLoop<Record>,
    fields_loop: SliceLoop<Field>,
    signatures_loop: SliceLoop<Signature>,
    parameters_loop: SliceLoop<ParameterTy>,
    functions_loop: EnumeratedLoop<Function>,
    // Constructed plan, return payload and separately retained caller plan.
    plans: [TypePlan<'static>; 2],
    returned: Result<TypePlan<'static>, Box<Diagnostic>>,
    // PaidStorage::new's constructor/return/caller are separately modeled by
    // AccountCarriers; this is the new field-expression result before move.
    constructed_storage: PaidStorage,
    record_field_addition: Result<usize, Box<Diagnostic>>,
    guard_returns: [Result<(), Box<Diagnostic>>; 5],
}
struct PartitionControls {
    plan: &'static mut TypePlan<'static>,
    work: &'static WorkMeter,
    origin: Span,
    function_option: Option<&'static Function>,
    function_result: Result<&'static Function, Box<Diagnostic>>,
    function: &'static Function,
    counts: TypeCounts,
    counts_return: Result<TypeCounts, Box<Diagnostic>>,
    storage: PaidStorage,
    // Explicit candidate array construction and independently named locals.
    arrays: [[usize; KINDS]; 4],
    cursor: std::ops::Range<usize>,
    iteration_next: Option<usize>,
    current: usize,
    slot_subtraction: Option<usize>,
    request_subtraction: Option<usize>,
    subtraction_returns: [Result<usize, Box<Diagnostic>>; 2],
    next: usize,
    next_return: Result<usize, Box<Diagnostic>>,
    ordinal: usize,
    quota: FunctionQuota,
    returned: Result<FunctionQuota, Box<Diagnostic>>,
    visit_returns: [Result<(), Box<Diagnostic>>; 2],
}
struct PartitionCarriers {
    controls: PartitionControls,
    caller: FunctionQuota,
}
struct BodiesReserveCarriers {
    plan: &'static mut TypePlan<'static>,
    allocator: &'static mut Allocator,
    origin: Span,
    returned: Result<Vec<TypedBody>, Box<Diagnostic>>,
    caller: Vec<TypedBody>,
}
pub(super) const fn count_carrier_bytes() -> usize {
    size_of::<CountReturnCarriers>()
        + size_of::<CountGuardCarriers>()
        + size_of::<BodyCountCarriers>()
        + size_of::<FunctionCountCarriers>()
}
pub(super) const fn preparation_carrier_bytes() -> usize {
    size_of::<PreparationCarriers>() + size_of::<ReconcileCarriers>()
}
pub(super) const fn partition_carrier_bytes() -> usize {
    size_of::<PartitionCarriers>() + size_of::<BodiesReserveCarriers>()
}

const fn reserve_control_bytes() -> usize {
    let mut largest = 0;
    macro_rules! include {
        ($($ty:ty),* $(,)?) => { $(
            let bytes = size_of::<ReserveControls<$ty>>();
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
        ValueTy
    );
    largest
}
const fn fill_control_bytes() -> usize {
    let mut largest = 0;
    macro_rules! include {
        ($($ty:ty),* $(,)?) => { $(
            let bytes = size_of::<FillControls<$ty>>();
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
/// One nonrecursive T0 execution bank, not complete checker/stack admission.
/// The construction must retain exactly one TypePlan and one Bodies receiver.
/// prepare is repeatable and does NOT enforce that multiplicity or confer
/// source association/one-shot authority. A future T1 entry must enforce it.
///
/// Existing HirPlan.fixed pays the selected Capacity/primitive Vec transports
/// once. Generic Vec caller roles are excluded from these cores: the reserve
/// receiver is the enclosing fill/finalize's local `values`, or Bodies' receiver.
/// Existing C actuals/L presence/three F stage/F frame headers pay those known
/// outer receivers. Remaining T1 cache/branch/final/path receivers are UNPAID.
/// In particular, a returned FieldId Vec receiver is separate from Projection
/// builder/Result and cache-embedded path headers; none implies its admission.
pub(super) const fn fixed_control_carrier_bytes() -> usize {
    size_of::<AccountControls>()
        + count_carrier_bytes()
        + preparation_carrier_bytes()
        + size_of::<PartitionControls>()
        + size_of::<BodiesReserveCarriers>()
        + reserve_control_bytes()
        + fill_control_bytes()
        + size_of::<FinalizeControls<ParameterTy>>()
        + size_of::<FinalizeControls<FlowSummary>>()
        + size_of::<FinalizeControls<ValueTy>>()
        + size_of::<ProjectionControls>()
}
/// Conservative per-function output allowance, not permission for a plan table.
/// Remove the same caller role from the fixed PartitionControls; keep its
/// distinct constructed quota and complete fallible return inside that bank.
pub(super) const fn function_output_carrier_bytes() -> usize {
    size_of::<FunctionQuota>()
}

#[cfg(test)]
#[path = "type_storage_tests.rs"]
mod tests;
