//! Private owned LLVM consumer. All metadata is bound to one immutable witness
//! by ExecutionPlan; the scalar source route does not enter this module.
use super::{plan::ExecutionPlan, verified::VerifiedOwnedProgram, *};
use std::fmt::Write;
use std::mem::size_of;

const MAX_FUNCTIONS: usize = 256;
const MAX_PARAMS: usize = 64;
const MAX_FUNCTION_SLOTS: usize = 256;
const MAX_SLOTS: usize = 8_192;
const MAX_BLOCKS: usize = 4_096;
const MAX_DEPTH: usize = 32;
const MAX_COST: usize = 100_000;
const MAX_NATIVE_BYTES: usize = 1024 * 1024;
const MAX_DIAGNOSTIC_BYTES: usize = 16 * 1024 * 1024;
const MAX_IR_BYTES: usize = 64 * 1024 * 1024;

fn reject(message: impl Into<String>, span: Option<Span>) -> Box<Diagnostic> {
    Diagnostic::new("E0700", "native-admission", message, span)
}
fn limit(value: usize, maximum: usize, name: &str, span: Span) -> Result<(), Box<Diagnostic>> {
    if value > maximum {
        Err(reject(
            format!("native owned {name} limit exceeded ({maximum})"),
            Some(span),
        ))
    } else {
        Ok(())
    }
}
fn add(a: usize, b: usize) -> Result<usize, Box<Diagnostic>> {
    a.checked_add(b)
        .ok_or_else(|| reject("native owned count overflow", None))
}
/// Requested metadata payload; inherited formatter/name allocations have a
/// separately bounded transient envelope, never an element-sized buffer.
const DIAGNOSTIC_TRANSIENT_BYTES: usize = size_of::<Diagnostic>() + 64;
const EMITTER_TRANSIENT_BYTES: usize = 32_768;

#[derive(Clone, Debug, Default)]
pub(super) struct NativeMetrics {
    pub occurrences: usize,
    pub unique: usize,
    pub row_sizes: [usize; 4],
    pub occurrence_bytes: usize,
    pub lookup_bytes: usize,
    pub message_header_bytes: usize,
    pub message_bytes: usize,
    pub plan_bytes: usize,
    pub admission_scratch_peak: usize,
    pub retained_bound_bytes: usize,
    pub metadata_admitted_bytes: usize,
    pub metadata_peak: usize,
    pub diagnostic_transient_peak: usize,
    pub emitter_transient_bound: usize,
    /// Sampled retained Invoke collection only; fixed names use the separate envelope.
    pub count_call_scratch_peak: usize,
    pub render_call_scratch_peak: usize,
    pub allocation_attempts: usize,
    pub failed_allocation: Option<&'static str>,
    pub sort_comparisons: [usize; 4],
    pub inventory_rows: usize,
    pub prefix_preflight_rows: usize,
    pub coordinate_rows: usize,
    pub source_prefix_bound: usize,
    pub source_bytes: usize,
    pub source_scalars: usize,
    pub message_count_bytes: usize,
    pub message_render_bytes: usize,
    pub transfer_cells: usize,
    pub count_bytes: usize,
    pub render_bytes: usize,
    pub count_expansions: usize,
    pub render_expansions: usize,
    pub count_ordinary_visits: usize,
    pub render_ordinary_visits: usize,
    pub count_predecessor_visits: usize,
    pub render_predecessor_visits: usize,
}
#[derive(Default)]
struct Accounting {
    metrics: NativeMetrics,
    #[cfg(test)]
    fail_after: Option<usize>,
}
impl Accounting {
    fn admission_peak(&mut self, bytes: usize) -> Result<(), Box<Diagnostic>> {
        self.metrics.admission_scratch_peak = self.metrics.admission_scratch_peak.max(bytes);
        self.metrics.metadata_peak = self
            .metrics
            .metadata_peak
            .max(add(self.metrics.plan_bytes, bytes)?);
        Ok(())
    }
    fn allocation(&mut self, phase: &'static str) -> Result<(), Box<Diagnostic>> {
        let attempt = self.metrics.allocation_attempts;
        self.metrics.allocation_attempts = add(attempt, 1)?;
        #[cfg(test)]
        if self.fail_after == Some(attempt) {
            self.metrics.failed_allocation = Some(phase);
            return Err(reject(
                format!("injected native owned {phase} allocation"),
                None,
            ));
        }
        let _ = phase;
        Ok(())
    }
    fn vector<T>(&mut self, count: usize, phase: &'static str) -> Result<Vec<T>, Box<Diagnostic>> {
        mul(count, size_of::<T>())?;
        self.allocation(phase)?;
        let mut result = Vec::new();
        result.try_reserve_exact(count).map_err(|_| {
            self.metrics.failed_allocation = Some(phase);
            reject(format!("native owned {phase} allocation"), None)
        })?;
        Ok(result)
    }
    fn string(&mut self, bytes: usize, phase: &'static str) -> Result<String, Box<Diagnostic>> {
        self.allocation(phase)?;
        let mut result = String::new();
        result.try_reserve_exact(bytes).map_err(|_| {
            self.metrics.failed_allocation = Some(phase);
            reject(format!("native owned {phase} allocation"), None)
        })?;
        Ok(result)
    }
}
fn mul(a: usize, b: usize) -> Result<usize, Box<Diagnostic>> {
    a.checked_mul(b)
        .ok_or_else(|| reject("native owned count overflow", None))
}
fn mismatch(name: &str) -> Box<Diagnostic> {
    Diagnostic::new(
        "E0500",
        "native-emission",
        format!("internal compiler error: owned {name} mismatch"),
        None,
    )
}

#[derive(Clone, Copy, Debug)]
struct Limits {
    functions: usize,
    parameters: usize,
    function_slots: usize,
    scalar_slots: usize,
    blocks: usize,
    depth: usize,
    cost: usize,
    cells: usize,
    live_cells: usize,
    bytes: usize,
    live_bytes: usize,
    diagnostic_bytes: usize,
    metadata_bytes: usize,
    ir_bytes: usize,
}
impl Limits {
    const DEFAULT: Self = Self {
        functions: MAX_FUNCTIONS,
        parameters: MAX_PARAMS,
        function_slots: MAX_FUNCTION_SLOTS,
        scalar_slots: MAX_SLOTS,
        blocks: MAX_BLOCKS,
        depth: MAX_DEPTH,
        cost: MAX_COST,
        cells: MAX_SLOTS,
        live_cells: MAX_SLOTS,
        bytes: MAX_NATIVE_BYTES,
        live_bytes: MAX_NATIVE_BYTES,
        diagnostic_bytes: MAX_DIAGNOSTIC_BYTES,
        metadata_bytes: plan::MAX_PLAN_BYTES,
        ir_bytes: MAX_IR_BYTES,
    };
    fn bounded(self) -> Self {
        let d = Self::DEFAULT;
        Self {
            functions: self.functions.min(d.functions),
            parameters: self.parameters.min(d.parameters),
            function_slots: self.function_slots.min(d.function_slots),
            scalar_slots: self.scalar_slots.min(d.scalar_slots),
            blocks: self.blocks.min(d.blocks),
            depth: self.depth.min(d.depth),
            cost: self.cost.min(d.cost),
            cells: self.cells.min(d.cells),
            live_cells: self.live_cells.min(d.live_cells),
            bytes: self.bytes.min(d.bytes),
            live_bytes: self.live_bytes.min(d.live_bytes),
            diagnostic_bytes: self.diagnostic_bytes.min(d.diagnostic_bytes),
            metadata_bytes: self.metadata_bytes.min(d.metadata_bytes),
            ir_bytes: self.ir_bytes.min(d.ir_bytes),
        }
    }
}
#[derive(Clone, Copy, Default, Debug)]
struct Bound {
    cost: usize,
    cyclic: bool,
    depth: usize,
    scalar_slots: usize,
    cells: usize,
    bytes: usize,
}

pub(super) fn native_module(
    witness: &VerifiedOwnedProgram,
    entry: Option<hir::DefId>,
    sources: &SourceMap,
) -> Result<String, Box<Diagnostic>> {
    native_module_limits(witness, entry, sources, plan::MAX_FUEL, Limits::DEFAULT)
}
#[cfg(test)]
pub(super) fn native_module_with_fuel(
    witness: &VerifiedOwnedProgram,
    entry: hir::DefId,
    sources: &SourceMap,
    fuel: usize,
) -> Result<String, Box<Diagnostic>> {
    native_module_limits(witness, Some(entry), sources, fuel, Limits::DEFAULT)
}
fn native_module_limits(
    witness: &VerifiedOwnedProgram,
    entry: Option<hir::DefId>,
    sources: &SourceMap,
    fuel: usize,
    limits: Limits,
) -> Result<String, Box<Diagnostic>> {
    native_module_accounted(
        witness,
        entry,
        sources,
        fuel,
        limits,
        &mut Accounting::default(),
    )
}
fn native_module_accounted(
    witness: &VerifiedOwnedProgram,
    entry: Option<hir::DefId>,
    sources: &SourceMap,
    fuel: usize,
    limits: Limits,
    accounting: &mut Accounting,
) -> Result<String, Box<Diagnostic>> {
    // Entry denial is deliberately before plan construction, diagnostics,
    // output text or tools. An invalid identity is an internal error.
    let id = entry.ok_or_else(|| {
        reject(
            "native compile requires a declared zero-argument main",
            None,
        )
    })?;
    let root = witness
        .functions()
        .get(id.0)
        .filter(|f| f.id == id)
        .ok_or_else(|| {
            Diagnostic::new(
                "E0500",
                "native-admission",
                "internal compiler error: invalid native entry identity",
                None,
            )
        })?;
    if !root.parameters.is_empty() {
        return Err(reject(
            "native main must have no parameters",
            Some(root.span),
        ));
    }
    if !matches!(root.result, ValueTy::Scalar(_)) {
        return Err(reject("native main must return a scalar", Some(root.span)));
    }
    let plan = ExecutionPlan::build(witness).map_err(|e| reject(e.name, e.span))?;
    accounting.metrics.plan_bytes = plan.metadata_bytes();
    accounting.metrics.metadata_peak = accounting.metrics.metadata_peak.max(plan.metadata_bytes());
    let limits = limits.bounded();
    let bounds = admit_accounted(&plan, limits, accounting)?;
    let admission_metadata = add(
        plan.metadata_bytes(),
        accounting.metrics.admission_scratch_peak,
    )?;
    accounting.metrics.metadata_peak = accounting.metrics.metadata_peak.max(admission_metadata);
    limit(
        admission_metadata,
        limits.metadata_bytes,
        "admission metadata bytes",
        root.span,
    )?;
    let guarded = bounds.iter().any(|b| b.cyclic);
    let diagnostics = Diagnostics::new_accounted(
        &plan,
        id,
        sources,
        guarded,
        limits.diagnostic_bytes,
        limits.metadata_bytes,
        accounting,
    )?;
    let emission_metadata = add(
        add(
            add(
                plan.metadata_bytes(),
                accounting.metrics.retained_bound_bytes,
            )?,
            add(
                accounting.metrics.lookup_bytes,
                accounting.metrics.message_header_bytes,
            )?,
        )?,
        EMITTER_TRANSIENT_BYTES,
    )?;
    accounting.metrics.emitter_transient_bound = EMITTER_TRANSIENT_BYTES;
    accounting.metrics.metadata_peak = accounting.metrics.metadata_peak.max(emission_metadata);
    limit(
        emission_metadata,
        limits.metadata_bytes,
        "emission metadata bytes",
        root.span,
    )?;
    let mut count = Emission::count(limits.ir_bytes);
    emit(
        &plan,
        id,
        &diagnostics,
        guarded,
        fuel.min(plan::MAX_FUEL),
        &mut count,
    );
    accounting.metrics.count_bytes = count.len;
    accounting.metrics.count_call_scratch_peak = count.call_scratch_peak;
    if count.exceeded {
        return Err(reject(
            format!(
                "native owned LLVM bytes limit exceeded ({})",
                limits.ir_bytes
            ),
            Some(root.span),
        ));
    }
    let text = accounting.string(count.len, "LLVM")?;
    let mut output = Emission {
        text: Some(text),
        ..Emission::count(count.len)
    };
    emit(
        &plan,
        id,
        &diagnostics,
        guarded,
        fuel.min(plan::MAX_FUEL),
        &mut output,
    );
    accounting.metrics.render_bytes = output.len;
    accounting.metrics.render_call_scratch_peak = output.call_scratch_peak;
    if output.exceeded || output.len != count.len {
        return Err(Diagnostic::new(
            "E0500",
            "native-emission",
            "internal compiler error: owned LLVM count/render mismatch",
            None,
        ));
    }
    Ok(output.text.expect("render pass"))
}

fn admit(plan: &ExecutionPlan<'_>, limits: Limits) -> Result<Vec<Bound>, Box<Diagnostic>> {
    admit_accounted(plan, limits, &mut Accounting::default())
}
fn admit_accounted(
    plan: &ExecutionPlan<'_>,
    limits: Limits,
    accounting: &mut Accounting,
) -> Result<Vec<Bound>, Box<Diagnostic>> {
    let functions = plan.witness().functions();
    let origin = functions[0].span; // Entry validation proves a nonempty program.
    limit(functions.len(), limits.functions, "function count", origin)?;
    let mut callers = vec![Vec::new(); functions.len()];
    let mut remaining = vec![0usize; functions.len()];
    accounting.metrics.plan_bytes = plan.metadata_bytes();
    let mut early_scratch = add(
        mul(callers.capacity(), size_of::<Vec<usize>>())?,
        mul(remaining.capacity(), size_of::<usize>())?,
    )?;
    accounting.admission_peak(early_scratch)?;
    let (mut scalar_slots, mut blocks, mut cells, mut bytes) = (0, 0, 0, 0);
    for f in functions {
        let u = plan.function(f.id).usage();
        limit(
            f.parameters.len(),
            limits.parameters,
            "parameter count",
            f.span,
        )?;
        limit(
            u.scalar_slots,
            limits.function_slots,
            "scalar slots per function",
            f.span,
        )?;
        limit(
            add(u.scalar_slots, u.owners)?,
            limits.function_slots,
            "scalar and owner slots per function",
            f.span,
        )?;
        scalar_slots = add(scalar_slots, u.scalar_slots)?;
        blocks = add(blocks, f.blocks.len())?;
        cells = add(cells, u.expanded_cells)?;
        bytes = add(bytes, u.native_bytes)?;
        for b in &f.blocks {
            if let OwnedTerminatorKind::Invoke { call, .. } =
                b.terminator.as_ref().expect("verified terminator").kind
            {
                remaining[f.id.0] = add(remaining[f.id.0], 1)?;
                let caller = &mut callers[f.calls[call.0].target.0];
                let before = caller.capacity();
                caller.push(f.id.0);
                early_scratch = add(
                    early_scratch,
                    mul(caller.capacity() - before, size_of::<usize>())?,
                )?;
                accounting.admission_peak(early_scratch)?;
            }
        }
    }
    limit(
        scalar_slots,
        limits.scalar_slots,
        "aggregate scalar slots",
        origin,
    )?;
    limit(blocks, limits.blocks, "aggregate blocks", origin)?;
    limit(cells, limits.cells, "aggregate expanded cells", origin)?;
    let mut ready: Vec<_> = remaining
        .iter()
        .enumerate()
        .filter_map(|(i, &n)| (n == 0).then_some(i))
        .collect();
    let mut bounds = vec![Bound::default(); functions.len()];
    let mut caller_bytes = mul(callers.capacity(), size_of::<Vec<usize>>())?;
    for caller in &callers {
        caller_bytes = add(caller_bytes, mul(caller.capacity(), size_of::<usize>())?)?;
    }
    let fixed_scratch = add(
        caller_bytes,
        add(
            mul(remaining.capacity(), size_of::<usize>())?,
            mul(bounds.capacity(), size_of::<Bound>())?,
        )?,
    )?;
    accounting.metrics.retained_bound_bytes = mul(bounds.capacity(), size_of::<Bound>())?;
    accounting.admission_peak(add(
        fixed_scratch,
        mul(ready.capacity(), size_of::<usize>())?,
    )?)?;
    let mut visited = 0;
    while let Some(i) = ready.pop() {
        visited += 1;
        let f = &functions[i];
        let u = plan.function(f.id).usage();
        // Determine unknown cost before adding any placeholders: even a late
        // cyclic callee makes the entire static sum nonbinding. Resource and
        // known acyclic arithmetic stay checked, while unknown sums saturate.
        let (local_cycle, cycle_scratch) = has_cycle(f)?;
        let scratch = add(
            add(fixed_scratch, mul(ready.capacity(), size_of::<usize>())?)?,
            cycle_scratch,
        )?;
        accounting.admission_peak(scratch)?;
        let cyclic = local_cycle
            || f.blocks.iter().any(|b| {
                match &b.terminator.as_ref().expect("verified terminator").kind {
                    OwnedTerminatorKind::Invoke { call, .. } => {
                        bounds[f.calls[call.0].target.0].cyclic
                    }
                    _ => false,
                }
            });
        let cost_add = |a: usize, b: usize| {
            if cyclic {
                Ok(a.saturating_add(b))
            } else {
                add(a, b)
            }
        };
        let mut bound = Bound {
            cost: u.expanded_cells,
            cyclic,
            depth: 1,
            scalar_slots: u.scalar_slots,
            cells: u.expanded_cells,
            bytes: u.native_bytes,
        };
        for b in &f.blocks {
            bound.cost = cost_add(bound.cost, usize::from(b.merge.is_some()))?;
            for statement in &b.statements {
                bound.cost = cost_add(bound.cost, plan.statement_cost(f.id, &statement.kind))?;
            }
            let term = &b.terminator.as_ref().expect("verified terminator").kind;
            let mut cost = plan.terminator_cost(f.id, term);
            if let OwnedTerminatorKind::Invoke { call, .. } = term {
                let target = f.calls[call.0].target;
                let child = bounds[target.0];
                // Child X is already in its bound, so never charge it twice.
                cost -= plan.function(target).usage().expanded_cells;
                cost = cost_add(cost, child.cost)?;
                bound.depth = bound.depth.max(add(1, child.depth)?);
                bound.scalar_slots = bound
                    .scalar_slots
                    .max(add(u.scalar_slots, child.scalar_slots)?);
                bound.cells = bound.cells.max(add(u.expanded_cells, child.cells)?);
                bound.bytes = bound.bytes.max(add(u.native_bytes, child.bytes)?);
            }
            bound.cost = cost_add(bound.cost, cost)?;
        }
        if !bound.cyclic {
            limit(
                add(1, bound.cost)?,
                limits.cost,
                "reference fuel upper bound",
                f.span,
            )?;
        }
        limit(bound.depth, limits.depth, "call depth", f.span)?;
        limit(
            bound.scalar_slots,
            limits.scalar_slots,
            "live scalar slots",
            f.span,
        )?;
        limit(
            bound.cells,
            limits.live_cells,
            "live expanded cells",
            f.span,
        )?;
        bounds[i] = bound;
        for &caller in &callers[i] {
            remaining[caller] -= 1;
            if remaining[caller] == 0 {
                ready.push(caller);
            }
        }
    }
    if visited != functions.len() {
        let f = &functions[remaining
            .iter()
            .position(|&n| n != 0)
            .expect("cycle member")];
        return Err(reject("native preview does not support recursive call graphs, including unused functions and unchosen branches", Some(f.span)));
    }
    let fuel_bytes = if bounds.iter().any(|b| b.cyclic) {
        8
    } else {
        0
    };
    limit(
        add(bytes, fuel_bytes)?,
        limits.bytes,
        "aggregate storage bytes",
        origin,
    )?;
    for (f, bound) in functions.iter().zip(&bounds) {
        limit(
            add(bound.bytes, fuel_bytes)?,
            limits.live_bytes,
            "live storage bytes",
            f.span,
        )?;
    }
    Ok(bounds)
}
fn successors(kind: &OwnedTerminatorKind) -> impl Iterator<Item = BlockId> {
    let targets = match kind {
        OwnedTerminatorKind::ReturnScalar(_) | OwnedTerminatorKind::ReturnOwned(_) => [None, None],
        OwnedTerminatorKind::Goto(target) => [Some(*target), None],
        OwnedTerminatorKind::Invoke { continuation, .. } => [Some(*continuation), None],
        OwnedTerminatorKind::Branch {
            then_block,
            else_block,
            ..
        } => [Some(*then_block), Some(*else_block)],
    };
    targets.into_iter().flatten()
}
fn has_cycle(f: &RawOwnedFunction) -> Result<(bool, usize), Box<Diagnostic>> {
    let mut incoming = vec![0usize; f.blocks.len()];
    for b in &f.blocks {
        for target in successors(&b.terminator.as_ref().expect("verified terminator").kind) {
            incoming[target.0] += 1;
        }
    }
    let mut ready: Vec<_> = incoming
        .iter()
        .enumerate()
        .filter_map(|(i, &n)| (n == 0).then_some(i))
        .collect();
    let mut scratch = mul(
        add(incoming.capacity(), ready.capacity())?,
        size_of::<usize>(),
    )?;
    let mut visited = 0;
    while let Some(i) = ready.pop() {
        visited += 1;
        for target in successors(
            &f.blocks[i]
                .terminator
                .as_ref()
                .expect("verified terminator")
                .kind,
        ) {
            incoming[target.0] -= 1;
            if incoming[target.0] == 0 {
                ready.push(target.0);
            }
        }
        scratch = scratch.max(mul(
            add(incoming.capacity(), ready.capacity())?,
            size_of::<usize>(),
        )?);
    }
    Ok((visited != f.blocks.len(), scratch))
}

struct Emission {
    len: usize,
    text: Option<String>,
    maximum: usize,
    exceeded: bool,
    call_scratch_peak: usize,
    #[cfg(test)]
    field_visits: usize,
}
impl Emission {
    fn count(maximum: usize) -> Self {
        Self {
            len: 0,
            text: None,
            maximum,
            exceeded: false,
            call_scratch_peak: 0,
            #[cfg(test)]
            field_visits: 0,
        }
    }
}
impl Write for Emission {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        // The formatter remains infallible, but every expansion loop observes
        // this stop flag. A rejected count pass never walks an arbitrarily
        // large field-copy suffix or allocates the final text buffer.
        if self.exceeded {
            return Ok(());
        }
        let Some(next) = self
            .len
            .checked_add(text.len())
            .filter(|&n| n <= self.maximum)
        else {
            self.exceeded = true;
            return Ok(());
        };
        self.len = next;
        if let Some(buffer) = &mut self.text {
            buffer.push_str(text);
        }
        Ok(())
    }
}
struct LimitedCount {
    len: usize,
    maximum: usize,
    exceeded: bool,
}
impl Write for LimitedCount {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let Some(next) = self
            .len
            .checked_add(text.len())
            .filter(|&n| n <= self.maximum)
        else {
            self.exceeded = true;
            return Err(std::fmt::Error);
        };
        self.len = next;
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FailureKind {
    Fuel,
    Overflow,
    Bounds,
}
impl FailureKind {
    fn diagnostic(self, span: Span, sources: &SourceMap) -> Box<Diagnostic> {
        match self {
            Self::Fuel => RunFailure::Fuel(span).diagnostic(sources),
            Self::Overflow => RunFailure::Overflow(span).diagnostic(sources),
            Self::Bounds => execute::OwnedRunFailure::Bounds(span).diagnostic(sources),
        }
    }
}
type DiagnosticKey = (FailureKind, usize, usize, usize);
#[derive(Clone, Copy)]
struct DiagnosticOccurrence {
    key: DiagnosticKey,
    encounter: usize,
    line: usize,
    column: usize,
    rendered_len: usize,
}
impl DiagnosticOccurrence {
    fn span(self) -> Span {
        Span {
            file: crate::frontend::source::SourceFileId(self.key.1),
            start: self.key.2,
            end: self.key.3,
        }
    }
    fn write(
        &self,
        sources: &SourceMap,
        out: &mut impl Write,
        accounting: &mut Accounting,
    ) -> std::fmt::Result {
        let span = self.span();
        let diagnostic = self.key.0.diagnostic(span, sources);
        let transient = size_of::<Diagnostic>() + diagnostic.message.capacity();
        accounting.metrics.diagnostic_transient_peak =
            accounting.metrics.diagnostic_transient_peak.max(transient);
        if transient > DIAGNOSTIC_TRANSIENT_BYTES
            || !diagnostic.secondary.is_empty()
            || !diagnostic.notes.is_empty()
            || diagnostic.primary != (self.line != 0).then_some(span)
        {
            return Err(std::fmt::Error);
        }
        diagnostic.write_human_with_locations(out, |requested| {
            if self.line == 0 || requested != span {
                return Err(std::fmt::Error);
            }
            Ok((sources.get(span.file).path(), self.line, self.column))
        })
    }
}
type DiagnosticLookup = (DiagnosticKey, usize);
struct Diagnostics {
    messages: Vec<String>,
    ids: Vec<DiagnosticLookup>,
}
fn diagnostic_occurrences(
    plan: &ExecutionPlan<'_>,
    entry: hir::DefId,
    guarded: bool,
    mut visit: impl FnMut(FailureKind, Span) -> Result<(), Box<Diagnostic>>,
) -> Result<(), Box<Diagnostic>> {
    if guarded {
        visit(FailureKind::Fuel, plan.witness().functions()[entry.0].span)?;
    }
    for f in plan.witness().functions() {
        for b in &f.blocks {
            if guarded {
                if let Some(m) = &b.merge {
                    visit(FailureKind::Fuel, m.span)?;
                }
            }
            for statement in &b.statements {
                if guarded {
                    visit(FailureKind::Fuel, plan::instruction_span(statement))?;
                }
                match &statement.kind {
                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                        value: Rvalue::CheckedI32 { operator_span, .. },
                        ..
                    })) => visit(FailureKind::Overflow, *operator_span)?,
                    OwnedInstruction::ReadIndex { .. } | OwnedInstruction::WriteIndex { .. } => {
                        visit(FailureKind::Bounds, plan::instruction_span(statement))?
                    }
                    _ => {}
                }
            }
            if guarded {
                visit(
                    FailureKind::Fuel,
                    b.terminator.as_ref().expect("verified terminator").span,
                )?;
            }
        }
    }
    Ok(())
}
impl Diagnostics {
    fn new(
        plan: &ExecutionPlan<'_>,
        entry: hir::DefId,
        sources: &SourceMap,
        guarded: bool,
        maximum: usize,
    ) -> Result<Self, Box<Diagnostic>> {
        Self::new_accounted(
            plan,
            entry,
            sources,
            guarded,
            maximum,
            plan::MAX_PLAN_BYTES,
            &mut Accounting::default(),
        )
    }
    fn new_accounted(
        plan: &ExecutionPlan<'_>,
        entry: hir::DefId,
        sources: &SourceMap,
        guarded: bool,
        maximum: usize,
        metadata_maximum: usize,
        accounting: &mut Accounting,
    ) -> Result<Self, Box<Diagnostic>> {
        let mut count = 0;
        diagnostic_occurrences(plan, entry, guarded, |_, _| {
            count = add(count, 1)?;
            Ok(())
        })?;
        accounting.metrics.occurrences = count;
        accounting.metrics.inventory_rows = count;
        accounting.metrics.row_sizes = [
            size_of::<DiagnosticOccurrence>(),
            size_of::<DiagnosticLookup>(),
            size_of::<String>(),
            size_of::<Bound>(),
        ];
        accounting.metrics.plan_bytes = plan.metadata_bytes();
        accounting.metrics.retained_bound_bytes = mul(plan.functions().len(), size_of::<Bound>())?;
        let retained = add(
            accounting.metrics.plan_bytes,
            accounting.metrics.retained_bound_bytes,
        )?;
        accounting.metrics.metadata_peak = accounting.metrics.metadata_peak.max(retained);
        let occurrence_bytes = mul(count, size_of::<DiagnosticOccurrence>())?;
        accounting.metrics.occurrence_bytes = occurrence_bytes;
        // Gate the conservative U<=K inventory before its first reservation.
        let admitted = add(
            add(retained, occurrence_bytes)?,
            add(
                mul(count, size_of::<DiagnosticLookup>() + size_of::<String>())?,
                DIAGNOSTIC_TRANSIENT_BYTES,
            )?,
        )?;
        accounting.metrics.metadata_admitted_bytes = admitted;
        limit(
            admitted,
            metadata_maximum.min(plan::MAX_PLAN_BYTES),
            "diagnostic metadata bytes",
            plan.witness().functions()[entry.0].span,
        )?;
        let mut rows = accounting.vector(count, "diagnostic occurrences")?;
        accounting.metrics.metadata_peak = accounting
            .metrics
            .metadata_peak
            .max(add(retained, occurrence_bytes)?);
        diagnostic_occurrences(plan, entry, guarded, |kind, span| {
            rows.push(DiagnosticOccurrence {
                key: (kind, span.file.0, span.start, span.end),
                encounter: rows.len(),
                line: 0,
                column: 0,
                rendered_len: 0,
            });
            Ok(())
        })?;
        if rows.len() != count {
            return Err(mismatch("diagnostic occurrence count"));
        }
        rows.sort_unstable_by(|a, b| {
            accounting.metrics.sort_comparisons[0] += 1;
            (a.key, a.encounter).cmp(&(b.key, b.encounter))
        });
        rows.dedup_by_key(|row| row.key);
        let unique = rows.len();
        accounting.metrics.unique = unique;
        rows.sort_unstable_by(|a, b| {
            accounting.metrics.sort_comparisons[1] += 1;
            (a.key.1, a.key.2, a.encounter).cmp(&(b.key.1, b.key.2, b.encounter))
        });
        // Filtering precedes any file lookup. Calculate H before walking bytes;
        // raw caller-owned sources are not subject to a new public-loader cap.
        let mut last: Option<(usize, usize)> = None;
        let mut prefix_bound = 0;
        for row in &mut rows {
            accounting.metrics.prefix_preflight_rows += 1;
            if !sources.is_valid_span(row.span()) {
                continue;
            }
            row.line = 1;
            match last {
                Some((file, _)) if file == row.key.1 => last = Some((file, row.key.2)),
                Some((_, start)) => {
                    prefix_bound = add(prefix_bound, start)?;
                    last = Some((row.key.1, row.key.2));
                }
                None => last = Some((row.key.1, row.key.2)),
            }
        }
        if let Some((_, start)) = last {
            prefix_bound = add(prefix_bound, start)?;
        }
        accounting.metrics.source_prefix_bound = prefix_bound;
        let (mut file, mut offset, mut line, mut column) = (None, 0, 1, 1);
        for row in &mut rows {
            accounting.metrics.coordinate_rows += 1;
            if row.line == 0 {
                continue;
            }
            if file != Some(row.key.1) {
                file = Some(row.key.1);
                (offset, line, column) = (0, 1, 1);
            }
            let source = sources.get(row.span().file);
            for ch in source.text()[offset..row.key.2].chars() {
                accounting.metrics.source_bytes =
                    add(accounting.metrics.source_bytes, ch.len_utf8())?;
                accounting.metrics.source_scalars = add(accounting.metrics.source_scalars, 1)?;
                if accounting.metrics.source_bytes > prefix_bound {
                    return Err(mismatch("diagnostic prefix work"));
                }
                if ch == '\n' {
                    line = add(line, 1)?;
                    column = 1;
                } else {
                    column = add(column, 1)?;
                }
            }
            offset = row.key.2;
            row.line = line;
            row.column = column;
        }
        if accounting.metrics.source_bytes != prefix_bound {
            return Err(mismatch("diagnostic prefix count"));
        }
        rows.sort_unstable_by(|a, b| {
            accounting.metrics.sort_comparisons[2] += 1;
            a.encounter.cmp(&b.encounter)
        });
        let lookup_bytes = mul(unique, size_of::<DiagnosticLookup>())?;
        let header_bytes = mul(unique, size_of::<String>())?;
        accounting.metrics.lookup_bytes = lookup_bytes;
        accounting.metrics.message_header_bytes = header_bytes;
        let metadata = add(
            add(retained, occurrence_bytes)?,
            add(lookup_bytes, header_bytes)?,
        )?;
        limit(
            add(metadata, DIAGNOSTIC_TRANSIENT_BYTES)?,
            metadata_maximum.min(plan::MAX_PLAN_BYTES),
            "diagnostic metadata bytes",
            plan.witness().functions()[entry.0].span,
        )?;
        let mut ids = accounting.vector(unique, "diagnostic lookup")?;
        accounting.metrics.metadata_peak = accounting
            .metrics
            .metadata_peak
            .max(add(add(retained, occurrence_bytes)?, lookup_bytes)?);
        let mut messages = accounting.vector(unique, "diagnostic headers")?;
        accounting.metrics.metadata_peak = accounting.metrics.metadata_peak.max(metadata);
        for (id, row) in rows.iter().enumerate() {
            ids.push((row.key, id));
        }
        ids.sort_unstable_by(|a, b| {
            accounting.metrics.sort_comparisons[3] += 1;
            a.0.cmp(&b.0)
        });
        let mut bytes = LimitedCount {
            len: 0,
            maximum,
            exceeded: false,
        };
        for row in &mut rows {
            let before = bytes.len;
            let result = row.write(sources, &mut bytes, accounting);
            accounting.metrics.message_count_bytes = bytes.len;
            accounting.metrics.metadata_peak = accounting
                .metrics
                .metadata_peak
                .max(add(metadata, accounting.metrics.diagnostic_transient_peak)?);
            result.map_err(|_| {
                if bytes.exceeded {
                    reject(
                        format!("native owned diagnostic bytes limit exceeded ({maximum})"),
                        Some(row.span()),
                    )
                } else {
                    mismatch("diagnostic association")
                }
            })?;
            row.rendered_len = bytes.len - before;
        }
        accounting.metrics.message_count_bytes = bytes.len;
        accounting.metrics.message_bytes = bytes.len;
        for row in &rows {
            let text = accounting.string(row.rendered_len, "diagnostic message")?;
            let mut output = Emission {
                text: Some(text),
                ..Emission::count(row.rendered_len)
            };
            row.write(sources, &mut output, accounting)
                .map_err(|_| mismatch("diagnostic association"))?;
            if output.exceeded || output.len != row.rendered_len {
                return Err(mismatch("diagnostic count/render"));
            }
            accounting.metrics.message_render_bytes =
                add(accounting.metrics.message_render_bytes, output.len)?;
            messages.push(output.text.expect("message render pass"));
        }
        if accounting.metrics.message_render_bytes != bytes.len {
            return Err(mismatch("diagnostic total"));
        }
        accounting.metrics.metadata_peak = accounting
            .metrics
            .metadata_peak
            .max(add(metadata, accounting.metrics.diagnostic_transient_peak)?);
        // Capacity K remains charged until this drop, even after deduplication.
        drop(rows);
        Ok(Self { messages, ids })
    }
    fn get(&self, kind: FailureKind, span: Span) -> (usize, usize) {
        let key = (kind, span.file.0, span.start, span.end);
        let index = self
            .ids
            .binary_search_by_key(&key, |&(key, _)| key)
            .expect("inventoried diagnostic");
        let id = self.ids[index].1;
        (id, self.messages[id].len())
    }
    #[cfg(test)]
    fn contains(&self, kind: FailureKind, span: Span) -> bool {
        self.ids
            .binary_search_by_key(&(kind, span.file.0, span.start, span.end), |&(key, _)| key)
            .is_ok()
    }
}
fn emit_failure(out: &mut Emission, diagnostics: &Diagnostics, kind: FailureKind, span: Span) {
    let (id, len) = diagnostics.get(kind, span);
    writeln!(
        out,
        "  call void @__oxid_overflow(ptr @__oxid_owned_error_{id}, i64 {len})\n  unreachable"
    )
    .unwrap();
}
fn emit_guard(out: &mut Emission, diagnostics: &Diagnostics, name: &str, cost: usize, span: Span) {
    writeln!(out, "  %{name}_remaining = load i64, ptr %fuel\n  %{name}_exhausted = icmp ult i64 %{name}_remaining, {cost}\n  br i1 %{name}_exhausted, label %{name}_error, label %{name}_ok\n{name}_error:").unwrap();
    emit_failure(out, diagnostics, FailureKind::Fuel, span);
    writeln!(out, "{name}_ok:\n  %{name}_next = sub i64 %{name}_remaining, {cost}\n  store i64 %{name}_next, ptr %fuel").unwrap();
}
fn ty(t: hir::Ty) -> &'static str {
    match t {
        hir::Ty::Bool => "i1",
        hir::Ty::I32 => "i32",
        hir::Ty::Unit => "i8",
    }
}
fn result_ty(t: ValueTy) -> &'static str {
    match t {
        ValueTy::Scalar(t) => ty(t),
        ValueTy::Owned(_) => "void",
    }
}
fn load_slot(out: &mut Emission, name: &str, pointer: &str, t: hir::Ty) -> String {
    writeln!(out, "  %{name}_wide = load i64, ptr {pointer}, align 8\n  %{name} = trunc i64 %{name}_wide to {}", ty(t)).unwrap();
    format!("%{name}")
}
fn store_slot(out: &mut Emission, name: &str, pointer: &str, t: hir::Ty, value: &str) {
    writeln!(
        out,
        "  %{name}_wide = zext {} {value} to i64\n  store i64 %{name}_wide, ptr {pointer}, align 8",
        ty(t)
    )
    .unwrap();
}
fn load_operand(out: &mut Emission, f: &RawOwnedFunction, name: &str, value: Operand) -> String {
    load_slot(
        out,
        name,
        &format!("%s{}", value.local.0),
        f.locals[value.local.0].ty,
    )
}
fn base_pointer(out: &mut Emission, name: &str, base: AccessBase) -> String {
    match base {
        AccessBase::Owner(o) => format!("%o{}", o.0),
        AccessBase::Parameter(p) => {
            writeln!(out, "  %{name}_base = load ptr, ptr %r{}, align 8", p.0).unwrap();
            format!("%{name}_base")
        }
    }
}
fn field_pointer(out: &mut Emission, name: &str, base: &str, offset: usize) -> String {
    writeln!(
        out,
        "  %{name}_ptr = getelementptr i8, ptr {base}, i64 {offset}"
    )
    .unwrap();
    format!("%{name}_ptr")
}
fn transfer(
    out: &mut Emission,
    plan: &ExecutionPlan<'_>,
    name: &str,
    aggregate: AggregateTy,
    source: &str,
    destination: &str,
) {
    if out.exceeded {
        return;
    }
    let AggregateTy::Record(record) = aggregate else {
        unreachable!("array carriers cannot obtain a Unit2A witness");
    };
    let fields = plan
        .witness()
        .declarations()
        .fields(record)
        .expect("verified record");
    if fields.is_empty() {
        writeln!(out, "  %{name}_empty = load i8, ptr {source}, align 1\n  store i8 %{name}_empty, ptr {destination}, align 1").unwrap();
    }
    for (i, field) in fields.iter().enumerate() {
        if out.exceeded {
            return;
        }
        #[cfg(test)]
        {
            out.field_visits += 1;
        }
        let input = field_pointer(out, &format!("{name}_in{i}"), source, field.offset());
        let output = field_pointer(out, &format!("{name}_out{i}"), destination, field.offset());
        // Field-wise transfer deliberately never reads record padding.
        writeln!(out, "  %{name}_field{i} = load {}, ptr {input}, align 1\n  store {} %{name}_field{i}, ptr {output}, align 1", ty(field.ty()), ty(field.ty())).unwrap();
    }
}

fn emit(
    plan: &ExecutionPlan<'_>,
    entry: hir::DefId,
    diagnostics: &Diagnostics,
    guarded: bool,
    fuel: usize,
    out: &mut Emission,
) {
    out.write_str("; Oxid private owned native ABI 1\nsource_filename = \"oxid-owned-native\"\ntarget triple = \"x86_64-unknown-linux-gnu\"\n\ndeclare i32 @__oxid_print_bool(i32)\ndeclare i32 @__oxid_print_i32(i32)\ndeclare i32 @__oxid_print_unit()\ndeclare void @__oxid_overflow(ptr, i64) noreturn\ndeclare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32)\ndeclare { i32, i1 } @llvm.ssub.with.overflow.i32(i32, i32)\ndeclare { i32, i1 } @llvm.smul.with.overflow.i32(i32, i32)\n").unwrap();
    for (id, message) in diagnostics.messages.iter().enumerate() {
        if out.exceeded {
            return;
        }
        write!(
            out,
            "@__oxid_owned_error_{id} = private unnamed_addr constant [{} x i8] c\"",
            message.len()
        )
        .unwrap();
        for byte in message.bytes() {
            if out.exceeded {
                return;
            }
            write!(out, "\\{byte:02X}").unwrap();
        }
        writeln!(out, "\"").unwrap();
    }
    for f in plan.witness().functions() {
        if out.exceeded {
            return;
        }
        emit_function(plan, f.id, diagnostics, guarded, out);
    }
    if out.exceeded {
        return;
    }
    let root = &plan.witness().functions()[entry.0];
    let ValueTy::Scalar(result) = root.result else {
        unreachable!("entry gate")
    };
    out.write_str("\ndefine i32 @main() {\nentry:\n").unwrap();
    if guarded {
        writeln!(
            out,
            "  %fuel = alloca i64, align 8\n  store i64 {fuel}, ptr %fuel, align 8"
        )
        .unwrap();
        emit_guard(
            out,
            diagnostics,
            "root",
            1 + plan.function(entry).usage().expanded_cells,
            root.span,
        );
    }
    writeln!(
        out,
        "  %value = call {} @__oxid_owned_fn_{}({})",
        ty(result),
        entry.0,
        if guarded { "ptr %fuel" } else { "" }
    )
    .unwrap();
    match result {
        hir::Ty::Bool => out.write_str("  %wide = zext i1 %value to i32\n  %status = call i32 @__oxid_print_bool(i32 %wide)\n").unwrap(),
        hir::Ty::I32 => out.write_str("  %status = call i32 @__oxid_print_i32(i32 %value)\n").unwrap(),
        hir::Ty::Unit => out.write_str("  %status = call i32 @__oxid_print_unit()\n").unwrap(),
    }
    out.write_str("  ret i32 %status\n}\n").unwrap();
}

fn exit_label(f: &RawOwnedFunction, block: usize, guarded: bool) -> String {
    if guarded {
        return format!(
            "f{}_b{block}_g{}_ok",
            f.id.0,
            f.blocks[block].statements.len() + 1
        );
    }
    f.blocks[block]
        .statements
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, s)| {
            matches!(
                &s.kind,
                OwnedInstruction::Scalar(Statement::Assign(Assign {
                    value: Rvalue::CheckedI32 { .. },
                    ..
                }))
            )
            .then(|| format!("f{}_b{block}_i{i}_checked_ok", f.id.0))
        })
        .unwrap_or_else(|| format!("b{block}"))
}
fn emit_function(
    plan: &ExecutionPlan<'_>,
    id: hir::DefId,
    diagnostics: &Diagnostics,
    guarded: bool,
    out: &mut Emission,
) {
    if out.exceeded {
        return;
    }
    let f = &plan.witness().functions()[id.0];
    let fp = plan.function(id);
    let u = fp.usage();
    write!(
        out,
        "\ndefine internal {} @__oxid_owned_fn_{}(",
        result_ty(f.result),
        id.0
    )
    .unwrap();
    let mut separator = "";
    if guarded {
        out.write_str("ptr %fuel").unwrap();
        separator = ", ";
    }
    if matches!(f.result, ValueTy::Owned(_)) {
        write!(out, "{separator}ptr %result").unwrap();
        separator = ", ";
    }
    for (i, parameter) in f.parameters.iter().enumerate() {
        if out.exceeded {
            return;
        }
        let t = match parameter {
            ParameterBinding::Scalar(l) => ty(f.locals[l.0].ty),
            _ => "ptr",
        };
        write!(out, "{separator}{t} %arg{i}").unwrap();
        separator = ", ";
    }
    out.write_str(") noinline {\nentry:\n").unwrap();
    let slots = u.scalar_slots + u.arguments;
    if slots != 0 {
        writeln!(out, "  %scalars = alloca [{slots} x i64], align 8").unwrap();
    }
    // Round the byte alloca itself to the four-byte size charged in Dnative.
    if u.payload_bytes != 0 {
        writeln!(
            out,
            "  %owners = alloca [{} x i8], align 4",
            u.payload_bytes.div_ceil(4) * 4
        )
        .unwrap();
    }
    if u.references + u.loans != 0 {
        writeln!(
            out,
            "  %references = alloca [{} x ptr], align 8",
            u.references + u.loans
        )
        .unwrap();
    }
    for i in 0..slots {
        if out.exceeded {
            return;
        }
        writeln!(
            out,
            "  %s{i} = getelementptr i8, ptr %scalars, i64 {}",
            i * 8
        )
        .unwrap();
    }
    for i in 0..u.owners {
        if out.exceeded {
            return;
        }
        writeln!(
            out,
            "  %o{i} = getelementptr i8, ptr %owners, i64 {}",
            fp.owner_offset(OwnerPlaceId(i))
        )
        .unwrap();
    }
    for i in 0..u.references + u.loans {
        if out.exceeded {
            return;
        }
        writeln!(
            out,
            "  %r{i} = getelementptr i8, ptr %references, i64 {}",
            i * 8
        )
        .unwrap();
    }
    for (i, parameter) in f.parameters.iter().enumerate() {
        if out.exceeded {
            return;
        }
        let name = format!("f{}_param{i}", id.0);
        match parameter {
            ParameterBinding::Scalar(l) => store_slot(
                out,
                &name,
                &format!("%s{}", l.0),
                f.locals[l.0].ty,
                &format!("%arg{i}"),
            ),
            ParameterBinding::Reference(r) => {
                writeln!(out, "  store ptr %arg{i}, ptr %r{}, align 8", r.0).unwrap()
            }
            ParameterBinding::Owned(o) => transfer(
                out,
                plan,
                &name,
                f.owners[o.0].aggregate(),
                &format!("%arg{i}"),
                &format!("%o{}", o.0),
            ),
        }
    }
    writeln!(out, "  br label %b{}", f.entry.0).unwrap();
    for (b, block) in f.blocks.iter().enumerate() {
        if out.exceeded {
            return;
        }
        writeln!(out, "b{b}:").unwrap();
        if let Some(m) = &block.merge {
            let name = format!("f{}_b{b}_merge", id.0);
            let [left, right] = m.incoming;
            // Select storage, then read exactly the taken input after charging.
            // A value phi would read an uninitialized slot on the untaken arm.
            writeln!(
                out,
                "  %{name}_slot = phi ptr [ %s{}, %{} ], [ %s{}, %{} ]",
                left.value.local.0,
                exit_label(f, left.predecessor.0, guarded),
                right.value.local.0,
                exit_label(f, right.predecessor.0, guarded)
            )
            .unwrap();
            if guarded {
                emit_guard(out, diagnostics, &format!("f{}_b{b}_g0", id.0), 1, m.span);
            }
            let value = load_slot(out, &name, &format!("%{name}_slot"), hir::Ty::Bool);
            store_slot(
                out,
                &format!("{name}_store"),
                &format!("%s{}", m.destination.0),
                hir::Ty::Bool,
                &value,
            );
        }
        for (i, statement) in block.statements.iter().enumerate() {
            if out.exceeded {
                return;
            }
            let name = format!("f{}_b{b}_i{i}", id.0);
            if guarded {
                emit_guard(
                    out,
                    diagnostics,
                    &format!("f{}_b{b}_g{}", id.0, i + 1),
                    plan.statement_cost(id, &statement.kind),
                    plan::instruction_span(statement),
                );
            }
            emit_statement(plan, id, &name, statement, diagnostics, out);
        }
        if out.exceeded {
            return;
        }
        let term = block.terminator.as_ref().expect("verified terminator");
        if guarded {
            emit_guard(
                out,
                diagnostics,
                &format!("f{}_b{b}_g{}", id.0, block.statements.len() + 1),
                plan.terminator_cost(id, &term.kind),
                term.span,
            );
        }
        emit_terminator(
            plan,
            id,
            &format!("f{}_b{b}_term", id.0),
            &term.kind,
            guarded,
            out,
        );
    }
    out.write_str("}\n").unwrap();
}

fn emit_statement(
    plan: &ExecutionPlan<'_>,
    id: hir::DefId,
    name: &str,
    statement: &OwnedStatement,
    diagnostics: &Diagnostics,
    out: &mut Emission,
) {
    if out.exceeded {
        return;
    }
    let f = &plan.witness().functions()[id.0];
    let fp = plan.function(id);
    match &statement.kind {
        OwnedInstruction::ConstructArray { .. }
        | OwnedInstruction::ReadIndex { .. }
        | OwnedInstruction::WriteIndex { .. }
        | OwnedInstruction::ArrayLength { .. } => {
            unreachable!("production array admission is closed until both consumers are complete")
        }
        OwnedInstruction::Scalar(s) => emit_scalar(plan, id, name, s, diagnostics, out),
        OwnedInstruction::Construct {
            destination,
            fields,
        } => {
            if fields.is_empty() {
                writeln!(out, "  store i8 0, ptr %o{}, align 1", destination.0).unwrap();
            }
            for (i, (field, value)) in fields.iter().enumerate() {
                if out.exceeded {
                    return;
                }
                #[cfg(test)]
                {
                    out.field_visits += 1;
                }
                let field = plan
                    .witness()
                    .declarations()
                    .field(field.record, *field)
                    .expect("verified field");
                let n = format!("{name}_field{i}");
                let value = load_operand(out, f, &format!("{n}_value"), *value);
                let ptr = field_pointer(out, &n, &format!("%o{}", destination.0), field.offset());
                writeln!(
                    out,
                    "  store {} {value}, ptr {ptr}, align 1",
                    ty(field.ty())
                )
                .unwrap();
            }
        }
        OwnedInstruction::MoveInitialize {
            destination,
            source,
        }
        | OwnedInstruction::Replace {
            destination,
            source,
        } => {
            transfer(
                out,
                plan,
                name,
                f.owners[source.0].aggregate(),
                &format!("%o{}", source.0),
                &format!("%o{}", destination.0),
            );
        }
        OwnedInstruction::ReadField {
            destination,
            base,
            field,
        } => {
            let field = plan
                .witness()
                .declarations()
                .field(field.record, *field)
                .expect("verified field");
            let base = base_pointer(out, name, *base);
            let ptr = field_pointer(out, name, &base, field.offset());
            writeln!(
                out,
                "  %{name}_value = load {}, ptr {ptr}, align 1",
                ty(field.ty())
            )
            .unwrap();
            store_slot(
                out,
                &format!("{name}_store"),
                &format!("%s{}", destination.0),
                field.ty(),
                &format!("%{name}_value"),
            );
        }
        OwnedInstruction::WriteField { base, field, value } => {
            let field = plan
                .witness()
                .declarations()
                .field(field.record, *field)
                .expect("verified field");
            let value = load_operand(out, f, &format!("{name}_value"), *value);
            let base = base_pointer(out, name, *base);
            let ptr = field_pointer(out, name, &base, field.offset());
            writeln!(
                out,
                "  store {} {value}, ptr {ptr}, align 1",
                ty(field.ty())
            )
            .unwrap();
        }
        OwnedInstruction::PrepareScalar {
            call,
            argument,
            value,
        } => {
            let t = f.locals[value.local.0].ty;
            let value = load_operand(out, f, &format!("{name}_value"), *value);
            let slot = fp.usage().scalar_slots + fp.call(*call).argument_start() + argument;
            store_slot(
                out,
                &format!("{name}_store"),
                &format!("%s{slot}"),
                t,
                &value,
            );
        }
        OwnedInstruction::PrepareOwned {
            call,
            argument,
            source,
        } => {
            let ArgumentSlot::Owned(destination) = f.calls[call.0].arguments[*argument] else {
                unreachable!("verified owned argument")
            };
            transfer(
                out,
                plan,
                name,
                f.owners[source.0].aggregate(),
                &format!("%o{}", source.0),
                &format!("%o{}", destination.0),
            );
        }
        OwnedInstruction::PrepareBorrow { loan, .. } => {
            let pointer = base_pointer(out, name, f.loans[loan.0].authority);
            writeln!(
                out,
                "  store ptr {pointer}, ptr %r{}, align 8",
                f.references.len() + loan.0
            )
            .unwrap();
        }
        // The witness proves these logical transitions. Backing storage remains
        // allocated throughout the activation, including suspended staging.
        OwnedInstruction::StorageLive(_)
        | OwnedInstruction::StorageEnd(_)
        | OwnedInstruction::Discard(_)
        | OwnedInstruction::OpenCall(_) => {}
    }
}
fn emit_scalar(
    plan: &ExecutionPlan<'_>,
    id: hir::DefId,
    name: &str,
    statement: &Statement,
    diagnostics: &Diagnostics,
    out: &mut Emission,
) {
    if out.exceeded {
        return;
    }
    let f = &plan.witness().functions()[id.0];
    let assign = match statement {
        Statement::Assign(a) => a,
        Statement::Initialize { place, value, .. } | Statement::Store { place, value, .. } => {
            let value = load_operand(out, f, &format!("{name}_value"), *value);
            store_slot(
                out,
                &format!("{name}_store"),
                &format!("%s{}", f.locals.len() + place.id.0),
                f.places[place.id.0].ty,
                &value,
            );
            return;
        }
    };
    let t = f.locals[assign.destination.0].ty;
    let value = match assign.value {
        Rvalue::Bool(v) => if v { "true" } else { "false" }.to_string(),
        Rvalue::I32(v) => v.to_string(),
        Rvalue::Unit => "0".into(),
        Rvalue::Copy(v) => load_operand(out, f, &format!("{name}_value"), v),
        Rvalue::Load(p) => load_slot(
            out,
            &format!("{name}_value"),
            &format!("%s{}", f.locals.len() + p.id.0),
            t,
        ),
        Rvalue::NotBool { operand, .. } => {
            let operand = load_operand(out, f, &format!("{name}_operand"), operand);
            writeln!(out, "  %{name}_value = xor i1 {operand}, true").unwrap();
            format!("%{name}_value")
        }
        Rvalue::CompareScalar {
            op, left, right, ..
        } => {
            let operand_type = ty(f.locals[left.local.0].ty);
            let left = load_operand(out, f, &format!("{name}_left"), left);
            let right = load_operand(out, f, &format!("{name}_right"), right);
            let predicate = match op {
                hir::ComparisonOp::Equal => "eq",
                hir::ComparisonOp::NotEqual => "ne",
                hir::ComparisonOp::Less => "slt",
                hir::ComparisonOp::LessEqual => "sle",
                hir::ComparisonOp::Greater => "sgt",
                hir::ComparisonOp::GreaterEqual => "sge",
            };
            writeln!(
                out,
                "  %{name}_value = icmp {predicate} {operand_type} {left}, {right}"
            )
            .unwrap();
            format!("%{name}_value")
        }
        Rvalue::CheckedI32 {
            op,
            left,
            right,
            operator_span,
        } => {
            let left = load_operand(out, f, &format!("{name}_left"), left);
            let right = load_operand(out, f, &format!("{name}_right"), right);
            let intrinsic = match op {
                hir::ArithmeticOp::Add => "sadd",
                hir::ArithmeticOp::Subtract => "ssub",
                hir::ArithmeticOp::Multiply => "smul",
            };
            writeln!(out, "  %{name}_checked = call {{ i32, i1 }} @llvm.{intrinsic}.with.overflow.i32(i32 {left}, i32 {right})\n  %{name}_overflow = extractvalue {{ i32, i1 }} %{name}_checked, 1\n  br i1 %{name}_overflow, label %{name}_checked_error, label %{name}_checked_ok\n{name}_checked_error:").unwrap();
            emit_failure(out, diagnostics, FailureKind::Overflow, operator_span);
            writeln!(out, "{name}_checked_ok:\n  %{name}_value = extractvalue {{ i32, i1 }} %{name}_checked, 0").unwrap();
            format!("%{name}_value")
        }
    };
    store_slot(
        out,
        &format!("{name}_store"),
        &format!("%s{}", assign.destination.0),
        t,
        &value,
    );
}
fn emit_terminator(
    plan: &ExecutionPlan<'_>,
    id: hir::DefId,
    name: &str,
    term: &OwnedTerminatorKind,
    guarded: bool,
    out: &mut Emission,
) {
    if out.exceeded {
        return;
    }
    let f = &plan.witness().functions()[id.0];
    match term {
        OwnedTerminatorKind::Goto(target) => writeln!(out, "  br label %b{}", target.0).unwrap(),
        OwnedTerminatorKind::Branch {
            condition,
            then_block,
            else_block,
        } => {
            let condition = load_operand(out, f, &format!("{name}_condition"), *condition);
            writeln!(
                out,
                "  br i1 {condition}, label %b{}, label %b{}",
                then_block.0, else_block.0
            )
            .unwrap();
        }
        OwnedTerminatorKind::ReturnScalar(value) => {
            let t = ty(f.locals[value.local.0].ty);
            let value = load_operand(out, f, &format!("{name}_value"), *value);
            writeln!(out, "  ret {t} {value}").unwrap();
        }
        OwnedTerminatorKind::ReturnOwned(owner) => {
            transfer(
                out,
                plan,
                name,
                f.owners[owner.0].aggregate(),
                &format!("%o{}", owner.0),
                "%result",
            );
            out.write_str("  ret void\n").unwrap();
        }
        OwnedTerminatorKind::Invoke { call, continuation } => {
            let descriptor = &f.calls[call.0];
            let callee = &plan.witness().functions()[descriptor.target.0];
            // Bounded by MAX_PARAMS; argument strings are emission scratch only,
            // never runtime A-sized scalar or owned result scratch.
            let mut args = Vec::with_capacity(descriptor.arguments.len() + 2);
            if guarded {
                args.push("ptr %fuel".into());
            }
            if let CallResult::Owned(o) = descriptor.result {
                args.push(format!("ptr %o{}", o.0));
            }
            for (i, argument) in descriptor.arguments.iter().enumerate() {
                if out.exceeded {
                    return;
                }
                match argument {
                    ArgumentSlot::Scalar => {
                        let ParameterBinding::Scalar(l) = callee.parameters[i] else {
                            unreachable!("verified scalar parameter")
                        };
                        let t = callee.locals[l.0].ty;
                        let slot = plan.function(id).usage().scalar_slots
                            + plan.function(id).call(*call).argument_start()
                            + i;
                        let value =
                            load_slot(out, &format!("{name}_arg{i}"), &format!("%s{slot}"), t);
                        args.push(format!("{} {value}", ty(t)));
                    }
                    ArgumentSlot::Owned(o) => args.push(format!("ptr %o{}", o.0)),
                    ArgumentSlot::Borrow(l) => {
                        writeln!(
                            out,
                            "  %{name}_arg{i} = load ptr, ptr %r{}, align 8",
                            f.references.len() + l.0
                        )
                        .unwrap();
                        args.push(format!("ptr %{name}_arg{i}"));
                    }
                }
            }
            if out.exceeded {
                return;
            }
            let prefix = match descriptor.result {
                CallResult::Scalar(_) => format!("%{name}_result = "),
                CallResult::Owned(_) => String::new(),
            };
            let joined = args.join(", ");
            // This inherited, parameter-bounded collection is gone before the
            // next operation. Fixed nested names are covered separately.
            let scratch = args.capacity() * size_of::<String>()
                + args.iter().map(String::capacity).sum::<usize>()
                + joined.capacity()
                + prefix.capacity();
            out.call_scratch_peak = out.call_scratch_peak.max(scratch);
            writeln!(
                out,
                "  {prefix}call {} @__oxid_owned_fn_{}({})",
                result_ty(callee.result),
                descriptor.target.0,
                joined
            )
            .unwrap();
            if let CallResult::Scalar(destination) = descriptor.result {
                store_slot(
                    out,
                    &format!("{name}_store"),
                    &format!("%s{}", destination.0),
                    f.locals[destination.0].ty,
                    &format!("%{name}_result"),
                );
            }
            writeln!(out, "  br label %b{}", continuation.0).unwrap();
        }
    }
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
