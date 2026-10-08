//! Private comparison and fixed-facts Verify entries. No default caller is
//! connected. The experimental facade uses fixed Verify/Run facts or owned text.
use super::{
    allocation, ast_compare, candidate, source_work_bound_for, BoundObservation, Boundary, Protocol,
};
use crate::frontend::{
    declaration_index::{IndexLimits, SourceOwner, WorkMeter},
    diagnostic::Diagnostic,
    hir,
    oir::native::emit_work,
    project::{budget::Allocator, ModuleId},
    source::{SourceFile, SourceView, Span},
};
use std::{
    convert::Infallible,
    mem::{size_of, size_of_val},
};

// Validation instrumentation only. No production field, allocation hook or
// caller-controlled owner is added. Reset at every entry, including preflight
// failure, so stale observations cannot qualify a later request.
#[cfg(test)]
type AllocationObservation = (usize, usize, isize, isize);
#[cfg(test)]
thread_local! {
    static LAST_CANDIDATE_OBSERVATION: std::cell::Cell<Option<AllocationObservation>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
pub(super) fn take_candidate_observation() -> Option<AllocationObservation> {
    LAST_CANDIDATE_OBSERVATION.with(|slot| slot.take())
}

#[derive(Debug)]
pub(super) struct Facts {
    pub(super) candidate: candidate::ComparisonFacts,
    pub(super) source_work: u64,
    pub(super) canonical_work: u64,
    pub(super) total_work: u64,
    pub(super) outside_fixed_bytes: usize,
}
#[derive(Debug)]
pub(super) enum Rejected {
    Boundary(Boundary),
    Budget,
    Canonical(Vec<Diagnostic>),
    Candidate(allocation::Failure),
    Mismatch(Facts),
    Compared(Facts),
}
impl From<Boundary> for Rejected {
    fn from(value: Boundary) -> Self {
        Self::Boundary(value)
    }
}
impl From<allocation::Failure> for Rejected {
    fn from(value: allocation::Failure) -> Self {
        Self::Candidate(value)
    }
}

#[derive(Debug)]
pub(super) struct VerifyFacts {
    pub(super) verified: candidate::VerifyFacts,
    pub(super) source_work: u64,
    pub(super) canonical_work: u64,
    pub(super) total_work: u64,
    pub(super) outside_fixed_bytes: usize,
}
#[derive(Debug)]
pub(super) struct EmitOutput {
    pub(super) artifact: candidate::EmitArtifact,
    pub(super) source_work: u64,
    pub(super) canonical_work: u64,
    pub(super) total_work: u64,
    pub(super) outside_fixed_bytes: usize,
}
#[derive(Debug)]
enum Requested {
    Fixed(VerifyFacts),
    Emitted(EmitOutput),
}
#[derive(Debug)]
pub(super) enum VerifyRejected {
    Disabled,
    Source(Rejected),
    Terminal(candidate::VerifyRejected),
}
impl From<Boundary> for VerifyRejected {
    fn from(value: Boundary) -> Self {
        Self::Source(Rejected::Boundary(value))
    }
}
impl From<candidate::VerifyRejected> for VerifyRejected {
    fn from(value: candidate::VerifyRejected) -> Self {
        Self::Terminal(value)
    }
}

#[derive(Clone, Copy, Debug)]
struct SourcePlan {
    outside_fixed_bytes: usize,
    fixed_bytes: usize,
    source_work: u64,
}
impl SourcePlan {
    #[allow(clippy::result_large_err)] // Fixed denial facts are deliberately prepaid, never boxed.
    fn calculate(limits: IndexLimits) -> Result<Self, Rejected> {
        Self::calculate_for(limits, Protocol::V1)
    }
    #[allow(clippy::result_large_err)]
    fn calculate_for(limits: IndexLimits, protocol: Protocol) -> Result<Self, Rejected> {
        let outside_fixed_bytes = outer_named_bytes()?
            .checked_add(ast_compare::named_bytes()?)
            .ok_or(Boundary::Overflow)?;
        let builder_named = candidate::builder_named_bytes()?;
        let helper_named = allocation::helper_named_bytes()?;
        let fixed_bytes = outside_fixed_bytes
            .checked_add(builder_named)
            .and_then(|n| n.checked_add(helper_named))
            .ok_or(Boundary::Overflow)?;
        let source_work = source_work_bound_for(protocol)?;
        let default = IndexLimits::default();
        let bytes = u64::try_from(fixed_bytes).map_err(|_| Boundary::Overflow)?;
        if bytes > limits.scratch.min(default.scratch)
            || bytes > limits.retained.min(default.retained)
            || source_work > limits.work.min(default.work)
        {
            return Err(Rejected::Budget);
        }
        Ok(Self {
            outside_fixed_bytes,
            fixed_bytes,
            source_work,
        })
    }

    #[allow(clippy::result_large_err)]
    fn calculate_verify(limits: IndexLimits) -> Result<Self, Rejected> {
        Self::calculate_verify_for(limits, Protocol::V1)
    }
    #[allow(clippy::result_large_err)]
    fn calculate_verify_for(limits: IndexLimits, protocol: Protocol) -> Result<Self, Rejected> {
        let mut plan = Self::calculate_for(limits, protocol)?;
        let extra = verify_outer_named_bytes()?
            .checked_add(candidate::verify_named_bytes()?)
            .ok_or(Boundary::Overflow)?;
        plan.outside_fixed_bytes = plan
            .outside_fixed_bytes
            .checked_add(extra)
            .ok_or(Boundary::Overflow)?;
        plan.fixed_bytes = plan
            .fixed_bytes
            .checked_add(extra)
            .ok_or(Boundary::Overflow)?;
        let default = IndexLimits::default();
        let bytes = u64::try_from(plan.fixed_bytes).map_err(|_| Boundary::Overflow)?;
        if bytes > limits.scratch.min(default.scratch)
            || bytes > limits.retained.min(default.retained)
        {
            return Err(Rejected::Budget);
        }
        Ok(plan)
    }

    #[allow(clippy::result_large_err)]
    fn calculate_emit(limits: IndexLimits) -> Result<Self, Rejected> {
        Self::calculate_emit_for(limits, Protocol::V1)
    }
    #[allow(clippy::result_large_err)]
    fn calculate_emit_for(limits: IndexLimits, protocol: Protocol) -> Result<Self, Rejected> {
        let mut plan = Self::calculate_verify_for(limits, protocol)?;
        let extra = emit_outer_named_bytes()?
            .checked_add(candidate::emit_named_bytes()?)
            .ok_or(Boundary::Overflow)?;
        plan.outside_fixed_bytes = plan
            .outside_fixed_bytes
            .checked_add(extra)
            .ok_or(Boundary::Overflow)?;
        plan.fixed_bytes = plan
            .fixed_bytes
            .checked_add(extra)
            .ok_or(Boundary::Overflow)?;
        let default = IndexLimits::default();
        let bytes = u64::try_from(plan.fixed_bytes).map_err(|_| Boundary::Overflow)?;
        if bytes > limits.scratch.min(default.scratch)
            || bytes > limits.retained.min(default.retained)
        {
            return Err(Rejected::Budget);
        }
        Ok(plan)
    }
}

/// Private fixed-facts Verify entry. Admission is compile-time only; no caller
/// flag inside this leaf, default source route, or cfg(test) bypass exists.
#[allow(clippy::result_large_err)]
pub(super) fn verify(
    owner: SourceOwner<'_>,
    captured_source: &[u8],
    observation: &[u8],
    candidate_allocator: &mut Allocator,
    limits: IndexLimits,
) -> Result<VerifyFacts, VerifyRejected> {
    match requested(
        candidate::Request::Verify,
        owner,
        captured_source,
        observation,
        candidate_allocator,
        limits,
    )? {
        Requested::Fixed(facts) => Ok(facts),
        Requested::Emitted(_) => {
            Err(candidate::VerifyRejected::Candidate(allocation::Failure::Shape).into())
        }
    }
}

/// Private Run uses the same complete checked terminal. No test-only switch,
/// caller-supplied entry, or default source route exists.
#[allow(clippy::result_large_err)]
pub(super) fn run(
    owner: SourceOwner<'_>,
    captured_source: &[u8],
    observation: &[u8],
    candidate_allocator: &mut Allocator,
    limits: IndexLimits,
) -> Result<VerifyFacts, VerifyRejected> {
    if !candidate::RUN_ADMITTED {
        return Err(VerifyRejected::Disabled);
    }
    match requested(
        candidate::Request::Run,
        owner,
        captured_source,
        observation,
        candidate_allocator,
        limits,
    )? {
        Requested::Fixed(facts) => Ok(facts),
        Requested::Emitted(_) => {
            Err(candidate::VerifyRejected::Candidate(allocation::Failure::Shape).into())
        }
    }
}

/// Owned-text transport through the compiled paid terminal. No supplied fact
/// or caller flag can skip source binding, checking, comparison or admission.
#[allow(clippy::result_large_err)]
pub(super) fn emit(
    owner: SourceOwner<'_>,
    captured_source: &[u8],
    observation: &[u8],
    candidate_allocator: &mut Allocator,
    limits: IndexLimits,
) -> Result<EmitOutput, VerifyRejected> {
    if !candidate::EMIT_ADMITTED {
        return Err(VerifyRejected::Disabled);
    }
    match requested(
        candidate::Request::Emit,
        owner,
        captured_source,
        observation,
        candidate_allocator,
        limits,
    )? {
        Requested::Emitted(output) => Ok(output),
        Requested::Fixed(_) => {
            Err(candidate::VerifyRejected::Candidate(allocation::Failure::Shape).into())
        }
    }
}

/// One complete source/canonical/candidate/checker terminal. The shared enum's
/// full layout is paid even for the enabled fixed-facts requests.
#[allow(clippy::result_large_err)]
fn requested(
    request: candidate::Request,
    owner: SourceOwner<'_>,
    captured_source: &[u8],
    observation: &[u8],
    candidate_allocator: &mut Allocator,
    limits: IndexLimits,
) -> Result<Requested, VerifyRejected> {
    if !candidate::VERIFY_ADMITTED {
        return Err(VerifyRejected::Disabled);
    }
    if request == candidate::Request::Run && !candidate::RUN_ADMITTED {
        return Err(VerifyRejected::Disabled);
    }
    if request == candidate::Request::Emit && !candidate::EMIT_ADMITTED {
        return Err(VerifyRejected::Disabled);
    }
    let protocol = Protocol::from_opa(observation).unwrap_or(Protocol::V1);
    let (plan, work, origin) = if request == candidate::Request::Emit {
        // Explicit unmetered, constant-time bootstrap: inspect the genuine
        // owner's count/view, look up root file 0 to obtain its real empty span,
        // cap the requested work limit, and construct the original meter. No
        // bank calculation, source/path walk or compiler consumer occurs here.
        if owner.count() != 1 || !matches!(owner.view(), SourceView::Map(_)) {
            return Err(Boundary::Domain.into());
        }
        let origin = owner
            .file(ModuleId(0))
            .map_err(|_| Boundary::Source)?
            .span(0, 0);
        let work = WorkMeter::new(limits.work.min(IndexLimits::default().work));
        let plan = metered_emit_plan_for(limits, &work, origin, protocol)?;
        (plan, work, origin)
    } else {
        // Preserve Verify/Run's existing preflight/error order and work cost.
        let plan =
            SourcePlan::calculate_verify_for(limits, protocol).map_err(VerifyRejected::Source)?;
        if owner.count() != 1 || !matches!(owner.view(), SourceView::Map(_)) {
            return Err(Boundary::Domain.into());
        }
        let origin = owner
            .file(ModuleId(0))
            .map_err(|_| Boundary::Source)?
            .span(0, 0);
        let work = WorkMeter::new(limits.work.min(IndexLimits::default().work));
        (plan, work, origin)
    };
    work.debit(
        plan.source_work,
        origin,
        "checked HIR source/OPA comparison",
    )
    .map_err(|_| VerifyRejected::Source(Rejected::Budget))?;
    let bound_result = BoundObservation::bind(owner, captured_source, observation);
    let bound = bound_result?;
    let syntax_result = ast_compare::compare(&bound);
    let syntax = syntax_result?;
    let canonical_result = {
        let mut canonical_allocator = Allocator::default();
        hir::resolve_sources_with_meter(owner, &work, &mut canonical_allocator)
    };
    let canonical = canonical_result
        .map_err(Rejected::Canonical)
        .map_err(VerifyRejected::Source)?;
    let connection_work = if request == candidate::Request::Emit {
        candidate::EMIT_CONNECTION_WORK
    } else {
        0
    };
    let paid_source = plan
        .source_work
        .checked_add(connection_work)
        .ok_or(Boundary::Overflow)?;
    let canonical_work = work
        .used()
        .checked_sub(paid_source)
        .ok_or(Boundary::Overflow)?;
    let remaining = IndexLimits {
        retained: limits.retained.min(IndexLimits::default().retained),
        scratch: limits.scratch.min(IndexLimits::default().scratch),
        work: work
            .limit()
            .checked_sub(work.used())
            .ok_or(Boundary::Overflow)?,
    };
    // Canonical ownership is moved into the one candidate construction body.
    // Its admitted metadata prepays candidate/helper + genuine compiler passes
    // on this meter before the first reserve. Emit's fixed connection glue is
    // already paid; its scan/formula/native phases separately use this meter.
    let outcome_result = candidate::request_candidate(
        request,
        &syntax,
        canonical,
        candidate_allocator,
        plan.outside_fixed_bytes,
        remaining,
        &work,
        origin,
    );
    let outcome = outcome_result?;
    match outcome {
        candidate::Outcome::Fixed(verified) => Ok(Requested::Fixed(VerifyFacts {
            verified,
            source_work: plan.source_work,
            canonical_work,
            total_work: work.used(),
            outside_fixed_bytes: plan.outside_fixed_bytes,
        })),
        candidate::Outcome::Emitted(artifact) => Ok(Requested::Emitted(EmitOutput {
            artifact,
            source_work: plan.source_work,
            canonical_work,
            total_work: work.used(),
            outside_fixed_bytes: plan.outside_fixed_bytes,
        })),
    }
}

/// Fixed planning only: no source inspection, compiler owner, native call or
/// callback. The caller retains this same meter even when admission fails.
#[allow(clippy::result_large_err)]
fn metered_emit_plan(
    limits: IndexLimits,
    work: &WorkMeter,
    origin: Span,
) -> Result<SourcePlan, VerifyRejected> {
    metered_emit_plan_for(limits, work, origin, Protocol::V1)
}
#[allow(clippy::result_large_err)]
fn metered_emit_plan_for(
    limits: IndexLimits,
    work: &WorkMeter,
    origin: Span,
    protocol: Protocol,
) -> Result<SourcePlan, VerifyRejected> {
    emit_work::debit(
        work,
        candidate::EMIT_CONNECTION_WORK,
        origin,
        "private Emit fixed connection and banks",
    )
    .map_err(candidate::VerifyRejected::from)?;
    let remaining = IndexLimits {
        work: work
            .limit()
            .checked_sub(work.used())
            .ok_or(Boundary::Overflow)?,
        ..limits
    };
    SourcePlan::calculate_emit_for(remaining, protocol).map_err(VerifyRejected::Source)
}

/// Not called by any production/default path. Paid controls exercise only
/// contained comparison and fixed denial facts, with no typed authority.
#[allow(clippy::result_large_err)] // Preserve fixed paid transports without an error allocation.
pub(super) fn denied(
    owner: SourceOwner<'_>,
    captured_source: &[u8],
    observation: &[u8],
    candidate_allocator: &mut Allocator,
    limits: IndexLimits,
) -> Result<Infallible, Rejected> {
    #[cfg(test)]
    LAST_CANDIDATE_OBSERVATION.with(|slot| slot.set(None));
    let protocol = Protocol::from_opa(observation).unwrap_or(Protocol::V1);
    let plan = SourcePlan::calculate_for(limits, protocol)?;
    if owner.count() != 1 {
        return Err(Rejected::Boundary(Boundary::Domain));
    }
    let origin = owner
        .file(ModuleId(0))
        .map_err(|_| Boundary::Source)?
        .span(0, 0);
    let work = WorkMeter::new(limits.work.min(IndexLimits::default().work));
    work.debit(
        plan.source_work,
        origin,
        "checked HIR source/OPA comparison",
    )
    .map_err(|_| Rejected::Budget)?;
    let bound_result = BoundObservation::bind(owner, captured_source, observation);
    let bound = bound_result?;
    let syntax_result = ast_compare::compare(&bound);
    let syntax = syntax_result?;
    // Full source correspondence precedes canonical resolver assumptions.
    // Its own existing checks run with this same work meter. The inherited
    // resolver's allocations are not reclassified as newly fallible imports.
    let canonical_result = {
        let mut canonical_allocator = Allocator::default();
        hir::resolve_sources_with_meter(owner, &work, &mut canonical_allocator)
    };
    let canonical = canonical_result.map_err(Rejected::Canonical)?;
    let canonical_work = work
        .used()
        .checked_sub(plan.source_work)
        .ok_or(Boundary::Overflow)?;
    let remaining = IndexLimits {
        retained: limits.retained.min(IndexLimits::default().retained),
        scratch: limits.scratch.min(IndexLimits::default().scratch),
        work: work
            .limit()
            .checked_sub(work.used())
            .ok_or(Boundary::Overflow)?,
    };
    // Session admits observed Hc + exact Hn + this complete fixed envelope
    // before the first candidate reserve; its work includes the builder.
    #[cfg(not(test))]
    let candidate_result = candidate::compare_candidate(
        &syntax,
        &canonical,
        candidate_allocator,
        plan.outside_fixed_bytes,
        remaining,
    );
    #[cfg(test)]
    let candidate_result = {
        let mut result = None;
        let observed = crate::frontend::oir::owned::hir_import_measure_allocations(|| {
            result = Some(candidate::compare_candidate(
                &syntax,
                &canonical,
                candidate_allocator,
                plan.outside_fixed_bytes,
                remaining,
            ));
        });
        // The existing observer guards have reset before publishing fixed
        // statistics or interpreting any failure. The candidate has dropped.
        LAST_CANDIDATE_OBSERVATION.with(|slot| slot.set(Some(observed)));
        result.expect("candidate observation invokes its action exactly once")
    };
    let candidate = candidate_result?;
    // Admission already checked this against the same remaining allowance.
    // No intervening callback can use or replace the private meter.
    work.debit(
        candidate.charged_work,
        origin,
        "checked HIR candidate comparison",
    )
    .map_err(|_| Rejected::Budget)?;
    let facts = Facts {
        candidate,
        source_work: plan.source_work,
        canonical_work,
        total_work: work.used(),
        outside_fixed_bytes: plan.outside_fixed_bytes,
    };
    drop(canonical);
    if facts.candidate.equal {
        Err(Rejected::Compared(facts))
    } else {
        Err(Rejected::Mismatch(facts))
    }
}

/// Additional named leaf roles, conservatively added across phases. Candidate
/// owns its Program header; this term owns the canonical Program header. The
/// helper and comparator each price their complete banks/transport separately.
/// Already-owned source/AST/capture payload and test-observer trace backing are
/// explicit baseline/instrumentation, not hidden candidate allocations. This
/// does not claim a cap on inherited resolver internals, diagnostics or RSS.
pub(super) fn outer_named_bytes() -> Result<usize, Boundary> {
    let copies = |n: usize, count: usize| n.checked_mul(count).ok_or(Boundary::Overflow);
    let roles = [
        // Shared source/domain helper input, returned and held borrowed slice.
        3 * size_of::<(SourceOwner<'_>, &[u8], Protocol)>(),
        3 * size_of::<Result<&[u8], Boundary>>(),
        size_of::<[Protocol; 16]>(), // Complete version-selection, call and return roles.
        size_of::<[Option<Protocol>; 4]>(),
        size_of::<[(IndexLimits, Protocol); 6]>(),
        size_of::<[(IndexLimits, &WorkMeter, Span, Protocol); 3]>(),
        size_of::<(SourceOwner<'_>, &[u8], &[u8], &mut Allocator, IndexLimits)>(),
        size_of::<SourcePlan>(),
        copies(size_of::<Result<SourcePlan, Rejected>>(), 3)?,
        size_of::<SourcePlan>(),
        size_of::<WorkMeter>(),
        size_of::<Allocator>(), // canonical allocator within resolution scope
        size_of::<Allocator>(), // selected candidate allocator outside the borrow
        size_of::<Span>(),
        size_of::<BoundObservation<'_, '_>>(),
        size_of::<Result<BoundObservation<'_, '_>, Boundary>>(),
        size_of::<hir::Program>(),
        size_of::<Result<hir::Program, Vec<Diagnostic>>>(),
        size_of::<Result<hir::Program, Rejected>>(),
        size_of::<(SourceOwner<'_>, &WorkMeter, &mut Allocator)>(),
        size_of::<Result<&SourceFile, Box<Diagnostic>>>(),
        size_of::<Result<&super::ast::Program, Box<Diagnostic>>>(),
        copies(size_of::<IndexLimits>(), 3)?,
        size_of::<candidate::ComparisonFacts>(),
        size_of::<Result<candidate::ComparisonFacts, allocation::Failure>>(),
        size_of::<Result<candidate::ComparisonFacts, Rejected>>(),
        size_of::<Facts>(),
        copies(size_of::<Rejected>(), 2)?,
        copies(size_of::<Result<Infallible, Rejected>>(), 3)?,
        // Work meter input/return, checked cost calculation and local counters.
        size_of::<(&WorkMeter, u64, Span, &'static str)>(),
        size_of::<Result<(), Box<Diagnostic>>>(),
        copies(size_of::<u64>(), 12)?,
        copies(size_of::<usize>(), 8)?,
        copies(size_of::<Result<u64, Boundary>>(), 3)?,
        copies(size_of::<Result<usize, Boundary>>(), 3)?,
        copies(size_of::<[u64; 7]>(), 2)?,
        size_of::<std::array::IntoIter<u64, 7>>(),
    ];
    let bank = size_of_val(&roles)
        .checked_mul(2)
        .and_then(|n| n.checked_add(size_of_val(&roles.into_iter())))
        .ok_or(Boundary::Overflow)?;
    roles
        .into_iter()
        .try_fold(bank, |sum, n| sum.checked_add(n).ok_or(Boundary::Overflow))
}

/// Added Verify request/result/call roles; the common source/resolve owners and
/// source-comparison bank remain fully included by calculate(). Sum complete
/// transport types rather than field subtotals or supposed optimized frames.
fn verify_outer_named_bytes() -> Result<usize, Boundary> {
    let copies = |bytes: usize, count: usize| bytes.checked_mul(count).ok_or(Boundary::Overflow);
    let roles = [
        // Both fixed-facts wrappers and their complete larger common dispatch
        // call/return roles are paid. Emit-only wrapper roles are separate.
        copies(
            size_of::<(SourceOwner<'_>, &[u8], &[u8], &mut Allocator, IndexLimits)>(),
            2,
        )?,
        copies(
            size_of::<(
                candidate::Request,
                SourceOwner<'_>,
                &[u8],
                &[u8],
                &mut Allocator,
                IndexLimits,
            )>(),
            2,
        )?,
        copies(size_of::<candidate::Request>(), 3)?,
        size_of::<SourceView<'_>>(),
        copies(size_of::<SourcePlan>(), 2)?,
        // Complete common branch tuple and destructuring transport. This moves
        // the original meter; it does not replace or reset its used counter.
        copies(size_of::<(SourcePlan, WorkMeter, Span)>(), 2)?,
        copies(size_of::<Result<SourcePlan, Rejected>>(), 2)?,
        size_of::<Result<SourcePlan, VerifyRejected>>(),
        size_of::<Result<hir::Program, VerifyRejected>>(),
        size_of::<candidate::VerifyFacts>(),
        size_of::<candidate::Outcome>(),
        size_of::<Result<candidate::Outcome, candidate::VerifyRejected>>(),
        size_of::<Result<candidate::Outcome, VerifyRejected>>(),
        size_of::<VerifyFacts>(),
        // Common result construction plus the two wrapper pattern bindings.
        copies(size_of::<Requested>(), 3)?,
        copies(size_of::<Result<Requested, VerifyRejected>>(), 5)?,
        copies(size_of::<VerifyRejected>(), 2)?,
        copies(size_of::<Result<VerifyFacts, VerifyRejected>>(), 5)?,
        // Each old wrapper's impossible Emit conversion is fixed; no boxed
        // artifact or new error allocation hides the larger transport.
        copies(size_of::<allocation::Failure>(), 2)?,
        copies(size_of::<candidate::VerifyRejected>(), 2)?,
        copies(size_of::<VerifyRejected>(), 2)?,
        copies(size_of::<u64>(), 4)?,
        copies(size_of::<u64>(), 2)?,
        size_of::<Option<u64>>(),
        size_of::<Result<u64, Boundary>>(),
        copies(size_of::<usize>(), 4)?,
        copies(size_of::<Result<usize, Boundary>>(), 3)?,
    ];
    let bank = size_of_val(&roles)
        .checked_mul(2)
        .and_then(|n| n.checked_add(size_of_val(&roles.into_iter())))
        .ok_or(Boundary::Overflow)?;
    roles.into_iter().try_fold(bank, |sum, value| {
        sum.checked_add(value).ok_or(Boundary::Overflow)
    })
}

/// Only the private Emit plan adds these actual additional outer roles. Shared
/// Requested/Outcome growth is already fully paid by the ordinary private bank.
fn emit_outer_named_bytes() -> Result<usize, Boundary> {
    let roles = [
        // Emit wrapper input, common dispatch call, wrapper success/error and
        // caller return. The String payload is not duplicated by these moves.
        size_of::<(SourceOwner<'_>, &[u8], &[u8], &mut Allocator, IndexLimits)>(),
        size_of::<(
            candidate::Request,
            SourceOwner<'_>,
            &[u8],
            &[u8],
            &mut Allocator,
            IndexLimits,
        )>(),
        size_of::<Result<Requested, VerifyRejected>>(),
        size_of::<Requested>(),
        size_of::<candidate::EmitArtifact>(),
        size_of::<EmitOutput>(),
        size_of::<EmitOutput>(),
        size_of::<Result<EmitOutput, VerifyRejected>>(),
        size_of::<Result<EmitOutput, VerifyRejected>>(),
        size_of::<Result<EmitOutput, VerifyRejected>>(),
        size_of::<Result<Requested, VerifyRejected>>(),
        size_of::<allocation::Failure>(),
        size_of::<candidate::VerifyRejected>(),
        size_of::<VerifyRejected>(),
        // Complete added source-plan input/local/result/forwarding carriers.
        size_of::<[IndexLimits; 2]>(),
        // Emit bootstrap's additional local plan/meter/origin and complete
        // metered-plan call/return/conversion roles, prior to new bank use.
        size_of::<SourcePlan>(),
        size_of::<WorkMeter>(),
        size_of::<Span>(),
        size_of::<[(IndexLimits, &WorkMeter, Span); 2]>(),
        size_of::<(IndexLimits, &WorkMeter, Span)>(),
        size_of::<Result<SourcePlan, VerifyRejected>>(),
        size_of::<Result<SourcePlan, VerifyRejected>>(),
        size_of::<(&WorkMeter, u64, Span, &'static str)>(),
        size_of::<Result<(), emit_work::Failure>>(),
        size_of::<Result<(), candidate::VerifyRejected>>(),
        size_of::<Result<(), VerifyRejected>>(),
        size_of::<IndexLimits>(),
        size_of::<[(&WorkMeter,); 2]>(),
        size_of::<[u64; 2]>(),
        size_of::<Option<u64>>(),
        size_of::<Result<u64, Boundary>>(),
        size_of::<[SourcePlan; 2]>(),
        size_of::<[Result<SourcePlan, Rejected>; 2]>(),
        size_of::<Result<SourcePlan, VerifyRejected>>(),
        size_of::<[usize; 2]>(),
        size_of::<[Result<usize, Boundary>; 2]>(),
        size_of::<Result<usize, allocation::Failure>>(),
        size_of::<[Option<usize>; 3]>(),
        size_of::<[(usize, usize); 3]>(),
        size_of::<u64>(),
        size_of::<Result<u64, std::num::TryFromIntError>>(),
        size_of::<std::num::TryFromIntError>(),
        size_of::<Result<u64, Boundary>>(),
        size_of::<[u64; 2]>(),
        // Actual accounting loop/checked sum roles and forwarded results.
        size_of::<[usize; 3]>(),
        size_of::<[Option<usize>; 3]>(),
        size_of::<[Result<usize, Boundary>; 2]>(),
        size_of::<Option<usize>>(),
    ];
    let bank = size_of_val(&roles)
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(size_of_val(&roles.into_iter())))
        .and_then(|bytes| bytes.checked_add(size_of_val(&roles.into_iter())))
        .ok_or(Boundary::Overflow)?;
    roles.into_iter().try_fold(bank, |total, bytes| {
        total.checked_add(bytes).ok_or(Boundary::Overflow)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_hir_import_v2_source_preflight_exact_and_minus_one() {
        let plan = SourcePlan::calculate_verify_for(IndexLimits::default(), Protocol::V2).unwrap();
        assert_eq!(plan.source_work, 190_160);
        let exact = IndexLimits {
            retained: plan.fixed_bytes as u64,
            scratch: plan.fixed_bytes as u64,
            work: plan.source_work,
        };
        assert!(SourcePlan::calculate_verify_for(exact, Protocol::V2).is_ok());
        for limit in [
            IndexLimits {
                work: exact.work - 1,
                ..exact
            },
            IndexLimits {
                scratch: exact.scratch - 1,
                ..exact
            },
            IndexLimits {
                retained: exact.retained - 1,
                ..exact
            },
        ] {
            assert!(matches!(
                SourcePlan::calculate_verify_for(limit, Protocol::V2),
                Err(Rejected::Budget)
            ));
        }
    }

    #[test]
    fn checked_hir_import_run_carriers_reject_predecessor_private_admissions() {
        // Measured private predecessor envelopes. The current complete
        // enclosing carriers are charged without increasing any ceiling.
        assert!(matches!(
            SourcePlan::calculate(IndexLimits {
                scratch: 74_924,
                ..IndexLimits::default()
            }),
            Err(Rejected::Budget)
        ));
        assert!(matches!(
            SourcePlan::calculate_verify(IndexLimits {
                scratch: 90_177,
                ..IndexLimits::default()
            }),
            Err(Rejected::Budget)
        ));
        let text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checked_hir_import/rich-source.txt"
        ));
        let wire = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checked_hir_import/rich-success.bin"
        ));
        let mut sources = crate::frontend::source::SourceMap::new();
        let id = sources.add("predecessor-admission.ox".into(), text.into());
        let source = sources.get(id);
        let ast =
            crate::frontend::parser::parse(source, crate::frontend::lexer::lex(source).unwrap())
                .unwrap();
        let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        let mut allocator = Allocator::default();
        let checker = crate::frontend::typeck::measurement::begin();
        let result = verify(
            owner,
            text.as_bytes(),
            wire,
            &mut allocator,
            IndexLimits {
                retained: 95_890,
                scratch: IndexLimits::default().scratch,
                work: 1_276_867,
            },
        );
        assert!(matches!(
            result,
            Err(VerifyRejected::Source(Rejected::Budget))
                | Err(VerifyRejected::Terminal(
                    candidate::VerifyRejected::Candidate(allocation::Failure::Admission)
                ))
        ));
        assert_eq!(allocator.attempts, 0);
        assert_eq!(checker.finish().frame_bytes, 0);
    }

    #[test]
    fn checked_hir_import_leaf_layout_and_preflight_only() {
        let limits = IndexLimits::default();
        let plan = SourcePlan::calculate(limits).unwrap();
        println!(
            "HIR_IMPORT_LEAF outer={} outside={} fixed={} work={} facts={} rejection={} result={}",
            outer_named_bytes().unwrap(),
            plan.outside_fixed_bytes,
            plan.fixed_bytes,
            plan.source_work,
            size_of::<Facts>(),
            size_of::<Rejected>(),
            size_of::<Result<Infallible, Rejected>>()
        );
        assert_eq!(plan.source_work, 124_501);
        let exact = IndexLimits {
            retained: plan.fixed_bytes as u64,
            scratch: plan.fixed_bytes as u64,
            work: plan.source_work,
        };
        assert!(SourcePlan::calculate(exact).is_ok());
        for limits in [
            IndexLimits {
                retained: exact.retained - 1,
                ..exact
            },
            IndexLimits {
                scratch: exact.scratch - 1,
                ..exact
            },
            IndexLimits {
                work: exact.work - 1,
                ..exact
            },
        ] {
            assert!(matches!(
                SourcePlan::calculate(limits),
                Err(Rejected::Budget)
            ));
        }
        assert_eq!(
            SourcePlan::calculate(IndexLimits {
                retained: u64::MAX,
                scratch: u64::MAX,
                work: u64::MAX
            })
            .unwrap()
            .fixed_bytes,
            plan.fixed_bytes
        );
    }

    #[test]
    fn checked_hir_import_verify_leaf_layout_and_preflight_only() {
        let plan = SourcePlan::calculate_verify(IndexLimits::default()).unwrap();
        let comparison = SourcePlan::calculate(IndexLimits::default()).unwrap();
        assert_eq!(plan.source_work, comparison.source_work);
        assert_eq!(
            plan.fixed_bytes - comparison.fixed_bytes,
            verify_outer_named_bytes().unwrap() + candidate::verify_named_bytes().unwrap(),
        );
        let exact = IndexLimits {
            retained: plan.fixed_bytes as u64,
            scratch: plan.fixed_bytes as u64,
            work: plan.source_work,
        };
        assert!(SourcePlan::calculate_verify(exact).is_ok());
        for limits in [
            IndexLimits {
                retained: exact.retained - 1,
                ..exact
            },
            IndexLimits {
                scratch: exact.scratch - 1,
                ..exact
            },
            IndexLimits {
                work: exact.work - 1,
                ..exact
            },
        ] {
            assert!(matches!(
                SourcePlan::calculate_verify(limits),
                Err(Rejected::Budget)
            ));
        }
        println!(
            "HIR_IMPORT_VERIFY_LEAF extra={} candidate={} outside={} fixed={} facts={} rejection={} result={} requested={} requested_result={} emit_output={} emit_result={}",
            verify_outer_named_bytes().unwrap(), candidate::verify_named_bytes().unwrap(),
            plan.outside_fixed_bytes, plan.fixed_bytes, size_of::<VerifyFacts>(),
            size_of::<VerifyRejected>(), size_of::<Result<VerifyFacts, VerifyRejected>>(),
            size_of::<Requested>(), size_of::<Result<Requested, VerifyRejected>>(),
            size_of::<EmitOutput>(), size_of::<Result<EmitOutput, VerifyRejected>>(),
        );
    }

    #[test]
    fn checked_hir_import_emit_shared_carriers_require_successor_admission() {
        // The enabled fixed-facts wrappers retain their historical layouts;
        // their genuinely larger common dispatch carriers do not get free bytes.
        assert_eq!(size_of::<candidate::VerifyFacts>(), 288);
        assert_eq!(size_of::<VerifyFacts>(), 320);
        assert!(size_of::<Requested>() > size_of::<VerifyFacts>());
        assert!(matches!(
            SourcePlan::calculate_verify(IndexLimits {
                scratch: 94_955,
                ..IndexLimits::default()
            }),
            Err(Rejected::Budget)
        ));
    }

    #[test]
    fn checked_hir_import_emit_leaf_and_dispatch_reject_before_consumers() {
        use crate::frontend::{lexer, oir::owned, parser, source::SourceMap, typeck};

        let mut sources = SourceMap::new();
        let id = sources.add("denied-emit.ox".into(), String::new());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
        for limits in [
            IndexLimits::default(),
            IndexLimits {
                retained: 0,
                scratch: 0,
                work: 0,
            },
        ] {
            for direct in [false, true] {
                let mut allocator = Allocator {
                    fail_at: Some(1),
                    ..Allocator::default()
                };
                let checker = typeck::measurement::begin();
                let mut denied = false;
                let observed = owned::hir_import_measure_allocations(|| {
                    let result = if direct {
                        requested(
                            candidate::Request::Emit,
                            owner,
                            b"wrong",
                            b"bad",
                            &mut allocator,
                            limits,
                        )
                    } else {
                        emit(owner, b"wrong", b"bad", &mut allocator, limits)
                            .map(Requested::Emitted)
                    };
                    denied = match result {
                        Err(VerifyRejected::Source(Rejected::Boundary(Boundary::Source))) => {
                            limits.work != 0
                        }
                        Err(VerifyRejected::Terminal(candidate::VerifyRejected::EmitWork(
                            emit_work::Failure::Work,
                        ))) => limits.work == 0,
                        _ => false,
                    };
                });
                assert!(denied);
                assert_eq!(observed, (0, 0, 0, 0));
                assert_eq!(allocator.attempts, 0);
                assert_eq!(checker.finish().frame_bytes, 0);
            }
        }
    }

    #[test]
    fn checked_hir_import_emit_glue_precedes_even_zero_byte_bank_admission() {
        use crate::frontend::{oir::owned, source::SourceMap};

        let mut sources = SourceMap::new();
        let id = sources.add("emit-preflight.ox".into(), String::new());
        let origin = sources.get(id).span(0, 0);
        for available in [0, 32_767, 32_768] {
            let limits = IndexLimits {
                retained: 0,
                scratch: 0,
                work: available,
            };
            let work = WorkMeter::new(available);
            let mut result = None;
            let observed = owned::hir_import_measure_allocations(|| {
                result = Some(metered_emit_plan(limits, &work, origin));
            });
            if available < candidate::EMIT_CONNECTION_WORK {
                assert!(matches!(
                    result.unwrap(),
                    Err(VerifyRejected::Terminal(
                        candidate::VerifyRejected::EmitWork(emit_work::Failure::Work)
                    ))
                ));
                assert_eq!(work.used(), 0);
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(VerifyRejected::Source(Rejected::Budget))
                ));
                assert_eq!(work.used(), candidate::EMIT_CONNECTION_WORK);
            }
            assert_eq!(observed, (0, 0, 0, 0));
        }
    }

    #[test]
    fn checked_hir_import_emit_plan_layout_and_exact_boundaries_only() {
        use crate::frontend::source::SourceMap;

        let mut sources = SourceMap::new();
        let id = sources.add("emit-plan.ox".into(), String::new());
        let origin = sources.get(id).span(0, 0);
        let work = WorkMeter::default();
        let plan = metered_emit_plan(IndexLimits::default(), &work, origin).unwrap();
        let fixed = SourcePlan::calculate_verify(IndexLimits::default()).unwrap();
        assert_eq!(work.used(), candidate::EMIT_CONNECTION_WORK);
        assert_eq!(
            plan.fixed_bytes - fixed.fixed_bytes,
            emit_outer_named_bytes().unwrap() + candidate::emit_named_bytes().unwrap()
        );
        assert_eq!(
            plan.fixed_bytes,
            plan.outside_fixed_bytes
                + candidate::builder_named_bytes().unwrap()
                + allocation::helper_named_bytes().unwrap()
        );
        let exact = IndexLimits {
            retained: plan.fixed_bytes as u64,
            scratch: plan.fixed_bytes as u64,
            work: candidate::EMIT_CONNECTION_WORK + plan.source_work,
        };
        let work = WorkMeter::new(exact.work);
        assert!(metered_emit_plan(exact, &work, origin).is_ok());
        assert_eq!(work.used(), candidate::EMIT_CONNECTION_WORK);
        for limits in [
            IndexLimits {
                retained: exact.retained - 1,
                ..exact
            },
            IndexLimits {
                scratch: exact.scratch - 1,
                ..exact
            },
            IndexLimits {
                work: exact.work - 1,
                ..exact
            },
            // Qualified first transport envelope, not the larger successor.
            IndexLimits {
                scratch: 102_157,
                ..exact
            },
        ] {
            let work = WorkMeter::new(limits.work);
            assert!(matches!(
                metered_emit_plan(limits, &work, origin),
                Err(VerifyRejected::Source(Rejected::Budget))
            ));
            assert_eq!(work.used(), candidate::EMIT_CONNECTION_WORK);
        }
        println!("HIR_IMPORT_EMIT_PLAN private outer={} terminal={} outside={} fixed={} common_fixed={} glue={} source_work={} artifact={} outcome={} requested={} result={}",
            emit_outer_named_bytes().unwrap(), candidate::emit_named_bytes().unwrap(),
            plan.outside_fixed_bytes, plan.fixed_bytes, fixed.fixed_bytes,
            candidate::EMIT_CONNECTION_WORK, plan.source_work, size_of::<candidate::EmitArtifact>(),
            size_of::<candidate::Outcome>(), size_of::<Requested>(), size_of::<Result<EmitOutput, VerifyRejected>>());
    }
}
