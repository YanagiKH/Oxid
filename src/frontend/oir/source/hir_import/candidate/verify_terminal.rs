//! Closed Verify terminal. Only candidate's single construction body calls it.
//! The public/default source path is disconnected and its private entry stays
//! hard-denied until actual carrier measurement and independent boundary review.
use super::super::{Counts, MAX_ROWS};
use super::{hir, typed_compare, ComparedSyntax, ComparisonFacts, Failure};
use crate::frontend::{
    declaration_index::WorkMeter,
    diagnostic::Diagnostic,
    oir::{self, lower, source::association, verify},
    project::ModuleId,
    source::{SourceMap, SourceView, Span},
    typeck,
};
use std::mem::{size_of, size_of_val};

pub(super) struct Context<'m> {
    pub(super) work: &'m WorkMeter,
    pub(super) origin: Span,
}

/// A fixed scalar plan, calculated from admitted HIR family counts. It is not
/// a typed/verified witness and grants no ownership or allocation authority.
#[derive(Clone, Copy, Debug)]
pub(super) struct WorkPlan {
    pub(super) blocks: u64,
    pub(super) slots: u64,
    pub(super) definitions: u64,
    pub(super) pass_work: u64,
    pub(super) typed_work: u64,
    pub(super) total: u64,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::frontend::oir::source::hir_import) struct Facts {
    pub(in crate::frontend::oir::source::hir_import) candidate: ComparisonFacts,
    pub(in crate::frontend::oir::source::hir_import) typed_cells: usize,
    pub(in crate::frontend::oir::source::hir_import) functions: usize,
    pub(in crate::frontend::oir::source::hir_import) association_declarations: usize,
    pub(in crate::frontend::oir::source::hir_import) association_spans: usize,
    pub(in crate::frontend::oir::source::hir_import) pass_work: u64,
    pub(in crate::frontend::oir::source::hir_import) typed_work: u64,
}

#[derive(Debug)]
pub(in crate::frontend::oir::source::hir_import) enum Rejected {
    Disabled,
    Candidate(Failure),
    HirMismatch(ComparisonFacts),
    Typed(Vec<Diagnostic>),
    TypedMismatch { cells: usize },
    Association(Box<Diagnostic>),
    Oir(oir::OirFailure),
}
impl From<Failure> for Rejected {
    fn from(value: Failure) -> Self {
        Self::Candidate(value)
    }
}
impl From<super::super::Boundary> for Rejected {
    fn from(value: super::super::Boundary) -> Self {
        Self::Candidate(value.into())
    }
}

fn add(a: u64, b: u64) -> Result<u64, Failure> {
    a.checked_add(b).ok_or(Failure::Overflow)
}
fn mul(a: u64, b: u64) -> Result<u64, Failure> {
    a.checked_mul(b).ok_or(Failure::Overflow)
}
fn weighted<const N: usize>(terms: [(u64, u64); N]) -> Result<u64, Failure> {
    terms
        .into_iter()
        .try_fold(0, |total, (weight, count)| add(total, mul(weight, count)?))
}

impl WorkPlan {
    pub(super) fn calculate(counts: Counts, rows: usize) -> Result<Self, Failure> {
        super::require(rows <= MAX_ROWS)?;
        let [signatures, functions, parameters, locals, expressions, blocks, statements, arguments] =
            counts.0;
        super::require(counts.0.into_iter().all(|count| count <= MAX_ROWS))?;
        super::require(signatures == functions && parameters <= locals)?;
        let f = u64::try_from(functions).map_err(|_| Failure::Overflow)?;
        let p = u64::try_from(parameters).map_err(|_| Failure::Overflow)?;
        let l = u64::try_from(locals).map_err(|_| Failure::Overflow)?;
        let e = u64::try_from(expressions).map_err(|_| Failure::Overflow)?;
        let h = u64::try_from(blocks).map_err(|_| Failure::Overflow)?;
        let s = u64::try_from(statements).map_err(|_| Failure::Overflow)?;
        let a = u64::try_from(arguments).map_err(|_| Failure::Overflow)?;
        // Ordinary scalar lower::preflight: at most two new blocks per
        // expression, three per statement, and one entry per function.
        let k = weighted([(1, f), (2, e), (3, s)])?;
        let n = weighted([(1, l), (1, e), (1, s)])?;
        let d = add(e, s)?;
        let typecheck = weighted([
            (4, f),
            (3, p),
            (4, l),
            (4, h),
            (4, e),
            (4, s),
            (2, a),
            (1, 8),
        ])?;
        let lowering = weighted([
            (8, f),
            (4, p),
            (4, l),
            (8, h),
            (8, e),
            (8, s),
            (4, a),
            (4, k),
            (1, 8),
        ])?;
        let association_walk = weighted([(2, f), (1, n), (3, k), (4, e), (4, d), (1, a)])?;
        let association = weighted([(2, association_walk), (1, f), (1, 8)])?;
        let verification = weighted([
            (16, f),
            (4, p),
            (4, n),
            (64, k),
            (8, d),
            (4, e),
            (4, a),
            (1, 16),
        ])?;
        let linear = weighted([
            (128, typecheck),
            (128, lowering),
            (128, association),
            (128, verification),
        ])?;
        // Every verifier ancestor link goes to a strict DFS ancestor. At most
        // 3K evals each have 2K+1 explicit ascent/path-pop events; no loop-fuel
        // assumption, logarithmic constant, or new CFG restriction is used.
        let paths = mul(32, mul(mul(3, k)?, add(mul(2, k)?, 1)?)?)?;
        let diagnostics = mul(1024, add(f, 1)?)?;
        let pass_work = add(add(linear, paths)?, diagnostics)?;
        let typed_work = typed_compare::work_bound(rows)?;
        Ok(Self {
            blocks: k,
            slots: n,
            definitions: d,
            pass_work,
            typed_work,
            total: add(pass_work, typed_work)?,
        })
    }
}

/// The construction body has completed exact HIR equality, completed/dropped
/// Session, dropped canonical HIR, and prepaid every operation below on the
/// original shared WorkMeter. No callback or owner-return channel is accepted.
#[allow(clippy::result_large_err)]
pub(super) fn run(
    syntax: &ComparedSyntax<'_, '_, '_>,
    candidate: hir::Program,
    comparison: ComparisonFacts,
    plan: WorkPlan,
) -> Result<Facts, Rejected> {
    super::require(comparison.equal)?;
    let owner = syntax.bound.owner;
    let SourceView::Map(sources) = owner.view() else {
        return Err(Failure::Shape.into());
    };
    let root = owner.ast(ModuleId(0)).map_err(|_| Failure::Shape)?;
    super::require(owner.count() == 1 && root.modules.is_empty() && root.imports.is_empty())?;
    // Genuine check is the sole constructor. Keep its entire authentic error
    // vector, including diagnostics from later functions and their order.
    let typed_result = typeck::check(candidate);
    let typed = typed_result.map_err(Rejected::Typed)?;
    let compared_result = typed_compare::compare(syntax, &typed, plan.typed_work);
    let compared = compared_result?;
    if !compared.equal {
        return Err(Rejected::TypedMismatch {
            cells: compared.cells,
        });
    }
    let raw_result = lower::lower(&typed);
    let raw = raw_result.map_err(Rejected::Oir)?;
    // TypedProgram includes the candidate HIR. Both are gone before raw OIR
    // association and verification, including every following error path.
    drop(typed);
    let associated_result =
        association::scalar(&raw, sources, association::Declarations::Original(root));
    let associated = associated_result.map_err(Rejected::Association)?;
    let verified_result = verify::verify(raw, sources);
    let verified = verified_result.map_err(Rejected::Oir)?;
    let functions = verified.function_count();
    drop(verified);
    Ok(Facts {
        candidate: comparison,
        typed_cells: compared.cells,
        functions,
        association_declarations: associated.validation.declarations,
        association_spans: associated.validation.spans,
        pass_work: plan.pass_work,
        typed_work: plan.typed_work,
    })
}

/// New importer-owned fixed carriers only. Inherited checker/lower/verifier
/// payload and internal scratch retain their ordinary behavior and limits;
/// they require separate phase observations before success can be enabled.
pub(super) fn named_bytes() -> Result<usize, Failure> {
    let copies = |bytes: usize, count: usize| bytes.checked_mul(count).ok_or(Failure::Overflow);
    let roles = [
        size_of::<Context<'_>>(),
        size_of::<(&Context<'_>, Counts, usize, u64)>(),
        copies(size_of::<WorkPlan>(), 3)?,
        copies(size_of::<Result<WorkPlan, Failure>>(), 3)?,
        copies(size_of::<Option<WorkPlan>>(), 2)?,
        size_of::<(Counts, usize)>(),
        size_of::<[usize; 8]>(),
        size_of::<std::array::IntoIter<usize, 8>>(),
        copies(size_of::<usize>(), 10)?,
        copies(size_of::<u64>(), 24)?,
        copies(size_of::<Result<u64, Failure>>(), 6)?,
        copies(size_of::<(u64, u64)>(), 5)?,
        // Every concrete weighted call's array, by-value input and iterator:
        // K/N/association (3), typecheck/verify (8), lower (9), association
        // walk (6), and combined linear work (4). Sum across sequential calls
        // too; this accounting makes no optimized-lifetime or stack claim.
        copies(size_of::<[(u64, u64); 3]>(), 6)?,
        copies(size_of::<std::array::IntoIter<(u64, u64), 3>>(), 3)?,
        copies(size_of::<[(u64, u64); 8]>(), 4)?,
        copies(size_of::<std::array::IntoIter<(u64, u64), 8>>(), 2)?,
        copies(size_of::<[(u64, u64); 9]>(), 2)?,
        size_of::<std::array::IntoIter<(u64, u64), 9>>(),
        copies(size_of::<[(u64, u64); 6]>(), 2)?,
        size_of::<std::array::IntoIter<(u64, u64), 6>>(),
        copies(size_of::<[(u64, u64); 4]>(), 2)?,
        size_of::<std::array::IntoIter<(u64, u64), 4>>(),
        size_of::<(
            &ComparedSyntax<'_, '_, '_>,
            hir::Program,
            ComparisonFacts,
            WorkPlan,
        )>(),
        size_of::<crate::frontend::declaration_index::SourceOwner<'_>>(),
        size_of::<SourceView<'_>>(),
        size_of::<&SourceMap>(),
        size_of::<&crate::frontend::ast::Program>(),
        size_of::<Result<&crate::frontend::ast::Program, Box<Diagnostic>>>(),
        // Genuine pass call argument, return, conversion and held owners.
        size_of::<hir::Program>(),
        size_of::<Result<typeck::TypedProgram, Vec<Diagnostic>>>(),
        size_of::<Result<typeck::TypedProgram, Rejected>>(),
        size_of::<typeck::TypedProgram>(),
        size_of::<&typeck::TypedProgram>(),
        size_of::<Result<oir::Program, oir::OirFailure>>(),
        size_of::<Result<oir::Program, Rejected>>(),
        size_of::<oir::Program>(),
        size_of::<(&oir::Program, &SourceMap, association::Declarations<'_, '_>)>(),
        size_of::<association::Declarations<'_, '_>>(),
        size_of::<Result<association::BindUsage, Box<Diagnostic>>>(),
        size_of::<Result<association::BindUsage, Rejected>>(),
        size_of::<association::BindUsage>(),
        size_of::<(oir::Program, &SourceMap)>(),
        size_of::<Result<oir::VerifiedProgram, oir::OirFailure>>(),
        size_of::<Result<oir::VerifiedProgram, Rejected>>(),
        size_of::<oir::VerifiedProgram>(),
        size_of::<&oir::VerifiedProgram>(),
        size_of::<usize>(),
        size_of::<Facts>(),
        copies(size_of::<Rejected>(), 2)?,
        copies(size_of::<Result<Facts, Rejected>>(), 3)?,
        size_of::<(&WorkMeter, u64, Span, &'static str)>(),
        size_of::<Result<(), Box<Diagnostic>>>(),
        copies(size_of::<Result<(), Failure>>(), 4)?,
    ];
    let bank = size_of_val(&roles)
        .checked_mul(2)
        .and_then(|n| n.checked_add(size_of_val(&roles.into_iter())))
        .ok_or(Failure::Overflow)?;
    roles
        .into_iter()
        .try_fold(bank, |sum, value| {
            sum.checked_add(value).ok_or(Failure::Overflow)
        })?
        .checked_add(typed_compare::named_bytes()?)
        .ok_or(Failure::Overflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_hir_import_verify_work_and_layout_only() {
        let rich = WorkPlan::calculate(Counts([2, 2, 1, 2, 11, 5, 7, 2]), 29).unwrap();
        assert_eq!((rich.blocks, rich.slots, rich.definitions), (45, 20, 18));
        assert_eq!(
            (rich.pass_work, rich.typed_work, rich.total),
            (946_976, 62_464, 1_009_440)
        );
        let maximum = WorkPlan::calculate(Counts([MAX_ROWS; 8]), MAX_ROWS).unwrap();
        assert_eq!(maximum.pass_work, 123_385_856);
        assert_eq!(maximum.total, 123_651_072);
        assert!(WorkPlan::calculate(Counts([MAX_ROWS + 1; 8]), MAX_ROWS).is_err());
        assert!(WorkPlan::calculate(Counts::default(), MAX_ROWS + 1).is_err());
        println!(
            "HIR_IMPORT_VERIFY_TERMINAL named={} context={} plan={} facts={} rejected={} result={} typed={} raw={} verified={}",
            named_bytes().unwrap(), size_of::<Context<'_>>(), size_of::<WorkPlan>(),
            size_of::<Facts>(), size_of::<Rejected>(), size_of::<Result<Facts, Rejected>>(),
            size_of::<typeck::TypedProgram>(), size_of::<oir::Program>(), size_of::<oir::VerifiedProgram>(),
        );
    }
}
