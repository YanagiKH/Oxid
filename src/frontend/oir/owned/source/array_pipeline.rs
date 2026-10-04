//! Test-only source-to-validation observations, with no executable object escape.
//! Tuple rows carry version, sequence, family and explicitly ordered primitive data.
use super::super::{budget as raw_budget, verified, *};
use super::{association, budget, diagnostic, hir as source_hir, lower, resolve, typeck};
use crate::frontend::{
    ast,
    declaration_index::{IndexLimits, SourceOwner, WorkMeter},
    lexer, parser,
    project::{budget::Allocator, ModuleId, ProjectLimits, ProjectSources},
    source::{SourceFile, SourceView},
};
use std::{
    cell::RefCell,
    fmt::{self, Write},
    mem::size_of,
    path::Path,
};

#[path = "array_pipeline_rows.rs"]
mod rows;
use rows::{Json, Optional, SpanRow};

const MAX_ROWS: usize = 200_000;
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_AUXILIARY: usize = 16 * 1024 * 1024;
const MAX_DIAGNOSTIC_WORK: usize = 100_000_000;

pub(super) enum SourceInput<'a> {
    Bytes { path: &'a str, text: &'a str },
    Root { path: &'a Path },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Validate,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Control {
    Complete,
    FailLiteralOperand { ordinal: usize },
    ExpandedBoundaryFirstOperandFailure,
    FailTranscriptReservation,
    FailTraceReservation,
}
impl Control {
    fn name(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::FailLiteralOperand { .. } => "literal-capacity-failure",
            Self::ExpandedBoundaryFirstOperandFailure => "expanded-boundary-first-operand-failure",
            Self::FailTranscriptReservation => "transcript-capacity-failure",
            Self::FailTraceReservation => "trace-capacity-failure",
        }
    }
    fn resource_only(self) -> bool {
        self == Self::ExpandedBoundaryFirstOperandFailure
    }
}
#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub source: ProjectLimits,
    pub index: IndexLimits,
    pub raw_bytes: usize,
    pub verification: raw_budget::Limits,
    pub units: usize,
    pub bytes: usize,
    pub auxiliary: usize,
    pub trace_rows: usize,
    pub diagnostic_work: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            source: ProjectLimits::default(),
            index: IndexLimits::default(),
            raw_bytes: budget::MAX_RAW_BYTES,
            verification: raw_budget::Limits::DEFAULT,
            units: MAX_ROWS,
            bytes: MAX_BYTES,
            auxiliary: MAX_AUXILIARY,
            trace_rows: MAX_ROWS,
            diagnostic_work: MAX_DIAGNOSTIC_WORK,
        }
    }
}
impl Limits {
    fn bounded(mut self) -> Self {
        let d = Self::default();
        macro_rules! clamp { ($($field:ident),* $(,)?) => { $(self.source.$field = self.source.$field.min(d.source.$field);)* }; }
        clamp!(
            source_bytes,
            tokens,
            nodes,
            modules,
            depth,
            component_bytes,
            relative_bytes,
            path_bytes,
            probes,
            directory_entries,
            directory_name_units
        );
        self.index.retained = self.index.retained.min(d.index.retained);
        self.index.scratch = self.index.scratch.min(d.index.scratch);
        self.index.work = self.index.work.min(d.index.work);
        self.raw_bytes = self.raw_bytes.min(d.raw_bytes);
        self.verification.owners = self.verification.owners.min(d.verification.owners);
        self.verification.events = self.verification.events.min(d.verification.events);
        self.verification.work = self.verification.work.min(d.verification.work);
        self.verification.scratch = self.verification.scratch.min(d.verification.scratch);
        self.verification.metadata = self.verification.metadata.min(d.verification.metadata);
        self.units = self.units.min(d.units);
        self.bytes = self.bytes.min(d.bytes);
        self.auxiliary = self.auxiliary.min(d.auxiliary);
        self.trace_rows = self.trace_rows.min(d.trace_rows);
        self.diagnostic_work = self.diagnostic_work.min(d.diagnostic_work);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Validated,
    Diagnostic,
    ResourceControl,
    IncompleteObservation,
}
impl Outcome {
    fn name(self) -> &'static str {
        match self {
            Self::Validated => "validated",
            Self::Diagnostic => "diagnostic",
            Self::ResourceControl => "resource-control",
            Self::IncompleteObservation => "incomplete-observation",
        }
    }
}
pub(super) struct Receipt {
    pub transcript: String,
    pub outcome: Outcome,
    pub reason: Option<&'static str>,
    pub units: usize,
    pub bytes: usize,
}
impl Receipt {
    fn incomplete(reason: &'static str) -> Self {
        Self {
            transcript: String::new(),
            outcome: Outcome::IncompleteObservation,
            reason: Some(reason),
            units: 0,
            bytes: 0,
        }
    }
}
type ObservationResult<T> = Result<T, &'static str>;

#[derive(Default)]
struct Counter {
    bytes: usize,
    units: usize,
    string: bool,
    escape: bool,
    atom: bool,
}
impl Write for Counter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(text.len()).ok_or(fmt::Error)?;
        for byte in text.bytes() {
            if self.string {
                if self.escape {
                    self.escape = false;
                } else if byte == b'\\' {
                    self.escape = true;
                } else if byte == b'"' {
                    self.string = false;
                }
                continue;
            }
            match byte {
                b'"' => {
                    self.units = self.units.checked_add(1).ok_or(fmt::Error)?;
                    self.string = true;
                    self.atom = false;
                }
                b'[' | b'{' => {
                    self.units = self.units.checked_add(1).ok_or(fmt::Error)?;
                    self.atom = false;
                }
                b']' | b'}' | b',' | b':' | b' ' | b'\r' | b'\n' | b'\t' => self.atom = false,
                _ => {
                    if !self.atom {
                        self.units = self.units.checked_add(1).ok_or(fmt::Error)?;
                    }
                    self.atom = true;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum LowerPass {
    PreflightCount,
    EmissionCount,
    BlockCount,
    Emit,
}
impl LowerPass {
    fn name(self) -> &'static str {
        match self {
            Self::PreflightCount => "preflight-count",
            Self::EmissionCount => "emission-count",
            Self::BlockCount => "block-count",
            Self::Emit => "emit",
        }
    }
}
#[derive(Clone, Copy)]
struct LiteralRequest {
    function: usize,
    expression: usize,
    count: raw_budget::FunctionCounts,
}
struct Output {
    text: String,
    limits: Limits,
    control: Control,
    rows: usize,
    units: usize,
    failure: Option<&'static str>,
    auxiliary: usize,
    auxiliary_peak: usize,
    diagnostic_work: usize,
    ast_elements: Option<usize>,
    hir_elements: Option<usize>,
    raw_elements: Option<usize>,
    literal_requests: usize,
    literal: Option<LiteralRequest>,
}
impl Output {
    fn new(limits: Limits, control: Control) -> ObservationResult<Self> {
        let mut text = String::new();
        text.try_reserve_exact(if control == Control::FailTranscriptReservation {
            usize::MAX
        } else {
            limits.bytes
        })
        .map_err(|_| "transcript reservation failed")?;
        Ok(Self {
            text,
            limits,
            control,
            rows: 0,
            units: 0,
            failure: None,
            auxiliary: 0,
            auxiliary_peak: 0,
            diagnostic_work: 0,
            ast_elements: None,
            hir_elements: None,
            raw_elements: None,
            literal_requests: 0,
            literal: None,
        })
    }
    fn row(&mut self, family: &'static str, fields: fmt::Arguments<'_>) -> ObservationResult<()> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let result = self.append_row(family, fields);
        if let Err(error) = result {
            self.failure = Some(error);
        }
        result
    }
    fn append_row(
        &mut self,
        family: &'static str,
        fields: fmt::Arguments<'_>,
    ) -> ObservationResult<()> {
        let mut count = Counter::default();
        write!(
            &mut count,
            "[1,{},{},{}]\n",
            self.rows,
            Json(family),
            fields
        )
        .map_err(|_| "row counting overflow")?;
        let bytes = self
            .text
            .len()
            .checked_add(count.bytes)
            .ok_or("transcript byte overflow")?;
        let units = self
            .units
            .checked_add(count.units)
            .ok_or("transcript unit overflow")?;
        let rows = self.rows.checked_add(1).ok_or("transcript row overflow")?;
        if bytes > self.limits.bytes || units > self.limits.units {
            return Err("structural observation limit exceeded");
        }
        if bytes > self.text.capacity() {
            return Err("unadmitted transcript growth");
        }
        write!(
            &mut self.text,
            "[1,{},{},{}]\n",
            self.rows,
            Json(family),
            fields
        )
        .map_err(|_| "row formatting failed")?;
        if self.text.len() != bytes {
            return Err("row count/render mismatch");
        }
        self.rows = rows;
        self.units = units;
        Ok(())
    }
    fn aux(&mut self, bytes: usize) -> ObservationResult<()> {
        self.auxiliary = self
            .auxiliary
            .checked_add(bytes)
            .ok_or("auxiliary count overflow")?;
        if self.auxiliary > self.limits.auxiliary {
            return Err("auxiliary observation limit exceeded");
        }
        self.auxiliary_peak = self.auxiliary_peak.max(self.auxiliary);
        Ok(())
    }
    fn phase(&mut self, phase: &'static str, status: &'static str) -> ObservationResult<()> {
        self.row("phase", format_args!("{},{}", Json(phase), Json(status)))
    }
    fn finish(mut self, outcome: Outcome) -> Receipt {
        if let Some(error) = self.failure {
            return Receipt::incomplete(error);
        }
        let units = self.units;
        let bytes = self.text.len();
        let accepted = outcome == Outcome::Validated;
        let requested = self.limits.bytes;
        let auxiliary = self.auxiliary_peak;
        let work = self.diagnostic_work;
        let elements = (self.ast_elements, self.hir_elements, self.raw_elements);
        if let Err(error) = self.row(
            "terminal",
            format_args!(
                "{},{},{},{},{},{},{},{},{},{},{},{}",
                Json(outcome.name()),
                accepted,
                accepted,
                accepted,
                units,
                bytes,
                requested,
                auxiliary,
                work,
                Optional(elements.0),
                Optional(elements.1),
                Optional(elements.2)
            ),
        ) {
            return Receipt::incomplete(error);
        }
        let bytes = self.text.len();
        Receipt {
            transcript: self.text,
            outcome,
            reason: None,
            units: self.units,
            bytes,
        }
    }
}

thread_local! { static CAPTURE: RefCell<Option<Output>> = const { RefCell::new(None) }; }
fn event(operation: impl FnOnce(&mut Output) -> ObservationResult<()>) {
    CAPTURE.with(|capture| {
        let mut capture = capture.borrow_mut();
        if let Some(out) = capture.as_mut() {
            if out.failure.is_none() {
                if let Err(error) = operation(out) {
                    out.failure = Some(error);
                }
            }
        }
    });
}
pub(super) fn completed(pass: LowerPass, function: usize, expression: usize, span: Span) {
    event(|out| {
        if out.control.resource_only() {
            return Ok(());
        }
        out.row(
            "lower-complete",
            format_args!(
                "{},{},{},{}",
                Json(pass.name()),
                function,
                expression,
                SpanRow(span)
            ),
        )
    });
}
pub(super) fn counted(
    pass: LowerPass,
    function: usize,
    count: raw_budget::FunctionCounts,
    complete: bool,
) {
    event(|out| {
        if !complete && !out.control.resource_only() {
            return Ok(());
        }
        out.row(
            "lower-counts",
            format_args!(
                "{},{},{},{}",
                Json(pass.name()),
                function,
                complete,
                rows::CountsRow(count)
            ),
        )
    });
}
pub(super) fn counted_block(function: usize, block: usize, statements: usize) {
    event(|out| {
        if out.control.resource_only() {
            return Ok(());
        }
        out.row(
            "block-count",
            format_args!("{function},{block},{statements}"),
        )
    });
}
pub(super) fn literal_context(
    function: usize,
    expression: usize,
    count: raw_budget::FunctionCounts,
) {
    event(|out| {
        out.literal = Some(LiteralRequest {
            function,
            expression,
            count,
        });
        Ok(())
    });
}
pub(super) fn literal_reserved(length: usize, capacity_failure: bool, success: bool) {
    event(|out| {
        let context = out
            .literal
            .take()
            .ok_or("missing actual literal request context")?;
        let ordinal = out.literal_requests;
        out.literal_requests = ordinal.checked_add(1).ok_or("literal request overflow")?;
        let payload = length
            .checked_mul(size_of::<Operand>())
            .ok_or("literal payload overflow")?;
        out.row(
            "literal-reservation",
            format_args!(
                "{},{},{},{},{},{},{},{},{}",
                ordinal,
                context.function,
                context.expression,
                length,
                size_of::<Operand>(),
                payload,
                capacity_failure,
                success,
                rows::CountsRow(context.count)
            ),
        )
    });
}

fn captured_lower(
    typed: &typeck::TypedOwnedProgram<'_>,
    limits: budget::Limits,
    out: Output,
) -> ObservationResult<(Output, Result<RawOwnedProgram, OwnedFailure>)> {
    if CAPTURE.with(|capture| capture.borrow().is_some()) {
        return Err("nested source observation");
    }
    let control = out.control;
    CAPTURE.with(|capture| *capture.borrow_mut() = Some(out));
    struct Clear;
    impl Drop for Clear {
        fn drop(&mut self) {
            CAPTURE.with(|capture| {
                capture.borrow_mut().take();
            });
        }
    }
    let _clear = Clear;
    let raw = match control {
        Control::FailLiteralOperand { ordinal } => {
            budget::fail_array_operand_after(ordinal, || lower::lower_with_limits(typed, limits))
        }
        Control::ExpandedBoundaryFirstOperandFailure => {
            budget::fail_array_operand_after(0, || lower::lower_with_limits(typed, limits))
        }
        _ => lower::lower_with_limits(typed, limits),
    };
    let out = CAPTURE
        .with(|capture| capture.borrow_mut().take())
        .ok_or("lost source observation")?;
    if let Some(error) = out.failure {
        return Err(error);
    }
    Ok((out, raw))
}

pub(super) fn observe(
    input: SourceInput<'_>,
    mode: Mode,
    limits: Limits,
    control: Control,
) -> Receipt {
    match observe_inner(input, mode, limits.bounded(), control) {
        Ok((out, outcome)) => out.finish(outcome),
        Err(error) => Receipt::incomplete(error),
    }
}
fn observe_inner(
    input: SourceInput<'_>,
    mode: Mode,
    limits: Limits,
    control: Control,
) -> ObservationResult<(Output, Outcome)> {
    let mut out = Output::new(limits, control)?;
    let Mode::Validate = mode;
    out.aux(size_of::<Output>() + size_of::<Allocator>())?;
    out.aux(
        limits
            .trace_rows
            .checked_mul(size_of::<crate::frontend::project::budget::ReserveEvent>())
            .ok_or("trace payload overflow")?,
    )?;
    let mut allocator = Allocator::default();
    if control == Control::FailTraceReservation {
        allocator
            .observer_trace_bound_capacity_failure(limits.trace_rows)
            .map_err(|_| "frontend trace reservation failed")?;
    } else {
        allocator
            .observer_trace_bound(limits.trace_rows)
            .map_err(|_| "frontend trace reservation failed")?;
    }
    out.row(
        "header",
        format_args!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            Json("oxid-array-source-lowering-v1"),
            Json("validate"),
            Json(control.name()),
            Json("ObserveArrayPipeline"),
            limits.units,
            limits.bytes,
            limits.auxiliary,
            limits.trace_rows,
            limits.index.retained,
            limits.index.scratch,
            limits.index.work,
            limits.raw_bytes,
            limits.verification.events,
            limits.verification.work,
            limits.diagnostic_work
        ),
    )?;
    out.phase("load-parse", "attempted")?;
    match input {
        SourceInput::Root { path } => {
            let project = match ProjectSources::load_array_candidate(
                path.to_str().ok_or("non-UTF-8 source path")?,
                limits.source,
                &mut allocator,
            ) {
                Ok(project) => project,
                Err(mut failure) => {
                    out.phase("load-parse", "failed")?;
                    rows::sources(&mut out, &failure.sources)?;
                    rows::trace(&mut out, &failure.allocator)?;
                    release_trace(&mut out, &mut failure.allocator)?;
                    rows::diagnostics(&mut out, &failure.sources, &failure.diagnostics)?;
                    return Ok((out, Outcome::Diagnostic));
                }
            };
            out.phase("load-parse", "completed")?;
            rows::sources(&mut out, project.sources())?;
            rows::modules(&mut out, project.modules())?;
            let owner = SourceOwner::project(&project);
            checked_owner(owner, project.sources(), &mut allocator, out)
        }
        SourceInput::Bytes { path, text } => {
            if text.len() > limits.source.source_bytes || path.len() > limits.source.path_bytes {
                return Err("single-source input limit exceeded");
            }
            let mut source_text = String::new();
            allocator
                .string(&mut source_text, text.len(), "observer original source")
                .map_err(|_| "source text reservation failed")?;
            source_text.push_str(text);
            let mut source_path = String::new();
            allocator
                .string(&mut source_path, path.len(), "observer original path")
                .map_err(|_| "source path reservation failed")?;
            source_path.push_str(path);
            let mut sources = SourceMap::new();
            let id = sources
                .try_add(source_path, source_text, &mut allocator)
                .map_err(|_| "single-source map reservation failed")?;
            let source = sources.get(id);
            let parsed = lexer::lex_with_limit(source, limits.source.tokens)
                .map_err(|error| vec![*error])
                .and_then(|tokens| {
                    parser::parse_counted_with_arrays(
                        source,
                        tokens,
                        parser::SourceMode::OwnedCandidate,
                        limits.source.nodes,
                        &mut allocator,
                        parser::ArraySyntaxPolicy::Candidate,
                    )
                    .map(|(ast, _)| ast)
                });
            rows::sources(&mut out, &sources)?;
            rows::original_module(&mut out, source)?;
            let ast = match parsed {
                Ok(ast) => ast,
                Err(errors) => {
                    out.phase("load-parse", "failed")?;
                    rows::trace(&mut out, &allocator)?;
                    release_trace(&mut out, &mut allocator)?;
                    rows::diagnostics(&mut out, &sources, &errors)?;
                    return Ok((out, Outcome::Diagnostic));
                }
            };
            out.phase("load-parse", "completed")?;
            let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources))
                .map_err(|_| "single-source owner association failed")?;
            checked_owner(owner, &sources, &mut allocator, out)
        }
    }
}
fn release_trace(out: &mut Output, allocator: &mut Allocator) -> ObservationResult<()> {
    let requested = out
        .limits
        .trace_rows
        .checked_mul(size_of::<crate::frontend::project::budget::ReserveEvent>())
        .ok_or("trace payload overflow")?;
    drop(std::mem::take(&mut allocator.trace));
    out.auxiliary = out
        .auxiliary
        .checked_sub(requested)
        .ok_or("trace lifetime underflow")?;
    Ok(())
}
fn checked_owner(
    owner: SourceOwner<'_>,
    sources: &SourceMap,
    allocator: &mut Allocator,
    mut out: Output,
) -> ObservationResult<(Output, Outcome)> {
    out.phase("source-owner", "completed")?;
    let work = WorkMeter::new(out.limits.index.work);
    out.phase("select", "attempted")?;
    let selected = match owner.owned(&work) {
        Ok(selected) => selected,
        Err(error) => {
            out.phase("select", "failed")?;
            rows::trace(&mut out, allocator)?;
            release_trace(&mut out, allocator)?;
            rows::diagnostics(&mut out, sources, &[*error])?;
            return Ok((out, Outcome::Diagnostic));
        }
    };
    out.phase("select", "completed")?;
    out.row(
        "selector",
        format_args!(
            "{},{}",
            selected,
            Json(if selected {
                "whole-source-owned-selection"
            } else {
                "private-owned-direct-control"
            })
        ),
    )?;
    rows::ast_inventory(&mut out, owner)?;
    out.phase("resolve-index", "attempted")?;
    let resolved = match resolve::resolve_array_pipeline(owner, out.limits.index, &work, allocator)
    {
        Ok(resolved) => resolved,
        Err(errors) => {
            out.phase("resolve-index", "failed")?;
            rows::trace(&mut out, allocator)?;
            release_trace(&mut out, allocator)?;
            rows::diagnostics(&mut out, sources, &errors)?;
            return Ok((out, Outcome::Diagnostic));
        }
    };
    out.phase("resolve-index", "completed")?;
    rows::resolved(&mut out, &resolved)?;
    out.phase("type", "attempted")?;
    let typed = match typeck::check(resolved) {
        Ok(typed) => typed,
        Err(errors) => {
            out.phase("type", "failed")?;
            rows::trace(&mut out, allocator)?;
            release_trace(&mut out, allocator)?;
            rows::diagnostics(&mut out, sources, &errors)?;
            return Ok((out, Outcome::Diagnostic));
        }
    };
    out.phase("type", "completed")?;
    if !std::ptr::eq(
        match typed.index().sources().view() {
            SourceView::Map(map) => map,
            _ => return Err("pipeline source map missing"),
        },
        sources,
    ) || typed.entry() != typed.index().root_original_main()
    {
        return Err("pipeline root/source identity mismatch");
    }
    rows::typed(&mut out, &typed)?;
    rows::trace(&mut out, allocator)?;
    release_trace(&mut out, allocator)?;
    out.phase("lower", "attempted")?;
    let raw_limit = budget::Limits {
        raw_bytes: out.limits.raw_bytes,
    };
    let (mut out, raw) = captured_lower(&typed, raw_limit, out)?;
    let raw = match raw {
        Ok(raw) => raw,
        Err(error) => {
            drop(typed);
            out.phase("lower", "failed")?;
            rows::diagnostics(&mut out, sources, &[*diagnostic::lower(&error, sources)])?;
            let outcome = if out.control.resource_only() {
                Outcome::ResourceControl
            } else {
                Outcome::Diagnostic
            };
            return Ok((out, outcome));
        }
    };
    out.phase("lower", "completed")?;
    if out.control.resource_only() {
        drop(typed);
        drop(raw);
        return Ok((out, Outcome::ResourceControl));
    }
    rows::raw(&mut out, &raw)?;
    if out.ast_elements != out.hir_elements || out.hir_elements != out.raw_elements {
        return Err("actual AST/HIR/raw element totals differ");
    }
    out.phase("associate", "attempted")?;
    let association = association::check(&raw, typed.index(), sources);
    drop(typed);
    match association {
        Ok(usage) => {
            out.row(
                "association",
                format_args!(
                    "{},{},{},{},{}",
                    usage.count.declarations,
                    usage.count.spans,
                    usage.validation.declarations,
                    usage.validation.spans,
                    usage.dimensions
                ),
            )?;
            out.phase("associate", "completed")?;
        }
        Err(error) => {
            out.phase("associate", "failed")?;
            rows::diagnostics(&mut out, sources, &[*error])?;
            return Ok((out, Outcome::Diagnostic));
        }
    }
    out.phase("validate", "attempted")?;
    match verified::probe_array_validation(&raw, sources, out.limits.verification) {
        Ok(usage) => {
            out.row("validation", format_args!("{}", rows::UsageRow(usage)))?;
            out.phase("validate", "completed")?;
            Ok((out, Outcome::Validated))
        }
        Err(error) => {
            out.phase("validate", "failed")?;
            rows::diagnostics(&mut out, sources, &[*diagnostic::verify(&error, sources)])?;
            Ok((out, Outcome::Diagnostic))
        }
    }
}
