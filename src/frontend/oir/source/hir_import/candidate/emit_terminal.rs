//! Private LLVM-text Emit connection, reachable only from its genuine parent
//! terminal. The caller has prepaid CONNECTION_WORK and the complete fixed bank.
//! Inherited compiler/native heap payloads are outside that named-carrier bank.
use super::super::{ComparedSyntax, Failure};
use super::{Context, EmitArtifact, Facts, Outcome, Rejected, WorkPlan};
use crate::frontend::{
    declaration_index::{IndexLimits, SourceOwner, WorkMeter},
    diagnostic::Diagnostic,
    hir,
    hir_protocol::Protocol,
    oir::{
        native::{emit_cost, emit_work, private_emit},
        VerifiedProgram,
    },
    project::{budget::Allocator, ModuleId},
    source::{SourceFile, SourceMap, SourceView, Span},
};
use std::mem::{size_of, size_of_val};

/// Finite connection work, paid in the leaf before any new bank calculation:
/// inventory construction, array moves and checked summation receive 32 units
/// per concrete row. RFC0030 successor: outer64, terminal97, scan66, formula42,
/// native30 plus three repeated native30 banks (run, fixed and final
/// preflights):389 rows,12448 units. The new native row is const sizeof of one complete role bank,
/// never another inventory loop. The predecessor fixed prologues/handlers cost
/// 19456; one extra fixed-preflight handler receives128. Total32032 <=32768.
/// This includes the new before-plan fixed check as well as final allocation.
/// Re-audit these concrete bodies if changed. Path/OIR/format/body loops,
/// allocator internals and physical-memory claims do not belong to this tariff.
const CONNECTION_WORK: u64 = super::super::EMIT_CONNECTION_WORK;
// Independently reviewed finite formula tariff: at most 4,048 units. This is
// paid immediately before calculate, never borrowed from scan setup or body.
const FORMULA_WORK: u64 = 4_096;

/// No reusable verified witness escapes. Only the parent genuine terminal can
/// call this helper, after Session's allocator borrow and canonical HIR drop.
#[allow(clippy::too_many_arguments, clippy::result_large_err)]
pub(super) fn run(
    verified: VerifiedProgram,
    syntax: &ComparedSyntax<'_, '_, '_>,
    entry: Option<hir::DefId>,
    facts: Facts,
    plan: WorkPlan,
    context: Context<'_>,
    allocator: &mut Allocator,
    limits: IndexLimits,
) -> Result<EmitArtifact, Rejected> {
    if !super::super::EMIT_ADMITTED {
        return Err(Rejected::Disabled);
    }
    let upper = emit_work::Upper {
        functions: u64::try_from(facts.candidate.allocation.requested.0[1])
            .map_err(|_| Failure::Overflow)?,
        blocks: plan.blocks,
        slots: plan.slots,
        definitions: plan.definitions,
    };
    let scan_before = context.work.used();
    let dimensions = emit_work::scan(&verified, upper, context.work, context.origin)?;
    let scan_work = context
        .work
        .used()
        .checked_sub(scan_before)
        .ok_or(emit_work::Failure::Invariant)?;
    let owner = syntax.bound.owner;
    let SourceView::Map(sources) = owner.view() else {
        return Err(Failure::Shape.into());
    };
    let source_result = owner.file(ModuleId(0));
    let source = source_result.map_err(|_| Failure::Shape)?;
    // Exact immutable owned display-path UTF-8 length, not capacity, source
    // length, a caller length, normalized text, or a new formatted allocation.
    let path_utf8_len = source.path().len();
    let cost = prepay_body_protocol(
        dimensions,
        path_utf8_len,
        syntax.bound.wire.protocol(),
        context.work,
        context.origin,
    )?;
    // The source plan included this exact native bank inside the complete F
    // carried by the completed Session receipt. Native preflight adds it once.
    let native_bytes = private_emit::named_bytes()?;
    let outside_native = facts
        .candidate
        .allocation
        .fixed_bytes
        .checked_sub(native_bytes)
        .ok_or(emit_work::Failure::Invariant)?;
    let retained = usize::try_from(limits.retained).map_err(|_| private_emit::Failure::Budget)?;
    let scratch = usize::try_from(limits.scratch).map_err(|_| private_emit::Failure::Budget)?;
    let admission = private_emit::Admission::new(retained, scratch, outside_native, allocator)?;
    let text = verified.native_module_private(entry, sources, admission)?;
    let bytes = text.len();
    let capacity = text.capacity();
    // Native finish already proved count/request/length/capacity equality.
    // SourceOwner is only a Copy borrowed adapter; the returned text has no
    // source lifetime and can survive the caller's genuine backing scope.
    drop(verified);
    Ok(EmitArtifact {
        text,
        verified: facts,
        connection_work: CONNECTION_WORK,
        scan_work,
        formula_work: FORMULA_WORK,
        body_work: cost.body,
        bytes,
        capacity,
    })
}

/// Fixed scalar data only. This private helper creates no owner/witness and
/// cannot enter native emission; it keeps each debit before its paid phase.
#[cfg(test)]
fn prepay_body(
    dimensions: emit_work::Dimensions,
    path_utf8_len: usize,
    work: &WorkMeter,
    origin: Span,
) -> Result<emit_cost::Cost, emit_work::Failure> {
    prepay_body_protocol(dimensions, path_utf8_len, Protocol::V1, work, origin)
}

fn prepay_body_protocol(
    dimensions: emit_work::Dimensions,
    path_utf8_len: usize,
    protocol: Protocol,
    work: &WorkMeter,
    origin: Span,
) -> Result<emit_cost::Cost, emit_work::Failure> {
    emit_work::debit(work, FORMULA_WORK, origin, "private Emit formula setup")?;
    let cost = emit_cost::calculate_protocol(dimensions, path_utf8_len, protocol)?;
    emit_work::debit(work, cost.body, origin, "private Emit native body")?;
    Ok(cost)
}

type Inputs<'a> = (
    VerifiedProgram,
    &'a ComparedSyntax<'a, 'a, 'a>,
    Option<hir::DefId>,
    Facts,
    WorkPlan,
    Context<'a>,
    &'a mut Allocator,
    IndexLimits,
);
type DebitInputs<'a> = (&'a WorkMeter, u64, Span, &'static str);
type CostInputs<'a> = (emit_work::Dimensions, usize, Protocol, &'a WorkMeter, Span);

/// Full real call/input/local/result roles, conservatively summed across moves
/// and branches. These inventory types are not runtime allocations or alternate
/// owners. The one eventual String payload is added once at native preflight.
pub(super) fn named_bytes() -> Result<usize, Failure> {
    let roles = [
        // Parent call transport, callee input, and named moved input bindings.
        size_of::<Inputs<'_>>(),
        size_of::<Inputs<'_>>(),
        size_of::<VerifiedProgram>(),
        size_of::<&ComparedSyntax<'_, '_, '_>>(),
        size_of::<Option<hir::DefId>>(),
        size_of::<Facts>(),
        size_of::<WorkPlan>(),
        size_of::<Context<'_>>(),
        size_of::<&mut Allocator>(),
        size_of::<IndexLimits>(),
        // Complete Upper/scan inputs, result conversion and held scalar data.
        size_of::<emit_work::Upper>(),
        size_of::<usize>(),
        size_of::<Result<u64, std::num::TryFromIntError>>(),
        size_of::<std::num::TryFromIntError>(),
        size_of::<Result<u64, Failure>>(),
        size_of::<Result<u64, Rejected>>(),
        size_of::<(&VerifiedProgram, emit_work::Upper, &WorkMeter, Span)>(),
        size_of::<Result<emit_work::Dimensions, emit_work::Failure>>(),
        size_of::<Result<emit_work::Dimensions, Rejected>>(),
        size_of::<emit_work::Dimensions>(),
        // Before/after meter reads, checked delta and its complete conversion.
        size_of::<[(&WorkMeter,); 2]>(),
        size_of::<[u64; 3]>(),
        size_of::<(u64, u64)>(),
        size_of::<Option<u64>>(),
        size_of::<Result<u64, emit_work::Failure>>(),
        size_of::<Result<u64, Rejected>>(),
        // Genuine source adapter/view/file lookup; path()/len() do not scan.
        size_of::<SourceOwner<'_>>(),
        size_of::<(SourceOwner<'_>,)>(),
        size_of::<SourceView<'_>>(),
        size_of::<&SourceMap>(),
        size_of::<(SourceOwner<'_>, ModuleId)>(),
        size_of::<Result<&SourceFile, Box<Diagnostic>>>(),
        size_of::<Result<&SourceFile, Failure>>(),
        size_of::<Result<&SourceFile, Rejected>>(),
        size_of::<&SourceFile>(),
        size_of::<(&SourceFile,)>(),
        size_of::<&str>(),
        size_of::<(&str,)>(),
        size_of::<usize>(),
        // Closed, source-bound protocol extraction and result/local transport.
        size_of::<(super::super::super::Wire<'_>,)>(),
        size_of::<Protocol>(),
        size_of::<Protocol>(),
        // Distinct formula setup and native-body debit calls. Reused helper
        // internals are also completely paid by emit_work's own bank below.
        size_of::<[DebitInputs<'_>; 2]>(),
        size_of::<[Result<(), emit_work::Failure>; 2]>(),
        size_of::<[Result<(), Rejected>; 2]>(),
        size_of::<CostInputs<'_>>(),
        size_of::<CostInputs<'_>>(),
        size_of::<CostInputs<'_>>(),
        size_of::<(emit_work::Dimensions, usize, Protocol)>(),
        size_of::<Result<emit_cost::Cost, emit_work::Failure>>(),
        size_of::<Result<emit_cost::Cost, emit_work::Failure>>(),
        size_of::<Result<emit_cost::Cost, emit_work::Failure>>(),
        size_of::<Result<emit_cost::Cost, Rejected>>(),
        size_of::<emit_cost::Cost>(),
        size_of::<emit_cost::Cost>(),
        // Complete retained/scratch/native-bank preflight values and results.
        size_of::<Result<usize, private_emit::Failure>>(),
        size_of::<Result<usize, Rejected>>(),
        size_of::<[usize; 4]>(),
        size_of::<(usize, usize)>(),
        size_of::<Option<usize>>(),
        size_of::<Result<usize, emit_work::Failure>>(),
        size_of::<Result<usize, Rejected>>(),
        size_of::<[u64; 2]>(),
        size_of::<[Result<usize, std::num::TryFromIntError>; 2]>(),
        size_of::<[std::num::TryFromIntError; 2]>(),
        size_of::<[Result<usize, private_emit::Failure>; 2]>(),
        size_of::<[Result<usize, Rejected>; 2]>(),
        size_of::<(usize, usize, usize, &mut Allocator)>(),
        size_of::<Result<private_emit::Admission<'_>, private_emit::Failure>>(),
        size_of::<Result<private_emit::Admission<'_>, Rejected>>(),
        size_of::<private_emit::Admission<'_>>(),
        // Genuine native call and single owned payload moving through results.
        size_of::<(
            &VerifiedProgram,
            Option<hir::DefId>,
            &SourceMap,
            private_emit::Admission<'_>,
        )>(),
        size_of::<Result<String, private_emit::Failure>>(),
        size_of::<Result<String, Rejected>>(),
        size_of::<String>(),
        size_of::<[(&String,); 2]>(),
        size_of::<[usize; 2]>(),
        size_of::<(VerifiedProgram,)>(),
        size_of::<EmitArtifact>(),
        size_of::<EmitArtifact>(),
        size_of::<Result<EmitArtifact, Rejected>>(),
        size_of::<Result<EmitArtifact, Rejected>>(),
        size_of::<Outcome>(),
        size_of::<Result<Outcome, Rejected>>(),
        // Fixed failure expressions and complete From transports. Authentic
        // native diagnostics keep their existing Box and content unchanged.
        size_of::<[Failure; 3]>(),
        size_of::<[emit_work::Failure; 2]>(),
        size_of::<[private_emit::Failure; 2]>(),
        size_of::<(emit_work::Failure,)>(),
        size_of::<(private_emit::Failure,)>(),
        size_of::<[Rejected; 3]>(),
        // This inventory and its forwarded candidate/terminal return roles.
        size_of::<[usize; 6]>(),
        size_of::<[Result<usize, Failure>; 6]>(),
        size_of::<Result<usize, emit_work::Failure>>(),
        size_of::<Result<usize, emit_work::Failure>>(),
        size_of::<Result<usize, private_emit::Failure>>(),
        size_of::<[Option<usize>; 4]>(),
        size_of::<Option<usize>>(),
    ];
    let bank = size_of_val(&roles)
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(size_of_val(&roles.into_iter())))
        .and_then(|bytes| bytes.checked_add(size_of_val(&roles.into_iter())))
        .ok_or(Failure::Overflow)?;
    let local = roles.into_iter().try_fold(bank, |total, bytes| {
        total.checked_add(bytes).ok_or(Failure::Overflow)
    })?;
    let scan = emit_work::named_bytes().map_err(|_| Failure::Overflow)?;
    let formula = emit_cost::named_bytes().map_err(|_| Failure::Overflow)?;
    let native = private_emit::named_bytes().map_err(|_| Failure::Overflow)?;
    local
        .checked_add(scan)
        .and_then(|bytes| bytes.checked_add(formula))
        .and_then(|bytes| bytes.checked_add(native))
        .ok_or(Failure::Overflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{lexer, oir::lower_and_verify, parser, source::SourceFileId, typeck};

    const RICH: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-source.txt"
    ));
    const RICH_UPPER: emit_work::Upper = emit_work::Upper {
        functions: 2,
        blocks: 45,
        slots: 20,
        definitions: 18,
    };
    // Independently qualified existing rich Run prefix, plus the explicit new
    // connection prepayment. These isolated controls never enter private Emit.
    const RICH_PRIOR: u64 = 1_310_659 + CONNECTION_WORK;
    const RICH_SCAN: u64 = 8_064;

    #[test]
    fn u8_native_inventory_connection_contains_all_four_bank_evaluations() {
        let rows = 64 + 97 + 66 + 42 + 4 * 30;
        let finite = rows * 32 + 19_456 + 128;
        assert_eq!((rows, finite), (389, 32_032));
        assert!(finite <= CONNECTION_WORK);
    }

    #[test]
    fn denied_emit_connection_glue_is_not_charged_again_by_the_fixed_pass_plan() {
        let counts = super::super::Counts([2, 2, 1, 2, 11, 5, 7, 2]);
        let run = WorkPlan::calculate_request(counts, 29, super::super::Request::Run).unwrap();
        let emit = WorkPlan::calculate_request(counts, 29, super::super::Request::Emit).unwrap();
        assert_eq!(
            (emit.pass_work, emit.typed_work, emit.entry_work),
            (run.pass_work, run.typed_work, run.entry_work)
        );
        assert_eq!(emit.total, run.total);
        assert_eq!((CONNECTION_WORK, FORMULA_WORK), (32_768, 4_096));
        assert_eq!(CONNECTION_WORK + FORMULA_WORK, 36_864);
    }

    fn ordinary(path: &str) -> (VerifiedProgram, SourceMap) {
        let mut sources = SourceMap::new();
        let id = sources.add(path.into(), RICH.into());
        let source = sources.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let typed = typeck::check(hir::resolve(source, &ast).unwrap()).unwrap();
        (lower_and_verify(&typed, &sources).unwrap(), sources)
    }

    #[test]
    fn denied_emit_formula_and_body_debits_preserve_the_original_prefix() {
        let (verified, sources) = ordinary("main.ox");
        let source = sources.get(SourceFileId(0));
        let origin = source.span(0, 0);
        // Independent work-report value for p=7: 8,721,664 less scan 8,064.
        let body = 8_713_600;
        let exact = RICH_PRIOR + RICH_SCAN + FORMULA_WORK + body;
        for (limit, path, expected_used, expected) in [
            (
                RICH_PRIOR + RICH_SCAN + FORMULA_WORK - 1,
                usize::MAX,
                RICH_PRIOR + RICH_SCAN,
                Err(emit_work::Failure::Work),
            ),
            (
                exact - 1,
                source.path().len(),
                RICH_PRIOR + RICH_SCAN + FORMULA_WORK,
                Err(emit_work::Failure::Work),
            ),
            (exact, source.path().len(), exact, Ok(body)),
        ] {
            let work = WorkMeter::new(limit);
            emit_work::debit(&work, RICH_PRIOR, origin, "already paid source/check/glue").unwrap();
            let dimensions = emit_work::scan(&verified, RICH_UPPER, &work, origin).unwrap();
            assert_eq!(work.used(), RICH_PRIOR + RICH_SCAN);
            assert_eq!(
                prepay_body(dimensions, path, &work, origin).map(|cost| cost.body),
                expected
            );
            assert_eq!(work.used(), expected_used);
        }
        assert_eq!(exact, 10_069_187);
    }

    #[test]
    fn v2_emit_formula_and_body_debits_preserve_the_original_meter() {
        let (verified, sources) = ordinary("main.ox");
        let source = sources.get(SourceFileId(0));
        let origin = source.span(0, 0);
        // v1 rich body 8,713,600 plus 44 human renders * 4,096.
        let body = 8_893_824;
        let prior = RICH_PRIOR + 65_659 + 32_512;
        let prefix = prior + RICH_SCAN;
        let exact = prefix + FORMULA_WORK + body;
        for (limit, expected_used, expected) in [
            (
                prefix + FORMULA_WORK - 1,
                prefix,
                Err(emit_work::Failure::Work),
            ),
            (
                exact - 1,
                prefix + FORMULA_WORK,
                Err(emit_work::Failure::Work),
            ),
            (exact, exact, Ok(body)),
        ] {
            let work = WorkMeter::new(limit);
            emit_work::debit(&work, prior, origin, "already paid v2 source/check/glue").unwrap();
            let dimensions = emit_work::scan(&verified, RICH_UPPER, &work, origin).unwrap();
            assert_eq!(work.used(), prefix);
            assert_eq!(
                prepay_body_protocol(dimensions, source.path().len(), Protocol::V2, &work, origin)
                    .map(|cost| cost.body),
                expected,
            );
            assert_eq!(work.used(), expected_used);
        }
    }

    #[test]
    fn denied_emit_rich_work_ceiling_includes_both_new_setup_fees() {
        for (path_len, expected_total, fits) in
            [(3_365, 255_928_515, true), (3_366, 256_001_731, false)]
        {
            let (verified, sources) = ordinary(&"p".repeat(path_len));
            let source = sources.get(SourceFileId(0));
            let origin = source.span(0, 0);
            let work = WorkMeter::default();
            emit_work::debit(&work, RICH_PRIOR, origin, "already paid source/check/glue").unwrap();
            let dimensions = emit_work::scan(&verified, RICH_UPPER, &work, origin).unwrap();
            let result = prepay_body(dimensions, source.path().len(), &work, origin);
            if fits {
                assert_eq!(
                    RICH_PRIOR + RICH_SCAN + FORMULA_WORK + result.unwrap().body,
                    expected_total
                );
                assert_eq!(work.used(), expected_total);
            } else {
                assert_eq!(result, Err(emit_work::Failure::Work));
                assert_eq!(work.used(), RICH_PRIOR + RICH_SCAN + FORMULA_WORK);
                assert!(expected_total > work.limit());
            }
        }
    }
}
