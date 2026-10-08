//! Private, disconnected requested-storage helper for the still-denied leaf.
//!
//! This borrows shape only. Genuine source/OPA correspondence and the canonical
//! resolver owner must be established by the enclosing private leaf. Neither a
//! supplied fixed-byte charge nor this helper establishes source authority.
//! No Program is built or returned here.
//!
//! The admitted affected-importer bound is Hc + Hn + named fixed carriers. The
//! separately owned source/AST/capture baseline is not observed by this helper.
//! Allocator metadata/rounding, physical memory and general machine stack are
//! not bounded. An allocator may briefly return excess capacity; it is rejected
//! and immediately dropped, so only successful retained vectors have exact cap.
use super::{hir, Boundary, Counts, StoragePlan, ELEMENT_BYTES, MAX_ROWS};
use crate::frontend::{
    declaration_index::IndexLimits,
    project::budget::{Allocator, ReserveFailure},
};
use std::{
    alloc::Layout,
    cell::{Cell, RefCell},
    mem::size_of,
};

// Two roots, <=128 signatures, three vectors per <=128 functions, <=128
// blocks, and <=128 call expressions. This is a derived row-domain bound,
// not a new compiler ceiling. No table with this many entries is retained.
const MAX_VECTORS: usize = 2 + 6 * MAX_ROWS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Failure {
    Overflow,
    Admission,
    Shape,
    Allocation,
}
impl From<Boundary> for Failure {
    fn from(value: Boundary) -> Self {
        match value {
            Boundary::Overflow => Self::Overflow,
            _ => Self::Shape,
        }
    }
}
impl From<ReserveFailure> for Failure {
    fn from(value: ReserveFailure) -> Self {
        match value {
            ReserveFailure::Overflow => Self::Overflow,
            ReserveFailure::Allocation => Self::Allocation,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Family {
    Signatures,
    Functions,
    Parameters,
    Locals,
    Expressions,
    Blocks,
    Statements,
    Arguments,
}
impl Family {
    fn index(self) -> usize {
        self as usize
    }
    fn kind(self) -> &'static str {
        match self {
            Self::Signatures => "checked HIR candidate signatures",
            Self::Functions => "checked HIR candidate functions",
            Self::Parameters => "checked HIR candidate parameters",
            Self::Locals => "checked HIR candidate locals",
            Self::Expressions => "checked HIR candidate expressions",
            Self::Blocks => "checked HIR candidate blocks",
            Self::Statements => "checked HIR candidate statements",
            Self::Arguments => "checked HIR candidate arguments",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Key {
    Signatures,
    Functions,
    Parameters(usize),
    Locals(usize),
    Expressions(usize),
    Blocks(usize),
    Statements { function: usize, block: usize },
    Arguments { function: usize, expression: usize },
}
impl Key {
    fn family(self) -> Family {
        match self {
            Self::Signatures => Family::Signatures,
            Self::Functions => Family::Functions,
            Self::Parameters(_) => Family::Parameters,
            Self::Locals(_) => Family::Locals,
            Self::Expressions(_) => Family::Expressions,
            Self::Blocks(_) => Family::Blocks,
            Self::Statements { .. } => Family::Statements,
            Self::Arguments { .. } => Family::Arguments,
        }
    }

    fn shape(self, program: &hir::Program) -> Result<(usize, usize), Failure> {
        fn lengths<T>(values: &Vec<T>) -> (usize, usize) {
            (values.len(), values.capacity())
        }
        Ok(match self {
            Self::Signatures => lengths(&program.signatures),
            Self::Functions => lengths(&program.functions),
            Self::Parameters(s) => {
                lengths(&program.signatures.get(s).ok_or(Failure::Shape)?.params)
            }
            Self::Locals(f) => lengths(&program.functions.get(f).ok_or(Failure::Shape)?.locals),
            Self::Expressions(f) => {
                lengths(&program.functions.get(f).ok_or(Failure::Shape)?.expressions)
            }
            Self::Blocks(f) => lengths(&program.functions.get(f).ok_or(Failure::Shape)?.blocks),
            Self::Statements { function, block } => lengths(
                &program
                    .functions
                    .get(function)
                    .and_then(|f| f.blocks.get(block))
                    .ok_or(Failure::Shape)?
                    .body,
            ),
            Self::Arguments {
                function,
                expression,
            } => {
                let expression = program
                    .functions
                    .get(function)
                    .and_then(|f| f.expressions.get(expression))
                    .ok_or(Failure::Shape)?;
                let hir::ExprKind::Call { args, .. } = &expression.kind else {
                    return Err(Failure::Shape);
                };
                lengths(args)
            }
        })
    }
}

mod sealed {
    pub trait Sealed {}
}
// Only these eight real HIR element types can instantiate the private helper.
pub(super) trait Element: sealed::Sealed {
    const FAMILY: Family;
}
macro_rules! element {
    ($ty:ty, $family:ident) => {
        impl sealed::Sealed for $ty {}
        impl Element for $ty {
            const FAMILY: Family = Family::$family;
        }
    };
}
element!(hir::Signature, Signatures);
element!(hir::Function, Functions);
element!(hir::Ty, Parameters);
element!(hir::Local, Locals);
element!(hir::Expr, Expressions);
element!(hir::BodyBlock, Blocks);
element!(hir::Stmt, Statements);
element!(hir::ExprId, Arguments);

fn checked_layout<T>(slots: usize) -> Result<Layout, Failure> {
    let width = size_of::<T>();
    if width == 0 {
        return Err(Failure::Shape);
    }
    let bytes = slots.checked_mul(width).ok_or(Failure::Overflow)?;
    let layout = Layout::array::<T>(slots).map_err(|_| Failure::Overflow)?;
    if layout.size() != bytes {
        return Err(Failure::Overflow);
    }
    Ok(layout)
}
fn family_layout(family: Family, slots: usize) -> Result<Layout, Failure> {
    match family {
        Family::Signatures => checked_layout::<hir::Signature>(slots),
        Family::Functions => checked_layout::<hir::Function>(slots),
        Family::Parameters => checked_layout::<hir::Ty>(slots),
        Family::Locals => checked_layout::<hir::Local>(slots),
        Family::Expressions => checked_layout::<hir::Expr>(slots),
        Family::Blocks => checked_layout::<hir::BodyBlock>(slots),
        Family::Statements => checked_layout::<hir::Stmt>(slots),
        Family::Arguments => checked_layout::<hir::ExprId>(slots),
    }
}

#[derive(Clone, Copy)]
enum Order {
    Reserve,
    Finish,
}

// Re-read immutable shape instead of retaining descriptor vectors or maps.
// The finish order permits locally owned parents to wait for all their children:
// params -> signatures; per function locals -> statements -> blocks -> call
// args -> expressions; functions last. Every zero-length vector is included.
fn key_at(program: &hir::Program, order: Order, mut ordinal: usize) -> Option<Key> {
    macro_rules! visit {
        ($key:expr) => {
            if ordinal == 0 {
                return Some($key);
            } else {
                ordinal -= 1;
            }
        };
    }
    if matches!(order, Order::Reserve) {
        visit!(Key::Signatures);
        visit!(Key::Functions);
    }
    for s in 0..program.signatures.len() {
        visit!(Key::Parameters(s));
    }
    if matches!(order, Order::Finish) {
        visit!(Key::Signatures);
    }
    for (f, function) in program.functions.iter().enumerate() {
        visit!(Key::Locals(f));
        if matches!(order, Order::Reserve) {
            visit!(Key::Expressions(f));
            visit!(Key::Blocks(f));
        }
        for b in 0..function.blocks.len() {
            visit!(Key::Statements {
                function: f,
                block: b,
            });
        }
        if matches!(order, Order::Finish) {
            visit!(Key::Blocks(f));
        }
        for (e, expression) in function.expressions.iter().enumerate() {
            if matches!(expression.kind, hir::ExprKind::Call { .. }) {
                visit!(Key::Arguments {
                    function: f,
                    expression: e,
                });
            }
        }
        if matches!(order, Order::Finish) {
            visit!(Key::Expressions(f));
        }
    }
    if matches!(order, Order::Finish) {
        visit!(Key::Functions);
    }
    let _ = ordinal;
    None
}

#[derive(Default)]
struct Inventory {
    requested: Counts,
    canonical_capacity: Counts,
    vectors: usize,
    nonempty: usize,
}
impl Inventory {
    fn read(program: &hir::Program) -> Result<Self, Failure> {
        // Bound enclosing loops before the shape re-read starts. Family running
        // totals below also reject excessive nested distribution immediately.
        if program.signatures.len() > MAX_ROWS || program.functions.len() > MAX_ROWS {
            return Err(Failure::Shape);
        }
        let mut result = Self::default();
        while let Some(key) = key_at(program, Order::Reserve, result.vectors) {
            let (slots, capacity) = key.shape(program)?;
            let family = key.family();
            result.requested.add(family.index(), slots)?;
            if result.requested.0[family.index()] > MAX_ROWS {
                return Err(Failure::Shape);
            }
            result.canonical_capacity.add(family.index(), capacity)?;
            family_layout(family, slots)?;
            result.vectors = result.vectors.checked_add(1).ok_or(Failure::Overflow)?;
            if result.vectors > MAX_VECTORS {
                return Err(Failure::Shape);
            }
            result.nonempty = result
                .nonempty
                .checked_add(usize::from(slots != 0))
                .ok_or(Failure::Overflow)?;
        }
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Receipt {
    pub(super) requested: Counts,
    pub(super) vectors: usize,
    pub(super) reserves: usize,
    pub(super) canonical_payload_bytes: usize,
    pub(super) candidate_request_bytes: usize,
    pub(super) helper_named_bytes: usize,
    pub(super) fixed_bytes: usize,
    pub(super) affected_bytes: usize,
    pub(super) helper_work: u64,
}

#[derive(Clone, Copy, Default)]
struct Progress {
    vectors: usize,
    nonempty: usize,
    slots: Counts,
}

#[derive(Clone, Copy)]
struct Request {
    key: Key,
    ordinal: usize,
    slots: usize,
    layout: Layout,
}

pub(super) struct Session<'h, 'a> {
    canonical: &'h hir::Program,
    allocator: RefCell<&'a mut Allocator>,
    plan: StoragePlan,
    receipt: Receipt,
    first_attempt: usize,
    end_attempt: usize,
    reserved: Cell<Progress>,
    finished: Cell<Progress>,
    failed: Cell<bool>,
}
impl<'h, 'a> Session<'h, 'a> {
    /// Planned/admitted scalar metadata only. This does not certify completed
    /// fills and cannot replace complete() or provide an owner/reserve token.
    pub(super) fn admitted_receipt(&self) -> Receipt {
        self.receipt
    }

    /// `outside_fixed_bytes` must come from the enclosing leaf's measured map,
    /// row, source-bound argument and transport roles. It is not a provenance
    /// claim. Its Program header roles must include canonical and candidate
    /// owners exactly once. This helper uses Hc + Hn, not the old hypothetical
    /// pair subtotal, so it does not add those two Program headers again.
    /// The caller must prepare cfg(test) trace backing before this interval and
    /// price that instrumentation separately. It is not candidate payload.
    pub(super) fn admit(
        canonical: &'h hir::Program,
        allocator: &'a mut Allocator,
        outside_fixed_bytes: usize,
        limits: IndexLimits,
    ) -> Result<Self, Failure> {
        let inventory = Inventory::read(canonical)?;
        let plan = StoragePlan::describe(canonical)?;
        if inventory.requested != plan.requested
            || inventory.canonical_capacity != plan.canonical_capacity
        {
            return Err(Failure::Shape);
        }
        let first_attempt = allocator.attempts;
        let end_attempt = first_attempt
            .checked_add(inventory.nonempty)
            .ok_or(Failure::Overflow)?;
        let helper_named_bytes = helper_named_bytes()?;
        let fixed_bytes = outside_fixed_bytes
            .checked_add(helper_named_bytes)
            .ok_or(Failure::Overflow)?;
        let affected_bytes = plan
            .canonical_payload_bytes
            .checked_add(plan.candidate_request_bytes)
            .and_then(|n| n.checked_add(fixed_bytes))
            .ok_or(Failure::Overflow)?;
        let helper_work = work_bound(&inventory)?;
        admit_limits(affected_bytes, fixed_bytes, helper_work, limits)?;
        let receipt = Receipt {
            requested: plan.requested,
            vectors: inventory.vectors,
            reserves: inventory.nonempty,
            canonical_payload_bytes: plan.canonical_payload_bytes,
            candidate_request_bytes: plan.candidate_request_bytes,
            helper_named_bytes,
            fixed_bytes,
            affected_bytes,
            helper_work,
        };
        Ok(Self {
            canonical,
            allocator: RefCell::new(allocator),
            plan,
            receipt,
            first_attempt,
            end_attempt,
            reserved: Cell::new(Progress::default()),
            finished: Cell::new(Progress::default()),
            failed: Cell::new(false),
        })
    }

    fn live(&self) -> Result<(), Failure> {
        if self.failed.get() {
            Err(Failure::Shape)
        } else {
            Ok(())
        }
    }
    fn observe<T>(&self, result: Result<T, Failure>) -> Result<T, Failure> {
        if result.is_err() {
            self.failed.set(true);
        }
        result
    }

    pub(super) fn reserve<T: Element>(
        &self,
        key: Key,
        mapped_len: usize,
    ) -> Result<ExactVec<'_, 'h, 'a, T>, Failure> {
        self.observe(self.reserve_inner(key, mapped_len))
    }
    fn reserve_inner<T: Element>(
        &self,
        key: Key,
        mapped_len: usize,
    ) -> Result<ExactVec<'_, 'h, 'a, T>, Failure> {
        self.live()?;
        let progress = self.reserved.get();
        if key_at(self.canonical, Order::Reserve, progress.vectors) != Some(key)
            || T::FAMILY != key.family()
            || size_of::<T>() != ELEMENT_BYTES[key.family().index()]
        {
            return Err(Failure::Shape);
        }
        let (slots, _) = key.shape(self.canonical)?;
        if slots > MAX_ROWS || mapped_len != slots {
            return Err(Failure::Shape);
        }
        let request = Request {
            key,
            ordinal: progress.vectors,
            slots,
            layout: checked_layout::<T>(slots)?,
        };
        let next = advance(progress, &request, self.plan.requested)?;
        let expected_attempt = self
            .first_attempt
            .checked_add(progress.nonempty)
            .ok_or(Failure::Overflow)?;
        let next_attempt = self
            .first_attempt
            .checked_add(next.nonempty)
            .ok_or(Failure::Overflow)?;
        if next.vectors > self.receipt.vectors || next_attempt > self.end_attempt {
            return Err(Failure::Shape);
        }
        let mut values = Vec::<T>::new();
        {
            let mut allocator = self.allocator.borrow_mut();
            if allocator.attempts != expected_attempt {
                return Err(Failure::Shape);
            }
            if slots != 0 {
                allocator.vector_exact(&mut values, slots, key.family().kind())?;
            }
            if allocator.attempts != next_attempt {
                return Err(Failure::Shape);
            }
        }
        exact_capacity(&values, slots)?;
        if !values.is_empty() {
            return Err(Failure::Shape);
        }
        self.reserved.set(next);
        Ok(ExactVec {
            owner: self,
            request,
            values,
        })
    }

    /// Fixed facts only. This does not confer import or executable authority.
    pub(super) fn complete(&self) -> Result<Receipt, Failure> {
        self.observe(self.complete_inner())
    }
    fn complete_inner(&self) -> Result<Receipt, Failure> {
        self.live()?;
        for (order, progress) in [
            (Order::Reserve, self.reserved.get()),
            (Order::Finish, self.finished.get()),
        ] {
            if progress.vectors != self.receipt.vectors
                || progress.nonempty != self.receipt.reserves
                || progress.slots != self.plan.requested
                || key_at(self.canonical, order, progress.vectors).is_some()
            {
                return Err(Failure::Shape);
            }
        }
        if self.allocator.borrow().attempts != self.end_attempt {
            return Err(Failure::Shape);
        }
        Ok(self.receipt)
    }
}

fn advance(mut progress: Progress, request: &Request, total: Counts) -> Result<Progress, Failure> {
    progress.vectors = progress.vectors.checked_add(1).ok_or(Failure::Overflow)?;
    progress.nonempty = progress
        .nonempty
        .checked_add(usize::from(request.slots != 0))
        .ok_or(Failure::Overflow)?;
    progress
        .slots
        .add(request.key.family().index(), request.slots)?;
    if progress
        .slots
        .0
        .iter()
        .zip(total.0)
        .any(|(&filled, planned)| filled > planned)
    {
        return Err(Failure::Shape);
    }
    // The complete Hn was prepaid. Recheck the actual aggregate prefix using
    // the same checked byte arithmetic, rather than relying only on counts.
    if progress.slots.payload_bytes()? > total.payload_bytes()? {
        return Err(Failure::Shape);
    }
    Ok(progress)
}

pub(super) struct ExactVec<'v, 'h, 'a, T: Element> {
    // A real borrow brands this vector to one immovable, live Session. There
    // is no public ticket, arbitrary owner parameter or cross-session finish.
    owner: &'v Session<'h, 'a>,
    request: Request,
    values: Vec<T>,
}
impl<T: Element> ExactVec<'_, '_, '_, T> {
    /// The caller validates the source/OPA ID; this additionally requires its
    /// mapped output ordinal to be the next exact vector position.
    pub(super) fn push(&mut self, mapped_index: usize, value: T) -> Result<(), Failure> {
        let result = (|| {
            self.owner.live()?;
            exact_capacity(&self.values, self.request.slots)?;
            if mapped_index != self.values.len() || self.values.len() >= self.request.slots {
                return Err(Failure::Shape);
            }
            self.values.push(value);
            Ok(())
        })();
        self.owner.observe(result)
    }

    pub(super) fn finish(self) -> Result<Vec<T>, Failure> {
        let result = (|| {
            self.owner.live()?;
            exact_capacity(&self.values, self.request.slots)?;
            let progress = self.owner.finished.get();
            let (slots, _) = self.request.key.shape(self.owner.canonical)?;
            if self.values.len() != self.request.slots
                || slots != self.request.slots
                || self.request.layout != checked_layout::<T>(slots)?
                || key_at(self.owner.canonical, Order::Finish, progress.vectors)
                    != Some(self.request.key)
                || key_at(self.owner.canonical, Order::Reserve, self.request.ordinal)
                    != Some(self.request.key)
                || self.request.ordinal >= self.owner.reserved.get().vectors
            {
                return Err(Failure::Shape);
            }
            let next = advance(progress, &self.request, self.owner.plan.requested)?;
            self.owner.finished.set(next);
            Ok(())
        })();
        self.owner.observe(result)?;
        Ok(self.values)
    }
}

fn exact_capacity<T>(values: &Vec<T>, slots: usize) -> Result<(), Failure> {
    if values.capacity() != slots {
        Err(Failure::Shape)
    } else {
        Ok(())
    }
}

fn admit_limits(
    affected: usize,
    fixed: usize,
    work: u64,
    requested: IndexLimits,
) -> Result<(), Failure> {
    let ceilings = IndexLimits::default();
    let retained = u64::try_from(affected).map_err(|_| Failure::Overflow)?;
    let scratch = u64::try_from(fixed).map_err(|_| Failure::Overflow)?;
    if retained > requested.retained.min(ceilings.retained)
        || scratch > requested.scratch.min(ceilings.scratch)
        || work > requested.work.min(ceilings.work)
    {
        Err(Failure::Admission)
    } else {
        Ok(())
    }
}

fn work_bound(inventory: &Inventory) -> Result<u64, Failure> {
    // At most V+1 inventory lookups, V reserves, and two lookups per V finishes,
    // plus two terminal lookups. Each re-read examines <=V vector keys, <=E
    // expression kinds, and <=F function headers. Eight units per examined
    // item also cover key/length/layout/arithmetic guards; extra V and slots
    // terms cover StoragePlan's walk, completion checks and guarded pushes.
    // This prices shape bookkeeping only; enclosing source/OPA comparison work
    // must retain its own existing bounded admission.
    let vectors = inventory.vectors;
    let scan = vectors
        .checked_add(inventory.requested.0[4])
        .and_then(|n| n.checked_add(inventory.requested.0[1]))
        .and_then(|n| n.checked_add(16))
        .ok_or(Failure::Overflow)?;
    let scans = vectors
        .checked_mul(4)
        .and_then(|n| n.checked_add(4))
        .ok_or(Failure::Overflow)?;
    let slots = checked_sum(inventory.requested.0)?;
    let units = scans
        .checked_mul(scan)
        .and_then(|n| n.checked_add(vectors))
        .and_then(|n| n.checked_add(slots))
        .and_then(|n| n.checked_mul(8))
        .ok_or(Failure::Overflow)?;
    u64::try_from(units).map_err(|_| Failure::Overflow)
}

fn checked_sum<const N: usize>(values: [usize; N]) -> Result<usize, Failure> {
    values.into_iter().try_fold(0usize, |sum, value| {
        sum.checked_add(value).ok_or(Failure::Overflow)
    })
}

fn copies<T>(count: usize) -> Result<usize, Failure> {
    size_of::<T>().checked_mul(count).ok_or(Failure::Overflow)
}

/// A conservative named-value envelope, not measured stack usage. Complete
/// enclosing types already include their fields; extra entries below name
/// separate local/input/return/caller roles. All eight typed reserve/fill/finish
/// chains are charged, even though their scalar helper lifetimes do not overlap.
/// No comparator bank, Program owner/header, source owner or AST baseline is
/// silently invented here: those belong to the supplied outside-fixed term/B.
pub(super) fn helper_named_bytes() -> Result<usize, Failure> {
    let roles = [
        // Admission inputs; preflight local and complete result; existing plan
        // description accumulators, local and full returned plan; caller plan.
        size_of::<(&hir::Program, &mut Allocator, usize, IndexLimits)>(),
        size_of::<Inventory>(),
        size_of::<Result<Inventory, Failure>>(),
        size_of::<Inventory>(),
        copies::<Counts>(2)?,
        copies::<usize>(3)?,
        size_of::<StoragePlan>(),
        size_of::<Result<StoragePlan, Boundary>>(),
        size_of::<StoragePlan>(),
        size_of::<Receipt>(),
        // Read-only admitted metadata receiver and complete return/caller copy.
        size_of::<&Session<'_, '_>>(),
        copies::<Receipt>(2)?,
        // Local admission result, forwarding return, caller result and owner.
        size_of::<Session<'_, '_>>(),
        copies::<Result<Session<'_, '_>, Failure>>(3)?,
        size_of::<Session<'_, '_>>(),
        // Live bounded admission arithmetic and limit-check inputs/locals.
        copies::<usize>(8)?,
        copies::<u64>(4)?,
        copies::<IndexLimits>(2)?,
        // Complete shape-lookup inputs, loop state, lookup return, per-vector
        // inventory locals and checked layout return across nested helpers.
        size_of::<(&hir::Program, Order, usize)>(),
        size_of::<Option<Key>>(),
        size_of::<(usize, &hir::Function, usize, &hir::Expr)>(),
        size_of::<(Key, Family, usize, usize)>(),
        size_of::<Result<(usize, usize), Failure>>(),
        size_of::<Result<Layout, Failure>>(),
        size_of::<Layout>(),
        copies::<usize>(4)?,
        // Work/sum inputs and loop/arithmetic locals, final receipt transport,
        // terminal loop state and fixed unit/error transport on helper chains.
        size_of::<[usize; 8]>(),
        copies::<usize>(8)?,
        copies::<Result<usize, Failure>>(3)?,
        size_of::<Result<u64, Failure>>(),
        copies::<Result<Receipt, Failure>>(3)?,
        size_of::<[(Order, Progress); 2]>(),
        copies::<Result<(), Failure>>(8)?,
        vector_named_bytes::<hir::Signature>()?,
        vector_named_bytes::<hir::Function>()?,
        vector_named_bytes::<hir::Ty>()?,
        vector_named_bytes::<hir::Local>()?,
        vector_named_bytes::<hir::Expr>()?,
        vector_named_bytes::<hir::BodyBlock>()?,
        vector_named_bytes::<hir::Stmt>()?,
        vector_named_bytes::<hir::ExprId>()?,
    ];
    // Include the actual named-cost array itself, its by-value sum input and
    // IntoIter owner. This bank is scalar size measurements, not shape records.
    let bank = std::mem::size_of_val(&roles)
        .checked_mul(2)
        .and_then(|n| n.checked_add(std::mem::size_of_val(&roles.into_iter())))
        .ok_or(Failure::Overflow)?;
    checked_sum(roles)?
        .checked_add(bank)
        .ok_or(Failure::Overflow)
}

fn vector_named_bytes<T: Element>() -> Result<usize, Failure> {
    let roles = [
        // reserve + reserve_inner inputs and fresh local Vec, request and
        // progress temporaries, exact wrapper local/forwarding/caller roles.
        copies::<(&Session<'_, '_>, Key, usize)>(2)?,
        size_of::<Vec<T>>(),
        size_of::<Request>(),
        copies::<Progress>(3)?,
        copies::<usize>(4)?,
        size_of::<std::cell::RefMut<'_, &mut Allocator>>(),
        size_of::<ExactVec<'_, '_, '_, T>>(),
        copies::<Result<ExactVec<'_, '_, '_, T>, Failure>>(3)?,
        size_of::<ExactVec<'_, '_, '_, T>>(),
        // push input/value and complete finish self/result/caller-vector roles.
        size_of::<(&mut ExactVec<'_, '_, '_, T>, usize, T)>(),
        size_of::<ExactVec<'_, '_, '_, T>>(),
        copies::<Result<Vec<T>, Failure>>(2)?,
        size_of::<Vec<T>>(),
        copies::<Result<(), Failure>>(2)?,
        size_of::<Result<Progress, Failure>>(),
        size_of::<Result<Layout, Failure>>(),
    ];
    let bank = std::mem::size_of_val(&roles)
        .checked_mul(2)
        .and_then(|n| n.checked_add(std::mem::size_of_val(&roles.into_iter())))
        .ok_or(Failure::Overflow)?;
    checked_sum(roles)?
        .checked_add(bank)
        .ok_or(Failure::Overflow)
}

#[cfg(test)]
mod tests;
