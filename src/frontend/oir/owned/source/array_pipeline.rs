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

#[path = "array_pipeline_tests.rs"]
mod tests;

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
    LexerNull { site: LexerNullSite },
}
impl Control {
    fn name(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::FailLiteralOperand { .. } => "literal-capacity-failure",
            Self::ExpandedBoundaryFirstOperandFailure => "expanded-boundary-first-operand-failure",
            Self::FailTranscriptReservation => "transcript-capacity-failure",
            Self::FailTraceReservation => "trace-capacity-failure",
            Self::LexerNull { .. } => "lexer-real-null",
        }
    }
    fn resource_only(self) -> bool {
        self == Self::ExpandedBoundaryFirstOperandFailure
    }
    fn ordinal(self) -> Option<usize> {
        match self {
            Self::FailLiteralOperand { ordinal } => Some(ordinal),
            Self::ExpandedBoundaryFirstOperandFailure => Some(0),
            Self::LexerNull { site } => Some(site.ordinal()),
            _ => None,
        }
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
/// Enforce the already admitted row extent on every actual write. The second
/// counter also detects equal-byte renderings with a different JSON unit count.
struct BoundedWrite<'a> {
    text: &'a mut String,
    end: usize,
    count: Counter,
}
impl Write for BoundedWrite<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let next = self.text.len().checked_add(text.len()).ok_or(fmt::Error)?;
        if next > self.end || next > self.text.capacity() {
            return Err(fmt::Error);
        }
        self.count.write_str(text)?;
        self.text.push_str(text);
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
        writeln!(&mut count, "[1,{},{},{}]", self.rows, Json(family), fields)
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
        let mut writer = BoundedWrite {
            text: &mut self.text,
            end: bytes,
            count: Counter::default(),
        };
        writeln!(&mut writer, "[1,{},{},{}]", self.rows, Json(family), fields)
            .map_err(|_| "row formatting failed")?;
        if writer.count.bytes != count.bytes
            || writer.count.units != count.units
            || writer.text.len() != bytes
        {
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
fn admit_bytes_input(
    path_bytes: usize,
    source_bytes: usize,
    limits: ProjectLimits,
) -> ObservationResult<()> {
    if limits.modules < 1 || source_bytes > limits.source_bytes || path_bytes > limits.path_bytes {
        Err("single-source input limit exceeded")
    } else {
        Ok(())
    }
}
fn observe_inner(
    input: SourceInput<'_>,
    mode: Mode,
    limits: Limits,
    control: Control,
) -> ObservationResult<(Output, Outcome)> {
    if matches!(control, Control::LexerNull { .. }) {
        match input {
            SourceInput::Bytes {
                path: LEXER_NULL_PATH,
                text: LEXER_NULL_TEXT,
            } if limits.source.tokens >= 8 => {}
            _ => return Err("lexer null fixture or token credit refused"),
        }
    }
    let mut out = Output::new(limits, control)?;
    let Mode::Validate = mode;
    // Include the fixed TLS wrapper while empty as well as the outside-capture
    // Output. During capture the wrapper contains that Output, not a second
    // transcript; this fixed allowance safely covers both lifetime phases.
    out.aux(size_of::<Output>() + size_of::<Allocator>() + size_of::<RefCell<Option<Output>>>())?;
    // Additive lexer helper/result allowance and its fixed qualification sink.
    // Preserve all earlier source/allocator/trace charges and the existing cap.
    out.aux(lexer::reservation_scratch_bytes() + lexer::reservation_observer_bytes())?;
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
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            Json("oxid-array-source-lowering-v1"),
            Json("validate"),
            Json(control.name()),
            Optional(control.ordinal()),
            Json(match input {
                SourceInput::Bytes { .. } => "bytes",
                SourceInput::Root { .. } => "root",
            }),
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
    let s = limits.source;
    out.row(
        "source-limits",
        format_args!(
            "{},{},{},{},{},{},{},{},{},{},{}",
            s.source_bytes,
            s.tokens,
            s.nodes,
            s.modules,
            s.depth,
            s.component_bytes,
            s.relative_bytes,
            s.path_bytes,
            s.probes,
            s.directory_entries,
            s.directory_name_units
        ),
    )?;
    let v = limits.verification;
    out.row(
        "verification-limits",
        format_args!(
            "{},{},{},{},{}",
            v.owners, v.events, v.work, v.scratch, v.metadata
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
                    // load_array_candidate moved its Allocator into LoadFailure;
                    // the original now-empty local remains alive as well.
                    out.aux(size_of::<crate::frontend::project::LoadFailure>())?;
                    out.phase("load-parse", "failed")?;
                    rows::sources(&mut out, &failure.sources)?;
                    rows::trace(&mut out, &failure.allocator)?;
                    release_trace(&mut out, &mut failure.allocator)?;
                    rows::diagnostics(
                        &mut out,
                        &failure.sources,
                        &failure.diagnostics,
                        failure.diagnostics.capacity(),
                    )?;
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
            admit_bytes_input(path.len(), text.len(), limits.source)?;
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
            let lexed = if let Control::LexerNull { site } = control {
                Err(source_null_lex(
                    source,
                    limits.source.tokens,
                    &mut allocator,
                    &mut out,
                    site,
                )?)
            } else {
                lexer::lex_with_allocator(source, limits.source.tokens, &mut allocator)
            };
            let parsed = lexed
                .map_err(|error| vec![*error.diagnostic()])
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
                    rows::diagnostics(&mut out, &sources, &errors, errors.capacity())?;
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
// Closed, test-only Bytes-source qualification. Selection is strictly narrower
// than transcript emission, diagnostic conversion and trace ownership release.
const LEXER_NULL_PATH: &str = "retained-null-source.ox";
const LEXER_NULL_TEXT: &str = "/**//**//**//**//**//**//**//**/";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LexerNullSite {
    First,
    GrowEight,
    GrowSixteen,
}
impl LexerNullSite {
    fn ordinal(self) -> usize {
        match self {
            Self::First => 5,
            Self::GrowEight => 6,
            Self::GrowSixteen => 7,
        }
    }
    fn slots(self) -> (usize, usize) {
        match self {
            Self::First => (0, 4),
            Self::GrowEight => (4, 8),
            Self::GrowSixteen => (8, 16),
        }
    }
}
use crate::frontend::project::budget::real_null_observer::{
    self as lexer_null, growth as lexer_growth,
};
#[derive(Clone, Copy, Debug)]
struct SourceNullFacts {
    failure: Option<lexer::Failure>,
    unexpected_success: bool,
}
#[derive(Clone, Copy, Debug)]
enum SourceNullReport {
    Fresh(lexer_null::Report),
    Growth(lexer_growth::GrowthReport),
}
#[derive(Clone, Copy, Debug)]
struct SourceNullRow {
    ordinal: usize,
    operation: &'static str,
    old_bytes: usize,
    new_bytes: usize,
    fired: bool,
    reserve_failed: bool,
    owner_unchanged: Option<bool>,
    address_unchanged: Option<bool>,
    length_unchanged: Option<bool>,
    capacity_unchanged: Option<bool>,
    drop_count: u8,
    trace_preserved: bool,
    source_preserved: bool,
}
fn source_null_action(
    source: &SourceFile,
    token_limit: usize,
) -> impl for<'a> FnOnce(&'a mut Allocator) -> SourceNullFacts + '_ {
    move |allocator| match lexer::lex_with_allocator(source, token_limit, allocator) {
        Err(failure) => SourceNullFacts {
            failure: Some(failure),
            unexpected_success: false,
        },
        Ok(tokens) => {
            // Qualification refusal never leaks, returns or invents an owner.
            drop(tokens);
            SourceNullFacts {
                failure: None,
                unexpected_success: true,
            }
        }
    }
}
#[allow(dead_code)]
struct SourceNullControlBank<'a> {
    control: Control,
    caller_control: Control,
    site: LexerNullSite,
    dispatch_site: LexerNullSite,
    facts: SourceNullFacts,
    returned_facts: SourceNullFacts,
    caller_facts: SourceNullFacts,
    report: SourceNullReport,
    returned_report: SourceNullReport,
    caller_report: SourceNullReport,
    optional_failure: Option<lexer::Failure>,
    failure: lexer::Failure,
    returned_failure: lexer::Failure,
    caller_failure: lexer::Failure,
    fresh_target: lexer_null::Target,
    growth_target: lexer_growth::GrowthTarget,
    old_layout: std::alloc::Layout,
    new_layout: std::alloc::Layout,
    old_layout_result: Result<std::alloc::Layout, std::alloc::LayoutError>,
    new_layout_result: Result<std::alloc::Layout, std::alloc::LayoutError>,
    source: &'a SourceFile,
    token_limit: usize,
    source_identity: u64,
    source_after: u64,
    source_matches: bool,
    report_matches: bool,
    trace_matches: bool,
    ordinal: usize,
    old_slots: usize,
    new_slots: usize,
    old_bytes: usize,
    new_bytes: usize,
    trace_len: usize,
    trace_capacity: usize,
    trace_limit: Option<usize>,
    configured_trace: usize,
    lexical_return: Result<Vec<lexer::Token>, lexer::Failure>,
    lexical_caller: Result<Vec<lexer::Token>, lexer::Failure>,
    returned: ObservationResult<lexer::Failure>,
    caller: ObservationResult<lexer::Failure>,
    selected_return: ObservationResult<(SourceNullFacts, SourceNullReport)>,
    selected_caller: ObservationResult<(SourceNullFacts, SourceNullReport)>,
    row: SourceNullRow,
    row_return: Option<SourceNullRow>,
    row_caller: Option<SourceNullRow>,
    format_arguments: fmt::Arguments<'a>,
    format_arguments_caller: fmt::Arguments<'a>,
    trace_event: &'a crate::frontend::project::budget::ReserveEvent,
    trace_index: usize,
}
#[allow(dead_code)]
struct SourceNullTrackerBank<'a> {
    raw_enabled_tls: std::cell::Cell<bool>,
    raw_count_tls: std::cell::Cell<usize>,
    // Existing GlobalAlloc accounting callbacks borrow these three TLS cells.
    // These are reference transport roles, separate from the referent storage.
    raw_enabled_callback: &'a std::cell::Cell<bool>,
    raw_count_callback: &'a std::cell::Cell<usize>,
    // The private source cell's referent is priced by the actual accessor.
    // Cell<()> names only its equally thin read-only reference transport;
    // no value/reference is constructed, cast or used to access source state.
    source_tracker_callback: &'a std::cell::Cell<()>,
    raw_read: bool,
    raw_count: usize,
    raw_increment: usize,
    raw_write: usize,
    // Private source tracker payload roles are added using the actual type
    // layout accessor below, never reconstructed as tuple surrogates.
    delta: isize,
    allocation: bool,
    accessor_return: (usize, usize, usize, usize, usize, usize),
    accessor_caller: (usize, usize, usize, usize, usize, usize),
}
#[allow(dead_code)]
struct SourceNullSizingCarriers<'a, F> {
    action: &'a F,
    source_layout: (usize, usize, usize, usize, usize, usize),
    fresh: usize,
    growth: usize,
    banks: [usize; 5],
    fold_value: usize,
    fold_result: Option<usize>,
    returned: ObservationResult<usize>,
    caller: ObservationResult<usize>,
}
fn source_null_bank_bytes<F>(action: &F) -> ObservationResult<usize>
where
    F: for<'a> FnOnce(&'a mut Allocator) -> SourceNullFacts,
{
    let source_layout = super::reviewer_source::integration_tracker_layout();
    let fresh = lexer_null::selection_carriers_bytes(action);
    let growth = lexer_growth::selection_carriers_bytes(action);
    [
        size_of::<SourceNullControlBank<'_>>(),
        size_of::<SourceNullTrackerBank<'_>>()
            .checked_add(source_layout.0)
            .and_then(|bytes| bytes.checked_add(source_layout.2))
            .and_then(|bytes| bytes.checked_add(source_layout.2))
            .and_then(|bytes| bytes.checked_add(source_layout.4))
            .ok_or("lexer null bank overflow")?,
        lexer_growth::fixed_carriers_bytes::<lexer::Token>(),
        fresh.max(growth),
        size_of::<SourceNullSizingCarriers<'_, F>>(),
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
    .ok_or("lexer null bank overflow")
}
fn source_null_trace_matches(allocator: &Allocator, ordinal: usize, failed: bool) -> bool {
    allocator.attempts == ordinal
        && allocator.trace.len() == ordinal
        && !allocator.observer_trace_overflow
        && allocator.trace.iter().enumerate().all(|(index, event)| {
            event.success == (!failed || index + 1 != ordinal)
                && match index {
                    0 => {
                        event.kind == "observer original source"
                            && event.length == LEXER_NULL_TEXT.len()
                            && event.element_bytes == 1
                    }
                    1 => {
                        event.kind == "observer original path"
                            && event.length == LEXER_NULL_PATH.len()
                            && event.element_bytes == 1
                    }
                    2 => {
                        event.kind == "line starts"
                            && event.length == 1
                            && event.element_bytes == size_of::<usize>()
                    }
                    3 => {
                        event.kind == "source files"
                            && event.length == 1
                            && event.element_bytes == size_of::<SourceFile>()
                    }
                    4 => {
                        event.kind == "lexer token tape"
                            && event.length == 4
                            && event.element_bytes == size_of::<lexer::Token>()
                    }
                    5 => {
                        event.kind == "lexer token tape"
                            && event.length == 8
                            && event.element_bytes == size_of::<lexer::Token>()
                    }
                    6 => {
                        event.kind == "lexer token tape"
                            && event.length == 16
                            && event.element_bytes == size_of::<lexer::Token>()
                    }
                    _ => false,
                }
        })
}
fn source_null_preselector(
    allocator: &Allocator,
    configured_trace: usize,
    site: LexerNullSite,
) -> ObservationResult<()> {
    if configured_trace < site.ordinal()
        || allocator.observer_trace_limit != Some(configured_trace)
        || allocator.trace.capacity() < configured_trace
        || allocator.fail_at.is_some()
        || !source_null_trace_matches(allocator, 4, false)
    {
        Err("lexer null trace prepayment or prefix refused")
    } else {
        Ok(())
    }
}
fn source_null_row(
    report: SourceNullReport,
    site: LexerNullSite,
    source_preserved: bool,
) -> Option<SourceNullRow> {
    let (old, new) = site.slots();
    let old_bytes = old.checked_mul(size_of::<lexer::Token>())?;
    let new_bytes = new.checked_mul(size_of::<lexer::Token>())?;
    match report {
        SourceNullReport::Fresh(report) => {
            let target = lexer_null::Target {
                attempt: site.ordinal(),
                kind: "lexer token tape",
                slots: new,
                element_bytes: size_of::<lexer::Token>(),
                layout: std::alloc::Layout::array::<lexer::Token>(new).ok()?,
            };
            if site != LexerNullSite::First
                || report.target != target
                || !report.selected
                || !report.matched
                || !report.fired
                || report.rejection.is_some()
                || report.actual
                    != Some(lexer_null::GlobalEvent {
                        operation: lexer_null::Operation::Alloc,
                        layout: target.layout,
                        new_size: None,
                    })
            {
                return None;
            }
            Some(SourceNullRow {
                ordinal: site.ordinal(),
                operation: "alloc",
                old_bytes,
                new_bytes,
                fired: true,
                reserve_failed: true,
                owner_unchanged: None,
                address_unchanged: None,
                length_unchanged: None,
                capacity_unchanged: None,
                drop_count: 0,
                trace_preserved: true,
                source_preserved,
            })
        }
        SourceNullReport::Growth(report) => {
            let target = lexer_growth::GrowthTarget {
                attempt: site.ordinal(),
                kind: "lexer token tape",
                old_len: old,
                old_capacity: old,
                additional: new.checked_sub(old)?,
                new_slots: new,
                element_bytes: size_of::<lexer::Token>(),
                element_align: std::mem::align_of::<lexer::Token>(),
                old_layout: std::alloc::Layout::array::<lexer::Token>(old).ok()?,
                new_layout: std::alloc::Layout::array::<lexer::Token>(new).ok()?,
                operation: lexer_null::Operation::Realloc,
            };
            if site == LexerNullSite::First
                || report.target != target
                || !report.selected
                || !report.matched
                || !report.fired
                || report.rejection.is_some()
                || report.reserve_failed != Some(true)
                || !report.owner_unchanged
                || !report.address_unchanged
                || !report.length_unchanged
                || !report.capacity_unchanged
                || !report.trace_preserved
                || report.drop_count != 1
                || report.actual
                    != Some(lexer_growth::GrowthEvent {
                        operation: lexer_null::Operation::Realloc,
                        layout: target.old_layout,
                        new_size: Some(target.new_layout.size()),
                        old_address_matches: true,
                    })
                || report.drop_event
                    != Some(lexer_growth::GrowthDropEvent {
                        layout: target.old_layout,
                        old_address_matches: true,
                        after_reserve_return: true,
                    })
            {
                return None;
            }
            Some(SourceNullRow {
                ordinal: site.ordinal(),
                operation: "realloc",
                old_bytes,
                new_bytes,
                fired: true,
                reserve_failed: true,
                owner_unchanged: Some(report.owner_unchanged),
                address_unchanged: Some(report.address_unchanged),
                length_unchanged: Some(report.length_unchanged),
                capacity_unchanged: Some(report.capacity_unchanged),
                drop_count: report.drop_count,
                trace_preserved: report.trace_preserved,
                source_preserved,
            })
        }
    }
}
fn source_null_failure(facts: SourceNullFacts) -> ObservationResult<lexer::Failure> {
    if facts.unexpected_success {
        return Err("lexer null unexpected successful tape");
    }
    facts
        .failure
        .filter(|failure| failure.is_storage())
        .ok_or("lexer null storage failure missing")
}
fn source_null_lex(
    source: &SourceFile,
    token_limit: usize,
    allocator: &mut Allocator,
    out: &mut Output,
    site: LexerNullSite,
) -> ObservationResult<lexer::Failure> {
    if source.path() != LEXER_NULL_PATH
        || source.text() != LEXER_NULL_TEXT
        || source.span(0, 0).file != crate::frontend::source::SourceFileId(0)
        || token_limit < 8
    {
        return Err("lexer null fixture or token credit refused");
    }
    // The actual closure can only be sized after this actual source is registered.
    // Admission failure retains all four completed requests and makes no NEW selected call.
    let action = source_null_action(source, token_limit);
    out.aux(source_null_bank_bytes(&action)?)?;
    source_null_preselector(allocator, out.limits.trace_rows, site)?;
    let source_identity = source.identity();
    let trace_capacity = allocator.trace.capacity();
    let (old, new) = site.slots();
    let (facts, report) = if site == LexerNullSite::First {
        let target = lexer_null::Target {
            attempt: site.ordinal(),
            kind: "lexer token tape",
            slots: new,
            element_bytes: size_of::<lexer::Token>(),
            layout: std::alloc::Layout::array::<lexer::Token>(new)
                .map_err(|_| "lexer null layout refused")?,
        };
        let (facts, report) = lexer_null::with_selected(allocator, target, action)
            .map_err(|_| "lexer null fresh setup refused")?;
        (facts, SourceNullReport::Fresh(report))
    } else {
        let target = lexer_growth::GrowthTarget {
            attempt: site.ordinal(),
            kind: "lexer token tape",
            old_len: old,
            old_capacity: old,
            additional: new - old,
            new_slots: new,
            element_bytes: size_of::<lexer::Token>(),
            element_align: std::mem::align_of::<lexer::Token>(),
            old_layout: std::alloc::Layout::array::<lexer::Token>(old)
                .map_err(|_| "lexer null old layout refused")?,
            new_layout: std::alloc::Layout::array::<lexer::Token>(new)
                .map_err(|_| "lexer null new layout refused")?,
            operation: lexer_null::Operation::Realloc,
        };
        let (facts, report) = lexer_growth::with_selected_growth(allocator, target, action)
            .map_err(|_| "lexer null growth setup refused")?;
        (facts, SourceNullReport::Growth(report))
    };
    // Both selectors are now completely ended. No diagnostics/Output/trace
    // ownership operation appeared inside the actual selected callback.
    let failure = source_null_failure(facts)?;
    let source_preserved = source.identity() == source_identity
        && source.text() == LEXER_NULL_TEXT
        && source.path() == LEXER_NULL_PATH;
    let row = source_null_row(report, site, source_preserved)
        .ok_or("lexer null callback qualification refused")?;
    if !source_preserved
        || allocator.trace.capacity() != trace_capacity
        || !source_null_trace_matches(allocator, site.ordinal(), true)
    {
        return Err("lexer null source or trace preservation refused");
    }
    out.row(
        "lexer-null",
        format_args!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}",
            row.ordinal,
            Json(row.operation),
            row.old_bytes,
            row.new_bytes,
            row.fired,
            row.reserve_failed,
            Optional(row.owner_unchanged),
            Optional(row.address_unchanged),
            Optional(row.length_unchanged),
            Optional(row.capacity_unchanged),
            row.drop_count,
            row.trace_preserved,
            row.source_preserved
        ),
    )?;
    Ok(failure)
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
            rows::diagnostics(&mut out, sources, &[*error], 1)?;
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
            rows::diagnostics(&mut out, sources, &errors, errors.capacity())?;
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
            rows::diagnostics(&mut out, sources, &errors, errors.capacity())?;
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
            rows::diagnostics(&mut out, sources, &[*diagnostic::lower(&error, sources)], 1)?;
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
            rows::diagnostics(&mut out, sources, &[*error], 1)?;
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
            rows::diagnostics(
                &mut out,
                sources,
                &[*diagnostic::verify(&error, sources)],
                1,
            )?;
            Ok((out, Outcome::Diagnostic))
        }
    }
}
