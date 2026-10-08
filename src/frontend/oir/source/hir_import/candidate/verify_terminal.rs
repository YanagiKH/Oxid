//! Private Verify terminal. Only candidate's single construction body calls it.
//! The public/default source path remains disconnected. Fixed facts or owned
//! LLVM text escape after candidate, typed and verified owners are dropped.
use super::super::{Counts, MAX_ROWS};
use super::{ast, hir, typed_compare, ComparedSyntax, ComparisonFacts, Failure, Request};
use crate::frontend::{
    declaration_index::{IndexLimits, WorkMeter},
    diagnostic::Diagnostic,
    hir_protocol::Protocol,
    oir::{
        self, lower,
        native::{emit_work, private_emit},
        source::association,
        verify, RunFailure, Scalar,
    },
    project::{budget::Allocator, ModuleId},
    source::{SourceFile, SourceMap, SourceView, Span},
    typeck,
};
use std::mem::{size_of, size_of_val};

// Only this genuine terminal can transfer verified ownership to the private
// Emit helper. Its entry and the separate native entry both remain hard denied.
#[path = "emit_terminal.rs"]
mod emit_terminal;

pub(super) struct Context<'m> {
    pub(super) request: Request,
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
    pub(super) entry_work: u64,
    pub(super) total: u64,
}

#[derive(Debug)]
pub(in crate::frontend::oir::source::hir_import) struct Facts {
    pub(in crate::frontend::oir::source::hir_import) candidate: ComparisonFacts,
    pub(in crate::frontend::oir::source::hir_import) typed_cells: usize,
    pub(in crate::frontend::oir::source::hir_import) functions: usize,
    pub(in crate::frontend::oir::source::hir_import) association_declarations: usize,
    pub(in crate::frontend::oir::source::hir_import) association_spans: usize,
    pub(in crate::frontend::oir::source::hir_import) pass_work: u64,
    pub(in crate::frontend::oir::source::hir_import) typed_work: u64,
    pub(in crate::frontend::oir::source::hir_import) entry_work: u64,
    pub(in crate::frontend::oir::source::hir_import) runtime: Option<Result<Scalar, RunFailure>>,
}

/// One future owned text payload, with no source/witness lifetime. This type
/// adds transport storage, not permission to construct an emitted artifact.
#[derive(Debug)]
pub(in crate::frontend::oir::source::hir_import) struct EmitArtifact {
    pub(in crate::frontend::oir::source::hir_import) text: String,
    pub(in crate::frontend::oir::source::hir_import) verified: Facts,
    pub(in crate::frontend::oir::source::hir_import) connection_work: u64,
    pub(in crate::frontend::oir::source::hir_import) scan_work: u64,
    pub(in crate::frontend::oir::source::hir_import) formula_work: u64,
    pub(in crate::frontend::oir::source::hir_import) body_work: u64,
    pub(in crate::frontend::oir::source::hir_import) bytes: usize,
    pub(in crate::frontend::oir::source::hir_import) capacity: usize,
}

#[derive(Debug)]
pub(in crate::frontend::oir::source::hir_import) enum Outcome {
    Fixed(Facts),
    Emitted(EmitArtifact),
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
    EmitWork(emit_work::Failure),
    Native(private_emit::Failure),
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
impl From<emit_work::Failure> for Rejected {
    fn from(value: emit_work::Failure) -> Self {
        Self::EmitWork(value)
    }
}
impl From<private_emit::Failure> for Rejected {
    fn from(value: private_emit::Failure) -> Self {
        Self::Native(value)
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
            entry_work: 0,
            total: add(pass_work, typed_work)?,
        })
    }

    #[cfg(test)]
    pub(super) fn calculate_request(
        counts: Counts,
        rows: usize,
        request: Request,
    ) -> Result<Self, Failure> {
        Self::calculate_request_protocol(counts, rows, request, Protocol::V1)
    }

    pub(super) fn calculate_request_protocol(
        counts: Counts,
        rows: usize,
        request: Request,
        protocol: Protocol,
    ) -> Result<Self, Failure> {
        let mut plan = Self::calculate(counts, rows)?;
        if matches!(request, Request::Run | Request::Emit) {
            // Complete source/OPA admission bounds functions by MAX_ROWS and
            // source bytes by the wire's closed protocol. Each function's
            // identity/name check plus all name-byte comparisons fit
            // functions + source_max visits; two
            // visits cover entry/exit. 256 covers fixed checked field access.
            // This is shared compile work, never runtime fuel.
            let functions = u64::try_from(counts.0[1]).map_err(|_| Failure::Overflow)?;
            let source_bytes =
                u64::try_from(protocol.source_max()).map_err(|_| Failure::Overflow)?;
            plan.entry_work = mul(256, add(add(functions, source_bytes)?, 2)?)?;
            plan.total = add(plan.total, plan.entry_work)?;
        }
        // Emit's fixed connection setup is already paid by its original leaf
        // meter before its source-plan banks are computed. Never debit it again.
        Ok(plan)
    }
}

/// Source/AST/function identity projection only, after complete source/OPA and
/// candidate/canonical equality. The caller never supplies an entry ordinal.
/// The returned DefId is captured from the corresponding candidate function
/// before genuine typechecking consumes that candidate.
#[cfg(test)]
pub(super) fn root_entry(
    root: &ast::Program,
    source: &SourceFile,
    candidate: &hir::Program,
) -> Result<Option<hir::DefId>, Failure> {
    root_entry_protocol(root, source, candidate, Protocol::V1)
}

fn root_entry_protocol(
    root: &ast::Program,
    source: &SourceFile,
    candidate: &hir::Program,
    protocol: Protocol,
) -> Result<Option<hir::DefId>, Failure> {
    super::require(root.modules.is_empty() && root.imports.is_empty())?;
    super::require(
        source.text().len() <= protocol.source_max() && root.functions.len() <= MAX_ROWS,
    )?;
    super::require(
        root.functions.len() == candidate.functions.len()
            && root.functions.len() == candidate.signatures.len(),
    )?;
    let mut entry = None;
    let mut ordinal = 0usize;
    while ordinal < root.functions.len() {
        let function = &root.functions[ordinal];
        let resolved = &candidate.functions[ordinal];
        let signature = &candidate.signatures[ordinal];
        super::require(resolved.id == hir::DefId(ordinal) && signature.span == function.name)?;
        let name = source.try_text(function.name).ok_or(Failure::Shape)?;
        if name == "main" {
            super::require(entry.is_none())?;
            entry = Some(resolved.id);
        }
        ordinal += 1;
    }
    Ok(entry)
}

/// The construction body has completed exact HIR equality, completed/dropped
/// Session, dropped canonical HIR, and prepaid the genuine passes below on the
/// original shared WorkMeter. Emit separately pays its scan/formula/native work.
/// No callback or compiler-owner return channel is accepted.
#[allow(clippy::too_many_arguments, clippy::result_large_err)]
pub(super) fn run(
    context: Context<'_>,
    syntax: &ComparedSyntax<'_, '_, '_>,
    candidate: hir::Program,
    comparison: ComparisonFacts,
    plan: WorkPlan,
    allocator: &mut Allocator,
    limits: IndexLimits,
) -> Result<Outcome, Rejected> {
    let request = context.request;
    if !super::VERIFY_ADMITTED
        || (request == Request::Run && !super::RUN_ADMITTED)
        || (request == Request::Emit && !super::EMIT_ADMITTED)
    {
        return Err(Rejected::Disabled);
    }
    super::require(comparison.equal)?;
    let owner = syntax.bound.owner;
    let SourceView::Map(sources) = owner.view() else {
        return Err(Failure::Shape.into());
    };
    let root = owner.ast(ModuleId(0)).map_err(|_| Failure::Shape)?;
    super::require(owner.count() == 1 && root.modules.is_empty() && root.imports.is_empty())?;
    let entry = if matches!(request, Request::Run | Request::Emit) {
        let source_result = owner.file(ModuleId(0));
        let source = source_result.map_err(|_| Failure::Shape)?;
        root_entry_protocol(root, source, &candidate, syntax.bound.wire.protocol())?
    } else {
        None
    };
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
    let runtime = match request {
        Request::Verify | Request::Emit => None,
        Request::Run => Some(verified.run(entry)),
    };
    let facts = Facts {
        candidate: comparison,
        typed_cells: compared.cells,
        functions,
        association_declarations: associated.validation.declarations,
        association_spans: associated.validation.spans,
        pass_work: plan.pass_work,
        typed_work: plan.typed_work,
        entry_work: plan.entry_work,
        runtime,
    };
    if request == Request::Emit {
        emit_terminal::run(
            verified, syntax, entry, facts, plan, context, allocator, limits,
        )
        .map(Outcome::Emitted)
    } else {
        drop(verified);
        Ok(Outcome::Fixed(facts))
    }
}

/// Emit-only consumers and their full input/return/owner carriers. The source
/// plan adds this before Session admission; Verify/Run do not pay this bank.
pub(super) fn emit_named_bytes() -> Result<usize, Failure> {
    emit_terminal::named_bytes()
}

/// New importer-owned fixed carriers only. Inherited checker/lower/verifier
/// payload and internal scratch retain their ordinary behavior and limits;
/// they require separate phase observations before success can be enabled.
pub(super) fn named_bytes() -> Result<usize, Failure> {
    let copies = |bytes: usize, count: usize| bytes.checked_mul(count).ok_or(Failure::Overflow);
    let roles = [
        size_of::<Context<'_>>(),
        size_of::<(&Context<'_>, Counts, usize, u64)>(),
        copies(size_of::<Request>(), 4)?,
        copies(size_of::<(Counts, usize, Request, Protocol)>(), 2)?,
        // Closed wire protocol extraction at planning and entry: receiver,
        // returned/local enum, and source_max receiver/result transport.
        copies(size_of::<(super::super::Wire<'_>,)>(), 2)?,
        copies(size_of::<Protocol>(), 4)?,
        copies(size_of::<(Protocol, usize)>(), 2)?,
        copies(size_of::<WorkPlan>(), 5)?,
        copies(size_of::<Result<WorkPlan, Failure>>(), 5)?,
        copies(size_of::<Option<WorkPlan>>(), 2)?,
        size_of::<(Counts, usize)>(),
        size_of::<[usize; 8]>(),
        size_of::<std::array::IntoIter<usize, 8>>(),
        copies(size_of::<usize>(), 10)?,
        copies(size_of::<u64>(), 32)?,
        copies(size_of::<Result<u64, Failure>>(), 12)?,
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
        copies(
            size_of::<(
                Context<'_>,
                &ComparedSyntax<'_, '_, '_>,
                hir::Program,
                ComparisonFacts,
                WorkPlan,
                &mut Allocator,
                IndexLimits,
            )>(),
            2,
        )?,
        size_of::<crate::frontend::declaration_index::SourceOwner<'_>>(),
        size_of::<SourceView<'_>>(),
        size_of::<&SourceMap>(),
        size_of::<&crate::frontend::ast::Program>(),
        size_of::<Result<&crate::frontend::ast::Program, Box<Diagnostic>>>(),
        // Run entry capture: complete helper arguments, fixed scan locals,
        // source-file lookup/conversion, and helper call/return transports.
        copies(
            size_of::<(&ast::Program, &SourceFile, &hir::Program, Protocol)>(),
            2,
        )?,
        size_of::<(
            crate::frontend::declaration_index::SourceOwner<'_>,
            ModuleId,
        )>(),
        size_of::<(&SourceFile, Span)>(),
        copies(size_of::<&SourceFile>(), 2)?,
        size_of::<Result<&SourceFile, Box<Diagnostic>>>(),
        size_of::<Result<&SourceFile, Failure>>(),
        size_of::<Result<&SourceFile, Rejected>>(),
        size_of::<&ast::Function>(),
        size_of::<&hir::Function>(),
        size_of::<&hir::Signature>(),
        size_of::<&str>(),
        size_of::<Option<&str>>(),
        size_of::<Result<&str, Failure>>(),
        copies(size_of::<usize>(), 2)?,
        copies(size_of::<hir::DefId>(), 2)?,
        copies(size_of::<Option<hir::DefId>>(), 4)?,
        copies(size_of::<Result<Option<hir::DefId>, Failure>>(), 3)?,
        size_of::<Result<Option<hir::DefId>, Rejected>>(),
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
        // Existing reference runtime, retaining its ordinary limits. This
        // pays only new importer transport, not engine-owned frame payloads.
        size_of::<(&oir::VerifiedProgram, Option<hir::DefId>)>(),
        copies(size_of::<Result<Scalar, RunFailure>>(), 2)?,
        copies(size_of::<Option<Result<Scalar, RunFailure>>>(), 2)?,
        size_of::<usize>(),
        size_of::<Facts>(),
        copies(size_of::<Outcome>(), 3)?,
        copies(size_of::<Rejected>(), 2)?,
        copies(size_of::<Result<Outcome, Rejected>>(), 3)?,
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
    fn checked_hir_import_v2_request_work_preserves_v1_and_row_caps() {
        let counts = Counts([2, 2, 1, 2, 11, 5, 7, 2]);
        for request in [Request::Verify, Request::Run, Request::Emit] {
            let old = WorkPlan::calculate_request(counts, 29, request).unwrap();
            let v1 =
                WorkPlan::calculate_request_protocol(counts, 29, request, Protocol::V1).unwrap();
            let v2 =
                WorkPlan::calculate_request_protocol(counts, 29, request, Protocol::V2).unwrap();
            assert_eq!((old.total, old.entry_work), (v1.total, v1.entry_work));
            assert_eq!((v2.pass_work, v2.typed_work), (v1.pass_work, v1.typed_work));
            if matches!(request, Request::Verify) {
                assert_eq!(v2.entry_work, 0);
                assert_eq!(v2.total, v1.total);
            } else {
                assert_eq!((v1.entry_work, v2.entry_work), (33_792, 66_304));
                assert_eq!(v2.total - v1.total, 32_512);
            }
        }
        for protocol in [Protocol::V1, Protocol::V2] {
            assert!(WorkPlan::calculate_request_protocol(
                Counts([MAX_ROWS + 1; 8]),
                MAX_ROWS,
                Request::Run,
                protocol,
            )
            .is_err());
            assert!(WorkPlan::calculate_request_protocol(
                Counts::default(),
                MAX_ROWS + 1,
                Request::Run,
                protocol,
            )
            .is_err());
        }
    }

    #[test]
    fn checked_hir_import_v2_entry_projection_retains_versioned_source_cap() {
        use crate::frontend::{lexer, parser};
        let body = "fn main()->i32{return 0;}";
        for length in [128, 129, 254, 255, 256] {
            let mut sources = SourceMap::new();
            let text = format!("{}{body}", " ".repeat(length - body.len()));
            let id = sources.add("capacity.ox".into(), text);
            let source = sources.get(id);
            let root = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
            let candidate = hir::resolve(source, &root).unwrap();
            assert_eq!(root_entry(&root, source, &candidate).is_ok(), length <= 128);
            for protocol in [Protocol::V1, Protocol::V2] {
                let result = root_entry_protocol(&root, source, &candidate, protocol);
                if length <= protocol.source_max() {
                    assert_eq!(result, Ok(Some(hir::DefId(0))));
                } else {
                    assert_eq!(result, Err(Failure::Shape));
                }
            }
        }
    }

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
            "HIR_IMPORT_VERIFY_TERMINAL named={} context={} plan={} facts={} rejected={} result={} typed={} raw={} verified={} request={} entry={} entry_result={} scalar={} run_failure={} runtime={} optional_runtime={} source_lookup_call={} source_text_call={} outcome={} emit_artifact={}",
            named_bytes().unwrap(), size_of::<Context<'_>>(), size_of::<WorkPlan>(),
            size_of::<Facts>(), size_of::<Rejected>(), size_of::<Result<Outcome, Rejected>>(),
            size_of::<typeck::TypedProgram>(), size_of::<oir::Program>(), size_of::<oir::VerifiedProgram>(),
            size_of::<Request>(), size_of::<Option<hir::DefId>>(), size_of::<Result<Option<hir::DefId>, Failure>>(),
            size_of::<Scalar>(), size_of::<RunFailure>(), size_of::<Result<Scalar, RunFailure>>(), size_of::<Option<Result<Scalar, RunFailure>>>(),
            size_of::<(crate::frontend::declaration_index::SourceOwner<'_>, ModuleId)>(), size_of::<(&SourceFile, Span)>(),
            size_of::<Outcome>(), size_of::<EmitArtifact>(),
        );
    }
}
