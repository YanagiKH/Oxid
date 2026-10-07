//! Private comparison and fixed-facts Verify entries. No default caller is
//! connected. Successful terminal values contain fixed facts only.
use super::{allocation, ast_compare, candidate, source_work_bound, BoundObservation, Boundary};
use crate::frontend::{
    declaration_index::{IndexLimits, SourceOwner, WorkMeter},
    diagnostic::Diagnostic,
    hir,
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
        let outside_fixed_bytes = outer_named_bytes()?
            .checked_add(ast_compare::named_bytes()?)
            .ok_or(Boundary::Overflow)?;
        let builder_named = candidate::builder_named_bytes()?;
        let helper_named = allocation::helper_named_bytes()?;
        let fixed_bytes = outside_fixed_bytes
            .checked_add(builder_named)
            .and_then(|n| n.checked_add(helper_named))
            .ok_or(Boundary::Overflow)?;
        let source_work = source_work_bound()?;
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
        let mut plan = Self::calculate(limits)?;
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
}

/// Private fixed-facts Verify entry. Admission is compile-time only; no caller
/// flag, default source route, or cfg(test) bypass exists.
#[allow(clippy::result_large_err)]
pub(super) fn verify(
    owner: SourceOwner<'_>,
    captured_source: &[u8],
    observation: &[u8],
    candidate_allocator: &mut Allocator,
    limits: IndexLimits,
) -> Result<VerifyFacts, VerifyRejected> {
    if !candidate::VERIFY_ADMITTED {
        return Err(VerifyRejected::Disabled);
    }
    let plan = SourcePlan::calculate_verify(limits).map_err(VerifyRejected::Source)?;
    if owner.count() != 1 || !matches!(owner.view(), SourceView::Map(_)) {
        return Err(Boundary::Domain.into());
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
    // Canonical ownership is moved into the one candidate construction body.
    // Its admitted metadata prepays candidate/helper + all downstream work on
    // this same meter before the first reserve. No second debit follows here.
    let verified_result = candidate::verify_candidate(
        &syntax,
        canonical,
        candidate_allocator,
        plan.outside_fixed_bytes,
        remaining,
        &work,
        origin,
    );
    let verified = verified_result?;
    Ok(VerifyFacts {
        verified,
        source_work: plan.source_work,
        canonical_work,
        total_work: work.used(),
        outside_fixed_bytes: plan.outside_fixed_bytes,
    })
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
    let plan = SourcePlan::calculate(limits)?;
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
fn outer_named_bytes() -> Result<usize, Boundary> {
    let copies = |n: usize, count: usize| n.checked_mul(count).ok_or(Boundary::Overflow);
    let roles = [
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
        size_of::<(SourceOwner<'_>, &[u8], &[u8], &mut Allocator, IndexLimits)>(),
        size_of::<SourceView<'_>>(),
        copies(size_of::<SourcePlan>(), 2)?,
        copies(size_of::<Result<SourcePlan, Rejected>>(), 2)?,
        size_of::<Result<SourcePlan, VerifyRejected>>(),
        size_of::<Result<hir::Program, VerifyRejected>>(),
        size_of::<candidate::VerifyFacts>(),
        size_of::<Result<candidate::VerifyFacts, candidate::VerifyRejected>>(),
        size_of::<Result<candidate::VerifyFacts, VerifyRejected>>(),
        size_of::<VerifyFacts>(),
        copies(size_of::<VerifyRejected>(), 2)?,
        copies(size_of::<Result<VerifyFacts, VerifyRejected>>(), 3)?,
        copies(size_of::<u64>(), 4)?,
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

#[cfg(test)]
mod tests {
    use super::*;
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
            "HIR_IMPORT_VERIFY_LEAF extra={} candidate={} outside={} fixed={} facts={} rejection={} result={}",
            verify_outer_named_bytes().unwrap(), candidate::verify_named_bytes().unwrap(),
            plan.outside_fixed_bytes, plan.fixed_bytes, size_of::<VerifyFacts>(),
            size_of::<VerifyRejected>(), size_of::<Result<VerifyFacts, VerifyRejected>>(),
        );
    }
}
