//! Explicit supplied-observation facade. Only authentic ProjectSources creates
//! the source owner. Outputs carry no witness or caller-selected compiler seam.
use super::{allocation, candidate, leaf, Boundary, SourceOwner, SUCCESS_BYTES};
use crate::frontend::{
    declaration_index::IndexLimits,
    diagnostic::Diagnostic,
    oir::{
        native::{emit_work, private_emit},
        Scalar,
    },
    options::{ImportOptions, Operation},
    project::{budget::Allocator, ModuleId, ProjectSources},
};
use std::mem::size_of;

pub(in crate::frontend) const IMPORT_BYTES: usize = SUCCESS_BYTES;
// Named fixed setup, dispatch and result projection work; external I/O/rendering
// retains the ordinary driver's contract and is not compiler work.
const FACADE_WORK: u64 = 4_096;

#[derive(Debug)]
pub(in crate::frontend) enum Imported {
    Checked(usize),
    Ran(Scalar),
    Emitted(String),
}
type Result = std::result::Result<Imported, Vec<Diagnostic>>;

fn refusal(message: &'static str) -> Vec<Diagnostic> {
    vec![*Diagnostic::new("E0702", "hir-import", message, None)]
}

/// A conservative sum of actual complete enclosing carriers, not a claim about
/// physical stack frames. The source/AST/argv strings retain their existing
/// loader/CLI baseline; the new box payload and fixed observation buffer do not.
/// Leaf internals already price their own banks. Three complete copies cover
/// returned/local/moved leaf carriers; all alternatives are summed deliberately.
fn facade_bytes() -> Option<usize> {
    [
        size_of::<ImportOptions>(),
        size_of::<Box<[ImportOptions; 1]>>(),
        crate::frontend::driver::import_transport_bytes(),
        3 * size_of::<std::result::Result<leaf::VerifyFacts, leaf::VerifyRejected>>(),
        3 * size_of::<std::result::Result<leaf::EmitOutput, leaf::VerifyRejected>>(),
        3 * size_of::<leaf::VerifyRejected>(),
        4 * size_of::<Result>(),
        3 * size_of::<Imported>(),
        3 * size_of::<IndexLimits>(),
        size_of::<Allocator>(),
        3 * size_of::<SourceOwner<'static>>(),
        4 * size_of::<&[u8]>(),
        4 * size_of::<&ProjectSources>(),
        4 * size_of::<&mut Allocator>(),
        size_of::<[usize; 8]>(),
        4 * size_of::<Option<usize>>(),
        4 * size_of::<Operation>(),
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
}

fn allowance(limits: IndexLimits) -> Option<IndexLimits> {
    let fixed = u64::try_from(facade_bytes()?).ok()?;
    let default = IndexLimits::default();
    Some(IndexLimits {
        retained: limits.retained.min(default.retained).checked_sub(fixed)?,
        scratch: limits.scratch.min(default.scratch).checked_sub(fixed)?,
        work: limits.work.min(default.work).checked_sub(FACADE_WORK)?,
    })
}

fn diagnostic(error: leaf::VerifyRejected, project: &ProjectSources) -> Vec<Diagnostic> {
    use candidate::VerifyRejected as Terminal;
    use leaf::{Rejected as Source, VerifyRejected as Rejected};
    match error {
        Rejected::Source(Source::Canonical(errors)) | Rejected::Terminal(Terminal::Typed(errors)) => errors,
        Rejected::Terminal(Terminal::Association(error))
        | Rejected::Terminal(Terminal::Native(private_emit::Failure::Diagnostic(error))) => vec![*error],
        Rejected::Terminal(Terminal::Oir(error)) => vec![*error.diagnostic(project.sources())],
        Rejected::Source(Source::Boundary(Boundary::Domain)) => refusal("experimental HIR import requires one root-only scalar source of at most 128 ASCII bytes"),
        Rejected::Source(Source::Boundary(Boundary::Frame)) => refusal("experimental HIR import requires an exact successful OPA1/STF1 observation"),
        Rejected::Source(Source::Budget)
        | Rejected::Source(Source::Candidate(allocation::Failure::Admission))
        | Rejected::Terminal(Terminal::Candidate(allocation::Failure::Admission))
        | Rejected::Terminal(Terminal::EmitWork(emit_work::Failure::Work))
        | Rejected::Terminal(Terminal::Native(private_emit::Failure::Budget)) => refusal("experimental HIR import resource limit exceeded"),
        Rejected::Source(Source::Candidate(allocation::Failure::Allocation))
        | Rejected::Terminal(Terminal::Candidate(allocation::Failure::Allocation))
        | Rejected::Terminal(Terminal::Native(private_emit::Failure::Allocation)) => refusal("experimental HIR import allocation failed"),
        _ => refusal("experimental HIR observation does not match the authoritative source and checked program"),
    }
}

pub(in crate::frontend) fn import_checked(
    project: &ProjectSources,
    observation: &[u8],
    operation: Operation,
) -> Result {
    execute(
        project,
        observation,
        operation,
        IndexLimits::default(),
        &mut Allocator::default(),
    )
}

/// Producer orchestration has additional complete transport carriers. Only
/// this closed entry derives their debit; no external caller supplies a byte
/// allowance, source owner, witness, fuel or producer-derived compiler fact.
pub(in crate::frontend) fn import_produced(
    project: &ProjectSources,
    observation: &[u8],
    operation: Operation,
) -> Result {
    let defaults = IndexLimits::default();
    let extra = crate::frontend::hir_producer::named_bytes()
        .checked_add(crate::frontend::driver::producer_transport_bytes())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or_else(|| refusal("experimental HIR producer carrier accounting overflow"))?;
    let limited = (|| {
        Some(IndexLimits {
            retained: defaults.retained.checked_sub(extra)?,
            scratch: defaults.scratch.checked_sub(extra)?,
            // Source-independent dispatch/projection only. Bounded hashing/I/O and
            // external process costs have their own limits, not this work tariff.
            work: defaults.work.checked_sub(8_192)?,
        })
    })()
    .ok_or_else(|| refusal("experimental HIR producer resource limit exceeded"))?;
    execute(
        project,
        observation,
        operation,
        limited,
        &mut Allocator::default(),
    )
}

fn execute(
    project: &ProjectSources,
    observation: &[u8],
    operation: Operation,
    limits: IndexLimits,
    allocator: &mut Allocator,
) -> Result {
    let limits = allowance(limits)
        .ok_or_else(|| refusal("experimental HIR import resource limit exceeded"))?;
    let owner = SourceOwner::project(project);
    let source = owner.file(ModuleId(0)).map_err(|error| vec![*error])?;
    let source = source.text().as_bytes();
    match operation {
        Operation::Check => leaf::verify(owner, source, observation, allocator, limits)
            .map(|facts| Imported::Checked(facts.verified.functions))
            .map_err(|error| diagnostic(error, project)),
        Operation::Run => {
            let facts = leaf::run(owner, source, observation, allocator, limits)
                .map_err(|error| diagnostic(error, project))?;
            match facts.verified.runtime {
                Some(Ok(value)) => Ok(Imported::Ran(value)),
                Some(Err(error)) => Err(vec![*error.diagnostic(project.sources())]),
                None => Err(refusal(
                    "experimental HIR import returned no runtime result",
                )),
            }
        }
        Operation::Compile => leaf::emit(owner, source, observation, allocator, limits)
            .map(|output| Imported::Emitted(output.artifact.text))
            .map_err(|error| diagnostic(error, project)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-source.txt"
    ));
    const WIRE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-success.bin"
    ));

    #[test]
    fn checked_hir_public_facade_debits_only_explicit_wrapper_before_leaf() {
        let fixed = facade_bytes().unwrap() as u64;
        let d = IndexLimits::default();
        let allowed = allowance(d).unwrap();
        assert_eq!(
            (
                allowed.retained + fixed,
                allowed.scratch + fixed,
                allowed.work + FACADE_WORK
            ),
            (d.retained, d.scratch, d.work)
        );
        for limits in [
            IndexLimits {
                scratch: fixed - 1,
                ..d
            },
            IndexLimits {
                retained: fixed - 1,
                ..d
            },
            IndexLimits {
                work: FACADE_WORK - 1,
                ..d
            },
        ] {
            let project = super::super::tests::project(SOURCE);
            let mut allocator = Allocator::default();
            assert_eq!(
                execute(&project, WIRE, Operation::Check, limits, &mut allocator).unwrap_err()[0]
                    .code,
                "E0702"
            );
            assert_eq!(allocator.attempts, 0);
        }
        println!("HIR_IMPORT_PUBLIC fixed={fixed} work={FACADE_WORK} default_unchanged=true");
    }

    #[test]
    fn checked_hir_public_facade_project_owner_admits_all_three_results() {
        let project = super::super::tests::project(SOURCE);
        assert!(matches!(
            import_checked(&project, WIRE, Operation::Check).unwrap(),
            Imported::Checked(2)
        ));
        assert!(matches!(
            import_checked(&project, WIRE, Operation::Run).unwrap(),
            Imported::Ran(Scalar::I32(1))
        ));
        assert!(
            matches!(import_checked(&project, WIRE, Operation::Compile).unwrap(), Imported::Emitted(text) if text.contains("define i32 @main"))
        );
        let mut wrong = WIRE.to_vec();
        wrong[2091] ^= 1;
        assert_eq!(
            import_checked(&project, &wrong, Operation::Run).unwrap_err()[0].code,
            "E0702"
        );
    }
}
