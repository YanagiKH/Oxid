//! Error-only live-producer validation. No supplied facts construct HIR or a
//! typed/executable witness; only the genuine source checker's errors escape.
use super::{ast_compare, leaf, source_work_bound_for, BoundObservation, Boundary, Protocol};
use crate::frontend::{
    declaration_index::{IndexLimits, SourceOwner, WorkMeter},
    diagnostic::Diagnostic,
    oir::{project::CheckedProjectTypes, source::check_project_candidate},
    owned_diagnostic,
    project::{budget::Allocator, ModuleId, ProjectSources},
    source::{SourceFile, Span},
};
use std::{convert::Infallible, fmt, mem::size_of};

// The existing Verify terminal charges 128 logical visits for each weighted
// scalar checker unit. Each admitted family is bounded by 128 OPA rows, so
// 128 * ((4+3+4+4+4+4+2)*128 + 8) dominates its typecheck term. Its diagnostic
// allowance is 1024*(functions+1). Unknown-type syntax takes the genuine owned
// resolution schedule and necessarily fails before owned typing in this domain.
const CHECK_WORK: u64 = 128 * (25 * 128 + 8) + 1024 * (128 + 1);
// Fixed header/span/label checks, bounded source-name/template streaming and
// comparison with at most 1024 message bytes. No temporary String is allocated.
const MATCH_WORK: u64 = 4_096;

#[derive(Debug)]
pub(super) enum Rejected {
    Boundary(Boundary),
    Budget,
    Mismatch,
    Confirmed(Vec<Diagnostic>),
}
impl From<Boundary> for Rejected {
    fn from(error: Boundary) -> Self {
        Self::Boundary(error)
    }
}
impl Rejected {
    pub(super) fn message(&self) -> &'static str {
        match self {
            Self::Budget => "experimental HIR producer diagnostic resource limit exceeded",
            Self::Boundary(_) => "experimental HIR producer diagnostic does not match the authoritative source and exact diagnostic frame",
            Self::Mismatch | Self::Confirmed(_) => "experimental HIR producer first diagnostic does not match the authoritative source checker",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Plan {
    fixed_bytes: usize,
    prepaid_work: u64,
    canonical_limits: IndexLimits,
}
impl Plan {
    fn calculate(limits: IndexLimits, protocol: Protocol) -> Result<Self, Rejected> {
        let fixed_bytes = leaf::outer_named_bytes()?
            .checked_add(ast_compare::named_bytes()?)
            .and_then(|n| n.checked_add(named_bytes()))
            .ok_or(Boundary::Overflow)?;
        let bytes = u64::try_from(fixed_bytes).map_err(|_| Boundary::Overflow)?;
        let prepaid_work = source_work_bound_for(protocol)?
            .checked_add(CHECK_WORK)
            .and_then(|n| n.checked_add(MATCH_WORK))
            .ok_or(Boundary::Overflow)?;
        let defaults = IndexLimits::default();
        let canonical_limits = IndexLimits {
            retained: limits
                .retained
                .min(defaults.retained)
                .checked_sub(bytes)
                .ok_or(Rejected::Budget)?,
            scratch: limits
                .scratch
                .min(defaults.scratch)
                .checked_sub(bytes)
                .ok_or(Rejected::Budget)?,
            work: limits.work.min(defaults.work),
        };
        if prepaid_work > canonical_limits.work {
            return Err(Rejected::Budget);
        }
        Ok(Self {
            fixed_bytes,
            prepaid_work,
            canonical_limits,
        })
    }
}

/// Complete additional enclosing carriers, conservatively summed across phases.
/// The shared source/OPA envelope is added separately. Genuine declaration,
/// resolver/typechecker and diagnostic allocations keep their inherited
/// accounting; this is not a total heap/stack/RSS or allocator-capacity bound.
fn named_bytes() -> usize {
    4 * size_of::<Plan>()
        + 3 * size_of::<Result<Plan, Rejected>>()
        + 4 * size_of::<Rejected>()
        + 4 * size_of::<Result<Infallible, Rejected>>()
        + 3 * size_of::<Result<CheckedProjectTypes, Vec<Diagnostic>>>()
        + 3 * size_of::<CheckedProjectTypes>()
        + 3 * size_of::<Vec<Diagnostic>>()
        + 2 * size_of::<WorkMeter>()
        + 2 * size_of::<Allocator>()
        + 4 * size_of::<IndexLimits>()
        + 3 * size_of::<(&ProjectSources, &[u8], IndexLimits)>()
        + 3 * size_of::<(
            &ProjectSources,
            &[u8],
            IndexLimits,
            &WorkMeter,
            &mut Allocator,
        )>()
        + 3 * size_of::<(&ProjectSources, IndexLimits, &WorkMeter, &mut Allocator)>()
        + 3 * size_of::<Result<BoundObservation<'_, '_>, Boundary>>()
        + 3 * size_of::<BoundObservation<'_, '_>>()
        + 3 * size_of::<Result<&SourceFile, Box<Diagnostic>>>()
        + 3 * size_of::<(&[u8], &SourceFile, &Diagnostic)>()
        + 4 * size_of::<Match<'_>>()
        + 4 * size_of::<owned_diagnostic::Name<'_>>()
        + 4 * size_of::<fmt::Arguments<'_>>()
        + 4 * size_of::<fmt::Result>()
        + 4 * size_of::<[&str; 4]>()
        + 4 * size_of::<Option<&str>>()
        + 8 * size_of::<Span>()
        + 4 * size_of::<Option<Span>>()
        + 4 * size_of::<[u8; 16]>()
        + 4 * size_of::<[usize; 16]>()
        + 4 * size_of::<[&[u8]; 8]>()
        + 4 * size_of::<[bool; 16]>()
}

pub(super) fn validate(
    project: &ProjectSources,
    observation: &[u8],
    limits: IndexLimits,
) -> Result<Infallible, Rejected> {
    let work = WorkMeter::new(limits.work);
    execute(
        project,
        observation,
        limits,
        &work,
        &mut Allocator::default(),
    )
}

fn execute(
    project: &ProjectSources,
    observation: &[u8],
    limits: IndexLimits,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<Infallible, Rejected> {
    let protocol = Protocol::from_opa(observation).unwrap_or(Protocol::V1);
    let plan = Plan::calculate(limits, protocol)?;
    let owner = SourceOwner::project(project);
    let source = owner.file(ModuleId(0)).map_err(|_| Boundary::Source)?;
    work.debit(
        plan.prepaid_work,
        source.span(0, 0),
        "producer diagnostic source/check/projection",
    )
    .map_err(|_| Rejected::Budget)?;
    let bound = BoundObservation::bind_diagnostic(owner, source.text().as_bytes(), observation)?;
    let _syntax = ast_compare::compare(&bound)?;
    // This existing Types-depth source facade selects genuine owned/scalar
    // resolution, including global-first and full-vector diagnostic ordering.
    // It never lowers, emits or runs anything. Successful checking invalidates
    // the producer's claimed error and is deliberately not a fallback result.
    let errors = match check_project_candidate(project, plan.canonical_limits, work, allocator) {
        Ok(_) => return Err(Rejected::Mismatch),
        Err(errors) => errors,
    };
    let first = errors.first().ok_or(Rejected::Mismatch)?;
    if !matches_first(&observation[super::OPA_BYTES..], source, first) {
        return Err(Rejected::Mismatch);
    }
    Err(Rejected::Confirmed(errors))
}

// Streaming equality avoids allocating a projected producer Diagnostic/String.
// Only the actual canonical error vector can be returned by the leaf.
struct Match<'a> {
    remaining: &'a str,
}
impl fmt::Write for Match<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.remaining = self.remaining.strip_prefix(text).ok_or(fmt::Error)?;
        Ok(())
    }
}
fn message(actual: &str, expected: fmt::Arguments<'_>) -> bool {
    if actual.len() > 1024 {
        return false;
    }
    let mut target = Match { remaining: actual };
    fmt::write(&mut target, expected).is_ok() && target.remaining.is_empty()
}

fn matches_first(header: &[u8], source: &SourceFile, first: &Diagnostic) -> bool {
    if header.len() != 16 || !first.notes.is_empty() {
        return false;
    }
    let [kind, start, end, secondary_start, secondary_end, label, expected, actual, nargs, argc] =
        <[u8; 10]>::try_from(&header[6..]).expect("exact diagnostic header");
    if !(1..=14).contains(&kind)
        || start >= end
        || usize::from(end) > source.text().len()
        || first.primary != Some(source.span(usize::from(start), usize::from(end)))
    {
        return false;
    }
    let label_text = match label {
        0 => "",
        1 => "first declared here",
        2 => "function declared here",
        3 => "binding declared here",
        4 => "immutable binding declared here",
        _ => return false,
    };
    if label == 0 {
        if secondary_start != 0 || secondary_end != 0 || !first.secondary.is_empty() {
            return false;
        }
    } else if secondary_start >= secondary_end
        || usize::from(secondary_end) > source.text().len()
        || first.secondary.len() != 1
        || first.secondary[0].0
            != source.span(usize::from(secondary_start), usize::from(secondary_end))
        || first.secondary[0].1 != label_text
    {
        return false;
    }
    let valid_label = match kind {
        3 => label == 1,
        8 => matches!(label, 0 | 2 | 3),
        10 => label == 2,
        14 => label == 4,
        _ => label == 0,
    };
    let valid_payload = match kind {
        8 => {
            (1..=3).contains(&expected)
                && (1..=3).contains(&actual)
                && expected != actual
                && nargs == 0
                && argc == 0
        }
        10 => {
            expected == 0
                && actual == 0
                && nargs != argc
                && usize::from(nargs) <= super::MAX_ROWS
                && usize::from(argc) <= super::MAX_ROWS
        }
        _ => expected == 0 && actual == 0 && nargs == 0 && argc == 0,
    };
    if !valid_label || !valid_payload {
        return false;
    }
    let code = match kind {
        1 | 2 => "E0200",
        3 => "E0201",
        4 => "E0202",
        5 => "E0203",
        6 | 7 => "E0204",
        8 | 9 => "E0300",
        10 => "E0301",
        11 => "E0302",
        12 | 13 => "E0303",
        14 => "E0304",
        _ => return false,
    };
    if first.code != code || first.stage != if kind <= 7 { "resolve" } else { "type" } {
        return false;
    }
    let name = &source.text()[usize::from(start)..usize::from(end)];
    match kind {
        1 => message(&first.message, format_args!("unknown local `{name}`")),
        2 => message(
            &first.message,
            format_args!("unknown direct function `{name}`"),
        ),
        3 => first.message == "duplicate binding; shadowing is unavailable in typed-preview",
        4 => message(
            &first.message,
            format_args!("unknown type `{}`", owned_diagnostic::name(name)),
        ),
        5 => first.message == "decimal literal is outside the i32 range [-2147483648, 2147483647]",
        6 => first.message == "`break` requires an enclosing while in the same function",
        7 => first.message == "`continue` requires an enclosing while in the same function",
        8 => {
            let types = ["", "bool", "i32", "()"];
            message(
                &first.message,
                format_args!(
                    "type mismatch: expected {}, found {}",
                    types[usize::from(expected)],
                    types[usize::from(actual)]
                ),
            )
        }
        9 => first.message == "equality requires i32 or bool operands, found ()",
        10 => message(
            &first.message,
            format_args!("wrong argument count: expected {nargs}, found {argc}"),
        ),
        11 => first.message == "function requires an explicit terminal return",
        12 => first.message == "statement after terminal return is unavailable in typed-preview",
        13 => {
            first.message
                == "statement after terminal control transfer is unavailable in typed-preview"
        }
        14 => first.message == "assignment requires a mutable local",
        _ => false,
    }
}

#[cfg(test)]
mod tests;
